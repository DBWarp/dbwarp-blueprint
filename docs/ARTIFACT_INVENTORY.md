# Non-Table Artifact Inventory

**Language:** English is authoritative. Machine-translated editions, when
available, are non-authoritative and may contain errors. See
[Documentation Translations](TRANSLATIONS.md).

Since schema v4, Blueprints can describe non-table database objects and deployment prerequisites
without publishing their source names, definitions, endpoint strings, secrets,
certificates, keys, or binaries. This inventory helps DBWarp estimate migration
complexity and identify work that needs packages, infrastructure, security
approval, or assisted conversion.

Inventory is not a capability claim. An object being reported does not mean
that DBWarp can automatically recreate or translate it. Migration capability
must be checked separately against DBWarp's route and artifact capability
matrix.

## Detail Levels

Use `--artifact-detail` to choose the privacy and planning tradeoff:

| Value | Database reads | Blueprint output | Consent |
|---|---|---|---|
| `none` | No artifact catalogs or definitions | No artifact counts or graph | No additional consent |
| `summary` | Artifact catalogs, but not definitions | Counts by kind and external-prerequisite class | Default; no additional consent |
| `graph` | Artifact catalogs and dependency metadata, but not definitions | Counts plus stable anonymous object records and dependency edges | Requires `--yes` |
| `analyzed` | Artifact catalogs, dependencies, and available definitions | Graph plus bounded language-feature and complexity bands | Requires `--yes` |

The default is `summary`. Use `none` when policy permits table-structure capture but
forbids non-table catalog collection. Use `graph` when dependency-aware planning
is required without reading definitions. Use `analyzed` only after approving
transient definition reads.

```bash
./dbwarp-blueprint \
  --connect postgresql://blueprint_user@db.internal/appdb \
  --password-file /etc/dbwarp/blueprint.pass \
  --tls-mode verify-full \
  --tls-ca /etc/pki/internal-root.crt \
  --artifact-detail analyzed \
  --out appdb.blueprint.toml \
  --audit-log appdb.blueprint.audit.txt \
  --yes
```

## Privacy Contract

Artifact output contains only bounded, closed-vocabulary metadata:

- internally consistent anonymous ids such as `view-001`, `function-002`, and
  `schema-A`; cross-run stability requires reuse of a protected
  `--anonymization-key-file`;
- closed object kind, subkind, tier, visibility, and security-mode tokens;
- dependencies expressed only through anonymous artifact or table ids;
- counts and bounded bands rather than free-form descriptions;
- standard catalog labels such as `pg_proc`, `information_schema.views`, or
  `sys.objects`;
- external prerequisite classes, never their names or material.

It does not contain source object names, SQL or procedural source text, schema
names, principals, endpoint strings, provider strings, credentials, key
material, certificate bodies, assembly files, extension package names, or
loadable-library names.

In `analyzed` mode, definitions are held only long enough to scrub comments and
literals and derive bounded lexical aggregates. Definitions are wrapped in a
zeroizing owner and are not serialized, logged, placed in the audit log, or
sent to another service. This is a process-memory minimization control, not a
claim that operating-system paging or a privileged process debugger is
impossible.

Anonymous graphs can still fingerprint an application through object counts and
topology. That is why `graph` and `analyzed` fail with `DBP1014E` unless the
operator supplies `--yes`.

## Completeness Evidence

The `[artifact_inventory]` block is deliberately self-auditing:

| Field | Meaning |
|---|---|
| `contract` | Independently versioned contract; currently `dbwarp-blueprint-artifacts/v1` |
| `detail` | Requested detail level |
| `visibility` | `full`, `privilege_filtered`, or `unknown` |
| `inventory_complete` | True only with full visibility, no unreadable catalogs, and no declared unmodeled families |
| `dependencies_complete` | True only when dependency sources were readable and the collector can account for the modeled families |
| `analysis_complete` | True only for `analyzed` detail when every available analysis is complete |
| `catalogs_read` | Standard catalog families successfully inspected |
| `catalogs_unreadable` | Catalog families that failed or were unavailable |
| `families_not_inventoried` | Known object families outside the current collector contract |

An optional catalog failure does not silently remove objects. The run emits
`DBP1410W`, records the affected catalog, and forces the corresponding
completeness claims to false. A low-privilege account can therefore produce a
useful partial inventory without presenting absence as proof.

## Engine Coverage

The v1 collector inventories the following modeled families:

| Engine | Modeled object families |
|---|---|
| PostgreSQL | views, materialized views, sequences, routines, aggregates, enum/domain/composite/range types, triggers, defaults, checks, policies, rules, event triggers, extensions, foreign tables/servers, publications, subscriptions, tablespaces, and native-language functions |
| MySQL | views, stored functions and procedures, triggers, scheduled events, view dependencies, FEDERATED tables, and loadable UDF registrations |
| SQL Server | views, stored procedures, scalar/table functions, CLR modules, triggers, defaults, checks, rules, synonyms, sequences, user-defined types, CLR assemblies, external data objects, full-text catalogs, partition objects, non-primary filegroups, certificates, keys, database-scoped credentials, linked servers, and SQL Server Agent jobs |

Each Blueprint lists known unmodeled families. Do not infer that an empty count means
an engine has no such objects unless `visibility`, completeness fields, and the
unmodeled-family list support that conclusion.

## External Prerequisites

Objects that depend on something outside portable table DDL carry an anonymous
external-prerequisite class. Current classes include:

| Class | Examples of what an operator must resolve |
|---|---|
| `postgresql_extension` | Compatible extension package and target version |
| `postgresql_native_function` | Native language library and ABI compatibility |
| `mysql_loadable_udf` | Loadable UDF binary and source-server ABI assumptions |
| `sqlserver_clr_assembly` | CLR enablement, assembly, runtime, and trust policy |
| `foreign_endpoint` | Network, provider, remote database, and authentication configuration |
| `replication_topology` | Publication/subscription topology and target policy |
| `physical_storage` | Filegroup or physical-placement design |
| `server_feature` | Managed-service or server feature availability |
| `certificate_material` | Certificate issuance or import under target policy |
| `encryption_or_credential_material` | Keys, credentials, external key store, and secret handling |
| `sqlserver_agent` | Agent availability, operating environment, and job governance |

The Blueprint records whether binary, secret, or endpoint material is required, but
never captures that material. External objects should become explicit migration
tasks, not best-effort omissions.

## Language Feature Census

`analyzed` detail adds `dbwarp-language-feature-census/v1` blocks for available
SQL and procedural definitions. The first analyzer is `lexical-v1` and reports
`status = "partial"`; it is not a parser, compiler, semantic binder, or
translation-success guarantee.

It records bounded bands for definition size, statement count, token count,
nesting, cyclomatic complexity, and opaque/dynamic regions. It also records
closed feature families such as control flow, joins, subqueries, CTEs,
aggregation, windows, DML, DDL, temporary objects, dynamic SQL, JSON, XML,
spatial, vector, and security modes. Engine context includes normalized grammar
profile, MySQL SQL modes, and SQL Server compatibility, `ANSI_NULLS`, and
`QUOTED_IDENTIFIER` settings where available.

The lexical analyzer removes comments, quoted literals, and quoted identifiers
before counting. It has context rules for trigger event declarations,
PostgreSQL `EXECUTE FUNCTION`, and SQL Server module options. Even so, all
results remain coarse planning evidence. A future grammar-backed analyzer can
use a new analyzer version without changing the outer artifact contract.

## Recommended Review Workflow

1. Run the default `summary` level with an artifact-catalog review. If policy
   permits table catalogs only, use `--artifact-detail none` instead and omit
   this inventory.
2. Inspect counts, external classes, visibility, unreadable catalogs, and known
   unmodeled families.
3. Approve `graph` only if anonymous dependency topology is acceptable.
4. Approve `analyzed` only if transient definition reads are acceptable.
5. Keep the audit log locally as access-controlled evidence. Share it only when
   a named recipient needs the endpoint, identity, path, and degradation detail
   through an approved secure channel.
6. Compare the inventory with DBWarp's migration capability matrix before
   promising automated recreation or cross-engine translation.

For exact serialized fields, see the [Format Reference](../FORMAT.md). For
runtime reads, writes, warnings, and trust assertions, see the [Audit
Reference](../AUDIT.md).
