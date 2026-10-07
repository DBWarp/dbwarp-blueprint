# DBWarp Blueprint file format v7

Human-readable. Diff-able. Forensically reviewable.

> **This format reduces hidden-channel and direct-disclosure risk through a
> bounded schema, secret-keyed identifiers, and documented numeric precision.
> Anonymous graph structure and exact opt-in fields can still fingerprint a
> workload, so review the file under your own data-classification policy.**

## File header

Verbatim, byte-for-byte:

```
# dbwarp-blueprint v7
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
| `schema_version` | int | Format version. Currently `7`. Versions 1 through 6 remain readable. |
| `generated_at` | ISO-8601 string | UTC timestamp, seconds resolution, no fractional. **Pinnable** via the `--generated-at "2026-04-26T00:00:00Z"` CLI flag. Byte-identical live captures also require the same protected `--anonymization-key-file`, source state, options, and collector build. The audit log records `generated_at_pin: ...` whenever the flag is set so the pin is forensic-visible. No environment variable pins this value. |
| `engine` | string | `"postgresql"`, `"mysql"`, `"sqlserver"`, `"oracle"`, `"parquet"`, or `"avro"`. `oracle` appears only in output from the Oracle preview. |
| `engine_version` | string | Numeric product version of the source database; empty for structured-file sources. Distribution banners are excluded. |
| `source_kind` | string | Database sources use operator-declared `"production"`, `"staging"`, `"scrubbed-replica"`, or `"synthetic"`. Structured sources use `"parquet"` or `"avro"`. |
| `length_metadata` | string | Summary marker kept for earlier readers: `"hybrid-v2"`, `"exact"`, `"rounded"`, or `"not-captured"`. The three fields below are authoritative. |
| `declared_length_fidelity` | string | `"exact"` for PostgreSQL declared character capacities and for the default balanced/exact MySQL modes; `"coarse-rounded-v1"` for strict MySQL privacy; `"not-captured"` where unavailable. |
| `index_length_fidelity` | string | `"exact"` for default balanced/exact MySQL index prefixes; `"rounded-down-v1"` for strict privacy; `"not-captured"` where unavailable. |
| `observed_length_fidelity` | string | `"relative-rounded-v2"` by default when sampled, `"exact"` in exact mode, `"coarse-rounded-v1"` in strict mode, or `"not-sampled"`. Sampling coverage remains a separate per-column requirement. |
| `[totals]` | inline table | Aggregated counts (see below). |
| `[network]` | table | Optional client-to-database connection and query RTT evidence. |
| `[database_topology]` | table | Required for schema-v6 and newer database sources. Schema v7 uses topology contract v2 and records the scope of every member count. Absent for structured files. |
| `[dataset_scope]` | table | Required for every schema-v6 and newer Blueprint. Declares what the totals cover and whether table, row, and byte coverage are complete. |
| `[structure_scope]` | table | Required in v7. Separately qualifies table, column, index, and relationship inventory completeness. |
| `[source_environment]` | table | Required in v7 database Blueprints and forbidden for structured files. Contains only coarse capacity/hosting evidence observed through the database endpoint or an explicit provider. |
| `[statistics_evidence]` | table | Required in v7. Exact aggregate of the per-table row-count, optimizer-statistics, and size-evidence classifications. |
| `[activity_snapshot]` | table | Not written by DBWarp Blueprint 1.6. |
| `[tables.X]` | tables | One per table, anonymized id. |
| `[fk_edges]` | inline table | FK graph between anonymized tables. Optional. |
| `[artifact_inventory]` | table | Required in v7 so “not requested”, “not applicable”, “unreadable”, and a verified zero-object inventory remain distinct. Contains bounded, name-free object counts, optional typed anonymous relationships, requirements, and a bounded language census. |

## `[totals]`

| Field | Type | Precision |
|---|---|---|
| `table_count` | int | exact |
| `row_count` | int | sum of serialized per-table `rows`; catalog estimates are rounded, while a proven complete bounded read is exact |
| `table_bytes` | int | sum of per-table rounded `table_bytes` |
| `index_bytes` | int | sum of per-table rounded `index_bytes` |

These numbers are not automatically whole-cluster totals. Always interpret
them together with `[dataset_scope]`. A sharded gateway or coordinator can
expose a complete-looking catalog while holding none of the underlying shards;
schema v6 and v7 represent that uncertainty explicitly instead of silently treating
local catalog statistics as global truth.

`row_count` is an arithmetic sum of the serialized per-table values, not a
second unrounded measurement. A known-positive count below the first privacy
bucket is represented as `100` with per-table `row_count_quality =
"engine-estimate"`; an estate containing many such small tables can therefore
have a conservatively high aggregate. In that case `dataset_scope.limitations`
also contains `row-counts-statistical`. Zero remains reserved for evidence that
the source table is empty.

## `[database_topology]` (database sources)

This block records only bounded facts visible through the connected database
endpoint. It never stores node names, hostnames, IP addresses, cluster names,
replication channel names, server identifiers, or endpoints.

| Field | Values / rule |
|---|---|
| `contract` | `dbwarp-blueprint-topology/v1` in schema v6; `dbwarp-blueprint-topology/v2` in v7. |
| `deployment` | `single-node`, `replicated`, `sharded`, `distributed`, or `unknown`. |
| `local_role` | `standalone`, `primary`, `secondary`, `coordinator`, `worker`, `member`, `physical-standby`, `logical-standby`, `snapshot-standby`, or `unknown`. |
| `visibility` | `full`, `partial`, or `unknown`; describes topology evidence, not data correctness. |
| `member_count` | Number of members visible through successful evidence queries. `0` means unknown, never zero members. |
| `member_count_scope` | V7 only: `deployment`, `visible-subset`, `connected-member`, or `unknown`. Full topology visibility requires `deployment`; `connected-member` requires a count of one. |
| `identifiers_redacted` | Must be `true`. |
| `role_counts` | Optional counts by closed role token. Full visibility requires these counts to equal `member_count`. |
| `features` | Sorted closed tokens such as `citus`, MySQL replication/cluster forms, `postgresql-streaming-replication`, `sqlserver-availability-group`, `oracle-non-cdb`, `oracle-cdb`, `oracle-pdb`, `oracle-rac`, `oracle-data-guard`, or `vitess`. |
| `catalogs_read` | Sorted closed labels for topology catalogs successfully read. |
| `catalogs_unreadable` | Sorted closed labels for topology catalogs that could not be read. Any entry prevents a full-visibility claim. |
| `catalogs_not_applicable` | V7 only. Sorted closed labels proven inapplicable to this source. It is disjoint from the readable and unreadable sets. |

An ordinary endpoint may legitimately report `deployment = "unknown"` while
still reporting complete local full-copy table statistics. Blueprint does not
infer that an unremarkable server is single-node merely because no cluster
feature was visible.

## `[dataset_scope]` (schema v6 and newer)

This block qualifies every sizing total independently. Do not treat the totals
as whole-dataset figures when any required completeness dimension is
`incomplete` or `unknown`.

| Field | Values / rule |
|---|---|
| `contract` | Always `dbwarp-blueprint-dataset-scope/v1`. |
| `layout` | `full-copy`, `sharded`, `distributed`, `structured-dataset`, or `unknown`. |
| `table_inventory_completeness` | `complete`, `incomplete`, or `unknown`. |
| `row_count_completeness` | `complete`, `incomplete`, or `unknown`. |
| `size_completeness` | `complete`, `incomplete`, or `unknown`. |
| `row_count_method` | Closed provenance token such as `postgres-planner-estimate`, `mysql-table-statistics`, `sqlserver-partition-counter`, `oracle-table-statistics`, or `oracle-segment-statistics`. `bounded-complete-read` and `mixed-catalog-and-bounded-read` identify totals recovered from a proven complete Tier-2 read, alone or alongside catalog counts. Oracle uses `not-applicable` together with the same size method only when a non-empty inventory has no tables in the copy-total population. `distributed-aggregate` is accepted on input but is not written by this release. |
| `size_method` | Closed provenance token such as `postgres-local-relation-size`, `citus-distributed-relation-size`, `mysql-information-schema`, `sqlserver-partition-pages`, `oracle-segment-bytes`, `oracle-table-logical-estimate`, `mixed`, or `not-applicable`. Oracle uses `mixed` when included tables combine attributed segment counters with labelled logical estimates. It uses `not-applicable` only when a non-empty inventory has no tables in the copy-total population, so a complete zero total does not falsely claim a measurement method. `distributed-aggregate` is accepted on input but is not written by this release. |
| `limitations` | Sorted closed reasons for incomplete or unknown coverage. At least one is required unless every dimension is complete. |

`selection-limited` means the totals and completeness statements cover exactly
the schemas requested through repeatable live `--schema`; they do not claim to
cover the whole connected database. Omitting `--schema` preserves the
all-visible-schema capture behavior.

A readable selected schema may legitimately contain only non-table objects and
is retained in the applicable inventories. When the complete capture contains
no tables at all, however, the collector must not publish a definitive empty
dataset: table, row, and size completeness remain incomplete and
`table-inventory-visibility-unknown` records the conservative boundary.

`row-count-evidence-incomplete` and `size-evidence-incomplete` mean at least
one included table lacked the corresponding catalog value. The numeric total
is then the sum of known contributions, not an assertion that an unavailable
table contained zero rows or bytes. Per-table statistics evidence identifies
which records are unavailable.

For Oracle, `oracle-segment-bytes` is the preferred exact allocated-size
evidence. If storage for a table cannot be attributed from `DBA_SEGMENTS`, for
example, a clustered table or an index-organized table whose index catalogue
is unavailable, the collector may emit `oracle-table-logical-estimate` using the already available
`DBA_TABLES.NUM_ROWS * AVG_ROW_LEN` values. The table evidence then carries
`size_quality = "engine-estimate"`, `size_scope = "table-only"`, and
`size_accounting = "logical-estimate"`, `size_visibility = "partial"`, and the
dataset size completeness is `incomplete`; it does not claim LOB or index
bytes. A missing refinement catalogue never causes bytes already attributed
to that logical table to be discarded: the measured contribution remains
`oracle-segment-bytes`, with partial visibility and incomplete aggregate
coverage. Unattributable shared or index-organized storage is not published as
an exact zero. An Oracle table with a domain index also uses partial visibility
and unknown scope because Text, Spatial, and other domain implementations may
store bytes in secondary objects outside the emitted user-table inventory.
The logical fallback is copy-sizing evidence rather than an
allocated-byte counter. It can overstate current allocation when optimizer row
counts remain stale after storage was released (for example, after `TRUNCATE
... DROP STORAGE`), and its estimate provenance must be retained. No
`DBA_TABLESPACES` grant is required for this fallback.

For Oracle index storage, the completed index-catalogue read is the logical
snapshot boundary. A later `DBA_SEGMENTS` row with no matching index identity
is outside that population and is not assigned to an arbitrary table. An index
that was present at the boundary but lacks its expected segment contribution
does withdraw full size visibility for its table.

`logical-partition-root-unmeasured` is PostgreSQL-specific evidence that an
included logical partition root deliberately contributes neither rows nor
bytes because those values live on its physical leaves. Unlike a curable
`row-count-evidence-incomplete` statistics gap, a complete read of another
table cannot restore dataset-level completeness while such a root remains in
the selected inventory.

`table-inventory-visibility-unknown` means the collector could not read the
engine-owned classification needed to separate user objects from support
objects. Visible records may still be present, but table, row, and size
completeness are withdrawn rather than treating that subset as the estate.

`catalog-capture-truncated` means a single-source catalogue session stopped
before every intended family or selected schema was read. Records already
proved complete may still be emitted, but no dataset or structure completeness
claim may extend to the unread remainder.

The native PostgreSQL, MySQL, and SQL Server collectors probe supported
topology catalogs before deciding whether local statistics can represent the
logical dataset. Known distributed gateways suppress unsafe totals when a
reliable aggregate is unavailable. The SQL fallback formatter has no topology
probe, so it emits its useful local estimates with all scope dimensions marked
`unknown` and the limitations `topology-unobserved` and
`topology-visibility-unknown`.

Structured Parquet and Avro Blueprints omit `[database_topology]` and use
`layout = "structured-dataset"` with footer/container provenance.

Blueprint does not run a storage-speed test during ordinary capture and
does not infer database-server hardware from the machine running the client.
Database byte totals describe stored data volume through the named catalog
method; they do not claim disk type, IOPS, throughput, CPU, RAM, or target
migration performance.

## `[structure_scope]` (schema v7)

This block makes a verified empty catalog distinguishable from a catalog that
was filtered, unreadable, or not inspected.

| Field | Values / rule |
|---|---|
| `contract` | Always `dbwarp-blueprint-structure-scope/v1`. |
| `visibility` | `full`, `privilege-filtered`, or `unknown`. Completeness is within the selected schemas and visible privilege scope; it is not a claim of unrestricted database visibility. |
| `table_inventory_completeness`, `column_inventory_completeness`, `index_inventory_completeness`, `relationship_inventory_completeness` | Independently `complete`, `incomplete`, or `unknown`. Dependent families cannot claim complete when their required parent family is incomplete. |
| `catalogs_read`, `catalogs_unreadable`, `catalogs_not_applicable` | Sorted, disjoint closed catalog labels. `catalogs_read` records positive read evidence; on a multi-owner capture it may retain a catalog when at least one intended owner read succeeded even though another did not. `catalogs_unreadable` means no positive read survived. A complete family requires its engine-specific catalog in `catalogs_read`, every intended family query to have completed, and no affected per-object gap. |
| `limitations` | Sorted closed reasons such as `selection-limited`, `metadata-visibility-privilege-filtered`, or `table-kinds-not-inventoried`. Partial or unknown evidence requires a reason. |

Schema selectors are part of the scope: `complete` means complete for the
resolved selected schemas, not necessarily every schema in the service. A
selector that resolves to no schema is an error and must not become a complete
empty Blueprint.

`catalog-capture-truncated` has the same meaning in structure evidence: the
published table and column records are the verified prefix or owner subset,
not a claim that the remaining intended catalogue work was completed. A
catalogue left unattempted by that stop appears in none of the three catalogue
sets; it must not be relabelled as unreadable or not applicable.

For a multi-owner read, `index-inventory-unavailable` or
`relationship-inventory-unavailable` may therefore accompany a catalog in
`catalogs_read`: the catalog label preserves the successful owner's positive
evidence while the completeness field and limitation record that the whole
selected population was not observed. Per-table limitations identify emitted
objects with a representability gap; they do not replace query-status evidence
for a denied or unattempted owner that emitted no tables.

Oracle records `oracle-identity-columns` and `oracle-constraint-columns`
separately from their parent column and constraint catalogues. Their presence
or absence describes optional identity-generation and relationship-key
evidence; it must not be collapsed into a claim that the parent catalogue was
unreadable.

## `[source_environment]` (schema v7 database sources)

This block never describes the workstation running `dbwarp-blueprint`.
`collector_machine_excluded` must be `true`. Capacity evidence comes only from
the connected database endpoint or an explicitly authorized provider,
orchestrator, or operator attestation.

| Field | Values / rule |
|---|---|
| `contract` | Always `dbwarp-blueprint-source-environment/v1`. |
| `evidence_origin` | `database-endpoint`, `provider-api`, `orchestrator-api`, `operator-attested`, `mixed`, or `none`. |
| `hosting_model` | `managed-service`, `self-managed`, `orchestrated`, or `unknown`. |
| `infrastructure_location` | `cloud`, `on-premises`, `hybrid`, or `unknown`. |
| `capacity_scope` | `connected-instance`, `database-resource`, `cluster-aggregate`, `member-subset`, or `unknown`. |
| `capacity_visibility` | `full`, `partial`, `unknown`, or `not-requested`. `not-requested` requires unknown capacity bands, bases and scope, and no classified capacity catalogue. Non-capacity classification, such as SQL Server edition, may still be present. |
| `cpu_capacity_band` | `1`, `2`, `3-4`, `5-8`, `9-16`, `17-32`, `33-64`, `65-128`, `129-plus`, or `unknown`. |
| `cpu_capacity_basis` | `logical-cpu-limit`, `database-resource-limit`, `operating-system-visible`, `physical-host`, or `unknown`. `operating-system-visible` does not claim that a VM, container, or managed-service allocation is the underlying physical host. |
| `memory_capacity_band` | Coarse bands from `under-2-gib` through `512-gib-plus`, or `unknown`. |
| `memory_capacity_basis` | `database-buffer-cache`, `database-resource-limit`, `operating-system-visible`, `physical-host`, or `unknown`. `operating-system-visible` is the conservative basis for an engine DMV whose value may describe a guest or container rather than bare metal. A `database-buffer-cache` band is the configured cache allocation and therefore only a lower bound on total source memory; it must never be rendered as host capacity without its basis. |
| `member_capacity_uniform` | Optional observed/attested boolean; omission means unknown. |
| `features` | Sorted closed facts such as `autoscaling`, `burstable`, `container-limits-visible`, `database-resource-governed`, `serverless`, or `shared-host`. |
| `limitations` | Sorted closed provenance limitations. `oracle-client-version-mismatch` or `oracle-client-version-unreadable` records that an Oracle SQL*Plus client version could not be fully attested. `oracle-client-version-below-tested-floor` records an attested client older than 12.1, the comparison floor encoded by this contract. Catalog capture still proceeds because client-banner provenance does not determine database shape. |
| catalog sets | Sorted, disjoint evidence for the exact source-environment catalogs attempted. SQL Server records edition classification at every tier. An enhanced capture also records the optional capacity DMV outcome. |

Unknown capacity is not zero capacity. A remote connection does not authorize
reading the collector host's CPU or memory and relabelling it as server
capacity.

For Oracle, a capacity catalogue that completed for only part of the intended
query set remains positive `catalogs_read` evidence, but its values are
withheld and `capacity_visibility` is `unknown`. Partial rows must not be
presented as an estate-wide CPU or memory limit.

Oracle SQL*Plus feature settings are selected from the running session's
numeric release when readable, then from the executable banner, and otherwise
from a conservative protocol floor that selects neither `ROWLIMIT` nor CSV
markup as an output feature. Both settings passes still attempt to clear an
inherited `ROWLIMIT` and CSV mode; an older client's unknown-option diagnostic
is tolerated only inside the framed reset window. Version mismatch, partial
attestation, parse failure, or an attested client older than 12.1 weakens
provenance only. It never blocks catalog capture.

## `[statistics_evidence]` and `[tables.<id>.statistics]` (schema v7)

Every v7 table has a statistics block. The top-level block contains exact
counts by `statistics_state`, `row_count_quality`, and `size_quality`; each map
must cover every table and exactly equal the table-level classifications.
Aggregate visibility is `full` only for a non-empty copy-total population when
every counted table has full size visibility, known row evidence and a
classified statistics state, and no statistics catalogue is unreadable.
Deliberately excluded external, temporary and derived objects remain
inventoried; their policy-driven row and size unavailability does not lower
that copy-population visibility, but an unclassified statistics state still
does. When every table is excluded, aggregate visibility is `unknown` with
`statistics-visibility-unknown`; an empty population must not earn `full`
vacuously. `catalog-capture-truncated` records that intended statistics
catalogue work stopped before every owner was reached while retaining any
positive catalogue-read evidence that was already obtained. The same positive
evidence rule applies when one owner read succeeds and another is denied: the
catalog remains in `catalogs_read`, while `statistics-partial` and aggregate
visibility record that the selected population was not fully observed.

Table-level fields are:

| Field | Values / rule |
|---|---|
| `row_count_method` | Engine/version-aware catalog method, `bounded-complete-read` when a Tier-2 statement safely enumerated the visible table, a structured-file counter, or `unknown`; ordinary capture does not silently fall back to `COUNT(*)`. |
| `row_count_quality` | `exact-counter`, `exact-read`, `engine-counter`, `engine-estimate`, `cached-engine-estimate`, `sample-extrapolation`, `unavailable`, or `unknown`. A known-positive SQL Server counter below the first non-zero privacy bucket uses `engine-estimate` after serializing `rows = 100`; this distinguishes the privacy bucket from both an exact counter and a measured zero. |
| `statistics_state` | `current`, `possibly-stale`, `known-stale`, `never-analyzed`, `locked`, `user-supplied`, `not-applicable`, or `unknown`. |
| `refresh_age_band` | `under-1h`, `1h-1d`, `1-7d`, `1-4w`, `1-3m`, `3m-plus`, `unknown`, or `not-applicable`. |
| `modification_ratio_band` | `none`, `under-1pct`, `1-5pct`, `5-10pct`, `10-20pct`, `20-50pct`, `over-50pct`, `unknown`, or `not-applicable`. |
| `sample_fraction_band` | `full`, `75-99pct`, `50-74pct`, `25-49pct`, `under-25pct`, `unknown`, or `not-applicable`. |
| `statistics_scope` | `global`, `partition`, `subpartition`, `session`, `local-member`, `logical-dataset`, `database-resource`, `structured-dataset`, `selected-object`, or `unknown`. |
| `size_method`, `size_quality`, `size_scope`, `size_accounting`, `size_visibility` | Separately describe where size came from, whether it is a counter or estimate, whether it includes LOB/index storage, whether it is allocated or logical, and whether visibility is full, partial, unavailable, or unknown. |

The top-level statistics block uses `visibility = "full"`, `"partial"`, or
`"unknown"` for the non-empty copy-total population described above. It never
uses excluded-only objects to earn `full` visibility.

Oracle `oracle-segment-bytes` evidence is valid only with `exact-counter`,
`allocated-segment`, full or partial visibility, and a `segment_state` proving
that the segment census was attributed (`created`, `deferred`, `mixed`, or
`mixed-table-and-index`). A
logical fallback instead uses `oracle-table-logical-estimate`,
`engine-estimate`, `logical-estimate`, partial visibility, and an unavailable
segment state. This prevents an unattributed storage class from becoming a
measured zero. The state is derived from attributed catalogue evidence before
privacy rounding. `created` can therefore accompany serialized zero table bytes
when a known-positive raw table counter falls below the first byte bucket.
Partial measured Oracle evidence uses `size_scope = "unknown"`: the attributed
bytes remain exact, but a missing LOB, nested-storage, or index mapping means
the collector cannot honestly claim the full table/LOB/index scope. For Oracle,
`mixed` means a positive attributed index allocation was suppressed to
serialized `index_bytes = 0` by rounding; it keeps the zero distinct from a
table for which the segment census found no index allocation.
`mixed-table-and-index` means both table and index allocations were positive
before rounding and both serialized counters are zero, preserving both facts
without disclosing sub-bucket byte values.
`deferred` means the attributed raw counters were zero and requires both
serialized byte values to be zero. Use the state to distinguish
a rounded sub-bucket allocation from storage proven not to be materialized.

The top-level block also records sorted, disjoint catalogs and closed
limitations. A zero `rows` or `table_bytes` is usable as an observed zero only
with supporting quality/visibility evidence; do not ignore the
provenance block. A counted table whose row or size quality is `unavailable`
or `unknown` forces the corresponding dataset completeness away from
`complete`; the validator rejects a numeric placeholder presented as complete
coverage. In particular, a PostgreSQL table with neither optimizer statistics
nor a proven complete bounded read has unknown row volume, not a measured
zero.

Oracle Basic omits `check_count` when the dictionary cannot distinguish a
declared `NOT NULL` constraint from an explicit, textually identical `CHECK`.
It does not guess from a generated constraint name or from the column's current
nullability. Other tables whose constraint rows are unambiguous may still carry
an exact count.

## `[activity_snapshot]`

DBWarp Blueprint 1.6 does not write this block.

## `[network]` (optional)

Round-trip time from the machine running the collector to your database. This
is not the round-trip time between migration source and target.

The probe runs after connection establishment and before catalog
queries, so timings aren't skewed by query-cache warmup. It executes
**5× `SELECT 1`** and emits the median latency. Each `SELECT 1`
returns the constant integer 1: no row data is ever read by this
probe.

Absent when `--no-rtt-probe` is used or when the probe
itself failed mid-flight (recorded as a non-fatal warning to stderr
and audit log; the Blueprint file is still emitted without the block).

| Field | Type | Precision |
|---|---|---|
| `sample_count` | int | exact (always 5 in v1) |
| `connect_total_ms` | int | total wall-clock from start of TCP connect to authenticated session ready, in milliseconds. Includes TCP handshake + TLS handshake (when applicable) + auth challenge/response. Rounded to nearest ms. Typically 3–6× `query_rtt_ms_p50`. |
| `query_rtt_ms_p50` | int | median single-round-trip latency from the 5 `SELECT 1` samples, in milliseconds. Rounded to nearest ms. The natural network noise floor (≥ 1 ms in practice) is wider than the rounding granularity, so this kills any low-bit hidden channel without losing useful precision. Sub-ms LAN values collapse to 0 or 1. |
| `query_rtt_ms_p95` | int | nearest-rank 95th percentile of the 5 samples (the slowest observation), in milliseconds. Rounded to nearest ms. Use it with p50 to identify short latency spikes; five samples are an orientation signal, not a workload performance test. |

The 5 probe queries appear in the audit log as a **single summary
entry** (not 5 separate rows) labelled `5x SELECT 1 (RTT probe;
constant integer 1, no row data)`: matching the trust posture that
no row content is read.

## `[tables.<id>]`

Identifier is `table-NNN` where `NNN` is the 1-indexed ordinal in a
domain-separated HMAC-SHA256 ordering of the schema and table name. The default
key is freshly generated for the process and is never emitted. Passing the same
protected `--anonymization-key-file` preserves the ordering across approved
comparison runs. Schema v7 requires the complete dense ordinal set from
`table-001` through the emitted table count (the width grows naturally at
`table-1000`); skipped, zero, non-decimal, or source-derived suffixes are
invalid.

| Field | Type | Precision / values |
|---|---|---|
| `rows` | int | Catalog estimates are rounded: nearest 100 (≤10k), 1000 (≤1M), 10000 (>1M). A known-positive estimate that would otherwise round to zero uses the first non-zero bucket (`100`); zero is reserved for a catalog zero or unavailable evidence identified by the adjacent statistics quality. When a bounded Tier-2 read proves it enumerated the complete visible table, `rows` is the exact number already disclosed by that sample's exact `sample_rows`; this avoids contradictory per-table, cardinality, and aggregate counts without adding a new channel. |
| `table_bytes` | int | rounded: nearest 1KiB / 1MiB / 100MiB by magnitude |
| `index_bytes` | int | rounded: same as `table_bytes` |
| `schema` | string | Anonymized id `schema-A`, `schema-B`, ..., `schema-AA`. Schema v7 requires the dense alphabetic ordinal set over every schema referenced by an emitted table or graph/analyzed artifact; a selected schema containing only non-table objects is therefore retained. |
| `object_kind` | string | V7 required closed token: `ordinary-table`, `materialized-view`, `external-table`, `temporary-table`, `nested-table`, or `object-table`. Object identity is independent of physical storage and partitioning. |
| `storage_organization` | string | V7 required closed token: `heap`, `index-organized`, `clustered`, `external`, or `unknown`. `external` is valid only for `object_kind = "external-table"`. |
| `partitioning` | string | V7 required closed token: `none`, `range`, `list`, `hash`, `interval`, `reference`, `composite`, `system`, `key`, `linear-hash`, `linear-key`, or `unknown`. |
| `segment_state` | string | V7 required closed token: `created`, `deferred`, `mixed`, `mixed-table-and-index`, `unavailable`, or `unknown`. This separates metadata-only objects from materialized storage. It is categorical evidence established before byte rounding, so `created` may accompany zero serialized table bytes for a positive sub-bucket allocation. With Oracle segment-counter evidence, `mixed` records a positive attributed index allocation whose serialized `index_bytes` rounded to zero; `mixed-table-and-index` records that both raw allocations were positive while both serialized counters rounded to zero. |
| `parent_table`, `child_tables` | string / array | Optional reciprocal anonymous table links for nested, partitioned, or otherwise contained objects. Child ids are sorted and unique; the parent graph must be acyclic. |
| `table_features` | array | Sorted closed tokens: `graph-edge`, `graph-node`, `memory-optimized`, `temporal-current`, or `temporal-history`. |
| `unlogged` | bool | Optional PostgreSQL logged-state observation. Omitted when not captured; explicit `false` means the catalog proved the table is logged. |
| `partition_count` | int | Exact in-scope physical leaf-partition count, required when `partitioning` names a known partitioning strategy. PostgreSQL reports recursive leaf partitions and excludes leaves outside the resolved schemas under `selection-limited`. MySQL composite tables count subpartitions because those are their physical leaves; for example, four top-level partitions with eight subpartitions each report `32`. Zero is valid only for a logical partition root with `segment_state = "unavailable"` and no in-scope leaf. |
| `partition_key_cols` | array of int | Complete simple partition-key column ordinals. Omitted for a wholly or partly expression-based key, or when catalog evidence is unavailable; a partial ordinal list and key expressions are never serialized. |
| `partition_rows_max` | int | Optional rounded largest-leaf row estimate. For estimate-grade table totals, a known-positive value uses the first non-zero row bucket capped by serialized `rows`. With an exact-read table population, a largest-leaf estimate whose privacy bucket would be zero or exceed that exact population is omitted as unrepresentable rather than clamped into a false value. When present it cannot be zero while `rows` is positive or exceed `rows`. |
| `temporal_history` | string | Anonymous table id of the paired temporal-history table, required with the `temporal-current` feature unless that table carries an applicable per-object `table_limitations` token. Capture-wide selection alone never waives the link. |
| `table_limitations` | array | Sorted closed per-object evidence. `table-classification-unavailable` identifies a table whose object-kind inputs were incomplete. `column-inventory-unavailable` identifies a table with one or more missing or unreadable column records; `dependent-structure-suppressed` says both index and relationship structure for that table cannot be claimed complete because an emitted column is absent. `index-inventory-unavailable` and `relationship-inventory-unavailable` narrow a refinement-only gap to the affected dependent family without withdrawing the required column inventory. Any index, partition key, or relationship that references an absent emitted column is omitted rather than allowed to invalidate the whole capture. `relationship-target-outside-selected-scope` records that at least one declared foreign key on this table targets an object outside the resolved selected-schema scope; it is valid only in a `selection-limited` capture. `relationship-target-visibility-unknown` records that the catalog disclosed a foreign-key target that could not be resolved in the visible inventory; relationship completeness must then be incomplete. `row-security-filter-active` records a visible enabled SQL Server filter predicate. `row-security-visibility-unknown` records that complete SQL Server security-policy catalog visibility could not be proved, so Tier-2 sampling is suppressed rather than treating a potentially filtered subset as the table population. `temporal-history-outside-selected-scope` is valid only for an unlinked temporal-current table in a `selection-limited` capture after the collector resolved the history schema outside the selection. `temporal-history-visibility-unknown` records that the catalog disclosed a history object id but not enough metadata to resolve it. |
| `counted_in_totals` | bool | Omitted means included. An `external-table`, `materialized-view`, `temporary-table`, or table carrying `memory-optimized` requires explicit `false`, excluding external, derived, session-scoped, or currently unmeasured data from `table_count`, `row_count`, `table_bytes`, and `index_bytes`. Per-object evidence remains available for recreation planning without presenting unavailable values as measured totals. No other explicit value is canonical. |
| `check_count` | int | Optional exact structural CHECK-constraint count. Omitted means unknown; `0` means the relevant catalog proved none. |
| `has_clustered_index` | bool | always `false` for PostgreSQL |
| `[tables.<id>.statistics]` | sub-table | Required v7 provenance for row count, optimizer-statistics state, and size evidence. The v6 `stats_freshness` field is accepted only when reading older files and is never emitted in v7. |
| `[tables.<id>.cols.<cid>]` | sub-tables | one per column |
| `[tables.<id>.idxs.<iid>]` | sub-tables | one per index |
| `[tables.<id>.compression]` | sub-table | only if Tier 2 |

## `[tables.<id>.cols.<cid>]`

Identifier is `col-N` where `N` is the column's natural attribute order
(1-indexed, preserving the on-disk ordinal). Stable across runs. In schema v7
the decimal suffix must exactly equal `ordinal`; zero, leading-zero spellings,
and source-derived labels are invalid. Source engines may retain gaps in
physical column ordinals after a dropped column.

| Field | Type | Notes |
|---|---|---|
| `ordinal` | int | the same N as the id |
| `type` | string | normalized type family such as `"integer"`, `"numeric(12,2)"`, `"text"`, `"json"`, `"binary"`, `"timestamp"`, `"uuid"`, `"array<integer>"`, or `"user-defined"`. Real domain, enum, alias, composite, and user-defined type names are not emitted. |
| `nullable` | bool | |
| `value_source` | string | Schema v6 optional closed token: `identity-always`, `identity-default`, `auto-increment`, `identity`, `sequence-default`, `generated-stored`, `generated-virtual`, `computed-persisted`, `computed-virtual`, `system-time`, or `rowversion`. Omitted for an ordinary supplied value or unknown evidence. |
| `has_default` | bool | Schema v6 optional catalog observation. Omitted means unknown; explicit `false` means the catalog proved no default. |
| `default_kind` | string | Schema v6 optional classification `constant`, `function`, or `expression`; valid only with `has_default = true`. Default text and literals are never serialized. |
| `default_on_null` | bool | V7 optional source-catalog observation for Oracle `DEFAULT ON NULL`; valid only when a default is present. Omitted means not observed. |
| `type_kind` | string | Schema v6 optional closed token: `enum`, `set`, `domain`, `composite`, `array`, `range`, or `alias`. Omitted for a base type or unknown evidence. |
| `member_count` | int | Schema v6 exact positive structural member count, required only for `enum` and `set`; member names are never serialized. |
| `domain_has_check` | bool | Schema v6 optional domain CHECK observation, valid only with `type_kind = "domain"`. |
| `hidden`, `invisible`, `masked`, `encrypted`, `sparse` | bool | Optional catalog observations. `invisible` is distinct from an engine-created hidden column. Omitted means unknown; explicit `false` means the catalog proved the property absent. |
| `has_check` | bool | Schema v6 optional single-column CHECK observation. Every explicit `true` is covered by the table's `check_count`. |
| `null_fraction` | float | Optional observed null fraction from `0.0` through `1.0`. When cardinality is present it is derived from that block's privacy-rounded public counts; otherwise it is rounded independently. No null bitmap is retained. |
| `native_type` | string | Optional sanitized engine base type, such as `varchar` or `longtext`; no identifiers, enum members, defaults, or expressions. Emitted by the native MySQL and SQL Server collectors. |
| `declared_max_chars` | int | Optional declared character capacity. Exact for PostgreSQL `character`/`character varying` catalog values and in default balanced/exact MySQL modes; coarsely rounded only with MySQL `--length-fidelity strict`. |
| `declared_max_bytes` | int | Optional declared byte capacity. Exact in default balanced/exact MySQL modes; coarsely rounded only with `--length-fidelity strict`. |
| `length_semantics` | string | V7 optional declared length unit: `characters`, `bytes`, `not-applicable`, or `unknown`. This preserves Oracle CHAR-versus-BYTE semantics without serializing declarations. |
| `numeric_model` | string | V7 required closed family: `integer`, `fixed-decimal`, `unconstrained-decimal`, `decimal-float`, `binary-float`, `not-applicable`, or `unknown`. `not-applicable` marks a known non-numeric type; `unknown` is reserved for a numeric or user-defined type whose semantics were not classified. `decimal-float` includes exact Oracle `FLOAT(p)` values and is not IEEE floating point. |
| `numeric_precision` | int | Optional positive declared precision, bounded by the source engine and model: Oracle `NUMBER` and SQL Server decimal precision through 38, Oracle `FLOAT(p)` through 126 binary digits, MySQL decimal precision through 65, and PostgreSQL numeric precision through 1,000. |
| `numeric_scale` | int | Optional signed declared scale, validated against the source engine. Oracle `NUMBER` uses `-84..127`; PostgreSQL supports its wider version-dependent declaration range, while MySQL, SQL Server, Parquet, and Avro require a non-negative scale no greater than precision. Negative Oracle/PostgreSQL scale and scale greater than precision where the engine permits it are retained. |
| `numeric_precision_radix` | string | `decimal` or `binary` when required by the numeric model. Oracle `FLOAT(p)` uses binary precision with the exact `decimal-float` value model; `BINARY_FLOAT` and `BINARY_DOUBLE` use `binary-float`. |
| `numeric_unsigned`, `bit_width` | bool / int | Optional integer semantics where the source engine exposes them. |
| `datetime_precision` | int | Optional engine-declared date/time fractional precision. |
| `charset`, `collation` | string | Optional sanitized character metadata. MySQL emits its catalog charset and collation names. SQL Server emits `utf-16le` for `nchar`/`nvarchar`/`ntext`, `utf-8` for code page 65001, `windows-N` for Windows code pages 1250-1258, or `code-page-N` for another positive catalog code page, plus the catalog collation name. These are encoding facts and catalog names, never your identifiers or values. |
| `len_avg` | int | Sampled average bytes for variable-length values. Default relative buckets have about 3.2% maximum error and preserve values through 32 bytes exactly; exact with `--length-fidelity exact --yes`; coarse nearest-10 only in strict mode. 0 = fixed-length or unmeasured. |
| `len_p95` | int | Sampled 95th percentile with the same default relative buckets; exact with `--length-fidelity exact --yes`; coarse nearest-100 only in strict mode. 0 = unmeasured. |
| `style` | string | Tier 2 only. One of `"json"`, `"xml"`, `"natural-text"`, `"base64"`, `"hex"`, `"numeric-text"`, `"mixed"`, or `"precompressed"`; empty if not classified. `"precompressed"` is emitted only for a material byte-dominant sample of binary values carrying recognized standard container signatures. It deliberately does not disclose the detected container family. |
| `[tables.<id>.cols.<cid>.lob_storage]` | sub-table | V7 optional database LOB storage evidence: `storage_class` (`basicfile`, `securefile`, `external`, `unknown`), compression (`none`, `low`, `medium`, `high`, `not-applicable`, `unknown`), deduplication (`enabled`, `disabled`, `not-applicable`, `unknown`), optional in-row/encrypted flags, and visibility (`full`, `partial`, `unknown`). External content requires the two storage controls to be `not-applicable` and omits in-database flags. No path or segment name is retained. |
| `magnitude_min`, `magnitude_max` | int | Schema v6 optional signed decimal exponents bounding sampled non-null numeric magnitudes. They are emitted together with `has_negative`; exact values are never serialized. |
| `has_negative` | bool | Schema v6 optional sampled sign observation, emitted only with both magnitude bounds. |
| `time_span` | string | Schema v6 optional sampled date/time range: `intraday`, `days`, `weeks`, `months`, `years`, or `decades`. |
| `time_recent_decade` | int | Schema v6 decade containing the newest sampled date/time, emitted only with `time_span` and always divisible by 10. |
| `[tables.<id>.cols.<cid>.compression]` | sub-table | Tier 2 only. Present for sampled text/binary candidate columns. Same field layout as table-level compression, but scoped to one anonymized column. |
| `[tables.<id>.cols.<cid>.cardinality]` | sub-table | Schema v3 sampled-value distribution summary. Contains bounded/rounded counts and frequencies only. |

`numeric_model` is authoritative for numeric semantics. `type` keeps the
engine-family spelling: Oracle's
`NUMBER` family is accompanied by `type = "number"` and `FLOAT(p)` by
`"float"`, while `native_type` preserves the sanitized original declaration.


### `[tables.<id>.cols.<cid>.cardinality]` (schema v3)

When row sampling is enabled, the collector keeps at most 8,192 temporary
64-bit fingerprints per column in memory, derives aggregate NDV/skew
statistics, and discards the fingerprints. Neither values nor fingerprints are
serialized. The block contains `measured`, `sample_rows`, `non_null_rows`,
`observed_distinct_count`, `estimated_distinct_count`, `top_value_fraction`,
`frequency_p50`, `frequency_p95`, `frequency_p99`, `frequency_max`,
`sample_method`, `complete_source_read`, `sample_layout`, `sampled_with_bias`,
and `bias_reason`. `complete_source_read = true` is machine-readable evidence
that one bounded statement observed the complete visible source population and
retained this column without value-level truncation. A proven complete row read
keeps `sample_rows` in the table's exact row domain even when a cell cap or
bounded fingerprint reservoir makes `complete_source_read` false; the exact
table population is already present in `tables.<id>.rows`, so this discloses no
additional fact. `non_null_rows` is privacy-rounded first, and `null_fraction`
is then derived as `(sample_rows - non_null_rows) / sample_rows`. The fraction
therefore remains exactly consistent with the public counts and can fall off
the standalone 0.005 fraction grid without disclosing another fact. Exact zero
and all-non-NULL endpoints are preserved; a mixed census keeps a positive
non-NULL population positive and remains below `sample_rows`. Mixed
`non_null_rows` uses the same
magnitude-relative count grid as the other cardinality counts. At the dense
tail this can place the count as much as one full count bucket below the true
retained population (for example, `9,728` for `9,999`); it is not a near-exact
count. The exact all-non-NULL endpoint deliberately reveals that no NULL was
observed in the retained rows, while any observed NULL keeps the count below
`sample_rows`. Distinct and frequency counts stay on their documented privacy
grid even when bounded by that population. For database
sources, an incomplete read caps `sample_rows` at the rounded catalogue row
estimate; for Parquet and Avro, the exact footer row count is the cap. In both
paths `sample_rows` is the exact retained row count already disclosed by the
table-level compression block's `sample_rows`, unless that cap is lower. It
must never be clamped upward to imply complete coverage. Value
truncation keeps distinct and frequency evidence conservative and must never
inflate it beyond the table population. Do not infer completeness
by parsing `sample_method`.
`sample_layout` is an optional machine-readable enum. The current emitted
value is `primary-key-range-windows`; absence means that no ordering contract
is available. Do not infer semantics by parsing the
human-readable `sample_method` field.

Counts and fractions are privacy-rounded where appropriate. The statistics
describe duplicate density, hot-value skew and finite domains. They contain no
sampled values, but distinctive
distributions can fingerprint a workload; do not treat them as irreversible
or as proof that business meaning cannot be inferred from outside knowledge.
A bounded statement can prove the visible row population without proving that
every sampled cell was retained in full. If a server-side cell cap truncates a
column, its cardinality remains a bounded, biased estimate even when the table
row count is recorded from a complete bounded read; unaffected columns may
still carry complete-read cardinality provenance.

### `[tables.<id>.cols.<cid>.compression]` (Tier 2 only)

Per-column compression is emitted only for bounded text/binary candidates when
`--measure-compression --yes` is used. It gives a per-column compression
estimate.

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
sample_method = "TABLESAMPLE SYSTEM REPEATABLE(0) LIMIT N (adaptive estimate-aware rate; simple-query text fields; raw binary/vector decoded; server-side cell cap)"
sampled_with_bias = true
bias_reason = "server_side_cell_cap"
ratio_zstd_3 = 8.4
ratio_stddev = 0.25
sample_encoding = "blueprint-compression-probe-v2"
```

No sampled column values are written to the Blueprint file.

For binary columns, the same bounded Tier-2 sample may emit the coarse
`style = "precompressed"` profile. Recognition happens only at sampled value
boundaries and requires a material, byte-dominant observation. Blueprint does
not parse or decompress the value, retain its signature, or distinguish image,
archive, compressed-media, encrypted, and random payloads beyond this one
high-confidence label. Textual and base64 encodings remain classified by their
text style and are not treated as precompressed binary containers.

## `[tables.<id>.idxs.<iid>]`

Identifier is `idx-N` where `N` is the 1-indexed ordinal of the index
within the table, sorted by a domain-separated HMAC-SHA256 of the index name.
Schema v7 requires the dense set `idx-1` through `idx-N` for each table; zero,
leading zeros, gaps, and non-decimal suffixes are invalid.

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
| `partitioning` | string | V7 optional physical partitioning: `none`, `local`, `global`, or `unknown`. |
| `visibility` | string | V7 optional source visibility: `visible`, `invisible`, or `unknown`. |
| `state` | string | V7 optional operational state: `usable`, `unusable`, `in-progress`, `failed`, or `unknown`. |
| `prefix_distinct_counts` | array of int | Schema v3 estimated distinct tuple count for each key prefix from one through N columns. Zero means unavailable for that prefix. |
| `cardinality_sample_method` | string | Bounded provenance for `prefix_distinct_counts`; inferred products are explicitly labelled and are not presented as direct tuple samples. |

## `[tables.<id>.compression]` and `[tables.<id>.cols.<cid>.compression]` (Tier 2 only)

Present only when the file was generated with `--measure-compression --yes`.
The table-level block measures a neutral columnar projection of the complete
sample and remains the authoritative ratio for whole-table transfer estimates.
Column-level blocks are projected from the same sampled rows, one column at a
time, and show which columns compress well without exposing sampled values.
They do not trigger extra database reads.

PostgreSQL tables governed by active row-level security, including inherited
or partition children whose ancestor policy would be bypassed by a direct
child query, and SQL Server tables governed by an enabled security-filter
predicate are not sampled. Their catalog evidence is retained, and the run
records `DBP1407W`, rather than extrapolating a policy-filtered subset as the
whole table.

| Field | Type | Precision |
|---|---|---|
| `measured` | bool | always `true` if block is present |
| `sample_rows` | int | exact |
| `sample_bytes` | int | size of the in-memory sample buffer, **bucketed**: nearest **64 KiB** below 1 MiB, nearest **1 MiB** below 1 GiB, nearest **100 MiB** above. Bytes never written to disk. The bucketing kills the per-table low-bit hidden channel an exact `buf.len()` would otherwise expose. |
| `sample_method` | string | engine-specific bounded sampling description, for example `"TABLESAMPLE SYSTEM REPEATABLE(0) LIMIT N (adaptive estimate-aware rate; simple-query text fields; raw binary/vector decoded; server-side cell cap)"`, `"LIMIT N (fallback after underfilled adaptive TABLESAMPLE; simple-query text fields; raw binary/vector decoded; server-side cell cap)"`, or `"SELECT TOP N bounded projection FROM <table> (compression sample; server-side cell cap)"` |
| `sampled_with_bias` | bool | true if the sample is non-uniform, for example a LIMIT-only fallback |
| `bias_reason` | string | empty if `sampled_with_bias = false`, else a tag such as `"unordered_limit_after_underfilled_adaptive_TABLESAMPLE+server_side_cell_cap"` |
| `ratio_zstd_3` | float | rounded to nearest **0.05**, using the contract's zstd level 3 measurement policy. Measured on bytes encoded via `sample_encoding`. |
| `ratio_zstd_19` | float | Not written by this release; may appear in files from earlier releases. |
| `ratio_stddev` | float | rounded to nearest **0.05**, stddev of level-3 ratios across bounded table probe frames. Column-level projection blocks currently emit `0.0` because they are advisory entropy hints, not a variance model. |
| `sample_encoding` | string | Identifier for the byte-level encoding and compression-session policy used for the measurement. PostgreSQL live table blocks use `"blueprint-columnar-transfer-probe-v2"`. MySQL and SQL Server use `"blueprint-columnar-transfer-probe-v3"`, which additionally flushes at 256 KiB probe chunk boundaries. SQL Server `nvarchar`/`nchar`/`ntext` payloads retain native UTF-16LE byte distributions; `varchar`/`char`/`text` retain their sampled byte width and the `charset` field identifies the catalogue code page. V1 is accepted on input. Per-column blocks use `"blueprint-compression-probe-v2"`. Ratios measured with different `sample_encoding` values are not comparable. |

PostgreSQL uses v2 and MySQL and SQL Server use v3; compare ratios only within
one encoding.

### `blueprint-compression-probe-v2` byte-level encoding

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

Type tags are part of the probe contract and will not be renumbered without a
new versioned probe identifier.

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

### `blueprint-columnar-transfer-probe-v1`, `v2`, and `v3` byte-level encoding

Live-database table ratios transform the same bounded per-column v2 samples
into neutral 1,000-row frames. Each frame has a versioned probe header and, for
every column, an ordinal, one type tag, a four-byte length per row, and then the
column-contiguous payload bytes. A length of `0xffffffff` represents NULL. The
byte representation is shared by all three versions. V1 compressed the joined
frame sequence as one pledged-input zstd level-3 operation. V2 feeds
the frames through one persistent zstd level-3 context and flushes after every
frame. V3 preserves that context and neutral row-group representation, but
also flushes at each 256 KiB probe compression chunk within a row group. MySQL
and SQL Server capture use v3; v2 remains the current PostgreSQL measurement.
SQL Server Unicode text is measured as UTF-16LE.
SQL Server narrow text retains the source byte width and records a closed,
sanitized charset derived from the collation code page. Outer row-group outputs
provide `ratio_stddev` observations. The versioned tags prevent one
framing or flushing policy from being silently reinterpreted as another.

This representation models generic compression-relevant properties of
columnar bulk transfer. It is not a database protocol capture, a migration wire
format, or an encoded data export. Sample bytes remain memory-only and are
discarded after aggregate measurements are derived.

### Accuracy bounds

`ratio_zstd_3` describes the named `sample_encoding`; it is not a capture of
database-protocol or migration-wire bytes. The test suite in this repository validates
deterministic encoding, bounded sampling, and serialization, but does not claim
a universal cross-engine percentage error against every extraction path.

Before using the ratio for a high-stakes capacity decision, check the ratio
against representative source data and the intended
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
explicitly marked inferred.

## `[artifact_inventory]` (since schema v4; required in v7)

Schema v7 uses the independently versioned
`dbwarp-blueprint-artifacts/v2` contract to describe non-table objects without
serializing source names or definitions. Older schema versions retain the v1
contract. V7 always emits this block: `--artifact-detail none` explicitly
records a not-requested database inventory, while structured-file sources emit
an explicit not-applicable inventory. A missing block is therefore never
mistaken for a verified empty catalog.

The default `--artifact-detail summary` emits `object_count`,
`external_prerequisite_count`, `counts_by_kind`, and
`counts_by_external_class`. `graph` additionally emits one anonymous object
record per artifact plus dependency edges. `analyzed` adds bounded
`dbwarp-language-feature-census/v1` records derived transiently from available
definitions. `graph` and `analyzed` require explicit `--yes` because graph
topology can fingerprint an application.

`object_count` is the number of artifact records emitted by the collector, not
the number of rows returned by any one native catalog. A package or type can
therefore contribute separate specification, body, and member records. A
native object that appears in more than one catalog is still one record: for
example, Oracle trigger rows from the trigger and source catalogs are joined by
their native object identity, and source rows enrich rather than duplicate the
trigger record.

Oracle packages and object types use the same record shape: a `specification`
record, a `body` record linked as its implementation, and one `package_member`
procedure/function record per catalog member with the specification as parent.
Only the body owns the combined source text and language census; members retain
their catalog facts but use non-applicable definition analysis. This prevents a
lexical analyzer from pretending it can split package source into member bodies.

Top-level inventory evidence includes:

| Field | Values / rule |
|---|---|
| `detail` | `none`, `summary`, `graph`, or `analyzed` |
| `scope` | V7: `all-visible-schemas`, `selected-schemas`, `structured-source`, or `unknown`; it must agree with schema-selection evidence elsewhere in the file |
| `visibility` | `full`, `privilege_filtered`, or `unknown` |
| `inventory_complete` | May be true only with full visibility, no unreadable catalogs, and no declared unmodeled families |
| `dependencies_complete` | May be true only when the modeled dependency catalogs were readable |
| `requirements_complete` | V7 aggregate: true only with complete assessment-population coverage for the selected scope and `requirement_status = complete | not_applicable` on every emitted artifact; omission means false, and an empty requirement list is not completeness evidence |
| `analysis_complete` | May be true only for analyzed detail and only when every emitted analysis is complete |
| `catalogs_read` | Closed, standard engine catalog labels successfully inspected |
| `catalogs_unreadable` | Catalog labels that failed; each entry prevents the completeness claims fed by that catalog, while unrelated per-object requirement evidence may remain complete |
| `catalogs_not_applicable` | V7 catalog labels proved inapplicable; disjoint from readable and unreadable catalogs |
| `families_not_inventoried` | Known object families not inventoried by this release |

### `[artifact_inventory.complexity]` (schema v7)

The `dbwarp-blueprint-artifact-complexity/v1` block is an
aggregate-only assessment over the anonymous artifact census. It is absent at
`none` and `summary` detail and required at `graph` and `analyzed` detail, where
presence means that assessment was attempted. A calculation failure produces a
fail-closed unknown result rather than aborting the Blueprint.

The top-level fields are fixed:

| Field | Values / rule |
|---|---|
| `contract` | `dbwarp-blueprint-artifact-complexity/v1` |
| `assessor_version` | `1` |
| `scope` | Must exactly equal `artifact_inventory.scope` |
| `population_policy` | `exclude-known-engine-generated-and-secondary`; missing flags remain eligible and temporary objects remain eligible |
| `assessment_population_complete` | True only when every object eligible under the population policy is known; omission means false, and this claim is independent of the broader `inventory_complete` field |
| `eligible_object_count` | Objects assessed by the policy |
| `fully_assessed_object_count` | Every dimension is either known or proved `not-applicable` |
| `partially_assessed_object_count` | At least one applicable dimension known and at least one unknown |
| `unassessed_object_count` | No applicable dimension known |
| `excluded_object_count` | Objects excluded by the recorded policy |
| `analyzer_version` | The single analyzer used by the v7 capture: `lexical-v2`, or `not-applicable` in graph mode |
| `analysis_spans` | Sorted unique closed spans present in eligible census records: `executable-body`, `not-applicable`, or `unknown`; empty in graph mode |
| `dialects` | Sorted unique closed dialect tokens present in eligible census records |
| `grammar_profiles` | Sorted unique grammar profiles present in eligible census records |
| `overall_band` | `trivial`, `low`, `moderate`, `high`, `very-high`, `not-applicable`, or `unknown` |
| `overall_score` | Not written by this release. |
| `limitations` | Sorted closed reasons described below |

Both population equations use checked arithmetic:

```text
artifact_inventory.object_count = eligible_object_count + excluded_object_count
eligible_object_count = fully_assessed_object_count
                      + partially_assessed_object_count
                      + unassessed_object_count
```

`dimensions` contains exactly `volume`, `control_flow`, `feature_breadth`,
`entanglement`, `environment_coupling`, `opacity`, and `dialect_coupling`.
Each dimension has a closed `band`, a `coverage` value (`complete`, `partial`,
`not-applicable`, or `unknown`), and one fixed histogram. The `volume`
dimension `band`, like the other dimension verdicts, uses `trivial`, `low`,
`moderate`, `high`, `very-high`, `not-applicable`, or `unknown`. Its histogram
uses the size keys `0`, `1-255`, `256-1k`, `1k-4k`, `4k-16k`, `16k-64k`, and
`64k+`. The other six histograms use the count keys `0`, `1`, `2-4`, `5-8`,
`9-16`, `17-32`, and `33+`. Every histogram also has
`not_applicable` and `unknown` buckets. For every dimension, checked arithmetic
requires:

```text
eligible_object_count = assessed evidence-band counts
                      + not_applicable
                      + unknown
```

Coverage is therefore per dimension, not a single estate-level bit. A
`not_applicable` census result is determined evidence and contributes to the
dimension's `not_applicable` bucket; it does not make the object unassessed.
The top-level fully/partially/unassessed object counts are a derived summary:
all-not-applicable objects are fully assessed, partial means at least one
applicable dimension is known and another is unknown, and unassessed means no
applicable dimension is known.

A partial dimension is evaluated as a lower and upper bound. Its `band` is
`unknown` unless the known lower bound is already `very-high`, because any
unknown observation may occupy the highest bucket. This prevents a partial
histogram from presenting its observed lower bound as a final verdict.

`external_binary` is a definition-visibility state, not an exclusion flag.
Site-installed plugins, CLR assemblies, Java objects, and external libraries
remain eligible migration work and normally contribute unknown
definition-dependent evidence. Only the explicit `generated_by_engine = true`
flag excludes an engine-provided object under assessor v1.

Histograms are deliberately one-dimensional. Cross-tabulations by kind,
feature, schema, or any other attribute are not part of the contract. Exact
counts add no information beyond the serialized per-object census in analyzed
mode, while the fixed shape avoids publishing an official estate-fingerprinting
aid.

Closed limitation reasons are `definition-analysis-not-requested`,
`definitions-withheld`, `unsupported-dialect`, `wrapped-source`,
`graph-incomplete`, `requirements-incomplete`, `outside-selected-scope`,
`computation-limit`, and `computation-failed`. `requirements-incomplete` means
requirement capture is not complete. Objects with
`partial` or `unavailable` requirement status contribute unknown rather than
zero environment- and dialect-coupling observations; independently complete
objects remain assessed.
`unsupported-dialect` means the definition was obtained
but its language or dialect has no supported analyzer; it is neither withheld
nor deliberately opaque. Graph detail uses
`definition-analysis-not-requested`; it must not claim a definition-read
limitation because no such read was attempted.

Unknown evidence is bounded independently for every affected dimension. The
overall band is emitted only when the lower and upper assessments agree. A
complete empty eligible population is `not-applicable`, never `trivial`.
Graph mode always uses an `unknown` overall band for a non-empty population,
because it does not read the definitions required by the overall assessment.
Missing graph edges make affected entanglement evidence unknown.
`computation-limit` is not written by this release. An unexpected calculation
failure records `computation-failed`, retains the full artifact inventory, and
marks the aggregate assessment fail-closed rather than suppressing the
Blueprint.

`assessment_population_complete`, rather than broad
`artifact_inventory.inventory_complete`, governs whether the bounded result
may be definitive. If the assessment population is incomplete, the overall
band is `unknown` unless the known lower bound is already `very-high`; no
finite upper bound is assumed for objects that may be invisible.

Entirely wrapped objects contribute `unknown` to opacity; they are not omitted
from the opacity histogram merely because no partial census could be produced.
Present opacity beside its coverage so a low observed
opaque-region band cannot hide a large unknown population.

`unsupported-dialect` remains a distinct limitation because the census can
name a dialect and report `unavailable`, but has no `unsupported` status. It is
derived only when a definition was available and the recorded dialect is not
supported by the named analyzer. Other definition limitations are likewise
derived from definition visibility, census status, and artifact evidence
rather than maintained as an independent assertion.

Comparison eligibility is computed between files from the complexity contract,
assessor version, analyzer version, exact analysis-span, dialect and
grammar-profile sets, scope, and population policy. A coarse homogeneous/mixed
flag is not serialized because different mixed sets are not necessarily
comparable.

Complexity is always source-local. A bundle preserves each child Blueprint's
assessment and never creates a bundle-level complexity band or histogram across
engines, analyzer versions, dialects, or grammar profiles.

Per-object ids have the form `<kind>-NNN`, such as `view-001`,
`package-002`, or `procedure-003`. V7 recognizes the common object families
plus Oracle packages, scheduler objects, database links, directories,
libraries, Java objects, operators, index types, domains, annotations, and
property graphs, as well as engine-neutral `queue` and `edition` kinds. The
three-digit minimum is zero-padded and each kind has its own dense ordinal set
starting at `001`; the width grows above 999. The
record contains only closed kind/subkind/tier tokens,
anonymous schema/parent ids, definition visibility/security mode, optional
validity and catalog flags, closed requirement coverage, an optional external
prerequisite, and optional language census. A parent may be an anonymous table or another artifact, so a
package-to-procedure hierarchy can be preserved without names; parent graphs
must be acyclic.

V7 uses one closed `subkind` vocabulary across all engines:

```text
ordinary, other, materialized, integer_sequence, stored_procedure,
stored_function, scalar_function, inline_table_function, table_function,
user_defined_aggregate, table_trigger, ddl_event_trigger, before_insert,
before_update, before_delete, after_insert, after_update, after_delete,
generated_column, column_default, default_constraint, check_constraint,
row_security, rewrite_rule, legacy_rule, enum, domain, composite, range,
alias_type, table_type, clr_type, clr_procedure, clr_scalar_function,
clr_table_function, clr_aggregate, clr_trigger, clr_assembly,
server_extension, loadable_udf, foreign_data_wrapper_server, foreign_table,
federated_table, external_table, external_data_source, external_file_format,
logical_replication_publication, logical_replication_subscription,
full_text_catalog, partition_scheme, partition_function, tablespace, filegroup,
database_certificate, symmetric_key, asymmetric_key, column_master_key,
column_encryption_key, database_scoped_credential, linked_server,
enabled_event, disabled_event, enabled_agent_job, disabled_agent_job,
database_synonym, specification, body, package_member, public, private,
java_source, java_class, java_resource, external_library,
user_defined_operator, domain_indextype, scheduler_job, scheduler_program,
scheduler_schedule, scheduler_chain, advanced_queuing, service_broker
```

V7 replaces the ambiguous v1 dependency list with sorted typed
`relationships`. Relationship kinds distinguish calls, reads, writes,
table/object references, trigger ownership, implementation, physical
placement, security, extension use, external binaries/services, and remote
database/server use. Every relationship records closed evidence
(`catalog-confirmed`, `dependency-confirmed`, `syntax-confirmed`,
`lexical-hint`, or `unresolved`). `dependency_edge_count` must exactly equal
the emitted graph.

`requirements` use closed engine-qualified tokens and a
bounded count band. They identify compatibility needs such as an Oracle wrapped
source, compound trigger, package state, dynamic SQL, autonomous transaction,
pipelined/parallel/aggregate routine, external library, database link, domain
index, scheduler, object/collection/spatial/vector type, Java object, or
property graph. They are planning evidence only.

Requirements come from a bounded catalogue fact or a dedicated engine-aware
syntax check. Generic lexical analysis never manufactures an engine-qualified
requirement. Every schema-v7 graph or analyzed artifact carries
`requirement_status = complete | partial | unavailable | not_applicable`.
`complete` establishes that the list is exhaustive for that artifact;
`not_applicable` establishes that the requirement model does not apply and
therefore forbids requirement and external-prerequisite records. `partial` and
`unavailable` make that object's environment- and dialect-coupling observations
unknown. `partial` means at least one fact source succeeded without
exhaustive coverage; `unavailable` means no requirement source established
usable coverage and therefore cannot carry known requirement or external
prerequisite evidence. Such evidence requires `partial`. This lets one
inaccessible object degrade locally instead of erasing useful coverage for the
rest of the estate.

The inventory-level `requirements_complete` is the aggregate claim. It can be
true only when every emitted artifact is `complete`
or `not_applicable` and the assessment population is complete for the selected
scope; it may remain false even when every artifact is `complete`. Never
interpret an empty
`requirements` array as zero coupling unless that artifact's status is
`complete`.

`unresolved_relationships` is a bounded reason-to-count map. It distinguishes
remote and cross-database references, selected-schema boundaries,
privilege-hidden targets, encrypted or withheld definitions, dynamic SQL,
ambiguous binding, missing or incomplete native identities, unmodeled target
families, and unknown cases. Complete dependency evidence requires this map to
be empty. Source object names, SQL text, principals, endpoints, credentials,
keys, certificates, and binaries are not fields in the contract.

External prerequisites record a closed `class`, deployment scope, whether
binary/secret/endpoint material is required but not captured, and a bounded
compatibility category. Their count is evidence for migration planning, not a
claim that DBWarp can automatically provision or translate them.

V7 language census records use `analyzer_version = "lexical-v2"` and record
`analysis_span`. The analyzer receives only the executable or declarative body,
excluding the outer creation wrapper, identity, signature, return declaration,
and module options. Header facts remain catalog requirements or flags. A
collector that cannot safely isolate the body records `analysis_span =
"unknown"` and unavailable evidence instead of analyzing the wrapper. A proven
non-applicable definition uses `analysis_span = "not-applicable"`. Omitted span
evidence is interpreted conservatively as `unknown`; it is never inferred from
the engine or object kind. A supported definition analyzed by this lexical
implementation uses `status = "partial"`;
missing or unsupported definition evidence uses `unavailable`, and a proven
non-applicable object may use `not_applicable`. Count, size, nesting,
complexity, and opaque-region values are bands, not exact source fingerprints.
Features are selected from a closed vocabulary. The analyzer removes comments,
literals, and quoted identifiers; it is not a parser, semantic binder, or
translation-success guarantee.

Wrapped PL/SQL is never executable-body evidence. The collector marks it encrypted
and withholds its bytes from analysis; the shared analyzer also refuses a PL/SQL
unit whose header contains the wrapped marker, preventing a classification mistake
from producing plausible but false census bands.

See [Non-Table Artifact Inventory](docs/ARTIFACT_INVENTORY.md) for operational
guidance and engine coverage.

## Steganography defenses, by vector

| Vector | Defeat |
|---|---|
| Identifier ordering | Domain-separated HMAC-SHA256 with a secret process-local key prevents offline candidate-name checks. Reuse a key you hold only when stable cross-run labels are required. |
| Numeric low-bits | Statistics are rounded to documented precision by default. Exact-length mode is explicit, consent-gated, recorded in the audit log, and must be handled as more sensitive metadata. |
| Sub-second timestamp | One UTC timestamp at the top, seconds resolution only |
| TOML formatting | Canonical: fixed key order, fixed indentation, and only the fixed header/producer comments; no input-derived comments |
| Sampling randomness | Sampling uses fixed seeds (PG's deterministic `TABLESAMPLE SYSTEM`). Separately, identifier anonymization intentionally obtains a secret key from the operating-system CSPRNG unless you supply one. |
| Unused fields | Every field is documented above; no "metadata"/"comment"/"reserved" fields that carry unbounded data |
| Artifact source text and external material | Definitions are transient and zeroized after bounded analysis; names, SQL text, endpoints, provider strings, credentials, keys, certificates, package names, and binaries have no serialized field |

## Schema-version compatibility

Current producers emit schema version 7. Versions 1 through 6 remain accepted
for backward compatibility. A v1/v2 file has no distribution blocks. A v3 file has distribution metadata but no
artifact inventory. A v4 file may contain an artifact inventory but predates
the current Blueprint contract identifiers. Readers normalize former v4
identifiers on input and re-emit that document with canonical Blueprint
identifiers. A v5 file predates the topology and dataset-scope evidence
added in v6. V6 uses topology contract v1, artifact contract v1, combined table
kind/partition fields, unsigned decimal scale, and optional statistics
freshness. V7 uses topology contract v2 and artifact contract v2, requires
explicit structure/environment/statistics evidence, separates orthogonal table
semantics, supports signed decimal scale and Oracle numeric models, and requires
an explicit artifact-inventory state. It also reserves the fixed
`dbwarp-blueprint-artifact-complexity/v1` aggregate contract without adding a
per-object score or cross-tabulation field. Readers reject unknown
future schema versions with a clear upgrade message rather than silently
discarding fields. Readers apply the same strict v7 validation to standalone
and embedded Blueprints rather than rewriting invalid evidence during parsing.

## Why TOML and not JSON

- TOML separates structural sections from leaf data more readably
  (`[tables.table-001.cols.col-2]` vs. nested JSON).
- Easier to diff (one key per line; identifier-based sub-tables stay
  contiguous).
- You can hand-edit the file to redact a specific field before
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

When `engine` or `source_kind` is `"parquet"` or `"avro"`, schema version 3 or
newer may also emit the following bounded fields. Readers must preserve the
distinction between source-file storage and bounded decoded-sample
measurements; readers that do not support the document's schema version must
reject it with an upgrade message rather than discard unknown fields.

V7 structured-file Blueprints require complete `[structure_scope]` and
`[statistics_evidence]` blocks, omit `[database_topology]`,
`[source_environment]`, and `[activity_snapshot]`, and emit an explicit
not-applicable `[artifact_inventory]`. They never infer database topology or
server-machine capacity from the collector host.

Structured-file Blueprints use the same anonymized identifiers as database
Blueprints: `table-NNN` in secret-keyed order and `col-N` in schema ordinal order.
Source file stems, Parquet paths, Avro field names, and a manifest's
`logical_table` label are not emitted as table or column identifiers.

At table scope, `table_bytes` is the logical transfer-sizing estimate, whereas
`storage_bytes` is the actual source-object size on disk. Metadata-only Parquet
uses uncompressed column-chunk bytes for `table_bytes`; optional decoded sampling
replaces that estimate with projected `blueprint-compression-probe-v2` bytes. Avro
derives it from its decoded full scan. The optional `source_partitions`,
`row_group_count`, and `source_codec` fields describe file layout. Multi-file datasets aggregate these values. `row_group_count` is
Parquet-specific; `source_partitions` is `1` for a single input object.

At column scope, `null_fraction` is an observed value from `0.0` through `1.0`.
`length_sample_rows` and `length_sample_method` state how `len_avg` and
`len_p95` were obtained. `source_semantics` records bounded compatibility facts
such as `"repeated-leaf"`, `"nested-json"`, or `"multi-type-union"`; it never
contains your field names or values. Decimal precision/scale, timestamp
precision and UTC/local semantics, UUID, and fixed-size binary metadata are
carried by the existing sanitized scalar fields and `native_type`.

At compression scope, table-level `ratio_storage` compares `table_bytes` with
actual source-object bytes. A Parquet column-level value compares the footer's
uncompressed and compressed column-chunk bytes. Both are file-storage planning
signals, not decoded-sample estimates. `ratio_zstd_3` and
`ratio_zstd_19` are comparable only when
`sample_encoding` is the recognized `"blueprint-compression-probe-v2"` value. A
Parquet footer ratio or Avro container ratio must never be copied into those
zstd fields.
