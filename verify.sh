#!/usr/bin/env bash
# verify.sh: compare a shipped release binary's sha256 against the binary
# this repo produced. This compares bytes, not semantic equivalence or source
# authenticity; reproducing release bytes requires the full release inputs.
#
# Usage:
#   ./verify.sh /path/to/extracted/dbwarp-blueprint
#
# Returns 0 only on byte identity and non-zero on mismatch or invalid input.
# A mismatch alone is not evidence of tampering.
#
# This script does NOT build by itself. Run ./build.sh first to produce
# target/release/dbwarp-blueprint, then run this to compare.
set -euo pipefail

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    echo "verify.sh: no SHA-256 utility found; install sha256sum or shasum" >&2
    return 1
  fi
}

if [[ $# -ne 1 ]]; then
  echo "usage: $0 <path-to-released-binary>" >&2
  exit 64
fi

RELEASE_BIN="$1"
SOURCE_BIN="${SOURCE_BIN:-target/release/dbwarp-blueprint}"

if [[ ! -f "$RELEASE_BIN" ]]; then
  echo "verify.sh: release binary not found: $RELEASE_BIN" >&2
  exit 1
fi
if [[ ! -f "$SOURCE_BIN" ]]; then
  echo "verify.sh: source build not found at $SOURCE_BIN: run ./build.sh first" >&2
  exit 1
fi

echo "verify.sh: This is a byte-identity check, not a functional-equivalence test."
echo "verify.sh: DIFFER is expected unless the full release build inputs were"
echo "verify.sh: reproduced. A normal ./build.sh alone does not establish that."
echo "verify.sh: The release workflow builds twice on one machine; this does not establish"
echo "verify.sh: cross-machine reproducibility. See BUILD.md: Reproducing a release binary."
echo

RELEASE_SHA="$(sha256_file "$RELEASE_BIN")"
SOURCE_SHA="$(sha256_file "$SOURCE_BIN")"

echo "verify.sh: comparing"
echo "  release: $RELEASE_BIN"
echo "    sha256: $RELEASE_SHA"
echo "  source:  $SOURCE_BIN"
echo "    sha256: $SOURCE_SHA"
echo

if [[ "$RELEASE_SHA" == "$SOURCE_SHA" ]]; then
  echo "verify.sh: MATCH: the released binary is bit-identical to the binary"
  echo "verify.sh: built from this source."
  exit 0
fi

echo "verify.sh: DIFFER: sha256 differs between release and source build."
echo "verify.sh:"
echo "verify.sh: Byte identity was not established. This result alone neither"
echo "verify.sh: proves tampering nor establishes functional equivalence. Confirm the"
echo "verify.sh: source revision, target, features, pinned toolchain, linker,"
echo "verify.sh: and release build flags. To investigate, run:"
echo "verify.sh:   cmp -l \"$RELEASE_BIN\" \"$SOURCE_BIN\" | head -20"
echo "verify.sh:   mkdir -p test-output"
echo "verify.sh:   readelf -a \"$RELEASE_BIN\" > test-output/release.txt"
echo "verify.sh:   readelf -a \"$SOURCE_BIN\"  > test-output/source.txt"
echo "verify.sh:   diff test-output/release.txt test-output/source.txt | head -30"
echo "verify.sh:"
echo "verify.sh: Diagnostic similarity does not replace a matching SHA-256."
echo "verify.sh: See BUILD.md for the controlled release-build requirements."
exit 1
