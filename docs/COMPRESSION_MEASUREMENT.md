# Compression Measurement

`dbwarp-blueprint` can optionally measure how well representative table data compresses. This makes DBWarp estimates more accurate because WAN transfer time and egress cost depend on compressed bytes, not raw table size.

Compression measurement is opt-in and requires explicit consent. Interactive
live runs may accept the preflight prompt; unattended and structured-file runs
use:

```bash
--measure-compression --yes
```

When compression measurement is disabled, live database capture does not sample
user-table row values. Structured-file behavior differs: Avro records must still
be walked to collect row counts, lengths and null metadata; see
[Structured Files](STRUCTURED_FILES.md).

## What Is Sampled

For each eligible user table that is not safely proven empty, the tool reads a
bounded number of rows into memory, encodes them into stable transient probe
buffers, compresses those buffers locally with zstd level 3, and derives aggregate
compression, null-density, cardinality/frequency, length, and style
measurements before discarding sampled values and temporary fingerprints.

For selected text/binary columns, Tier 2 may also sample that column alone. This
gives per-column compressibility instead of only table-level averages.

Live-database table ratios use a neutral sequence of bounded 1,000-row
groups with one descriptor per column, fixed-width value lengths, and
column-contiguous payloads. This measures compression-relevant structure
without capturing any database or transfer protocol. Per-column ratios retain `blueprint-compression-probe-v2`, whose
tagged length-prefixed values remain the more specific entropy input.

PostgreSQL table blocks use `blueprint-columnar-transfer-probe-v2`,
which passes row groups through one persistent zstd level-3 context and flushes
after every group. MySQL and SQL Server use
`blueprint-columnar-transfer-probe-v3`: the same neutral bytes and persistent
context, with additional flushes at 256 KiB probe chunk boundaries.
SQL Server `nvarchar`, `nchar`, and `ntext` samples are measured as
UTF-16LE byte distributions. SQL Server `varchar`, `char`, and `text` retain their
sampled narrow-byte width; the Blueprint records the source collation's catalog
code page as `utf-8`, `windows-N`, or `code-page-N`. The database
driver still exposes decoded strings to the sampler, so legacy-code-page byte
identity is not claimed. Table `ratio_stddev` is measured across outer row-group
outputs. Per-column
projection blocks remain independent one-shot entropy measurements and emit
`0.0`. Blueprints from earlier versions may carry
`blueprint-columnar-transfer-probe-v1`; ratios with different tags are not
comparable.

The sampled bytes travel only over the selected database session into the local
process. They are not written to disk, included in `blueprint.toml`, included
in the audit log, uploaded, or sent to DBWarp infrastructure.

## Local Worker Concurrency

Database sampling always uses one sequential connection. The optional
`--compression-workers N` setting parallelizes only local compression of
already-read, in-memory samples. It accepts 1–32 workers and defaults to 1 to
minimize source-host impact. Increase it explicitly to use more local CPU:

```bash
--measure-compression --yes \
--compression-workers 4
```

Higher values can reduce elapsed time when zstd is the bottleneck, but they
increase local CPU and peak memory. They do not create concurrent database
sampling connections. Each worker owns its zstd contexts and the input queue
is bounded to the worker count. Worker count does not change the measurements.
Anonymous label ordering intentionally varies with the default fresh key; reuse
a protected `--anonymization-key-file` only for approved cross-run comparisons.

The collector avoids row and style queries only when an engine-maintained
catalog value safely proves a table empty at catalog-read time. PostgreSQL
requires fresh analyzed statistics with no subsequent modifications; SQL
Server uses its partition row counter. MySQL table-row estimates can report
zero for a non-empty table, so the collector does not use them to skip
sampling. This conservative difference protects fidelity.

## What Appears in the Blueprint File

Only aggregate summaries are emitted. For text-like columns, the Tier 2 pass may emit a bounded style label such as `json`, `xml`, `natural-text`, `base64`, `hex`, `numeric-text`, or `mixed`.

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
sample_method = "LIMIT N (fallback after underfilled adaptive TABLESAMPLE; simple-query text fields; raw binary/vector decoded; server-side cell cap)"
sampled_with_bias = true
bias_reason = "unordered_limit_after_underfilled_adaptive_TABLESAMPLE+server_side_cell_cap"
ratio_zstd_3 = 12.35
ratio_stddev = 0.2
sample_encoding = "blueprint-compression-probe-v2"

[tables.table-001.compression]
measured = true
sample_rows = 1000
sample_bytes = 1048576
sample_method = "LIMIT N (engine-specific bounded sample)"
sampled_with_bias = false
ratio_zstd_3 = 4.35
ratio_stddev = 0.15
sample_encoding = "blueprint-columnar-transfer-probe-v3"
```

These values are used to estimate network transfer size.

## Why It Matters

Two databases with the same raw table size can behave very differently during migration:

- JSON, XML, repeated business codes, sparse text, and natural-language text often compress well.
- Encrypted values, already-compressed blobs, random tokens, and high-entropy binary do not.
- SQL Server Unicode and narrow text have different byte distributions. The sampler models `nvarchar` as UTF-16LE and records the collation code page needed to interpret `varchar` without treating every text column as UTF-8.

A small local measurement is usually more useful than guessing from column types.

## Bias and Transparency

Some engines do not offer perfectly uniform table sampling. MySQL spreads a
bounded sample over four numeric-primary-key ranges when that access path is
available, and otherwise falls back to `LIMIT N`; both remain explicitly
marked as biased because neither is a statistical random sample. Non-final
range windows have an exclusive upper bound. Sparse or skewed primary-key
regions can therefore underfill a window, but one window cannot re-read rows
from the next. Other less ideal engine fallbacks are likewise recorded through
`sampled_with_bias` and `bias_reason`.

Blueprint records the layout of a bounded sample separately from those prose
fields. MySQL numeric primary-key range sampling emits
`sample_layout = "primary-key-range-windows"` and orders every window by the
complete primary key.

Biased samples are still useful, but they are lower-confidence. The audit log records that row sampling was enabled and the
locally encoded probe byte count. Database session byte totals are reported as
`unknown` when the driver does not expose them.

## Practical Sampling Settings

First production-safe pass:

```bash
--measure-compression --yes \
--sample-rows 500 \
--max-wall-secs 120
```

More accurate measurement when a read replica or maintenance window is available:

```bash
--measure-compression --yes \
--sample-rows 1000 \
--max-wall-secs 300
```

Large databases do not require huge samples. The goal is a stable compression
signal, not exact row-level profiling. `--max-wall-secs` is a hard deadline for
the entire live capture, including connection setup, catalogs, RTT probes, and
sampling; it is not a fresh budget for each phase.

Live database sampling also has a non-configurable 16 MiB projected payload
ceiling per table. The initial SQL projection is type-budgeted and observes
original octet lengths separately. When a projected value was narrowed, MySQL
and SQL Server can retry with fewer rows and revised per-column limits that
still fit the budget. Values too wide for that budget remain bounded prefixes;
compression and value-summary provenance records this limitation, while length
statistics retain the original server-reported sampled-value lengths under the
selected length-fidelity policy.

The ceiling is not a network-byte or process-memory limit. Protocol encoding,
original-length metadata, retries and driver buffers add overhead. The audit
records the configured payload ceiling, performed queries and the exact locally
encoded probe byte total; it does not report measured database wire traffic.

## How The Measurements Are Interpreted

The `sample_encoding` field is part of the contract. Ratios are comparable only
within one encoding tag, because different sample encodings can produce
different compression ratios for the same logical data. In particular, the
table-level columnar transfer-probe ratio and the per-column v2 ratios are
complementary measurements and must not be substituted for one another.
