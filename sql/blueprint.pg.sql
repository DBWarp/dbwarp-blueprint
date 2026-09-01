-- blueprint.pg.sql — pure-SQL alternative to the dbwarp-blueprint binary.
--
-- Run as:
--   psql -h <host> -U <user> -d <database> -A -t -X -v ON_ERROR_STOP=1 \
--        -f sql/blueprint.pg.sql > blueprint.json
--   python3 blueprint_format.py blueprint.json > blueprint.toml
-- To preserve anonymous labels across approved comparison runs, add:
--   --anonymization-key-file /path/to/protected-32-byte-or-64-hex-key
--
-- This script is read-only (catalog SELECTs only) and emits one large JSON
-- object describing the schema shape. The companion blueprint_format.py script
-- normalizes the JSON into the canonical anonymized Blueprint TOML format.
--
-- This script is the floor of trust: any DBA who refuses to run a
-- third-party binary can read every line below in their own editor and
-- execute it via their existing psql install. The separate stdlib-only Python
-- normalizer is also short enough to review before it is run.
--
-- Notes on what this collects:
--   - Per table: schema, name, reltuples, table_bytes, index_bytes, last analyze
--   - Per column: ordinal, type string, nullable, avg_width
--   - Per index: method, uniqueness, column ordinals
--   - FK edges: from-table, to-table, column ordinals
--
-- What this does NOT collect:
--   - Row content
--   - Trigger / rule / RLS / function bodies
--   - A schema subset: the query covers every non-system schema in the
--     connected database. Use the Rust collector when only selected schemas
--     are approved.
--
-- Real names ARE present in this output — anonymization is performed by
-- blueprint_format.py at the next step. The customer can review this JSON
-- before running the normalizer if they want to verify which tables /
-- columns it covered.
SELECT json_build_object(
  'schema_version', 2,
  'engine', 'postgresql',
  'engine_version', format(
    '%s.%s',
    current_setting('server_version_num')::integer / 10000,
    current_setting('server_version_num')::integer % 10000
  ),
  'tables', COALESCE((
    SELECT json_agg(json_build_object(
      'oid',          c.oid::int8,
      'schema_name',  n.nspname,
      'table_name',   c.relname,
      'reltuples',    c.reltuples::float8,
      'table_bytes',  pg_table_size(c.oid)::int8,
      'index_bytes',  pg_indexes_size(c.oid)::int8,
      'last_analyze', COALESCE(s.last_analyze, s.last_autoanalyze),
      'columns', COALESCE((
        SELECT json_agg(json_build_object(
          'attnum',   a.attnum,
          'type',     format_type(a.atttypid, a.atttypmod),
          'not_null', a.attnotnull,
          'avg_width', COALESCE(st.avg_width, 0)::int4
        ) ORDER BY a.attnum)
        FROM pg_attribute a
        LEFT JOIN pg_stats st
               ON st.schemaname = n.nspname
              AND st.tablename  = c.relname
              AND st.attname    = a.attname
        WHERE a.attrelid = c.oid
          AND a.attnum > 0
          AND NOT a.attisdropped
      ), '[]'::json),
      'indexes', COALESCE((
        SELECT json_agg(json_build_object(
          'name',     ic.relname,
          'method',   am.amname,
          'unique',   i.indisunique,
          'col_ords', i.indkey::int2[]
        ))
        FROM pg_index i
        JOIN pg_class ic ON ic.oid  = i.indexrelid
        JOIN pg_am am   ON am.oid = ic.relam
        WHERE i.indrelid = c.oid
      ), '[]'::json),
      'fks', COALESCE((
        SELECT json_agg(json_build_object(
          'to_oid',  con.confrelid::int8,
          'col_ords', con.conkey::int2[],
          'to_col_ords', con.confkey::int2[]
        ))
        FROM pg_constraint con
        WHERE con.contype = 'f'
          AND con.conrelid = c.oid
      ), '[]'::json)
    ))
    FROM pg_class c
    JOIN pg_namespace n ON n.oid = c.relnamespace
    LEFT JOIN pg_stat_all_tables s ON s.relid = c.oid
    WHERE c.relkind = 'r'
      AND n.nspname NOT IN ('pg_catalog','information_schema')
      AND n.nspname NOT LIKE 'pg_temp_%'
      AND n.nspname NOT LIKE 'pg_toast_temp_%'
  ), '[]'::json)
);
