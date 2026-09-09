# Contributing

Start with a non-sensitive issue describing the problem and a small synthetic
reproduction. For substantial changes, discuss the approach before preparing a
patch. Maintainers decide whether a proposed change fits the product and its
security boundaries; opening an issue or pull request does not imply acceptance.

Follow [SUPPORT.md](SUPPORT.md) for safe problem reports and [SECURITY.md](SECURITY.md)
for private vulnerability reporting. Keep discussions respectful and focused on
reproducible behavior.

## Preparing a change

- Use the pinned toolchain and locked dependencies described in [BUILD.md](BUILD.md).
- Keep changes focused and add regression coverage for changed behavior.
- Never include customer data, credentials, private infrastructure details or
  identifying audit evidence in a patch or fixture.
- Preserve explicit consent, bounded source impact, least-privilege access and
  honest reporting of missing or degraded observations.
- Document any Blueprint contract or compression-encoding change. Existing
  optional-field and compatibility rules are part of the interface.
- Keep CLI options, diagnostic codes and serialized fields canonical English.
  Human-facing runtime changes must update every shipped runtime catalog.
  English Markdown is authoritative; translated Markdown is supplemental.

## Local checks

From the source repository, with the pinned toolchain installed:

```bash
cargo fmt --all --check
cargo test --locked --all-targets
./tools/check_blueprint_core_sync.sh
python3 tools/check_public_tree.py
```

The shared core has its own unit suite:

```bash
cargo test --locked --manifest-path crates/dbwarp-blueprint-core/Cargo.toml --lib
```

Local test success is not database-version or platform qualification. Describe
what was actually tested and leave other configurations explicitly untested.
Do not publish artifacts, update release tags or change repository security
settings as part of a patch without maintainer approval.

## Runtime language changes

The canonical source is the English Rust help and the message/UI definitions
in `src/i18n.rs`. When any customer-visible phrase changes:

1. update every locale catalog under `locales/` in the same commit;
2. retain all placeholders and canonical operational tokens exactly;
3. run the focused exact-coverage test;
4. add or update the relevant operator-boundary case in
   `tests/cli_errors.rs` when a failure or warning changes;
5. run the full test suite and inspect representative help/deck output;
6. obtain native technical review before treating new wording as final for a
   customer contract, regulatory filing, or public marketing material.

This exact-coverage workflow applies to runtime catalogs embedded in the
binary. Translated Markdown is supplemental; see
[`docs/TRANSLATIONS.md`](docs/TRANSLATIONS.md).

Focused validation:

```bash
mkdir -p tmp/test-runtime
TMPDIR="$PWD/tmp/test-runtime" \
  cargo test --locked every_embedded_locale_exactly_covers_the_live_cli
TMPDIR="$PWD/tmp/test-runtime" cargo test --locked --test i18n
```

The integration tests also prove that option tokens are identical across
languages, localized DBP codes stay stable, emitted TOML is language-invariant,
and generated deck prose carries the selected locale.
