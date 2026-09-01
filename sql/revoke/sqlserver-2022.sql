-- =============================================================================
-- DBWarp Blueprint collector-login removal — SQL Server 2022 / 2025
-- =============================================================================
-- Run in SQLCMD mode as a sysadmin. EDIT the login and database values below
-- to match the grant scripts exactly. The script removes the dedicated users
-- before their server logins, which also removes role membership and explicit
-- grants. Azure SQL Database has no server login or msdb Agent user: run only
-- the target-database block there.

:setvar basic_login dbwarp_blueprint_basic
:setvar standard_login dbwarp_blueprint_standard
:setvar enhanced_login dbwarp_blueprint_enhanced
:setvar database target_database
:on error exit
SET NOCOUNT ON;

USE [$(database)];
IF USER_ID('$(basic_login)') IS NOT NULL DROP USER [$(basic_login)];
IF USER_ID('$(standard_login)') IS NOT NULL DROP USER [$(standard_login)];
IF USER_ID('$(enhanced_login)') IS NOT NULL DROP USER [$(enhanced_login)];
GO

USE [msdb];
IF USER_ID('$(enhanced_login)') IS NOT NULL DROP USER [$(enhanced_login)];
GO

USE [master];
IF SUSER_ID('$(basic_login)') IS NOT NULL DROP LOGIN [$(basic_login)];
IF SUSER_ID('$(standard_login)') IS NOT NULL DROP LOGIN [$(standard_login)];
IF SUSER_ID('$(enhanced_login)') IS NOT NULL DROP LOGIN [$(enhanced_login)];
GO
