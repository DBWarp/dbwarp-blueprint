# DBWarp Blueprint file format v6

Human-readable. Diff-able. Forensically reviewable.

> **This format reduces hidden-channel and direct-disclosure risk through a
> bounded schema, secret-keyed identifiers, and documented numeric precision.
> Anonymous graph structure and exact opt-in fields can still fingerprint a
> workload, so review the file under your own data-classification policy.**

## File header

Verbatim, byte-for-byte:

```
# dbwarp-blueprint v6
# Anonymous database Blueprint. Source object names and row values are excluded.
# Review under your organization's data-classification policy before sharing.
# https://github.com/DBWarp/dbwarp-blueprint

```

The blank line is part of the canonical header. The Rust collector emits
exactly this header and no other comments. The SQL fallback normalizer retains
it verbatim, then adds one fixed `Producer: blueprint_format.py SQL fallback`
comment with the key source so recipients can distinguish the producer. This
is not a claim that the remaining structured fields cannot identify a
distinctive schema or dependency graph.

## Top-level fields

| Field | Type | Description |
|---|---|---|
| `schema_version` | int | Format version. Currently `6`. Versions 1 through 5 remain readable. |
| `generated_at` | ISO-8601 string | UTC timestamp, seconds resolution, no fractional. **Pinnable** via the `--generated-at "2026-04-26T00:00:00Z"` CLI flag. Byte-identical live captures also require the same protected `--anonymization-key-file`, source state, options, and producer. The audit log records `generated_at_pin: ...` whenever the flag is set so the pin is forensic-visible. No environment variable pins this value. |
| `engine` | string | `"postgresql"`, `"mysql"`, `"sqlserver"`, `"parquet"`, or `"avro"`. |
| `engine_version` | string | Narrow numeric product version used for version-aware live-database capture and generation; empty for structured-file sources. Producer/distribution banners are excluded. |
| `source_kind` | string | One of `"production"`, `"staging"`, `"scrubbed-replica"`, `"synthetic"`. Customer-declared. |
| `length_metadata` | string | Legacy compatibility marker: `"hybrid-v2"`, `"exact"`, `"rounded"`, or `"not-captured"`. New consumers must use the three fields below. |
| `declared_length_fidelity` | string | `"exact"` for PostgreSQL declared character capacities and for the default balanced/exact MySQL modes; `"coarse-rounded-v1"` for strict MySQL privacy; `"not-captured"` where unavailable. |
| `index_length_fidelity` | string | `"exact"` for default balanced/exact MySQL index prefixes; `"rounded-down-v1"` for strict privacy; `"not-captured"` where unavailable. |
| `observed_length_fidelity` | string | `"relative-rounded-v2"` by default when sampled, `"exact"` in exact mode, `"coarse-rounded-v1"` in strict mode, or `"not-sampled"`. Sampling coverage remains a separate per-column requirement. |
| `[totals]` | inline table | Aggregated counts (see below). |
| `[network]` | table | Optional client-to-database connection and query RTT evidence. |
| `[database_topology]` | table | Required for schema-v6 database sources. Bounded, name-free deployment, local-role, visibility, and catalog evidence. Absent for structured files. |
| `[dataset_scope]` | table | Required for every schema-v6 Blueprint. Declares what the totals cover and whether table, row, and byte coverage are complete. |
| `[tables.X]` | tables | One per table, anonymized id. |
| `[fk_edges]` | inline table | FK graph between anonymized tables. Optional. |
| `[artifact_inventory]` | table | Bounded, name-free non-table object counts, optional anonymous dependency graph, external prerequisites, and optional bounded language census. Database sources only. |

## `[totals]`

| Field | Type | Precision |
|---|---|---|
| `table_count` | int | exact |
| `row_count` | int | sum of per-table rounded `rows` |
| `table_bytes` | int | sum of per-table rounded `table_bytes` |
| `index_bytes` | int | sum of per-table rounded `index_bytes` |

These numbers are not automatically whole-cluster totals. Always interpret
them together with `[dataset_scope]`. A sharded gateway or coordinator can
expose a complete-looking catalog while holding none of the underlying shards;
schema v6 represents that uncertainty explicitly instead of silently treating
local catalog statistics as global truth.

## `[database_topology]` (schema v6 database sources)

This block records only bounded facts visible through the connected database
endpoint. It never stores node names, hostnames, IP addresses, cluster names,
replication channel names, server identifiers, or endpoints.

| Field | Values / rule |
|---|---|
| `contract` | Always `dbwarp-blueprint-topology/v1`. |
| `deployment` | `single-node`, `replicated`, `sharded`, `distributed`, or `unknown`. |
| `local_role` | `standalone`, `primary`, `secondary`, `coordinator`, `worker`, `member`, or `unknown`. |
| `visibility` | `full`, `partial`, or `unknown`; describes topology evidence, not data correctness. |
| `member_count` | Number of members visible through successful evidence queries. `0` means unknown, never zero members. |
| `identifiers_redacted` | Must be `true`. |
| `role_counts` | Optional counts by closed role token. Full visibility requires these counts to equal `member_count`. |
| `features` | Sorted closed tokens such as `citus`, `mysql-group-replication`, `mysql-galera`, `mysql-ndb`, `postgresql-streaming-replication`, `sqlserver-availability-group`, or `vitess`. |
| `catalogs_read` | Sorted closed labels for topology catalogs successfully read. |
| `catalogs_unreadable` | Sorted closed labels for topology catalogs that could not be read. Any entry prevents a full-visibility claim. |

An ordinary endpoint may legitimately report `deployment = "unknown"` while
still reporting complete local full-copy table statistics. Blueprint does not
infer that an unremarkable server is single-node merely because no cluster
feature was visible.

## `[dataset_scope]` (schema v6)

This block qualifies every sizing total independently. Consumers must refuse
unqualified whole-dataset arithmetic when any required completeness dimension
is `incomplete` or `unknown`.

| Field | Values / rule |
|---|---|
| `contract` | Always `dbwarp-blueprint-dataset-scope/v1`. |
| `layout` | `full-copy`, `sharded`, `distributed`, `structured-dataset`, or `unknown`. |
| `table_inventory_completeness` | `complete`, `incomplete`, or `unknown`. |
| `row_count_completeness` | `complete`, `incomplete`, or `unknown`. |
| `size_completeness` | `complete`, `incomplete`, or `unknown`. |
| `row_count_method` | Closed provenance token such as `postgres-planner-estimate`, `mysql-table-statistics`, `sqlserver-partition-counter`, or `distributed-aggregate`. |
| `size_method` | Closed provenance token such as `postgres-local-relation-size`, `mysql-information-schema`, `sqlserver-partition-pages`, `citus-distributed-relation-size`, or `distributed-aggregate`. |
| `limitations` | Sorted closed reasons for incomplete or unknown coverage. At least one is required unless every dimension is complete. |

`selection-limited` means the totals and completeness statements cover exactly
the schemas requested through repeatable live `--schema`; they do not claim to
cover the whole connected database. Omitting `--schema` preserves the
all-visible-schema capture behavior.

The native PostgreSQL, MySQL, and SQL Server collectors probe supported
topology catalogs before deciding whether local statistics can represent the
logical dataset. Known distributed gateways suppress unsafe totals when a
reliable aggregate is unavailable. The SQL fallback formatter has no topology
probe, so it emits its useful local estimates with all scope dimensions marked
`unknown` and the limitations `topology-unobserved` and
`topology-visibility-unknown`.

Structured Parquet and Avro Blueprints omit `[database_topology]` and use
`layout = "structured-dataset"` with footer/container provenance.

Blueprint does not run a storage-speed benchmark during ordinary capture and
does not infer database-server hardware from the machine running the client.
Database byte totals describe stored data volume through the named catalog
method; they do not claim disk type, IOPS, throughput, CPU, RAM, or target
migration performance.

## `[network]` (optional)

Customer-side observed network round-trip statistics from the
Blueprint tool to the source database. **NOT** the migration source-target
RTT — this is just evidence of how far the Blueprint tool was from the
customer's source DB at run time. The downstream estimator uses it
only as a sanity-check on operator-supplied migration RTT (e.g. an
operator claiming 200 ms migration RTT is implausible if the
customer's local probe was 0.4 ms — the Blueprint tool was probably
running on the source DB itself).

The probe runs after connection establishment and before catalog
queries, so timings aren't skewed by query-cache warmup. It executes
**5× `SELECT 1`** and emits the median latency. Each `SELECT 1`
returns the constant integer 1 — no row data is ever read by this
probe.

Absent when the customer passed `--no-rtt-probe` or when the probe
itself failed mid-flight (recorded as a non-fatal warning to stderr
and audit log; the Blueprint file is still emitted without the block).

| Field | Type | Precision |
|---|---|---|
| `sample_count` | int | exact (always 5 in v1) |
| `connect_total_ms` | int | total wall-clock from start of TCP connect to authenticated session ready, in milliseconds. Includes TCP handshake + TLS handshake (when applicable) + auth challenge/response. Rounded to nearest ms. Typically 3–6× `query_rtt_ms_p50`. |
| `query_rtt_ms_p50` | int | median single-round-trip latency from the 5 `SELECT 1` samples, in milliseconds. Rounded to nearest ms. The natural network noise floor (≥ 1 ms in practice) is wider than the rounding granularity, so this kills any low-bit hidden channel without losing useful precision. Sub-ms LAN values collapse to 0 or 1. |
| `query_rtt_ms_p95` | int | nearest-rank 95th percentile of the 5 samples (the slowest observation), in milliseconds. Rounded to nearest ms. Use it with p50 to identify short latency spikes; five samples are an orientation signal, not a workload benchmark. |

The 5 probe queries appear in the audit log as a **single summary
entry** (not 5 separate rows) labelled `5x SELECT 1 (RTT probe;
constant integer 1, no row data)` — matching the trust posture that
no row content is read.

## `[tables.<id>]`

Identifier is `table-NNN` where `NNN` is the 1-indexed ordinal in a
domain-separated HMAC-SHA256 ordering of the schema and table name. The default
key is freshly generated for the process and is never emitted. Passing the same
customer-held `--anonymization-key-file` preserves the ordering across approved
comparison runs.

| Field | Type | Precision / values |
|---|---|---|
| `rows` | int | rounded: nearest 100 (≤10k), 1000 (≤1M), 10000 (>1M) |
| `table_bytes` | int | rounded: nearest 1KiB / 1MiB / 100MiB by magnitude |
| `index_bytes` | int | rounded: same as `table_bytes` |
| `schema` | string | anonymized id `schema-A`, `schema-B`, ..., `schema-AA` |
| `kind` | string | Schema v6 optional closed token: `partitioned`, `materialized-view`, `temporal-current`, `temporal-history`, `memory-optimized`, `external`, `graph-node`, or `graph-edge`. Omitted for an ordinary table or unknown evidence. |
| `unlogged` | bool | Schema v6 optional PostgreSQL logged-state observation. Omitted when not captured; explicit `false` means the catalog proved the table is logged. |
| `partition_strategy` | string | Schema v6 optional token for `partitioned` tables: `range`, `list`, `hash`, `key`, or `linear-hash`. |
| `partition_count` | int | Schema v6 exact positive leaf-partition count, required when `kind = "partitioned"`. |
| `partition_key_cols` | array of int | Schema v6 simple partition-key column ordinals. Omitted for an expression key or when catalog evidence is unavailable; no key expression is serialized. |
| `partition_rows_max` | int | Schema v6 optional rounded largest-leaf row estimate. |
| `temporal_history` | string | Schema v6 table id of the paired `temporal-history` table, required on `temporal-current`. |
| `counted_in_totals` | bool | Schema v6. Omitted means included in all aggregate totals. `external` requires explicit `false`, excluding that table from `table_count`, `row_count`, `table_bytes`, and `index_bytes`; no other explicit value is canonical. |
| `check_count` | int | Schema v6 optional exact structural CHECK-constraint count. Omitted means unknown; `0` means the relevant catalog proved none. |
| `has_clustered_index` | bool | always `false` for PostgreSQL |
| `stats_freshness` | string | `"fresh"` / `"stale"` / `"never_analyzed"` (PG) — empty if SQL fallback |
| `[tables.<id>.cols.<cid>]` | sub-tables | one per column |
| `[tables.<id>.idxs.<iid>]` | sub-tables | one per index |
| `[tables.<id>.compression]` | sub-table | only if Tier 2 |

## `[tables.<id>.cols.<cid>]`

Identifier is `col-N` where `N` is the column's natural attribute order
(1-indexed, preserving the on-disk ordinal). Stable across runs.

| Field | Type | Notes |
|---|---|---|
| `ordinal` | int | the same N as the id |
| `type` | string | normalized type family such as `"integer"`, `"numeric(12,2)"`, `"text"`, `"json"`, `"binary"`, `"timestamp"`, `"uuid"`, `"array<integer>"`, or `"user-defined"`. Real domain, enum, alias, composite, and user-defined type names are not emitted. |
| `nullable` | bool | |
| `value_source` | string | Schema v6 optional closed token: `identity-always`, `identity-default`, `auto-increment`, `identity`, `sequence-default`, `generated-stored`, `generated-virtual`, `computed-persisted`, `computed-virtual`, `system-time`, or `rowversion`. Omitted for an ordinary supplied value or unknown evidence. |
| `has_default` | bool | Schema v6 optional catalog observation. Omitted means unknown; explicit `false` means the catalog proved no default. |
| `default_kind` | string | Schema v6 optional classification `constant`, `function`, or `expression`; valid only with `has_default = true`. Default text and literals are never serialized. |
| `type_kind` | string | Schema v6 optional closed token: `enum`, `set`, `domain`, `composite`, `array`, `range`, or `alias`. Omitted for a base type or unknown evidence. |
| `member_count` | int | Schema v6 exact positive structural member count, required only for `enum` and `set`; member names are never serialized. |
| `domain_has_check` | bool | Schema v6 optional domain CHECK observation, valid only with `type_kind = "domain"`. |
| `hidden`, `masked`, `encrypted`, `sparse` | bool | Schema v6 optional catalog observations. Omitted means unknown; explicit `false` means the catalog proved the property absent. |
| `has_check` | bool | Schema v6 optional single-column CHECK observation. Every explicit `true` is covered by the table's `check_count`. |
| `null_fraction` | float | Optional observed null fraction from `0.0` through `1.0`. Rounded aggregate only; no null bitmap is retained. |
| `native_type` | string | Optional sanitized engine base type, such as `varchar` or `longtext`; no identifiers, enum members, defaults, or expressions. Currently emitted by corrected MySQL capture. |
| `declared_max_chars` | int | Optional declared character capacity. Exact for PostgreSQL `character`/`character varying` catalog values and in default balanced/exact MySQL modes; coarsely rounded only with MySQL `--length-fidelity strict`. |
| `declared_max_bytes` | int | Optional declared byte capacity. Exact in default balanced/exact MySQL modes; coarsely rounded only with `--length-fidelity strict`. |
| `numeric_precision`, `numeric_scale`, `datetime_precision` | int | Optional engine-declared scalar precision. |
| `charset`, `collation` | string | Optional sanitized MySQL character metadata. These are catalog names, never customer identifiers or values. |
| `len_avg` | int | Sampled average bytes for variable-length values. Default relative buckets have about 3.2% maximum error and preserve values through 32 bytes exactly; exact with `--length-fidelity exact --yes`; coarse nearest-10 only in strict mode. 0 = fixed-length or unmeasured. |
| `len_p95` | int | Sampled 95th percentile with the same default relative buckets; exact with `--length-fidelity exact --yes`; coarse nearest-100 only in strict mode. 0 = unmeasured. |
| `style` | string | Tier 2 only. One of `"json"`, `"xml"`, `"natural-text"`, `"base64"`, `"hex"`, `"numeric-text"`, `"mixed"`; empty if not classified. |
| `magnitude_min`, `magnitude_max` | int | Schema v6 optional signed decimal exponents bounding sampled non-null numeric magnitudes. They are emitted together with `has_negative`; exact values are never serialized. |
| `has_negative` | bool | Schema v6 optional sampled sign observation, emitted only with both magnitude bounds. |
| `time_span` | string | Schema v6 optional sampled date/time range: `intraday`, `days`, `weeks`, `months`, `years`, or `decades`. |
| `time_recent_decade` | int | Schema v6 decade containing the newest sampled date/time, emitted only with `time_span` and always divisible by 10. |
| `[tables.<id>.cols.<cid>.compression]` | sub-table | Tier 2 only. Present for sampled text/binary candidate columns. Same field layout as table-level compression, but scoped to one anonymized column. |
| `[tables.<id>.cols.<cid>.cardinality]` | sub-table | Schema v3 sampled-value distribution summary. Contains bounded/rounded counts and frequencies only. |

### `[tables.<id>.cols.<cid>.cardinality]` (schema v3)

When row sampling is enabled, the collector keeps at most 8,192 temporary
64-bit fingerprints per column in memory, derives aggregate NDV/skew
statistics, and discards the fingerprints. Neither values nor fingerprints are
serialized. The block contains `measured`, `sample_rows`, `non_null_rows`,
`observed_distinct_count`, `estimated_distinct_count`, `top_value_fraction`,
`frequency_p50`, `frequency_p95`, `frequency_p99`, `frequency_max`,
`sample_method`, `sampled_with_bias`, and `bias_reason`.

Counts and fractions are privacy-rounded where appropriate. The statistics are
intended to reproduce duplicate density, hot-value skew, and finite domains in
synthetic fixtures. They contain no sampled values, but distinctive
distributions can fingerprint a workload; do not treat them as irreversible
or as proof that business meaning cannot be inferred from outside knowledge.

### `[tables.<id>.cols.<cid>.compression]` (Tier 2 only)

Per-column compression is emitted only for bounded text/binary candidates when
`--measure-compression --yes` is used. It lets downstream tools generate
synthetic text/binary data with more realistic entropy than table-level ratios
alone.

The block has the same fields as `[tables.<id>.compression]`: `measured`,
`sample_rows`, `sample_bytes`, `sample_method`, `sampled_with_bias`,
`bias_reason`, `ratio_zstd_3`, `ratio_zstd_19`, `ratio_stddev`, and
`sample_encoding`.

Example:

```toml
[tables.table-001.cols.col-2]
ordinal = 2
type = "json"
nullable = false
len_avg = 430
len_p95 = 0
style = "json"

[tables.table-001.cols.col-2.compression]
measured = true
sample_rows = 1000
sample_bytes = 65536
sample_method = "column TABLESAMPLE SYSTEM(0.1) LIMIT N (text format)"
sampled_with_bias = false
ratio_zstd_3 = 8.4
ratio_stddev = 0.25
sample_encoding = "dbwarp-blueprint-rowframe-v1"
```

No sampled column values are written to the Blueprint file.

## `[tables.<id>.idxs.<iid>]`

Identifier is `idx-N` where `N` is the 1-indexed ordinal of the index
within the table, sorted by a domain-separated HMAC-SHA256 of the index name.

| Field | Type | Values |
|---|---|---|
| `type` | string | Normalized index method family such as `"btree"`, `"hash"`, `"gin"`, `"gist"`, `"brin"`, `"spgist"`, `"fulltext"`, `"spatial"`, `"clustered"`, `"nonclustered"`, `"clustered columnstore"`, `"nonclustered columnstore"`, or `"other"`. Extension/custom method names are not emitted. |
| `primary` | bool | Optional; emitted as `true` for primary-key indexes. Omitted/false otherwise. |
| `unique` | bool | |
| `cols` | array of int | column ordinals participating, in index column order |
| `prefix_lengths` | array of int | Optional MySQL index prefix lengths aligned with `cols`; zero means full column. Exact by default; rounded downward only with `--length-fidelity strict`. |
| `include_cols` | array of int | Optional; non-key INCLUDE column ordinals where the source engine exposes them. |
| `expression` | bool | Optional; true when expression/function key material exists and cannot be represented as simple column ordinals. |
| `filtered` | bool | Optional; true for filtered/partial indexes. |
| `descending` | bool | Optional; true when any key column is explicitly descending. |
| `prefix_distinct_counts` | array of int | Schema v3 estimated distinct tuple count for each key prefix from one through N columns. Zero means unavailable for that prefix. |
| `cardinality_sample_method` | string | Bounded provenance for `prefix_distinct_counts`; inferred products are explicitly labelled and are not presented as direct tuple samples. |

## `[tables.<id>.compression]` and `[tables.<id>.cols.<cid>.compression]` (Tier 2 only)

Present only when the file was generated with `--measure-compression --yes`.
The table-level block measures the complete sampled row stream and
remains the authoritative ratio for whole-table transfer estimates.
Column-level blocks are projected from the same sampled rows, one
column at a time, and exist to help downstream synthetic fixture
generators tune per-column entropy without seeing customer values.
They do not trigger extra database reads.

| Field | Type | Precision |
|---|---|---|
| `measured` | bool | always `true` if block is present |
| `sample_rows` | int | exact |
| `sample_bytes` | int | size of the in-memory sample buffer, **bucketed**: nearest **64 KiB** below 1 MiB, nearest **1 MiB** below 1 GiB, nearest **100 MiB** above. Bytes never written to disk. The bucketing kills the per-table low-bit hidden channel an exact `buf.len()` would otherwise expose. |
| `sample_method` | string | engine-specific bounded sampling description, for example `"TABLESAMPLE SYSTEM(0.1) LIMIT N"`, `"LIMIT N (fallback after empty TABLESAMPLE)"`, or `"SELECT TOP N"` |
| `sampled_with_bias` | bool | true if the sample is non-uniform, for example a LIMIT-only fallback |
| `bias_reason` | string | empty if `sampled_with_bias = false`, else a tag such as `"unordered_limit_after_empty_TABLESAMPLE"` |
| `ratio_zstd_3` | float | rounded to nearest **0.05**, zstd level 3 (production default). Measured on bytes encoded via `sample_encoding`. |
| `ratio_zstd_19` | float | legacy zstd level 19 ceiling accepted from older captures; the tool no longer measures or emits it |
| `ratio_stddev` | float | rounded to nearest **0.05**, stddev of level-3 ratios across row-aligned 64 KiB chunks of the sample. Column-level projection blocks currently emit `0.0` because they are advisory entropy hints, not a variance model. |
| `sample_encoding` | string | identifier for the byte-level encoding the sample was zstd-compressed in. Current value: `"dbwarp-blueprint-rowframe-v1"`. The dbwarp estimator MUST validate this string before consuming the ratio — different encodings produce different ratios for the same logical data and are NOT interchangeable. Older Blueprint files may not include this field; estimators should only consume measured ratios when the encoding tag is present and recognized. |

The dbwarp estimator should prefer recognized per-column compression blocks when
building synthetic fixtures, then fall back to table-level compression, then to
type/style defaults.

### `dbwarp-blueprint-rowframe-v1` byte-level encoding

The Tier 2 sampler concatenates rows or sampled column values into an in-memory
buffer using this format, then runs zstd level 3 on it. The buffer is
discarded. The Blueprint retains only the documented aggregate compression,
null-density, cardinality/frequency, length, and style fields.

```text
Buffer = (Column)*       # flat stream; rows are NOT delimited

Column:
  u8 type_tag                     # see table below
  if type_tag != 0x00 (NULL):
    varint length (LEB128)        # payload byte count, 1-5 bytes
    length bytes payload
```

Type tags are part of the encoding contract and will not be renumbered without
a `-v2` suffix bump.

| Tag | Name | Used for |
|---|---|---|
| 0x00 | Null | SQL NULL (no length, no payload) |
| 0x01 | TextUtf8 | UTF-8 text |
| 0x02 | TextUtf16Le | UTF-16LE bytes, primarily SQL Server `nvarchar`/`nchar`/`ntext` |
| 0x03 | TextOther | Bytes in another charset |
| 0x04 | NumberText | Decimal-textual representation of numeric values |
| 0x05 | BoolText | Boolean as text |
| 0x06 | TimestampText | ISO-8601 timestamp text |
| 0x07 | DateText | ISO-8601 date text |
| 0x08 | TimeText | `HH:MM:SS[.fff]` text |
| 0x09 | UuidText | Canonical 36-character UUID text |
| 0x0F | JsonText | JSON UTF-8 |
| 0x10 | BinaryRaw | `bytea`, `varbinary`, `image`, or blob bytes |
| 0xFE | UnknownText | Fallback DB-provided textual representation |

### Accuracy bounds

`ratio_zstd_3` describes the named `sample_encoding`; it is not a capture of
database-protocol or migration-wire bytes. The public automated suite validates
deterministic encoding, bounded sampling, and serialization, but does not claim
a universal cross-engine percentage error against every extraction path.

Before using the ratio for a high-stakes capacity decision, qualify the current
binary and engine version against representative source data and the intended
extraction mechanism. Record the comparison method, sample size, binary hash,
engine version, and observed error with the resulting plan. The primitive
relationship is `compressed_bytes ≈ sample_bytes / ratio_zstd_3` under the byte
distribution produced by the recorded encoding.

## `[fk_edges]`

Optional. Inline table where each key is a `table-NNN` id mapping to a
list of edges. Schema v3 preserves parent ordinals, referential actions, match
mode, deferrability, validation/trust state, and an optional bounded, name-free
relationship summary. Edges are sorted by destination then by column list.

```toml
[fk_edges]
table-005 = [{ to = "table-001", cols = [2], to_cols = [1], on_delete = "CASCADE", validated = true }]
```

The optional `statistics` block records sampled/inferred `non_null_rows`,
`distinct_parent_values`, `parent_coverage_fraction`, fanout p50/p95/p99/max,
and `orphan_rows`, plus provenance and bias fields. Validated source constraints
imply zero orphans. Composite estimates derived from per-column samples are
explicitly marked inferred. Generators use these aggregates to reproduce null
coverage and fanout while mapping every composite child key to one consistent
synthetic parent tuple.

## `[artifact_inventory]` (since schema v4, database sources)

The independently versioned `dbwarp-blueprint-artifacts/v1` contract describes
non-table objects without serializing source names or definitions. It is absent
for structured-file sources and when `--artifact-detail none` is selected.

The default `--artifact-detail summary` emits `object_count`,
`external_prerequisite_count`, `counts_by_kind`, and
`counts_by_external_class`. `graph` additionally emits one anonymous object
record per artifact plus dependency edges. `analyzed` adds bounded
`dbwarp-language-feature-census/v1` records derived transiently from available
definitions. `graph` and `analyzed` require explicit `--yes` because graph
topology can fingerprint an application.

Top-level inventory evidence includes:

| Field | Values / rule |
|---|---|
| `detail` | `none`, `summary`, `graph`, or `analyzed` |
| `visibility` | `full`, `privilege_filtered`, or `unknown` |
| `inventory_complete` | May be true only with full visibility, no unreadable catalogs, and no declared unmodeled families |
| `dependencies_complete` | May be true only when the modeled dependency catalogs were readable |
| `analysis_complete` | May be true only for analyzed detail and only when every emitted analysis is complete |
| `catalogs_read` | Closed, standard engine catalog labels successfully inspected |
| `catalogs_unreadable` | Catalog labels that failed; any entry prevents a complete claim |
| `families_not_inventoried` | Known object families outside the current collector contract |

Per-object ids have the form `<kind>-NNN`, such as `view-001` or
`function-002`. The record contains only closed kind/subkind/tier tokens,
anonymous schema/parent ids, anonymous dependencies, unresolved-dependency
count, bounded definition visibility/security mode, an optional external
prerequisite, and optional language census. Source object names, SQL text,
principals, endpoints, credentials, keys, certificates, and binaries are not
fields in the contract.

External prerequisites record a closed `class`, deployment scope, whether
binary/secret/endpoint material is required but not captured, and a bounded
compatibility category. Their count is evidence for migration planning, not a
claim that DBWarp can automatically provision or translate them.

Language census records use `analyzer_version = "lexical-v1"` and
`status = "partial"`. Count, size, nesting, complexity, and opaque-region values
are bands, not exact source fingerprints. Features are selected from a closed
vocabulary. The analyzer removes comments, literals, and quoted identifiers;
it is not a parser, semantic binder, or translation-success guarantee.

See [Non-Table Artifact Inventory](docs/ARTIFACT_INVENTORY.md) for operational
guidance and engine coverage.

## Steganography defenses, by vector

| Vector | Defeat |
|---|---|
| Identifier ordering | Domain-separated HMAC-SHA256 with a secret process-local key prevents offline candidate-name checks. Reuse a customer-held key only when stable cross-run labels are required. |
| Numeric low-bits | Statistics are rounded to documented precision by default. Exact-length mode is explicit, consent-gated, recorded in the audit log, and must be handled as more sensitive metadata. |
| Sub-second timestamp | One UTC timestamp at the top, seconds resolution only |
| TOML formatting | Canonical: alphabetical keys, fixed indentation, and only the fixed header/producer comments; no input-derived comments |
| Sampling randomness | Sampling uses fixed seeds (PG's deterministic `TABLESAMPLE SYSTEM`). Separately, identifier anonymization intentionally obtains a secret key from the operating-system CSPRNG unless the customer supplies one. |
| Unused fields | Every field is documented above; no "metadata"/"comment"/"reserved" fields that carry unbounded data |
| Artifact source text and external material | Definitions are transient and zeroized after bounded analysis; names, SQL text, endpoints, provider strings, credentials, keys, certificates, package names, and binaries have no serialized field |

## Schema-version compatibility

Current producers emit schema version 6. Versions 1 through 5 remain accepted
for backward compatibility. A v1/v2 file has no distribution blocks, so
generators use deterministic type/width and uniform-relationship fallbacks and
report that loss of fidelity. A v3 file has distribution metadata but no
artifact inventory. A v4 file may contain an artifact inventory but predates
the current Blueprint contract identifiers. Readers normalize former v4
identifiers on input and re-emit that document with canonical Blueprint
identifiers. A v5 file predates the topology and dataset-scope qualification
added in v6. Consumers must reject unknown future schema versions with a clear
upgrade message rather than silently discarding fields.

## Why TOML and not JSON

- TOML separates structural sections from leaf data more readably
  (`[tables.table-001.cols.col-2]` vs. nested JSON).
- Easier to diff (one key per line; identifier-based sub-tables stay
  contiguous).
- Customer can hand-edit if they want to redact a specific field before
  sharing.

JSON is used as the **intermediate format** in the SQL fallback path. Each
`sql/blueprint.*.sql` script produces JSON and `blueprint_format.py` normalizes
it to TOML. The intermediate JSON contains real source identifiers; MySQL can
also include enum/set declarations through `COLUMN_TYPE`, so it must remain
protected inside the source environment. The normalizer uses a fresh secret
key by default and accepts the same protected `--anonymization-key-file`
contract for approved cross-run comparisons. The end-state file reviewed for
sharing with DBWarp is always TOML.

## Structured-file provenance extensions

When `engine` or `source_kind` is `"parquet"` or `"avro"`, schema version 3 or newer may
also emit the following bounded fields. Older readers must ignore fields they do
not understand; newer readers must preserve the distinction between source-file
storage and DBWarp transport measurements.

Structured-file Blueprints use the same anonymized identifiers as database
Blueprints: `table-NNN` in secret-keyed order and `col-N` in schema ordinal order.
Source file stems, Parquet paths, Avro field names, and a manifest's
`logical_table` label are not emitted as table or column identifiers.

At table scope, `table_bytes` is the logical transfer-sizing estimate, whereas
`storage_bytes` is the actual source-object size on disk. Metadata-only Parquet
uses uncompressed column-chunk bytes for `table_bytes`; optional decoded sampling
replaces that estimate with projected `dbwarp-blueprint-rowframe-v1` bytes. Avro
derives it from its decoded full scan. The optional `source_partitions`,
`row_group_count`, and `source_codec` fields describe file layout and scheduling
provenance. Multi-file datasets aggregate these values. `row_group_count` is
Parquet-specific; `source_partitions` is `1` for a single input object.

At column scope, `null_fraction` is an observed value from `0.0` through `1.0`.
`length_sample_rows` and `length_sample_method` state how `len_avg` and
`len_p95` were obtained. `source_semantics` records bounded compatibility facts
such as `"repeated-leaf"`, `"nested-json"`, or `"multi-type-union"`; it never
contains a customer field name or value. Decimal precision/scale, timestamp
precision and UTC/local semantics, UUID, and fixed-size binary metadata are
carried by the existing sanitized scalar fields and `native_type`.

At compression scope, table-level `ratio_storage` compares `table_bytes` with
actual source-object bytes. A Parquet column-level value compares the footer's
uncompressed and compressed column-chunk bytes. Both are file-storage planning
signals, not DBWarp transfer estimates. `ratio_zstd_3` and
`ratio_zstd_19` are valid transfer-calibration inputs only when
`sample_encoding` is the recognized `"dbwarp-blueprint-rowframe-v1"` value. A
Parquet footer ratio or Avro container ratio must never be copied into those
zstd fields.
