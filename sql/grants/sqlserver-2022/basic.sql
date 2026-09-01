-- =============================================================================
-- dbwarp-blueprint least-privilege grants — SQL Server 2022 / 2025 (16.x, 17.x)
-- Tier: BASIC
-- =============================================================================
-- Authorizes this command:
--
--   dbwarp-blueprint --connect sqlserver://dbwarp_blueprint_basic@HOST/DBNAME --schema app \
--       --password-file /etc/dbwarp/db.pass --artifact-detail none \
--       --out blueprint.toml --audit-log blueprint.audit.txt
--
-- BASIC = catalog walk only; no customer row can be read.
-- Reads: SERVERPROPERTY, sys.tables, sys.dm_db_partition_stats, sys.columns,
-- sys.types, sys.indexes, sys.index_columns, sys.foreign_keys,
-- sys.foreign_key_columns. (Verified: full table/column/index/FK metadata and
-- partition stats visible; SELECT TOP 5 on a table denied.)
--
-- PRE-CAPTURE REQUIREMENTS (full engine/tier matrix: ../README.md): keep DDL
-- and partition maintenance stable; record the selected schema and expected
-- table/index/FK counts; confirm this is the intended primary/replica. Do not
-- run UPDATE STATISTICS solely for Blueprint: the collector reads partition
-- counters, not optimizer histograms. Repeat --schema for approved schemas;
-- an unresolved selector fails with DBP1420E.
--
-- NOT granted: SELECT on any object, db_datareader, VIEW SERVER STATE,
-- VIEW ANY DEFINITION, CONTROL, IMPERSONATE, ALTER, sysadmin, db_owner.
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
--     sqlcmd -S HOST -E -i basic.sql           (Windows auth)
--     sqlcmd -S HOST -U admin -P '...' -i basic.sql
-- Re-running is safe; it resets the login password to the value below.
-- Azure SQL Database: USE is not supported there. Run the [master] batch
-- connected to master, the remaining batches connected to the target database,
-- and skip any [msdb] batch (no Agent).  Or create a contained user instead:
--     CREATE USER [dbwarp_blueprint_basic] WITH PASSWORD = N'...';
-- =============================================================================

-- ---- EDIT THESE LINES -------------------------------------------------------
:setvar login     dbwarp_blueprint_basic
:setvar password  CHANGE-ME
:setvar database  target_database
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

-- ---- verification (informational): what the account actually sees ----------
USE [$(database)];
EXECUTE AS USER = '$(login)';
SELECT 'dbwarp-blueprint BASIC tier applied for' AS note, '$(login)' AS login_name, DB_NAME() AS database_name;
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
