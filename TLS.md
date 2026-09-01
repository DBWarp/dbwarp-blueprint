# TLS and certificates

Use TLS whenever the database connection crosses a network boundary.
`verify-full` is the default: the certificate chain and server hostname are
validated unless the operator explicitly selects another mode.

## Common options

```bash
--tls-mode disable|prefer|require|verify-ca|verify-full
--tls-ca /path/to/ca-bundle.pem
--tls-cert /path/to/client-cert.pem
--tls-key /path/to/client-key.pem
```

Recommended production setting:

```bash
--tls-mode verify-full --tls-ca /etc/pki/internal-root.crt
```

## Internal CA

If your database certificate is signed by an internal CA:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.toml
```

## Hostname mismatch

Use a `--connect` hostname that matches the certificate when running with
`--tls-mode verify-full`. This release does not support overriding the TLS
server name; passing `--tls-server-name` fails loudly rather than silently
weakening verification. If your policy permits CA validation without hostname
validation, use `--tls-mode verify-ca`.

Trust defaults are engine-specific:

- PostgreSQL and MySQL use the Mozilla roots compiled into the binary when
  `--tls-ca` is omitted. A supplied PEM bundle replaces those roots.
- SQL Server uses the operating-system trust store when `--tls-ca` is omitted.
  A supplied `.pem` or `.crt` file must contain exactly one CA certificate and
  replaces the operating-system roots.

The SQL Server driver validates the connection hostname in both `verify-ca`
and `verify-full`; for that engine, `verify-ca` is intentionally no weaker than
`verify-full`.

## Plaintext and compatibility modes

`prefer` is accepted only for loopback targets. PostgreSQL may fall back to
local plaintext in that mode and emits `DBP1404W`; other engines still attempt
TLS. Remote `disable` and `require` modes need `--i-know-what-im-doing` because
they either permit plaintext or encrypt without authenticating the server.
That override records an explicit operator decision; it does not make those
modes suitable for production.

## mTLS

PostgreSQL and MySQL support client-certificate authentication. If either
database requires a client certificate:

```bash
--tls-cert /etc/dbwarp/client.crt \
--tls-key /etc/dbwarp/client.key
```

Private key files must not be group/world readable on Unix-like systems.
SQL Server client-certificate authentication is not implemented; supplying
`--tls-cert`/`--tls-key` with that engine fails with `DBP1015E` instead of
silently ignoring the files.

## Skip verification

`--tls-skip-verify` is for diagnostics only. Do not use it for production database Blueprint collection unless your security team has explicitly approved it.

## Audit log

The audit log records the requested TLS mode, CA path, client cert path, and
whether verification was skipped. After a successful connection it records
whether TLS was negotiated; current drivers do not expose a reliable protocol
version, so the audit says that the version is unavailable rather than
inventing one. It does not log private key contents.
