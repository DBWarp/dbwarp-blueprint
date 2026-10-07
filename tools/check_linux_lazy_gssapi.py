#!/usr/bin/env python3
"""Verify that a Linux release loads GSSAPI only for integrated authentication."""

from __future__ import annotations

import argparse
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path


GSSAPI_NEEDED = re.compile(r"Shared library: \[(?:libgssapi|libkrb5)[^]]*\]")
GSSAPI_RUNTIME = re.compile(r"\s+(libgssapi(?:_krb5)?\.so\.[^ ]+)\s+.*=>\s+(\S+)$")


def run(command: list[str], *, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(command, check=check, text=True, capture_output=True)


def dynamic_dependencies(binary: Path) -> str:
    return run(["readelf", "-d", str(binary)]).stdout


def runtime_libraries() -> list[Path]:
    output = run(["ldconfig", "-p"]).stdout
    found = {
        Path(match.group(2)).resolve()
        for line in output.splitlines()
        if (match := GSSAPI_RUNTIME.match(line))
    }
    return sorted(path for path in found if path.exists())


def verify_no_startup_load(binary: Path, trace: Path) -> None:
    result = run(
        ["strace", "-f", "-e", "trace=openat", "-o", str(trace), str(binary), "--version"]
    )
    if result.returncode != 0:
        raise RuntimeError("the release binary did not start for --version")
    opened = trace.read_text(encoding="utf-8", errors="replace").casefold()
    if "libgssapi" in opened or "libkrb5" in opened:
        raise RuntimeError("the release binary loaded Kerberos/GSSAPI during --version")


def verify_missing_runtime(binary: Path, libraries: list[Path]) -> None:
    if not libraries:
        raise RuntimeError("no Kerberos/GSSAPI runtime library was found for the negative check")
    # Keep the libraries as positional parameters while building the command
    # explicitly after the mount loop. This avoids interpolating paths into the
    # shell program.
    mount_lines = [f'mount --bind /dev/null "${{{index}}}"' for index in range(1, len(libraries) + 1)]
    shell = "set -eu; " + "; ".join(mount_lines) + f'; shift {len(libraries)}; exec "$@"'
    command = [
        "unshare",
        "--mount",
        "--propagation",
        "private",
        "bash",
        "-c",
        shell,
        "lazy-gssapi-check",
        *map(str, libraries),
        str(binary),
        "--connect",
        "sqlserver://db.example.invalid/inventory",
        "--schema",
        "app",
        "--auth-mode",
        "integrated",
        "--dry-run",
    ]
    if os.geteuid() != 0:
        sudo = shutil.which("sudo")
        if sudo is None:
            raise RuntimeError("the missing-runtime check needs root or passwordless sudo")
        command.insert(0, sudo)
        command.insert(1, "-n")
    result = run(command, check=False)
    diagnostic = result.stdout + result.stderr
    if result.returncode == 0:
        raise RuntimeError("integrated authentication succeeded with the runtime hidden")
    if "DBP1604E" not in diagnostic:
        raise RuntimeError("missing Kerberos/GSSAPI runtime did not report DBP1604E")
    if "no compatible library could be loaded" not in diagnostic:
        raise RuntimeError("missing Kerberos/GSSAPI runtime did not explain the local prerequisite")
    if "DBP0001E" in diagnostic:
        raise RuntimeError("missing Kerberos/GSSAPI runtime fell through to DBP0001E")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--trace", type=Path)
    parser.add_argument("--skip-missing-runtime", action="store_true")
    args = parser.parse_args()

    binary = args.binary.resolve()
    if not binary.is_file():
        parser.error(f"binary does not exist: {binary}")
    dependencies = dynamic_dependencies(binary)
    if GSSAPI_NEEDED.search(dependencies):
        raise SystemExit("Linux release has a startup dependency on Kerberos/GSSAPI")

    trace = args.trace or binary.with_name(f".{binary.name}.gssapi-startup.trace")
    verify_no_startup_load(binary, trace)
    trace.unlink(missing_ok=True)
    if not args.skip_missing_runtime:
        verify_missing_runtime(binary, runtime_libraries())
    print("Linux lazy Kerberos/GSSAPI checks passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
