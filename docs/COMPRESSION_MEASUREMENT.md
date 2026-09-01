# Compression Measurement

`dbwarp-blueprint` can optionally measure how well representative table data compresses. This makes DBWarp estimates more accurate because WAN transfer time and egress cost depend on compressed bytes, not raw table size.

Compression measurement is opt-in and requires explicit consent. Interactive
live runs may accept the preflight prompt; unattended and structured-file runs
use:

```bash
--measure-compression --yes
```

Without those flags, the tool reads catalog metadata only.

## What Is Sampled

For each eligible user table that is not safely proven empty, the tool reads a
bounded number of rows into memory, encodes them into a stable row-frame
buffer, compresses that buffer locally with zstd level 3, and derives aggregate
compression, null-density, cardinality/frequency, length, and style
measurements before discarding sampled values and temporary fingerprints.

For selected text/binary columns, Tier 2 may also sample that column alone. This
lets downstream planning tools match per-column entropy instead of relying only
on table-level averages.

Each measurement is an independent one-shot zstd frame with the input size pledged. Ratio variance (`ratio_stddev`) is measured over row-aligned 64 KiB chunks of the same buffer, so the variance describes the transfer the estimator predicts rather than a single whole-buffer average. Because the input size is pledged, zstd selects size-adapted parameters consistent with how the estimator models the transfer. On small samples (under roughly 1 MiB) this can shift ratios noticeably compared with captures from earlier releases that measured through an unpledged streaming context, so small-table ratios are not directly comparable across that boundary; the pledged measurement is the one that matches the transfer.

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
sample_method = "column LIMIT N (engine-specific bounded sample)"
sampled_with_bias = true
bias_reason = "unordered_limit_after_empty_TABLESAMPLE"
ratio_zstd_3 = 12.35
ratio_stddev = 0.2
sample_encoding = "dbwarp-blueprint-rowframe-v1"

[tables.table-001.compression]
measured = true
sample_rows = 1000
sample_bytes = 1048576
sample_method = "LIMIT N (engine-specific bounded sample)"
sampled_with_bias = false
ratio_zstd_3 = 4.35
ratio_stddev = 0.15
sample_encoding = "dbwarp-blueprint-rowframe-v1"
```

These values help approved downstream tools estimate network transfer size and
generate synthetic text/binary data with similar compressibility.

## Why It Matters

Two databases with the same raw table size can behave very differently during migration:

- JSON, XML, repeated business codes, sparse text, and natural-language text often compress well.
- Encrypted values, already-compressed blobs, random tokens, and high-entropy binary do not.
- SQL Server `nvarchar` data has a different byte distribution than UTF-8 text and is encoded accordingly for sampling.

A small local measurement is usually more useful than guessing from column types.

## Bias and Transparency

Some engines do not offer perfectly uniform table sampling. When the tool falls back to a less ideal method, the Blueprint file marks it with `sampled_with_bias` and `bias_reason`.

Biased samples are still useful, but downstream tools should treat them with
lower confidence. The audit log records that row sampling was enabled and the
locally encoded row-frame byte count. Database wire-byte totals are reported as
`unknown` when the driver does not expose them.

## Practical Sampling Settings

First production-safe pass:

```bash
--measure-compression --yes \
--sample-rows 500 \
--max-wall-secs 120
```

Better estimator input when a read replica or maintenance window is available:

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
ceiling per table. The SQL projection truncates variable-width cells on the
server and reduces the row limit for exceptionally wide tables before the
driver receives data. Consequently, very large LOB values contribute bounded
prefixes rather than their full contents. The audit records the active table
payload ceiling and the exact locally encoded row-frame byte total.

## How Downstream Consumers Use It

A downstream consumer should use compression evidence in this order:

1. recognized per-column compression blocks;
2. recognized table-level compression blocks;
3. type/style defaults when no measured ratio exists.

The `sample_encoding` field is part of the contract. Consumers should only use ratios with a recognized encoding tag, because different sample encodings can produce different compression ratios for the same logical data.
