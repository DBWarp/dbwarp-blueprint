# Security model

`dbwarp-blueprint` has separate live-database, structured-file, batch, bundle,
and deck modes. The selected mode determines its network and filesystem scope.
It has no telemetry, update check, license check, analytics call, or upload path.

This page explains the security boundaries so your team can decide whether to run it.

## Reporting a vulnerability

Please report suspected vulnerabilities privately through
[GitHub private vulnerability reporting](https://github.com/DBWarp/dbwarp-blueprint/security/advisories/new).
Do not include security-sensitive details in a public issue. Include the exact
release version, operating system, reproduction steps, and the smallest safe
evidence needed to assess the report.

## Network

| Mode | Runtime network use |
|---|---|
| Live `--connect` | One database-driver session to the named database endpoint. DNS resolution may contact the configured resolver. Integrated Kerberos/SSPI authentication may also contact configured identity infrastructure such as a KDC or domain controller. |
| `--batch-manifest` | One database-driver session for each database source in the manifest, processed sequentially. Local Parquet and Avro sources use no network. DNS and integrated-auth qualifications above still apply. |
| `--from-toml`, `--from-parquet`, `--from-avro`, `--bundle-list`, `--bundle-extract`, `--bundle-pack` | No application-initiated network connection. Inputs on network-mounted filesystems remain an operating-system/storage concern. |

The tool does not call a DBWarp service or cloud API. Database drivers and the
host operating system may perform protocol support traffic described above and
may consult system trust stores, DNS configuration, dynamic libraries, and
integrated-auth configuration or credential caches.

`--max-wall-secs` sets two independent protections. PostgreSQL uses a
session-local `statement_timeout`, and MySQL uses session-local
`max_execution_time` for the collector's read-only `SELECT` statements. SQL
Server has no equivalent session setting for total statement elapsed time, so
the collector sets session-local `LOCK_TIMEOUT` to bound lock waits and retains
the client wall deadline for other stalls. If that client deadline expires, the
tool drops its connection; it does not claim that SQL Server acknowledged a
server-side cancellation. Confirm server work stopped before retrying.

## Files read

At runtime, the application directly reads only inputs selected on the command
line or referenced by a batch/bundle input:

| File | When |
|---|---|
| `--user-file` | username source |
| `--password-file` | password source |
| `--anonymization-key-file` | optional customer-held HMAC key used by the binary or SQL fallback normalizer to preserve anonymous object labels across approved runs; mode must prevent group/other read on Unix |
| `--azure-token-file` | SQL Server Entra ID token source |
| `--tls-ca` | trusted CA bundle |
| `--tls-cert` | client TLS certificate |
| `--tls-key` | client TLS private key |
| `--from-toml` | existing dbwarp-blueprint TOML file used to build a deck offline |
| `--from-parquet` | Parquet file metadata and, with explicit sampling consent, bounded decoded rows |
| `--from-avro` | Avro object-container metadata and records; Avro must be walked to count records |
| `--batch-manifest` | batch manifest plus every local structured file, credential file, token file, and TLS file that it references |
| `--bundle-list`, `--bundle-extract`, `--bundle-pack` | bundle TOML and any relative Blueprint files required by the selected operation |
| controlling terminal or console | interactive password prompt (`/dev/tty` on Unix-like systems) |

The application has no explicit fallback that reads `~/.pgpass`, `~/.my.cnf`,
cloud credential files, SSH keys, shell history, or default database-password
environment variables. Platform database, TLS, DNS, and identity libraries can
still consult their own system configuration and credential caches; use an
OS-level trace when policy requires a complete process-and-library inventory.

For PostgreSQL and MySQL, a supplied `--tls-ca` PEM bundle replaces the
compiled Mozilla roots. SQL Server uses the operating-system trust store when
`--tls-ca` is omitted; a supplied `.pem` or `.crt` file must contain exactly
one CA certificate and replaces those roots. SQL Server validates the hostname
in both certificate-verifying modes and rejects `--tls-cert`/`--tls-key` with
`DBP1015E` because its driver does not implement client-certificate
authentication.

## Files written

At runtime, the tool may write:

| File | When |
|---|---|
| `--out` | Blueprint output for live database, structured-file, bundle-extract, or bundle-pack modes |
| `--deck` | optional PowerPoint (.pptx) summary, generated locally from the anonymized Blueprint or from `--from-toml` input (no extra database read, no network, no third-party library) |
| `--audit-log` | optional copy of the audit log |
| `--out-dir` | batch directory containing `bundle.toml`, `blueprints/*.blueprint.toml`, `audits/*.audit.txt`, an ownership marker, and `errors.txt` when one or more sources fail; a sibling staging directory is used during atomic publication and removed on handled failure |

The audit log is also printed to stderr.

Treat every audit and batch `errors.txt` as access-controlled operational
evidence. They may contain endpoint names, local paths, manifest source ids,
driver errors, and timing data. For SQL Server, the audit includes the exact
authenticated login (`ORIGINAL_LOGIN()`), effective
server principal (`SUSER_SNAME()`), and database principal (`USER_NAME()`),
plus an optional expected principal and assertion result. These identities are
not written to a single-source Blueprint or deck. Bundle metadata retains
operator-supplied source ids, tags, and dataset-group ids, so choose anonymous
values and review bundle TOML before transfer.

## Environment variables

By default, no runtime environment variables are read for credentials.

If you pass `--password-env NAME`, `--user-env NAME`, or `--azure-token-env NAME`, the tool reads exactly that named variable. It does not fall back to common defaults such as `PGPASSWORD`, `MYSQL_PWD`, or `MSSQL_PASSWORD`.

## Credentials

Credentials are wrapped in a `Secret` type that intentionally does not implement `Debug`, `Display`, `Clone`, or serialization. That makes accidental logging difficult to compile.

Credentials are handed to the database driver only for connection setup. They are not written to the output file or audit log. The audit log records the credential source, such as `file:/etc/dbwarp/db.pass`, not the value.

## Driver-owned credential copies

Zeroization covers the `Secret` buffer owned by DBWarp Blueprint; it cannot
guarantee erasure of copies made inside a database driver, TLS library,
operating-system authentication provider, or allocator. The current MySQL
driver API requires an owned `String`, so `src/engine_mysql.rs` explicitly
copies the password from `Secret` into `OptsBuilder`. That copy is not
zeroizing and remains live until the builder/options are dropped. PostgreSQL,
SQL Server, and platform authentication libraries may also make internal
copies outside the wrapper's control. Restrict process inspection and swap as
appropriate for credentials used on sensitive hosts.

## Refused credential patterns

Passwords embedded in the connection URI are refused. For example, this is not accepted:

```text
postgresql://user:password@host/db
```

Use `--password-file`, `--password-env`, or the interactive prompt instead. This avoids leaking passwords through shell history, process listings, or terminal scrollback.

## Output safety

The Blueprint file is designed to be human-readable and reviewable:

- real identifiers are replaced with keyed anonymous names such as `table-001` and `col-1`
- numeric values use the exact or rounded precision documented for each field;
  exact length modes require explicit consent
- comments are fixed and not used as a data channel
- row values are never emitted
- compression samples, when enabled, are compressed locally and discarded

Live Tier 2 applies a hard 16 MiB projected payload ceiling per table before
the database driver receives row data. It reduces the requested row count for
extremely wide tables and projects variable-width cells through engine-native
server-side truncation. Style probes are separately capped in their SQL
projection. The local row-frame encoder independently enforces the same table
ceiling. This prevents a small `--sample-rows` value from transferring an
unbounded LOB payload; it also means very large values contribute only their
bounded prefixes to compression and length estimates.

Table, schema, index, and non-table-object ordering uses domain-separated
HMAC-SHA256. By default the tool obtains a fresh process-local key from the
operating system and never emits it, preventing an offline reader from checking
candidate source names. Use `--anonymization-key-file` only when the same
anonymous labels must survive across approved comparison runs. The file must
contain exactly 32 raw bytes or 64 hexadecimal characters and must be protected
like a credential. The audit records whether an ephemeral or customer-held key
was used, never the key value.

The stdlib-only `blueprint_format.py` normalizer used by the SQL fallback has
the same keyed-ordering contract. It obtains a fresh operating-system-random
key unless `--anonymization-key-file` is supplied and adds a fixed producer/key
source comment after the canonical header so its output cannot be confused
with output emitted by the Rust collector.

Before normalization, the SQL fallback's intermediate JSON contains real
schema, table, column, and index names. MySQL `COLUMN_TYPE` can additionally
contain declared enum/set members. The SQL scripts have no schema-subset
selector and produce no application audit log. Keep the intermediate JSON
inside the source environment as sensitive schema material, normalize it
locally, and transfer only the reviewed TOML. Use the Rust collector when only
selected schemas are approved or when complete audit, topology, artifact, or
sampling evidence is required.

This reduces disclosure risk; it does not make every output safe for every
recipient. Anonymous schema shape, dependency graphs, engine versions, exact
opt-in fields, and unusual size distributions can fingerprint a workload.
Review Blueprint and bundle outputs under your organization's data-classification
policy before sharing. Do not send audits or `errors.txt` as if they were
anonymized Blueprints.

See [`FORMAT.md`](FORMAT.md) for the exact fields.

## Audit log

Every run emits an audit log that lists:

- database endpoint contacted
- credential source used
- SQL Server authenticated, effective server, and database principals when the
  connected session can report them
- TLS mode
- files read
- files written
- queries executed
- whether row sampling was enabled
- final outcome

See [`AUDIT.md`](AUDIT.md).

## Source review starting points

For a focused review:

- `src/secret.rs`: credential wrapper
- `src/main.rs`: CLI, consent gates, audit emission
- `src/audit.rs`: audit log rendering
- `src/format.rs`: anonymized output format
- `src/tls.rs`: TLS configuration
- `src/engine_pg.rs`, `src/engine_mysql.rs`, `src/engine_mssql.rs`: database-specific catalog readers
