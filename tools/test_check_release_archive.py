#!/usr/bin/env python3
"""Adversarial tests for the release-archive trust boundary."""

from __future__ import annotations

import hashlib
import io
import json
import tarfile
import tempfile
import unittest
from pathlib import Path

import check_release_archive


REVISION = "a" * 40
TOP = "dbwarp-blueprint-linux-x86_64"
SOURCE_TOP = "dbwarp-blueprint-source-vendored"


def json_bytes(value: object) -> bytes:
    return (json.dumps(value, sort_keys=True) + "\n").encode()


def legal_fixture_files() -> dict[str, tuple[bytes, int]]:
    notice = b"example licence\n"
    manifest = {
        "schema": "dbwarp-blueprint-third-party-licenses/v1",
        "cargo_resolved_package_count": 3,
        "first_party_package_count": 2,
        "third_party_package_count": 1,
        "packages": [
            {
                "name": "example",
                "version": "1.0.0",
                "license": "MIT",
                "files": [
                    {
                        "path": "LICENSE",
                        "sha256": hashlib.sha256(notice).hexdigest(),
                    }
                ],
            }
        ],
    }
    return {
        "LICENSE-APACHE": ((Path.cwd() / "LICENSE-APACHE").read_bytes(), 0o644),
        "third-party-licenses/.generated-by-dbwarp-blueprint": (b"generated\n", 0o644),
        "third-party-licenses/MANIFEST.json": (json_bytes(manifest), 0o644),
        "third-party-licenses/crates/example-1.0.0/LICENSE": (notice, 0o644),
    }


def fixture_files() -> dict[str, tuple[bytes, int]]:
    binary = b"dbwarp-blueprint-test-binary\n"
    provenance = {
        "schema": "dbwarp-blueprint-release-provenance/v1",
        "artifact_kind": "binary",
        "repository": "DBWarp/dbwarp-blueprint",
        "source_revision": REVISION,
        "source_ref": "refs/tags/v1.4.0",
        "rust_toolchain": "1.94.0",
        "target": "x86_64-unknown-linux-gnu",
        "features": [],
        "cargo_lock": "locked",
        "source_date_epoch": 1_788_131_408,
        "native_dependency_compiler": "gcc 15.2.0",
        "binary": {
            "name": "dbwarp-blueprint",
            "sha256": hashlib.sha256(binary).hexdigest(),
            "size_bytes": len(binary),
        },
    }
    sbom = {
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "version": 1,
        "metadata": {
            "component": {
                "type": "application",
                "name": "dbwarp-blueprint",
                "version": "1.4.0",
            },
            "properties": [
                {"name": "dbwarp:source-revision", "value": REVISION},
                {"name": "dbwarp:build-target", "value": "x86_64-unknown-linux-gnu"},
                {"name": "dbwarp:cargo-lock", "value": "locked"},
            ],
        },
        "components": [
            {
                "type": "library",
                "bom-ref": "cargo:example@1.0.0",
                "name": "example",
                "version": "1.0.0",
                "hashes": [{"alg": "SHA-256", "content": "b" * 64}],
            }
        ],
    }
    files = {
        name: (b"fixture\n", 0o644)
        for name in check_release_archive.COMMON_REQUIRED
        if name not in {"PROVENANCE.json", "SBOM.cdx.json"}
    }
    files.update(
        {
            "blueprint_format.py": (b"#!/usr/bin/env python3\n", 0o755),
            "dbwarp-blueprint": (binary, 0o755),
            "PROVENANCE.json": (json_bytes(provenance), 0o644),
            "SBOM.cdx.json": (json_bytes(sbom), 0o644),
            "verify.sh": (b"#!/usr/bin/env bash\n", 0o755),
        }
    )
    files.update(legal_fixture_files())
    return files


def source_fixture_files() -> dict[str, tuple[bytes, int]]:
    provenance = {
        "schema": "dbwarp-blueprint-release-provenance/v1",
        "artifact_kind": "source",
        "repository": "DBWarp/dbwarp-blueprint",
        "source_revision": REVISION,
        "source_ref": "refs/tags/v1.4.0",
        "rust_toolchain": "1.94.0",
        "target": "source",
        "features": [],
        "cargo_lock": "locked",
    }
    sbom = {
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "version": 1,
        "metadata": {
            "component": {
                "type": "application",
                "name": "dbwarp-blueprint",
                "version": "1.4.0",
            },
            "properties": [
                {"name": "dbwarp:source-revision", "value": REVISION},
                {"name": "dbwarp:build-target", "value": "source"},
                {"name": "dbwarp:cargo-lock", "value": "locked"},
            ],
        },
        "components": [
            {
                "type": "library",
                "bom-ref": "cargo:dbwarp-blueprint@1.4.0",
                "name": "dbwarp-blueprint",
                "version": "1.4.0",
            }
        ],
    }
    files = {
        name: (b"fixture\n", 0o644)
        for name in check_release_archive.COMMON_REQUIRED
        if name
        not in {
            "PROVENANCE.json",
            "SBOM.cdx.json",
            "licenses/mysql_async/LICENSE-APACHE",
            "licenses/mysql_async/LICENSE-MIT",
            "licenses/mysql_async/MODIFICATIONS.md",
        }
    }
    files.update(
        {
            ".cargo/config.toml": (b"[net]\noffline = true\n", 0o644),
            ".dbwarp-source-revision": ((REVISION + "\n").encode(), 0o644),
            "Cargo.lock": (
                b"version = 3\n\n[[package]]\nname = \"dbwarp-blueprint\"\nversion = \"1.4.0\"\n",
                0o644,
            ),
            "Cargo.toml": (
                b"[package]\nname = \"dbwarp-blueprint\"\nversion = \"1.4.0\"\n",
                0o644,
            ),
            "PROVENANCE.json": (json_bytes(provenance), 0o644),
            "SBOM.cdx.json": (json_bytes(sbom), 0o644),
            "build.sh": (b"#!/usr/bin/env bash\n", 0o755),
            "blueprint_format.py": (b"#!/usr/bin/env python3\n", 0o755),
            "rust-toolchain.toml": (
                b"[toolchain]\nchannel = \"1.94.0\"\n",
                0o644,
            ),
            "verify.sh": (b"#!/usr/bin/env bash\n", 0o755),
            "vendor/mysql_async/LICENSE-APACHE": (b"license\n", 0o644),
            "vendor/mysql_async/LICENSE-MIT": (b"license\n", 0o644),
            "vendor/mysql_async/MODIFICATIONS.md": (b"modifications\n", 0o644),
            "vendor-crates/example/Cargo.toml": (b"[package]\n", 0o644),
        }
    )
    files.update(legal_fixture_files())
    return files


def write_tar(
    path: Path,
    files: dict[str, tuple[bytes, int]],
    *,
    top: str = TOP,
    symlink: bool = False,
) -> None:
    with tarfile.open(path, "w:gz") as archive:
        for name, (data, mode) in sorted(files.items()):
            info = tarfile.TarInfo(f"{top}/{name}")
            info.size = len(data)
            info.mode = mode
            info.mtime = 0
            archive.addfile(info, io.BytesIO(data))
        if symlink:
            info = tarfile.TarInfo(f"{top}/unsafe-link")
            info.type = tarfile.SYMTYPE
            info.linkname = "../../outside"
            archive.addfile(info)


class ReleaseArchiveTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        scratch = Path.cwd() / "tmp"
        scratch.mkdir(exist_ok=True)
        cls._temporary = tempfile.TemporaryDirectory(
            prefix="release-archive-test-", dir=scratch
        )
        cls.root = Path(cls._temporary.name)

    @classmethod
    def tearDownClass(cls) -> None:
        cls._temporary.cleanup()

    def archive(self, label: str, files: dict[str, tuple[bytes, int]] | None = None) -> Path:
        directory = self.root / label
        directory.mkdir()
        path = directory / f"{TOP}.tar.gz"
        write_tar(path, files or fixture_files())
        return path

    def test_accepts_internally_consistent_binary_archive(self) -> None:
        path = self.archive("valid")
        self.assertEqual(
            check_release_archive.validate_archive(path, "binary", REVISION), []
        )

    def test_rejects_payload_hash_mismatch(self) -> None:
        files = fixture_files()
        files["dbwarp-blueprint"] = (b"tampered\n", 0o755)
        failures = check_release_archive.validate_archive(
            self.archive("hash", files), "binary", REVISION
        )
        self.assertTrue(any("binary sha256" in failure for failure in failures), failures)

    def test_rejects_wrong_workflow_revision(self) -> None:
        failures = check_release_archive.validate_archive(
            self.archive("revision"), "binary", "c" * 40
        )
        self.assertTrue(any("does not match expected" in failure for failure in failures), failures)

    def test_rejects_binary_without_reproducible_native_build_inputs(self) -> None:
        files = fixture_files()
        provenance = json.loads(files["PROVENANCE.json"][0])
        del provenance["source_date_epoch"]
        del provenance["native_dependency_compiler"]
        files["PROVENANCE.json"] = (json_bytes(provenance), 0o644)
        failures = check_release_archive.validate_archive(
            self.archive("native-build-inputs", files), "binary", REVISION
        )
        self.assertTrue(
            any("source_date_epoch" in failure for failure in failures), failures
        )
        self.assertTrue(
            any("native_dependency_compiler" in failure for failure in failures),
            failures,
        )

    def test_rejects_non_executable_unix_binary(self) -> None:
        files = fixture_files()
        binary, _ = files["dbwarp-blueprint"]
        files["dbwarp-blueprint"] = (binary, 0o644)
        failures = check_release_archive.validate_archive(
            self.archive("mode", files), "binary", REVISION
        )
        self.assertTrue(any("not marked executable" in failure for failure in failures), failures)

    def test_rejects_archive_without_verifier_or_grant_script(self) -> None:
        files = fixture_files()
        del files["verify.sh"]
        del files["sql/grants/postgresql/standard.sql"]
        failures = check_release_archive.validate_archive(
            self.archive("operator-files", files), "binary", REVISION
        )
        self.assertIn("missing required archive member: verify.sh", failures)
        self.assertIn(
            "missing required archive member: sql/grants/postgresql/standard.sql",
            failures,
        )

    def test_accepts_registered_supplemental_translation(self) -> None:
        files = fixture_files()
        files["docs/de/README.md"] = (b"machine translation\n", 0o644)
        failures = check_release_archive.validate_archive(
            self.archive("registered-translation", files), "binary", REVISION
        )
        self.assertFalse(
            any("docs/de/README.md" in failure for failure in failures), failures
        )

    def test_rejects_internal_or_unregistered_operator_documents(self) -> None:
        files = fixture_files()
        files["docs/ESTIMATOR_HANDOFF.md"] = (b"internal\n", 0o644)
        files["docs/de/UNREGISTERED.md"] = (b"unregistered translation\n", 0o644)
        files["sql/grants/INTERNAL-NOTES.md"] = (b"internal\n", 0o644)
        failures = check_release_archive.validate_archive(
            self.archive("operator-allowlist", files), "binary", REVISION
        )
        self.assertTrue(
            any(
                "unexpected file in closed operator-document/script subtree" in failure
                and "docs/ESTIMATOR_HANDOFF.md" in failure
                and "docs/de/UNREGISTERED.md" in failure
                and "sql/grants/INTERNAL-NOTES.md" in failure
                for failure in failures
            ),
            failures,
        )

    def test_rejects_duplicate_sbom_references(self) -> None:
        files = fixture_files()
        sbom = json.loads(files["SBOM.cdx.json"][0])
        sbom["components"].append(dict(sbom["components"][0]))
        files["SBOM.cdx.json"] = (json_bytes(sbom), 0o644)
        failures = check_release_archive.validate_archive(
            self.archive("sbom", files), "binary", REVISION
        )
        self.assertTrue(any("bom-ref values" in failure for failure in failures), failures)

    def test_rejects_tampered_third_party_licence_notice(self) -> None:
        files = fixture_files()
        files["third-party-licenses/crates/example-1.0.0/LICENSE"] = (
            b"tampered notice\n",
            0o644,
        )
        failures = check_release_archive.validate_archive(
            self.archive("third-party-notice", files), "binary", REVISION
        )
        self.assertTrue(any("notice hash mismatch" in failure for failure in failures), failures)

    def test_rejects_unsafe_member_type(self) -> None:
        directory = self.root / "symlink"
        directory.mkdir()
        path = directory / f"{TOP}.tar.gz"
        write_tar(path, fixture_files(), symlink=True)
        failures = check_release_archive.validate_archive(path, "binary", REVISION)
        self.assertTrue(any("unsafe archive member type" in failure for failure in failures), failures)

    def test_accepts_source_archive_with_matching_lock_and_provenance(self) -> None:
        directory = self.root / "source-valid"
        directory.mkdir()
        path = directory / f"{SOURCE_TOP}.tar.gz"
        write_tar(path, source_fixture_files(), top=SOURCE_TOP)
        self.assertEqual(
            check_release_archive.validate_archive(path, "source", REVISION), []
        )

    def test_rejects_source_sbom_that_omits_locked_packages(self) -> None:
        files = source_fixture_files()
        sbom = json.loads(files["SBOM.cdx.json"][0])
        sbom["components"] = [
            {
                "type": "library",
                "bom-ref": "cargo:other@1.0.0",
                "name": "other",
                "version": "1.0.0",
            }
        ]
        files["SBOM.cdx.json"] = (json_bytes(sbom), 0o644)
        directory = self.root / "source-lock"
        directory.mkdir()
        path = directory / f"{SOURCE_TOP}.tar.gz"
        write_tar(path, files, top=SOURCE_TOP)
        failures = check_release_archive.validate_archive(path, "source", REVISION)
        self.assertTrue(any("Cargo.lock packages" in failure for failure in failures), failures)


if __name__ == "__main__":
    unittest.main()
