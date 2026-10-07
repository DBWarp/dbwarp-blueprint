# Build dbwarp-blueprint from source

Use this guide if you prefer to build the tool yourself before running it against a database.

## Quick build

```bash
git clone https://github.com/DBWarp/dbwarp-blueprint
cd dbwarp-blueprint
./build.sh
```

The binary is written to:

```text
target/release/dbwarp-blueprint
```

All examples elsewhere use `./dbwarp-blueprint`; after a source build, either
run `target/release/dbwarp-blueprint` directly or copy that file to
`./dbwarp-blueprint` before following them.

If the pinned Rust toolchain is not already installed and reviewed network
access is permitted, opt in explicitly:

```bash
ALLOW_NETWORK=1 ./build.sh
```

## What the build script does

`build.sh` is intentionally conservative:

- reads the pinned Rust version from `rust-toolchain.toml`
- uses your existing `rustc` if it matches the pinned version
- refuses to download Rust unless `ALLOW_NETWORK=1` is set
- pins the rustup bootstrap version and verifies its official SHA-256 before use
- keeps toolchain state under `./build/`
- uses Cargo.lock for reproducible dependency versions
- builds with `cargo build --release --locked` by default
- automatically switches to `--frozen --offline --locked` when run from a vendored source bundle
- refuses `DBWARP_BLUEPRINT_OFFLINE=1` unless `vendor-crates/` is present
- prints the SHA256 of the resulting binary
- stamps the audit with the exact source revision and worktree dirty state

It does not use `sudo` and does not modify your system Rust installation.

## Downloadable binaries

Prebuilt binaries are available on the Releases page:

<https://github.com/DBWarp/dbwarp-blueprint/releases>

They are provided for convenience. Pin an exact release tag and verify its
SHA-256 before use; do not use a mutable download URL for a reproducible run. If
your policy requires source review, build locally from the same tag.

Platform binary archives are operator bundles, not source trees, and cannot be
rebuilt in place. Their copy of this guide and `verify.sh` describes the
matching-source verification path. Use a checkout of the exact release tag or
the release's vendored source archive when you need `build.sh`, Cargo sources,
or a local comparison build.

Release files:

| Platform | File |
|---|---|
| Linux x86_64 | `dbwarp-blueprint-linux-x86_64.tar.gz` |
| Linux ARM64 | `dbwarp-blueprint-linux-arm64.tar.gz` |
| macOS Apple Silicon | `dbwarp-blueprint-macos-arm64.tar.gz` |
| Windows x86_64 | `dbwarp-blueprint-windows-x86_64.zip` |

## Verify a downloaded archive

Linux:

```bash
sha256sum -c SHA256SUMS.txt --ignore-missing
```

macOS:

```bash
shasum -a 256 dbwarp-blueprint-macos-arm64.tar.gz
```

Compare the printed value with the matching line in `SHA256SUMS.txt`.

Windows PowerShell:

```powershell
Get-FileHash .\dbwarp-blueprint-windows-x86_64.zip -Algorithm SHA256
```

Each release also publishes a `dbwarp-blueprint-<platform>.binary.sha256` file
for the extracted executable. See [Download binaries](binaries/README.md) for
the verification command.

## Authentication-specific builds

The default build supports password, token-file, token-env, and TLS flows;
client-certificate mTLS is available for PostgreSQL and MySQL.

SQL Server integrated authentication has platform-specific support:

| Platform | Build command | Purpose |
|---|---|---|
| Linux | GitHub release Linux binary, or `DBWARP_BLUEPRINT_FEATURES=integrated-auth-gssapi ./build.sh` | Password, token and TLS authentication, plus Kerberos / GSSAPI when selected |
| Windows | GitHub release Windows binary, or `cargo build --release --locked --features winauth` | Windows Integrated Auth / SSPI |

Linux release binaries do not require Kerberos libraries to start. They load
the platform GSSAPI runtime only when `--auth-mode integrated` is selected. If
`kinit` works, the required runtime pieces are usually already present. Source
builds enable Kerberos/GSSAPI with `integrated-auth-gssapi` as shown above.

## Build without the script

If your policy prefers direct Cargo commands:

```bash
cargo build --release --locked
```

Windows SSPI build:

```powershell
cargo build --release --locked --features winauth
```

Linux Kerberos build:

```bash
cargo build --release --locked --features integrated-auth-gssapi
```

## Reproducing a release binary

`./build.sh` proves that the reviewed source builds; byte identity additionally
requires the release's complete native build inputs. Check out the exact source
revision recorded in `PROVENANCE.json`, published with each release, use its target and feature list, the
pinned Rust toolchain, the recorded native compiler/linker, the commit timestamp
as `SOURCE_DATE_EPOCH`, and the release workflow's path-remapping and linker
flags. Windows releases also use `clang-cl` and `/Brepro`.

After reproducing those inputs, compare the extracted release binary with the
local result:

```bash
SOURCE_BIN=target/release/dbwarp-blueprint \
  ./verify.sh /path/to/extracted/dbwarp-blueprint
```

If the hashes differ, do not treat the binaries as equivalent. Each release is
built twice and a byte mismatch fails the release. `PROVENANCE.json` records the source revision, target,
features, toolchain, source-date epoch, native compiler, binary size, and hash
needed to assess a local reproduction.

## Vendored dependencies

The normal repository includes patched dependencies under `vendor/` so MySQL
and SQL Server `--tls-ca` inputs have the same restrictive trust semantics as
the rest of the tool, Linux integrated authentication loads GSSAPI only when
requested, and Windows integrated authentication uses a maintained
random-number dependency. All other dependency versions are pinned by
`Cargo.lock`.

Each GitHub Release publishes a separate `dbwarp-blueprint-source-vendored.tar.gz` bundle for security teams that want to inspect and build from every dependency source file offline.

```bash
tar -xzf dbwarp-blueprint-source-vendored.tar.gz
cd dbwarp-blueprint-source-vendored
DBWARP_BLUEPRINT_OFFLINE=1 ./build.sh
```

That bundle contains the patched dependencies under `vendor/`, a generated
`vendor-crates/` tree for all other dependencies, and a generated
`.cargo/config.toml` that redirects crates.io to the local vendor tree. In
that mode, `build.sh` uses `cargo build --release --frozen --offline --locked`.
