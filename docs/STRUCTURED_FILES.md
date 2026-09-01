# Structured File Blueprint Sources

`dbwarp-blueprint` can build a bounded, anonymized Blueprint TOML from local
Parquet and Avro inputs when the source is already a file rather than a live
database.

This is an offline mode:

- no database connection;
- no credentials;
- no telemetry;
- no row values written to the output;
- table and column identifiers are emitted only as `table-NNN` and `col-N`;
- the audit records local input/output paths, the output hash, and normal
  operational evidence such as mode, timing, sampling work, and warnings; it
  records no database endpoint in this mode.

## Parquet

```bash
dbwarp-blueprint \
  --from-parquet /data/customer-sample.parquet \
  --out blueprint.toml \
  --audit-log audit.txt
```

Parquet mode reads footer and row-group metadata. It derives:

- row count from file metadata;
- column type labels from Parquet physical/logical types;
- nullability from definition levels;
- observed null fractions when complete column statistics are available;
- coarse encoded average width and per-column source-storage ratio from column-chunk metadata;
- source object bytes, row-group count, partition count, and codec provenance.

Metadata-only Parquet capture does not invent a decoded p95 width. Optional
decoded sampling replaces encoded width hints with decoded `len_avg`, `len_p95`,
`null_fraction`, and logical `table_bytes` observations.

Metadata-only Parquet uses uncompressed column-chunk bytes as the logical
`table_bytes` sizing estimate. Table-level `ratio_storage` compares that value
with the actual object size; per-column `ratio_storage` compares uncompressed
and compressed column chunks. These are file-planning signals, not DBWarp
transport compression, and are never emitted as `ratio_zstd_3`.

## Avro

```bash
dbwarp-blueprint \
  --from-avro /data/customer-sample.avro \
  --out blueprint.toml \
  --audit-log audit.txt
```

Avro object containers do not expose a Parquet-style footer row count. Avro mode
therefore walks the container once to count records, derive logical
`table_bytes`, and observe per-column `len_avg`, `len_p95`, and `null_fraction`.
The writer schema supplies the logical type metadata. `storage_bytes` and
`ratio_storage` describe the Avro container, not a DBWarp transfer estimate.
This is suitable for estimator and synthetic-fixture planning.

## Logical Type Fidelity

Structured-file capture preserves bounded logical metadata needed by the
estimator: decimal precision/scale, date and time families, timestamp precision
and UTC/local semantics, UUIDs, fixed-size binary width, UTF-8 strings, and raw
bytes. Null-only fields remain `type = "null"` rather than becoming synthetic
text.

Nested Parquet leaves and Avro arrays, maps, records, or multi-type unions cannot
be represented as one exact SQL scalar. The Blueprint records a normalized `json`
type plus `source_semantics` such as `"repeated-leaf"`, `"nested-json"`, or
`"multi-type-union"`. Downstream generators must identify those values as
representative JSON pressure, not claim an exact nested-schema round trip.

Source file stems, Parquet paths, Avro field names, and batch `logical_table`
labels are not written as Blueprint identifiers. A multi-file dataset emits
secret-keyed `table-NNN` identifiers, aggregates object bytes, partitions, row
groups, codecs, widths, null rates, and compatible compression provenance, and
rejects files whose structured logical column contracts differ.

## Decoded Compression Sampling

Structured file mode supports optional decoded compression sampling:

```bash
dbwarp-blueprint \
  --from-parquet /data/customer-sample.parquet \
  --measure-compression --yes \
  --sample-rows 5000 \
  --out blueprint.toml \
  --audit-log audit.txt
```

The same flags work with `--from-avro`.

When enabled, `dbwarp-blueprint`:

- decodes up to `--sample-rows` records from the file;
- encodes sampled values using the same `dbwarp-blueprint-rowframe-v1` rowframe used by live database Blueprint capture;
- emits table-level and per-column zstd-3 compression summaries;
- records `sample_encoding = "dbwarp-blueprint-rowframe-v1"` in the generated TOML;
- keeps sampled bytes in memory only and never writes row values to disk.

`--measure-compression` requires `--yes` because it reads decoded customer
values. It persists aggregate compression, null-density,
cardinality/frequency, length, and style measurements, never sampled values.

The current sampler uses a deterministic first-N sample. That is reproducible and cheap, but it can be biased if a file is sorted or clustered. For high-stakes estimates, prefer a representative file or generate multiple Blueprint files from different shards. A future version may add row-group/block-stratified sampling.

## Scope

Structured-file Blueprint mode is useful for:

- sizing a Parquet/Avro import before a DBWarp run;
- generating a representative synthetic fixture without copying source names
  or row values;
- planning Parquet/Avro -> DBWarp columnar -> target database flows.

It is not a replacement for live database Blueprint capture when the real source is a supported database, meaning PostgreSQL, MySQL, or SQL Server. A database catalog has index, key, FK, statistics-freshness, and engine-layout details that are not present in generic file metadata.
