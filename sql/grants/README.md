# dbwarp-blueprint one-step DBA grant scripts — basic / standard / enhanced

Least-privilege grant sets for the three capture tiers. Derived by tracing
every catalog and row query in
`src/engine_pg.rs`, `src/engine_mysql.rs`, `src/engine_mssql.rs`, then
**verifying each script live** against the complete non-managed version
matrix. The coverage is summarized below.

## Files

| Engine | basic | standard | enhanced | Run with |
|---|---|---|---|---|
| PostgreSQL 13–18 | `postgresql/basic.sql` | `postgresql/standard.sql` | `postgresql/enhanced.sql` | `psql -d TARGET_DB -f FILE` (psql ≥ 10) |
| MySQL 8.0 / 8.4 / 9.7 | `mysql/basic.sql` | `mysql/standard.sql` | `mysql/enhanced.sql` | `mysql -u root -p < FILE` |
| SQL Server 2019 | `sqlserver-2019/basic.sql` | `sqlserver-2019/standard.sql` | `sqlserver-2019/enhanced.sql` | `sqlcmd -S HOST -E -i FILE` (or SSMS SQLCMD mode) |
| SQL Server 2022 / 2025 | `sqlserver-2022/basic.sql` | `sqlserver-2022/standard.sql` | `sqlserver-2022/enhanced.sql` | same |

After the capture, remove the dedicated accounts with the matching script in
[`../revoke/`](../revoke/). MySQL removal drops all three accounts and grants.
PostgreSQL revokes the grants from the connected target database and fails
closed if another database or default privilege still depends on a role. SQL
Server removes target and `msdb` users before their server logins; its header
explains the narrower contained-user procedure for Azure SQL Database. Review
the targets before running any removal script.

`DATABASE_PERMISSIONS.md` in this folder is the full DBA/security
review behind these scripts (profiles, query-to-permission mapping, cloud IAM,
acceptance gates); its catalog-only / synthetic-copy-ready / enhanced profiles
are the `basic` / `standard` / `enhanced` tiers here.

Each file is self-contained and idempotent: it creates its own principal when
absent — `dbwarp_blueprint_basic`, `dbwarp_blueprint_standard`, or
`dbwarp_blueprint_enhanced` — applies exactly the grants for its tier, and ends
with a verification query. Because the principals are distinct, all three
scripts can be applied to one server and tested side by side without clashing;
for a production capture, create only the tier that was approved. The DBA edits
the marked lines at the top (principal name, database/schema scope, and, for
PostgreSQL or SQL Server, password) and runs the file once. PostgreSQL and SQL
Server files refuse to run while the password is still `CHANGE-ME`. MySQL
8.0.18 and later generates a random initial password and returns it to the DBA;
the checked-in scripts never contain a usable password literal.


## Tier definitions (what the tool reads → which CLI flags → what a DBA grants)

| Tier | `dbwarp-blueprint` flags | Data collected | Customer rows read? |
|---|---|---|---|
| **basic** | `--schema NAME --artifact-detail none` (add `--yes` for non-interactive runs: the pre-flight `Continue? [y/N]` prompt is shown at every tier) | tables, columns (type family, declared capacity), indexes, foreign keys, optimizer row estimates, table/index bytes, engine version. Observed widths are 0 (PG `pg_stats` hidden; no samples). | **No** |
| **standard** | `--schema NAME --artifact-detail none --measure-compression --yes --sample-rows N --max-wall-secs S` | basic + server-bounded all-column projections and single-column style probes → compression ratios, sampled cardinality, observed lengths. These are the inputs `crates/dbwarp-blueprint-core` generation planning consumes, so this is the **synthetic-copy-ready** tier. | Bounded samples, in memory only |
| **enhanced** | `--schema NAME --artifact-detail analyzed --measure-compression --yes ...` (`summary`/`graph` need strictly fewer reads but the same grants) | standard + non-table inventory: views, routines, triggers, scheduled objects, types, policies/rules, extensions/FDW servers/publications (PG), UDF registrations (MySQL), keys/certificates/credentials, partition objects, filegroups, linked servers, Agent jobs (SQL Server), anonymous dependency graph, and language-feature bands derived from definitions read transiently. | Bounded samples + object definitions (transient) |

The tool reads **no host/OS information today**. See "Host discovery" below.

## Required scope and pre-capture preparation

The DBA and operator must agree the scope before applying a script. Pass one
`--schema NAME` for the approved application schema; repeat the option when a
complete application model genuinely spans schemas:

```text
dbwarp-blueprint --connect ENGINE_URI --schema app --schema shared_reference TIER_FLAGS
```

For PostgreSQL and SQL Server, the URI selects one database and `--schema`
selects schemas inside it. In MySQL, a schema is a database; the URI database
does not replace `--schema`. Names are matched by the connected engine using
its native catalog comparison. If any requested schema is absent or hidden by
the account, the run stops with `DBP1420E` and emits no Blueprint. Omitting
`--schema` preserves the broader, backward-compatible walk of every visible
non-system schema.

The selector limits emitted tables, row sampling, and schema-owned artifact
definitions. It does not suppress database-wide topology evidence or genuinely
global artifact families such as PostgreSQL extensions/publications, MySQL
loadable UDFs and global routine visibility, or SQL Server linked-server and
Agent-job censuses. The Blueprint records `selection-limited`; the audit adds
only `schema_selector_count` for this option, not its values. The redacted
connection URI still identifies the connected database (also the schema name
on MySQL). Select both
sides of an approved cross-schema foreign key or dependency; a relationship to
an unselected schema is intentionally absent.

The maintenance statements below are **DBA pre-work, not collector grants**.
Run them through the normal change process, with a suitable lock/load window;
never grant their write/maintenance permissions to the Blueprint account.

| Engine / qualified family | Tier | Required preparation before capture | Fidelity expectation and caveat |
|---|---|---|---|
| PostgreSQL 13-18 | **basic** | Freeze application DDL for the run; record the selected schemas and expected table/index/FK counts. Confirm every populated selected table has been auto-analyzed since its last bulk load; otherwise have its owner/DBA run `ANALYZE schema.table`. | Table rows come from approximate `pg_class.reltuples`. No row samples are read, and `pg_stats` widths are hidden without table `SELECT`; use for inventory/coarse sizing only. PostgreSQL 13 requires an owner/superuser to analyze; later versions may support delegated `MAINTAIN` according to their version policy. |
| PostgreSQL 13-18 | **standard** | Complete the basic preparation. Confirm the approved account's RLS-visible population is the intended population. Use at least `--sample-rows 1000 --max-wall-secs 300`; increase both for many/large tables or when higher-detail output is requested. | Bounded samples add null density, cardinality, observed lengths, and compression evidence. `ANALYZE` remains approximate; sampling and RLS can still bias the result. Do not grant `BYPASSRLS`. |
| PostgreSQL 13-18 | **enhanced** | Complete the standard preparation. Inventory approved cross-schema dependencies and decide whether database-wide artifact counts are acceptable before using `--artifact-detail analyzed`. | Same table-statistics fidelity as standard, plus transient definition reads and anonymous artifact/dependency evidence. No extra PostgreSQL grant is required, but the broader information boundary must be approved. |
| MySQL 8.0 / 8.4 / 9.7 | **basic** | Freeze application DDL for the run; record the selected databases and expected table/index/FK counts. After a material bulk change, have a DBA run `ANALYZE TABLE schema.table` only if its operational cost is approved. | InnoDB `INFORMATION_SCHEMA.TABLES.TABLE_ROWS` is a rough estimate even after analysis; basic has no row sample. `ANALYZE TABLE` requires `SELECT` and `INSERT`, updates dictionary statistics, and can take a read lock, so it must not be granted to or run by the Blueprint account. |
| MySQL 8.0 / 8.4 / 9.7 | **standard** | Complete the basic preparation. Confirm RLS-equivalent application filtering, masking, replica lag, and first-row ordering are acceptable. Use at least `--sample-rows 1000 --max-wall-secs 300`, with larger values for higher requested detail. | Bounded first-`N` samples run sequentially on the catalog session and add value-shape evidence, but may be biased by primary-key, tenant, or time order. Do not create histograms solely for Blueprint: the current collector does not read MySQL histogram objects. |
| MySQL 8.0 / 8.4 / 9.7 | **enhanced** | Complete the standard preparation. Obtain explicit approval for DDL-capable `TRIGGER`/`EVENT`, global `SHOW_ROUTINE`, and the global UDF census before running analyzed detail. | Same table-statistics fidelity as standard. Definitions and anonymous artifact topology are added, but least-privilege MySQL still reports privilege-filtered visibility unless the account is globally privileged; do not broaden it merely to change that label. |
| SQL Server 2019 | **basic** | Freeze application DDL for the run; record the selected schemas and expected table/index/FK counts. Confirm the source is the intended primary/replica and that partition maintenance is quiescent. Do **not** run `UPDATE STATISTICS` solely for Blueprint. | Rows and pages come from `sys.dm_db_partition_stats`, not optimizer histograms. The DMV row count is approximate; `UPDATE STATISTICS` does not improve the counter the collector reads and requires broader `ALTER` authority. |
| SQL Server 2019 | **standard** | Complete the basic preparation. Confirm RLS, dynamic masking, Always Encrypted, explicit `DENY`, and replica filtering yield the approved sample population. Use at least `--sample-rows 1000 --max-wall-secs 300`, increasing both for higher detail. | Bounded `TOP (N)` samples add value-shape evidence but may be biased by physical/clustered ordering. Never add `UNMASK`, key access, or RLS bypass merely to raise fidelity. |
| SQL Server 2019 | **enhanced** | Complete the standard preparation. Approve transient module-definition reads, the database dependency view, and the `msdb` Agent-job census. | Same table-statistics fidelity as standard plus anonymous artifacts/dependencies. `VIEW DEFINITION` allows module text to be read by the account in other clients too. |
| SQL Server 2022 / 2025 | **basic** | Apply the SQL Server 2019 basic preparation, but use the 2022+ script so the DMV grant is `VIEW DATABASE PERFORMANCE STATE`. | Same measurement behavior as 2019; `VIEW SECURITY DEFINITION` is not a substitute for table metadata visibility. |
| SQL Server 2022 / 2025 | **standard** | Apply the SQL Server 2019 standard preparation and the 2022+ grant family. | Same first-`N`, RLS/masking/encryption, and counter caveats as 2019. |
| SQL Server 2022 / 2025 | **enhanced** | Apply the SQL Server 2019 enhanced preparation and the 2022+ grant family. | Same definition/dependency/Agent-job boundary as 2019. |

For a more detailed request, raise the sample and wall-time budgets together;
do not respond by broadening privileges. A run is synthetic-copy-ready only
when every nonempty selected table was sampled, the expected inventory and
relationships reconcile, and no in-scope `DBP1406W`, `DBP1407W`, or
`DBP1408W` remains. The final audit fidelity estimate describes evidence
coverage, not measured error against source truth.

## Minimum grants per engine (as applied by the scripts)

| | PostgreSQL | MySQL | SQL Server 2019 | SQL Server 2022 / 2025 |
|---|---|---|---|---|
| **basic** | `CONNECT` on the database (standard `pg_catalog` ACLs do the rest) | `REFERENCES ON schema.*` per in-scope schema | `CONNECT`, `VIEW DEFINITION`, `VIEW DATABASE STATE` in the database | `CONNECT`, `VIEW DEFINITION`, `VIEW DATABASE PERFORMANCE STATE` |
| **standard** | + `USAGE` on schemas, `SELECT ON ALL TABLES IN SCHEMA` (default: every non-system schema; switch: `pg_read_all_data`, PG 14+) | `SELECT ON schema.*` instead of `REFERENCES` | + `SELECT ON SCHEMA::x` for every schema owning a user table (switch: `db_datareader`) | same |
| **enhanced** | **nothing more** — every artifact catalog and `pg_get_*def` function is PUBLIC-readable | `SELECT, SHOW VIEW, TRIGGER, EVENT ON schema.*` + `SHOW_ROUTINE ON *.*` + `SELECT ON performance_schema.user_defined_functions` | + `SELECT ON sys.sql_expression_dependencies`; user in `msdb` + `SELECT ON msdb.dbo.sysjobs` | same |

No tier grants table-data writes, `CREATE`/`ALTER`/`DROP` on tables or schemas,
ownership, impersonation, server state, RLS bypass, unmasking, key control,
superuser/sysadmin, or cloud IAM. MySQL enhanced is the narrow exception to
the general no-DDL posture:
`TRIGGER` and `EVENT` are DDL-capable metadata privileges used to inspect those
object families, and another client using the same principal could create or
drop triggers or events in the selected schema. Use standard when that
capability is not approved. Cloud token/connector permissions are unchanged
from `DATABASE_PERMISSIONS.md` §4.

SQL Server also reads `ORIGINAL_LOGIN()`, `SUSER_SNAME()`, and `USER_NAME()`
from the established session for local audit evidence. These built-ins require
no additional grant in any tier. Operators should pass
`--expect-server-principal PRINCIPAL` when the approved login is known; a
mismatch fails before catalog capture rather than broadening permissions.

## Verification evidence

Each script was applied with its placeholders substituted, then the binary was
run with that tier's flags. "Negative" rows run a lower-tier account with a
higher-tier CLI to prove the extra grants are necessary.

These grant sets are verified against PostgreSQL 13-18, MySQL 8.0/8.4/9.7, and
SQL Server 2019/2022/2025, covering both the scoped-grant and built-in-role
models on every applicable target. PostgreSQL 13 uses scoped grants because
`pg_read_all_data` starts at PostgreSQL 14; MySQL retains the shipped
database-object grant model because its scripts offer no predefined-role
toggle; SQL Server uses `db_datareader`. Published artifacts carry their own
provenance and checksums.

The scope contract is additionally exercised against PostgreSQL 18.6,
MySQL 8.4.11, and SQL Server 2022 (16.0.4265): basic,
standard, and enhanced each captured only the selected five-table fixture and
recorded `selection-limited` plus `schema_selector_count: 1`. A validly
connected request for a nonexistent schema failed with `DBP1420E`, wrote no
Blueprint, and did not copy the requested name into its audit on all three
engines. This is representative selector proof, not qualification of every
version in the matrix.

| Run | PG 17.11 | MySQL 9.7 | SQL 2022 | SQL 2019 |
|---|---|---|---|---|
| basic account, basic CLI | rc 0, 4 tables, no warnings | rc 0, 3 tables, none | rc 0, 4 tables, none | same as 2022 |
| standard account, standard CLI | rc 0, 21 compression blocks, `DBP1408W×5`¹ | rc 0, 15 blocks, none | rc 0, 16 blocks, none | same |
| enhanced account, enhanced CLI | `visibility="full"`, 17 catalogs read, 29 objects | 109 artifacts, all 7 catalogs read, no `DBP1410W` | `visibility="full"`, incl. `scheduled_job:1` from msdb | same |
| **negative:** standard acct + enhanced CLI | identical to enhanced (PG needs nothing extra) | routines/triggers/events silently absent; `catalogs_unreadable=["performance_schema.user_defined_functions"]`, `DBP1410W×2` | `catalogs_unreadable=["msdb.dbo.sysjobs","sys.sql_expression_dependencies"]`, `DBP1410W×4`, `visibility="privilege_filtered"` | same |
| **negative:** basic acct + standard CLI | `DBP1407W×12` (samples denied) | `DBP1407W×9` | `DBP1407W×12` | same |
| superuser comparison run, enhanced CLI | same measured facts except `len_avg` on the RLS-enabled table² | – | – | – |

¹ `DBP1408W "Style sample failed: error serializing parameter 0"` occurs
identically for a superuser: it is a known limitation of the column style
probe for enum/domain columns, not a permission gap.
² `pg_stats` hides rows of a table with row-level security from any role
without `BYPASSRLS`, so observed widths for that table are 0 for the collector
(and for its owner). Do not grant `BYPASSRLS` for this; accept it or capture
from a scrubbed replica.


## Verification findings

1. **SQL Server 2022+: `VIEW SECURITY DEFINITION` + `VIEW DATABASE PERFORMANCE
   STATE` does NOT make tables visible** (`sys.tables` returned 0 rows; only
   certificates/keys/credentials became visible). `VIEW DEFINITION` is still
   required for the basic walk on 2022/2025. The granular DMV permission alone
   is correct for `sys.dm_db_partition_stats`.
2. **SQL Server graph/analyzed needs two explicit grants**: `SELECT ON
   sys.sql_expression_dependencies` (only `db_owner` has it by default;
   `db_datareader` also covers it) and, for the Agent job census, a user in
   `msdb` with `SELECT ON dbo.sysjobs` — `SQLAgentReaderRole` is not enough
   because the code reads the base table rather than `sysjobs_view`.
3. **MySQL enhanced is not achievable with read-only privileges**: `TRIGGER`
   and `EVENT` (both DDL-capable) are the only way to see
   `information_schema.TRIGGERS`/`EVENTS`; `SHOW_ROUTINE` (global, 8.0.20+) is
   needed for routine definitions and reveals routines in **every** schema on
   the server (`EXECUTE` is unnecessary); `SHOW VIEW` is needed for view
   definitions but is insufficient for basic (leaves `COLUMNS` and
   `KEY_COLUMN_USAGE` empty). `REFERENCES` remains the correct basic privilege.
4. **PostgreSQL enhanced = standard.** A `CONNECT`-only role already sees all
   17 artifact catalogs and every definition function; `GRANT CONNECT` itself
   is implied by the default `PUBLIC` ACL but is kept explicit.
5. The basic tier is not prompt-free: automation must pass `--yes` (documented
   as harmless) or the run ends with `DBP1701E`.

## Scope caveats a DBA should know before approving

- `--schema` should match the edited schema list in the chosen grant script.
  Without it, all three collectors retain the broader walk of every visible
  non-system schema. Scripts still default to all user schemas so applying a
  script without editing its scope cannot silently under-grant an unscoped run.
- MySQL global `SHOW_ROUTINE` (enhanced) makes the routine census include
  out-of-scope schemas (anonymous counts only). MySQL reports
  `visibility="privilege_filtered"` for any account without `ALL ON *.*`.
- SQL Server `VIEW DEFINITION` (every tier) lets the account read module text
  in any client, even though the tool reads it only in `analyzed` mode.
- Row-level security, masking, and Always Encrypted still determine what the
  sampler sees; none of the scripts bypass them.
- The `pg_read_all_data` switch removes the ability to withhold a single table.
  The privilege comes from role membership and PostgreSQL has no `DENY`, so no
  `REVOKE` excludes an object from a member. If any table must stay out of the
  Blueprint, keep the per-schema grants. SQL Server's `db_datareader` switch
  does not have this problem, because `DENY` overrides a fixed role. See
  [DATABASE_PERMISSIONS.md](DATABASE_PERMISSIONS.md#dba-friendly-predefined-role-pg_read_all_data).

## Windows and domain principals

These scripts create a SQL login with a password. For `--auth-mode integrated`
on SQL Server, create the login `FROM WINDOWS` first and then apply the tier's
grants unchanged; only the login DDL differs. PostgreSQL and MySQL reject that
mode with `DBP1005E`, so it is a SQL Server path only.

The account the collector process runs as is the identity SQL Server sees. If
that process is an administrator and `BUILTIN\Administrators` is in `sysadmin`,
the session is sysadmin and the grants in these scripts are bypassed while the
capture still succeeds. Pass `--expect-server-principal` to make that fail
closed with `DBP1606E` before any catalog read.

Full DDL and the service-account caveats are in
[DATABASE_PERMISSIONS.md](DATABASE_PERMISSIONS.md#windows-and-domain-principals-for-integrated-authentication).

## Host-level information — not collected

The tool reads no host or operating-system information. The only server-level
values it reads are the engine version (PostgreSQL `server_version`, MySQL
`VERSION()`, SQL Server `ProductVersion`), the connected SQL
Server database's `compatibility_level`, and per-module `ANSI_NULLS` /
`QUOTED_IDENTIFIER` and per-routine `sql_mode` settings in `analyzed` mode. No
server-scoped permission (`VIEW SERVER STATE`, `PROCESS`, `pg_read_all_settings`,
or similar) is required or granted by any script.
