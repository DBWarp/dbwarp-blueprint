-- =============================================================================
-- dbwarp-blueprint least-privilege grants — PostgreSQL 13-18
-- Tier: BASIC  (catalog walk only; no customer rows are read)
-- =============================================================================
-- Authorizes this command:
--
--   dbwarp-blueprint --connect postgresql://dbwarp_blueprint_basic@HOST/DBNAME --schema app \
--       --password-file /etc/dbwarp/db.pass --artifact-detail none \
--       --out blueprint.toml --audit-log blueprint.audit.txt
--
-- What the account gets: LOGIN, CONNECT to this one database, and the
-- PUBLIC-readable catalogs the collector walks (pg_class, pg_namespace,
-- pg_attribute, pg_index, pg_am, pg_constraint, pg_stat_all_tables,
-- pg_table_size(oid), pg_indexes_size(oid), current_setting('server_version')).
-- It cannot read any user table, view, or sequence. pg_stats is empty for it,
-- so observed column widths are reported as 0 at this tier.
--
-- PRE-CAPTURE REQUIREMENTS (full engine/tier matrix: ../README.md): keep DDL
-- stable; record the selected schema and expected table/index/FK counts; have
-- the owner/DBA run ANALYZE app.table after a material load if auto-analyze is
-- not current. reltuples remains approximate. ANALYZE is DBA pre-work, never a
-- grant for this BASIC account. Repeat --schema for an approved multi-schema
-- application; an unresolved selector fails with DBP1420E.
--
-- NOT granted: SUPERUSER, CREATEDB, CREATEROLE, REPLICATION, BYPASSRLS, schema
-- USAGE, table SELECT, pg_read_all_data, pg_read_all_stats, pg_monitor.
--
-- Run ONCE, connected to the TARGET database as a superuser or its owner:
--     psql "postgresql://postgres@HOST/DBNAME" -f basic.sql
-- Requires psql 10+ (uses \if). Re-running is safe; it resets the password to
-- the value below.
-- =============================================================================

-- ---- EDIT THESE TWO LINES ---------------------------------------------------
\set role     dbwarp_blueprint_basic
\set password 'CHANGE-ME'
-- -----------------------------------------------------------------------------

\set ON_ERROR_STOP on
SELECT current_database() AS database \gset
SELECT :'password' = 'CHANGE-ME' AS password_unchanged \gset
\if :password_unchanged
  \echo 'ERROR: edit the password line at the top of this file before running it.'
  \quit
\endif

SELECT NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = :'role') AS role_missing \gset
\if :role_missing
  CREATE ROLE :"role" LOGIN PASSWORD :'password'
    NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS;
\else
  ALTER ROLE :"role" WITH LOGIN PASSWORD :'password'
    NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS;
\endif

-- PostgreSQL grants CONNECT to PUBLIC by default; this makes the intent
-- explicit and survives a later REVOKE CONNECT ... FROM PUBLIC.
GRANT CONNECT ON DATABASE :"database" TO :"role";

-- ---- verification (informational) ------------------------------------------
\echo
\echo '--- dbwarp-blueprint BASIC tier applied for role' :role 'on database' :database
SELECT r.rolname, r.rolcanlogin, r.rolsuper, r.rolbypassrls,
       has_database_privilege(r.rolname, :'database', 'CONNECT') AS can_connect
FROM pg_roles r WHERE r.rolname = :'role';
