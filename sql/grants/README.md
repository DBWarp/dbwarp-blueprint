# dbwarp-blueprint one-step DBA grant scripts: basic / standard / enhanced

Least-privilege grant sets for the three capture tiers, based on the catalog
and sampling queries implemented by the PostgreSQL, MySQL, and SQL Server
collectors. Choose the script for your engine version and approved capture tier.

Oracle 12.1/12.2, 19c, 21c, and 23ai/26ai use separate `minimum.sql` and
`basic.sql` catalogue-only scripts under `sql/grants/oracle-*`. Oracle capture
is supported as a preview; read
[`ORACLE_PREVIEW.md`](ORACLE_PREVIEW.md) before applying them.

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
scope and completeness checks); its catalog-only / standard / enhanced profiles
are the `basic` / `standard` / `enhanced` tiers here.

The supported-engine grant files are self-contained and idempotent. The DBA
edits the marked principal, scope, and password settings applicable to that
engine and runs the file once. Oracle uses the shared principal file and
lifecycle described in `ORACLE_PREVIEW.md` instead.
PostgreSQL and SQL Server files refuse to run while the password is still
`CHANGE-ME`. MySQL
8.0.18 and later generates a random initial password and returns it to the DBA;
the shipped scripts never contain a usable password literal. Protect any
edited script containing credentials, never commit or share it, and remove it
after use according to your secure-disposal policy.


## Tier definitions (what the tool reads → which CLI flags → what a DBA grants)

| Tier | `dbwarp-blueprint` flags | Data collected | Table rows read? |
|---|---|---|---|
| **basic** | `--schema NAME --artifact-detail none` (add `--yes` for non-interactive runs: the pre-flight `Continue? [y/N]` prompt is shown at every tier) | tables, columns (type family, declared capacity), indexes, foreign keys, optimizer row estimates, table/index bytes, engine version, and best-effort coarse capacity evidence reported by the connected database. Observed widths are 0 (PG `pg_stats` hidden; no samples). | **No** |
| **standard** | `--schema NAME --artifact-detail none --measure-compression --yes --sample-rows N --max-wall-secs S` | basic + server-bounded all-column projections and single-column style probes → compression ratios, sampled cardinality, observed lengths. | Bounded samples, in memory only |
| **enhanced** | `--schema NAME --artifact-detail analyzed --measure-compression --yes ...` (`summary`/`graph` need strictly fewer reads but the same grants) | standard + non-table inventory: views, routines, triggers, scheduled objects, types, policies/rules, extensions/FDW servers/publications (PG), UDF registrations (MySQL), keys/certificates/credentials, partition objects, filegroups, linked servers, Agent jobs (SQL Server), anonymous dependency graph, and language-feature bands derived from definitions read transiently. | Bounded samples + object definitions (transient) |

The source-environment block never inspects the machine running the collector.
It uses only coarse, banded values returned by the connected database and
records the evidence basis and any unreadable catalog explicitly. See
**Source-environment evidence** below.

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

| Engine / version family | Tier | Required preparation before capture | Fidelity expectation and caveat |
|---|---|---|---|
| PostgreSQL 13-18 | **basic** | Freeze application DDL for the run; record the selected schemas and expected table/index/FK counts. Confirm every populated selected table has been auto-analyzed since its last bulk load; otherwise have its owner/DBA run `ANALYZE schema.table`. | Table rows come from approximate `pg_class.reltuples`. No row samples are read, and `pg_stats` widths are hidden without table `SELECT`; use for inventory/coarse sizing only. PostgreSQL 13 requires an owner/superuser to analyze; later versions may support delegated `MAINTAIN` according to their version policy. |
| PostgreSQL 13-18 | **standard** | Complete the basic preparation. Confirm the approved account's RLS-visible population is the intended population. Use at least `--sample-rows 1000 --max-wall-secs 300`; increase both for many/large tables or when higher-detail output is requested. | Bounded samples add null density, cardinality, observed lengths, and compression evidence. `ANALYZE` remains approximate; sampling and RLS can still bias the result. Do not grant `BYPASSRLS`. |
| PostgreSQL 13-18 | **enhanced** | Complete the standard preparation. Inventory approved cross-schema dependencies and decide whether database-wide artifact counts are acceptable before using `--artifact-detail analyzed`. | Same table-statistics fidelity as standard, plus transient definition reads and anonymous artifact/dependency evidence. No extra PostgreSQL grant is required, but the broader information boundary must be approved. |
| MySQL 8.0 / 8.4 / 9.7 | **basic** | Freeze application DDL for the run; record the selected databases and expected table/index/FK counts. After a material bulk change, have a DBA run `ANALYZE TABLE schema.table` only if its operational cost is approved. | InnoDB `INFORMATION_SCHEMA.TABLES.TABLE_ROWS` is a rough estimate even after analysis; basic has no row sample. `ANALYZE TABLE` requires `SELECT` and `INSERT`, updates dictionary statistics, and can take a read lock, so it must not be granted to or run by the Blueprint account. |
| MySQL 8.0 / 8.4 / 9.7 | **standard** | Complete the basic preparation. Confirm RLS-equivalent application filtering, masking, replica lag, primary-key distribution, and fallback first-row ordering are acceptable. Use at least `--sample-rows 1000 --max-wall-secs 300`, with larger values for higher requested detail. | When an eligible numeric primary key is present, bounded samples use disjoint range windows; otherwise they fall back to first-`N`. Both run sequentially on the catalog session and may still be biased by key distribution, tenant, or time order. Do not create histograms solely for Blueprint: the current collector does not read MySQL histogram objects. |
| MySQL 8.0 / 8.4 / 9.7 | **enhanced** | Complete the standard preparation. Obtain explicit approval for DDL-capable `TRIGGER`/`EVENT`, global `SHOW_ROUTINE`, the global UDF census, and Blueprint's count-only query to `mysql.component` before running analyzed detail. The table-level `SELECT` grant also allows another client using this credential to read component names/URNs; omit that grant and accept unknown UDF origin if this is not acceptable. | Same table-statistics fidelity as standard. Definitions and anonymous artifact topology are added; when readable, the component count lets Blueprint distinguish built-in UDFs from potentially customer-installed code without the tool collecting component names. Least-privilege MySQL still reports privilege-filtered visibility unless the account is globally privileged; do not broaden it merely to change that label. |
| SQL Server 2019 | **basic** | Freeze application DDL for the run; record the selected schemas and expected table/index/FK counts. Confirm the source is the intended primary/replica and that partition maintenance is quiescent. Do **not** run `UPDATE STATISTICS` solely for Blueprint. | Rows and pages come from `sys.dm_db_partition_stats`, not optimizer histograms. The DMV row count is approximate; `UPDATE STATISTICS` does not improve the counter the collector reads and requires broader `ALTER` authority. |
| SQL Server 2019 | **standard** | Complete the basic preparation. Confirm RLS, dynamic masking, Always Encrypted, explicit `DENY`, and replica filtering yield the approved sample population. Use at least `--sample-rows 1000 --max-wall-secs 300`, increasing both for higher detail. | Bounded `TOP (N)` samples add value-shape evidence but may be biased by physical/clustered ordering. Never add `UNMASK`, key access, or RLS bypass merely to raise fidelity. |
| SQL Server 2019 | **enhanced** | Complete the standard preparation. Approve transient module-definition reads, the database dependency view, the `msdb` Agent-job census, and the server-level `VIEW SERVER STATE` batch (or remove that batch and accept unknown CPU/memory bands). | Same table-statistics fidelity as standard plus anonymous artifacts/dependencies. `VIEW DEFINITION` allows module text to be read by the account in other clients too. |
| SQL Server 2022 / 2025 | **basic** | Apply the SQL Server 2019 basic preparation, but use the 2022+ script so the DMV grant is `VIEW DATABASE PERFORMANCE STATE`. | Same measurement behavior as 2019; `VIEW SECURITY DEFINITION` is not a substitute for table metadata visibility. |
| SQL Server 2022 / 2025 | **standard** | Apply the SQL Server 2019 standard preparation and the 2022+ grant family. | Same first-`N`, RLS/masking/encryption, and counter caveats as 2019. |
| SQL Server 2022 / 2025 | **enhanced** | Apply the SQL Server 2019 enhanced preparation and the 2022+ grant family; its server-level batch grants `VIEW SERVER PERFORMANCE STATE`. | Same definition/dependency/Agent-job boundary as 2019. |

For a more detailed request, raise the sample and wall-time budgets together;
do not respond by broadening privileges. A standard run is complete only
when every nonempty selected table was sampled, the expected inventory and
relationships reconcile, and no in-scope `DBP1406W`, `DBP1407W`, `DBP1408W`,
or `DBP1423W` remains. The final audit fidelity estimate describes evidence
coverage, not measured error against source truth.

## Minimum grants per engine (as applied by the scripts)

| | PostgreSQL | MySQL | SQL Server 2019 | SQL Server 2022 / 2025 |
|---|---|---|---|---|
| **basic** | `CONNECT` on the database (standard `pg_catalog` ACLs do the rest) | `REFERENCES ON schema.*` per in-scope schema | `CONNECT`, `VIEW DEFINITION`, `VIEW DATABASE STATE` in the database | `CONNECT`, `VIEW DEFINITION`, `VIEW DATABASE PERFORMANCE STATE` |
| **standard** | + `USAGE` on schemas, `SELECT ON ALL TABLES IN SCHEMA` (default: every non-system schema; switch: `pg_read_all_data`, PG 14+) | `SELECT ON schema.*` instead of `REFERENCES` | + object-level `SELECT` on current local tables in the selected schemas (switch: `db_datareader`) | same |
| **enhanced** | **nothing more**: every artifact catalog and `pg_get_*def` function is PUBLIC-readable | `SELECT, SHOW VIEW, TRIGGER, EVENT ON schema.*` + `SHOW_ROUTINE ON *.*` + `SELECT ON performance_schema.user_defined_functions` + `SELECT ON mysql.component` | + `SELECT ON sys.sql_expression_dependencies`; user in `msdb` + `SELECT ON msdb.dbo.sysjobs`; server-level `VIEW SERVER STATE` in a separate, removable batch | same, with `VIEW SERVER PERFORMANCE STATE` as the server-level permission |

The scripts do not grant table-data writes, `CREATE`/`ALTER`/`DROP` on tables
or schemas, ownership, impersonation, RLS bypass, unmasking, key control,
superuser/sysadmin, or cloud IAM. The only server-state permission is the one
in the SQL Server enhanced script's separate batch (`VIEW SERVER STATE` on
2019, `VIEW SERVER PERFORMANCE STATE` on 2022/2025); no capture requires it,
and the basic and standard scripts grant none. Effective privileges can still come
from pre-existing role membership or `PUBLIC` ACLs; review the principal after
provisioning. In particular, PostgreSQL 13 and 14 installations created with
their default ACLs can allow `PUBLIC` to create objects in schema `public`.
The scripts do not change that cluster-wide policy. MySQL enhanced is the
narrow exception to the scripts' general no-DDL posture:
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

## Operational limitations

- PostgreSQL 13 uses scoped grants; `pg_read_all_data` is available from 14.
  Enhanced capture needs no additional PostgreSQL grants beyond standard.
- SQL Server 2022/2025 still requires `VIEW DEFINITION` for table visibility;
  `VIEW SECURITY DEFINITION` is not a substitute. Enhanced dependency and
  Agent-job reads need the explicit grants listed above.
- MySQL enhanced grants `TRIGGER` and `EVENT`, which are DDL-capable, and
  global `SHOW_ROUTINE`. Approve these broader capabilities explicitly.
- Row-level security can hide `pg_stats` widths. Do not add `BYPASSRLS` merely
  to improve the capture. Use the approved visible population or an approved replica.
- Every tier prompts for consent; automation must pass `--yes`.

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

## Source-environment evidence

The tool never reads CPU or memory from the workstation running
`dbwarp-blueprint`. It makes one best-effort database query and emits only
closed capacity bands plus the evidence basis:

- PostgreSQL reads `shared_buffers` from `pg_settings`. This is database buffer
  configuration, not physical server memory; CPU remains unknown.
- MySQL reads `@@innodb_buffer_pool_size`. This is InnoDB buffer configuration,
  not physical server memory; CPU remains unknown.
- SQL Server reads `SERVERPROPERTY('EngineEdition')` to distinguish a known
  managed-service edition from self-managed software. Only for a recognized
  self-managed edition does it then attempt `sys.dm_os_sys_info` for
  operating-system-visible logical CPU and memory bands. Managed editions do
  not use that host-level DMV as database-resource evidence.

PostgreSQL and MySQL need no additional grant for these reads and retain them
at every tier. SQL Server requests its capacity DMV only with
`--artifact-detail graph` or `analyzed`. Basic and standard captures do not
issue the query and record CPU/memory capacity as `not-requested`. The DMV,
when applicable to self-managed software, needs server-wide visibility. The
enhanced script grants `VIEW SERVER STATE` on SQL Server 2019 or `VIEW SERVER
PERFORMANCE STATE` on SQL Server 2022/2025 in a separate server-level batch,
because enhanced is the tier that collects this additional operating-system
evidence. Those permissions expose server-wide operational metadata. A DBA who
does not accept that removes the batch; the capture still completes and leaves
the bands unknown. They are never a capture prerequisite.

## Topology evidence

Every capture, including `--artifact-detail none`, also runs one bounded
topology probe. It returns only booleans and counts; no host names, endpoints,
member identities, or replication channel names are selected.

- PostgreSQL reads `pg_is_in_recovery()` and whether the `citus` extension is
  present in `pg_extension`, then a row count from `pg_stat_wal_receiver` on a
  standby or `pg_stat_replication` on a primary. Only when Citus is installed
  does it count rows in `pg_dist_local_group`, `pg_dist_partition`, and
  `pg_dist_node`.
- MySQL checks `information_schema.TABLES` for the Performance Schema
  replication tables, then, where they exist and the account can read them,
  counts Group Replication members and roles in
  `performance_schema.replication_group_members` (comparing member IDs with
  `@@server_uuid` on the server, so no identity is returned), counts replica
  channels in `performance_schema.replication_connection_status`, and reads
  `SHOW GLOBAL VARIABLES LIKE 'wsrep_on'`.
- SQL Server reads `SERVERPROPERTY('IsHadrEnabled')`. Only when HADR is enabled
  does it count rows in `sys.dm_hadr_database_replica_states` and
  `sys.dm_hadr_availability_replica_states`. Those views need `VIEW SERVER
  STATE` (2019) or `VIEW SERVER PERFORMANCE STATE` (2022/2025), which only the
  enhanced script grants, so availability-group topology is recorded as
  unknown at the basic and standard tiers.

A refused or failed probe never stops the capture. The audit records a
warning and the Blueprint leaves that topology unknown.

No provider or orchestrator API is called. No `PROCESS`,
`pg_read_all_settings`, operating-system login, or cloud IAM permission is
required by the shipped scripts.
