# Non-Table Artifact Inventory

**Language:** English is authoritative. Machine-translated editions, when
available, are non-authoritative and may contain errors. See
[Documentation Translations](TRANSLATIONS.md).

Blueprints can describe non-table database objects and deployment prerequisites
without publishing their source names, definitions, endpoint strings, secrets,
certificates, keys, or binaries. This inventory helps DBWarp estimate migration
complexity and identify work that needs packages, infrastructure, security
approval, or assisted conversion.

Inventory is not a capability claim. An object being reported does not mean
that DBWarp can automatically recreate or translate it. Confirm with DBWarp
which object types are supported.

## Detail Levels

Use `--artifact-detail` to choose the privacy and planning tradeoff:

| Value | Database reads | Blueprint output | Consent |
|---|---|---|---|
| `none` | No artifact inventory catalogs or definitions (the count-only topology probe still runs) | Explicit v7 not-requested inventory; no counts or graph | No additional consent |
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
- typed relationships expressed only through anonymous artifact or table ids,
  with closed evidence and unresolved-reason tokens;
- bounded, engine-qualified feature requirements for migration planning;
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
| `contract` | Independently versioned contract; v7 uses `dbwarp-blueprint-artifacts/v2` and older Blueprint schemas retain v1 |
| `detail` | Requested detail level |
| `scope` | V7 catalog scope: `all-visible-schemas`, `selected-schemas`, `structured-source`, or `unknown` |
| `visibility` | `full`, `privilege_filtered`, or `unknown` |
| `inventory_complete` | True only with full visibility, no unreadable catalogs, and no declared unmodeled families |
| `dependencies_complete` | True only when dependency sources were readable and the collector can account for the modeled families |
| `requirements_complete` | V7 aggregate: true only after engine version and edition checks, complete assessment-population coverage for the selected scope, and `requirement_status = complete | not_applicable` on every emitted artifact; omission means false |
| `analysis_complete` | True only for `analyzed` detail when every available analysis is complete |
| `catalogs_read` | Standard catalog families successfully inspected |
| `catalogs_unreadable` | Catalog families that failed or were unavailable; affected completeness claims degrade without erasing unrelated per-object requirement evidence |
| `catalogs_not_applicable` | Catalog families proved inapplicable; disjoint from readable and unreadable sets |
| `families_not_inventoried` | Known object families not inventoried by this release |

An optional catalog failure does not silently remove objects. The run emits
`DBP1410W`, records the affected catalog, and forces the corresponding
completeness claims to false. A low-privilege account can therefore produce a
useful partial inventory without presenting absence as proof.

`object_count` counts emitted artifact records, not rows from one native
catalog. Oracle packages and object types are modeled as specification, body,
and member records; the body owns the combined language analysis. Conversely,
an object observed in multiple catalogs is counted once. Trigger metadata and
source, for example, are joined by native object identity before anonymization.

## Aggregate Complexity Contract

Schema v7 defines an aggregate-only
`[artifact_inventory.complexity]` record for `graph` and `analyzed` detail.
It does not add a database read or permission: the assessment is derived from
the already approved anonymous graph and language census. It is required for
`graph` and `analyzed`, and absent for `none` and `summary`.

The assessment reports seven closed dimensions: volume, control flow,
feature breadth, entanglement, environment coupling, opacity, and dialect
coupling. Results are bands rather than a numerical score. `overall_score` is
reserved and not populated, because a 0-100 value would imply unsupported
precision.

Every dimension carries a one-dimensional exact histogram over the eligible
population. Volume uses the language census size bands; the other six
dimensions use its count bands. Both forms add explicit `not_applicable` and
`unknown` buckets, and each histogram must reconcile as `eligible = assessed +
not_applicable + unknown`. The format has no per-object composite and no
cross-tabulation by object kind, feature, or schema. Engine-generated and
secondary objects that are positively identified are excluded from assessment
but remain visible in the inventory; temporary objects and objects with
missing flags remain eligible. An external binary is not excluded merely
because its body cannot be read: site-installed plugins, CLR assemblies, Java, and
libraries remain real migration work. Only an affirmative engine-generated
flag excludes that class of object.

`assessment_population_complete` states whether every object eligible under
that policy is known. It is independent of the broader `inventory_complete`
claim, and omission means false. An incomplete assessment population forces an
unknown overall band unless the known lower bound is already `very-high`.

Coverage is recorded per dimension. A `not_applicable` census result is a
completed assessment, not missing evidence: an object whose dimensions are all
proved inapplicable belongs in the fully assessed count. Partially assessed
means at least one applicable dimension is known and another is unknown;
unassessed means no applicable dimension is known. Unknown evidence is never
treated as low complexity. The assessor computes lower and upper outcomes for
each affected dimension. A partial dimension is `unknown` unless its known
lower bound is already `very-high`; the overall band is emitted only when its
bounds agree. Graph mode never emits an overall verdict for a non-empty population,
because definitions were not read. A complete empty population is
`not-applicable`.

Wrapped objects contribute to the opacity histogram's `unknown` bucket even
when no partial language census can be produced. Read the opacity band
together with its coverage so a small observed band is not read without its
unknown population. If the assessment itself fails, the full inventory is
retained with a canonical all-unknown aggregate. Wrapped or withheld definitions,
incomplete graphs, incomplete requirement evidence, selected-scope boundaries,
and unsupported languages or dialects remain
explicit limitations. The limitation assertions are derived from the artifact
and census evidence where possible. `unsupported-dialect` remains distinct
because the census can name a dialect and report `unavailable`, but has no
`unsupported` status; it means the definition was read but the named analyzer
does not support that dialect, not that the source was withheld or wrapped.

The record carries the single analyzer version and sorted sets of analysis
spans, dialects, and grammar profiles present in the eligible census. Two captures
are comparable only when those sets, the contract, assessor, scope, and
population policy all match. A bundle retains
complexity per child source and never aggregates it across engines or analyzers.

The complexity contract and assessor version are independent. For exact fields and invariants, see the
[Format Reference](../FORMAT.md).

## Engine Coverage

The current collector inventories the following modeled families:

| Engine | Modeled object families |
|---|---|
| PostgreSQL | views, materialized views, sequences, routines, aggregates, enum/domain/composite/range types, triggers, defaults, checks, policies, rules, event triggers, extensions, foreign tables/servers, publications, subscriptions, tablespaces, and native-language functions |
| MySQL | views, stored functions and procedures, triggers, scheduled events, view dependencies, FEDERATED tables, and loadable UDF registrations |
| SQL Server | views, stored procedures, scalar/table functions, CLR modules, triggers, defaults, checks, rules, synonyms, sequences, user-defined types, CLR assemblies, external data objects, full-text catalogs, partition objects, non-primary filegroups, certificates, keys, database-scoped credentials, linked servers, and SQL Server Agent jobs |

Each Blueprint lists known unmodeled families. Do not infer that an empty count means
an engine has no such objects unless `visibility`, completeness fields, and the
unmodeled-family list support that conclusion.

## Requirement Evidence

Artifact requirements are engine facts from bounded catalog columns
or dedicated engine-aware syntax checks. Generic lexical analysis does not
create engine-qualified requirements. When an analyzed language feature echoes
the same fact, the requirement takes precedence and the
feature remains a lexical observation. A missing requirement is not proof that
every requirement token was checked.

Every v7 graph/analyzed object records `requirement_status` as `complete`,
`partial`, `unavailable`, or `not_applicable`. Only `complete` makes an empty
list proof of zero requirements for that object. `partial` records that some
fact or producer succeeded without exhaustive coverage; `unavailable` records
that no producer established usable coverage and therefore cannot accompany
known requirement or external-prerequisite evidence. Such evidence requires
`partial`. Both contribute unknown
requirement-derived complexity observations while unaffected complete objects
remain assessable.
`not_applicable` forbids requirement and external-prerequisite records. The
inventory-level `requirements_complete` is true only after engine version and
edition checks, a complete assessment population, and when every
emitted object is complete or not applicable. PostgreSQL, MySQL, and SQL Server
set the aggregate only after every applicable artifact catalogue has been
attempted and the selected-scope population is proved complete. A denied or
unreadable catalogue, or a selection boundary whose population cannot be
proved, keeps the aggregate false without erasing complete per-object evidence
from the catalogues that were read. Artifact requirements are not reported
for Oracle.

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

`analyzed` detail adds `dbwarp-language-feature-census/v1` blocks. Schema v7
emits `lexical-v2`, which analyzes only the executable or declarative body and
records `analysis_span = "executable-body"`. It excludes the outer creation
wrapper, identity, signature, return declaration, and module options. If the
body cannot be safely isolated the collector records an unknown span and
unavailable evidence; objects with no definition dimension use
`not-applicable`. An omitted span is interpreted as unknown rather than being
inferred from the engine. The analyzer reports `status = "partial"` for supported
definitions because it is not a parser, compiler, semantic binder, or
translation-success guarantee. Missing or unsupported definition evidence is
`unavailable`, while a proved inapplicable analysis is `not_applicable`.

It records bounded bands for definition size, statement count, token count,
nesting, cyclomatic complexity, and opaque/dynamic regions. It also records
closed feature families such as control flow, joins, subqueries, CTEs,
aggregation, windows, DML, DDL, temporary objects, dynamic SQL, JSON, XML,
spatial, vector, raised errors, transaction control, ref cursors, anchored
types, interval/time-zone/Boolean/LOB use, and security modes. Engine context
includes normalized grammar profile, MySQL SQL modes, and SQL Server
compatibility, `ANSI_NULLS`, and `QUOTED_IDENTIFIER` settings where available.

The lexical analyzer removes comments, quoted literals, and quoted identifiers
before counting. It has context rules for trigger event declarations,
PostgreSQL `EXECUTE FUNCTION`, and SQL Server module options. Even so, all
results remain coarse planning evidence. Wrapped PL/SQL is refused;
obfuscated bytes never become plausible body measurements.

## Recommended Review Workflow

1. Run the default `summary` level with an artifact-catalog review. If policy
   permits table catalogs only, use `--artifact-detail none`; v7 records that
   decision explicitly rather than omitting the inventory state.
2. Inspect counts, external classes, visibility, unreadable catalogs, and known
   unmodeled families.
3. Approve `graph` only if anonymous dependency topology is acceptable.
4. Approve `analyzed` only if transient definition reads are acceptable.
5. Keep the audit log locally as access-controlled evidence. Share it only when
   a named recipient needs the endpoint, identity, path, and degradation detail
   through an approved secure channel.
6. Do not assume an inventoried object can be recreated or translated
   automatically; confirm with DBWarp.

For exact serialized fields, see the [Format Reference](../FORMAT.md). For
runtime reads, writes, warnings, and trust assertions, see the [Audit
Reference](../AUDIT.md).
