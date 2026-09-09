#!/usr/bin/env python3
"""Adversarial tests for the standalone public-tree checker."""

from __future__ import annotations

import tempfile
import subprocess
import re
import unittest
from unittest.mock import patch
from pathlib import Path

import check_public_tree


ROOT = Path(__file__).resolve().parents[1]


class PublicTreeCheckerTests(unittest.TestCase):
    def setUp(self) -> None:
        scratch = ROOT / "tmp"
        scratch.mkdir(exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(prefix="public-tree-test-", dir=scratch)
        self.root = Path(self.temporary.name)
        for relative in check_public_tree.REQUIRED:
            target = self.root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text("fixture\n", encoding="utf-8")
            if relative in check_public_tree.REQUIRED_EXECUTABLES:
                target.chmod(0o755)
        (self.root / "LICENSE-APACHE").write_bytes((ROOT / "LICENSE-APACHE").read_bytes())
        (self.root / "rustup-init.sha256").write_text("1" * 64 + "  rustup-init\n", encoding="utf-8")
        (self.root / "third-party-licenses" / "MANIFEST.json").write_text(
            '{"schema":"dbwarp-blueprint-third-party-licenses/v1",'
            '"third_party_package_count":0,"packages":[]}\n',
            encoding="utf-8",
        )

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def failures(self) -> list[str]:
        return check_public_tree.validate_public_tree(self.root)

    def write_probe(self, data: bytes, name: str = "probe.bin") -> None:
        (self.root / name).write_bytes(data)

    def test_clean_minimal_tree_passes(self) -> None:
        self.assertEqual(self.failures(), [])

    def test_release_history_and_support_documents_are_required(self) -> None:
        for name in ("CHANGELOG.md", "CONTRIBUTING.md", "SUPPORT.md"):
            with self.subTest(name=name):
                self.assertIn(name, check_public_tree.REQUIRED)
                path = self.root / name
                content = path.read_bytes()
                path.unlink()
                try:
                    self.assertIn(f"missing required public file: {name}", self.failures())
                finally:
                    path.write_bytes(content)

    def test_lost_executable_modes_fail_for_every_entry_point(self) -> None:
        for relative in check_public_tree.REQUIRED_EXECUTABLES:
            with self.subTest(relative=relative):
                path = self.root / relative
                path.chmod(0o644)
                try:
                    self.assertTrue(any(relative in error and "executable bit" in error for error in self.failures()))
                finally:
                    path.chmod(0o755)

    def test_git_index_modes_are_checked_even_when_disk_is_executable(self) -> None:
        for args in (["init", "--quiet"], ["add", "."], ["config", "core.filemode", "false"],
                     ["update-index", "--chmod=-x", "--", "build.sh"]):
            subprocess.run(["git", *args], cwd=self.root, check=True, capture_output=True)
        self.assertTrue(any("Git index" in error and "build.sh" in error for error in self.failures()))
        subprocess.run(["git", "update-index", "--chmod=+x", "--", "build.sh"], cwd=self.root, check=True)
        self.assertEqual(self.failures(), [])

    def test_windows_without_candidate_index_fails_closed(self) -> None:
        with patch.object(check_public_tree.os, "name", "nt"):
            self.assertTrue(any("without a staged" in error for error in
                                check_public_tree.validate_executable_modes(self.root, {"build.sh"})))

    def test_repository_rule_is_enforced_even_in_non_utf8_data(self) -> None:
        organization = "DBWarp"
        self.write_probe(f"https://github.com/{organization}/example-private-project".encode() + b"\xff")
        self.assertTrue(self.failures())

    def test_checker_source_is_scanned_too(self) -> None:
        path = self.root / "tools/check_public_tree.py"
        path.write_bytes(Path(check_public_tree.__file__).read_bytes())
        self.assertEqual(self.failures(), [])
        organization = "DBWarp"
        path.write_text(f"https://github.com/{organization}/example-private-project\n", encoding="utf-8")
        self.assertTrue(any("tools/check_public_tree.py" in error for error in self.failures()))

    def test_hyphenated_private_repository_url_is_blocked(self) -> None:
        repository = "example-private-project"
        self.write_probe(f"https://github.com/DBWarp/{repository}\n".encode())
        self.assertTrue(self.failures())

    def test_public_repository_url_is_allowed(self) -> None:
        self.write_probe(b"https://github.com/DBWarp/dbwarp-blueprint.git\n")
        self.assertEqual(self.failures(), [])

    def test_private_layout_lab_host_and_retired_command_are_blocked(self) -> None:
        probes = [
            "dbwarp-" + "other/internal/readme",
            "`dbwarp " + "estimate`",
        ]
        for probe in probes:
            with self.subTest(probe=probe):
                self.write_probe(probe.encode())
                self.assertTrue(self.failures())
                (self.root / "probe.bin").unlink()

    def test_lab_host_rule_with_synthetic_domain(self) -> None:
        pattern = re.compile(check_public_tree.LAB_HOST.pattern.replace("dbwarp", "example"), re.I)
        with patch.object(check_public_tree, "LAB_HOST", pattern):
            self.write_probe(b"host.example.test")
            self.assertTrue(self.failures())

    def test_vcs_metadata_is_deliberately_ignored(self) -> None:
        for args in (["init", "--quiet"], ["add", "."]):
            subprocess.run(["git", *args], cwd=self.root, check=True, capture_output=True)
        path = self.root / ".git" / "review-note"
        path.write_text("host.example.test", encoding="utf-8")
        self.assertEqual(self.failures(), [])

    def test_invalid_utf8_markdown_fails_instead_of_being_skipped(self) -> None:
        self.write_probe(b"heading\xff", "probe.md")
        self.assertTrue(any("not UTF-8" in failure for failure in self.failures()))


if __name__ == "__main__":
    unittest.main()
