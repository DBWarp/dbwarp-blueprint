-- blueprint.mysql.sql — pure-SQL alternative to the dbwarp-blueprint binary for MySQL.
--
-- Run as:
--   mysql -h <host> -P <port> -u <user> -p --skip-column-names \
--         --silent <dbname> -e "SOURCE sql/blueprint.mysql.sql" > blueprint.json
--   python3 blueprint_format.py blueprint.json > blueprint.toml
-- To preserve anonymous labels across approved comparison runs, add:
--   --anonymization-key-file /path/to/protected-32-byte-or-64-hex-key
--
-- This script is read-only (information_schema only) and emits one large
-- JSON object describing the schema shape. The companion blueprint_format.py
-- script normalizes the JSON into the canonical anonymized Blueprint TOML.
--
-- Read every line below before running. The query joins
-- information_schema views and emits structural data only — no row content.
--
-- The bare -p prompts without placing the password in shell history or the
-- process list. Real names and declared enum/set members are present in the
-- JSON output; anonymization happens in blueprint_format.py at the next step.
-- Keep the JSON protected inside the source environment. By default the
-- normalizer uses a fresh secret key, so anonymous labels intentionally differ
-- between runs.
-- The query covers every user table in <dbname>; it has no schema-subset
-- selector. Use the Rust collector when only a narrower scope is approved.

SELECT JSON_OBJECT(
  'schema_version', 2,
  'engine',         'mysql',
  -- Keep only the numeric product version. VERSION() can include
  -- producer-controlled distribution or host text that must not leave the
  -- source environment in the intermediate JSON or final Blueprint.
  'engine_version', COALESCE(
    REGEXP_SUBSTR(VERSION(), '^[0-9]+[.][0-9]+[.][0-9]+'),
    'unknown'
  ),
  'tables', COALESCE((
    SELECT JSON_ARRAYAGG(JSON_OBJECT(
      'oid',          CONCAT(t.TABLE_SCHEMA, '.', t.TABLE_NAME),
      'schema_name',  t.TABLE_SCHEMA,
      'table_name',   t.TABLE_NAME,
      'reltuples',    COALESCE(t.TABLE_ROWS, 0),
      'table_bytes',  COALESCE(t.DATA_LENGTH, 0),
      'index_bytes',  COALESCE(t.INDEX_LENGTH, 0),
      'last_analyze', t.UPDATE_TIME,
      'columns', COALESCE((
        SELECT JSON_ARRAYAGG(JSON_OBJECT(
          'attnum',     c.ORDINAL_POSITION,
          'name',       c.COLUMN_NAME,
          'type',       c.COLUMN_TYPE,
          'not_null',   IF(c.IS_NULLABLE = 'NO', TRUE, FALSE),
          'avg_width',  COALESCE(c.CHARACTER_OCTET_LENGTH, 0)
        ))
        FROM information_schema.COLUMNS c
        WHERE c.TABLE_SCHEMA = t.TABLE_SCHEMA AND c.TABLE_NAME = t.TABLE_NAME
      ), JSON_ARRAY()),
      'indexes', COALESCE((
        SELECT JSON_ARRAYAGG(JSON_OBJECT(
          'name',       s.INDEX_NAME,
          'method',     s.INDEX_TYPE,
          'unique',     IF(s.NON_UNIQUE = 0, TRUE, FALSE),
          'seq',        s.SEQ_IN_INDEX,
          'col_name',   s.COLUMN_NAME
        ))
        FROM information_schema.STATISTICS s
        WHERE s.TABLE_SCHEMA = t.TABLE_SCHEMA AND s.TABLE_NAME = t.TABLE_NAME
      ), JSON_ARRAY()),
      'fks', COALESCE((
        SELECT JSON_ARRAYAGG(JSON_OBJECT(
          'to_oid',     CONCAT(k.REFERENCED_TABLE_SCHEMA, '.', k.REFERENCED_TABLE_NAME),
          'fk_name',    k.CONSTRAINT_NAME,
          'position',   k.ORDINAL_POSITION,
          'col_name',   k.COLUMN_NAME,
          'to_col_name', k.REFERENCED_COLUMN_NAME
        ))
        FROM information_schema.KEY_COLUMN_USAGE k
        WHERE k.TABLE_SCHEMA = t.TABLE_SCHEMA
          AND k.TABLE_NAME   = t.TABLE_NAME
          AND k.REFERENCED_TABLE_NAME IS NOT NULL
      ), JSON_ARRAY())
    ))
    FROM information_schema.TABLES t
    WHERE t.TABLE_TYPE = 'BASE TABLE'
      AND t.TABLE_SCHEMA NOT IN ('mysql','information_schema','performance_schema','sys')
  ), JSON_ARRAY())
) AS blueprint;
