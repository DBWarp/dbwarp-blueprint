# Modifications to tiberius 0.13.0

This directory is based on the `tiberius` 0.13.0 crate published by the
upstream project. DBWarp Blueprint modifies the following files:

- `src/client/config.rs` and `src/client/tls_stream/rustls_tls_stream.rs` add a
  rustls-only `Config::trust_cert_ca_only` method. It constructs the TLS root
  store from the supplied CA file without also trusting the operating-system
  root store, preserving the documented restrictive meaning of `--tls-ca`.
- `src/lib.rs` adds the public `integrated_auth_runtime_available` function so
  a caller can check whether the platform Kerberos runtime is available before
  attempting integrated authentication.

The upstream additive `Config::trust_cert_ca` and
`Config::trust_cert_ca_bundle` methods are unchanged.

The upstream project is available at
<https://github.com/prisma/tiberius>. Its declared Apache-2.0 and MIT licence
texts are included alongside this notice.
