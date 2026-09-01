-- blueprint.sqlserver.sql — pure-SQL alternative to the dbwarp-blueprint binary
-- for SQL Server.
--
-- Run as:
--   sqlcmd -S <host>,<port> -U <user> -No -d <db> -h -1 -W \
--          -i sql/blueprint.sqlserver.sql > blueprint.json
--   python3 blueprint_format.py --source-kind=production blueprint.json > blueprint.toml
-- To preserve anonymous labels across approved comparison runs, add:
--   --anonymization-key-file C:\path\to\protected-32-byte-or-64-hex-key
--
-- Omitting -P makes sqlcmd prompt for the password rather than putting it in
-- shell history or the process list. Read every line below before running. The
-- query reads sys.* views only;
-- no row content is exported. Real schema/table/column names are present
-- in this intermediate JSON; anonymization happens in blueprint_format.py at
-- the next step. By default the normalizer uses a fresh secret key, so
-- anonymous labels intentionally differ between runs.
--
-- Notes:
--   - SET NOCOUNT ON suppresses the (N rows affected) trailers.
--   - Output is one JSON document. Use sqlcmd -h -1 -W to remove headers
--     and trim trailing whitespace.
--   - The query covers every user table in <db>; it has no schema-subset
--     selector. Use the Rust collector when only selected schemas are approved.

SET NOCOUNT ON;

WITH tbl AS (
    SELECT
        SCHEMA_NAME(t.schema_id) AS schema_name,
        t.name                   AS table_name,
        t.object_id              AS oid,
        t.is_ms_shipped
    FROM sys.tables t
    WHERE t.is_ms_shipped = 0
),
tbl_size AS (
    SELECT
        b.schema_name,
        b.table_name,
        b.oid,
        COALESCE(SUM(CASE WHEN p.index_id IN (0,1) THEN p.row_count ELSE 0 END), 0) AS row_count,
        COALESCE(SUM(CASE WHEN p.index_id IN (0,1) THEN p.used_page_count ELSE 0 END) * 8 * 1024, 0) AS table_bytes,
        COALESCE(SUM(CASE WHEN p.index_id  > 1     THEN p.used_page_count ELSE 0 END) * 8 * 1024, 0) AS index_bytes,
        CAST(MAX(CASE WHEN p.index_id = 1 THEN 1 ELSE 0 END) AS BIT) AS has_clustered_index
    FROM tbl b
    LEFT JOIN sys.dm_db_partition_stats p ON p.object_id = b.oid
    GROUP BY b.schema_name, b.table_name, b.oid
)
SELECT (
    SELECT
        2 AS schema_version,
        'sqlserver' AS engine,
        CAST(SERVERPROPERTY('ProductVersion') AS NVARCHAR(128)) AS engine_version,
        (
            SELECT
                CAST(b.oid AS VARCHAR(32))                           AS [oid],
                b.schema_name                                         AS [schema_name],
                b.table_name                                          AS [table_name],
                b.row_count                                           AS [reltuples],
                b.table_bytes                                         AS [table_bytes],
                b.index_bytes                                         AS [index_bytes],
                b.has_clustered_index                                 AS [has_clustered_index],
                (
                    SELECT
                        c.column_id  AS [attnum],
                        c.name       AS [name],
                        -- Format the type fully so the Python normalizer
                        -- doesn't need engine-specific knowledge.
                        CASE LOWER(ty.name)
                            WHEN 'varchar'   THEN CASE WHEN c.max_length = -1 THEN 'varchar(max)'   ELSE 'varchar('   + CAST(c.max_length AS NVARCHAR(8)) + ')' END
                            WHEN 'char'      THEN CASE WHEN c.max_length = -1 THEN 'char(max)'      ELSE 'char('      + CAST(c.max_length AS NVARCHAR(8)) + ')' END
                            WHEN 'varbinary' THEN CASE WHEN c.max_length = -1 THEN 'varbinary(max)' ELSE 'varbinary(' + CAST(c.max_length AS NVARCHAR(8)) + ')' END
                            WHEN 'binary'    THEN CASE WHEN c.max_length = -1 THEN 'binary(max)'    ELSE 'binary('    + CAST(c.max_length AS NVARCHAR(8)) + ')' END
                            WHEN 'nvarchar'  THEN CASE WHEN c.max_length = -1 THEN 'nvarchar(max)'  ELSE 'nvarchar('  + CAST(c.max_length / 2 AS NVARCHAR(8)) + ')' END
                            WHEN 'nchar'     THEN CASE WHEN c.max_length = -1 THEN 'nchar(max)'     ELSE 'nchar('     + CAST(c.max_length / 2 AS NVARCHAR(8)) + ')' END
                            WHEN 'decimal'   THEN 'decimal('   + CAST(c.precision AS NVARCHAR(4)) + ',' + CAST(c.scale AS NVARCHAR(4)) + ')'
                            WHEN 'numeric'   THEN 'numeric('   + CAST(c.precision AS NVARCHAR(4)) + ',' + CAST(c.scale AS NVARCHAR(4)) + ')'
                            WHEN 'datetime2'      THEN 'datetime2('      + CAST(c.scale AS NVARCHAR(4)) + ')'
                            WHEN 'time'           THEN 'time('           + CAST(c.scale AS NVARCHAR(4)) + ')'
                            WHEN 'datetimeoffset' THEN 'datetimeoffset(' + CAST(c.scale AS NVARCHAR(4)) + ')'
                            ELSE LOWER(ty.name)
                        END AS [type],
                        IIF(c.is_nullable = 0, CAST(1 AS BIT), CAST(0 AS BIT)) AS [not_null],
                        CAST(c.max_length AS INT)                              AS [avg_width]
                    FROM sys.columns c
                    JOIN sys.types ty ON ty.user_type_id = c.user_type_id
                    WHERE c.object_id = b.oid
                    ORDER BY c.column_id
                    FOR JSON PATH
                ) AS [columns],
                (
                    SELECT
                        i.name        AS [name],
                        i.type_desc   AS [method],
                        i.is_unique   AS [unique],
                        ic.key_ordinal AS [seq],
                        col.name      AS [col_name]
                    FROM sys.indexes i
                    JOIN sys.index_columns ic ON ic.object_id = i.object_id AND ic.index_id = i.index_id
                    JOIN sys.columns col ON col.object_id = ic.object_id AND col.column_id = ic.column_id
                    WHERE i.object_id = b.oid
                      AND i.index_id > 0
                      AND ic.is_included_column = 0
                      AND i.name IS NOT NULL
                    ORDER BY i.name, ic.key_ordinal
                    FOR JSON PATH
                ) AS [indexes],
                (
                    SELECT
                        CAST(fk.referenced_object_id AS VARCHAR(32)) AS [to_oid],
                        CAST(fk.object_id AS VARCHAR(32))            AS [fk_id],
                        fc.constraint_column_id                     AS [position],
                        col.name                                    AS [col_name],
                        ref_col.name                                AS [to_col_name]
                    FROM sys.foreign_keys fk
                    JOIN sys.foreign_key_columns fc ON fc.constraint_object_id = fk.object_id
                    JOIN sys.columns col ON col.object_id = fc.parent_object_id AND col.column_id = fc.parent_column_id
                    JOIN sys.columns ref_col ON ref_col.object_id = fc.referenced_object_id AND ref_col.column_id = fc.referenced_column_id
                    WHERE fk.parent_object_id = b.oid
                    ORDER BY fk.object_id, fc.constraint_column_id
                    FOR JSON PATH
                ) AS [fks]
            FROM tbl_size b
            ORDER BY schema_name, table_name
            FOR JSON PATH
        ) AS [tables]
    FOR JSON PATH, WITHOUT_ARRAY_WRAPPER
);
