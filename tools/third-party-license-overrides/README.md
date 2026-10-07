# Third-party licence-file overrides

Some crates in the locked graph declare a licence in `Cargo.toml` but omit the
standalone text from the published crate. The notice builder uses the exact
upstream licence files in this directory for those releases.

| Package | Source |
|---|---|
| `alloc-stdlib 0.3.0` | `dropbox/rust-alloc-no-stdlib`, `LICENSE` at the exact source revision recorded in the published crate |
| `keyed_priority_queue 0.4.2` | `AngelicosPhosphoros/keyed_priority_queue`, `LICENSE.md` on the upstream development branch |
| `libgssapi 0.11.0` | `estokes/libgssapi`, `LICENSE` at the exact source revision recorded in the published crate |
| `quad-rand 0.2.3` | canonical MIT text with the package author's published name; the crate and its exact upstream source revision declare MIT but include no standalone licence file |

Other packages without a bundled standalone file either select the complete
Apache-2.0 text already shipped at the repository root. The current locked
graph has no package represented only by a package-metadata notice.
