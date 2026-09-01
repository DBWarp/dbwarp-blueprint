-- =============================================================================
-- dbwarp-blueprint least-privilege grants — MySQL 8.0 / 8.4 / 9.7
-- Tier: BASIC  (catalog walk only; no customer rows are read)
-- =============================================================================
-- Authorizes this command:
--
--   dbwarp-blueprint --connect mysql://dbwarp_blueprint_basic@HOST/appdb --schema appdb \
--       --password-file /etc/dbwarp/db.pass --artifact-detail none \
--       --out blueprint.toml --audit-log blueprint.audit.txt
--
-- Reads only VERSION() and INFORMATION_SCHEMA rows (TABLES, COLUMNS,
-- STATISTICS, KEY_COLUMN_USAGE, REFERENTIAL_CONSTRAINTS, COLLATIONS).
-- INFORMATION_SCHEMA shows a table only to accounts holding SOME privilege on
-- it; REFERENCES is the one that exposes the full table metadata without
-- allowing any row read (verified: all five catalogs populated, SELECT on a
-- table denied with error 1142). SHOW VIEW alone is NOT enough (COLUMNS and
-- KEY_COLUMN_USAGE stay empty).
--
-- NOT granted: SELECT, PROCESS, FILE, SHOW DATABASES, SUPER, any DDL/DML,
-- anything on *.* . Never add CREATE/ALTER to this account: REFERENCES would
-- then let it name these tables in a FOREIGN KEY.
--
-- Scope rule: make --schema match the GRANT schema below. Omitting --schema
-- retains the broader walk of every visible non-system schema. Do not use *.* .
--
-- PRE-CAPTURE REQUIREMENTS (full engine/tier matrix: ../README.md): keep DDL
-- stable and record expected table/index/FK counts. After a material load a DBA
-- may run ANALYZE TABLE appdb.table when its read-lock/dictionary-write cost is
-- approved; never grant the needed INSERT privilege to this BASIC account.
-- InnoDB TABLE_ROWS remains a rough estimate and this tier reads no row sample.
--
-- EDIT: account name, host pattern, and schema name(s). Then run ONCE
-- as an administrative account:      mysql -u root -p < basic.sql
-- Host pattern: the host/network the collector connects FROM ('%' = any).
-- MySQL prints a generated password when it creates the account. Capture that
-- result securely and put only the password in the collector password file.
-- Re-running is safe: IF NOT EXISTS does not reset an existing password.
-- =============================================================================

CREATE USER IF NOT EXISTS 'dbwarp_blueprint_basic'@'collector-host' IDENTIFIED BY RANDOM PASSWORD;

GRANT REFERENCES ON `appdb`.* TO 'dbwarp_blueprint_basic'@'collector-host';
-- GRANT REFERENCES ON `second_schema`.* TO 'dbwarp_blueprint_basic'@'collector-host';

-- To list every candidate user schema (what the collector would walk with a
-- broad grant), run and copy the lines you approve:
--   SELECT CONCAT('GRANT REFERENCES ON `', SCHEMA_NAME, '`.* TO ''dbwarp_blueprint_basic''@''collector-host'';')
--   FROM information_schema.SCHEMATA
--   WHERE SCHEMA_NAME NOT IN ('mysql','information_schema','performance_schema','sys');

-- ---- verification (informational) ------------------------------------------
SHOW GRANTS FOR 'dbwarp_blueprint_basic'@'collector-host';
