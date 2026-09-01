# Batch Collection And Blueprint Bundles

`dbwarp-blueprint` supports both one-source Blueprint files and multi-source bundle directories.

Use a single `blueprint.toml` when the customer is sharing one database, one table subset, one Parquet file, or one Avro file. Use a bundle when the customer has multiple databases, multiple structured-file datasets, or wants one review package for a whole estate.

## Bundle Layout

A batch run writes a directory:

```text
customer-blueprint-bundle/
  bundle.toml
  blueprints/
    erp_pg.blueprint.toml
    billing_mysql.blueprint.toml
    orders_parquet.blueprint.toml
  audits/
    erp_pg.audit.txt
    billing_mysql.audit.txt
    orders_parquet.audit.txt
```

`bundle.toml` contains source-level metadata and relative paths to child Blueprint files. This is the preferred working form because each source stays independently reviewable, auditable, and rerunnable.

For a separately reviewed handoff, pack the directory into one embedded TOML:

```bash
dbwarp-blueprint \
  --bundle-pack customer-blueprint-bundle \
  --out customer-blueprint-bundle.packed.toml
```

The packed form embeds each child Blueprint under its source entry. It retains
the operator-supplied source ids, tags, dataset-group ids, and audit-path
metadata, so use anonymous manifest values and inspect the packed file before
transfer. The working directory is easier for review but also contains detailed
audits and any `errors.txt`; do not transfer it wholesale by default.

## Bundle Contract

Current bundles use `schema_version = 3` and
`kind = "dbwarp-blueprint-bundle"`. A directory bundle refers to each child
with `blueprint_path`; a packed bundle embeds it under `blueprint`. Writers emit
only these canonical identifiers.

Readers also accept bundle schemas v1 and v2. Those contracts are input-only
compatibility: an accepted legacy bundle is normalized to v3 and is never
re-emitted with former identifiers. Because old bundles do not state whether
sources are independent, replicas, or shards, their relationship becomes
`unknown` and cross-source aggregate totals are suppressed. Child paths must
be relative and must remain inside the bundle directory after canonicalization.

Bundle v3 separates physical capture sources from logical datasets. Every
source has `dataset_relationship`, `dataset_group`, and
`dataset_scope_completeness`. The top-level `dataset_groups` table records the
relationship, membership, and whether the declared member set is complete.

Aggregation is fail-closed:

- `independent`: exactly one source in its group; totals are added once.
- `replica`: matching copies count once, never once per replica. If declared
  replicas disagree, one deterministic representative is retained, no values
  are averaged, and the result is incomplete.
- `shard`: members are added only when `members_complete = true` and every
  declared member succeeded. An incomplete shard group contributes no totals.
- `unknown`: all cross-source table, row, and byte totals are suppressed.
- Any source whose `[dataset_scope]` is incomplete or unknown marks aggregate
  evidence incomplete even when its relationship is known.

The bundle always retains per-source totals. Suppression affects only the
cross-source aggregate, preventing a replica set from being multiplied or a
partial shard set from being presented as the whole dataset.

## Batch Manifest

Create a customer-owned manifest:

```toml
[defaults]
measure_compression = true
sample_rows = 5000
max_wall_secs = 600
continue_on_error = true
source_kind = "production"

[[source]]
id = "erp_pg"
kind = "postgresql"
connect_env = "ERP_PG_URI"
password_env = "ERP_PG_PASSWORD"
dataset_relationship = "independent"
tags = ["critical", "erp"]

[[source]]
id = "billing_mysql"
kind = "mysql"
connect_file = "/etc/dbwarp/billing.uri"
password_file = "/etc/dbwarp/billing.pass"
dataset_relationship = "independent"
tags = ["billing"]

[[source]]
id = "orders_parquet"
kind = "parquet"
paths = ["/data/orders/year=*/month=*/*.parquet"]
dataset_mode = "partitioned_dataset"
logical_table = "orders"
dataset_relationship = "independent"
tags = ["lake", "orders"]

[[source]]
id = "events_avro"
kind = "avro"
paths = ["/data/events/*.avro"]
dataset_mode = "one_table_per_file"
dataset_relationship = "independent"
tags = ["lake"]
```

If the relationship is omitted, it defaults to `unknown`; the run succeeds but
emits `DBP1414W` and `DBP1417W`, and aggregate totals are suppressed. This is
safer than assuming that two endpoints are two independent datasets.

Declare replicated members with one shared group:

```toml
[[source]]
id = "orders_primary"
kind = "postgresql"
connect_env = "ORDERS_PRIMARY_URI"
password_env = "ORDERS_PASSWORD"
dataset_relationship = "replica"
dataset_group = "orders_dataset"
dataset_group_complete = true

[[source]]
id = "orders_secondary"
kind = "postgresql"
connect_env = "ORDERS_SECONDARY_URI"
password_env = "ORDERS_PASSWORD"
dataset_relationship = "replica"
dataset_group = "orders_dataset"
dataset_group_complete = true
```

For sharded systems, list every known shard with one shared group and set
`dataset_group_complete = true` only when the manifest enumerates the complete
logical dataset. A failed member makes that group incomplete for the run.

Dry-run first:

```bash
dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --dry-run
```

Run the batch:

```bash
dbwarp-blueprint \
  --batch-manifest customer.batch.toml \
  --out-dir customer-blueprint-bundle \
  --yes
```

A non-dry-run batch requires `--yes` because it may connect to multiple databases or decode structured-file samples. Each child source gets its own audit file.

With `continue_on_error = true`, the tool completes the remaining sources and
atomically publishes the diagnostic bundle, including `errors.txt`. The command
still exits non-zero: `DBP1115E` when every source failed and `DBP1116E` when
only some sources failed. A partial bundle is evidence for review and retry,
not a successful complete collection.

Both dry-run and execution validate the complete manifest before touching a
source. Unknown fields, duplicate IDs, IDs that collide after safe filename
normalization, unsupported cross-kind fields, ambiguous database connection
sources, invalid dataset modes, and zero compression-sampling budgets fail
closed. Keep each `source.id` unique, unpadded, and at most 120 normalized ASCII
bytes.

## Structured-File Dataset Modes

For Parquet and Avro sources:

- `single_file` requires exactly one resolved file and keeps it as one logical table.
- `one_table_per_file` maps each file to a separate anonymously labelled table
  in one child Blueprint file.
- `merge_same_schema` merges many files into one logical table when column counts match.
- `partitioned_dataset` currently uses the same merge behavior as `merge_same_schema`; it reserves the semantic distinction for Hive-style partition discovery.

The merge check is intentionally conservative. It requires matching anonymized
column layout, canonical/native types, nullability, declared widths,
precision/scale, unsigned and `BIT(n)` semantics, timestamp precision,
charset/collation, and structured source semantics. For high-stakes data-lake
planning, keep datasets grouped by known schema even when this structural check
passes.

## Bundle Operations

List sources:

```bash
dbwarp-blueprint --bundle-list customer-blueprint-bundle/bundle.toml
```

The first lines report `aggregation`, physical `sources`,
`logical_datasets`, aggregate totals, and any `limitations`. Group lines show
`relationship`, `members_complete`, and member source ids. Source lines show
their `dataset_relationship`, `dataset_group`, and `dataset_scope`. Treat
`aggregation=suppressed` as an instruction to inspect or correct the manifest,
not as a zero-sized estate.

List one tagged source subset:

```bash
dbwarp-blueprint \
  --bundle-list customer-blueprint-bundle/bundle.toml \
  --select tag=erp
```

Extract one source:

```bash
dbwarp-blueprint \
  --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg \
  --out erp_pg.blueprint.toml
```

Extract one table from one source:

```bash
dbwarp-blueprint \
  --bundle-extract customer-blueprint-bundle/bundle.toml \
  --select source=erp_pg,table=table-042 \
  --out erp_pg_table_042.blueprint.toml
```

Supported selector keys are:

- `source=ID`
- `table=ID`
- `engine=postgresql|mysql|sqlserver|parquet|avro`
- `tag=NAME`

Selectors can be passed as one comma-separated string or as repeated `--select` flags. Conflicting values for the same key are rejected.

## Downstream Handoff

A bundle is a portable, reviewable Blueprint input. Before accepting one, a
downstream consumer must validate the bundle contract and schema versions,
apply the recorded selectors, and preserve source IDs when combining multiple
children so table IDs cannot collide. Commands and compatibility rules for
other DBWarp products belong to their separately reviewed documentation and
are intentionally not duplicated here.

## Privacy And Review Boundary

A bundle does not relax the privacy model:

- live DB sources still emit secret-keyed anonymous table/column/index IDs;
- structured-file values are only decoded when `--measure-compression --yes` is enabled;
- decoded samples stay in memory;
- bundle metadata uses customer-chosen source IDs and tags;
- no bundle command sends telemetry or uploads files.

The customer can remove any child Blueprint or source entry before sharing the bundle.
