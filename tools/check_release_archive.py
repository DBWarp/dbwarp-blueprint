#!/usr/bin/env python3
"""Validate DBWarp Blueprint release archives without extracting them."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import stat
import sys
import tarfile
import tomllib
import zipfile
from collections import Counter
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any


COMMON_REQUIRED = {
    "AUDIT.md",
    "AUTH.md",
    "DECK.md",
    "FORMAT.md",
    "MACHINE_TRANSLATIONS.md",
    "README.md",
    "SECURITY.md",
    "STATUS.md",
    "BUILD.md",
    "TLS.md",
    "THIRD_PARTY_NOTICES.md",
    "LICENSE-APACHE",
    "LICENSE-MIT",
    "binaries/README.md",
    "blueprint_format.py",
    "verify.sh",
    "docs/ARTIFACT_INVENTORY.md",
    "docs/BATCH_AND_BUNDLES.md",
    "docs/COMPRESSION_MEASUREMENT.md",
    "docs/COOKBOOK.md",
    "docs/DBA_REVIEW_GUIDE.md",
    "docs/INDEX.md",
    "docs/INTERNATIONALISATION.md",
    "docs/MESSAGES.md",
    "docs/QUICKSTART.md",
    "docs/STRUCTURED_FILES.md",
    "docs/TRANSLATIONS.md",
    "docs/TROUBLESHOOTING.md",
    "samples/ecommerce-large.toml",
    "samples/erp-enterprise.toml",
    "samples/saas-medium.toml",
    "sql/blueprint.mysql.sql",
    "sql/blueprint.pg.sql",
    "sql/blueprint.sqlserver.sql",
    "sql/grants/README.md",
    "sql/grants/DATABASE_PERMISSIONS.md",
    "sql/grants/mysql/basic.sql",
    "sql/grants/mysql/standard.sql",
    "sql/grants/mysql/enhanced.sql",
    "sql/grants/postgresql/basic.sql",
    "sql/grants/postgresql/standard.sql",
    "sql/grants/postgresql/enhanced.sql",
    "sql/grants/sqlserver-2019/basic.sql",
    "sql/grants/sqlserver-2019/standard.sql",
    "sql/grants/sqlserver-2019/enhanced.sql",
    "sql/grants/sqlserver-2022/basic.sql",
    "sql/grants/sqlserver-2022/standard.sql",
    "sql/grants/sqlserver-2022/enhanced.sql",
    "sql/revoke/mysql.sql",
    "sql/revoke/postgresql.sql",
    "sql/revoke/sqlserver-2019.sql",
    "sql/revoke/sqlserver-2022.sql",
    "assets/fonts/dm-sans/OFL.txt",
    "licenses/mysql_async/LICENSE-APACHE",
    "licenses/mysql_async/LICENSE-MIT",
    "licenses/mysql_async/MODIFICATIONS.md",
    "third-party-licenses/MANIFEST.json",
    "SBOM.cdx.json",
    "PROVENANCE.json",
}

TRANSLATED_LOCALES = ("de", "fr", "es", "pl", "ja", "zh")
TRANSLATED_DOCUMENTS = {
    "ARTIFACT_INVENTORY.md",
    "AUDIT.md",
    "AUTH.md",
    "BATCH_AND_BUNDLES.md",
    "BINARIES.md",
    "BUILD.md",
    "COMPRESSION_MEASUREMENT.md",
    "COOKBOOK.md",
    "DBA_REVIEW_GUIDE.md",
    "DECK.md",
    "FORMAT.md",
    "INDEX.md",
    "INTERNATIONALISATION.md",
    "MESSAGES.md",
    "QUICKSTART.md",
    "README.md",
    "SECURITY.md",
    "STRUCTURED_FILES.md",
    "TLS.md",
    "TROUBLESHOOTING.md",
}
SUPPLEMENTAL_TRANSLATIONS = {
    f"docs/{locale}/{document}"
    for locale in TRANSLATED_LOCALES
    for document in TRANSLATED_DOCUMENTS
}
OPERATOR_ALLOWED = COMMON_REQUIRED | SUPPLEMENTAL_TRANSLATIONS

# These operator-facing subtrees are intentionally closed allowlists. The
# internal checkout also contains planning notes and translation-maintenance
# records; an archive must fail if broad directory copying adds either class.
OPERATOR_SUBTREE_PREFIXES = ("docs/", "samples/", "sql/")

TARGETS = {
    "dbwarp-blueprint-linux-x86_64": ("x86_64-unknown-linux-gnu", [], "dbwarp-blueprint"),
    "dbwarp-blueprint-linux-arm64": ("aarch64-unknown-linux-gnu", [], "dbwarp-blueprint"),
    "dbwarp-blueprint-macos-arm64": ("aarch64-apple-darwin", [], "dbwarp-blueprint"),
    "dbwarp-blueprint-windows-x86_64": (
        "x86_64-pc-windows-msvc",
        ["winauth"],
        "dbwarp-blueprint.exe",
    ),
}


@dataclass(frozen=True)
class ArchiveMember:
    name: str
    is_dir: bool
    mode: int | None
    data: bytes | None


def sha256_bytes(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def archive_top_level(path: Path, kind: str) -> str:
    if kind == "source":
        return "dbwarp-blueprint-source-vendored"
    name = path.name
    for suffix in (".tar.gz", ".tgz", ".tar.xz", ".tar", ".zip"):
        if name.endswith(suffix):
            return name[: -len(suffix)]
    return path.stem


def read_archive(path: Path) -> tuple[list[ArchiveMember], list[str]]:
    unsafe_types: list[str] = []
    members: list[ArchiveMember] = []
    if zipfile.is_zipfile(path):
        with zipfile.ZipFile(path) as archive:
            for item in archive.infolist():
                mode = item.external_attr >> 16 if item.create_system == 3 else None
                is_dir = item.is_dir()
                if mode and not (stat.S_ISREG(mode) or stat.S_ISDIR(mode)):
                    unsafe_types.append(item.filename)
                data = None if is_dir else archive.read(item)
                members.append(ArchiveMember(item.filename, is_dir, mode, data))
        return members, unsafe_types

    with tarfile.open(path, "r:*") as archive:
        for item in archive.getmembers():
            if not (item.isfile() or item.isdir()):
                unsafe_types.append(item.name)
            data = None
            if item.isfile():
                handle = archive.extractfile(item)
                if handle is None:
                    unsafe_types.append(item.name)
                else:
                    data = handle.read()
            members.append(ArchiveMember(item.name, item.isdir(), item.mode, data))
    return members, unsafe_types


def normalize(name: str) -> PurePosixPath:
    while name.startswith("./"):
        name = name[2:]
    return PurePosixPath(name)


def require_json(
    files: dict[str, ArchiveMember], name: str, failures: list[str]
) -> dict[str, Any] | None:
    member = files.get(name)
    if member is None or member.data is None:
        return None
    try:
        value = json.loads(member.data.decode("utf-8-sig"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        failures.append(f"invalid JSON in {name}: {exc}")
        return None
    if not isinstance(value, dict):
        failures.append(f"{name} must contain a JSON object")
        return None
    return value


def validate_legal_bundle(
    files: dict[str, ArchiveMember], kind: str, failures: list[str]
) -> None:
    apache = files.get("LICENSE-APACHE")
    if apache is not None and apache.data is not None:
        try:
            text = apache.data.decode("utf-8")
        except UnicodeDecodeError:
            failures.append("LICENSE-APACHE must be UTF-8 text")
        else:
            if len(apache.data) < 10_000 or "3. Grant of Patent License." not in text:
                failures.append("LICENSE-APACHE is not the complete Apache License 2.0 text")

    manifest = require_json(files, "third-party-licenses/MANIFEST.json", failures)
    if manifest is None:
        return
    if manifest.get("schema") != "dbwarp-blueprint-third-party-licenses/v1":
        failures.append("third-party licence manifest has an invalid schema")
    packages = manifest.get("packages")
    if not isinstance(packages, list):
        failures.append("third-party licence manifest packages must be a list")
        return
    if manifest.get("third_party_package_count") != len(packages):
        failures.append("third-party licence manifest package count does not reconcile")
    expected = {
        "third-party-licenses/MANIFEST.json",
        "third-party-licenses/.generated-by-dbwarp-blueprint",
    }
    for package in packages:
        if not isinstance(package, dict):
            failures.append("third-party licence manifest contains a non-object package")
            continue
        name, version, notices = package.get("name"), package.get("version"), package.get("files")
        if not isinstance(name, str) or not re.fullmatch(r"[A-Za-z0-9._+-]+", name):
            failures.append("third-party licence package has an invalid name")
            continue
        if not isinstance(version, str) or not re.fullmatch(r"[A-Za-z0-9._+-]+", version):
            failures.append(f"third-party licence package {name} has an invalid version")
            continue
        if not isinstance(notices, list) or not notices:
            failures.append(f"third-party licence package {name}-{version} has no notice files")
            continue
        for notice in notices:
            if not isinstance(notice, dict):
                failures.append(f"third-party licence package {name}-{version} has an invalid notice")
                continue
            relative, digest = notice.get("path"), notice.get("sha256")
            if (
                not isinstance(relative, str)
                or not relative
                or "\\" in relative
                or PurePosixPath(relative).is_absolute()
                or ".." in PurePosixPath(relative).parts
            ):
                failures.append(f"third-party licence package {name}-{version} has an unsafe notice path")
                continue
            archive_name = f"third-party-licenses/crates/{name}-{version}/{relative}"
            expected.add(archive_name)
            member = files.get(archive_name)
            if member is None or member.data is None:
                failures.append(f"third-party licence notice is missing: {archive_name}")
            elif not isinstance(digest, str) or sha256_bytes(member.data) != digest:
                failures.append(f"third-party licence notice hash mismatch: {archive_name}")
    present = {name for name in files if name.startswith("third-party-licenses/")}
    for extra in sorted(present - expected):
        failures.append(f"unmanifested third-party licence file: {extra}")


def validate_provenance(
    provenance: dict[str, Any],
    *,
    kind: str,
    expected_revision: str | None,
    expected_target: str,
    expected_features: list[str],
    binary_name: str | None,
    files: dict[str, ArchiveMember],
    failures: list[str],
) -> str | None:
    exact = {
        "schema": "dbwarp-blueprint-release-provenance/v1",
        "artifact_kind": kind,
        "repository": "DBWarp/dbwarp-blueprint",
        "target": expected_target,
        "cargo_lock": "locked",
    }
    for field, expected in exact.items():
        if provenance.get(field) != expected:
            failures.append(
                f"PROVENANCE.json {field} is {provenance.get(field)!r}; expected {expected!r}"
            )
    revision = provenance.get("source_revision")
    if not isinstance(revision, str) or not re.fullmatch(r"[0-9a-f]{40}", revision):
        failures.append("PROVENANCE.json source_revision must be a lowercase 40-hex Git SHA")
        revision = None
    elif expected_revision is not None and revision != expected_revision:
        failures.append(
            f"PROVENANCE.json source_revision {revision} does not match expected {expected_revision}"
        )
    if not isinstance(provenance.get("source_ref"), str) or not provenance["source_ref"]:
        failures.append("PROVENANCE.json source_ref must be a non-empty string")
    if not isinstance(provenance.get("rust_toolchain"), str) or not re.fullmatch(
        r"\d+\.\d+\.\d+", provenance.get("rust_toolchain", "")
    ):
        failures.append("PROVENANCE.json rust_toolchain must be an exact stable version")
    if provenance.get("features") != expected_features:
        failures.append(
            f"PROVENANCE.json features are {provenance.get('features')!r}; "
            f"expected {expected_features!r}"
        )

    binary = provenance.get("binary")
    if kind == "source":
        if binary is not None:
            failures.append("source PROVENANCE.json must not contain binary metadata")
    elif not isinstance(binary, dict):
        failures.append("binary PROVENANCE.json has no binary object")
    else:
        source_date_epoch = provenance.get("source_date_epoch")
        if not isinstance(source_date_epoch, int) or isinstance(source_date_epoch, bool):
            failures.append("binary PROVENANCE.json source_date_epoch must be an integer")
        compiler = provenance.get("native_dependency_compiler")
        if not isinstance(compiler, str) or not compiler.strip():
            failures.append(
                "binary PROVENANCE.json native_dependency_compiler must be non-empty"
            )
    if isinstance(binary, dict) and binary_name is not None:
        member = files.get(binary_name)
        if member is None or member.data is None:
            failures.append(f"binary payload is missing: {binary_name}")
        else:
            if binary.get("name") != binary_name:
                failures.append("PROVENANCE.json binary name does not match the archive payload")
            if binary.get("sha256") != sha256_bytes(member.data):
                failures.append("PROVENANCE.json binary sha256 does not match the archive payload")
            if binary.get("size_bytes") != len(member.data):
                failures.append("PROVENANCE.json binary size_bytes does not match the archive payload")
    return revision


def validate_sbom(
    sbom: dict[str, Any],
    *,
    revision: str | None,
    target: str,
    files: dict[str, ArchiveMember],
    kind: str,
    failures: list[str],
) -> None:
    if sbom.get("bomFormat") != "CycloneDX" or sbom.get("specVersion") != "1.5":
        failures.append("SBOM.cdx.json must be CycloneDX 1.5")
    if sbom.get("version") != 1:
        failures.append("SBOM.cdx.json document version must be 1")
    metadata = sbom.get("metadata")
    if not isinstance(metadata, dict):
        failures.append("SBOM.cdx.json metadata must be an object")
        return
    application = metadata.get("component")
    if not isinstance(application, dict) or application.get("name") != "dbwarp-blueprint":
        failures.append("SBOM metadata component must identify dbwarp-blueprint")
        application = {}
    if not isinstance(application.get("version"), str) or not application.get("version"):
        failures.append("SBOM metadata component has no version")

    properties = metadata.get("properties")
    if not isinstance(properties, list):
        failures.append("SBOM metadata properties must be a list")
        properties = []
    pairs: list[tuple[str, Any]] = []
    for item in properties:
        if not isinstance(item, dict) or not isinstance(item.get("name"), str):
            failures.append("SBOM metadata contains an invalid property")
            continue
        pairs.append((item["name"], item.get("value")))
    names = [name for name, _ in pairs]
    duplicates = sorted(name for name, count in Counter(names).items() if count > 1)
    if duplicates:
        failures.append(f"SBOM metadata has duplicate properties: {duplicates}")
    property_map = dict(pairs)
    expected_properties = {
        "dbwarp:source-revision": revision,
        "dbwarp:build-target": target,
        "dbwarp:cargo-lock": "locked",
    }
    for name, expected in expected_properties.items():
        if property_map.get(name) != expected:
            failures.append(
                f"SBOM property {name} is {property_map.get(name)!r}; expected {expected!r}"
            )

    components = sbom.get("components")
    if not isinstance(components, list) or not components:
        failures.append("SBOM components must be a non-empty list")
        return
    component_refs: list[str] = []
    component_pairs: list[tuple[str, str]] = []
    for item in components:
        if not isinstance(item, dict):
            failures.append("SBOM contains a non-object component")
            continue
        name, version, bom_ref = item.get("name"), item.get("version"), item.get("bom-ref")
        if not all(isinstance(value, str) and value for value in (name, version, bom_ref)):
            failures.append("SBOM component must have non-empty name, version, and bom-ref")
            continue
        component_refs.append(bom_ref)
        component_pairs.append((name, version))
        hashes = item.get("hashes", [])
        if not isinstance(hashes, list):
            failures.append(f"SBOM component {bom_ref} hashes must be a list")
        for digest in hashes if isinstance(hashes, list) else []:
            if not isinstance(digest, dict) or digest.get("alg") != "SHA-256" or not re.fullmatch(
                r"[0-9a-f]{64}", str(digest.get("content", ""))
            ):
                failures.append(f"SBOM component {bom_ref} has an invalid SHA-256")
    duplicate_refs = sorted(
        value for value, count in Counter(component_refs).items() if count > 1
    )
    if duplicate_refs:
        failures.append(f"SBOM component bom-ref values are not unique: {duplicate_refs}")

    if kind == "source":
        lock_member = files.get("Cargo.lock")
        manifest_member = files.get("Cargo.toml")
        if lock_member and lock_member.data:
            try:
                lock = tomllib.loads(lock_member.data.decode("utf-8"))
                locked_pairs = Counter(
                    (str(item["name"]), str(item["version"]))
                    for item in lock.get("package", [])
                )
                if Counter(component_pairs) != locked_pairs:
                    failures.append("SBOM components do not exactly match Cargo.lock packages")
            except (UnicodeDecodeError, tomllib.TOMLDecodeError, KeyError) as exc:
                failures.append(f"could not validate Cargo.lock against SBOM: {exc}")
        if manifest_member and manifest_member.data:
            try:
                manifest = tomllib.loads(manifest_member.data.decode("utf-8"))
                package = manifest["package"]
                if application.get("name") != package.get("name") or application.get(
                    "version"
                ) != package.get("version"):
                    failures.append("SBOM application component does not match Cargo.toml")
            except (UnicodeDecodeError, tomllib.TOMLDecodeError, KeyError) as exc:
                failures.append(f"could not validate Cargo.toml against SBOM: {exc}")


def validate_archive(
    path: Path, kind: str, expected_revision: str | None = None
) -> list[str]:
    try:
        members, unsafe_types = read_archive(path)
    except (OSError, tarfile.TarError, zipfile.BadZipFile) as exc:
        return [f"could not read archive: {exc}"]
    failures = [f"unsafe archive member type: {name}" for name in unsafe_types]
    paths = [normalize(member.name) for member in members]
    canonical_names = [path.as_posix().rstrip("/") for path in paths]
    for member, normalized in zip(members, paths):
        if (
            "\\" in member.name
            or "\x00" in member.name
            or re.match(r"^[A-Za-z]:", member.name)
            or normalized.is_absolute()
            or bool(normalized.parts and re.fullmatch(r"[A-Za-z]:", normalized.parts[0]))
            or ".." in normalized.parts
        ):
            failures.append(f"unsafe archive path: {member.name}")
    duplicates = sorted(name for name, count in Counter(canonical_names).items() if count > 1)
    if duplicates:
        failures.append(f"duplicate archive paths: {duplicates}")
    casefolded: dict[str, set[str]] = {}
    for name in canonical_names:
        casefolded.setdefault(name.casefold(), set()).add(name)
    collisions = sorted(
        sorted(values) for values in casefolded.values() if len(values) > 1
    )
    if collisions:
        failures.append(f"case-colliding archive paths: {collisions}")

    top_levels = {path.parts[0] for path in paths if path.parts}
    expected_top = archive_top_level(path, kind)
    if top_levels != {expected_top}:
        failures.append(
            f"archive top-level directory is {sorted(top_levels)}; expected {expected_top!r}"
        )
        return failures

    files: dict[str, ArchiveMember] = {}
    for member, normalized in zip(members, paths):
        if member.is_dir or len(normalized.parts) <= 1:
            continue
        relative = PurePosixPath(*normalized.parts[1:]).as_posix()
        files[relative] = member

    required = set(COMMON_REQUIRED)
    if kind == "binary":
        target_data = TARGETS.get(expected_top)
        if target_data is None:
            failures.append(f"unrecognized binary archive platform: {expected_top}")
            expected_target, expected_features, binary_name = "unknown", [], None
        else:
            expected_target, expected_features, binary_name = target_data
            required.add(binary_name)
    else:
        expected_target, expected_features, binary_name = "source", [], None
        required.update(
            {
                ".cargo/config.toml",
                ".dbwarp-source-revision",
                "Cargo.lock",
                "Cargo.toml",
                "build.sh",
                "rust-toolchain.toml",
                "vendor/mysql_async/LICENSE-APACHE",
                "vendor/mysql_async/LICENSE-MIT",
                "vendor/mysql_async/MODIFICATIONS.md",
            }
        )
        required.discard("licenses/mysql_async/LICENSE-APACHE")
        required.discard("licenses/mysql_async/LICENSE-MIT")
        required.discard("licenses/mysql_async/MODIFICATIONS.md")
        if not any(name.startswith("vendor-crates/") for name in files):
            failures.append("source archive has no vendor-crates dependency tree")
    for missing in sorted(required - files.keys()):
        failures.append(f"missing required archive member: {missing}")

    unexpected_operator_files = sorted(
        name
        for name in files
        if name.startswith(OPERATOR_SUBTREE_PREFIXES) and name not in OPERATOR_ALLOWED
    )
    if unexpected_operator_files:
        failures.append(
            "unexpected file in closed operator-document/script subtree: "
            + ", ".join(unexpected_operator_files)
        )

    validate_legal_bundle(files, kind, failures)

    if kind == "source":
        executable_names = ["blueprint_format.py", "build.sh", "verify.sh"]
    elif binary_name and not binary_name.endswith(".exe"):
        executable_names = ["blueprint_format.py", binary_name, "verify.sh"]
    else:
        executable_names = []
    for name in executable_names:
        member = files.get(name)
        if member is not None and (member.mode is None or member.mode & 0o111 == 0):
            failures.append(f"required executable is not marked executable: {name}")

    provenance = require_json(files, "PROVENANCE.json", failures)
    revision = None
    if provenance is not None:
        revision = validate_provenance(
            provenance,
            kind=kind,
            expected_revision=expected_revision,
            expected_target=expected_target,
            expected_features=expected_features,
            binary_name=binary_name,
            files=files,
            failures=failures,
        )
    sbom = require_json(files, "SBOM.cdx.json", failures)
    if sbom is not None:
        validate_sbom(
            sbom,
            revision=revision,
            target=expected_target,
            files=files,
            kind=kind,
            failures=failures,
        )

    if kind == "source":
        revision_member = files.get(".dbwarp-source-revision")
        if revision_member and revision_member.data:
            try:
                source_revision = revision_member.data.decode("ascii").strip()
            except UnicodeDecodeError:
                source_revision = ""
            if source_revision != revision:
                failures.append(".dbwarp-source-revision does not match PROVENANCE.json")
        toolchain_member = files.get("rust-toolchain.toml")
        if toolchain_member and toolchain_member.data and provenance is not None:
            try:
                toolchain = tomllib.loads(toolchain_member.data.decode("utf-8"))
                channel = toolchain["toolchain"]["channel"]
                if channel != provenance.get("rust_toolchain"):
                    failures.append("rust-toolchain.toml does not match PROVENANCE.json")
            except (UnicodeDecodeError, tomllib.TOMLDecodeError, KeyError) as exc:
                failures.append(f"could not validate rust-toolchain.toml: {exc}")
    return failures


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("archive", type=Path)
    parser.add_argument("--kind", choices=("binary", "source"), required=True)
    parser.add_argument("--expected-source-revision")
    args = parser.parse_args()
    if args.expected_source_revision is not None and not re.fullmatch(
        r"[0-9a-f]{40}", args.expected_source_revision
    ):
        parser.error("--expected-source-revision must be a lowercase 40-hex Git SHA")

    failures = validate_archive(args.archive, args.kind, args.expected_source_revision)
    if failures:
        for failure in failures:
            print(f"release-archive error: {failure}", file=sys.stderr)
        return 1
    print(f"release archive valid: {args.archive}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
