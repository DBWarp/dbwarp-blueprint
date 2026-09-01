#!/usr/bin/env bash
# build.sh — build dbwarp-blueprint with a pinned Rust toolchain.
#
# Default path:
#   - small git clone
#   - Cargo.lock pinned dependencies
#   - crates downloaded by Cargo as needed
#
# Offline/audit path:
#   - download the matching *source-vendored* release asset
#   - run this same script inside that extracted tree
#   - vendor/ and .cargo/config.toml force offline vendored builds
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$REPO_DIR"

readonly BUILD_DIR="$REPO_DIR/build"
readonly TOOLCHAIN_DIR="$BUILD_DIR/rust-toolchain"
readonly CARGO_HOME_DIR="$BUILD_DIR/cargo-home"
readonly RUSTUP_INIT_VERSION="1.29.0"

mkdir -p "$BUILD_DIR"

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    echo "build.sh: no SHA-256 utility found; install sha256sum or shasum" >&2
    return 1
  fi
}

worktree_dirty() {
  [[ -n "$(git status --porcelain --untracked-files=normal 2>/dev/null || true)" ]]
}

if [[ ! -f rust-toolchain.toml ]]; then
  echo "build.sh: rust-toolchain.toml not found in $REPO_DIR" >&2
  exit 1
fi
PINNED_RUST="${PINNED_RUST:-$(grep -E '^channel = "[^"]+"' rust-toolchain.toml | head -1 | sed -E 's/^channel = "([^"]+)"$/\1/')}"
if [[ -z "$PINNED_RUST" ]]; then
  echo "build.sh: failed to parse channel from rust-toolchain.toml" >&2
  exit 1
fi
echo "build.sh: pinned toolchain = $PINNED_RUST"

USE_SYSTEM_RUSTC=0
if command -v rustc >/dev/null 2>&1; then
  SYS_VERSION="$(rustc --version | awk '{print $2}')"
  if [[ "$SYS_VERSION" == "$PINNED_RUST" ]]; then
    echo "build.sh: system rustc $SYS_VERSION matches pinned version; using it"
    USE_SYSTEM_RUSTC=1
  else
    echo "build.sh: system rustc is $SYS_VERSION; pinned is $PINNED_RUST"
  fi
fi

if [[ "$USE_SYSTEM_RUSTC" -eq 0 ]]; then
  if [[ "${ALLOW_NETWORK:-0}" != "1" ]]; then
    echo "build.sh: refusing to download rustup-init (set ALLOW_NETWORK=1 to permit)" >&2
    echo "build.sh: alternatively, install rustc $PINNED_RUST and re-run." >&2
    exit 2
  fi
  if [[ ! -x "$TOOLCHAIN_DIR/bin/cargo" ]]; then
    echo "build.sh: downloading rustup-init (verified against rustup-init.sha256)..."
    UNAME_S="$(uname -s)"
    UNAME_M="$(uname -m)"
    case "$UNAME_S-$UNAME_M" in
      Linux-x86_64)  RUSTUP_TARGET="x86_64-unknown-linux-gnu" ;;
      Linux-aarch64) RUSTUP_TARGET="aarch64-unknown-linux-gnu" ;;
      Darwin-x86_64) RUSTUP_TARGET="x86_64-apple-darwin" ;;
      Darwin-arm64)  RUSTUP_TARGET="aarch64-apple-darwin" ;;
      *)             echo "build.sh: unsupported platform $UNAME_S-$UNAME_M" >&2; exit 1 ;;
    esac
    URL="https://static.rust-lang.org/rustup/archive/$RUSTUP_INIT_VERSION/$RUSTUP_TARGET/rustup-init"
    curl -fsSL -o "$BUILD_DIR/rustup-init" "$URL"
    EXPECTED="$(grep "$RUSTUP_TARGET" rustup-init.sha256 | awk '{print $1}')"
    if [[ -z "$EXPECTED" ]]; then
      echo "build.sh: no sha256 entry for $RUSTUP_TARGET in rustup-init.sha256" >&2
      exit 1
    fi
    ACTUAL="$(sha256_file "$BUILD_DIR/rustup-init")"
    if [[ "$ACTUAL" != "$EXPECTED" ]]; then
      echo "build.sh: rustup-init sha256 mismatch (got $ACTUAL, expected $EXPECTED)" >&2
      exit 1
    fi
    chmod +x "$BUILD_DIR/rustup-init"
    RUSTUP_HOME="$BUILD_DIR/rustup" CARGO_HOME="$CARGO_HOME_DIR" \
      "$BUILD_DIR/rustup-init" -y \
        --no-modify-path \
        --default-toolchain "$PINNED_RUST" \
        --profile minimal
    mkdir -p "$TOOLCHAIN_DIR"
    cp -al "$BUILD_DIR/rustup/toolchains/$PINNED_RUST-$RUSTUP_TARGET"/* "$TOOLCHAIN_DIR/" 2>/dev/null \
      || cp -a "$BUILD_DIR/rustup/toolchains/$PINNED_RUST-$RUSTUP_TARGET"/* "$TOOLCHAIN_DIR/"
  fi
  export PATH="$TOOLCHAIN_DIR/bin:$PATH"
fi

export CARGO_HOME="$CARGO_HOME_DIR"
mkdir -p "$CARGO_HOME"
export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$REPO_DIR=."
export DBWARP_BLUEPRINT_TOOLCHAIN="$PINNED_RUST"

if [[ -z "${DBWARP_BLUEPRINT_BUILD_REVISION:-}" ]]; then
  if [[ -f .dbwarp-source-revision ]]; then
    DBWARP_BLUEPRINT_BUILD_REVISION="$(head -1 .dbwarp-source-revision | tr -d '\r\n')"
  else
    DBWARP_BLUEPRINT_BUILD_REVISION="$(git rev-parse HEAD 2>/dev/null || true)"
  fi
fi
if [[ -z "$DBWARP_BLUEPRINT_BUILD_REVISION" ]]; then
  DBWARP_BLUEPRINT_BUILD_REVISION="(source revision unavailable)"
fi
export DBWARP_BLUEPRINT_BUILD_REVISION

if [[ -z "${DBWARP_BLUEPRINT_BUILD_DIRTY:-}" ]]; then
  if git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    if worktree_dirty; then
      DBWARP_BLUEPRINT_BUILD_DIRTY=true
    else
      DBWARP_BLUEPRINT_BUILD_DIRTY=false
    fi
  elif [[ -f .dbwarp-source-revision ]]; then
    DBWARP_BLUEPRINT_BUILD_DIRTY=false
  else
    DBWARP_BLUEPRINT_BUILD_DIRTY=unknown
  fi
fi
export DBWARP_BLUEPRINT_BUILD_DIRTY

echo "build.sh: source revision = $DBWARP_BLUEPRINT_BUILD_REVISION"
echo "build.sh: source dirty    = $DBWARP_BLUEPRINT_BUILD_DIRTY"

CARGO_TARGET="${TARGET:-}"
TARGET_ARG=()
if [[ -n "$CARGO_TARGET" ]]; then
  TARGET_ARG=(--target "$CARGO_TARGET")
fi

FEATURES_ARG=()
if [[ -n "${DBWARP_BLUEPRINT_FEATURES:-}" ]]; then
  FEATURES_ARG=(--features "$DBWARP_BLUEPRINT_FEATURES")
  echo "build.sh: enabling Cargo features: $DBWARP_BLUEPRINT_FEATURES"
fi

BUILD_MODE="locked-online"
CARGO_ARGS=(build --release --locked)
if [[ -d vendor-crates ]]; then
  BUILD_MODE="vendored-offline"
  CARGO_ARGS=(build --release --frozen --offline --locked)
elif [[ "${DBWARP_BLUEPRINT_OFFLINE:-0}" == "1" ]]; then
  echo "build.sh: DBWARP_BLUEPRINT_OFFLINE=1 but vendor-crates/ is not present." >&2
  echo "build.sh: download and extract the dbwarp-blueprint-source-vendored release asset, then re-run ./build.sh there." >&2
  exit 3
fi

echo "build.sh: mode = $BUILD_MODE"

echo "build.sh: cargo ${CARGO_ARGS[*]} ${TARGET_ARG[*]} ${FEATURES_ARG[*]}"
cargo "${CARGO_ARGS[@]}" "${TARGET_ARG[@]}" "${FEATURES_ARG[@]}"

if [[ -n "$CARGO_TARGET" ]]; then
  BIN="target/$CARGO_TARGET/release/dbwarp-blueprint"
else
  BIN="target/release/dbwarp-blueprint"
fi
if [[ "${CARGO_TARGET:-}" == *windows* ]]; then
  BIN="$BIN.exe"
fi
if [[ ! -x "$BIN" && ! -f "$BIN" ]]; then
  echo "build.sh: expected binary at $BIN — not found" >&2
  exit 4
fi
SHA="$(sha256_file "$BIN")"
echo
echo "build.sh: binary:    $BIN"
echo "build.sh: sha256:    $SHA"
echo "build.sh: size:      $(wc -c < "$BIN") bytes"
echo "build.sh: toolchain: $PINNED_RUST"
echo "build.sh: done."
