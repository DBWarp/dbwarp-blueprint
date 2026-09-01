-- =============================================================================
-- DBWarp Blueprint collector-role removal — PostgreSQL 13-18
-- =============================================================================
-- Run in the same target database used by the grant scripts. This revokes the
-- grants those scripts can apply, then drops each dedicated login role.
-- DROP ROLE fails closed if another database, owner default privilege, or
-- operator-added grant still depends on the role; inspect and revoke that
-- dependency explicitly rather than using DROP OWNED on a production system.

\set ON_ERROR_STOP on

DO $$
DECLARE
  role_name text;
  schema_name text;
BEGIN
  FOREACH role_name IN ARRAY ARRAY[
    'dbwarp_blueprint_basic',
    'dbwarp_blueprint_standard',
    'dbwarp_blueprint_enhanced'
  ]
  LOOP
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = role_name) THEN
      EXECUTE format('REVOKE CONNECT ON DATABASE %I FROM %I', current_database(), role_name);

      FOR schema_name IN
        SELECT nspname
        FROM pg_namespace
        WHERE nspname <> 'information_schema'
          AND nspname NOT LIKE 'pg\_%'
        ORDER BY nspname
      LOOP
        EXECUTE format(
          'REVOKE ALL PRIVILEGES ON ALL TABLES IN SCHEMA %I FROM %I',
          schema_name,
          role_name
        );
        EXECUTE format('REVOKE USAGE ON SCHEMA %I FROM %I', schema_name, role_name);
      END LOOP;

      IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'pg_read_all_data') THEN
        EXECUTE format('REVOKE pg_read_all_data FROM %I', role_name);
      END IF;

      EXECUTE format('DROP ROLE %I', role_name);
    END IF;
  END LOOP;
END $$;
