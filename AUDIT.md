# What dbwarp-blueprint reads and writes

This document describes the application-level runtime network, filesystem,
environment, database, and audit behavior. Cross-reference each active mode
and option against your security policy. Database drivers, TLS and identity
libraries, the dynamic loader, and the operating system can additionally
consult platform configuration, trust stores, DNS, credential caches, and
network-mounted storage; those support-layer actions are outside the
application audit's exhaustive visibility.

## Network egress

Live `--connect` mode opens one database-driver session to the named endpoint.
Batch mode processes its sources sequentially and opens one session for each
database source. DNS resolution may use the configured resolver, and integrated
Kerberos/SSPI authentication may contact a KDC or domain controller. Offline
TOML, Parquet, Avro, and bundle operations open no application-initiated
network connection, although a path on a network filesystem remains subject to
the host's storage stack.

There is no telemetry, license check, version update, cloud API call, or upload
path in the binary.

You can verify with `strace -f -e trace=connect,sendto,recvfrom`,
`tcpdump`, or eBPF on your platform of choice.

## Filesystem reads

The tool reads the inputs selected by the active mode:

| File | When | Content |
|---|---|---|
| `--user-file PATH` | If supplied | Username only. Trailing whitespace stripped; empty file is an error. |
| `--password-file PATH` | If supplied | Reads once. The DBWarp-owned `Secret` buffer zeroizes on drop; database drivers may retain their own copies as documented in SECURITY.md. Refuses group/other-readable mode on Unix. |
| `--anonymization-key-file PATH` | If supplied | Customer-held 32-byte or 64-hex-character HMAC key. Refuses if mode is world/group readable on Unix. The key is never emitted. |
| `--azure-token-file PATH` | If supplied | SQL Server Entra ID token. Reads once; the DBWarp-owned `Secret` buffer zeroizes on drop. Refuses group/other-readable mode on Unix. |
| `--tls-ca PATH` | If supplied | Trusted CA PEM read at connect time. PostgreSQL/MySQL accept a bundle; SQL Server accepts exactly one certificate. The supplied file replaces the engine's default roots. |
| `--tls-cert PATH` | If supplied | PostgreSQL/MySQL client TLS cert (PEM), read at connect time. Rejected for SQL Server with `DBP1015E`. |
| `--tls-key PATH` | If supplied | PostgreSQL/MySQL client TLS key (PEM). Refuses group/other-readable mode on Unix. Read at connect time; rejected for SQL Server with `DBP1015E`. |
| `--from-toml PATH` | If supplied | Existing dbwarp-blueprint TOML file, read locally to build a deck without a database connection. |
| `--from-parquet PATH` | If supplied | Parquet metadata and, only with explicit sampling consent, bounded decoded rows. |
| `--from-avro PATH` | If supplied | Avro container metadata and records; the container is walked to obtain its row count. |
| `--batch-manifest PATH` | If supplied | Manifest and every local input, credential, token, and TLS path it references. |
| `--bundle-list`, `--bundle-extract`, `--bundle-pack` | If supplied | Bundle TOML plus relative Blueprint files needed for listing, extraction, or packing. |
| controlling terminal/console (`/dev/tty` on Unix-like systems) | If no password source supplied | Echo-disabled prompt. |
| (build-time only) `rust-toolchain.toml`, `Cargo.toml`, `Cargo.lock`, `.dbwarp-source-revision` in vendored releases, `vendor/mysql_async`, `vendor-crates/*` in offline bundles | Only when `./build.sh` runs | Toolchain, source provenance, and standard Cargo build inputs |

The application has no explicit path to read:
- `~/.pgpass`, `~/.my.cnf`, `~/.aws/credentials`, `~/.azure/credentials`
- Any `~/.ssh/*` files
- `/etc/passwd` or `/etc/shadow` as Blueprint inputs (platform identity and
  authentication libraries may still consult operating-system account data)
- Any database credential environment variable other than the one named via
  `--password-env`, `--user-env`, or `--azure-token-env`. Integrated Kerberos
  builds may also cause the platform GSSAPI/Kerberos stack to consult its own
  configuration, cache, keytab, and environment settings. Locale and
  terminal-presentation variables are described below.

## Filesystem writes

The tool writes only outputs selected by the active mode:

| File | When | Content |
|---|---|---|
| `--out PATH` (default `./blueprint.toml`) | Live database, Parquet, Avro, bundle-extract, and bundle-pack runs | Blueprint or packed-bundle TOML. Not written by deck-only, bundle-list, dry-run, or help/version modes. |
| `--deck PATH` | Only if specified | A PowerPoint (.pptx) deck summarising the anonymized Blueprint. Built locally from the same in-memory Blueprint, or from `--from-toml` input — no extra database read, no network, no third-party library. |
| `--audit-log PATH` | Only if specified | An atomic replacement copy of the audit log emitted to stderr. Existing content is not appended. |
| `--out-dir DIR` | Non-dry-run batch mode | `bundle.toml`, per-source `blueprints/` and `audits/`, an ownership marker, and `errors.txt` after partial failure. Publication uses a sibling staging directory and recovery marker. |
| (build-time only) `./target/`, `./build/` | Only when `./build.sh` runs | Standard Cargo build outputs |

The application has no explicit path to write:
- `/var/log/*`
- `~/.cache/*`, `~/.local/*`, `~/.config/*`
- an implicit system temporary directory (the user may still direct an output
  or batch directory there explicitly)

## Environment variables read

The audit lists only variables consulted by DBWarp Blueprint itself. Locale selection may read
`DBWARP_BLUEPRINT_LANG`, `LC_ALL`, `LC_MESSAGES`, and `LANG`, in that order,
when `--lang` does not already select a supported locale. Terminal rendering may read `NO_COLOR`,
`TERM`, `COLORTERM`, and `COLUMNS`; these affect presentation only.

When `--password-env VAR_NAME` or `--user-env VAR_NAME` is specified,
the tool reads exactly that named variable. There is no fallback to
common defaults like `PGPASSWORD`, `MYSQL_PWD`, `MSSQL_PASSWORD`,
`USER`, or `LOGNAME` — those fallbacks are deliberately not
implemented.

Platform database, TLS, DNS, and integrated-auth libraries can consult their
own variables and configuration outside this application-level list. Use an
OS-level trace when policy requires a complete process-and-library inventory.

When `./build.sh` runs, it reads `PINNED_RUST` (override), `ALLOW_NETWORK`
(opt-in for rustup-init download), `TARGET` (cross-compile target), plus
the standard cargo / rustup vars. None of these are read by the tool
itself at runtime.

## Per-run audit log

The tool emits an audit log to stderr on every run using a stable plain-text
layout. Pipe to a file with `2>audit.txt` or use
`--audit-log PATH` for an explicit copy.

Sample (Tier 1):

```
=== dbwarp-blueprint audit ===
build_source_revision: 0123456789abcdef0123456789abcdef01234567
build_source_dirty:    false
build_toolchain:     1.94.0 (vendored)
mode:                tier-1
started_at_unix_ms:  1745596800000
outcome:             ok
anonymization_key:   ephemeral-random
schema_selector_count: 1

connection:
  - postgresql://app@db.example:5432/payments
    auth: scram-sha-256-or-md5
    tls: yes (protocol version unavailable from driver)
    tls_ca_only: false

auth:
  user_source:        file:/etc/dbwarp/db.user
  password_source:    file:/etc/dbwarp/db.pass (mode 0o600)
  password_persisted: false
  password_logged:    false
  authenticated_principal: (not observed)
  effective_server_principal: (not observed)
  database_principal: (not observed)
  expected_server_principal: (not requested)
  principal_assertion: not-observed

topology_and_scope:
  topology:
    deployment: unknown
    local_role: unknown
    visibility: partial
    member_count: 2
    identifiers_redacted: true
    role_counts: primary=1, secondary=1
    features: postgresql-streaming-replication
    catalogs_read: pg-is-in-recovery, pg-stat-replication
    catalogs_unreadable: (none)
  dataset_scope:
    layout: full-copy
    table_inventory_completeness: complete
    row_count_completeness: complete
    size_completeness: complete
    row_count_method: postgres-planner-estimate
    size_method: postgres-local-relation-size
    limitations: row-counts-statistical

blueprint_fidelity_estimate:
  basis: evidence-coverage-v1
  overall_score: 79/100
  band: good
  structure_score: 90/100
  sizing_score: 100/100
  column_statistics_score: 68/100
  relationship_score: 75/100
  artifact_score: 50/100
  limitations: biased-column-sampling, cardinality-lower-bounds
  qualification: evidence estimate, not source-truth accuracy or a confidence interval

artifact_inventory:
  detail: summary
  visibility: full
  objects: 42
  dependency_edges: 0
  external_prerequisites: 3
  inventory_complete: false
  dependencies_complete: false
  analysis_complete: false

database_operations_observed:
  1. [succeeded, 14ms, 28 rows]   server version lookup
  2. [succeeded, 9ms, 312 rows]   column catalog lookup
  ... (every observed catalog operation enumerated)

wire_bytes_observed:
  catalog_responses: unknown (driver does not expose wire-byte totals)
  row_data:          unknown (driver does not expose wire-byte totals)

local_sample_processing:
  encoded_rowframe_bytes: 0 B

sampling_work:
  compression_workers: 0
  compression_queue_capacity: 0
  compression_jobs_submitted: 0
  compression_jobs_completed: 0
  compression_pipeline_wall_ms: 0
  compression_worker_ms: 0
  tables_skipped_proven_empty: 0
  chunk_level_3_attempts: 0
  table_level_3_attempts: 0
  column_level_3_attempts: 0

files_read_local:
  - /etc/dbwarp/db.pass        (mode 0o600 ✓)

files_written_local:
  - ./blueprint.toml         (12 KiB, sha256: 7f3e2af1...)

warnings:
  - (none)

network_egress:
  - db.example:5432 (database-driver session; DNS may use the configured resolver)

env_vars_read:
  - (none)

trust_assertions:
  - no row content was read
  - no telemetry was sent anywhere
  - length policy balanced: declared capacities and index prefixes exact; sampled lengths relatively rounded
  - identifier ordering uses domain-separated HMAC-SHA256 with a fresh process-local key; labels intentionally vary between runs
  - the anonymization key and source identifiers are not written to the Blueprint
  - artifact summary stores bounded counts and external-prerequisite classes; no object identities or definitions
  - artifact output excludes source object names, SQL text, endpoints, credentials, keys, certificates, and binaries
  - credential entered through the Secret wrapper and its buffer is zeroized on drop; driver APIs may retain copies as documented under 'Driver-owned credential copies' in SECURITY.md

run_duration_ms:    142
finished_at_unix_ms: 1745596800142
=== end audit ===
```

MySQL runs emit a mode-specific `length policy balanced|strict|exact`
assertion. It states independently whether structural and sampled lengths are
exact or rounded, so the audit never claims that all numeric values were
rounded for a balanced or exact run.

The audit log:

- Identifies the source revision embedded at compile time and whether worktree
  changes were present. The final binary SHA-256 remains an external
  release/registry checksum because a binary cannot embed its own final hash.
- Records the **source** of the credential (file path, env var name,
  TTY) — never the value.
- On SQL Server, records the exact session identities reported by
  `ORIGINAL_LOGIN()`, `SUSER_SNAME()`, and `USER_NAME()`. When
  `--expect-server-principal` is supplied, it also records the expected value
  and whether the server-side comparison matched before catalog capture.
- Records only the number of repeatable live `--schema` selectors; their
  values are shown in the interactive pre-flight but are not added to the
  audit. The existing redacted connection URI still identifies the connected
  database, which is also the schema name on MySQL. A selected Blueprint is
  marked `selection-limited` in `dataset_scope`.
- Lists every observed database operation with outcome, elapsed time, and row
  count when the driver supplied one. Failed terminal operations use a bounded,
  identifier-free label.
- Reports database wire-byte totals as `unknown` unless a driver can expose
  them, and separately reports locally encoded sample bytes.
- Reports total bytes written locally (with sha256 of each file).
- Records non-fatal capture and sampling degradations with stable DBP warning
  codes; an empty section means no known degradation was observed.
- Copies validated `[database_topology]` and `[dataset_scope]` evidence into
  `topology_and_scope` using closed tokens and counts only; node names,
  endpoints, cluster identifiers, and database identifiers cannot appear.
- Retains `DBP1411W`, `DBP1412W`, and `DBP1413W` when topology or dataset
  coverage is incomplete, so a successful capture cannot hide a sizing caveat.
- Records a deterministic, dimensioned Blueprint fidelity estimate. The score
  describes captured evidence coverage for structure, sizing, column
  statistics, relationships, and artifacts. It is not measured error against
  source truth and is not a statistical confidence interval.
- Declares trust assertions appropriate to the mode (Tier 1 vs Tier 2).
- Uses a stable text format, but values can differ with database state, timing,
  warnings, and the default fresh anonymization key. For approved comparisons,
  reuse a protected `--anonymization-key-file` and pin `--generated-at`; timing
  fields still vary.

**Trust-assertion conditional emission.** The
"credential read once via Secret wrapper..." line is emitted only on
runs where a credential was actually read. Failure paths that abort
before credential acquisition (URI parse errors, refusal of
URI-embedded passwords, dry-run, etc.) intentionally do *not* emit
this line — there's nothing to assert about a credential that was
never obtained. Use the presence/absence of the line plus
`auth.password_source` to tell whether credential handling was
exercised on a given run.

**The audit is emitted on operational success and failure paths**, including
command-line parse failures after startup. Help/version exits and failures that
occur before the embedded localization contract can be loaded do not emit a
full audit. If the tool fails partway through (auth refused, network error),
the audit log still prints to stderr (and to `--audit-log PATH` if
specified) with `outcome: error: <stage>` so the customer always has a
forensic record of what was attempted before the failure.
Example failure outcome line:

```
outcome:             error: parsing --connect URI (value redacted to avoid logging embedded credentials)
```

The terminal output also includes a coded operator summary such as
`DBP1001E` or `DBP0001E` with the causal chain. The audit outcome is
bounded and may truncate long text; use the terminal output plus the
message code for support triage. See `docs/MESSAGES.md`.

Optional RTT, compression, and text-style probes may fail without invalidating
the primary catalog capture. Those cases are printed and retained under
`warnings:` as `DBP1405W` through `DBP1408W`, so a successful but partial Tier
2 result is distinguishable from a complete result. Repeated identical
warnings are deduplicated and multi-line driver details are flattened to keep
the audit bounded and machine-scannable.

## Non-table artifact reads

Artifact capture is independent from Tier 2 row sampling:

- `--artifact-detail none` skips artifact catalogs and definitions.
- `summary` reads modeled object catalogs but not definition text.
- `graph` additionally reads dependency catalogs but not definition text.
- `analyzed` additionally reads available SQL/procedural definitions into
  bounded process memory for lexical analysis.

The audit records the requested detail, visibility, object/dependency/external
counts, and all completeness flags. Every artifact catalog operation appears in
`database_operations_observed`. A failed optional catalog emits `DBP1410W`, appears under
`warnings`, and prevents an inaccurate complete claim.

In analyzed mode, definitions are wrapped in a zeroizing owner, scrubbed, and
reduced to bounded bands and closed feature tokens. Definition text, source
object names, external endpoints, artifact principals, credentials,
key/certificate material, package/library names, and binaries are never written
to the Blueprint or audit log. The only exact principal names retained are the
three SQL Server session identities in the explicit `auth` audit block above;
they are never written to the Blueprint, deck, or publication artifacts. Graph
and analyzed modes require `--yes` because anonymous topology can still
fingerprint an application.

The audit distinguishes the two privacy postures with one of these trust
assertions:

- summary: bounded counts only, no object identities or definitions;
- graph: anonymous dependency graph, no definitions;
- analyzed: definitions read transiently, only bounded feature bands retained.

See [`docs/ARTIFACT_INVENTORY.md`](docs/ARTIFACT_INVENTORY.md) for object-family
coverage and completeness interpretation.

## Tier 2 additions

When compression measurement is accepted interactively, or non-interactively
with `--measure-compression --yes`, the tool additionally:

- For each table not proven empty, runs an engine-specific bounded sampling
  path. PostgreSQL starts with `TABLESAMPLE SYSTEM(0.1) LIMIT N` and falls back
  to `LIMIT N` when needed; MySQL uses `LIMIT N`; SQL Server uses `TOP N`.
  Biased paths set `sampled_with_bias = true` in the output.
- Reads the sampled rows into a local in-memory buffer.
- Keeps database reads sequential. `--compression-workers N` may run 1–32
  bounded local compression workers (default 1 to minimize source-host impact).
  Increase it explicitly to use more local CPU. Each worker owns its zstd
  contexts, so the workers do not contend on a shared zstd lock.
- Compresses with zstd at level 3.
- Records the resulting ratios + stddev.
- **Discards each buffer when its bounded local compression job completes**.
  The bytes are not written to disk or transmitted anywhere. At most N queued
  samples plus N actively compressed samples are retained by the worker pool.

`local_sample_processing.encoded_rowframe_bytes` in the audit log shows the
number of bytes encoded locally for compression. It is not a database wire-byte
counter; database wire bytes remain `unknown` when the driver does not expose
them. The output file's per-table `[compression]` block records the ratio
numbers. `--max-wall-secs` is a hard deadline for the complete live capture,
including connection setup, catalogs, RTT probes, and Tier 2 sampling.
PostgreSQL also sets session `statement_timeout`; MySQL sets session
`max_execution_time` for read-only `SELECT`; SQL Server sets session
`LOCK_TIMEOUT` because it has no equivalent session-wide elapsed-statement
limit. At the outer deadline the client drops the connection. The audit does
not treat that drop as proof that SQL Server acknowledged cancellation, so an
operator must confirm server work stopped before retrying.

`sampling_work` is identifier-free operational evidence. It records the local
worker and queue bounds, the 16 MiB per-table projected payload ceiling,
submitted and completed jobs, compression attempts,
and tables omitted from sampling because the engine catalog proved them empty
at catalog-read time. `compression_worker_ms` is aggregate worker wall time,
not process CPU time, and may exceed `compression_pipeline_wall_ms` when
workers overlap. The pipeline wall time can overlap the still-sequential
database reads. These counters describe work performed; they are not database
row counts, wire-byte measurements, or source-accuracy claims.

## Verification protocol

If you want to *prove* the tool is doing only what's documented:

1. **Source audit**: clone the repo, read `src/secret.rs`, then grep
   for `\.expose\(\)` outside that file:
   ```
   $ rg -n '\.expose\(\)' src --glob '!secret.rs'
   ```
   The production call sites immediately hand the exposed `&str` to a driver's
   connection-builder. MySQL additionally calls `.to_string()` because
   `mysql_async`'s API requires `String`; that copy is non-zeroizing and lives
   until the `OptsBuilder` is dropped. Tier 1 and Tier 2 reuse the same MySQL
   connection. See **Driver-owned credential copies** in SECURITY.md for the
   full discussion.
2. **Build from source**: `./build.sh`. Release CI performs an independent
   same-runner rebuild in a separate Cargo target directory and rejects a
   byte mismatch. A local comparison is meaningful only with the same source
   revision, target, features, pinned Rust toolchain, linker, and build flags.
3. **Compare to release**: from a build of the matching source revision, run
   `./verify.sh /path/to/extracted/dbwarp-blueprint`. See **Reproducing a
   release binary** in BUILD.md for the required target, features, toolchain,
   linker, source-date epoch, and build flags.
4. **Runtime trace**: on Linux, run with
   `strace -f -e trace=open,connect,read,write` in a sandbox. If `strace` or
   `rg` is unavailable, use the equivalent file/network tracer and recursive
   text-search tool approved for your platform. Compare against the lists
   above.
5. **Network trace**: `tcpdump` on the host. In a password-authenticated live
   run, verify the database session plus expected DNS traffic. For integrated
   authentication, also account for expected KDC/domain-controller traffic. In
   batch mode, reconcile one database session per database source.

If any of these do not match what is documented here, report the discrepancy
through the channel in SECURITY.md and include the smallest safe trace needed
to reproduce it. Do not put credentials, customer identifiers, or sensitive
driver output in a public issue.
