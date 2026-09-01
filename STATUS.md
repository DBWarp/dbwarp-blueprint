# Current capabilities

This file summarizes the customer-visible state of `dbwarp-blueprint`.

## Databases

| Qualified database versions | Catalog Blueprint | Optional compression measurement | TLS |
|---|---:|---:|---:|
| PostgreSQL 13-18 | yes | yes | yes |
| MySQL 8.0 / 8.4 / 9.7 | yes | yes | yes |
| SQL Server 2019 / 2022 / 2025 | yes | yes | yes |

## Authentication

| Database | Supported modes |
|---|---|
| PostgreSQL | username/password, password file, named password env, externally generated managed-service token, TLS/mTLS |
| MySQL | username/password, password file, named password env, externally generated managed-service token, TLS/mTLS |
| SQL Server | username/password, Entra ID token file/env, Kerberos/GSSAPI build, Windows SSPI build, TLS (client-certificate mTLS is not implemented) |

## Output

Live database runs write a TOML Blueprint file with anonymized identifiers and rounded statistics. See [`FORMAT.md`](FORMAT.md).

Version 1.5 emits Blueprint schema v6. Schema v3 introduced bounded, rounded
cardinality and skew summaries, index-prefix selectivity, and relationship
fanout/coverage provenance when bounded sampling is explicitly enabled. Schema
v4 introduced the non-table artifact inventory. Schema v5 establishes the
Blueprint technical identifiers while preserving the same bounded physical
metadata model. Schema v6 adds bounded, name-free deployment topology and explicit
dataset-scope completeness, so local endpoint statistics cannot silently pose
as complete sharded-dataset totals. Existing schema-v1 through schema-v5 files
remain readable; missing topology evidence remains unknown rather than being
invented during normalization. These fields omit sampled values, but distinctive
structure and distributions can still fingerprint a workload; review every
Blueprint before sharing it.

With `--deck blueprint.pptx` a live run also writes an optional PowerPoint summary of the same Blueprint. With `--from-toml blueprint.toml --deck blueprint.pptx`, the binary builds that same deck later from an existing reviewed Blueprint file, without connecting to a database. See [`DECK.md`](DECK.md).

## Languages

Human-facing help, prompts, diagnostics, progress, and deck prose support
English, German, French, Spanish, Polish, Japanese, and Simplified Chinese.
Operational syntax and generated artifacts remain language-neutral canonical
English. Embedded catalogs are exact-coverage checked at startup and in tests;
there is no silent English fallback for an advertised locale. See
[`docs/INTERNATIONALISATION.md`](docs/INTERNATIONALISATION.md).

English customer documentation is authoritative. Machine-translated document
sets may be offered separately only after multiple independent reviews against
the same English revision. They remain explicitly non-authoritative and may
contain errors. See [`docs/TRANSLATIONS.md`](docs/TRANSLATIONS.md).

## Build and downloads

- Download binaries from <https://github.com/DBWarp/dbwarp-blueprint/releases>
- Build from source with [`BUILD.md`](BUILD.md)
- Review security model in [`SECURITY.md`](SECURITY.md)
