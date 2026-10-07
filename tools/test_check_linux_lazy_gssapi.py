#!/usr/bin/env python3

from __future__ import annotations

import importlib.util
import unittest
from pathlib import Path


MODULE_PATH = Path(__file__).with_name("check_linux_lazy_gssapi.py")
SPEC = importlib.util.spec_from_file_location("check_linux_lazy_gssapi", MODULE_PATH)
assert SPEC and SPEC.loader
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)


class LazyGssapiCheckTests(unittest.TestCase):
    def test_rejects_any_startup_kerberos_or_gssapi_dependency(self) -> None:
        for library in ("libgssapi_krb5.so.2", "libgssapi.so.3", "libkrb5.so.3"):
            text = f" 0x0000000000000001 (NEEDED) Shared library: [{library}]\n"
            self.assertIsNotNone(CHECK.GSSAPI_NEEDED.search(text))

    def test_accepts_unrelated_startup_dependencies(self) -> None:
        text = " 0x0000000000000001 (NEEDED) Shared library: [libc.so.6]\n"
        self.assertIsNone(CHECK.GSSAPI_NEEDED.search(text))

    def test_parses_mit_and_heimdal_runtime_paths(self) -> None:
        lines = (
            "\tlibgssapi_krb5.so.2 (libc6,x86-64) => /usr/lib/libgssapi_krb5.so.2\n"
            "\tlibgssapi.so.3 (libc6,x86-64) => /usr/lib/libgssapi.so.3\n"
        )
        matches = [CHECK.GSSAPI_RUNTIME.match(line) for line in lines.splitlines()]
        self.assertEqual([match.group(2) for match in matches if match], [
            "/usr/lib/libgssapi_krb5.so.2",
            "/usr/lib/libgssapi.so.3",
        ])


if __name__ == "__main__":
    unittest.main()
