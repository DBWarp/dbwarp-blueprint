-- =============================================================================
-- dbwarp-blueprint least-privilege grants — SQL Server 2022 / 2025 (16.x, 17.x)
-- Tier: STANDARD
-- =============================================================================
-- Authorizes this command:
--
--   dbwarp-blueprint --connect sqlserver://dbwarp_blueprint_standard@HOST/DBNAME --schema app \
--       --password-file /etc/dbwarp/db.pass --artifact-detail none --measure-compression --yes --sample-rows 1000 --max-wall-secs 300 \
--       --out blueprint.toml --audit-log blueprint.audit.txt
--
-- STANDARD = BASIC + bounded row samples (synthetic-copy-ready). Adds SELECT on
-- the in-scope schemas so the collector can run on the same database session:
--   SELECT TOP (N) bounded-column-projection FROM [s].[t]
--   ORDER BY (SELECT NULL) + separately bounded column style probes
--
-- Scope rule: make --schema match the edited schema list below. Omitting the
-- selector retains the broader walk of every visible user schema. A selected
-- table it cannot read yields DBP1407W. Schema SELECT also covers views and table-valued
-- functions in the schema (read-only; broader than the base-table minimum).
-- db_datareader is the per-database low-maintenance alternative (all current
-- and future tables/views in this one database).
--
-- NOT granted: VIEW SERVER STATE, VIEW ANY DEFINITION, UNMASK, key/certificate
-- CONTROL, IMPERSONATE, ALTER, DML, DDL, sysadmin, db_owner.
-- Row-level security, dynamic data masking, Always Encrypted and explicit DENY
-- rules still apply to what the sampler sees.
--
-- PRE-CAPTURE REQUIREMENTS (full engine/tier matrix: ../README.md): keep DDL
-- and partition maintenance stable; record expected counts; confirm RLS,
-- masking, Always Encrypted, DENY and replica filtering produce the approved
-- population. Start with sample-rows 1000 / max-wall-secs 300 and raise both
-- for higher requested detail. Do not add UNMASK, key access or RLS bypass.
-- Common to every tier (verified live on SQL Server 2022 16.0.4265 Developer):
--   VIEW DEFINITION  makes the database's object metadata visible. Catalog
--     views only show securables the user has SOME permission on: with CONNECT
--     alone the collector sees 0 tables, and VIEW SECURITY DEFINITION + the DMV
--     permission still shows 0 tables. Trust note: VIEW DEFINITION also lets the
--     account read module text (procedures, views, functions) outside the tool.
--   VIEW DATABASE PERFORMANCE STATE  is required by sys.dm_db_partition_stats
--     (table/index bytes and row counts) on this version family.
--
-- Run ONCE with sqlcmd (or SSMS in SQLCMD mode) as a sysadmin, or a principal
-- with ALTER ANY LOGIN on the server and db_owner in the target database:
--     sqlcmd -S HOST -E -i standard.sql           (Windows auth)
--     sqlcmd -S HOST -U admin -P '...' -i standard.sql
-- Re-running is safe; it resets the login password to the value below.
-- Azure SQL Database: USE is not supported there. Run the [master] batch
-- connected to master, the remaining batches connected to the target database,
-- and skip any [msdb] batch (no Agent).  Or create a contained user instead:
--     CREATE USER [dbwarp_blueprint_standard] WITH PASSWORD = N'...';
-- =============================================================================

-- ---- EDIT THESE LINES -------------------------------------------------------
:setvar login     dbwarp_blueprint_standard
:setvar password  CHANGE-ME
:setvar database  target_database
:setvar schemas   ALL
--   ALL = every schema that owns a user table (for an unscoped run),
--   or an explicit comma-separated list, e.g.  :setvar schemas "app, billing"
:setvar use_db_datareader 0
--   1 = add the login to db_datareader instead of granting schema SELECT.
-- -----------------------------------------------------------------------------
:on error exit
SET NOCOUNT ON;
IF '$(password)' = 'CHANGE-ME'
    RAISERROR('Edit the :setvar password line at the top of this file before running it.', 16, 1);
GO

USE [master];
IF SUSER_ID('$(login)') IS NULL
    CREATE LOGIN [$(login)] WITH PASSWORD = N'$(password)', DEFAULT_DATABASE = [$(database)], CHECK_POLICY = ON;
ELSE
    ALTER LOGIN [$(login)] WITH PASSWORD = N'$(password)', DEFAULT_DATABASE = [$(database)];
GO

USE [$(database)];
IF USER_ID('$(login)') IS NULL CREATE USER [$(login)] FOR LOGIN [$(login)];
GRANT CONNECT TO [$(login)];
GRANT VIEW DEFINITION TO [$(login)];
GRANT VIEW DATABASE PERFORMANCE STATE TO [$(login)];
GO

USE [$(database)];
IF '$(use_db_datareader)' = '1'
BEGIN
    ALTER ROLE [db_datareader] ADD MEMBER [$(login)];
    PRINT 'added to db_datareader';
END
ELSE
BEGIN
    -- STRING_SPLIT needs database compatibility level 130+ (SQL Server 2016).
    DECLARE @s sysname, @sql nvarchar(max);
    DECLARE c CURSOR LOCAL FAST_FORWARD FOR
        SELECT s.name FROM sys.schemas s
        WHERE ('$(schemas)' = 'ALL'
               AND EXISTS (SELECT 1 FROM sys.tables t WHERE t.schema_id = s.schema_id AND t.is_ms_shipped = 0))
           OR ('$(schemas)' <> 'ALL'
               AND s.name IN (SELECT LTRIM(RTRIM(value)) FROM STRING_SPLIT('$(schemas)', ',')))
        ORDER BY s.name;
    OPEN c; FETCH NEXT FROM c INTO @s;
    WHILE @@FETCH_STATUS = 0
    BEGIN
        SET @sql = N'GRANT SELECT ON SCHEMA::' + QUOTENAME(@s) + N' TO ' + QUOTENAME('$(login)') + N';';
        PRINT @sql; EXEC sp_executesql @sql;
        FETCH NEXT FROM c INTO @s;
    END
    CLOSE c; DEALLOCATE c;
END
GO

-- ---- verification (informational): what the account actually sees ----------
USE [$(database)];
EXECUTE AS USER = '$(login)';
SELECT 'dbwarp-blueprint STANDARD tier applied for' AS note, '$(login)' AS login_name, DB_NAME() AS database_name;
SELECT s.name AS schema_name,
       COUNT(*) AS visible_user_tables,
       SUM(CASE WHEN HAS_PERMS_BY_NAME(QUOTENAME(s.name) + '.' + QUOTENAME(t.name), 'OBJECT', 'SELECT') = 1 THEN 1 ELSE 0 END) AS readable_tables
FROM sys.tables t JOIN sys.schemas s ON s.schema_id = t.schema_id
WHERE t.is_ms_shipped = 0
GROUP BY s.name ORDER BY s.name;
SELECT COUNT(*) AS partition_stats_rows
FROM sys.dm_db_partition_stats p JOIN sys.tables t ON t.object_id = p.object_id WHERE t.is_ms_shipped = 0;
REVERT;
GO
