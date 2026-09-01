#!/usr/bin/env python3
"""Fail closed when a DBWarp Blueprint public source tree is unsafe."""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path


REQUIRED = {
    "AGENTS.md",
    "CLAUDE.md",
    "BUILD.md",
    "Cargo.lock",
    "Cargo.toml",
    "GEMINI.md",
    "GROK.md",
    "LICENSE-APACHE",
    "LICENSE-MIT",
    "MACHINE_TRANSLATIONS.md",
    "README.md",
    "SECURITY.md",
    "THIRD_PARTY_NOTICES.md",
    "assets/fonts/dm-sans/OFL.txt",
    "build.sh",
    "crates/dbwarp-blueprint-core/SOURCE_MANIFEST.sha256",
    "rust-toolchain.toml",
    "rustup-init.sha256",
    "src/main.rs",
    "third-party-licenses/MANIFEST.json",
    "tools/check_public_tree.py",
    "tools/generate_third_party_notices.py",
    "vendor/mysql_async/LICENSE-APACHE",
    "vendor/mysql_async/LICENSE-MIT",
    "vendor/mysql_async/MODIFICATIONS.md",
}

FORBIDDEN_PATH_PARTS = {"artifact-publish", "internal", "tmp"}
# __pycache__ is here because the release job runs other tools/*.py before
# this checker, and Python compiles this module to bytecode that embeds the
# FORBIDDEN_TEXT set below. Without it the checker finds its own denylist
# inside its own .pyc and fails on itself.
IGNORED_PATH_PARTS = {".git", ".hg", ".svn", "target", "__pycache__"}

FORBIDDEN_TEXT = {
    "CLOUDSDK_AUTH_CREDENTIAL_FILE_OVERRIDE",
    "DBWARP_BLUEPRINT_UPLOAD_ARTIFACT",
    "gcloud artifacts generic upload",
    "Safe to share via email",
    "/".join(("tests", "manual")),
    "/".join(("marketing", "ascii")),
}

# Keep this standalone checker free of organization-private identifiers.
# Structural checks reject non-public repository and lab-host classes without
# embedding a list of private repository names in the public checker itself.
DBWARP_REPOSITORY_URL = re.compile(r"github\.com/DBWarp/([A-Za-z0-9._-]+)", re.I)
DBWARP_REPOSITORY_LAYOUT = re.compile(
    r"\b(dbwarp-[a-z0-9._-]+)/(?:build|docs|internal|scripts|src)(?:/|\b)", re.I
)
# Crates this repository actually publishes. A path like
# `dbwarp-blueprint-core/src/` is this tree's own shared core, not a reference
# to a private sibling repository, so it must not trip the layout rule. Listing
# the public crates keeps the rule's coverage of genuinely private names such
# as `dbwarp-<something>/src` intact; loosening the pattern would not.
PUBLIC_DBWARP_PROJECTS = frozenset({"dbwarp-blueprint", "dbwarp-blueprint-core"})
LAB_HOST = re.compile(r"\b[a-z0-9-]+\.dbwarp\.test\b", re.I)
RETIRED_COMMAND = re.compile(
    r"(?:`|^)[ \t]*dbwarp[ \t]+estimate(?:[ \t`]|$)", re.I | re.M
)
MARKDOWN_LINK = re.compile(r"(?<!!)\[[^\]]*\]\(([^)]+)\)")


def files_under(root: Path) -> list[Path]:
    return sorted(
        path
        for path in root.rglob("*")
        if path.is_file()
        and not set(path.relative_to(root).parts).intersection(IGNORED_PATH_PARTS)
    )


def markdown_link_target(raw: str) -> str:
    value = raw.strip()
    if value.startswith("<") and ">" in value:
        return value[1 : value.index(">")]
    return value.split(maxsplit=1)[0] if value else ""


def inspect_text(path: str, text: str, failures: list[str]) -> None:
    for match in DBWARP_REPOSITORY_URL.finditer(text):
        repository = match.group(1)
        normalized = repository[:-4] if repository.casefold().endswith(".git") else repository
        if normalized.casefold() != "dbwarp-blueprint":
            failures.append(f"non-public DBWarp repository URL: {path}")
    for match in DBWARP_REPOSITORY_LAYOUT.finditer(text):
        if match.group(1).casefold() not in PUBLIC_DBWARP_PROJECTS:
            failures.append(f"non-public DBWarp repository layout: {path}")
    if LAB_HOST.search(text):
        failures.append(f"non-public DBWarp lab hostname: {path}")
    if RETIRED_COMMAND.search(text):
        failures.append(f"retired downstream command: {path}")


def validate_public_tree(root: Path, allow_vendored_dependencies: bool = False) -> list[str]:
    root = root.resolve()
    failures: list[str] = []
    relative_files = {path.relative_to(root).as_posix() for path in files_under(root)}

    for required in sorted(REQUIRED - relative_files):
        failures.append(f"missing required public file: {required}")
    for path in sorted(relative_files):
        if path.startswith("vendor-crates/"):
            if not allow_vendored_dependencies:
                failures.append(f"unexpected generated dependency path: {path}")
            # Cargo.lock and the archive validator bind this generated graph.
            # First-party prose checks are not applied to third-party sources.
            continue
        parts = set(Path(path).parts)
        if parts & FORBIDDEN_PATH_PARTS:
            failures.append(f"forbidden public path: {path}")
            continue
        if path == "tools/check_public_tree.py":
            continue
        file_path = root / path
        try:
            data = file_path.read_bytes()
        except OSError as exc:
            failures.append(f"could not read public file: {path}: {exc}")
            continue
        folded_data = data.lower()
        for token in FORBIDDEN_TEXT:
            if token.encode("utf-8").lower() in folded_data:
                failures.append(f"forbidden public text {token!r}: {path}")
        text = data.decode("utf-8", errors="replace")
        inspect_text(path, text, failures)
        if file_path.suffix.lower() == ".md":
            try:
                text = data.decode("utf-8")
            except UnicodeDecodeError:
                failures.append(f"public Markdown is not UTF-8: {path}")
                continue
            for match in MARKDOWN_LINK.finditer(text):
                target = markdown_link_target(match.group(1))
                if not target or target.startswith(("#", "http://", "https://", "mailto:")):
                    continue
                relative_target = target.split("#", 1)[0].split("?", 1)[0]
                resolved_target = (file_path.parent / relative_target).resolve()
                if root != resolved_target and root not in resolved_target.parents:
                    failures.append(f"public Markdown link escapes the source tree: {path}: {target}")
                elif not resolved_target.exists():
                    failures.append(f"public Markdown link target is missing: {path}: {target}")

    checksum_path = root / "rustup-init.sha256"
    if checksum_path.is_file():
        try:
            checksum_text = checksum_path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as exc:
            failures.append(f"could not validate rustup-init.sha256: {exc}")
        else:
            for line in checksum_text.splitlines():
                if re.match(r"^0{64}\s", line):
                    failures.append("rustup-init.sha256 contains a placeholder hash")

    apache_path = root / "LICENSE-APACHE"
    if apache_path.is_file():
        try:
            apache = apache_path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as exc:
            failures.append(f"could not validate LICENSE-APACHE: {exc}")
        else:
            if len(apache.encode("utf-8")) < 10_000 or "3. Grant of Patent License." not in apache:
                failures.append("LICENSE-APACHE is not the complete Apache License 2.0 text")

    manifest_path = root / "third-party-licenses" / "MANIFEST.json"
    if manifest_path.is_file():
        try:
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        except (OSError, UnicodeDecodeError, json.JSONDecodeError) as exc:
            failures.append(f"invalid third-party licence manifest: {exc}")
        else:
            if manifest.get("schema") != "dbwarp-blueprint-third-party-licenses/v1":
                failures.append("third-party licence manifest has an invalid schema")
            packages = manifest.get("packages")
            if not isinstance(packages, list) or manifest.get("third_party_package_count") != len(packages):
                failures.append("third-party licence manifest package count does not reconcile")
    return failures


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument(
        "--allow-vendored-dependencies",
        action="store_true",
        help="allow the generated Cargo vendor-crates tree without treating third-party text as first-party",
    )
    args = parser.parse_args()
    root = args.root.resolve()
    failures = validate_public_tree(root, args.allow_vendored_dependencies)
    if failures:
        for failure in failures:
            print(f"public-tree error: {failure}", file=sys.stderr)
        return 1
    print(f"public source tree valid: {len(files_under(root))} files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
