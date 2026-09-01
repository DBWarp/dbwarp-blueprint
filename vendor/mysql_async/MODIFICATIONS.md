# Modifications to mysql_async 0.34.2

This directory is based on the `mysql_async` 0.34.2 crate published by the
upstream project. DBWarp Blueprint modifies the following files:

- `src/io/tls/rustls_io.rs`: when callers supply explicit root certificates,
  the TLS root store contains only those certificates instead of appending the
  public WebPKI root set. This implements the restrictive `--tls-ca` contract.
- `src/opts/mod.rs`: the return type of `PathOrBuf::read` names the elided
  `Cow<'_, [u8]>` lifetime explicitly for current Rust compiler compatibility;
  runtime behaviour is unchanged.

The upstream project is available at <https://github.com/blackbeam/mysql_async>.
Its Apache-2.0 and MIT licence texts remain alongside this notice.
