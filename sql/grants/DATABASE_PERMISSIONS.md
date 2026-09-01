# DBWarp Blueprint database permissions

> **Status:** Reviewed query by query against the current source, with each
> script proven live against PostgreSQL 13-18, MySQL 8.0/8.4/9.7, and SQL
> Server 2019/2022/2025 under both scoped grants and the applicable
> built-in-role alternatives. See Section 5.4 for the evidence boundary.
> Managed-service combinations remain separately qualified paths.

This document defines the minimum access required by the current
`dbwarp-blueprint` binary. It covers only the three implemented database
engines: PostgreSQL, MySQL, and SQL Server.

**Ready-to-run grant scripts exist for every profile and engine.** They are
checked into this repository under `sql/grants/` (the folder that also holds
this document) and were verified live on 2026-08-28 by applying each one and
running the binary at its tier:

| Engine | Catalog-only (`basic`) | Synthetic-copy-ready (`standard`) | Enhanced | Apply with |
|---|---|---|---|---|
| PostgreSQL 13-18 | `sql/grants/postgresql/basic.sql` | `sql/grants/postgresql/standard.sql` | `sql/grants/postgresql/enhanced.sql` | `psql -d TARGET_DB -f FILE` |
| MySQL 8.0 / 8.4 / 9.7 | `sql/grants/mysql/basic.sql` | `sql/grants/mysql/standard.sql` | `sql/grants/mysql/enhanced.sql` | `mysql -u root -p < FILE` |
| SQL Server 2019 | `sql/grants/sqlserver-2019/basic.sql` | `sql/grants/sqlserver-2019/standard.sql` | `sql/grants/sqlserver-2019/enhanced.sql` | `sqlcmd -S HOST -E -i FILE` |
| SQL Server 2022 / 2025 | `sql/grants/sqlserver-2022/basic.sql` | `sql/grants/sqlserver-2022/standard.sql` | `sql/grants/sqlserver-2022/enhanced.sql` | `sqlcmd -S HOST -E -i FILE` |

Each script is a single-step, idempotent file: the DBA edits the marked
principal/password/scope lines at the top, runs it once, and gets a
verification query at the end. The scripts create per-tier principals
(`dbwarp_blueprint_basic`, `dbwarp_blueprint_standard`,
`dbwarp_blueprint_enhanced`) so all three tiers can coexist on one test
server. `sql/grants/README.md` records the tier/CLI mapping and the run
evidence.

The profile names used below map to the scripts as **catalog-only =
`basic`**, **synthetic-copy-ready = `standard`**, and **enhanced =
`enhanced`** (the `--artifact-detail summary/graph/analyzed` contract). The
SQL fragments in Section 3 explain and justify each grant; the scripts are the
artefact to apply. The examples use the generic `dbwarp_blueprint` name for
whichever single tier is approved in production.

The document has two audiences:

1. **Executive, risk, and platform owners** should review Section 1 and decide
   which access profile and managed-service authentication path may be used.
2. **DBAs and security reviewers** should use Sections 2-6 to verify and grant
   the exact database, cloud-IAM, secret, and network permissions.

| Section | Purpose | Primary reader |
|---|---|---|
| **1. Executive review** | Select the profile, platform path, and acceptable scope | Executive, risk, platform owner |
| **2. Detailed review** | Confirm command, version, visibility, and query boundaries | DBA, security reviewer |
| **3. Database runbooks** | Apply exact SQL grants or approved convenience roles | DBA |
| **4. Managed-service IAM** | Apply external IAM, token, connector, and secret boundaries | Cloud security, IAM, DBA |
| **5. Control gates** | Prove completeness and reject over-broad access | DBA, security, product owner |
| **6. Evidence** | Trace requirements to code and vendor documentation | Reviewer, auditor |

## 1. Executive review

### 1.1 Recommendation and decision requested

> **Recommendation:** Approve the exact least-privilege grants for the chosen
> capture profile and apply them with the corresponding script from
> `sql/grants/` (`basic.sql`, `standard.sql`, or `enhanced.sql` per engine). Permit a documented built-in role only when its additional
> read scope is acceptable. Keep cloud provisioning and administrative roles
> off the collector identity. The managed MySQL token client path is now
> implemented with verified-TLS enforcement, but no provider/service/version
> row should be approved until its live qualification evidence exists.

The reviewer is being asked to approve five decisions:

1. Choose **catalog-only** (`basic`), **synthetic-copy-ready** (`standard`),
   or **enhanced** access for each source.
2. Use the exact object grants as the baseline policy.
3. Accept a broader built-in or reusable role only where its caveat is
   explicitly recorded.
4. Add cloud IAM only when the selected token or connector path requires it.
5. Keep one-time identity, network, and database provisioning authority with
   existing administrative operators, never with the collector.

### 1.2 Access profiles

| Profile | Business purpose | Source-row access | Approval position |
|---|---|---:|---|
| **Catalog-only** | Inventory, coarse sizing, and initial DBA review | No customer-row reads | Lower exposure, but weaker synthetic fidelity |
| **Synthetic-copy-ready** | Representative synthetic generation and benchmark preparation | Bounded `SELECT` samples from every in-scope table | Approve only when sampled source-row access and the acceptance gates are acceptable |
| **Enhanced** | Migration-discovery inventory of non-table objects, anonymous dependency graph, and definition-derived feature bands | Bounded samples plus transient reads of object definitions | Approve only when definition reads and anonymous topology are acceptable; on MySQL it requires the DDL-capable `TRIGGER` and `EVENT` privileges |

The synthetic-copy-ready profile is the minimum profile that provides enough
information for a representative synthetic copy. Catalog-only output may be
useful for estimation, but it must not be represented as equivalent.

### 1.3 Database-platform summary

| Platform | Catalog-only minimum | Synthetic-copy-ready addition | Low-maintenance option | Principal caveat |
|---|---|---|---|---|
| **PostgreSQL 13-18** | Login role, `CONNECT` to the target database, and intact standard catalog ACLs | `USAGE` on selected schemas and `SELECT` on every selected ordinary table | Schema-wide grants; `pg_read_all_data` on 14+ only | PostgreSQL 13 is a legacy/EOL target; `pg_read_all_data` is cluster-wide and includes future tables, views, and sequences |
| **MySQL 8.0, 8.4, and 9.7** | `REFERENCES` on every in-scope base table | Use `SELECT` instead of `REFERENCES` on every in-scope base table | Reusable schema-scoped role set as the account's default role | Use `--schema` to match the approved database grants; a global `*.*` grant remains too broad and an unscoped run walks all visible non-system schemas |
| **SQL Server 2019** | Database user with `CONNECT`, `VIEW DEFINITION`, and `VIEW DATABASE STATE` | `SELECT` on every selected user table | Schema `SELECT` or per-database `db_datareader` | `db_datareader` includes all current and future tables and views in the database |
| **SQL Server 2022/2025** | Database user with `CONNECT`, `VIEW DEFINITION`, and `VIEW DATABASE PERFORMANCE STATE` | `SELECT` on every selected user table | Schema `SELECT` or per-database `db_datareader` | `VIEW SECURITY DEFINITION` is not a substitute for `VIEW DEFINITION`: verified on 16.0.4265, it exposes keys and certificates but zero tables. Only the DMV permission differs from 2019 |

Enhanced additions on top of synthetic-copy-ready (all verified live on
2026-08-28):

| Platform | Enhanced addition |
|---|---|
| **PostgreSQL** | None; every artifact catalog and definition function is `PUBLIC`-readable |
| **MySQL** | `SHOW VIEW`, `TRIGGER`, `EVENT` on the in-scope schemas; global `SHOW_ROUTINE` (8.0.20+); `SELECT` on `performance_schema.user_defined_functions` |
| **SQL Server** | `SELECT` on `sys.sql_expression_dependencies`; a user in `msdb` with `SELECT` on `dbo.sysjobs` |

None of the catalog-only or synthetic-copy-ready profiles requires write
access, DDL, ownership, impersonation, server administration, RLS bypass,
unmasking, or decryption-key access. The one exception is the MySQL enhanced
profile: `TRIGGER` and `EVENT` are DDL-capable, and MySQL offers no read-only
way to expose those catalogs.

### 1.4 Cloud-managed platform summary

Managed hosting does not automatically require cloud IAM. With a native
database credential, a supplied endpoint, and an existing network path, the
runtime cloud permission set is empty.

| Cloud platform | Native database credential | Token or connector minimum | Current review position |
|---|---|---|---|
| **AWS RDS/Aurora** | No AWS IAM required at runtime | PostgreSQL/MySQL IAM authentication: exact `rds-db:connect` on one database-user ARN | PostgreSQL and MySQL token paths are implemented and require `cloud-token` plus `verify-full`; qualify each service/version live. RDS SQL Server has no IAM database-authentication path. |
| **Azure managed databases** | No Azure RBAC required at runtime | Entra token: no Azure resource role; create/map the Entra database principal and grant database permissions | PostgreSQL/MySQL use `cloud-token`; Azure SQL uses `entra-token`. Qualify each service/version live. |
| **Google Cloud SQL** | No Google Cloud IAM required for a direct native-password connection | Direct PostgreSQL/MySQL IAM login: `cloudsql.instances.login`; Auth Proxy/connector: `roles/cloudsql.client` | Direct PostgreSQL/MySQL token paths are implemented with `cloud-token`; proxy/automatic-IAM paths remain separate live qualification rows. SQL Server still needs a database credential. |

Secret-store access is optional and belongs to the external credential
wrapper, not to the database principal. Network and service provisioning
belong to the customer's existing cloud/IaC operators.

### 1.5 Trust boundaries and material caveats

| Issue | Executive significance |
|---|---|
| **Exact grants are command-dependent** | The catalog-only and synthetic-copy-ready contracts are exact only with `--artifact-detail none`; the CLI default (`summary`) and `graph`/`analyzed` fall under the enhanced contract. Unattended runs at every tier need `--yes`, otherwise the pre-flight prompt ends the run with `DBP1701E`. |
| **Successful exit is not completeness proof** | Every engine can hide metadata or rows through privilege filtering or row-security controls. Captured counts must be reconciled. |
| **Schema scope is explicit** | Pass repeatable `--schema NAME` and align it with the approved SQL grants. An unresolved or privilege-hidden schema fails closed with `DBP1420E`; omitting the option intentionally restores the broader all-visible walk. |
| **Managed token implementation is not live qualification** | PostgreSQL/MySQL `cloud-token` requires exactly one external token source and `verify-full`; MySQL enables `mysql_clear_password` only in that mode. Each provider/service/version path still needs live proof. |
| **Convenience roles trade maintenance for scope** | `pg_read_all_data`, schema-wide MySQL roles, `db_datareader`, and some cloud built-ins remain read-oriented but authorize more than the literal command minimum. |
| **Version compatibility is not support** | Every engine version, managed-service variant, TLS mode, and authentication path needs its own live qualification evidence. |

### 1.6 Executive approval record

- [ ] The approved profile is recorded as catalog-only or
  synthetic-copy-ready.
- [ ] The source databases, schemas, and tables in scope are recorded.
- [ ] Any convenience role and its additional current/future object scope are
  accepted explicitly.
- [ ] The authentication path and any cloud token, connector, or secret-reader
  permission are recorded.
- [ ] Provisioning and network administration remain separate from the
  collector identity.
- [ ] Row-security, masking, encryption, and replica-filtering behavior are
  accepted or the run is directed to an approved scrubbed source.
- [ ] The exact engine/service/version combination has passed the required
  qualification gate.

## 2. Detailed DBA and security review

### 2.1 Capture profile and command contract

Authentication is a prerequisite rather than a data privilege. PostgreSQL
needs a login role accepted by `pg_hba.conf`; MySQL needs an account whose
host pattern matches the collector; SQL Server needs a login or contained
identity mapped to the database user. TLS and authentication choices do not
change the table/catalog grant set.

For catalog-only capture, omit `--measure-compression` and run with:

```text
--artifact-detail none
```

For synthetic-copy-ready capture, run with:

```text
--artifact-detail none --measure-compression --yes
```

For enhanced capture, run with:

```text
--artifact-detail analyzed --measure-compression --yes
```

Add one `--schema NAME` for the approved application schema, repeating it when
the application model legitimately spans schemas. For example:

```text
--schema app --schema shared_reference --artifact-detail none --measure-compression --yes
```

PostgreSQL and SQL Server still select the database in the connection URI.
MySQL treats schema and database as synonyms, but the collector nevertheless
requires `--schema` for an explicit scope; the URI database alone is not a
scope control. If a selected schema is absent or invisible to the account, the
run ends with `DBP1420E` before a Blueprint is written.

`summary` and `graph` read strictly less than `analyzed` but need the same
grants. `--yes` is also required for unattended catalog-only runs: the
pre-flight prompt is shown at every tier, and a closed stdin ends the run with
`DBP1701E`.

The default RTT probe is five constant `SELECT 1` statements. It needs no
table privilege and reads no customer rows, so `--no-rtt-probe` is a policy
choice rather than a least-privilege requirement. Non-table artifact capture
is a separate migration-discovery concern; the current synthetic generation
core consumes tables, columns, primary/unique index structure, foreign keys, row
counts, and sampled statistics, not the non-table artifact inventory.

The CLI default is `--artifact-detail summary`, not `none`. A default run
therefore attempts additional non-table catalog queries. Those queries are
best-effort and privilege-filtered; a successful process exit does not prove
that the artifact inventory is complete. The grant sets in this document are
exact only when the operator explicitly selects `--artifact-detail none`.

For either profile, the operator must reconcile captured table and
relationship counts and inspect the generated blueprint and audit before
acceptance.

### 2.2 Engine and version boundary

There is no separately published, versioned support matrix for these three
engines yet. The rows below therefore distinguish the catalog SQL's
compatibility boundary from a qualified product claim.

| Engine | Version-aware boundary in the current implementation | Review position |
|---|---|---|
| PostgreSQL | The mandatory index query reads `pg_index.indnkeyatts`, introduced in PostgreSQL 11. | PostgreSQL 11 is the SQL floor. The qualification matrix is PostgreSQL 13 through 18; 13 is retained as a legacy target after upstream support ended in November 2025. Do not claim versions below 11. |
| MySQL | Functional-index metadata is detected at runtime by checking whether `information_schema.STATISTICS.EXPRESSION` exists. | Qualify MySQL 8.0, 8.4, and each intended newer release line independently. MySQL 5.7 is end-of-life and is not proposed as a support target. |
| SQL Server | `sys.dm_db_partition_stats` changed to the granular 2022 permission family. | Use the SQL Server 2019 grant set for 15.x; use the SQL Server 2022+ set for 16.x and 17.x (SQL Server 2025). Qualify 2019, 2022, and 2025 separately; do not infer support for older releases from the legacy permission syntax. |

Record the exact engine version from every accepted run. A compatible query is
not enough to prove driver, TLS, type, catalog, or sampling fidelity on that
version.

### 2.3 Account visibility and grant scope

The grant scope must match the collector's actual catalog scope.

| Engine | Current catalog scope |
|---|---|
| PostgreSQL | Every ordinary table (`relkind = 'r'`) in the selected schemas of the connected database. With no selector, every visible non-system schema is retained for compatibility. |
| MySQL | Every base table in the selected schemas. With no selector, every non-system schema visible to the account is retained, not only the database named in the URI. |
| SQL Server | Every visible non-system table in the selected schemas of the connected database. With no selector, every visible user schema is retained. |

The selected schema list must match the scope edited into the grant script. A
synthetic-copy-ready account needs `SELECT` on every selected table. Otherwise
the tool may complete with `DBP1407W` sampling warnings and a partially
representative Blueprint. Select every approved schema needed for a
cross-schema foreign key or dependency; an edge to an unselected schema is
intentionally absent.

The selector limits emitted tables, row sampling, and schema-owned artifact
definitions. Database-wide topology evidence and genuinely global artifact
families remain in scope. The Blueprint carries the closed
`selection-limited` limitation, and the audit adds only
`schema_selector_count` for this option, not its values. The existing redacted
connection URI still identifies the connected database (also the schema name
on MySQL).

### 2.4 Version- and tier-aware pre-capture requirements

These requirements are operational pre-work by an existing owner/DBA. They do
not add privileges to the collector identity. Keep application DDL stable for
the capture and record the exact database, selected schemas, expected
table/index/foreign-key counts, engine version, and source role (primary or
replica) for every tier.

| Engine/version | Catalog-only (`basic`) | Synthetic-copy-ready (`standard`) | Enhanced |
|---|---|---|---|
| **PostgreSQL 13-18** | Verify auto-analyze ran after the last material load, or have the owner/DBA run `ANALYZE schema.table`. `pg_class.reltuples` remains approximate and `pg_stats` widths are unavailable without table reads. On PostgreSQL 13, use an owner/DBA; do not invent a `MAINTAIN` grant that the release does not provide. | Add the standard grants; confirm RLS exposes exactly the approved population; begin with `--sample-rows 1000 --max-wall-secs 300` and raise both for a higher-detail request. Do not grant `BYPASSRLS` or maintenance authority to the collector. | Complete standard preparation and approve transient definition reads plus database-wide artifact families. No additional PostgreSQL grant is needed. |
| **MySQL 8.0, 8.4, and 9.7** | Treat InnoDB `TABLE_ROWS` as a rough estimate. After material loads, an existing DBA may run `ANALYZE TABLE schema.table` only with an approved read-lock/write window. The command requires `SELECT` and `INSERT`; neither `INSERT` nor maintenance execution belongs to the collector. | Add the standard grants and assess bias from first-row order, replica lag, masking, or tenant filtering. Sampling reuses the catalog session. Begin with `--sample-rows 1000 --max-wall-secs 300`; do not create histograms solely for Blueprint because the collector does not read them. | Complete standard preparation and explicitly approve DDL-capable `TRIGGER`/`EVENT`, global `SHOW_ROUTINE`, and the global UDF census. Do not grant global `ALL` merely to obtain a `full` visibility label. |
| **SQL Server 2019** | Keep partition maintenance stable and verify the intended primary/replica. Do not run `UPDATE STATISTICS` solely for Blueprint: row/page evidence comes from `sys.dm_db_partition_stats`, not optimizer histograms, and `UPDATE STATISTICS` requires broader `ALTER` authority. | Add the standard grants; confirm RLS, masking, Always Encrypted, `DENY`, replica filtering, and physical/clustered first-row ordering produce the approved population. Begin with `--sample-rows 1000 --max-wall-secs 300`; do not add `UNMASK`, key access, or RLS bypass. | Complete standard preparation and approve module-definition reads, `sys.sql_expression_dependencies`, and the `msdb` Agent-job census. |
| **SQL Server 2022/2025** | Apply the SQL Server 2019 preparation but use `VIEW DATABASE PERFORMANCE STATE`, not the 2019 DMV grant. | Apply the 2019 standard preparation with the 2022+ script. | Apply the 2019 enhanced preparation with the 2022+ script. |

For higher requested detail, increase sampling and wall-time budgets together,
not the privilege scope. Synthetic-copy readiness requires successful sampling
of every nonempty selected table, reconciliation of the expected inventory and
relationships, and no in-scope `DBP1406W`, `DBP1407W`, or `DBP1408W`. The
audit's fidelity estimate measures evidence coverage; it is not an empirical
error rate or confidence interval. The copy bundled with each grant set in
`sql/grants/README.md` gives a separate row for every engine/version family and
tier so the approving DBA can keep the requirement beside the applied SQL.

### 2.5 Query-to-permission mapping

The code audit maps every mandatory table-structure query and every optional
row query in the current engine implementations.

| Engine | Current mandatory reads with `--artifact-detail none` | Permission consequence |
|---|---|---|
| PostgreSQL | `current_setting`, `pg_class`, `pg_namespace`, `pg_stat_all_tables`, `pg_attribute`, `pg_stats`, `pg_index`, `pg_am`, `pg_constraint`, `pg_table_size`, and `pg_indexes_size` | Standard catalog/function ACLs plus `CONNECT` run the walk. Table `SELECT` is needed for `pg_stats` rows and for Tier 2 row samples. |
| MySQL | `VERSION()` and `information_schema.TABLES`, `COLUMNS`, `STATISTICS`, `KEY_COLUMN_USAGE`, and `REFERENTIAL_CONSTRAINTS` | `INFORMATION_SCHEMA` filters rows by effective object privilege. `REFERENCES` exposes the required table metadata without authorizing row reads; `SELECT` exposes the metadata and authorizes Tier 2. |
| SQL Server | `ORIGINAL_LOGIN()`, `SUSER_SNAME()`, `USER_NAME()`, `SERVERPROPERTY`, `sys.tables`, `sys.dm_db_partition_stats`, `sys.columns`, `sys.types`, `sys.indexes`, `sys.index_columns`, `sys.foreign_keys`, and `sys.foreign_key_columns` | The three identity built-ins require no additional grant and provide local audit evidence. Metadata visibility plus the version-specific DMV permissions are mandatory. Tier 2 additionally needs table `SELECT`. |

Immediately after connecting, the collector also sets a session-local safety
limit. PostgreSQL issues `SET statement_timeout`; MySQL issues `SET SESSION
max_execution_time`; SQL Server issues `SET LOCK_TIMEOUT`. These statements do
not require another database grant. PostgreSQL and MySQL bound the collector's
read-only statements on the server. SQL Server's setting bounds lock waits
only; the independent client wall deadline drops the connection for other
stalls and does not prove that the server acknowledged cancellation.

The optional Tier 2 statements are exactly:

- PostgreSQL: a bounded column projection with `LEFT(column::text, N)`,
  `TABLESAMPLE ... LIMIT N`, a `LIMIT N` fallback, and bounded single-column
  style probes;
- MySQL: a bounded column projection using `LEFT` for variable-width cells,
  `LIMIT N`, and bounded single-column style probes;
- SQL Server: a bounded column projection using `LEFT`/`SUBSTRING` for
  variable-width cells, `SELECT TOP (N) ... ORDER BY (SELECT NULL)`, and bounded
  single-column style probes.

Each engine's compression sample reads every source column through a bounded
projection, and the style probes then read individual text-like columns. A set
of column-level grants is therefore not a supported substitute for table read
permission. Explicit denials,
row-security policies, masked/encrypted values, or a role that is granted but
not active can still make a syntactically correct grant set insufficient.

#### Default artifact-summary boundary

With the default `--artifact-detail summary`, the code also queries non-table
catalogs for views, routines, triggers, scheduled objects, types, dependencies,
and external-object classes. Definitions are not read in summary mode, but
each engine still applies its own metadata-visibility rules:

- PostgreSQL catalog defaults usually allow the queries, but some cluster- or
  owner-sensitive catalogs can remain incomplete.
- MySQL schema `SELECT` does not prove visibility of every routine, event,
  trigger, view, or loadable UDF family. Verified on 9.7.0: view definitions
  need `SHOW VIEW`, routine rows and definitions need global `SHOW_ROUTINE`
  (`EXECUTE` reveals rows without definitions), triggers need `TRIGGER`,
  events need `EVENT`, and the UDF census needs `SELECT` on
  `performance_schema.user_defined_functions`. Missing privileges filter
  silently; only the UDF table records `DBP1410W`.
- SQL Server's completeness probe recognizes `VIEW DEFINITION`, which every
  tier grants. `graph`/`analyzed` additionally need `SELECT` on
  `sys.sql_expression_dependencies` (granted only to `db_owner` by default)
  and, for the Agent job census, a user in `msdb` with `SELECT` on
  `dbo.sysjobs`; without them the run records `DBP1410W` and
  `privilege_filtered` visibility.

This is why the catalog-only and synthetic-copy-ready contracts pin
`--artifact-detail none`. The `summary`/`graph`/`analyzed` contract is the
**enhanced** profile, specified in `*/enhanced.sql` and detailed per engine in
Section 3; its additional grants must not be silently added to a
synthetic-data reader.

## 3. Database grant runbooks

The statements in this section are the database-side grants for the selected
profile, shown so a reviewer can see and justify each one. To apply a profile,
run the matching ready-made script from `sql/grants/<engine>/<tier>.sql`
rather than transcribing these fragments. Replace the example principals and object names with the approved
customer scope; do not broaden them mechanically.

### 3.1 PostgreSQL

#### Version family

The same exact-grant permission model applies to the PostgreSQL 13-18
qualification matrix. The current mandatory SQL also parses on PostgreSQL 11+
because `pg_index.indnkeyatts` is present there. This is a compatibility floor,
not a promise to support releases outside the matrix. PostgreSQL 13 is an
explicit legacy target after upstream support ended in November 2025 and must
be qualified with object/schema grants because `pg_read_all_data` starts at 14.

#### Catalog-only minimum

The account needs the ability to log in and connect to the one database being
captured:

```sql
GRANT CONNECT ON DATABASE target_database TO dbwarp_blueprint;
```

On a standard PostgreSQL installation, the required `pg_catalog` relations and
catalog helper functions are readable/executable through their default
`PUBLIC` privileges. The current catalog-only path does not need `USAGE` on a
user schema and does not need `SELECT` on user tables merely to see object
names in `pg_catalog`.

This minimum has an intentional fidelity limit: `pg_stats` returns rows only
for tables the account can read. Without table `SELECT`, average-width
statistics can therefore be absent and the collector emits zero for those
widths. That result is catalog-only, not synthetic-copy-ready.

If the installation has revoked normal access to `pg_catalog` or its helper
functions, do not compensate with `SUPERUSER`, `pg_read_all_data`, or broad
ownership. Review the local hardening policy and grant only the exact catalog
relations/functions used by `src/engine_pg.rs`; PostgreSQL catalog ACLs and
function availability are version-specific.

#### Synthetic-copy-ready minimum

Add schema lookup permission and table read permission for every ordinary
table in the connected database. The literal minimum is object-level:

```sql
GRANT USAGE ON SCHEMA app, billing TO dbwarp_blueprint;
GRANT SELECT ON TABLE app.orders, app.customers, billing.invoices
    TO dbwarp_blueprint;
```

When every table-like relation in the named schemas is approved, the practical
schema-wide shortcut is:

```sql
GRANT SELECT ON ALL TABLES IN SCHEMA app, billing TO dbwarp_blueprint;
```

That shortcut can grant read access to views, materialized views, and foreign
tables that the current ordinary-table sampler does not use. It is still
read-only, but it is broader than the literal minimum.

For future relations, each owning role can set matching default privileges for
each fully approved schema it owns:

```sql
ALTER DEFAULT PRIVILEGES FOR ROLE application_owner IN SCHEMA app, billing
    GRANT SELECT ON TABLES TO dbwarp_blueprint;
```

Default privileges are another scope decision: they deliberately authorize
future table-like objects without a new review. Keep object-level grants when
new tables must be approved individually.

`SELECT` covers all current bounded sampling statements:

- a bounded all-column projection using `LEFT(column::text, N)`,
  `TABLESAMPLE SYSTEM (0.1) LIMIT N`, with a `LIMIT N` fallback;
- one-column `TABLESAMPLE ... LIMIT 32` style probes;
- `pg_stats` visibility for average widths.

No write, DDL, temporary-object, sequence, replication, superuser, or
`pg_read_all_stats` privilege is required by the current table-structure path.

PostgreSQL row-level security still applies. A role that can select a table but
sees only policy-filtered rows is not sufficient for a representative
synthetic copy unless the approved source population is exactly that filtered
population. Do not grant `BYPASSRLS` automatically; use an approved policy or a
scrubbed/read replica when full-population sampling is required.

#### Enhanced

No additional privilege. Verified on 17.11: a `CONNECT`-only role already
reads `pg_proc`, `pg_rewrite`, `pg_trigger`, `pg_policy`, `pg_event_trigger`,
`pg_extension`, `pg_foreign_server`, `pg_foreign_table`, `pg_publication`,
`pg_publication_rel`, `pg_subscription` (connection data is never selected),
`pg_tablespace`, `pg_type`, `pg_attrdef`, `pg_depend`, and every `pg_get_*def`
function that `analyzed` mode calls. The enhanced script therefore grants the
same set as the synthetic-copy-ready script.

PostgreSQL 13 needs a version-specific query, not a broader grant. Its
`pg_subscription.oid` is a protected hidden system column; selecting it would
require relation-wide `SELECT`, which would also authorize the secret-bearing
`subconninfo` column. On 13, Blueprint therefore inventories only subscriptions
for `current_database()` through the publicly readable `subdbid` and `subname`
columns and derives the anonymous native identity from those values. PostgreSQL
14 and later use the ordinary safe `oid`/`subname` projection, also scoped to
`current_database()`. Never grant relation-wide `SELECT` on
`pg_catalog.pg_subscription` merely to make enhanced capture complete.

Row-level-security caveat: `pg_stats` hides the rows of an RLS-enabled table
from every role without `BYPASSRLS`, so observed widths for such tables are
reported as 0 at every tier. Accept that or capture from a scrubbed replica;
do not grant `BYPASSRLS` for it.

#### DBA-friendly predefined role: `pg_read_all_data`

PostgreSQL 14 introduced the predefined `pg_read_all_data` role. On PostgreSQL
14-18, a DBA can replace all schema/table grants and default-privilege upkeep
with:

```sql
GRANT CONNECT ON DATABASE target_database TO dbwarp_blueprint;
GRANT pg_read_all_data TO dbwarp_blueprint;
```

This is a safe convenience option only when its scope is acceptable. It adds
no write, DDL, file, program-execution, replication, or superuser capability,
and it does not bypass RLS. It is broader than the exact requirement because it
authorizes every table, view, and sequence and grants schema `USAGE`, including
future objects that the current collector does not read.

The more important caveat is scope: PostgreSQL roles are cluster-wide, not
per-database. If the login can connect to another database in the same cluster,
`pg_read_all_data` applies there too. Offer this shortcut on a dedicated
instance, a single-database managed service, or a cluster where authentication
and `CONNECT` policy demonstrably restrict the collector to approved
databases. On a shared cluster with default `PUBLIC CONNECT`, keep the exact
object-level grants or the approved per-schema shortcut.

The shortcut also removes the ability to withhold a single object, and this is
not a scope question but an enforcement one. Membership of `pg_read_all_data`
carries `SELECT` on every table, and PostgreSQL has no `DENY`: revoking on one
table takes away nothing, because the privilege does not come from a grant on
that table. There is no combination of `REVOKE` statements that excludes a
table from a role member.

```sql
-- Has no effect while the login is a member of pg_read_all_data.
REVOKE SELECT ON TABLE app.payment_card FROM dbwarp_blueprint;
```

So if any table must be kept out of the Blueprint, `pg_read_all_data` is not
an option and the exact object-level or per-schema grants are required. This
differs from SQL Server: `DENY` there overrides `db_datareader`, so the same
carve-out remains possible. See
[DBA-friendly fixed role: `db_datareader`](#dba-friendly-fixed-role-db_datareader).

Verified live on 2026-08-31: with the role shortcut
applied, a table whose `SELECT` was revoked was still read; under per-schema
grants the same revoke withheld it.

There is no useful predefined-role shortcut for the catalog-only profile.
`pg_read_all_stats` and `pg_monitor` expose broader server monitoring
information and are not substitutes for the table-read permission that makes
the required `pg_stats` rows visible. Do not grant either merely for this tool.

### 3.2 MySQL

#### Version family

Use the same base grant model for MySQL 8.0, 8.4, and 9.7
lines. The collector probes for the optional `STATISTICS.EXPRESSION` column
before selecting it, so functional-index catalog differences do not require a
different grant.

#### Catalog-only minimum

MySQL permits any account to query most `INFORMATION_SCHEMA` tables but returns
only rows for objects on which the account has some privilege. The least
row-safe privilege that makes an in-scope base table visible is `REFERENCES`.
The literal minimum is object-level:

```sql
GRANT REFERENCES ON `appdb`.`orders`
    TO 'dbwarp_blueprint'@'collector-host';
```

Repeat the grant for every in-scope base table. If an entire schema is approved,
the practical shortcut is:

```sql
GRANT REFERENCES ON `appdb`.*
    TO 'dbwarp_blueprint'@'collector-host';
```

Do not grant it on `*.*`: an unscoped collector walks all visible non-system
schemas, while an explicitly scoped run needs only its selected schemas.

`SHOW VIEW` alone is not a substitute: verified on 9.7.0, it makes `TABLES`,
`STATISTICS`, and `REFERENTIAL_CONSTRAINTS` rows visible but leaves `COLUMNS`
and `KEY_COLUMN_USAGE` empty, so the catalog walk is incomplete.

`REFERENCES` does not permit `SELECT` from the table. It does permit foreign-key
references if the account independently has enough DDL privilege elsewhere,
so the dedicated collector account must not also receive `CREATE` or `ALTER`.

This grant and role path was first checked locally on MySQL 9.7.0. With the catalog
role active, the account saw the expected rows in
`information_schema.TABLES`, `COLUMNS`, `STATISTICS`, `KEY_COLUMN_USAGE`, and
`REFERENTIAL_CONSTRAINTS`, while a direct row `SELECT` failed with error 1142.
With no default role, the same account saw zero fixture tables; with the reader
role active, it saw all fixture metadata and could read the bounded table row.
The subsequent all-version qualification matrix covered the same grant behavior
on MySQL 8.0, 8.4, and 9.7.

#### Synthetic-copy-ready minimum

Use object-level `SELECT` instead of `REFERENCES` for every in-scope base
table:

```sql
GRANT SELECT ON `appdb`.`orders`
    TO 'dbwarp_blueprint'@'collector-host';
```

When the whole schema is approved, `GRANT SELECT ON appdb.*` is the practical
read-only shortcut and automatically covers future tables. It can also read
approved-schema views, which the current base-table sampler does not need.

`SELECT` both exposes the required `INFORMATION_SCHEMA` rows and authorizes the
current bounded sample:

```sql
SELECT LEFT(`text_column`, N), `fixed_width_column`, ...
FROM `schema_name`.`table_name` LIMIT N;
```

No `PROCESS`, `FILE`, `LOCK TABLES`, DDL, write, replication, or administrative
privilege is required by the current table-structure path. Keep the default
`--length-fidelity balanced`; it preserves declared capacities and index
prefixes exactly while privacy-rounding observed lengths. Exact length mode is
not required to generate a representative synthetic copy.

MySQL has no native table sampling in this path, so the first `N` rows can be
biased. Sufficient permission proves access, not statistical
representativeness. A sorted or clustered table may require a separately
approved sampling strategy in a future release.

#### Enhanced minimum

MySQL couples catalog visibility to object privileges, so each artifact family
needs its own privilege (each verified one at a time on 9.7.0):

```sql
GRANT SELECT, SHOW VIEW, TRIGGER, EVENT ON `appdb`.*
    TO 'dbwarp_blueprint'@'collector-host';
GRANT SHOW_ROUTINE ON *.* TO 'dbwarp_blueprint'@'collector-host';
GRANT SELECT ON `performance_schema`.`user_defined_functions`
    TO 'dbwarp_blueprint'@'collector-host';
```

`EXECUTE` is not needed. Read this before approving:

- `TRIGGER` and `EVENT` are DDL-capable: the account can also create or drop
  triggers in the granted schemas and create, alter, or drop events there.
  MySQL has no read-only privilege for those catalogs.
- `SHOW_ROUTINE` is global (MySQL 8.0.20+): it exposes the definitions of every
  routine on the server, and the collector's routine census then counts
  routines from every non-system schema, not only the granted ones (anonymous
  counts). Below 8.0.20 the only substitute is global `SELECT ON *.*`, which is
  broader still and not recommended.
- The collector reports artifact `visibility = "full"` for MySQL only when the
  account holds `ALL PRIVILEGES ON *.*`; with this least-privilege set it
  records `privilege_filtered` even though every catalog was read.

If those trade-offs are unacceptable, use the synthetic-copy-ready grant with
`--artifact-detail none`, or accept a privilege-filtered inventory.

#### DBA-friendly reusable roles

The shipped scripts grant directly to per-tier accounts, which avoids the
inactive-role pitfall described below; roles remain an option for reuse.

MySQL 8.0+ supports roles, but MySQL Server does not ship a fixed
`datareader`-style role suitable for this account. The reserved `mysql.sys`,
`mysql.session`, and `mysql.infoschema` accounts are locked internal accounts,
not shortcuts that may be granted to a collector.

The low-maintenance alternative is to create the policy once as a reusable
role. Use one of these roles for an account, according to the approved profile:

```sql
CREATE ROLE IF NOT EXISTS 'dbwarp_blueprint_catalog';
GRANT REFERENCES ON `appdb`.* TO 'dbwarp_blueprint_catalog';

CREATE ROLE IF NOT EXISTS 'dbwarp_blueprint_reader';
GRANT SELECT ON `appdb`.* TO 'dbwarp_blueprint_reader';
```

Then grant and activate the selected role for the collector account. MySQL
roles are not necessarily active on login, so the `SET DEFAULT ROLE` statement
is part of the permission contract:

```sql
GRANT 'dbwarp_blueprint_reader'
    TO 'dbwarp_blueprint'@'collector-host';
SET DEFAULT ROLE 'dbwarp_blueprint_reader'
    TO 'dbwarp_blueprint'@'collector-host';
```

Repeat the object grant inside the role for each approved schema. Do not use a
global `*.*` role: any static global privilege can also broaden schema-name
visibility, and an unscoped collector walks every visible non-system schema.
At acceptance time, verify `CURRENT_ROLE()` and the effective grants:

```sql
SHOW GRANTS FOR 'dbwarp_blueprint'@'collector-host'
    USING 'dbwarp_blueprint_reader';
```

A granted but inactive role produces silently filtered metadata and failed row
samples.

### 3.3 SQL Server

#### SQL Server 2019 (15.x) permission family

After the login has a user in the target database, grant:

```sql
USE [target_database];
GRANT CONNECT TO [dbwarp_blueprint];
GRANT VIEW DEFINITION TO [dbwarp_blueprint];
GRANT VIEW DATABASE STATE TO [dbwarp_blueprint];
```

`VIEW DEFINITION` supplies complete database metadata visibility.
`VIEW DATABASE STATE` is required by the mandatory
`sys.dm_db_partition_stats` query that supplies table/index bytes and row
counts.

#### SQL Server 2022 and 2025 permission family

SQL Server 2022 split the DMV permissions. For 16.x and 17.x use:

```sql
USE [target_database];
GRANT CONNECT TO [dbwarp_blueprint];
GRANT VIEW DEFINITION TO [dbwarp_blueprint];
GRANT VIEW DATABASE PERFORMANCE STATE TO [dbwarp_blueprint];
```

`VIEW DATABASE PERFORMANCE STATE` is the documented granular permission for
`sys.dm_db_partition_stats` on SQL Server 2022 and later and is sufficient for
it (verified on 16.0.4265). `VIEW DEFINITION` is still required for catalog
visibility: with `CONNECT`, `VIEW SECURITY DEFINITION`, and the DMV permission
the account sees certificates, keys, and credentials but zero tables, columns,
or indexes. Do not substitute `VIEW SECURITY DEFINITION`.

The artifact-completeness probe checks `VIEW DEFINITION` by name and therefore
reports full visibility with this grant set; the enhanced addition below
covers the two catalogs that `VIEW DEFINITION` does not make readable.

Azure SQL Database and Azure SQL Managed Instance use database-contained
identity setup that can differ from an on-premises login, but the permission
decision must still follow the engine capability: use the 2022+ granular pair
where available and prove it against the exact service/version deployed.

On every SQL Server path, the collector reads `ORIGINAL_LOGIN()`,
`SUSER_SNAME()`, and `USER_NAME()` immediately after connection. These are
session identity built-ins and need no extra database or server grant. Pass
`--expect-server-principal PRINCIPAL` to make the server compare the approved
login before catalog capture; mismatch or unavailable evidence fails closed
with `DBP1606E`. The exact values are retained only in the local audit.

#### Synthetic-copy-ready addition

For both SQL Server permission families, add object-level `SELECT` on every
table visible to the collector. The literal minimum is:

```sql
GRANT SELECT ON OBJECT::[app].[orders] TO [dbwarp_blueprint];
GRANT SELECT ON OBJECT::[billing].[invoices] TO [dbwarp_blueprint];
```

When every current and future readable object in a schema is approved, the
practical shortcut is:

```sql
GRANT SELECT ON SCHEMA::[app] TO [dbwarp_blueprint];
GRANT SELECT ON SCHEMA::[billing] TO [dbwarp_blueprint];
```

Schema `SELECT` is read-only but also covers views and selectable functions in
the schema and exposes metadata for other schema-contained objects. The current
base-table sampler does not need that additional scope.

This authorizes the current bounded sample:

```sql
SELECT TOP (N) LEFT([text_column], N), [fixed_width_column], ...
FROM [schema_name].[table_name]
ORDER BY (SELECT NULL);
```

No write, DDL, impersonation, server-state, SQL Agent, `db_owner`, or
`sysadmin` permission is required by the current table-structure path.

SQL Server row-level security still applies. A filtered security predicate can
make the sample unrepresentative even though `sys.dm_db_partition_stats`
reports the full table row/page counts. Treat that mismatch as a failed
synthetic-copy-readiness check, not as a reason to grant `sysadmin`.

#### Enhanced addition

For both permission families, `graph`/`analyzed` need two more grants
(verified on 2019 and 2022; without them the run records `DBP1410W` with
`catalogs_unreadable = ["msdb.dbo.sysjobs", "sys.sql_expression_dependencies"]`):

```sql
USE [target_database];
GRANT SELECT ON sys.sql_expression_dependencies TO [dbwarp_blueprint];
USE [msdb];
CREATE USER [dbwarp_blueprint] FOR LOGIN [dbwarp_blueprint];
GRANT SELECT ON dbo.sysjobs TO [dbwarp_blueprint];
```

`sys.sql_expression_dependencies` is selectable only by `db_owner` by default;
`db_datareader` also covers it. `SQLAgentReaderRole` is not sufficient for the
job census because the collector reads the base table rather than
`sysjobs_view`. Azure SQL Database has no `msdb`, so skip that batch there.
Column master/encryption keys, partition objects, filegroups, `sys.servers`,
and `sys.databases` were visible with `CONNECT` alone, so `VIEW ANY COLUMN
MASTER KEY DEFINITION`, `VIEW ANY COLUMN ENCRYPTION KEY DEFINITION`, and
server-level `VIEW ANY DEFINITION` are not part of the minimum.

#### DBA-friendly fixed role: `db_datareader`

For a synthetic-copy-ready account, the fixed per-database `db_datareader`
role is the preferred low-maintenance alternative to maintaining object or
schema `SELECT` grants:

```sql
USE [target_database];
ALTER ROLE [db_datareader] ADD MEMBER [dbwarp_blueprint];
```

Retain the version-specific metadata and DMV grants from the sections above;
`db_datareader` does not replace `VIEW DATABASE STATE` on 2019 or the 2022+
granular DMV pair.

`db_datareader` adds no write, DDL, impersonation, server-state, SQL Agent, or
administrative capability. It is broader than the exact grant because it reads
all current and future user tables and views in every schema in that database,
while the current collector samples base tables only. Unlike PostgreSQL's
`pg_read_all_data`, its scope is one database, so it is usually a reasonable
shortcut when the entire connected database is approved for synthetic
sampling. Keep schema grants when only selected schemas are approved.

It also keeps per-object exclusion available, which the PostgreSQL shortcut
does not. `DENY` overrides any `GRANT`, including one inherited from a fixed
role, so a single table can still be withheld from a `db_datareader` member:

```sql
USE [target_database];
DENY SELECT ON OBJECT::[app].[payment_card] TO [dbwarp_blueprint];
```

Verified live on 2026-08-31 on SQL Server 2019, 2022 and 2025: the
denied table was absent from the Blueprint while the rest of the database was
sampled normally. Use `DENY` rather than `REVOKE` here; revoking a permission
the login never held directly changes nothing.

#### Windows and domain principals for integrated authentication

The tier scripts in this directory create a SQL login with a password. That is
the wrong shape for `--auth-mode integrated`, which needs a login mapped to the
Windows or domain account the collector process runs as. Create the login
`FROM WINDOWS` first, then apply the tier script's grants unchanged:

```sql
USE [master];
IF SUSER_ID(N'EXAMPLE\dbwarp-blueprint') IS NULL
    CREATE LOGIN [EXAMPLE\dbwarp-blueprint] FROM WINDOWS
        WITH DEFAULT_DATABASE = [target_database];
GO

USE [target_database];
IF USER_ID(N'EXAMPLE\dbwarp-blueprint') IS NULL
    CREATE USER [EXAMPLE\dbwarp-blueprint] FOR LOGIN [EXAMPLE\dbwarp-blueprint];
GO
```

Then run the tier script with `:setvar login` set to the same principal and its
`CREATE LOGIN`/`ALTER LOGIN` password statements removed. Every `GRANT` in the
script applies to a Windows login unchanged; only the login DDL differs.

A domain group works in place of a user, as do managed service accounts and
computer accounts. A service running under a virtual account such as
`NT SERVICE\MSSQL$INSTANCE` presents the **computer** account on the network,
so that is the principal to grant and the owner of any service principal name.
Give each collector identity its own login rather than sharing one, or
per-identity grants cannot be told apart in an audit afterwards.

Two operational points, both observed during live verification on
2026-08-31:

- **The account the process runs as is the identity SQL Server sees.** If the
  collector is launched by a process running as an administrator, and that
  account or one of its Windows groups was explicitly provisioned as
  `sysadmin`, the session is sysadmin and every `DENY` above is bypassed
  silently. The capture succeeds and appears correct. Modern SQL Server Setup
  does not add `BUILTIN\Administrators` to `sysadmin` by default, but upgraded
  or locally customized instances may retain that mapping. Use
  `--expect-server-principal` to make that fail closed: it compares
  `ORIGINAL_LOGIN()` on the established session and returns `DBP1606E` before
  any catalog read.
- **A dedicated service account inherits no file access from whoever launched
  it.** It needs read on its own password file where one is used, and write on
  the `--out` and `--audit-log` paths. On Windows the file mode check is a
  no-op, so the ACL is the only thing protecting a credential file; see
  `DBP1605W` and SECURITY.md.

PostgreSQL and MySQL do not accept `--auth-mode integrated` at all and reject
it with `DBP1005E`, so this section applies only to SQL Server.

RLS, dynamic data masking, Always Encrypted, and explicit object/column
`DENY` rules still apply. Do not add `UNMASK`, key access, `IMPERSONATE`, or
`sysadmin` automatically. Decide whether the approved synthetic population is
the protected representation the reader actually sees; otherwise use a
separately approved scrubbed source.

`db_datareader` also grants `SELECT` on `sys.sql_expression_dependencies`, so a
`db_datareader` account needs only the `msdb` grant for the enhanced profile.
Note that `VIEW DEFINITION`, required at every tier, lets the account read
stored module text outside this command even when `--artifact-detail none` is
used.

## 4. Cloud-managed service IAM and integration

The managed services for the three current engines are in scope as deployment
and authentication profiles. They are not additional database engines, and a
successful capture against self-managed PostgreSQL, MySQL, or SQL Server does
not qualify the corresponding managed service automatically.

This section covers the current mainstream mappings:

| Engine | AWS | Azure | Google Cloud |
|---|---|---|---|
| PostgreSQL | Amazon RDS for PostgreSQL and Aurora PostgreSQL | Azure Database for PostgreSQL Flexible Server | Cloud SQL for PostgreSQL |
| MySQL | Amazon RDS for MySQL and Aurora MySQL | Azure Database for MySQL Flexible Server | Cloud SQL for MySQL |
| SQL Server | Amazon RDS for SQL Server | Azure SQL Database and Azure SQL Managed Instance | Cloud SQL for SQL Server |

Each service/version/authentication combination is a separate qualification
row. Apply the underlying engine/version grant family above, then add only the
cloud permission required by the selected connection path below. Do not infer
support for a provider's compatibility service, proxy, directory integration,
or new major version merely because it speaks the same wire protocol.

The runtime cloud minimum can be reduced to this decision table:

| Managed connection path | Runtime permission outside the database |
|---|---|
| Any listed service, direct native database password/token already supplied | None |
| RDS/Aurora PostgreSQL or MySQL with IAM database authentication | `rds-db:connect` on one exact instance/cluster/proxy database-user ARN; invoke with `--auth-mode cloud-token --tls-mode verify-full` |
| Azure PostgreSQL/MySQL with an externally generated Entra token | No Azure resource RBAC role; the Entra identity must exist as the database principal; invoke with `--auth-mode cloud-token --tls-mode verify-full` |
| Azure SQL with an externally generated Entra token | No Azure resource RBAC role; the Entra identity must exist as the database principal; invoke with `--auth-mode entra-token --tls-mode verify-full` |
| Cloud SQL PostgreSQL/MySQL direct IAM database authentication | Exact: `cloudsql.instances.login`; built-in alternative: `roles/cloudsql.instanceUser` with the caveat below; invoke with `--auth-mode cloud-token --tls-mode verify-full` |
| Cloud SQL Auth Proxy/language connector for PostgreSQL, MySQL, or SQL Server | `roles/cloudsql.client`, constrained to the approved instance where possible |
| Optional external secret-fetching wrapper | One-secret read permission from the provider-specific table below |

### 4.1 Authority separation

Production deployment must distinguish four identities or permission sets:

| Authority | Runtime requirement | Holder |
|---|---|---|
| Database reader | The engine grants documented above | The database principal used by `dbwarp-blueprint` |
| Token or connector identity | Only the cloud login/connect permissions below, and only when cloud IAM authentication or a provider connector is selected | The external token helper, proxy, sidecar, or workload identity |
| Secret reader | Optional read of one credential secret | The external wrapper that writes the protected password/token file |
| Provisioning administrator | Enables authentication, creates principals, and configures network reachability | Existing DBA/cloud/IaC operator; never the collector identity |

`dbwarp-blueprint` opens the supplied database URI and reads explicitly named
credential/TLS files. It does not enumerate managed instances, call cloud
control-plane APIs, query instance metadata, generate or refresh cloud tokens,
or retrieve secrets. Therefore a native database password in a protected file
plus an already reachable endpoint requires **no cloud IAM permission at
runtime**. The wrapper or proxy may need cloud IAM; the binary does not inherit
that need merely because the database is managed.

Likewise, no collector identity needs AWS RDS describe permissions, Azure
`Reader`, or Google Cloud SQL Viewer merely to run a capture. The operator
supplies the endpoint and target database explicitly.

### 4.2 MySQL cloud-token client boundary

RDS/Aurora MySQL IAM, Azure Database for MySQL Entra, and manual Cloud SQL for
MySQL IAM authentication require the client to send the token with the
`mysql_clear_password` cleartext exchange over TLS. The word "cleartext"
describes the authentication payload inside TLS; it is not permission to send
the token over an unverified or plaintext network connection.

The current client implements this boundary explicitly:

- `--auth-mode cloud-token` is accepted only for PostgreSQL/MySQL;
- exactly one externally generated token must be supplied through
  `--password-file` or `--password-env`;
- `--tls-mode verify-full` is mandatory, so both the certificate chain and
  endpoint hostname are checked;
- MySQL enables the driver's cleartext plugin on its single catalog/sampling
  connection only in this mode;
- normal `sql-auth` keeps the plugin disabled; URI query parameters are not a
  supported bypass;
- the audit identifies the cloud-token authentication path without recording
  the token.

Local tests prove the activation and rejection logic, including that the
normal MySQL builder remains disabled. That is implementation evidence, not
provider qualification. RDS/Aurora MySQL IAM, Azure MySQL Entra, and Cloud SQL
MySQL IAM can now proceed to separate live service/version tests; none is a
support claim until those tests pass.

### 4.3 AWS RDS and Aurora

With native PostgreSQL, MySQL, or SQL Server database credentials, the runtime
cloud permission set is empty. Security-group, subnet, and RDS authentication
configuration must already permit the connection.

RDS IAM database authentication is available for the PostgreSQL and MySQL
families, but not RDS for SQL Server. When it is selected, the exact workload
permission is one action against one database user on one instance or cluster:

```json
{
  "Version": "2012-10-17",
  "Statement": [{
    "Effect": "Allow",
    "Action": "rds-db:connect",
    "Resource": "arn:aws:rds-db:REGION:ACCOUNT_ID:dbuser:RESOURCE_ID/dbwarp_blueprint"
  }]
}
```

Use the RDS `DbiResourceId`, the Aurora `DbClusterResourceId`, or the RDS Proxy
resource ID appropriate to the actual path. Do not wildcard the resource to
`dbuser:*/*`. The database side must also map the same user:

- on PostgreSQL, the DBA grants `rds_iam` to the login role and then applies
  the PostgreSQL grants from this document;
- on MySQL, the DBA creates/configures the account with the AWS IAM
  authentication plugin and then applies the MySQL grants from this document.

Both PostgreSQL and MySQL IAM paths can proceed to live qualification. Use
`--auth-mode cloud-token --tls-mode verify-full`; for MySQL, the explicit mode
activates the required plugin only inside verified TLS.

The token generator runs outside `dbwarp-blueprint`; the short-lived token is
passed through exactly one of `--password-file` or `--password-env` with
`--auth-mode cloud-token --tls-mode verify-full`. Cross-account role assumption can additionally require
`sts:AssumeRole` on the specific authentication role, but that is an identity
bootstrap permission, not permission to read any database object.

AWS does not provide a managed role that is as narrow as this one-user
`rds-db:connect` policy. A reusable customer-managed policy attached to the
workload role is the low-maintenance option. `AmazonRDSReadOnlyAccess` is a
control-plane inventory role and does not replace `rds-db:connect` or the
database grants; `AmazonRDSFullAccess` is administrative and must not be used.

RDS for SQL Server must use a qualified SQL Server authentication path. Do not
grant `rds-db:connect` for it: that action cannot turn an RDS SQL Server login
into IAM database authentication.

### 4.4 Azure managed databases

Microsoft Entra authentication is a database-principal mapping, not Azure
resource RBAC data access. The collector identity does not need `SQL Server
Contributor`, `SQL DB Contributor`, `Reader`, `Contributor`, or an Azure
database-service contributor role in order to execute database queries.

| Service | Token audience/resource | Database-side mapping | `dbwarp-blueprint` input |
|---|---|---|---|
| Azure Database for PostgreSQL Flexible Server | `https://ossrdbms-aad.database.windows.net` | Create the Entra principal with the service's `pgaadauth_create_principal` flow, then apply the PostgreSQL grants above | Token via exactly one `--password-file`/`--password-env`; `--auth-mode cloud-token --tls-mode verify-full` |
| Azure Database for MySQL Flexible Server | `https://ossrdbms-aad.database.windows.net` | Create the Entra principal with `CREATE AADUSER`, then apply the MySQL grants above | Token via exactly one `--password-file`/`--password-env`; `--auth-mode cloud-token --tls-mode verify-full` |
| Azure SQL Database or Managed Instance | `https://database.windows.net/` | `CREATE USER ... FROM EXTERNAL PROVIDER` or the service-appropriate Entra login mapping, then apply the SQL Server grants above | Token via `--azure-token-file` or `--azure-token-env`; `--auth-mode entra-token --tls-mode verify-full` |

A managed identity can request a token for itself without being assigned a
management role on the database resource. Its association with the compute
workload, tenant policy, and the database principal mapping establish the
identity and access boundary. Token acquisition and refresh remain the
wrapper's responsibility because the current binary does not call Azure CLI,
Azure Identity, or the instance metadata endpoint.

Azure Database for MySQL Entra authentication can proceed to live
qualification through the explicit cloud-token mode. A token working in the
`mysql` CLI is still not sufficient proof for this binary.

Some Entra principal-creation flows require the **managed database server's
own identity**, or the provisioning identity, to read Microsoft Graph. For
example, Azure SQL can require `User.Read.All`, `GroupMember.Read.All`, or
`Application.Read.All` according to the principal type, while Managed Instance
uses `Directory Readers`; Azure Database for MySQL has similar server-identity
directory requirements. Those permissions enable one-time identity resolution
by the service. Assign them to the documented server/provisioning identity,
not to the collector workload.

For low-maintenance database access, an Entra group can be mapped once where
the exact service supports it, and the database grants can be attached to that
principal. Group membership then becomes the operational access switch. This
does not make an Azure management role a database reader.

### 4.5 Google Cloud SQL

Direct native-password connections need no Google Cloud IAM permission after
network reachability is established. IAM database authentication is supported
for Cloud SQL for PostgreSQL and MySQL; Cloud SQL for SQL Server still uses a
database credential even when the network connection runs through the Cloud
SQL Auth Proxy.

| Connection path | Exact cloud permissions | Predefined-role option and caveat |
|---|---|---|
| Direct PostgreSQL/MySQL IAM database login | `cloudsql.instances.login` | `roles/cloudsql.instanceUser` is the provider-supported shortcut, but its current definition also includes `cloudsql.instances.get` and Cloud SQL recommender read permissions; it is broader than token login alone |
| Cloud SQL Auth Proxy or language connector, all three engines | `cloudsql.instances.connect` and `cloudsql.instances.get` | `roles/cloudsql.client` contains those two permissions; its normal project-level grant reaches all Cloud SQL instances, so constrain the binding to the approved instance with an IAM Condition where policy permits |
| Automatic IAM authentication through a proxy/connector, PostgreSQL/MySQL | Both rows above | Grant both predefined roles only when their documented extra scope is accepted |

For strict least privilege, use a custom role containing only
`cloudsql.instances.login` for a direct PostgreSQL/MySQL IAM login, then add
the IAM principal to the Cloud SQL instance and apply the PostgreSQL/MySQL
database grants above. This custom-role path requires live proof because the
provider documentation normally instructs operators to grant
`roles/cloudsql.instanceUser`.

The custom-role PostgreSQL and direct MySQL paths can proceed to live
qualification. Automatic IAM authentication through the Auth Proxy remains a
separate path and must be proven with the exact local PostgreSQL/MySQL
handshake before it can be offered.

`roles/cloudsql.client` is a reasonable built-in connector role: it authorizes
the encrypted Cloud SQL connection and instance lookup but grants no table
read by itself. Its project-wide default scope is the caveat. Do not substitute
`roles/cloudsql.admin`, `roles/cloudsql.editor`, or
`roles/cloudsql.viewer`; those add control-plane capabilities irrelevant to
blueprint capture.

The extra `cloudsql.instances.executeSql` in
`roles/cloudsql.instanceUser` does not override PostgreSQL/MySQL object grants,
so it does not turn the collector into a database administrator. It does add
an alternate Cloud SQL Data API query surface beyond the direct login
requirement when that API is enabled. The Data API executes with the mapped
database principal's grants, so the no-write/no-DDL grant sets in this document
still apply. That is often an acceptable read-only convenience, but it must be
disclosed and some customers will correctly require the one-permission custom
role instead.

The Auth Proxy or connector holds the Google identity and exposes a local
socket/TCP endpoint. `dbwarp-blueprint` connects to that endpoint and still
authenticates as the approved database principal. On Cloud SQL for SQL Server,
the proxy role cannot replace the SQL Server user or any grants in this
document.

### 4.6 Optional secret-store access

No secret-store role is required when an operator or workload mechanism
already supplies the protected credential file. If an external wrapper fetches
the credential, give **that wrapper identity**, not the database principal, the
following smallest practical access:

| Provider | Exact or built-in runtime access | Scope caveat |
|---|---|---|
| AWS Secrets Manager | `secretsmanager:GetSecretValue` on one secret ARN; add `kms:Decrypt` on the one customer-managed KMS key only when that key protects the secret | Do not use `SecretsManagerReadWrite`; `DescribeSecret` and list permissions are not needed to fetch a known secret value |
| Azure Key Vault | `Key Vault Secrets User` on the application vault, or legacy access-policy `get` when that permission model is in use | The built-in role reads every secret in its assigned scope; use a dedicated per-application/per-environment vault or an approved narrower assignment |
| Google Secret Manager | `roles/secretmanager.secretAccessor` on one secret | Do not grant it at project scope, which would expose every secret in the project |

The wrapper must write the credential with the file protections required by
`AUTH.md`, invoke the binary, and remove or rotate it according to the
operator's secret-handling policy. These roles are optional integration
permissions, not part of the database collector's mandatory role set.

### 4.7 Provisioning and network permissions are not runtime roles

An administrator may need to enable IAM/Entra authentication, set an Entra
administrator, create a cloud IAM database user, enable an API, establish a
private endpoint, authorize a network, or change a security-group/firewall
rule. The exact actions depend on the customer's existing network and IaC
model, so there is no honest universal provisioning-role minimum.

Use the customer's existing scoped DBA, identity, network, and deployment
operators for those changes. Broad shortcuts such as AWS RDS full access,
Azure `Contributor`/`Network Contributor`, Google Cloud SQL Admin, project IAM
administration, or directory administration may be convenient for a human or
deployment pipeline, but they are never runtime requirements for
`dbwarp-blueprint` and must never be attached to its workload identity.

Network reachability is a prerequisite, not a database data privilege. Once a
route and allowed endpoint already exist, the collector needs no permission to
modify VPCs/VNets, security groups, firewalls, private endpoints, DNS, service
networking, or authorized networks.

## 5. Acceptance and control gates

### 5.1 Real-world boundaries that grants do not fix

The query audit also found cases where a DBA can grant exactly the documented
permissions and the resulting blueprint can still be incomplete or biased. These
must be acceptance checks, not reasons to broaden the account:

- PostgreSQL's mandatory table query currently includes only `relkind = 'r'`.
  A partitioned parent (`relkind = 'p'`) is not a table-structure record, although
  ordinary leaf partitions can appear. Partitioned fixtures are therefore a
  product-qualification gate, not a permission problem.
- PostgreSQL `reltuples` and MySQL InnoDB `TABLE_ROWS` are optimizer estimates;
  stale statistics can produce coarse row totals even with full read access.
- MySQL and SQL Server Tier 2 use natural/first-row order rather than a random
  sample. Clustered keys, tenant ordering, and time ordering can bias values.
- MySQL invisible columns require a live qualification case because the
  catalog walk and a bounded all-column projection do not necessarily expose the same positional
  column set.
- SQL Server's DMV reports database storage counts independently of the rows
  visible through RLS. Dynamic data masking and client-side encryption can
  also change what the sampler observes without causing a permission error.
- All engines can acquire new tables during a long-running capture. The broad
  reader shortcuts cover future objects automatically, while exact grants need
  default privileges or role maintenance. In either case, reconcile the
  inventory against a stable expected snapshot.

Do not grant ownership, RLS bypass, unmasking, decryption-key access, or
administrative roles to make one of these cases disappear. Either accept the
protected population explicitly, capture from an approved scrubbed replica,
or fail the synthetic-copy-readiness gate.

### 5.2 Synthetic-copy-ready acceptance gate

Permissions are sufficient only when all of the following are true for the
actual capture:

1. Run with `--measure-compression --yes` and a sampling wall-time budget large
   enough to reach every in-scope table.
2. Reconcile the captured table count with an independently reviewed expected
   count. This detects silent metadata filtering.
3. Require a table-level compression block with `measured = true`, a recognized
   `sample_encoding`, and a nonzero `sample_rows` value for every nonempty
   table.
4. Require sampled cardinality for the columns needed by unique/index/FK and
   relationship generation. For MySQL, also require observed length statistics
   for every nonempty variable-width indexed column.
5. Reject `DBP1406W` (sampling budget exhausted), `DBP1407W` (table sample
   unavailable), and `DBP1408W` (column style sample unavailable) when the
   affected object is in scope.
6. Reconcile primary/unique indexes and foreign-key counts with expected
   metadata; generation planning depends on their exact column ordinals.
7. Confirm that row-level security, views, tenant predicates, and replica
   filtering did not change the approved source population.
8. Keep the engine version, command, output, and audit together as the run
   record.

Empty tables legitimately have no compression block. Sampling permission does
not repair stale or absent optimizer statistics, and first-`N` sampling on
MySQL and SQL Server can remain biased even when every grant is correct.

### 5.3 Explicitly unnecessary permissions

Do not grant any of the following merely to run the current table-structure or
synthetic-copy-ready capture:

- PostgreSQL `SUPERUSER`, `pg_monitor`, `pg_read_all_stats`, `BYPASSRLS`,
  schema `CREATE`, table write, or direct sequence privileges (`pg_read_all_data`
  is the documented convenience exception when its broader read scope is
  approved);
- MySQL `ALL`, `PROCESS`, `FILE`, `LOCK TABLES`, DDL, write, or replication
  privileges (the enhanced profile is the documented exception: `TRIGGER` and
  `EVENT` are DDL-capable and `SHOW_ROUTINE` is global);
- SQL Server `sysadmin`, `db_owner`, `CONTROL`, `IMPERSONATE`, `VIEW SERVER
  STATE`, SQL Agent roles, DDL, or write privileges (`db_datareader` is the
  documented per-database convenience exception);
- cloud control-plane inventory or administration roles, including AWS RDS
  read/full access, Azure `Reader`/`Contributor` and database contributor
  roles, or Google Cloud SQL Viewer/Editor/Admin. The documented
  `rds-db:connect`, Cloud SQL login/connector, and optional one-secret reader
  permissions are the only runtime cloud exceptions for their respective
  paths.

### 5.4 Qualification evidence and remaining release gate

This document is source-grounded and has live local qualification across the
current non-managed support matrix. Qualification status:

- direct script proof covers PostgreSQL 17.11, MySQL 9.7.0, SQL Server 2019
  (15.0.4480), and SQL Server 2022 (16.0.4265), including negative controls;
- the scoped-grant model is verified against PostgreSQL 13.23, 14.24, 15.19,
  16.15, 17.11, and 18.6; MySQL 8.0.46, 8.4.11, and 9.7.0; and SQL Server
  2019 (15.0.4480), 2022 (16.0.4265), and 2025 (17.0.4075);
- the built-in-role model is verified over the same twelve targets.
  PostgreSQL 13 uses scoped grants because `pg_read_all_data` starts at
  PostgreSQL 14; MySQL retains its shipped database-object grants because its
  scripts do not offer a predefined-role toggle; SQL Server uses
  `db_datareader`;
- published artifacts carry their own provenance and checksums; match the
  evidence you rely on to the exact artifact under review;
- preserve the documented exclusion difference: PostgreSQL
  `pg_read_all_data` cannot withhold one table because PostgreSQL has no `DENY`,
  while SQL Server `DENY` can override `db_datareader`. Both outcomes are
  asserted under test so the limitation cannot change silently;
- qualify every managed-service row separately with the exact service name,
  engine version, TLS mode, authentication mode, and any proxy/connector
  version recorded in the run evidence;
- prove AWS RDS/Aurora IAM authentication with the one-user
  `rds-db:connect` ARN, and prove that a denied or wildcard-free ARN for a
  different database user/instance cannot connect; for MySQL, prove that the
  explicit cloud-token mode succeeds and normal sql-auth does not enable the
  cleartext plugin;
- prove Azure PostgreSQL, Azure MySQL, and Azure SQL token paths with no Azure
  resource contributor/reader role on the collector identity; for Azure
  MySQL, record proof that the Rust client negotiates the required cleartext
  plugin only over `verify-full` TLS;
- prove Cloud SQL PostgreSQL/MySQL manual IAM login with the exact custom
  `cloudsql.instances.login` role and compare it with
  `roles/cloudsql.instanceUser`; prove proxy paths with only
  `roles/cloudsql.client` and an instance-constrained IAM Condition; accept
  manual and automatic MySQL IAM as separate rows only after each exact client
  handshake succeeds;
- when a secret-store wrapper is offered, prove one-secret access and a denied
  read of a second secret; do not treat a secret already supplied by the
  operator as requiring cloud secret-store IAM;
- on PostgreSQL, prove that the convenience account cannot connect to any
  unapproved database in a shared cluster before approving
  `pg_read_all_data`;
- include empty, partitioned, row-security-filtered, permission-denied, and
  mid-run revocation cases, plus MySQL invisible columns and SQL Server masked
  columns;
- test `--artifact-detail none` separately from the default summary path; do
  not let a best-effort artifact query hide a table-structure permission failure;
- treat any engine version, managed service, or authentication path that is
  not listed as verified above as unqualified until its own evidence exists.

## 6. Evidence and references

### 6.1 Source map

- `postgresql/`, `mysql/`, `sqlserver-2019/`, `sqlserver-2022/` in this
  folder: the executable `basic`, `standard`, and `enhanced` grant sets;
  `README.md` here: tier/CLI mapping and live verification evidence.
- [`src/engine_pg.rs`](../../src/engine_pg.rs): PostgreSQL catalog reads and bounded
  sampling.
- [`src/engine_mysql.rs`](../../src/engine_mysql.rs): MySQL `INFORMATION_SCHEMA`
  reads, capability probing, bounded sampling, and mode-gated
  `mysql_clear_password` activation on the connection pool.
- [`src/engine_mssql.rs`](../../src/engine_mssql.rs): SQL Server catalog/DMV reads
  and bounded sampling.
- [`src/main.rs`](../../src/main.rs): engine-aware authentication resolution,
  exactly-one token-source validation, and `verify-full` enforcement for
  externally generated PostgreSQL/MySQL cloud tokens.
- [`AUTH.md`](../../AUTH.md): explicit managed-service invocations, credential
  inputs, cloud-permission separation, and the no-cloud-CLI/no-metadata-service
  boundary.
- [`docs/DBA_REVIEW_GUIDE.md`](../../docs/DBA_REVIEW_GUIDE.md): the
  one-database-connection and no-cloud-API operating boundary.
- [`src/statistics.rs`](../../src/statistics.rs): sampled cardinality to index/FK
  relationship inference.
- [`crates/dbwarp-blueprint-core/src/generator.rs`](../../crates/dbwarp-blueprint-core/src/generator.rs):
  synthetic value generation from captured type, length, cardinality, and
  compression signals.
- [`crates/dbwarp-blueprint-core/src/generation_plan.rs`](../../crates/dbwarp-blueprint-core/src/generation_plan.rs):
  table, unique-key, and foreign-key generation planning.

### 6.2 Vendor references

#### PostgreSQL

- [PostgreSQL privileges](https://www.postgresql.org/docs/current/ddl-priv.html)
- [PostgreSQL predefined roles](https://www.postgresql.org/docs/current/predefined-roles.html)
- [PostgreSQL 14 introduction of `pg_read_all_data`](https://www.postgresql.org/docs/14/release-14.html)
- [PostgreSQL `pg_stats` visibility](https://www.postgresql.org/docs/current/view-pg-stats.html)
- [PostgreSQL 13 `ANALYZE` owner/DBA boundary](https://www.postgresql.org/docs/13/sql-analyze.html)
- [Current PostgreSQL `ANALYZE` permissions and automatic analysis](https://www.postgresql.org/docs/current/sql-analyze.html)
- [PostgreSQL planner row-count statistics are approximate](https://www.postgresql.org/docs/current/planner-stats.html)
- [PostgreSQL 13 `pg_subscription` catalog and protected `subconninfo`](https://www.postgresql.org/docs/13/catalog-pg-subscription.html)
- [PostgreSQL versioning policy](https://www.postgresql.org/support/versioning/)

#### MySQL

- [MySQL `INFORMATION_SCHEMA` and privilege filtering](https://dev.mysql.com/doc/refman/8.0/en/information-schema-introduction.html)
- [MySQL roles and default-role activation](https://dev.mysql.com/doc/refman/8.4/en/roles.html)
- [MySQL reserved internal accounts](https://dev.mysql.com/doc/refman/8.4/en/reserved-accounts.html)
- [MySQL `ANALYZE TABLE` privileges and locking](https://dev.mysql.com/doc/refman/8.4/en/analyze-table.html)
- [MySQL `INFORMATION_SCHEMA.TABLES` row-count accuracy](https://dev.mysql.com/doc/refman/8.4/en/information-schema-tables-table.html)
- [MySQL release model](https://dev.mysql.com/doc/refman/8.4/en/mysql-releases.html)

#### SQL Server

- [SQL Server fixed database roles](https://learn.microsoft.com/en-us/sql/relational-databases/security/authentication-access/database-level-roles)
- [SQL Server Setup and explicit Windows administrator provisioning](https://learn.microsoft.com/en-us/sql/sql-server/install/instance-configuration)
- [SQL Server `sys.dm_db_partition_stats` permissions](https://learn.microsoft.com/en-us/sql/relational-databases/system-dynamic-management-views/sys-dm-db-partition-stats-transact-sql)
- [SQL Server `UPDATE STATISTICS` permissions](https://learn.microsoft.com/en-us/sql/t-sql/statements/update-statistics-transact-sql)
- [SQL Server metadata visibility](https://learn.microsoft.com/en-us/sql/relational-databases/security/metadata-visibility-configuration)

#### AWS

- [Amazon RDS IAM database authentication policy](https://docs.aws.amazon.com/AmazonRDS/latest/UserGuide/UsingWithRDS.IAMDBAuth.IAMPolicy.html)
- [Amazon RDS IAM database-account setup](https://docs.aws.amazon.com/AmazonRDS/latest/UserGuide/UsingWithRDS.IAMDBAuth.DBAccounts.html)
- [RDS MySQL IAM client and cleartext-plugin requirement](https://docs.aws.amazon.com/AmazonRDS/latest/UserGuide/UsingWithRDS.IAMDBAuth.Connecting.AWSCLI.html)
- [Aurora MySQL IAM client and cleartext-plugin requirement](https://docs.aws.amazon.com/AmazonRDS/latest/AuroraUserGuide/UsingWithRDS.IAMDBAuth.Connecting.AWSCLI.html)
- [Amazon RDS engine authentication options](https://docs.aws.amazon.com/AmazonRDS/latest/UserGuide/database-authentication.html)
- [AWS Secrets Manager one-secret policy examples](https://docs.aws.amazon.com/secretsmanager/latest/userguide/auth-and-access_iam-policies.html)

#### Azure

- [Azure SQL Microsoft Entra authentication and RBAC boundary](https://learn.microsoft.com/en-us/azure/azure-sql/database/authentication-aad-overview?view=azuresql)
- [Azure Database for PostgreSQL managed-identity authentication](https://learn.microsoft.com/en-us/azure/postgresql/security/security-connect-with-managed-identity)
- [Azure Database for MySQL Microsoft Entra authentication](https://learn.microsoft.com/en-us/azure/mysql/security/security-how-to-entra)
- [Azure Key Vault data-plane roles](https://learn.microsoft.com/en-us/azure/key-vault/general/rbac-guide)

#### Google Cloud

- [Cloud SQL PostgreSQL IAM database login](https://cloud.google.com/sql/docs/postgres/iam-logins)
- [Cloud SQL MySQL IAM authentication](https://cloud.google.com/sql/docs/mysql/iam-authentication)
- [Cloud SQL MySQL IAM login and cleartext-plugin requirement](https://cloud.google.com/sql/docs/mysql/iam-logins)
- [Cloud SQL roles and their current permissions](https://cloud.google.com/iam/docs/roles-permissions/cloudsql)
- [Cloud SQL Data API and database-principal enforcement](https://cloud.google.com/sql/docs/postgres/executesql-instance)
- [Cloud SQL instance-constrained IAM Conditions](https://cloud.google.com/sql/docs/postgres/iam-conditions)
- [Cloud SQL for SQL Server Auth Proxy permissions](https://cloud.google.com/sql/docs/sqlserver/connect-auth-proxy)
- [Google Secret Manager least-privilege access](https://cloud.google.com/secret-manager/docs/access-control)
