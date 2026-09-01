# Third-party licence-file overrides

Some crates in the locked graph declare a licence in `Cargo.toml` but omit the
standalone text from the published crate. The notice generator uses the exact
upstream licence files in this directory for those releases.

| Package | Source |
|---|---|
| `alloc-stdlib 0.2.4` | `dropbox/rust-alloc-no-stdlib`, `LICENSE`; identical to the BSD-3-Clause file distributed by the companion `brotli` crate |
| `keyed_priority_queue 0.4.2` | `AngelicosPhosphoros/keyed_priority_queue`, `LICENSE.md` on the upstream development branch |
| `libgssapi 0.4.6` | `estokes/libgssapi`, `LICENSE` |
| `libgssapi-sys 0.2.4` | same upstream repository and licence as `libgssapi` |

Other packages without a bundled standalone file either select the complete
Apache-2.0 text already shipped at the repository root or receive an explicit
package-metadata notice when no upstream text was published with the crate.
