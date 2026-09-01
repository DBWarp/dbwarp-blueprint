-- =============================================================================
-- dbwarp-blueprint least-privilege grants — SQL Server 2019 (15.x)
-- Tier: ENHANCED
-- =============================================================================
-- Authorizes this command:
--
--   dbwarp-blueprint --connect sqlserver://dbwarp_blueprint_enhanced@HOST/DBNAME --schema app \
--       --password-file /etc/dbwarp/db.pass --artifact-detail analyzed --measure-compression --yes --sample-rows 1000 --max-wall-secs 300 \
--       --out blueprint.toml --audit-log blueprint.audit.txt
--
-- ENHANCED = STANDARD + non-table artifact inventory (--artifact-detail
-- summary/graph/analyzed; graph and analyzed require --yes).
-- VIEW DEFINITION (already granted) covers sys.objects, sys.sql_modules
-- (definitions for analyzed mode), sys.synonyms, user types, sys.assemblies,
-- sys.certificates, sys.symmetric_keys, sys.asymmetric_keys,
-- sys.database_scoped_credentials, sys.external_*; partition functions /
-- schemes, filegroups, sys.column_master_keys, sys.column_encryption_keys,
-- sys.servers and sys.databases were visible with CONNECT alone (verified).
-- Two catalogs need explicit grants (verified: denied without them):
--   sys.sql_expression_dependencies  (dependency edges) -> SELECT on that view
--     (by default only db_owner may select it; db_datareader also covers it)
--   msdb.dbo.sysjobs                 (Agent job census)  -> user in msdb +
--     SELECT on dbo.sysjobs (SQLAgentReaderRole is NOT enough: the collector
--     reads the base table, not sysjobs_view)
--
-- Trust note: analyzed mode reads module definitions transiently; VIEW
-- DEFINITION grants that read capability to the account in any tool.
-- The collector reports artifact visibility = "full" when the account holds
-- VIEW DEFINITION on the database (or is db_owner/sysadmin).
--
-- PRE-CAPTURE REQUIREMENTS (full engine/tier matrix: ../README.md): complete
-- STANDARD preparation; make --schema match the edited list; approve transient
-- module-definition reads, dependency metadata and the msdb Agent-job census.
-- Database/server-wide artifact families remain in scope. Start with
-- sample-rows 1000 / max-wall-secs 300; raise both for higher requested detail.
--
-- Optional, NOT part of the minimum (observed unnecessary on 2019/2022 but
-- vendor-documented for the key catalogs / for linked servers without a
-- public login mapping):
--   GRANT VIEW ANY COLUMN MASTER KEY DEFINITION TO [dbwarp_blueprint_enhanced];
--   GRANT VIEW ANY COLUMN ENCRYPTION KEY DEFINITION TO [dbwarp_blueprint_enhanced];
--   USE [master]; GRANT VIEW ANY DEFINITION TO [dbwarp_blueprint_enhanced];  -- server-wide metadata; broad
--
-- NOT granted: VIEW SERVER STATE, VIEW ANY DEFINITION, SQLAgent roles, UNMASK,
-- key/certificate CONTROL, IMPERSONATE, ALTER, DML, DDL, sysadmin, db_owner.
-- Common to every tier (verified live on SQL Server 2019 15.0.4480 Developer):
--   VIEW DEFINITION  makes the database's object metadata visible. Catalog
--     views only show securables the user has SOME permission on: with CONNECT
--     alone the collector sees 0 tables, and VIEW SECURITY DEFINITION + the DMV
--     permission still shows 0 tables. Trust note: VIEW DEFINITION also lets the
--     account read module text (procedures, views, functions) outside the tool.
--   VIEW DATABASE STATE  is required by sys.dm_db_partition_stats
--     (table/index bytes and row counts) on this version family.
--
-- Run ONCE with sqlcmd (or SSMS in SQLCMD mode) as a sysadmin, or a principal
-- with ALTER ANY LOGIN on the server and db_owner in the target database:
--     sqlcmd -S HOST -E -i enhanced.sql           (Windows auth)
--     sqlcmd -S HOST -U admin -P '...' -i enhanced.sql
-- Re-running is safe; it resets the login password to the value below.
-- Azure SQL Database: USE is not supported there. Run the [master] batch
-- connected to master, the remaining batches connected to the target database,
-- and skip any [msdb] batch (no Agent).  Or create a contained user instead:
--     CREATE USER [dbwarp_blueprint_enhanced] WITH PASSWORD = N'...';
-- =============================================================================

-- ---- EDIT THESE LINES -------------------------------------------------------
:setvar login     dbwarp_blueprint_enhanced
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
GRANT VIEW DATABASE STATE TO [$(login)];
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

USE [$(database)];
GRANT SELECT ON sys.sql_expression_dependencies TO [$(login)];
GO

-- Skip this batch on Azure SQL Database (no msdb / SQL Server Agent).
USE [msdb];
IF USER_ID('$(login)') IS NULL CREATE USER [$(login)] FOR LOGIN [$(login)];
GRANT SELECT ON dbo.sysjobs TO [$(login)];
GO

-- ---- verification (informational): what the account actually sees ----------
USE [$(database)];
EXECUTE AS USER = '$(login)';
SELECT 'dbwarp-blueprint ENHANCED tier applied for' AS note, '$(login)' AS login_name, DB_NAME() AS database_name;
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
