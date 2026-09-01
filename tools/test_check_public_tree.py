#!/usr/bin/env python3
"""Adversarial tests for the standalone public-tree checker."""

from __future__ import annotations

import tempfile
import unittest
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

    def test_every_explicit_token_is_blocked_even_in_non_utf8_data(self) -> None:
        for token in check_public_tree.FORBIDDEN_TEXT:
            with self.subTest(token=token):
                self.write_probe(b"\xff\xfe" + token.encode("utf-8"))
                self.assertTrue(self.failures())
                (self.root / "probe.bin").unlink()

    def test_hyphenated_private_repository_url_is_blocked(self) -> None:
        repository = "dbwarp-blueprint" + "-internal"
        self.write_probe(f"https://github.com/DBWarp/{repository}\n".encode())
        self.assertTrue(self.failures())

    def test_public_repository_url_is_allowed(self) -> None:
        self.write_probe(b"https://github.com/DBWarp/dbwarp-blueprint.git\n")
        self.assertEqual(self.failures(), [])

    def test_private_layout_lab_host_and_retired_command_are_blocked(self) -> None:
        probes = [
            "dbwarp-" + "other/internal/readme",
            "host." + "dbwarp.test",
            "`dbwarp " + "estimate`",
        ]
        for probe in probes:
            with self.subTest(probe=probe):
                self.write_probe(probe.encode())
                self.assertTrue(self.failures())
                (self.root / "probe.bin").unlink()

    def test_vcs_metadata_is_deliberately_ignored(self) -> None:
        path = self.root / ".git" / "config"
        path.parent.mkdir()
        path.write_text("host." + "dbwarp.test", encoding="utf-8")
        self.assertEqual(self.failures(), [])

    def test_invalid_utf8_markdown_fails_instead_of_being_skipped(self) -> None:
        self.write_probe(b"heading\xff", "probe.md")
        self.assertTrue(any("not UTF-8" in failure for failure in self.failures()))


if __name__ == "__main__":
    unittest.main()
