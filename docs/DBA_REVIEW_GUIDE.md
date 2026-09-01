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

Exact grants vary by engine and customer policy. If the account cannot read some catalog views or sample some tables, the tool should fail clearly or emit a reduced Blueprint; keep the audit log.

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
- bounded non-table object and external-prerequisite counts from object
  catalogs under the default `--artifact-detail summary` (no definitions);
- optional customer-side RTT probe unless `--no-rtt-probe` is set.

It does not read row values.

## Non-Table Artifact Inventory

Since schema v4, Blueprints inventory non-table objects independently from row
sampling. The
default `--artifact-detail summary` reads object catalogs but not definitions
and emits only bounded counts and external-prerequisite classes.

`--artifact-detail graph --yes` adds anonymous object ids and dependency edges.
`--artifact-detail analyzed --yes` also reads available definitions transiently
and emits only bounded lexical feature/complexity bands. Definition text,
source object names, endpoints, provider strings, principals, secrets, keys,
certificates, package names, and binaries are never serialized.

Catalog privileges affect absence claims. Review `visibility`,
`inventory_complete`, `dependencies_complete`, `catalogs_unreadable`, and
`families_not_inventoried`; do not interpret a zero count as proof when those
fields disclose a gap. `DBP1410W` identifies an optional artifact catalog that
could not be read.

Anonymous dependency topology can still fingerprint an application. Approve
`graph` or `analyzed` only when that risk is acceptable. See
[`ARTIFACT_INVENTORY.md`](ARTIFACT_INVENTORY.md).

## Tier 2: Compression Measurement

Tier 2 is enabled only by the explicit pair:

```bash
--measure-compression --yes
```

Tier 2 additionally reads bounded row samples into process memory. The sampled
bytes are encoded into an internal row-frame buffer and used to derive
aggregate compression, null-density, cardinality/frequency, length, and style
measurements before the values and temporary fingerprints are discarded.

The sample bytes are:

- not written to `blueprint.toml`;
- not written to the audit log;
- not written to temp files;
- not sent over any network other than the database connection;
- not retained after the sample is summarized.

Tier 2 is valuable because DBWarp performance and egress cost depend on compressed bytes, not raw table bytes.

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

- header is the fixed `dbwarp-blueprint v6` header;
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
still be absent. Missing fidelity markers are legacy/unknown and must not be
treated as benchmark-ready metadata.

The marker does not claim that sampling covered every table. A benchmark handoff
must also show zero unsampled variable-width indexed columns in the estimator
manifest; increase `--max-wall-secs` and recapture if that gate fails.

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
