#!/usr/bin/env bash
# Verify the canonical Blueprint core sources included in a public source tree.
set -euo pipefail

if [[ $# -ne 0 ]]; then
  echo "usage: $0" >&2
  exit 2
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CORE="${ROOT}/crates/dbwarp-blueprint-core"
MANIFEST="${CORE}/SOURCE_MANIFEST.sha256"

if [[ ! -f "${MANIFEST}" ]]; then
  echo "blueprint-core source manifest is missing: ${MANIFEST}" >&2
  exit 1
fi

(
  cd "${CORE}"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum --check SOURCE_MANIFEST.sha256
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 --check SOURCE_MANIFEST.sha256
  else
    echo "no SHA-256 utility found; install sha256sum or shasum" >&2
    exit 1
  fi
)

echo "blueprint-core source contract is synchronized"
