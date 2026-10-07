-- =============================================================================
-- dbwarp-blueprint least-privilege grants: MySQL 8.0.20+ / 8.4 / 9.7
-- Tier: ENHANCED  (STANDARD + non-table artifact inventory, --artifact-detail analyzed)
-- =============================================================================
-- Authorizes this command:
--
--   dbwarp-blueprint --connect mysql://dbwarp_blueprint_enhanced@HOST/appdb --schema appdb \
--       --password-file /etc/dbwarp/db.pass --artifact-detail analyzed \
--       --measure-compression --yes --sample-rows 1000 --max-wall-secs 300 \
--       --out blueprint.toml --audit-log blueprint.audit.txt
--
-- MySQL couples catalog VISIBILITY to object privileges, so each artifact
-- family needs its own privilege:
--   information_schema.VIEWS / VIEW_TABLE_USAGE + view definitions  -> SHOW VIEW
--   information_schema.ROUTINES rows + definitions                   -> SHOW_ROUTINE (global, 8.0.20+)
--   information_schema.TRIGGERS                                      -> TRIGGER on the schema
--   information_schema.EVENTS                                        -> EVENT on the schema
--   performance_schema.user_defined_functions (loadable UDF census)  -> SELECT on that one table
--   mysql.component (count only; UDF origin classification)          -> SELECT on that one table
--   FEDERATED tables                                                 -> covered by SELECT (TABLES.ENGINE)
-- EXECUTE is NOT needed (it only reveals routine rows without definitions).
--
-- !! READ BEFORE APPROVING !!
--   TRIGGER and EVENT are DDL-capable privileges: MySQL has no read-only way
--   to see triggers/events, so this account could also CREATE/DROP triggers in
--   the granted schemas and CREATE/ALTER/DROP events there. SHOW_ROUTINE is
--   global: it shows the definitions of ALL routines on the server.
--   Schema SELECT also lets this principal follow FEDERATED tables in another
--   client. Blueprint inventories them but deliberately never follows them.
--   If that is unacceptable, use standard.sql and run --artifact-detail none,
--   or accept a privilege_filtered inventory (the blueprint records what was
--   unreadable; absence is never reported as proof).
--   Below 8.0.20 SHOW_ROUTINE does not exist; the only substitute is global
--   SELECT ON *.*, which is broader than this tier and is not recommended.
--
-- Note: the collector reports artifact visibility = "full" for MySQL only when
-- the account holds ALL PRIVILEGES ON *.*; with this least-privilege set it
-- still records "privilege_filtered" even though every catalog was read.
--
-- NOT granted: PROCESS, FILE, LOCK TABLES, SUPER, SHOW DATABASES, table DDL,
-- DML, replication, SELECT on *.* .
--
-- PRE-CAPTURE REQUIREMENTS (full engine/tier matrix: ../README.md): complete
-- STANDARD preparation and make --schema match each approved GRANT below.
-- Explicitly approve DDL-capable TRIGGER/EVENT, global SHOW_ROUTINE, the
-- global UDF census and the count-only component read. Database-wide/global
-- artifact evidence remains in scope.
-- Start with sample-rows 1000 / max-wall-secs 300 and raise both for higher
-- requested detail; never add global ALL merely to change the visibility label.
--
-- EDIT: account name, host pattern, and schema name(s). Then run ONCE
-- as an administrative account:      mysql -u root -p < enhanced.sql
-- MySQL prints a generated password when it creates the account. Capture that
-- result securely and put only the password in the collector password file.
-- Re-running does not reset an existing password.
-- =============================================================================

CREATE USER IF NOT EXISTS 'dbwarp_blueprint_enhanced'@'collector-host' IDENTIFIED BY RANDOM PASSWORD;

GRANT SELECT, SHOW VIEW, TRIGGER, EVENT ON `appdb`.* TO 'dbwarp_blueprint_enhanced'@'collector-host';
-- GRANT SELECT, SHOW VIEW, TRIGGER, EVENT ON `second_schema`.* TO 'dbwarp_blueprint_enhanced'@'collector-host';

GRANT SHOW_ROUTINE ON *.* TO 'dbwarp_blueprint_enhanced'@'collector-host';
GRANT SELECT ON `performance_schema`.`user_defined_functions` TO 'dbwarp_blueprint_enhanced'@'collector-host';
-- Blueprint issues COUNT(*) only. MySQL cannot restrict this grant to that
-- aggregate, so another client using this credential could read component
-- names/URNs. Remove this line if that is unacceptable; capture continues
-- with a warning and conservatively unknown UDF origin.
GRANT SELECT ON `mysql`.`component` TO 'dbwarp_blueprint_enhanced'@'collector-host';

-- ---- verification (informational) ------------------------------------------
SHOW GRANTS FOR 'dbwarp_blueprint_enhanced'@'collector-host';
