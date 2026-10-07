-- =============================================================================
-- dbwarp-blueprint least-privilege grants - Oracle
-- Tier: BASIC
-- =============================================================================
-- STATUS: DBWarp Blueprint 1.6 Oracle Basic preview. Review the "Known
-- limitations" section of
-- sql/grants/ORACLE_PREVIEW.md before use. Oracle AI Database 26ai uses the
-- 23ai family script because both expose the major-23 catalogue contract.
--
-- Example live preview command after this grant is applied:
--
--   dbwarp-blueprint --connect oracle://HOST:1521/SERVICE \
--       --user dbwarp_blueprint_basic --password-file /private/path/oracle.pass \
--       --schema APP_SCHEMA --oracle-sqlplus /absolute/path/sqlplus \
--       --oracle-network-config-dir /private/empty-directory \
--       --acknowledge-oracle-preview --artifact-detail none \
--       --out blueprint.toml --audit-log blueprint.audit.txt
--
-- BASIC = the collector reads catalogues only and this script grants no
-- table-row access. The collector issues no row queries. Access
-- inherited through PUBLIC or ownership is outside this script and
-- must be reviewed separately. The granted DBA_* views can themselves
-- expose dictionary-held values described in APPROACH below.
--
-- APPROACH: direct SELECT on named dictionary views. These full DBA_* views
-- can expose dictionary-held values the collector
-- does not project, including column low/high values, default expressions and
-- constraint conditions. Oracle cannot grant selected columns of a dictionary
-- view. SELECT_CATALOG_ROLE is the still broader built-in alternative: it also
-- exposes the rest of the data dictionary. Prefer the named grants below and
-- review this residual visibility explicitly.
--
-- These are SYS-owned views. Connect directly to the intended PDB service as
-- an administrator authorised to grant them; SYSDBA is not available on every
-- managed Oracle service. The script refuses CDB$ROOT. For example:
--     sqlplus admin@//HOST:1521/APPPDB @basic.sql NO
-- The grants are local to that PDB. Create the local account first under the
-- site's own authentication and password policy; this script never creates an
-- account or handles a credential. Treat an edited copy as privileged DBA code.
-- Remove that copy after use according to your secure-disposal policy.
-- The executing administrator needs CREATE ROLE, GRANT ANY ROLE and DROP ANY
-- ROLE for the recoverable lifecycle marker, plus authority to issue the
-- explicit object grants below. These are administrator prerequisites only;
-- they are never granted to the collector account.
-- Remove the account afterwards with the matching script under ../../revoke/.
-- =============================================================================

-- Edit the shared principal file once for matching grant and revoke behavior.
@@../../oracle-accounts.sql
DEFINE bp_user            = &&bp_basic_user
DEFINE bp_marker_role     = &&bp_basic_marker_role
DEFINE bp_recover_marker  = &1

SET SERVEROUTPUT ON
SET VERIFY OFF
WHENEVER SQLERROR EXIT SQL.SQLCODE

BEGIN
  IF SYS_CONTEXT('USERENV', 'CON_NAME') = 'CDB$ROOT' THEN
    RAISE_APPLICATION_ERROR(-20005,
      'Connect directly to the intended PDB service; CDB$ROOT is not supported.');
  END IF;
END;
/

DECLARE
  v_exists      NUMBER;
  v_common      VARCHAR2(3);
  v_maintained  VARCHAR2(1);
  v_user        VARCHAR2(128) := SYS.DBMS_ASSERT.SIMPLE_SQL_NAME(UPPER('&bp_user'));
BEGIN
  SELECT COUNT(*) INTO v_exists FROM dba_users WHERE username = v_user;
  IF v_exists = 0 THEN
    RAISE_APPLICATION_ERROR(-20001,
      'Create the local collector account in this PDB before applying grants.');
  END IF;
  SELECT common, oracle_maintained INTO v_common, v_maintained
    FROM dba_users WHERE username = v_user;
  IF v_common <> 'NO' OR v_maintained <> 'N' THEN
    RAISE_APPLICATION_ERROR(-20006,
      'Collector account must be local and not Oracle-maintained.');
  END IF;
  DBMS_OUTPUT.PUT_LINE(v_user || ' exists; authentication left untouched');
END;
/

-- Prove every lifecycle privilege before the first DDL statement. A partial
-- role lifecycle is harder to recover safely than a refused preflight.
DECLARE
  v_admin_privileges NUMBER;
BEGIN
  SELECT COUNT(DISTINCT privilege) INTO v_admin_privileges
    FROM session_privs
   WHERE privilege IN
     ('CREATE ROLE', 'GRANT ANY ROLE', 'DROP ANY ROLE');
  IF v_admin_privileges <> 3 THEN
    RAISE_APPLICATION_ERROR(-20015,
      'administrator requires CREATE ROLE, GRANT ANY ROLE and DROP ANY ROLE before provisioning');
  END IF;
END;
/


-- Refuse to repurpose an account that already has broader access or owns
-- database objects. Re-running this exact script is allowed; only CREATE
-- SESSION, the privilege-free lifecycle marker role, and the named SELECT
-- grants below may pre-exist. The account's profile and password policy are
-- otherwise left untouched.
DECLARE
  v_unexpected NUMBER;
  v_role_exists NUMBER;
  v_role_privileges NUMBER;
  v_marker_grants NUMBER;
  v_malformed_grants NUMBER;
  v_target_grants NUMBER;
  v_admin_grants NUMBER;
  v_role_maintained VARCHAR2(1);
  v_recover_input VARCHAR2(32767) := UPPER(TRIM('&bp_recover_marker'));
  v_recover     VARCHAR2(3);
  v_creator    VARCHAR2(128) := SYS.DBMS_ASSERT.SIMPLE_SQL_NAME(
    SYS_CONTEXT('USERENV', 'SESSION_USER'));
  v_marker_admin VARCHAR2(128);
  v_user       VARCHAR2(128) := SYS.DBMS_ASSERT.SIMPLE_SQL_NAME(UPPER('&bp_user'));
  v_marker     VARCHAR2(128) := SYS.DBMS_ASSERT.SIMPLE_SQL_NAME(UPPER('&bp_marker_role'));
BEGIN
  IF v_recover_input IS NULL OR LENGTH(v_recover_input) > 3
     OR v_recover_input NOT IN ('YES', 'NO') THEN
    RAISE_APPLICATION_ERROR(-20014,
      'grant recovery argument must be YES or NO');
  END IF;
  v_recover := SYS.DBMS_ASSERT.SIMPLE_SQL_NAME(v_recover_input);
  IF v_creator = v_user THEN
    RAISE_APPLICATION_ERROR(-20013,
      'apply the collector grants from a separate authorised administrator account');
  END IF;
  SELECT COUNT(*) INTO v_unexpected FROM (
    SELECT 1 FROM dba_sys_privs
     WHERE grantee = v_user
       AND (privilege <> 'CREATE SESSION' OR admin_option <> 'NO')
    UNION ALL
    SELECT 1 FROM dba_role_privs
     WHERE grantee = v_user
       AND (granted_role <> v_marker OR admin_option <> 'NO')
    UNION ALL
    SELECT 1 FROM dba_col_privs WHERE grantee = v_user
    UNION ALL
    SELECT 1 FROM dba_objects WHERE owner = v_user
    UNION ALL
    SELECT 1 FROM dba_tab_privs
     WHERE grantee = v_user
       AND NOT (owner = 'SYS' AND privilege = 'SELECT' AND grantable = 'NO'
                AND table_name IN ('DBA_CONSTRAINTS', 'DBA_CONS_COLUMNS', 'DBA_EXTERNAL_TABLES', 'DBA_INDEXES', 'DBA_IND_COLUMNS', 'DBA_IND_EXPRESSIONS', 'DBA_LOBS', 'DBA_MVIEW_LOGS', 'DBA_NESTED_TABLES', 'DBA_OBJECTS', 'DBA_OBJECT_TABLES', 'DBA_PART_KEY_COLUMNS', 'DBA_PART_TABLES', 'DBA_QUEUE_TABLES', 'DBA_SEGMENTS', 'DBA_TABLES', 'DBA_TAB_COLS', 'DBA_TAB_COLUMNS', 'DBA_TAB_IDENTITY_COLS', 'DBA_TAB_PARTITIONS', 'DBA_TAB_STATISTICS', 'DBA_TAB_SUBPARTITIONS', 'V_$CONTAINERS', 'V_$DATABASE', 'V_$INSTANCE', 'V_$PARAMETER'))
  );
  IF v_unexpected <> 0 THEN
    RAISE_APPLICATION_ERROR(-20002,
      'collector account already has privileges or owned objects outside this Basic grant set');
  END IF;
  SELECT COUNT(*) INTO v_role_exists FROM dba_roles WHERE role = v_marker;
  IF v_role_exists = 1 THEN
    SELECT oracle_maintained INTO v_role_maintained
      FROM dba_roles WHERE role = v_marker;
  ELSE
    v_role_maintained := 'N';
  END IF;
  SELECT COUNT(*) INTO v_role_privileges FROM (
    SELECT 1 FROM dba_sys_privs WHERE grantee = v_marker
    UNION ALL SELECT 1 FROM dba_role_privs WHERE grantee = v_marker
    UNION ALL SELECT 1 FROM dba_tab_privs WHERE grantee = v_marker
    UNION ALL SELECT 1 FROM dba_col_privs WHERE grantee = v_marker
  );
  SELECT COUNT(*) INTO v_marker_grants FROM dba_role_privs
   WHERE granted_role = v_marker;
  SELECT COUNT(*) INTO v_malformed_grants FROM dba_role_privs
   WHERE granted_role = v_marker
     AND NOT ((grantee = v_user AND admin_option = 'NO')
       OR (grantee <> v_user AND admin_option = 'YES'));
  SELECT COUNT(*) INTO v_target_grants FROM dba_role_privs
   WHERE granted_role = v_marker AND grantee = v_user
     AND admin_option = 'NO';
  SELECT COUNT(*) INTO v_admin_grants FROM dba_role_privs
   WHERE granted_role = v_marker AND grantee <> v_user
     AND admin_option = 'YES';
  IF v_role_exists > 1 OR v_role_maintained <> 'N'
     OR v_role_privileges <> 0 OR v_malformed_grants <> 0
     OR v_target_grants > 1 OR v_admin_grants > 1
     OR (v_role_exists = 1 AND v_marker_grants NOT IN (1, 2))
     OR (v_role_exists = 1 AND v_target_grants + v_admin_grants <> v_marker_grants) THEN
    RAISE_APPLICATION_ERROR(-20010,
      'collector marker role is not an exclusive or recoverable marker for this account');
  END IF;
  IF v_admin_grants = 1 THEN
    IF v_role_exists = 1 AND v_recover <> 'YES' THEN
      RAISE_APPLICATION_ERROR(-20010,
        'collector marker role resembles an interrupted grant; verify it and explicitly enable recovery');
    END IF;
    SELECT grantee INTO v_marker_admin FROM dba_role_privs
     WHERE granted_role = v_marker AND grantee <> v_user
       AND admin_option = 'YES';
  END IF;

  IF v_role_exists = 0 THEN
    EXECUTE IMMEDIATE 'CREATE ROLE ' ||
      SYS.DBMS_ASSERT.ENQUOTE_NAME(v_marker, FALSE) || ' NOT IDENTIFIED';
    v_admin_grants := 1;
    v_marker_admin := v_creator;
  END IF;
  BEGIN
    IF v_target_grants = 0 THEN
      EXECUTE IMMEDIATE 'GRANT ' || SYS.DBMS_ASSERT.ENQUOTE_NAME(v_marker, FALSE) ||
        ' TO ' || SYS.DBMS_ASSERT.ENQUOTE_NAME(v_user, FALSE);
    END IF;
    -- Oracle grants a newly created role back to its creator WITH ADMIN
    -- OPTION. An interrupted earlier run may have left both grants in place;
    -- that exact two-grantee state is safe to reconcile here.
    IF v_admin_grants = 1 THEN
      EXECUTE IMMEDIATE 'REVOKE ' ||
        SYS.DBMS_ASSERT.ENQUOTE_NAME(v_marker, FALSE) || ' FROM ' ||
        SYS.DBMS_ASSERT.ENQUOTE_NAME(v_marker_admin, FALSE);
    END IF;
  EXCEPTION WHEN OTHERS THEN
    IF v_role_exists = 0 THEN
      BEGIN
        EXECUTE IMMEDIATE 'DROP ROLE ' ||
          SYS.DBMS_ASSERT.ENQUOTE_NAME(v_marker, FALSE);
      EXCEPTION WHEN OTHERS THEN NULL;
      END;
    END IF;
    RAISE;
  END;
END;
/

GRANT CREATE SESSION TO &bp_user;
-- ---- dictionary views -------------------------------------------------------
-- primary, unique, foreign and check constraints
GRANT SELECT ON SYS.DBA_CONSTRAINTS TO &bp_user;
-- constraint column membership and order
GRANT SELECT ON SYS.DBA_CONS_COLUMNS TO &bp_user;
-- classify external tables without reading their rows
GRANT SELECT ON SYS.DBA_EXTERNAL_TABLES TO &bp_user;
-- index list, uniqueness, type, clustering factor
GRANT SELECT ON SYS.DBA_INDEXES TO &bp_user;
-- index key order and direction
GRANT SELECT ON SYS.DBA_IND_COLUMNS TO &bp_user;
-- function-based index expressions
GRANT SELECT ON SYS.DBA_IND_EXPRESSIONS TO &bp_user;
-- LOB storage attached to columns
GRANT SELECT ON SYS.DBA_LOBS TO &bp_user;
-- identify materialized-view log support tables
GRANT SELECT ON SYS.DBA_MVIEW_LOGS TO &bp_user;
-- map nested collection storage to its logical table
GRANT SELECT ON SYS.DBA_NESTED_TABLES TO &bp_user;
-- object inventory and validity
GRANT SELECT ON SYS.DBA_OBJECTS TO &bp_user;
-- object and XMLType table inventory
GRANT SELECT ON SYS.DBA_OBJECT_TABLES TO &bp_user;
-- partition key columns
GRANT SELECT ON SYS.DBA_PART_KEY_COLUMNS TO &bp_user;
-- partition strategy
GRANT SELECT ON SYS.DBA_PART_TABLES TO &bp_user;
-- identify Advanced Queuing support tables
GRANT SELECT ON SYS.DBA_QUEUE_TABLES TO &bp_user;
-- allocated bytes for tables, indexes and LOBs
GRANT SELECT ON SYS.DBA_SEGMENTS TO &bp_user;
-- table list, optimizer row estimates, IOT flags
GRANT SELECT ON SYS.DBA_TABLES TO &bp_user;
-- hidden and virtual columns DBA_TAB_COLUMNS omits
GRANT SELECT ON SYS.DBA_TAB_COLS TO &bp_user;
-- declared type, capacity, nullability, defaults
GRANT SELECT ON SYS.DBA_TAB_COLUMNS TO &bp_user;
-- identity columns and their sequences
GRANT SELECT ON SYS.DBA_TAB_IDENTITY_COLS TO &bp_user;
-- partition inventory and per-partition rows
GRANT SELECT ON SYS.DBA_TAB_PARTITIONS TO &bp_user;
-- stats freshness and partition-level estimates
GRANT SELECT ON SYS.DBA_TAB_STATISTICS TO &bp_user;
-- physical subpartition inventory and row estimates
GRANT SELECT ON SYS.DBA_TAB_SUBPARTITIONS TO &bp_user;
-- connected container identity and role
GRANT SELECT ON SYS.V_$CONTAINERS TO &bp_user;
-- database identity and CDB flag
GRANT SELECT ON SYS.V_$DATABASE TO &bp_user;
-- instance identity and version
GRANT SELECT ON SYS.V_$INSTANCE TO &bp_user;
-- server-side filtered CPU and memory settings
GRANT SELECT ON SYS.V_$PARAMETER TO &bp_user;

-- ---- verification -----------------------------------------------------------
-- This script granted exactly 26 dictionary views. The count is
-- asserted rather than displayed, so a silently failed grant cannot pass
-- verification.
DECLARE
  v_granted NUMBER;
  v_marker_grants NUMBER;
BEGIN
  SELECT COUNT(*) INTO v_granted
    FROM dba_tab_privs
   WHERE grantee = UPPER('&bp_user') AND owner = 'SYS'
     AND privilege = 'SELECT' AND grantable = 'NO'
     AND table_name IN ('DBA_CONSTRAINTS', 'DBA_CONS_COLUMNS', 'DBA_EXTERNAL_TABLES', 'DBA_INDEXES', 'DBA_IND_COLUMNS', 'DBA_IND_EXPRESSIONS', 'DBA_LOBS', 'DBA_MVIEW_LOGS', 'DBA_NESTED_TABLES', 'DBA_OBJECTS', 'DBA_OBJECT_TABLES', 'DBA_PART_KEY_COLUMNS', 'DBA_PART_TABLES', 'DBA_QUEUE_TABLES', 'DBA_SEGMENTS', 'DBA_TABLES', 'DBA_TAB_COLS', 'DBA_TAB_COLUMNS', 'DBA_TAB_IDENTITY_COLS', 'DBA_TAB_PARTITIONS', 'DBA_TAB_STATISTICS', 'DBA_TAB_SUBPARTITIONS', 'V_$CONTAINERS', 'V_$DATABASE', 'V_$INSTANCE', 'V_$PARAMETER');
  DBMS_OUTPUT.PUT_LINE('dictionary_views_granted='||v_granted||' expected=26');
  IF v_granted <> 26 THEN
    RAISE_APPLICATION_ERROR(-20003,
      'expected 26 dictionary grants, found '||v_granted);
  END IF;
  SELECT COUNT(*) INTO v_marker_grants FROM dba_role_privs
   WHERE grantee = UPPER('&bp_user')
     AND granted_role = UPPER('&bp_marker_role')
     AND admin_option = 'NO';
  IF v_marker_grants <> 1 THEN
    RAISE_APPLICATION_ERROR(-20012,
      'expected one privilege-free lifecycle marker role grant');
  END IF;
END;
/
