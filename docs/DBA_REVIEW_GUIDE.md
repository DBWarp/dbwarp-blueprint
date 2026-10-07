# DBA Review Guide

This guide is for DBAs and security reviewers deciding whether to run `dbwarp-blueprint` in a production or production-like environment.

## Execution Model

`dbwarp-blueprint` is a local command-line binary. In live mode it opens one database connection to the URI you provide and writes a local TOML file. It does not contact DBWarp infrastructure, cloud APIs, telemetry endpoints, license servers, or update servers.

In `--from-toml` deck mode it does not connect to a database at all.

## Recommended Account

Use a dedicated low-privilege account with read access to catalog metadata and, if Tier 2 compression is enabled, permission to sample rows from user tables.

Recommended properties:

- no write privileges;
- no DDL privileges unless the reviewer explicitly approves MySQL enhanced
  capture, whose `TRIGGER` and `EVENT` metadata privileges are DDL-capable;
- no superuser/admin role;
- read access limited to the database being assessed;
- password or token supplied by file or prompt, not embedded in the URI.

Exact grants vary by engine and your policy. If the account cannot read some catalog views or sample some tables, the tool fails clearly or emits a reduced Blueprint; keep the audit log.

Use the version-aware scripts and caveats in
[`../sql/grants/README.md`](../sql/grants/README.md). After the approved capture,
remove the dedicated collector account with the matching script under
`sql/revoke/`; review the exact database, host pattern, role, and login targets
before execution.

## Tier 1: Metadata-Only (No Row Sampling)

Tier 1 is the default when `--measure-compression` is absent.

It reads:

- engine version;
- table list and anonymized ordering inputs;
- approximate row counts;
- table and index sizes;
- column type families, nullability, and rounded length statistics where available;
- index type, uniqueness, and anonymized column ordinals;
- foreign-key graph shape where available;
- best-effort coarse source-capacity bands returned by the database endpoint;
- bounded non-table object and external-prerequisite counts from object
  catalogs under the default `--artifact-detail summary` (no definitions);
- optional RTT probe unless `--no-rtt-probe` is set.

It does not read row values.

## Source Environment

The schema-v7 `[source_environment]` block is derived only from values returned
through the selected database connection. The collector never inspects its own
host or presents that workstation as the database server.

PostgreSQL and MySQL expose a database buffer setting under the normal minimum
grants, so memory is partial evidence with basis `database-buffer-cache` and
CPU remains unknown. SQL Server requests source-environment capacity only with
`--artifact-detail graph` or `analyzed`, the enhanced-tier modes. The basic and
standard modes do not issue an operating-system capacity query and record the
capacity bands as `not-requested`. The enhanced script grants the required
server-wide `VIEW SERVER STATE` (2019) or `VIEW SERVER PERFORMANCE STATE`
(2022/2025) in a separate batch that a DBA can remove. If an enhanced capture
cannot read the DMV, capture continues and records the catalog as unreadable
rather than borrowing local-machine values or inventing capacity.

No cloud, Kubernetes, hypervisor, or operating-system API is contacted by this
capture path.

## Non-Table Artifact Inventory

Blueprints inventory non-table objects independently from row
sampling. The
default `--artifact-detail summary` reads object catalogs but not definitions
and emits only bounded counts and external-prerequisite classes.

`--artifact-detail graph --yes` adds anonymous object ids and dependency edges.
`--artifact-detail analyzed --yes` also reads available definitions transiently
and emits only bounded lexical feature/complexity bands. Definition text,
source object names, endpoints, provider strings, principals, secrets, keys,
certificates, package names, and binaries are never serialized.

Catalog privileges affect absence claims. Review `visibility`,
`inventory_complete`, `dependencies_complete`, `requirements_complete`,
`catalogs_unreadable`, and `families_not_inventoried`; do not interpret a zero
count or an empty requirements list as proof when those fields disclose a gap.
At graph/analyzed detail, also review each object's `requirement_status`: only
`complete` makes an empty list proof of zero requirements for that object.
`partial` preserves known facts without claiming exhaustive coverage;
`unavailable` means no usable coverage was established. Both leave that
object's requirement-derived coupling assessment unknown.
`DBP1410W` identifies an optional artifact catalog that could not be read.

Anonymous dependency topology can still fingerprint an application. Approve
`graph` or `analyzed` only when that risk is acceptable. See
[`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md).

## Tier 2: Compression Measurement

Tier 2 is enabled only by the explicit pair:

```bash
--measure-compression --yes
```

Tier 2 additionally reads bounded row samples into process memory. The sampled
bytes are encoded into an in-memory buffer and used to derive
aggregate compression, null-density, cardinality/frequency, length, and style
measurements before the values and temporary fingerprints are discarded.

The sample bytes are:

- not written to `blueprint.toml`;
- not written to the audit log;
- not written to temp files;
- not sent over any network other than the database connection;
- not retained after the sample is summarized.

Tier 2 is valuable because transfer time and egress cost depend on compressed bytes, not raw table bytes.

## RTT Probe

By default, the tool runs five `SELECT 1` queries after connection setup. This emits a `[network]` block containing `connect_total_ms`, `query_rtt_ms_p50`, and `query_rtt_ms_p95`.

The probe exists to help operators understand where the Blueprint tool ran relative to the source database. It is not the migration WAN RTT.

Disable it with:

```bash
--no-rtt-probe
```

## Files Read

At runtime, the tool reads only files explicitly selected on the command line
or referenced by an explicitly selected batch manifest or bundle. These can
include password files, user files, anonymization-key files, TLS CA/cert/key
files, Entra token files, structured-file inputs, and Blueprint or bundle
inputs.

It deliberately does not read common implicit credential locations such as `~/.pgpass`, `~/.my.cnf`, cloud credential files, SSH keys, shell history, or default password environment variables.

That statement covers application-owned credential discovery. Database, TLS,
DNS, and integrated-auth libraries can consult operating-system trust stores,
configuration, and credential caches. Review or trace those platform
dependencies separately when the host policy requires it.

See [`../AUDIT.md`](../AUDIT.md) for the full list.

## Files Written

The tool writes only to paths selected by the active mode:

- `--out` Blueprint TOML in live mode;
- `--deck` if requested;
- `--audit-log` if requested;
- `--out-dir` in batch mode: `bundle.toml`, `blueprints/`, `audits/`, an
  ownership marker, and `errors.txt` when a partial failure must be reported;
- stderr audit log on every run.

It does not use an implicit operating-system temporary directory. Atomic batch
publication may create a sibling staging or recovery directory beside
`--out-dir`; a handled failure removes it or restores the previous bundle.

## Output Review Checklist

Before sharing `blueprint.toml`, verify:

- header is the fixed `dbwarp-blueprint v7` header;
- table ids look like `table-001`;
- column ids look like `col-1`;
- schema ids look like `schema-A`;
- no real table, column, index, schema, or user names are present;
- no non-table object names, definition text, endpoint strings, credentials,
  key/certificate material, package names, or binaries are present;
- no row values are present;
- numeric values use the exact or rounded precision documented in
  [`../FORMAT.md`](../FORMAT.md); review exact opt-in fields as more sensitive;
- optional sample-derived sections contain aggregate compression,
  null-density, cardinality/frequency, length, style, and sample-provenance
  metadata, never sampled values;
- artifact completeness fields disclose filtered visibility, unreadable
  catalogs, and known unmodeled families.

Default balanced MySQL output contains exact declared capacities and index
prefix lengths plus relatively rounded average/p95 samples. Review the three
fidelity markers explicitly. If `--length-fidelity exact --yes` was used,
approve exact sampled statistics as well. Row values and real object names must
still be absent. A Blueprint without fidelity markers was produced by an older
version; recapture it.

The marker does not claim that sampling covered every table. If `DBP1406W` is
reported, increase `--max-wall-secs` and recapture.

## Operational Safety

Recommended first run:

```bash
--sample-rows 500 --max-wall-secs 120
```

Recommended production-style run once approved:

```bash
--sample-rows 1000 --max-wall-secs 300
```

Run from a read replica if production policy forbids sampling on the primary.
