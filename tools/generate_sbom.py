#!/usr/bin/env python3
"""Generate a deterministic CycloneDX SBOM from the locked Cargo graph."""

from __future__ import annotations

import argparse
import json
import tomllib
from pathlib import Path


def component(package: dict[str, object]) -> dict[str, object]:
    name = str(package["name"])
    version = str(package["version"])
    source = str(package.get("source", ""))
    bom_ref = f"cargo:{name}@{version}"
    item: dict[str, object] = {
        "type": "library",
        "bom-ref": bom_ref,
        "name": name,
        "version": version,
    }
    if source.startswith("registry+"):
        item["purl"] = f"pkg:cargo/{name}@{version}"
    elif source:
        item["externalReferences"] = [{"type": "vcs", "url": source}]
    checksum = package.get("checksum")
    if isinstance(checksum, str):
        item["hashes"] = [{"alg": "SHA-256", "content": checksum}]
    return item


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--source-revision", default="unknown")
    parser.add_argument("--target", default="source")
    args = parser.parse_args()

    root = args.root.resolve()
    cargo_lock = tomllib.loads((root / "Cargo.lock").read_text(encoding="utf-8"))
    cargo_toml = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    package = cargo_toml["package"]
    components = sorted(
        (component(item) for item in cargo_lock.get("package", [])),
        key=lambda item: (str(item["name"]), str(item["version"]), str(item["bom-ref"])),
    )
    document = {
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "version": 1,
        "metadata": {
            "component": {
                "type": "application",
                "bom-ref": f"pkg:cargo/{package['name']}@{package['version']}",
                "name": package["name"],
                "version": package["version"],
                "purl": f"pkg:cargo/{package['name']}@{package['version']}",
            },
            "properties": [
                {"name": "dbwarp:source-revision", "value": args.source_revision},
                {"name": "dbwarp:build-target", "value": args.target},
                {"name": "dbwarp:cargo-lock", "value": "locked"},
            ],
        },
        "components": components,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(document, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    print(f"CycloneDX SBOM: {args.output} ({len(components)} locked packages)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
