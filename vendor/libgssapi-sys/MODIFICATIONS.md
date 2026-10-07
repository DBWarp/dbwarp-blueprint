# Modifications to libgssapi-sys 0.3.4

This directory is based on the `libgssapi-sys` 0.3.4 crate published by the
upstream project. DBWarp Blueprint modifies the following files:

- `build.rs` compiles the lazy-loading shim on Linux instead of linking a
  GSSAPI library into the executable. `LIBGSSAPI_PREFIX` still selects headers
  at build time, but no longer changes the Linux runtime library search.
- `src/lazy_gssapi.c` is new. It loads MIT Kerberos or Heimdal with `dlopen` on
  first use and resolves only the GSSAPI calls used by the collector.

The upstream bindings and non-Linux linking behaviour are unchanged. The
collector can start without a Kerberos runtime library; integrated
authentication reports an unavailable platform runtime if neither supported
library can be loaded.

The upstream project is available at <https://github.com/estokes/libgssapi>.
Its declared MIT licence text is included alongside this notice.
