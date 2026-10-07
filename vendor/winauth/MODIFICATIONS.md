# Modifications to winauth 0.0.5

This directory is based on the `winauth` 0.0.5 crate published by the upstream
project. DBWarp Blueprint modifies the following files:

- `Cargo.toml`: the `rand` dependency metadata is updated from the vulnerable 0.7 series to
  the API-compatible 0.8 series. The two call sites use only
  `thread_rng().fill_bytes`; the interface is unchanged and both versions use
  OS-seeded cryptographic generators.

The upstream project is available at <https://github.com/steffengy/winauth-rs>.
Its declared Apache-2.0 and MIT licence texts are included alongside this
notice.
