# Changelog

Release versions identify the collector. The Blueprint schema version and the
compression sample encoding are separate compatibility contracts; see
[FORMAT.md](FORMAT.md) and [compression measurement](docs/COMPRESSION_MEASUREMENT.md).

## 1.6.0

### Blueprint schema v7

- Emit schema v7 with explicit table classification, storage organization,
  partitioning, segment state, column ordinals and richer type, nullability,
  generated-value and identity semantics.
- Add per-table and capture-wide evidence for structure, row counts, allocated
  bytes, statistics and source-environment observations. Missing, denied,
  malformed and selection-limited evidence remains explicit instead of being
  represented as a measured zero or a complete inventory.
- Add catalog-backed per-object requirement status and bounded complexity
  evidence to the artifact inventory. Aggregate completeness remains false
  when a required catalog or the selected assessment population cannot be
  proved complete.
- Preserve schema-v1 through schema-v6 input compatibility. Earlier releases
  may not read schema-v7 files.

### Capture fidelity and safety

- Keep complete bounded reads, partial samples and catalog estimates distinct
  across PostgreSQL, MySQL and SQL Server, including row-security and
  inheritance boundaries, adaptive MySQL range windows and SQL Server
  memory-optimized tables.
- Tighten cardinality, null-count, partition, relationship and aggregate
  consistency so rounded or incomplete evidence cannot become an exact claim.
- Report SQL Server external-table coverage as an explicit limitation when
  PolyBase is not installed.
- Keep collection bounded by row, byte, value and deadline limits. Non-fatal
  degradation remains visible in the Blueprint and audit with stable message
  codes.

### Oracle scope boundary

- Add Oracle Basic catalogue capture for Oracle 12c, 19c, 21c and 23ai/26ai as
  a preview: acknowledgement-gated, catalogue only, and it reads no table rows.
  See `sql/grants/ORACLE_PREVIEW.md` for limitations.

### Release artifacts and authentication

- Linux release archives include SQL Server Kerberos/GSSAPI authentication.
  They load the platform Kerberos runtime only when integrated authentication
  is selected, so the collector starts without Kerberos libraries; a missing
  runtime is reported with `DBP1604E` only for integrated authentication.
  Windows release binaries continue to include SQL Server SSPI authentication.
- Both integrated modes use the operating-system credential and fail closed
  when the server principal does not match the expected one.

### Operation and compatibility

- The SQL Server enhanced grant scripts add one server-level permission in a
  separate batch: `VIEW SERVER STATE` on SQL Server 2019, `VIEW SERVER
  PERFORMANCE STATE` on 2022 and 2025. It lets an enhanced capture with
  `--artifact-detail graph` or `analyzed` report coarse CPU and memory bands for
  a self-managed server. Basic and standard do not grant it and report those
  bands as unknown; a DBA can remove the batch to keep enhanced without it.
- Update the SQL Server, PostgreSQL, Parquet and Windows authentication
  dependency stack to releases that fix published security advisories. The SQL
  Server driver moves to 0.13; `--tls-ca` keeps its restrictive meaning and
  trusts only the supplied CA. `--max-wall-secs` remains the single deadline
  for the whole capture.
- SQL Server reads operating-system capacity only when non-table object
  analysis is requested (`--artifact-detail graph` or `analyzed`). Other
  captures record the capacity bands as not requested instead of a failed
  read.
- English documentation remains authoritative. Translated Markdown is
  supplemental and carries its own translation notice.

### Deck

- Add a non-table objects slide and a separate artifact complexity slide to
  the deck. The complexity slide appears only when complexity was captured and
  shows coverage beside every band, so incomplete evidence is never shown as
  low complexity.

## 1.5.1

### Capture and fidelity

- Preserve Blueprint schema v6 and distinguish catalog estimates, sampled
  observations and unavailable statistics-freshness evidence.
- Improve binary payload measurement, precompressed-payload profiling and
  bounded compression probes. Ratios from different `sample_encoding` values
  are not interchangeable; check the encoding before comparing ratios.
- Bound adaptive MySQL and SQL Server sampling retries, including oversized
  values and character-set expansion. Record remaining prefix bias while
  retaining original sampled-value length metadata.
- Correct MySQL truncation detection when the connection character set changes
  the returned byte length.

### Operation and review

- Clarify dedicated least-privilege account setup, source-build instructions,
  comparison-build verification and the supported database versions.
- Refresh machine-translated documentation and runtime wording, retaining
  English as authoritative and translations as supplemental.
- Add release history, support and contribution guidance to source and binary
  distributions.

### Compatibility

Existing Blueprints remain readable by the new collector. The reverse is not
guaranteed: the 1.5.0 reader rejects the new optional `sample_layout` field,
and older releases may reject the new compression sample encodings. Use the
same release to produce a file and to build a deck from it.

Pin the exact release artifact and checksum. Results from one release are not
guaranteed to match another.

## 1.5.0

The preceding release provides schema-v6 capture for PostgreSQL, MySQL and SQL
Server, structured-file inspection, local Blueprint and deck output, and
version-aware grant scripts. See the
[release tag](https://github.com/DBWarp/dbwarp-blueprint/releases/tag/v1.5.0)
for its exact source and artifacts.

For problem reports, see [SUPPORT.md](SUPPORT.md). For contribution guidance,
see [CONTRIBUTING.md](CONTRIBUTING.md).
