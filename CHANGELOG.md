# Changelog

Release versions identify the collector. The Blueprint schema version and the
compression sample encoding are separate compatibility contracts; see
[FORMAT.md](FORMAT.md) and [compression measurement](docs/COMPRESSION_MEASUREMENT.md).

## 1.5.1

### Capture and fidelity

- Preserve Blueprint schema v6 and distinguish catalog estimates, sampled
  observations and unavailable statistics-freshness evidence.
- Improve binary payload measurement, precompressed-payload profiling and
  bounded compression probes. Ratios from different `sample_encoding` values
  are not interchangeable; consumers must recognize the encoding before using
  a measurement.
- Bound adaptive MySQL and SQL Server sampling retries, including oversized
  values and character-set expansion. Record remaining prefix bias while
  retaining original sampled-value length metadata.
- Correct MySQL truncation detection when the connection character set changes
  the returned byte length.
- Improve synthetic value-length, cardinality, distribution and locality
  handling in the shared Blueprint core.

### Operation and review

- Clarify dedicated least-privilege account setup, source-build instructions,
  comparison-build verification and the qualified database version matrix.
- Refresh machine-translated documentation and runtime wording, retaining
  English as authoritative and translations as supplemental.
- Harden public-source executable-mode and release-archive checks.
- Add release history, support and contribution guidance to source and binary
  distributions.
- Remove unused ASCII artwork without changing supported banner modes.

### Compatibility and rollout

Existing Blueprints remain readable by the new collector. The reverse is not
guaranteed: the 1.5.0 reader rejects the new optional `sample_layout` field,
and older consumers may reject the new compression sample encodings. A
schema-v6 parser alone is therefore not proof of forward compatibility.
Upgrade and validate the consuming tools together with the collector before
using new captures for decks, generation or compression-based planning.

Pin the exact release artifact and checksum. Prior-release qualification is
not evidence that a different binary produces identical results.

## 1.5.0

The preceding release provides schema-v6 capture for PostgreSQL, MySQL and SQL
Server, structured-file inspection, local Blueprint and deck output, and
version-aware grant scripts. See the
[release tag](https://github.com/DBWarp/dbwarp-blueprint/releases/tag/v1.5.0)
for its exact source and artifacts.

For problem reports, see [SUPPORT.md](SUPPORT.md). For contribution guidance,
see [CONTRIBUTING.md](CONTRIBUTING.md).
