# Download dbwarp-blueprint

Prebuilt `dbwarp-blueprint` binaries are published on the GitHub Releases page:

<https://github.com/DBWarp/dbwarp-blueprint/releases>

You can download a binary, verify its checksum, run it locally, and inspect the generated `blueprint.toml` before sharing anything with DBWarp.

Choose an exact release tag, for example
`https://github.com/DBWarp/dbwarp-blueprint/releases/tag/<release-tag>`, then
download the archive and `SHA256SUMS.txt` from that same tag. Do not use a mutable
`releases/latest` URL for a reproducible or audited run.

## Files

| Platform | File |
|---|---|
| Linux x86_64 | `dbwarp-blueprint-linux-x86_64.tar.gz` |
| Linux ARM64 | `dbwarp-blueprint-linux-arm64.tar.gz` |
| macOS Apple Silicon | `dbwarp-blueprint-macos-arm64.tar.gz` |
| Windows x86_64 | `dbwarp-blueprint-windows-x86_64.zip` |
| Offline source audit bundle | `dbwarp-blueprint-source-vendored.tar.gz` |
| Checksums | `SHA256SUMS.txt` |
| Extracted executable checksum | `dbwarp-blueprint-<platform>.binary.sha256` |

Each release also includes `SHA256SUMS.txt`.

Each Linux archive includes SQL Server Kerberos/GSSAPI support without a
Kerberos library startup dependency. The binary loads the platform runtime
only when integrated authentication is selected.

The platform archives are ready-to-run operator bundles, not source trees.
They include operating and audit references, but they cannot be rebuilt in
place. For a source audit or comparison build, use the exact release tag or the
`dbwarp-blueprint-source-vendored.tar.gz` asset from the same release.

## Verify the download

Linux:

```bash
sha256sum -c SHA256SUMS.txt --ignore-missing
```

macOS:

```bash
shasum -a 256 dbwarp-blueprint-macos-arm64.tar.gz
```

Windows PowerShell:

```powershell
Get-FileHash .\dbwarp-blueprint-windows-x86_64.zip -Algorithm SHA256
```

Compare the printed hash with the matching line in `SHA256SUMS.txt`.

After extracting the archive into the same directory, verify the executable
itself with the matching `*.binary.sha256` file from the same release tag. Each
file names the executable inside its extracted folder, for example
`dbwarp-blueprint-linux-x86_64/dbwarp-blueprint`:

```bash
sha256sum -c dbwarp-blueprint-linux-x86_64.binary.sha256
```

On macOS use `shasum -a 256 -c`, and on Windows compare
`Get-FileHash .\dbwarp-blueprint-windows-x86_64\dbwarp-blueprint.exe` with the
value in the `.binary.sha256` file.

## Downloaded binary or local build?

The downloadable binary is for convenience. The strongest trust path is still to build from source:

```bash
git clone https://github.com/DBWarp/dbwarp-blueprint
cd dbwarp-blueprint
git checkout <release-tag>
./build.sh
```

That normal source clone is intentionally small and uses `Cargo.lock` to pin dependency versions.

If your policy requires reviewing every dependency source file before build, download `dbwarp-blueprint-source-vendored.tar.gz` from the same release and build inside that extracted tree:

```bash
tar -xzf dbwarp-blueprint-source-vendored.tar.gz
cd dbwarp-blueprint-source-vendored
DBWARP_BLUEPRINT_OFFLINE=1 ./build.sh
```

See [`../BUILD.md`](../BUILD.md).

## What the tool does

`dbwarp-blueprint` reads database metadata and optionally measures compression on a small local sample. It writes an anonymized text file for DBWarp migration estimation. It does not upload the file and does not send telemetry.
