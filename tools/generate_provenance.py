#!/usr/bin/env python3
"""Generate deterministic, reviewable release provenance metadata."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--kind", choices=("binary", "source"), required=True)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--source-revision", required=True)
    parser.add_argument("--ref", required=True)
    parser.add_argument("--toolchain", required=True)
    parser.add_argument("--target", required=True)
    parser.add_argument("--features", default="")
    parser.add_argument("--source-date-epoch", type=int)
    parser.add_argument("--native-dependency-compiler")
    parser.add_argument("--binary", type=Path)
    args = parser.parse_args()

    if args.kind == "binary" and not args.binary:
        parser.error("--binary is required for binary provenance")
    document: dict[str, object] = {
        "schema": "dbwarp-blueprint-release-provenance/v1",
        "artifact_kind": args.kind,
        "repository": args.repository,
        "source_revision": args.source_revision,
        "source_ref": args.ref,
        "rust_toolchain": args.toolchain,
        "target": args.target,
        "features": sorted(filter(None, args.features.split(","))),
        "cargo_lock": "locked",
    }
    if args.binary:
        if args.source_date_epoch is None:
            parser.error("--source-date-epoch is required for binary provenance")
        if not args.native_dependency_compiler:
            parser.error("--native-dependency-compiler is required for binary provenance")
        document["source_date_epoch"] = args.source_date_epoch
        document["native_dependency_compiler"] = args.native_dependency_compiler
        document["binary"] = {
            "name": args.binary.name,
            "sha256": sha256(args.binary),
            "size_bytes": args.binary.stat().st_size,
        }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(document, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    print(f"release provenance: {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
