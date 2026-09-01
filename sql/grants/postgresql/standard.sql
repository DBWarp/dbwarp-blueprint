-- =============================================================================
-- dbwarp-blueprint least-privilege grants — PostgreSQL 13-18
-- Tier: STANDARD  (catalog walk + bounded row samples => synthetic-copy-ready)
-- =============================================================================
-- Authorizes this command:
--
--   dbwarp-blueprint --connect postgresql://dbwarp_blueprint_standard@HOST/DBNAME --schema app \
--       --password-file /etc/dbwarp/db.pass --artifact-detail none \
--       --measure-compression --yes --sample-rows 1000 --max-wall-secs 300 \
--       --out blueprint.toml --audit-log blueprint.audit.txt
--
-- Adds to BASIC: USAGE on the in-scope schemas and SELECT on their tables, so
-- pg_stats rows become visible (observed widths) and the bounded samples run:
--   SELECT LEFT(col::text,N), ... FROM s.t TABLESAMPLE ... LIMIT N
--   (bounded projection with LIMIT N fallback)
--   single-column TABLESAMPLE ... LIMIT 32 style probes
--
-- Scope rule: make --schema match the edited schema list below. Omitting the
-- selector retains the broader walk of every visible non-system schema. A
-- selected table the account cannot read yields DBP1407W and fails the
-- synthetic-copy-ready acceptance gate.
--
-- PRE-CAPTURE REQUIREMENTS (full engine/tier matrix: ../README.md): keep DDL
-- stable; record expected table/index/FK counts; have the owner/DBA run ANALYZE
-- after material loads when auto-analyze is not current; confirm RLS exposes
-- the approved population. Start with sample-rows 1000 / max-wall-secs 300 and
-- raise both for higher requested detail. Do not grant MAINTAIN/BYPASSRLS to
-- this account. Repeat --schema when approved relationships span schemas.
--
-- NOT granted: any write, DDL, sequence, replication, superuser, BYPASSRLS,
-- pg_read_all_stats or pg_monitor privilege. Row-level security still applies.
-- Note: "ON ALL TABLES IN SCHEMA" also covers views, materialized views and
-- foreign tables in those schemas (read-only; broader than the literal
-- ordinary-table minimum).
--
-- Run ONCE, connected to the TARGET database as a superuser or its owner:
--     psql "postgresql://postgres@HOST/DBNAME" -f standard.sql
-- Requires psql 10+ (uses \if). Re-running is safe.
-- =============================================================================

-- ---- EDIT THESE LINES -------------------------------------------------------
\set role     dbwarp_blueprint_standard
\set password 'CHANGE-ME'
\set schemas  ALL
--   ALL  = every non-system schema in this database (for an unscoped run).
--   Or an explicit comma-separated identifier list, e.g.  \set schemas 'app, billing'
--   Quote mixed-case names inside the list:                \set schemas '"App", billing'
\set use_pg_read_all_data false
--   true = grant the predefined pg_read_all_data role (PostgreSQL 14+) instead
--   of per-schema grants. Low maintenance (covers future tables) but it is
--   CLUSTER-WIDE: it reads every table/view/sequence in every database this
--   role can CONNECT to. Use only on a dedicated instance or with CONNECT
--   revoked from PUBLIC on the other databases.
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

GRANT CONNECT ON DATABASE :"database" TO :"role";

\if :use_pg_read_all_data
  GRANT pg_read_all_data TO :"role";
\else
  SELECT :'schemas' = 'ALL' AS all_schemas \gset
  \if :all_schemas
    -- psql variables are not expanded inside $$ bodies; pass the role via a GUC.
    SELECT set_config('dbwarp.grant_role', :'role', false);
    DO $$
    DECLARE s text;
    BEGIN
      FOR s IN SELECT nspname FROM pg_namespace
               WHERE nspname <> 'information_schema' AND nspname NOT LIKE 'pg\_%'
               ORDER BY nspname
      LOOP
        EXECUTE format('GRANT USAGE ON SCHEMA %I TO %I', s, current_setting('dbwarp.grant_role'));
        EXECUTE format('GRANT SELECT ON ALL TABLES IN SCHEMA %I TO %I', s, current_setting('dbwarp.grant_role'));
      END LOOP;
    END $$;
  \else
    GRANT USAGE ON SCHEMA :schemas TO :"role";
    GRANT SELECT ON ALL TABLES IN SCHEMA :schemas TO :"role";
  \endif
\endif

-- Optional (run as EACH table-owning role, not included in the minimum):
-- future tables created by that owner become readable without a new grant.
--   ALTER DEFAULT PRIVILEGES FOR ROLE application_owner IN SCHEMA app
--       GRANT SELECT ON TABLES TO dbwarp_blueprint_standard;

-- ---- verification (informational) ------------------------------------------
\echo
\echo '--- dbwarp-blueprint STANDARD tier applied for role' :role 'on database' :database
\echo '--- ordinary tables per schema and how many the role can SELECT:'
SELECT n.nspname AS schema,
       count(*) AS tables,
       count(*) FILTER (WHERE has_table_privilege(:'role', c.oid, 'SELECT')) AS readable
FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
WHERE c.relkind = 'r' AND n.nspname <> 'information_schema' AND n.nspname NOT LIKE 'pg\_%'
GROUP BY 1 ORDER BY 1;
