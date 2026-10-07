-- =============================================================================
-- dbwarp-blueprint account removal - Oracle (oracle-12c)
-- =============================================================================
-- STATUS: DBWarp Blueprint 1.6 Oracle Basic preview. Review the "Known
-- limitations" section of sql/grants/ORACLE_PREVIEW.md before use.
-- Removes only the Basic collector account after proving it is local,
-- non-Oracle-maintained, owns no objects, has no privilege outside the Basic
-- allowlist, and carries the privilege-free marker role assigned only by the matching
-- grant lifecycle. This admits a partially provisioned collector without
-- making an arbitrary monitoring account deletable. DROP USER intentionally
-- omits CASCADE as a second safety net.
-- Connect directly to the intended PDB as an authorised administrator;
-- CDB$ROOT is refused. An absent target is an error: a misspelled principal
-- must never look like successful cleanup. Invoke this script with NO for a
-- normal removal. An unassigned marker left by an interrupted prior removal
-- is reported for manual DBA review; this script never drops a role based on
-- its configured name alone after the collector identity has disappeared.
-- The executing administrator needs DROP USER and DROP ANY ROLE; the preflight
-- verifies both before mutation.
-- =============================================================================
SET SERVEROUTPUT ON
WHENEVER SQLERROR EXIT SQL.SQLCODE
@@../oracle-accounts.sql
DEFINE bp_recover_marker = &1
DECLARE
  v_users       NUMBER;
  v_owned       NUMBER;
  v_unexpected  NUMBER;
  v_marker_roles NUMBER;
  v_marker_privileges NUMBER;
  v_other_grantees NUMBER;
  v_admin_privileges NUMBER;
  v_role_maintained VARCHAR2(1);
  v_common      VARCHAR2(3);
  v_maintained  VARCHAR2(1);
  v_recover_input VARCHAR2(32767) := UPPER(TRIM('&bp_recover_marker'));
  v_marker      VARCHAR2(128) := SYS.DBMS_ASSERT.SIMPLE_SQL_NAME(UPPER('&bp_basic_marker_role'));
  v_user        VARCHAR2(128) := SYS.DBMS_ASSERT.SIMPLE_SQL_NAME(UPPER('&bp_basic_user'));
BEGIN
  IF v_recover_input IS NULL OR LENGTH(v_recover_input) > 3
     OR v_recover_input <> 'NO' THEN
    RAISE_APPLICATION_ERROR(-20014,
      'revoke argument must be NO; inspect an orphan marker role manually');
  END IF;
  v_recover_input := SYS.DBMS_ASSERT.SIMPLE_SQL_NAME(v_recover_input);
  IF SYS_CONTEXT('USERENV', 'CON_NAME') = 'CDB$ROOT' THEN
    RAISE_APPLICATION_ERROR(-20005,
      'Connect directly to the intended PDB service; CDB$ROOT is not supported.');
  END IF;
  -- Preflight every destructive privilege before either the normal or
  -- interrupted-recovery branch can mutate the database.
  SELECT COUNT(DISTINCT privilege) INTO v_admin_privileges FROM session_privs
   WHERE privilege IN ('DROP USER', 'DROP ANY ROLE');
  IF v_admin_privileges <> 2 THEN
    RAISE_APPLICATION_ERROR(-20013,
      'administrator requires DROP USER and DROP ANY ROLE before account removal');
  END IF;
  SELECT COUNT(*) INTO v_marker_roles FROM dba_roles WHERE role = v_marker;
  IF v_marker_roles = 1 THEN
    SELECT oracle_maintained INTO v_role_maintained
      FROM dba_roles WHERE role = v_marker;
  ELSE
    v_role_maintained := 'N';
  END IF;
  SELECT COUNT(*) INTO v_marker_privileges FROM (
    SELECT 1 FROM dba_sys_privs WHERE grantee = v_marker
    UNION ALL SELECT 1 FROM dba_role_privs WHERE grantee = v_marker
    UNION ALL SELECT 1 FROM dba_tab_privs WHERE grantee = v_marker
    UNION ALL SELECT 1 FROM dba_col_privs WHERE grantee = v_marker
  );
  SELECT COUNT(*) INTO v_other_grantees FROM dba_role_privs
   WHERE granted_role = v_marker AND grantee <> v_user;
  IF v_marker_roles > 1 OR v_role_maintained <> 'N'
     OR v_marker_privileges <> 0 THEN
    RAISE_APPLICATION_ERROR(-20011,
      'refusing to use a marker role that is absent, privileged or shared');
  END IF;
  SELECT COUNT(*) INTO v_users FROM dba_users WHERE username = v_user;
  IF v_users = 0 THEN
    RAISE_APPLICATION_ERROR(-20009,
      'collector account is absent; verify the principal and inspect any marker role manually');
  END IF;
  IF v_marker_roles <> 1 OR v_other_grantees <> 0 THEN
    RAISE_APPLICATION_ERROR(-20011,
      'refusing to use a marker role that is absent, privileged or shared');
  END IF;
  SELECT common, oracle_maintained
    INTO v_common, v_maintained
    FROM dba_users WHERE username = v_user;
  IF v_common <> 'NO' OR v_maintained <> 'N' THEN
    RAISE_APPLICATION_ERROR(-20006,
      'refusing to drop a common or Oracle-maintained account');
  END IF;
  SELECT COUNT(*) INTO v_owned FROM dba_objects WHERE owner = v_user;
  IF v_owned <> 0 THEN
    RAISE_APPLICATION_ERROR(-20007,
      'refusing to drop an account that owns database objects');
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
    SELECT 1 FROM dba_tab_privs
     WHERE grantee = v_user
       AND NOT (owner = 'SYS' AND privilege = 'SELECT' AND grantable = 'NO'
                AND table_name IN ('DBA_CONSTRAINTS', 'DBA_CONS_COLUMNS', 'DBA_EXTERNAL_TABLES', 'DBA_INDEXES', 'DBA_IND_COLUMNS', 'DBA_IND_EXPRESSIONS', 'DBA_LOBS', 'DBA_MVIEW_LOGS', 'DBA_NESTED_TABLES', 'DBA_OBJECTS', 'DBA_OBJECT_TABLES', 'DBA_PART_KEY_COLUMNS', 'DBA_PART_TABLES', 'DBA_QUEUE_TABLES', 'DBA_SEGMENTS', 'DBA_TABLES', 'DBA_TAB_COLS', 'DBA_TAB_COLUMNS', 'DBA_TAB_IDENTITY_COLS', 'DBA_TAB_PARTITIONS', 'DBA_TAB_STATISTICS', 'DBA_TAB_SUBPARTITIONS', 'V_$CONTAINERS', 'V_$DATABASE', 'V_$INSTANCE', 'V_$PARAMETER'))
  );
  IF v_unexpected <> 0 THEN
    RAISE_APPLICATION_ERROR(-20008,
      'refusing to drop an account with privileges outside the Basic allowlist');
  END IF;
  SELECT COUNT(*) INTO v_marker_roles FROM dba_role_privs
   WHERE grantee = v_user AND granted_role = v_marker AND admin_option = 'NO';
  IF v_marker_roles <> 1 THEN
    RAISE_APPLICATION_ERROR(-20010,
      'refusing to drop an account without its Blueprint collector marker role');
  END IF;
  DBMS_OUTPUT.PUT_LINE(v_user || ': dropping verified empty Basic collector account');
  -- DROP USER first. A concurrent-session rejection leaves both user and
  -- marker unchanged. If interruption follows the successful user drop, the
  -- unassigned marker remains for explicit DBA inspection; it is never
  -- removed later from its name alone.
  EXECUTE IMMEDIATE 'DROP USER ' || SYS.DBMS_ASSERT.ENQUOTE_NAME(v_user, FALSE);
  EXECUTE IMMEDIATE 'DROP ROLE ' || SYS.DBMS_ASSERT.ENQUOTE_NAME(v_marker, FALSE);
END;
/
-- Reconciliation: nothing below should return rows.
SELECT grantee, owner, table_name, privilege FROM dba_tab_privs
 WHERE grantee = UPPER('&bp_basic_user');
SELECT username FROM dba_users
 WHERE username = UPPER('&bp_basic_user');
SELECT role FROM dba_roles
 WHERE role = UPPER('&bp_basic_marker_role');
