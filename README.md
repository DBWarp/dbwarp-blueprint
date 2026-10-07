<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset=".github/assets/dbwarp-logo-dark.png">
    <img src=".github/assets/dbwarp-logo-light.png" alt="DBWarp" width="420">
  </picture>
</p>

<h3 align="center">DBWarp Blueprint</h3>

<p align="center">Global Data &middot; Local Speeds</p>

---

## What it is

DBWarp Blueprint is a trust-first database blueprint collector. You run it inside your own
environment against PostgreSQL, MySQL or SQL Server. It reads catalogue metadata, and optionally
a bounded row sample when you ask it to measure compression, then writes an anonymised structural
blueprint of your database: table sizes, row counts, type families, index and foreign-key shape.

Identifiers are replaced with keyed anonymous labels, and no row values are written to the
Blueprint. A fresh process-local key prevents offline dictionary checks by default;
`--anonymization-key-file` lets you preserve labels across approved comparison runs.
Read [`SECURITY.md`](SECURITY.md) before sharing any output: it sets out exactly what each mode
discloses, and which options widen that.

The output is a plain-text file. You can read every line of it before deciding whether to share it.

DBWarp Blueprint is free and open source, and it runs entirely inside your environment. It exists
so you can give us facts about your database without giving us your database.

## Why you would run it

Share your Blueprint output with us and we can tell you how much faster DBWarp would move your
data, and what that changes for your migration, CI/CD test-data and analytics timelines.

The distance matters most. The further your data has to travel, the bigger the improvement DBWarp
can show you.

[dbwarp.com/blueprint](https://dbwarp.com/blueprint) &middot;
[info@dbwarp.com](mailto:info@dbwarp.com) &middot; Zürich, Switzerland

---

# dbwarp-blueprint

**Documentation language:** English is authoritative. See
[`MACHINE_TRANSLATIONS.md`](MACHINE_TRANSLATIONS.md) for translation limitations.

Run `dbwarp-blueprint` inside your own environment to produce a bounded,
anonymized, reviewable `blueprint.toml` file that DBWarp can use for migration
sizing and planning without receiving database access, dumps, schema names, or
row data.

It connects to PostgreSQL, MySQL, or SQL Server, reads catalog metadata, optionally measures local compression from a bounded row sample, and writes plain-text TOML. It can also derive a Blueprint from local Parquet or Avro files in offline mode when the input is already a structured data file rather than a live database. You can open the output, review every line, and decide whether to share it.

Optionally, `--deck blueprint.pptx` also writes a PowerPoint summary of the same anonymized Blueprint. The deck can be written during a live database run, or later from a reviewed TOML file with `--from-toml blueprint.toml --deck blueprint.pptx`. The deck writer is built into the Rust binary and makes no network connection.

## What It Is For

DBWarp needs enough structural information to estimate and plan a transfer:

- number of tables;
- approximate row counts;
- table and index sizes;
- column type families, exact structural capacities/index prefixes, and
  privacy-rounded observed widths by default;
- index and foreign-key shape;
- bounded, name-free non-table artifact counts and external deployment
  prerequisites;
- optional table and column compression summaries from a small local sample;
- optional round-trip-time measurement from the collector to your database.

Those facts are enough to estimate transfer size and plan a transfer. Source names
and row values are omitted, but distinctive structure and statistics can still
fingerprint a workload; anonymization is risk reduction, not a claim of
irreversibility.

## What It Does Not Do

`dbwarp-blueprint` does not:

- send telemetry;
- call DBWarp servers;
- upload the Blueprint file;
- read `~/.pgpass`, `~/.my.cnf`, cloud credentials, or SSH keys;
- read default password environment variables such as `PGPASSWORD` or `MYSQL_PWD`;
- use implicit system temporary, cache, or configuration directories; it writes
  the explicitly selected output files, while batch mode also uses a sibling
  staging or recovery directory beside `--out-dir` for atomic publication;
- include real table names, column names, index names, schema names, non-table
  object names, SQL definitions, external endpoints, credentials, keys,
  certificates, binaries, or row values in the output.

Live Blueprint runs open a database session to the endpoint you specify. DNS
may use the configured resolver, and integrated Kerberos/SSPI authentication
may contact identity infrastructure. Batch mode repeats that boundary for each
database source. Local TOML, Parquet, Avro, and bundle operations open no
application-initiated network connection.

## Download or Build

| Path | Best for | Link |
|---|---|---|
| Download a binary | quick trial, isolated test host | [`binaries/README.md`](binaries/README.md) |
| Build from a small source clone | security review, production policy, reproducibility check | [`BUILD.md`](BUILD.md) |
| Build from a vendored source bundle | strict offline dependency audit | GitHub Releases |
| Review and run the SQL fallback | DBA policy refuses a third-party binary | [`sql/blueprint.pg.sql`](sql/blueprint.pg.sql), [`sql/blueprint.mysql.sql`](sql/blueprint.mysql.sql), [`sql/blueprint.sqlserver.sql`](sql/blueprint.sqlserver.sql), and [`blueprint_format.py`](blueprint_format.py) |

### SQL fallback boundary

The SQL fallback is a reviewable catalog floor, not a feature-equivalent
replacement for the Rust collector. Each SQL script writes an intermediate
JSON document containing real schema, table, column, and index names; MySQL
`COLUMN_TYPE` can also contain declared enum/set members. Treat that JSON as
sensitive schema material, keep it inside the source environment, normalize it
locally with `blueprint_format.py`, and share only the reviewed TOML output.

The fallback has no `--schema` selector: PostgreSQL covers ordinary tables in
all non-system schemas of the connected database, while MySQL and SQL Server
cover ordinary local user tables in the selected database. Do not use it when
only a subset is approved.
Its TOML contains the ordinary-table subset of table/column/index/FK structure
and approximate local sizing, but does not inventory every v7 table kind. It
marks all structure families incomplete and excludes MySQL FEDERATED and SQL
Server external tables rather than mislabelling their remote data as local. It
also has no row sampling, RTT evidence, non-table artifact inventory, or live
topology probes; topology and dataset completeness are explicitly `unknown`.

The trust-first path is to build from source. The normal repository stays small and uses `Cargo.lock` to pin dependency versions. For stricter offline audits, each release also publishes a vendored source bundle containing every dependency source file. Release binaries are provided for convenience with SHA256 checksums.

## Quick Start

Before any live run, have a DBA provision a dedicated least-privilege
collector account with the matching engine/version/tier script under
[`sql/grants/`](sql/grants/), and approve the exact schema scope. Never start
with an application-owner or administrator account. Pass each approved schema
with `--schema`, then remove the dedicated account after capture with the
matching [`sql/revoke/`](sql/revoke/) script. The complete first-run procedure
is in [`docs/QUICKSTART.md`](docs/QUICKSTART.md).

Choose a presentation language when useful. English is the default; complete
catalogs are embedded for German, French, Spanish, Polish, Japanese, and
Simplified Chinese:

```bash
./dbwarp-blueprint --lang ja --help
./dbwarp-blueprint --lang de --connect postgresql://db.internal/payments --schema app --dry-run
```

Only human-facing help, prompts, diagnostics, progress, and PowerPoint deck
labels are translated. Command and option names, accepted values, URI schemes,
environment-variable names, selectors, DBP codes, audit keys, and generated
TOML remain canonical English tokens. This keeps automation and support
procedures identical in every language. See
[`docs/INTERNATIONALISATION.md`](docs/INTERNATIONALISATION.md).

Before connecting to a database, inspect the examples under
[`samples/`](samples/). They are ordinary Blueprint TOML and require no setup
to review. After obtaining a binary, an offline first run can render one as a
deck without any database or network access:

```bash
./dbwarp-blueprint --from-toml samples/sqlserver-v6-analyzed.toml --deck sample.pptx
```

Start with the small Blueprint examples described in
[`samples/README.md`](samples/README.md): PostgreSQL catalog-only, MySQL sampled,
and SQL Server sampled with analyzed artifacts. These are hand-authored examples,
not real captures. The larger schema-v1 examples show the older format, which is
still readable. Use [`FORMAT.md`](FORMAT.md) to review the complete output surface
before approving a capture; no example covers every optional field.

Dry-run first. It prints the plan without connecting:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --schema app \
  --dry-run
```

Recommended production-style run with TLS, audit log, and compression measurement:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --schema app \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --sample-rows 1000 \
  --max-wall-secs 300 \
  --out blueprint.toml \
  --audit-log audit.txt
```

With `--measure-compression --yes`, the output includes table-level
zstd ratios and per-column compression projections. The per-column
blocks are computed from the same bounded sample as the table-level
ratio; they refine the transfer estimate and do not
write sampled values to disk. A materially dominant binary sample with
recognized standard compressed-container signatures receives only the coarse
`style = "precompressed"` label; no file type, signature, or sampled value is
serialized. Schema v3 and newer also emit bounded, name-free
per-column cardinality/skew aggregates and inferred index-prefix/relationship
summaries. Temporary per-value hashes are bounded in memory and discarded;
sampled values and per-value hashes never appear in the Blueprint TOML, while
the documented aggregates do.

Since schema v4, Blueprints also inventory non-table objects. The default
`--artifact-detail summary` stores bounded counts by object and external
prerequisite class without reading definitions. Use `graph` for anonymous
dependency topology or `analyzed` for bounded language-feature/complexity bands;
both require `--yes` because even an anonymous graph can fingerprint an
application:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --schema app \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail analyzed \
  --out blueprint.toml \
  --audit-log audit.txt \
  --yes
```

Artifact presence is planning evidence, not a claim that DBWarp can recreate or
translate it automatically. See
[`docs/ARTIFACT_INVENTORY.md`](docs/ARTIFACT_INVENTORY.md).

### MySQL length fidelity

The default `balanced` policy preserves declared character/byte capacities and
index-prefix lengths exactly. Sampled average/p95 value lengths use
relative-error buckets (about 3.2% maximum error, with values up to 32 bytes
preserved exactly). This keeps a normally 9-character `VARCHAR(3000)` key near
9 characters, so sizing reflects real value widths while retaining valid
source DDL/index limits:

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --schema appdb \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --measure-compression --yes \
  --out mysql-appdb.blueprint.toml
```

Use exact sampled statistics only when policy permits the additional precision:

```bash
./dbwarp-blueprint \
  --connect mysql://mysql-primary.internal:3306/appdb \
  --schema appdb \
  --password-file /etc/dbwarp/mysql-blueprint.pass \
  --measure-compression \
  --length-fidelity exact --yes \
  --out mysql-appdb-exact.blueprint.toml \
  --audit-log mysql-appdb-exact.audit.txt
```

Use `--length-fidelity strict` to apply coarse privacy bucketing for declared,
observed, and prefix lengths. Strict mode reduces the precision of the resulting
estimate. The `--preserve-exact-lengths --yes` spelling remains an alias for
`--length-fidelity exact --yes`.

New Blueprints record separate `declared_length_fidelity`,
`index_length_fidelity`, and `observed_length_fidelity` fields. The
`length_metadata` field is also written so tools reading the earlier format
keep working. PostgreSQL character capacities are exact catalog values;
encoding-dependent byte ceilings and index-prefix lengths remain unavailable.

For the most accurate estimate, run with `--measure-compression`: it records
observed average and p95 value lengths, so a column declared far wider than its
real values is not over-estimated. The default sampling wall budget is 300
seconds; increase `--max-wall-secs` for very large schemas.

Then review the files:

```bash
less blueprint.toml
less audit.txt
```

Follow the [review-and-share steps](docs/QUICKSTART.md#review-and-share) before sending
any artifact. Share only approved Blueprint content and, if separately reviewed
and approved, a deck; keep operational evidence local by default.

## Structured File Mode

If the source is already a local structured file, generate Blueprint TOML without database credentials:

```bash
./dbwarp-blueprint \
  --from-parquet /data/sample.parquet \
  --out blueprint.toml \
  --audit-log audit.txt
```

```bash
./dbwarp-blueprint \
  --from-avro /data/sample.avro \
  --out blueprint.toml \
  --audit-log audit.txt
```

Parquet mode reads footer and row-group metadata. Avro object containers do not have an equivalent footer row count, so Avro mode walks the container to count records and uses the writer schema for column shape. Neither mode connects to a database or reads credential flags.

If your policy permits decoded sampling, file mode can also measure bounded
local compressibility for transfer planning:

```bash
./dbwarp-blueprint \
  --from-parquet /data/sample.parquet \
  --measure-compression --yes \
  --sample-rows 5000 \
  --out blueprint.toml \
  --audit-log audit.txt
```

The same flags work with `--from-avro`. Sampled values are encoded in memory as
`blueprint-compression-probe-v2`; the Blueprint stores aggregate compression,
null-density, cardinality/frequency, length, and style measurements, never
sampled values.

## Batch And Bundle Mode

For multiple databases, multiple tables/datasets, or a review of a whole estate,
use a batch manifest and write a bundle directory:

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

```bash
./dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --yes
```

The working directory contains `bundle.toml`, per-source child Blueprint files,
and access-controlled per-source audit logs. Do not transfer the whole working
directory by default. You can list, extract, or create a separately reviewed
packed Blueprint bundle:

```bash
./dbwarp-blueprint --bundle-list customer-blueprint-bundle/bundle.toml
./dbwarp-blueprint --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg,table=table-042 --out table-042.blueprint.toml
./dbwarp-blueprint --bundle-pack customer-blueprint-bundle --out customer-blueprint-bundle.packed.toml
```

See [`docs/BATCH_AND_BUNDLES.md`](docs/BATCH_AND_BUNDLES.md) for manifest
syntax, structured-file dataset modes, and selector rules.

## Common Database Commands

PostgreSQL:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --schema app \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml
```

MySQL:

```bash
./dbwarp-blueprint \
  --connect mysql://app@db.internal/payments \
  --schema payments \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml
```

SQL Server:

```bash
./dbwarp-blueprint \
  --connect sqlserver://dbwarp_user@db.internal,1433/payments \
  --schema dbo \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --measure-compression --yes \
  --out blueprint.toml
```

For Kerberos, SSPI, and Entra ID examples, see [`AUTH.md`](AUTH.md). For internal CAs, mTLS, and hostname verification, see [`TLS.md`](TLS.md).

## Catalog-Only Mode

If policy permits only table/column/index/FK catalogs, omit
`--measure-compression` and explicitly disable the default non-table summary:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --schema app \
  --password-file /etc/dbwarp/db.pass \
  --artifact-detail none \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.toml \
  --yes
```

This catalog-only mode reads table metadata, statistics, and a count-only
topology probe, but no row values or non-table object inventories. DBWarp can still estimate from table size, row
counts, type families, and index/FK shape, but compression estimates are weaker
because text/binary entropy must be
inferred. Without `--artifact-detail none`, the default summary also reads
non-table object catalogs, but not definitions.

## Output Preview

```toml
# dbwarp-blueprint v7
# Anonymous database Blueprint. Source object names and row values are excluded.
# Review under your organization's data-classification policy before sharing.
# https://github.com/DBWarp/dbwarp-blueprint

schema_version = 7
generated_at = "2026-04-26T00:00:00Z"
engine = "postgresql"
engine_version = "16.2"
source_kind = "production"
length_metadata = "hybrid-v2"
declared_length_fidelity = "exact"
index_length_fidelity = "not-captured"
observed_length_fidelity = "not-sampled"

[totals]
table_count = 1
row_count = 12500000
table_bytes = 4194304000
index_bytes = 1048576000

[database_topology]
contract = "dbwarp-blueprint-topology/v2"
deployment = "unknown"
local_role = "unknown"
visibility = "unknown"
member_count = 0
member_count_scope = "unknown"
identifiers_redacted = true

[dataset_scope]
contract = "dbwarp-blueprint-dataset-scope/v1"
layout = "unknown"
table_inventory_completeness = "unknown"
row_count_completeness = "unknown"
size_completeness = "unknown"
row_count_method = "postgres-planner-estimate"
size_method = "postgres-local-relation-size"
limitations = ["topology-unobserved", "topology-visibility-unknown"]

[structure_scope]
contract = "dbwarp-blueprint-structure-scope/v1"
visibility = "unknown"
table_inventory_completeness = "unknown"
column_inventory_completeness = "unknown"
index_inventory_completeness = "unknown"
relationship_inventory_completeness = "unknown"
limitations = ["metadata-visibility-unknown"]

[source_environment]
contract = "dbwarp-blueprint-source-environment/v1"
evidence_origin = "database-endpoint"
hosting_model = "unknown"
infrastructure_location = "unknown"
capacity_scope = "connected-instance"
capacity_visibility = "partial"
cpu_capacity_band = "unknown"
cpu_capacity_basis = "unknown"
memory_capacity_band = "under-2-gib"
memory_capacity_basis = "database-buffer-cache"
collector_machine_excluded = true
catalogs_read = ["pg-capacity-settings"]

[statistics_evidence]
contract = "dbwarp-blueprint-statistics-evidence/v1"
visibility = "unknown"
table_count = 1
counts_by_statistics_state = { unknown = 1 }
counts_by_row_count_quality = { unknown = 1 }
counts_by_size_quality = { unknown = 1 }
limitations = ["statistics-provenance-unclassified"]

[artifact_inventory]
contract = "dbwarp-blueprint-artifacts/v2"
detail = "none"
scope = "all-visible-schemas"
visibility = "unknown"
inventory_complete = false
dependencies_complete = false
requirements_complete = false
analysis_complete = false
families_not_inventoried = ["non_table_objects"]

[tables.table-001]
rows = 12500000
table_bytes = 4194304000
index_bytes = 1048576000
schema = "schema-A"
has_clustered_index = false
object_kind = "ordinary-table"
storage_organization = "unknown"
partitioning = "none"
segment_state = "unknown"

[tables.table-001.statistics]
row_count_method = "postgres-planner-estimate"
row_count_quality = "unknown"
statistics_state = "unknown"
refresh_age_band = "unknown"
modification_ratio_band = "unknown"
sample_fraction_band = "unknown"
statistics_scope = "unknown"
size_method = "postgres-local-relation-size"
size_quality = "unknown"
size_scope = "unknown"
size_accounting = "unknown"
size_visibility = "unknown"

[tables.table-001.cols.col-1]
ordinal = 1
type = "bigint"
nullable = false
numeric_model = "integer"
numeric_precision_radix = "decimal"

[tables.table-001.idxs.idx-1]
type = "btree"
primary = true
unique = true
cols = [1]
```

The full file contract is documented in [`FORMAT.md`](FORMAT.md). The audit log is documented in [`AUDIT.md`](AUDIT.md).

## Visual Summary Deck

Generate a deck during the live run:

```bash
./dbwarp-blueprint \
  --connect postgresql://app@db.internal/payments \
  --schema app \
  --password-file /etc/dbwarp/db.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --out blueprint.toml \
  --deck blueprint.pptx \
  --yes
```

Or build it later from a reviewed Blueprint file, with no database connection:

```bash
./dbwarp-blueprint \
  --from-toml blueprint.toml \
  --deck blueprint.pptx
```

The deck adapts to schema size: per-table detail for small schemas, characterization slides for large schemas, compression summary when Tier 2 data is present, and a trust-model slide. See [`DECK.md`](DECK.md).

## Documentation

Start here:

- [`docs/QUICKSTART.md`](docs/QUICKSTART.md): first safe run and what to share.
- [`docs/COOKBOOK.md`](docs/COOKBOOK.md): practical recipes for PostgreSQL, MySQL, SQL Server, TLS, deck, and no-sampling workflows.
- [`docs/DBA_REVIEW_GUIDE.md`](docs/DBA_REVIEW_GUIDE.md): what a DBA/security reviewer needs to know before running the tool.
- [`sql/grants/README.md`](sql/grants/README.md): version-aware least-privilege grant scripts and post-capture account removal.
- [`docs/TROUBLESHOOTING.md`](docs/TROUBLESHOOTING.md): common failures and fixes.
- [`docs/MESSAGES.md`](docs/MESSAGES.md): stable `DBPnnnnS` operator message codes.
- [`docs/COMPRESSION_MEASUREMENT.md`](docs/COMPRESSION_MEASUREMENT.md): how Tier 2 compression sampling works.
- [`docs/INDEX.md`](docs/INDEX.md): complete documentation map.

Security review starting points:

- [`SECURITY.md`](SECURITY.md): security model and credential handling.
- [`AUDIT.md`](AUDIT.md): what is read, written, queried, and logged.
- [`FORMAT.md`](FORMAT.md): output fields and rounding rules.
- [`TLS.md`](TLS.md): TLS and mTLS behavior.
- [`AUTH.md`](AUTH.md): supported authentication modes.
- [`BUILD.md`](BUILD.md): building from source and release verification.
- [`DECK.md`](DECK.md): the optional PowerPoint summary deck.

## License

Apache-2.0 OR MIT.
