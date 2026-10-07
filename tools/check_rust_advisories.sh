#!/usr/bin/env bash
set -euo pipefail

required_version=0.22.2
if ! command -v cargo-audit >/dev/null 2>&1; then
  echo "cargo-audit $required_version is required" >&2
  echo "install it with: cargo install cargo-audit --version $required_version --locked" >&2
  exit 1
fi

installed_version=$(cargo audit --version)
case "$installed_version" in
  *" $required_version") ;;
  *)
    echo "cargo-audit $required_version is required; found: $installed_version" >&2
    exit 1
    ;;
esac

cargo audit --deny warnings
cargo audit --deny warnings --no-fetch \
  --file crates/dbwarp-blueprint-core/Cargo.lock
