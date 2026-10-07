# Current capabilities

This file summarizes what `dbwarp-blueprint` supports.

## Databases

| Supported database versions | Catalog Blueprint | Optional compression measurement | TLS |
|---|---:|---:|---:|
| PostgreSQL 13-18 | yes | yes | yes |
| MySQL 8.0 / 8.4 / 9.7 | yes | yes | yes |
| SQL Server 2019 / 2022 / 2025 | yes | yes | yes |

## Authentication

| Database | Supported modes |
|---|---|
| PostgreSQL | username/password, password file, named password env, externally generated managed-service token, TLS/mTLS |
| MySQL | username/password, password file, named password env, externally generated managed-service token, TLS/mTLS |
| SQL Server | username/password, Entra ID token file/env, lazily loaded Kerberos/GSSAPI in Linux release archives, Windows SSPI in the Windows release build, TLS (client-certificate mTLS is not implemented) |

## Output

Live database runs write a TOML Blueprint file with anonymized identifiers and rounded statistics. See [`FORMAT.md`](FORMAT.md).

Version 1.6 emits Blueprint schema v7. Schema v7 adds explicit table kind,
storage organization, partitioning and segment state; stable column ordinals
and richer type semantics; and per-object evidence for structure, row counts,
allocated bytes and statistics. Artifact records carry catalog-backed
requirement status and bounded complexity evidence. Capture-wide completeness
is earned from the emitted population and the source catalogs that were
actually read, so a denied, malformed or selection-limited observation cannot
silently become a measured zero or a complete inventory.

Schema v3 introduced bounded cardinality and relationship statistics, schema
v4 the non-table artifact inventory, and schema v6 bounded deployment topology
and dataset scope.
Schema-v1 through schema-v6 files remain readable. Older tools may reject
schema-v7 output; update and validate every tool that reads a Blueprint.
Blueprint fields omit sampled values, but distinctive structure
and distributions can still fingerprint a workload, so review every Blueprint
before sharing it.

Oracle Basic catalogue capture for Oracle 12c, 19c, 21c and 23ai/26ai is
supported as a preview. It must be acknowledged explicitly and reads catalogue
data only; limitations are in `sql/grants/ORACLE_PREVIEW.md`.

With `--deck blueprint.pptx` a live run also writes an optional PowerPoint summary of the same Blueprint. With `--from-toml blueprint.toml --deck blueprint.pptx`, the binary builds that same deck later from an existing reviewed Blueprint file, without connecting to a database. See [`DECK.md`](DECK.md).

## Languages

Human-facing help, prompts, diagnostics, progress, and deck prose support
English, German, French, Spanish, Polish, Japanese, and Simplified Chinese.
Operational syntax and generated artifacts remain language-neutral canonical
English. Embedded catalogs are exact-coverage checked at startup and in tests;
there is no silent English fallback for an advertised locale. See
[`docs/INTERNATIONALISATION.md`](docs/INTERNATIONALISATION.md).

English documentation is authoritative. See
[`MACHINE_TRANSLATIONS.md`](MACHINE_TRANSLATIONS.md) for translation limitations.

## Build and downloads

- Download binaries from <https://github.com/DBWarp/dbwarp-blueprint/releases>
- Build from source with [`BUILD.md`](BUILD.md)
- Review security model in [`SECURITY.md`](SECURITY.md)
