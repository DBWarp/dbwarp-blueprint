-- =============================================================================
-- dbwarp-blueprint least-privilege grants — MySQL 8.0 / 8.4 / 9.7
-- Tier: STANDARD  (catalog walk + bounded row samples => synthetic-copy-ready)
-- =============================================================================
-- Authorizes this command:
--
--   dbwarp-blueprint --connect mysql://dbwarp_blueprint_standard@HOST/appdb --schema appdb \
--       --password-file /etc/dbwarp/db.pass --artifact-detail none \
--       --measure-compression --yes --sample-rows 1000 --max-wall-secs 300 \
--       --out blueprint.toml --audit-log blueprint.audit.txt
--
-- SELECT replaces REFERENCES: it exposes the same INFORMATION_SCHEMA rows and
-- authorizes the bounded samples the collector runs on the same database session:
--   SELECT bounded-column-projection FROM `s`.`t` LIMIT N
--   plus separately bounded single-column style probes
-- (verified: metadata identical to BASIC, row sample returns rows).
--
-- NOT granted: PROCESS, FILE, LOCK TABLES, SHOW VIEW, EXECUTE, TRIGGER, EVENT,
-- any DDL/DML, replication, anything on *.* .
-- Note: schema-level SELECT also covers views in that schema (read-only,
-- broader than the base-table minimum).
--
-- Scope rule: make --schema match the GRANT schema below. Omitting --schema
-- retains the broader walk of every visible non-system schema; an unreadable
-- selected table yields DBP1407W.
--
-- PRE-CAPTURE REQUIREMENTS (full engine/tier matrix: ../README.md): keep DDL
-- stable; record expected counts; have a DBA
-- run ANALYZE TABLE after material loads only when its lock/write cost is
-- approved; confirm replica/filtering and first-row ordering are acceptable.
-- Start with sample-rows 1000 / max-wall-secs 300 and raise both for higher
-- requested detail. Do not grant INSERT or create histograms for Blueprint.
--
-- EDIT: account name, host pattern, and schema name(s). Then run ONCE
-- as an administrative account:      mysql -u root -p < standard.sql
-- MySQL prints a generated password when it creates the account. Capture that
-- result securely and put only the password in the collector password file.
-- Re-running does not reset an existing password.
-- =============================================================================

CREATE USER IF NOT EXISTS 'dbwarp_blueprint_standard'@'collector-host' IDENTIFIED BY RANDOM PASSWORD;

GRANT SELECT ON `appdb`.* TO 'dbwarp_blueprint_standard'@'collector-host';
-- GRANT SELECT ON `second_schema`.* TO 'dbwarp_blueprint_standard'@'collector-host';

-- Object-level alternative when only named tables are approved (each table):
--   GRANT SELECT ON `appdb`.`orders` TO 'dbwarp_blueprint_standard'@'collector-host';

-- ---- verification (informational) ------------------------------------------
SHOW GRANTS FOR 'dbwarp_blueprint_standard'@'collector-host';
