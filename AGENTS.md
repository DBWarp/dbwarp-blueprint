# DBWarp Blueprint public repository guidance

This repository contains the public source for `DBWarp Blueprint` and the
`dbwarp-blueprint` binary.

- Preserve the Blueprint naming contract. Legacy serialized identifiers are
  accepted only in compatibility parsers and must never be emitted.
- Never add credentials, private infrastructure identifiers, customer names,
  private hostnames, internal issue history, or operator-only procedures.
- Keep `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `build.sh`, and
  `BUILD.md` consistent.
- Customer-visible CLI and diagnostic changes must update every locale catalog
  and the message documentation in the same change.
- English documentation is authoritative. Machine-translated documentation
  must state that it may contain errors and must never be presented as
  contract-grade. It is supplemental, may follow the English source on a later
  translation cycle, and must not block an otherwise qualified release.
- Run `cargo fmt --all --check`, `cargo test --locked --all-targets`,
  `tools/check_blueprint_core_sync.sh`, and `tools/check_public_tree.py` before
  proposing a release.
- Do not publish releases or change release tags without explicit maintainer
  approval.
