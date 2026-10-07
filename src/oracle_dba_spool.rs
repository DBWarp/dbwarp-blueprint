//! Hands-off Oracle Basic SQL*Plus capture script and spool contract.
//!
//! The generated script is run by a DBA with the dedicated collector
//! account. DBWarp Blueprint is not present on that host and never reaches its
//! database network. The resulting customer-local spool contains native names;
//! this module validates and converts it only after it has been transferred to
//! the machine running DBWarp Blueprint.

#![allow(dead_code)]

use crate::artifacts::ArtifactDetail;
use crate::oracle_catalog::queries_for_tier;
use crate::oracle_provider::{
    OracleCaptureTier, OracleClientVersionAttestation, OracleProviderLimits, OracleVersion,
};
use crate::oracle_scope::{OracleOwnerScope, OracleOwnerScopeKind};
use crate::oracle_session::{
    OracleCaptureAbort, OracleCatalogCapture, OracleQueryFailure, OracleQueryOutcome,
    OracleQueryStatus, OracleRow, OracleValue,
};
use anyhow::{anyhow, bail, Context, Result};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Cursor};

const PREFIX: &str = "DBWARP_BP";
const RAW_PREFIX: &str = "DBWARP_BP_RAW";
const SPOOL_CONTRACT_VERSION: u16 = 2;
const MAX_LINE_BYTES: usize = 32_000;
const MAX_OWNER_COUNT: usize = 16_384;

#[derive(Debug)]
pub struct DecodedDbaSpool {
    pub capture: OracleCatalogCapture,
    pub scope: OracleOwnerScope,
    pub server_version: OracleVersion,
    pub artifact_detail: ArtifactDetail,
    pub limits: OracleProviderLimits,
    pub captured_at: String,
    pub capturing_principal: String,
    pub database_identity: String,
    pub recorded_client_version: Option<OracleVersion>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OracleDbaScriptFamily {
    Oracle12cR1,
    Oracle12cR2,
    Oracle19c,
    Oracle21c,
    Oracle26ai,
}

impl OracleDbaScriptFamily {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "12.1" | "12c-r1" | "oracle-12.1" => Some(Self::Oracle12cR1),
            "12.2" | "12c-r2" | "oracle-12.2" => Some(Self::Oracle12cR2),
            "19c" | "oracle-19c" => Some(Self::Oracle19c),
            "21c" | "oracle-21c" => Some(Self::Oracle21c),
            "26ai" | "oracle-26ai" => Some(Self::Oracle26ai),
            _ => None,
        }
    }

    fn registry_version(self) -> OracleVersion {
        let components: &[u16] = match self {
            Self::Oracle12cR1 => &[12, 1],
            Self::Oracle12cR2 => &[12, 2],
            Self::Oracle19c => &[19],
            Self::Oracle21c => &[21],
            Self::Oracle26ai => &[23],
        };
        OracleVersion::from_components(components)
            .expect("supported Oracle script family has a nonzero major")
    }

    /// Oracle AI Database 26ai uses the major-23 catalogue contract. Its
    /// `V_$INSTANCE.VERSION` value is the base release (`23.0.0.0.0`) while
    /// `PRODUCT_COMPONENT_VERSION.VERSION_FULL` carries the installed 23.26
    /// release update. Minimum grants deliberately do not depend on the latter
    /// public view, so on-host selection must use the stable major-23 fact.
    fn server_major(self) -> u16 {
        match self {
            Self::Oracle12cR1 | Self::Oracle12cR2 => 12,
            Self::Oracle19c => 19,
            Self::Oracle21c => 21,
            Self::Oracle26ai => 23,
        }
    }

    fn expected_minor(self) -> Option<u16> {
        match self {
            Self::Oracle12cR1 => Some(1),
            Self::Oracle12cR2 => Some(2),
            Self::Oracle19c | Self::Oracle21c | Self::Oracle26ai => None,
        }
    }

    fn matches_server_version(self, version: OracleVersion) -> bool {
        version.components()[0] == self.server_major()
            && self
                .expected_minor()
                .is_none_or(|minor| version.components().get(1) == Some(&minor))
    }

    fn label(self) -> &'static str {
        match self {
            Self::Oracle12cR1 => "oracle-12.1",
            Self::Oracle12cR2 => "oracle-12.2",
            Self::Oracle19c => "oracle-19c",
            Self::Oracle21c => "oracle-21c",
            Self::Oracle26ai => "oracle-26ai",
        }
    }
}

/// Render the customer-run SQL*Plus capture script directly from the canonical
/// query registry. Owner names are supplied as comma-separated UTF-8 hex and
/// decoded into bind variables; native names never enter rendered query text.
pub fn render_oracle_basic_dba_script(family: OracleDbaScriptFamily) -> String {
    render_oracle_basic_dba_script_current(family)
}

const SCRIPT_DIGEST_PLACEHOLDER: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";
const PORTABLE_CHECKSUM_PLACEHOLDER: &str = "0000000000";

fn render_oracle_basic_dba_script_current(family: OracleDbaScriptFamily) -> String {
    let template = render_oracle_basic_dba_script_template(
        family,
        SCRIPT_DIGEST_PLACEHOLDER,
        PORTABLE_CHECKSUM_PLACEHOLDER,
    );
    let digest = hex::encode(Sha256::digest(template.as_bytes()));
    let script_with_digest = template.replacen(SCRIPT_DIGEST_PLACEHOLDER, &digest, 1);
    let portable_checksum = format!("{:010}", posix_cksum(script_with_digest.as_bytes()));
    script_with_digest.replacen(PORTABLE_CHECKSUM_PLACEHOLDER, &portable_checksum, 1)
}

fn oracle_basic_dba_script_digest(family: OracleDbaScriptFamily) -> [u8; 32] {
    Sha256::digest(
        render_oracle_basic_dba_script_template(
            family,
            SCRIPT_DIGEST_PLACEHOLDER,
            PORTABLE_CHECKSUM_PLACEHOLDER,
        )
        .as_bytes(),
    )
    .into()
}

/// POSIX `cksum` CRC, including the input length as specified by the utility.
///
/// The customer-side Unix launcher uses this ubiquitous baseline utility for
/// an accidental-damage check. The canonical SHA-256 identity remains in the
/// spool and is verified by DBWarp Blueprint during offline ingestion.
fn posix_cksum(bytes: &[u8]) -> u32 {
    let mut crc = 0_u32;
    for byte in bytes {
        crc = posix_cksum_byte(crc, *byte);
    }
    let mut length = bytes.len() as u64;
    while length != 0 {
        crc = posix_cksum_byte(crc, (length & 0xff) as u8);
        length >>= 8;
    }
    !crc
}

fn posix_cksum_byte(mut crc: u32, byte: u8) -> u32 {
    crc ^= u32::from(byte) << 24;
    for _ in 0..8 {
        crc = if crc & 0x8000_0000 != 0 {
            (crc << 1) ^ 0x04c1_1db7
        } else {
            crc << 1
        };
    }
    crc
}

fn render_oracle_basic_dba_script_template(
    family: OracleDbaScriptFamily,
    script_digest: &str,
    portable_checksum: &str,
) -> String {
    let version = family.registry_version();
    let manifest = manifest_sha256(version);
    let limits = crate::oracle_sqlplus::SQLPLUS_BASIC_MAX_LIMITS;
    let query_count = queries_for_tier(OracleCaptureTier::Basic).count();
    let mut out = format!(
        "-- DBWarp Blueprint 1.6 Oracle Basic preview - DBA capture\n\
         -- Server family: {family}\n\
         -- Run through capture.sh or capture.ps1 as the dedicated collector account.\n\
         -- Do not run as SYS, SYSTEM, or a broadly privileged DBA account.\n\
         -- The protected raw spool is sanitized locally into the transferable spool.\n\
         -- Both contain real catalogue names and must stay inside your organisation.\n\
         -- Generated from DBWarp Blueprint's Oracle Basic catalogue queries. Do not edit.\n\
         -- POSIX cksum: {portable_checksum}\n\
         SET ECHO OFF\n\
         SET VERIFY OFF\n\
         SET FEEDBACK OFF\n\
         SET HEADING OFF\n\
         SET PAGESIZE 0\n\
         SET LINESIZE 32767\n\
         SET LONG 32767\n\
         SET LONGCHUNKSIZE 32767\n\
         SET WRAP OFF\n\
         SET TRIMOUT ON\n\
         SET TRIMSPOOL ON\n\
         SET TERMOUT OFF\n\
         SET TIMING OFF\n\
         SET AUTOPRINT OFF\n\
         SET AUTOTRACE OFF\n\
         SET PAUSE OFF\n\
         SET SQLCASE MIXED\n\
         SET ROWLIMIT OFF\n\
         SET CMDSEP OFF\n\
         SET SQLTERMINATOR \";\"\n\
         SET MARKUP HTML OFF\n\
         SET MARKUP CSV OFF\n\
         SET DEFINE OFF\n\
         WHENEVER OSERROR EXIT FAILURE\n\
         WHENEVER SQLERROR EXIT SQL.SQLCODE\n\
         ALTER SESSION SET NLS_NUMERIC_CHARACTERS = '.,';\n\
         ALTER SESSION SET NLS_CALENDAR = 'GREGORIAN';\n\
         ALTER SESSION SET NLS_DATE_FORMAT = 'YYYY-MM-DD\"T\"HH24:MI:SS';\n\
         ALTER SESSION SET NLS_TIMESTAMP_FORMAT = 'YYYY-MM-DD\"T\"HH24:MI:SS.FF9';\n\
         ALTER SESSION SET NLS_COMP = 'BINARY';\n\
         ALTER SESSION SET NLS_SORT = 'BINARY';\n\
         DECLARE\n\
           C_EXPECTED_MAJOR CONSTANT PLS_INTEGER := {server_major};\n\
           C_EXPECTED_MINOR CONSTANT PLS_INTEGER := {server_minor};\n\
           C_HEX_DIGITS CONSTANT VARCHAR2(16) := '0123456789ABCDEF';\n\
           V_INPUT CLOB := :DBWARP_BP_OWNER_HEX_LIST;\n\
           V_START PLS_INTEGER := 1;\n\
           V_STOP PLS_INTEGER;\n\
           V_HEX VARCHAR2(256);\n\
           V_PREVIOUS_HEX VARCHAR2(256);\n\
           V_OWNER VARCHAR2(128);\n\
           V_OWNER_COUNT PLS_INTEGER := 0;\n\
           V_OWNER_PRESENT PLS_INTEGER;\n\
           V_SERVER_VERSION VARCHAR2(64);\n\
           FUNCTION HEX_COMPARE(P_LEFT VARCHAR2, P_RIGHT VARCHAR2) RETURN PLS_INTEGER IS\n\
             V_LIMIT PLS_INTEGER := LEAST(LENGTH(P_LEFT), LENGTH(P_RIGHT));\n\
             V_LEFT PLS_INTEGER;\n\
             V_RIGHT PLS_INTEGER;\n\
           BEGIN\n\
             FOR I IN 1 .. V_LIMIT LOOP\n\
               V_LEFT := INSTR(C_HEX_DIGITS, SUBSTR(P_LEFT, I, 1));\n\
               V_RIGHT := INSTR(C_HEX_DIGITS, SUBSTR(P_RIGHT, I, 1));\n\
               IF V_LEFT < V_RIGHT THEN RETURN -1; END IF;\n\
               IF V_LEFT > V_RIGHT THEN RETURN 1; END IF;\n\
             END LOOP;\n\
             IF LENGTH(P_LEFT) < LENGTH(P_RIGHT) THEN RETURN -1; END IF;\n\
             IF LENGTH(P_LEFT) > LENGTH(P_RIGHT) THEN RETURN 1; END IF;\n\
             RETURN 0;\n\
           END;\n\
         BEGIN\n\
           IF SYS_CONTEXT('USERENV', 'CON_NAME') = 'CDB$ROOT' THEN\n\
             RAISE_APPLICATION_ERROR(-20005, 'connect-to-pdb');\n\
           END IF;\n\
           SELECT VERSION INTO V_SERVER_VERSION FROM SYS.V_$INSTANCE;\n\
           IF TO_NUMBER(REGEXP_SUBSTR(V_SERVER_VERSION, '^[0-9]+')) <> C_EXPECTED_MAJOR OR\n\
              (C_EXPECTED_MINOR >= 0 AND\n\
               TO_NUMBER(REGEXP_SUBSTR(V_SERVER_VERSION, '[0-9]+', 1, 2)) <> C_EXPECTED_MINOR) THEN\n\
             RAISE_APPLICATION_ERROR(-20004, 'wrong-script-family');\n\
           END IF;\n\
           IF V_INPUT IS NULL OR NOT REGEXP_LIKE(V_INPUT, '^[0-9A-F]+(,[0-9A-F]+)*$') THEN\n\
             RAISE_APPLICATION_ERROR(-20001, 'owner-input');\n\
           END IF;\n\
           LOOP\n\
             V_STOP := INSTR(V_INPUT, ',', V_START);\n\
             IF V_STOP = 0 THEN V_STOP := LENGTH(V_INPUT) + 1; END IF;\n\
             V_HEX := SUBSTR(V_INPUT, V_START, V_STOP - V_START);\n\
             IF MOD(LENGTH(V_HEX), 2) <> 0 OR LENGTH(V_HEX) > 256 OR\n\
                (V_PREVIOUS_HEX IS NOT NULL AND HEX_COMPARE(V_PREVIOUS_HEX, V_HEX) >= 0) THEN\n\
               RAISE_APPLICATION_ERROR(-20001, 'owner-input');\n\
             END IF;\n\
             V_OWNER := UTL_I18N.RAW_TO_CHAR(HEXTORAW(V_HEX), 'AL32UTF8');\n\
             IF V_OWNER IS NULL OR LENGTHB(V_OWNER) > 128 OR REGEXP_LIKE(V_OWNER, '[[:cntrl:]]') THEN\n\
               RAISE_APPLICATION_ERROR(-20001, 'owner-input');\n\
             END IF;\n\
             V_OWNER_COUNT := V_OWNER_COUNT + 1;\n\
             IF V_OWNER_COUNT > {max_owners} THEN\n\
               RAISE_APPLICATION_ERROR(-20001, 'owner-input');\n\
             END IF;\n\
             V_PREVIOUS_HEX := V_HEX;\n\
             EXIT WHEN V_STOP > LENGTH(V_INPUT);\n\
             V_START := V_STOP + 1;\n\
           END LOOP;\n\
           BEGIN\n\
             EXECUTE IMMEDIATE\n\
               'SELECT COUNT(*) FROM SYS.ALL_USERS '\n\
               || 'WHERE RAWTOHEX(UTL_I18N.STRING_TO_RAW(USERNAME, ''AL32UTF8'')) IN ('\n\
               || 'SELECT CAST(REGEXP_SUBSTR(:OWNER_LIST, ''[^,]+'', 1, LEVEL) '\n\
               || 'AS VARCHAR2(256)) FROM DUAL CONNECT BY LEVEL <= '\n\
               || 'REGEXP_COUNT(:OWNER_LIST, '','') + 1)'\n\
               INTO V_OWNER_PRESENT USING V_INPUT, V_INPUT;\n\
             IF V_OWNER_PRESENT <> V_OWNER_COUNT THEN\n\
               RAISE_APPLICATION_ERROR(-20006, 'owner-not-found');\n\
             END IF;\n\
           EXCEPTION\n\
             WHEN OTHERS THEN\n\
               -- ALL_USERS is normally public but may be revoked\n\
               -- on hardened estates. In that case the converter proves each\n\
               -- owner from the granted floor catalogues instead.\n\
               IF SQLCODE NOT IN (-942, -1031) THEN RAISE; END IF;\n\
           END;\n\
         END;\n\
         /\n\
         WHENEVER SQLERROR CONTINUE NONE\n\
         SPOOL dbwarp-blueprint-oracle-basic.raw\n\
         COLUMN DBWARP_BP_FRAME FORMAT A32000\n\
         PROMPT {raw_prefix}|H|{contract}|{family}|{manifest}|{script_digest}|{query_count}\n\
         SELECT '{raw_prefix}|C|' || TO_CHAR(ROUND(\n\
                  (CAST(SYS_EXTRACT_UTC(SYSTIMESTAMP) AS DATE) - DATE '1970-01-01') * 86400000),\n\
                  'FM99999999999999999990', 'NLS_NUMERIC_CHARACTERS=''.,''') FROM DUAL;\n\
         SELECT '{raw_prefix}|M|CAPTURED_AT|T' || RAWTOHEX(UTL_I18N.STRING_TO_RAW(\n\
                  TO_CHAR(SYS_EXTRACT_UTC(SYSTIMESTAMP), 'YYYY-MM-DD\"T\"HH24:MI:SS.FF3\"Z\"'),\n\
                  'AL32UTF8')) FROM DUAL;\n\
         SELECT '{raw_prefix}|M|PRINCIPAL|T' || RAWTOHEX(UTL_I18N.STRING_TO_RAW(\n\
                  SYS_CONTEXT('USERENV', 'SESSION_USER'), 'AL32UTF8')) FROM DUAL;\n\
         SELECT '{raw_prefix}|M|DATABASE|T' || RAWTOHEX(UTL_I18N.STRING_TO_RAW(\n\
                  SYS_CONTEXT('USERENV', 'DB_NAME') || '/' || SYS_CONTEXT('USERENV', 'CON_NAME'),\n\
                  'AL32UTF8')) FROM DUAL;\n\
         SELECT '{raw_prefix}|M|SERVER_VERSION|T' || RAWTOHEX(UTL_I18N.STRING_TO_RAW(\n\
                  VERSION, 'AL32UTF8')) FROM SYS.V_$INSTANCE;\n\
         SELECT '{raw_prefix}|M|CLIENT_RELEASE|T' || RAWTOHEX(UTL_I18N.STRING_TO_RAW(\n\
                  :DBWARP_BP_CLIENT_RELEASE, 'AL32UTF8')) FROM DUAL;\n\
         SELECT '{raw_prefix}|O|' || TO_CHAR(LEVEL, 'FM99999999999999999990') || '|T' ||\n\
                CAST(REGEXP_SUBSTR(:DBWARP_BP_OWNER_HEX_LIST, '[^,]+', 1, LEVEL)\n\
                     AS VARCHAR2(256))\n\
           FROM DUAL\n\
         CONNECT BY LEVEL <= REGEXP_COUNT(:DBWARP_BP_OWNER_HEX_LIST, ',') + 1;\n",
        family = family.label(),
        server_major = family.server_major(),
        server_minor = family.expected_minor().map_or(-1, i32::from),
        max_owners = MAX_OWNER_COUNT,
        raw_prefix = RAW_PREFIX,
        contract = SPOOL_CONTRACT_VERSION,
        manifest = hex::encode(manifest),
        script_digest = script_digest,
        portable_checksum = portable_checksum,
        query_count = query_count,
    );

    let mut sequence = 0_usize;
    for query in queries_for_tier(OracleCaptureTier::Basic) {
        sequence += 1;
        if !query.applies_to(family.server_major()) {
            out.push_str(&format!("PROMPT {RAW_PREFIX}|Q|{sequence}|S|VERSION\n"));
            continue;
        }
        let sql = render_multi_owner_query(query, version);
        let columns = query.output_columns_for_version(version);
        let encoded_columns = columns
            .iter()
            .map(|column| {
                format!(
                    "CASE WHEN q.{column} IS NULL THEN 'N' \
                     WHEN UTL_RAW.LENGTH(UTL_I18N.STRING_TO_RAW(TO_CHAR(q.{column}), 'AL32UTF8')) \
                          > {value_limit} THEN 'L' \
                     ELSE 'T' || RAWTOHEX(UTL_I18N.STRING_TO_RAW(TO_CHAR(q.{column}), \
                     'AL32UTF8')) END",
                    value_limit = limits.transient_value_bytes
                )
            })
            .collect::<Vec<_>>()
            .join(" || '|' ||\n");
        let over_width_tokens = std::iter::once("L")
            .chain(std::iter::repeat_n("N", columns.len().saturating_sub(1)))
            .collect::<Vec<_>>()
            .join("|");
        out.push_str(&format!(
            "PROMPT {RAW_PREFIX}|Q|{sequence}|{query_id}|{view}|{width}|B\n\
             SELECT CASE\n\
                      WHEN LENGTH(DBWARP_BP_FULL_FRAME) > {frame_limit} THEN\n\
                        TO_CLOB('{RAW_PREFIX}|R|{sequence}|') ||\n\
                        TO_CHAR(DBWARP_BP_ROW_NUMBER, 'FM99999999999999999990') ||\n\
                        '|{over_width_tokens}|Z'\n\
                      ELSE DBWARP_BP_FULL_FRAME\n\
                    END AS DBWARP_BP_FRAME\n\
               FROM (SELECT ROWNUM AS DBWARP_BP_ROW_NUMBER,\n\
                            TO_CLOB('{RAW_PREFIX}|R|{sequence}|') ||\n\
                            TO_CHAR(ROWNUM, 'FM99999999999999999990') || '|' ||\n\
                            {encoded_columns} || '|Z' AS DBWARP_BP_FULL_FRAME\n\
                       FROM ({sql}) q) DBWARP_BP_BOUNDED_ROWS;\n\
             PROMPT {RAW_PREFIX}|Q|{sequence}|E\n",
            query_id = query.query_id,
            view = query.view,
            width = columns.len(),
            frame_limit = MAX_LINE_BYTES,
        ));
    }
    out.push_str(&format!(
        "SELECT '{RAW_PREFIX}|X|{sequence}|' || TO_CHAR(ROUND(\n\
                (CAST(SYS_EXTRACT_UTC(SYSTIMESTAMP) AS DATE) - DATE '1970-01-01') * 86400000),\n\
                'FM99999999999999999990', 'NLS_NUMERIC_CHARACTERS=''.,''') FROM DUAL;\n\
         SPOOL OFF\n\
         EXIT SUCCESS\n"
    ));
    out
}

fn render_multi_owner_query(
    query: &crate::oracle_catalog::CatalogQuery,
    version: OracleVersion,
) -> String {
    let projection = query.projected_columns_for_version(version).join(", ");
    let mut predicates = Vec::new();
    if let Some(owner) = query.owner_column {
        predicates.push(format!(
            "RAWTOHEX(UTL_I18N.STRING_TO_RAW({owner}, 'AL32UTF8')) IN (\
             SELECT CAST(REGEXP_SUBSTR(:DBWARP_BP_OWNER_HEX_LIST, '[^,]+', 1, LEVEL) \
                         AS VARCHAR2(256)) \
             FROM DUAL CONNECT BY LEVEL <= \
             REGEXP_COUNT(:DBWARP_BP_OWNER_HEX_LIST, ',') + 1)"
        ));
    }
    if let Some(predicate) = query.fixed_predicate {
        predicates.push(predicate.to_string());
    }
    let where_clause = if predicates.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", predicates.join(" AND "))
    };
    format!(
        "SELECT {projection} FROM SYS.{view}{where_clause}",
        view = query.view
    )
}

fn manifest_sha256(version: OracleVersion) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash_field(&mut hash, b"dbwarp-blueprint-oracle-basic-dba-spool/v1");
    hash_field(&mut hash, &version.components()[0].to_be_bytes());
    for query in queries_for_tier(OracleCaptureTier::Basic) {
        hash_field(&mut hash, query.query_id.as_bytes());
        hash_field(&mut hash, query.view.as_bytes());
        hash_field(&mut hash, query.owner_column.unwrap_or("").as_bytes());
        hash_field(&mut hash, query.render_sql_for_version(version).as_bytes());
        for column in query.output_columns_for_version(version) {
            hash_field(&mut hash, column.as_bytes());
        }
    }
    hash.finalize().into()
}

fn hash_field(hash: &mut Sha256, value: &[u8]) {
    hash.update((value.len() as u64).to_be_bytes());
    hash.update(value);
}

pub fn decode_oracle_basic_dba_spool(bytes: &[u8]) -> Result<DecodedDbaSpool> {
    decode_oracle_basic_dba_spool_reader(BufReader::new(Cursor::new(bytes)))
}

pub(crate) fn decode_oracle_basic_dba_spool_reader<R: BufRead>(
    reader: R,
) -> Result<DecodedDbaSpool> {
    let mut lines = BoundedSpoolLines::new(reader).peekable();
    let header_line = next_spool_line(&mut lines, "Oracle Basic DBA spool is empty")?;
    let header = split_line(&header_line)?;
    if header.len() != 6 || header[0] != PREFIX || header[1] != "H" {
        bail!("DBP1505E Oracle Basic DBA spool has an invalid header");
    }
    let contract = parse_u16(header[2], "spool contract")?;
    if contract != SPOOL_CONTRACT_VERSION {
        bail!("DBP1505E unsupported Oracle Basic DBA spool contract");
    }
    let family = OracleDbaScriptFamily::parse(header[3])
        .ok_or_else(|| anyhow!("DBP1505E Oracle Basic DBA spool names an unknown family"))?;
    let supplied_manifest = decode_digest(header[4])?;
    let supplied_script_digest = decode_digest(header[5])?;
    if supplied_script_digest != oracle_basic_dba_script_digest(family) {
        bail!("DBP1505E Oracle Basic DBA spool script digest does not match this build");
    }

    let captured_at = read_metadata(&mut lines, "CAPTURED_AT")?;
    chrono::DateTime::parse_from_rfc3339(&captured_at)
        .map_err(|_| anyhow!("DBP1505E Oracle Basic DBA spool has an invalid capture time"))?;
    let capturing_principal = read_metadata(&mut lines, "PRINCIPAL")?;
    let database_identity = read_metadata(&mut lines, "DATABASE")?;
    let server_release = read_metadata(&mut lines, "SERVER_VERSION")?;
    let client_release = read_metadata(&mut lines, "CLIENT_RELEASE")?;
    if capturing_principal.is_empty()
        || database_identity.is_empty()
        || capturing_principal.len() > 128
        || database_identity.len() > 512
        || capturing_principal.chars().any(char::is_control)
        || database_identity.chars().any(char::is_control)
    {
        bail!("DBP1505E Oracle Basic DBA spool has invalid identity metadata");
    }
    if database_identity
        .rsplit_once('/')
        .is_some_and(|(_, container)| container == "CDB$ROOT")
    {
        bail!("DBP1505E Oracle Basic DBA spool was captured from CDB$ROOT");
    }
    let server_version = parse_dotted_version(&server_release)?;
    if !family.matches_server_version(server_version) {
        bail!("DBP1505E Oracle Basic DBA spool server family does not match its script");
    }
    if supplied_manifest != manifest_sha256(server_version) {
        bail!("DBP1505E Oracle Basic DBA spool manifest does not match this build");
    }
    let recorded_client_version = parse_sqlplus_release(&client_release);

    let mut owners = Vec::new();
    loop {
        let owner_line = match lines.peek() {
            Some(Ok(line)) if line.starts_with(&format!("{PREFIX}|O|")) => true,
            Some(Ok(_)) | None => false,
            Some(Err(_)) => {
                next_spool_line(&mut lines, "reading Oracle Basic DBA spool owner manifest")?;
                unreachable!("an I/O error cannot produce an owner line")
            }
        };
        if !owner_line {
            break;
        }
        let line = next_spool_line(
            &mut lines,
            "Oracle Basic DBA spool owner manifest is incomplete",
        )?;
        let fields = split_line(&line)?;
        if fields.len() != 4
            || fields[0] != PREFIX
            || fields[1] != "O"
            || parse_usize(fields[2], "owner ordinal")? != owners.len() + 1
        {
            bail!("DBP1505E Oracle Basic DBA spool owner manifest is malformed");
        }
        let owner = decode_text_token(fields[3], 128)?;
        if owner.is_empty()
            || owner.chars().any(char::is_control)
            || owners.last().is_some_and(|previous| previous >= &owner)
        {
            bail!("DBP1505E Oracle Basic DBA spool owner list is not sorted and unique");
        }
        owners.push(owner);
        if owners.len() > MAX_OWNER_COUNT {
            bail!("DBP1505E Oracle Basic DBA spool exceeds its owner-count bound");
        }
    }
    if owners.is_empty() {
        bail!("DBP1505E Oracle Basic DBA spool has no selected owner");
    }
    let scope =
        OracleOwnerScope::from_offline_capture(owners, OracleOwnerScopeKind::SelectedOwners, false)
            .map_err(|_| anyhow!("DBP1505E Oracle Basic DBA spool has an invalid owner scope"))?;
    let expected = queries_for_tier(OracleCaptureTier::Basic).collect::<Vec<_>>();
    let mut limits = crate::oracle_sqlplus::SQLPLUS_BASIC_MAX_LIMITS;
    let mut outcomes = Vec::with_capacity(expected.len());
    let mut retained_rows = 0_u64;
    let mut retained_bytes = 0_u64;
    let mut discarded_rows = false;
    let mut observed_abort_boundary = None;
    let mut unattributed_abort_tail = false;

    for (index, query) in expected.iter().enumerate() {
        let sequence = index + 1;
        let line = next_spool_line(
            &mut lines,
            "Oracle Basic DBA spool ended before its query manifest",
        )?;
        let fields = split_line(&line)?;
        if fields.len() == 5
            && fields[0] == PREFIX
            && fields[1] == "Q"
            && parse_usize(fields[2], "query sequence")? == sequence
            && fields[3] == "S"
        {
            let status = match fields[4] {
                "VERSION" if !query.applies_to(server_version.components()[0]) => {
                    OracleQueryStatus::SkippedVersion
                }
                "NOT_REACHED" if observed_abort_boundary.is_some() => OracleQueryStatus::NotReached,
                "NOT_REACHED" if unattributed_abort_tail => OracleQueryStatus::NotReached,
                "NOT_REACHED" => {
                    unattributed_abort_tail = true;
                    OracleQueryStatus::NotReached
                }
                _ => bail!("DBP1505E Oracle Basic DBA spool has an invalid query skip"),
            };
            push_query_outcomes(
                &mut outcomes,
                query,
                scope.owners(),
                server_version,
                status,
                Vec::new(),
            )?;
            continue;
        }
        if observed_abort_boundary.is_some() || unattributed_abort_tail {
            bail!("DBP1505E Oracle Basic DBA spool resumed after its abort boundary");
        }
        let width = query.output_columns_for_version(server_version).len();
        if fields.len() != 7
            || fields[0] != PREFIX
            || fields[1] != "Q"
            || parse_usize(fields[2], "query sequence")? != sequence
            || fields[3] != query.query_id
            || fields[4] != query.view
            || parse_usize(fields[5], "query width")? != width
            || fields[6] != "B"
        {
            bail!("DBP1505E Oracle Basic DBA spool does not match the query manifest");
        }
        let mut rows = Vec::new();
        loop {
            let line = next_spool_line(
                &mut lines,
                "Oracle Basic DBA spool ended inside a query frame",
            )?;
            let fields = split_line(&line)?;
            if fields.len() == width + 5
                && fields[0] == PREFIX
                && fields[1] == "R"
                && parse_usize(fields[2], "row query sequence")? == sequence
                && parse_usize(fields[3], "row ordinal")? == rows.len() + 1
                && fields.last().copied() == Some("Z")
            {
                ensure_spool_row_can_be_retained(retained_rows, limits.catalog_rows)?;
                let mut values = Vec::with_capacity(width);
                for token in &fields[4..fields.len() - 1] {
                    let value = decode_value_token(token, limits.transient_value_bytes as usize)?;
                    retained_bytes = retained_bytes
                        .checked_add(value_bytes(&value))
                        .ok_or_else(|| anyhow!("DBP1505E Oracle Basic DBA spool byte overflow"))?;
                    values.push(value);
                }
                retained_rows = retained_rows
                    .checked_add(1)
                    .ok_or_else(|| anyhow!("DBP1505E Oracle Basic DBA spool row overflow"))?;
                rows.push(OracleRow { values });
                continue;
            }
            if fields.len() == 5
                && fields[0] == PREFIX
                && fields[1] == "Q"
                && parse_usize(fields[2], "query sequence")? == sequence
                && fields[3] == "E"
                && parse_u64(fields[4], "query row count")? == rows.len() as u64
            {
                push_query_outcomes(
                    &mut outcomes,
                    query,
                    scope.owners(),
                    server_version,
                    OracleQueryStatus::Executed {
                        rows: rows.len() as u64,
                    },
                    rows,
                )?;
                break;
            }
            if fields.len() == 8
                && fields[0] == PREFIX
                && fields[1] == "Q"
                && parse_usize(fields[2], "query sequence")? == sequence
                && fields[3] == "F"
                && matches!(fields[5], "ORA" | "LOCAL")
            {
                let observed = parse_u64(fields[7], "failed-query row count")?;
                if observed < rows.len() as u64 {
                    bail!("DBP1505E Oracle Basic DBA spool failure row count is inconsistent");
                }
                let error_number = parse_u64(fields[6], "Oracle error number")?;
                let class = decode_failure_class(fields[4], fields[5], error_number)?;
                if class == OracleQueryFailure::SessionLost {
                    observed_abort_boundary = Some(OracleCaptureAbort::SessionLost);
                }
                discarded_rows |= !rows.is_empty() || observed != 0;
                push_query_outcomes(
                    &mut outcomes,
                    query,
                    scope.owners(),
                    server_version,
                    OracleQueryStatus::Failed { class },
                    Vec::new(),
                )?;
                break;
            }
            if fields.len() == 6
                && fields[0] == PREFIX
                && fields[1] == "Q"
                && parse_usize(fields[2], "query sequence")? == sequence
                && fields[3] == "A"
            {
                let observed = parse_u64(fields[5], "aborted-query row count")?;
                if observed < rows.len() as u64 || observed > rows.len() as u64 + 1 {
                    bail!("DBP1505E Oracle Basic DBA spool abort row count is inconsistent");
                }
                let reason = decode_abort(fields[4])?;
                observed_abort_boundary = Some(reason);
                discarded_rows = true;
                push_query_outcomes(
                    &mut outcomes,
                    query,
                    scope.owners(),
                    server_version,
                    OracleQueryStatus::ResultDiscarded { reason },
                    Vec::new(),
                )?;
                break;
            }
            bail!("DBP1505E Oracle Basic DBA spool has malformed query framing");
        }
    }

    let trailer_line = next_spool_line(&mut lines, "Oracle Basic DBA spool has no end sentinel")?;
    let trailer = split_line(&trailer_line)?;
    if trailer.len() != 7
        || trailer[0] != PREFIX
        || trailer[1] != "X"
        || parse_usize(trailer[2], "completed query count")? != expected.len()
    {
        bail!("DBP1505E Oracle Basic DBA spool has an invalid end sentinel");
    }
    let rows_consumed = parse_u64(trailer[3], "capture row count")?;
    let bytes_consumed = parse_u64(trailer[4], "capture byte count")?;
    let elapsed_ms = parse_u64(trailer[5], "capture elapsed time")?;
    let abort = if trailer[6] == "NONE" {
        None
    } else {
        Some(decode_abort(trailer[6])?)
    };
    let trailer_only_external_abort = observed_abort_boundary.is_none()
        && matches!(
            abort,
            Some(OracleCaptureAbort::SessionLost | OracleCaptureAbort::UnconfirmedCancellation)
        )
        && (unattributed_abort_tail
            || outcomes.len()
                == expected
                    .iter()
                    .map(|query| {
                        if query.owner_column.is_some() {
                            scope.owners().len()
                        } else {
                            1
                        }
                    })
                    .sum::<usize>());
    if unattributed_abort_tail && !trailer_only_external_abort {
        bail!("DBP1505E Oracle Basic DBA spool has an unexplained unreached tail");
    }
    if abort != observed_abort_boundary && !trailer_only_external_abort {
        bail!("DBP1505E Oracle Basic DBA spool abort evidence is inconsistent");
    }
    for line in lines {
        let line =
            line.map_err(|_| anyhow!("DBP1505E reading Oracle Basic DBA spool framing failed"))?;
        if !line.trim().is_empty() {
            bail!("DBP1505E Oracle Basic DBA spool has data after its end sentinel");
        }
    }
    if retained_rows > rows_consumed || retained_bytes > bytes_consumed {
        bail!("DBP1505E Oracle Basic DBA spool retained data exceeds its counters");
    }
    if !discarded_rows && (retained_rows != rows_consumed || retained_bytes != bytes_consumed) {
        bail!("DBP1505E Oracle Basic DBA spool counters do not match retained data");
    }
    // The hands-off DBA producer is intentionally not a network-side deadline
    // controller.  A complete, internally consistent spool remains useful if
    // a slow customer catalogue took longer than the live adapter's deadline.
    // For this immutable offline input, the observed elapsed time itself is a
    // hard bound: the provider cannot perform more work during conversion.
    limits.elapsed_ms = limits.elapsed_ms.max(elapsed_ms.max(1));
    validate_spool_limits(
        rows_consumed,
        bytes_consumed,
        elapsed_ms,
        limits,
        abort,
        outcomes
            .iter()
            .map(|outcome| {
                queries_for_tier(OracleCaptureTier::Basic)
                    .find(|query| query.query_id == outcome.query_id)
                    .map(|query| query.output_columns_for_version(server_version).len() as u64)
                    .unwrap_or(0)
            })
            .max()
            .unwrap_or(0),
    )?;
    validate_selected_owners(
        &outcomes,
        scope.owners(),
        &capturing_principal,
        server_version,
    )?;
    let catalogs_read = catalogue_set(&outcomes, true);
    let catalogs_unreadable = catalogue_set(&outcomes, false);
    let session_discarded = matches!(
        abort,
        Some(OracleCaptureAbort::SessionLost | OracleCaptureAbort::UnconfirmedCancellation)
    );
    Ok(DecodedDbaSpool {
        capture: OracleCatalogCapture {
            outcomes,
            catalogs_read,
            catalogs_unreadable,
            abort,
            session_discarded,
            unconfirmed_server_work: matches!(
                abort,
                Some(OracleCaptureAbort::UnconfirmedCancellation)
            ),
            rows_consumed,
            bytes_consumed,
            lob_bytes_consumed: 0,
            elapsed_ms,
            // A DBA spool records one in-session release value. It is useful
            // provenance, but it is not the independent banner comparison
            // required to call a command client attested.
            client_version_attestation: Some(OracleClientVersionAttestation::Unreadable),
        },
        scope,
        server_version,
        artifact_detail: ArtifactDetail::Summary,
        limits,
        captured_at,
        capturing_principal,
        database_identity,
        recorded_client_version,
    })
}

struct BoundedSpoolLines<R> {
    reader: R,
    finished: bool,
}

impl<R> BoundedSpoolLines<R> {
    fn new(reader: R) -> Self {
        Self {
            reader,
            finished: false,
        }
    }
}

impl<R: BufRead> Iterator for BoundedSpoolLines<R> {
    type Item = std::io::Result<String>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        let mut bytes = Vec::new();
        let mut newline_terminated = false;
        loop {
            let available = match self.reader.fill_buf() {
                Ok(available) => available,
                Err(error) => return Some(Err(error)),
            };
            if available.is_empty() {
                self.finished = true;
                if bytes.is_empty() {
                    return None;
                }
                break;
            }
            let newline = available.iter().position(|byte| *byte == b'\n');
            let take = newline.unwrap_or(available.len());
            if bytes.len().saturating_add(take) > MAX_LINE_BYTES + 1 {
                self.finished = true;
                return Some(Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Oracle Basic DBA spool line exceeds its framing bound",
                )));
            }
            bytes.extend_from_slice(&available[..take]);
            self.reader.consume(take + usize::from(newline.is_some()));
            if newline.is_some() {
                newline_terminated = true;
                break;
            }
        }
        if newline_terminated && bytes.last() == Some(&b'\r') {
            bytes.pop();
        }
        if bytes.len() > MAX_LINE_BYTES {
            self.finished = true;
            return Some(Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Oracle Basic DBA spool line exceeds its framing bound",
            )));
        }
        Some(String::from_utf8(bytes).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Oracle Basic DBA spool framing is not UTF-8",
            )
        }))
    }
}

fn push_query_outcomes(
    outcomes: &mut Vec<OracleQueryOutcome>,
    query: &crate::oracle_catalog::CatalogQuery,
    owners: &[String],
    version: OracleVersion,
    status: OracleQueryStatus,
    rows: Vec<OracleRow>,
) -> Result<()> {
    let Some(owner_column) = query.owner_column else {
        outcomes.push(OracleQueryOutcome {
            query_id: query.query_id,
            view: query.view,
            owner_ordinal: None,
            status,
            rows,
        });
        return Ok(());
    };
    let columns = query.output_columns_for_version(version);
    let owner_index = columns
        .iter()
        .position(|column| column.eq_ignore_ascii_case(owner_column))
        .ok_or_else(|| {
            anyhow!("DBP1505E Oracle Basic query owner column is absent from its projection")
        })?;
    let mut grouped = vec![Vec::new(); owners.len()];
    if matches!(status, OracleQueryStatus::Executed { .. }) {
        for row in rows {
            let owner = match row.values.get(owner_index) {
                Some(OracleValue::Text(value) | OracleValue::Number(value)) => value,
                _ => bail!("DBP1505E Oracle Basic DBA spool row has no owner identity"),
            };
            let ordinal = owners
                .binary_search_by(|candidate| candidate.as_str().cmp(owner))
                .map_err(|_| {
                    anyhow!(
                        "DBP1505E Oracle Basic DBA spool row is outside the selected owner scope"
                    )
                })?
                + 1;
            grouped[ordinal - 1].push(row);
        }
    }
    for (index, owner_rows) in grouped.into_iter().enumerate() {
        let owner_status = match status {
            OracleQueryStatus::Executed { .. } => OracleQueryStatus::Executed {
                rows: owner_rows.len() as u64,
            },
            ref status => status.clone(),
        };
        outcomes.push(OracleQueryOutcome {
            query_id: query.query_id,
            view: query.view,
            owner_ordinal: Some((index + 1) as u32),
            status: owner_status,
            rows: owner_rows,
        });
    }
    Ok(())
}

fn validate_selected_owners(
    outcomes: &[OracleQueryOutcome],
    owners: &[String],
    capturing_principal: &str,
    version: OracleVersion,
) -> Result<()> {
    let mut observed = BTreeSet::new();
    observed.insert(capturing_principal.to_string());
    for query_id in ["oracle-users", "oracle-objects"] {
        let query = queries_for_tier(OracleCaptureTier::Basic)
            .find(|query| query.query_id == query_id)
            .expect("owner proof query belongs to the Basic registry");
        let owner_column = query
            .owner_column
            .unwrap_or_else(|| query.output_columns_for_version(version)[0]);
        let owner_index = query
            .output_columns_for_version(version)
            .iter()
            .position(|column| column.eq_ignore_ascii_case(owner_column))
            .unwrap_or(0);
        for outcome in outcomes.iter().filter(|outcome| {
            outcome.query_id == query_id
                && matches!(outcome.status, OracleQueryStatus::Executed { .. })
        }) {
            for row in &outcome.rows {
                if let Some(OracleValue::Text(owner) | OracleValue::Number(owner)) =
                    row.values.get(owner_index)
                {
                    observed.insert(owner.clone());
                }
            }
        }
    }
    if let Some(owner) = owners
        .iter()
        .find(|owner| !observed.contains(owner.as_str()))
    {
        let _ = owner;
        bail!("DBP1505E a requested Oracle owner could not be proven from the capture");
    }
    Ok(())
}

fn split_line(line: &str) -> Result<Vec<&str>> {
    if line.len() > MAX_LINE_BYTES || line.chars().any(|ch| ch == '\0' || ch == '\r') {
        bail!("DBP1505E Oracle Basic DBA spool line violates its framing bound");
    }
    Ok(line.split('|').collect())
}

fn next_spool_line<I>(lines: &mut std::iter::Peekable<I>, missing: &str) -> Result<String>
where
    I: Iterator<Item = std::io::Result<String>>,
{
    lines
        .next()
        .ok_or_else(|| anyhow!("DBP1505E {missing}"))?
        .map_err(|_| anyhow!("DBP1505E reading Oracle Basic DBA spool framing failed"))
}

fn read_metadata<I>(lines: &mut std::iter::Peekable<I>, expected: &str) -> Result<String>
where
    I: Iterator<Item = std::io::Result<String>>,
{
    let line = next_spool_line(lines, "Oracle Basic DBA spool metadata is incomplete")?;
    let fields = split_line(&line)?;
    if fields.len() != 4 || fields[0] != PREFIX || fields[1] != "M" || fields[2] != expected {
        bail!("DBP1505E Oracle Basic DBA spool metadata order is invalid");
    }
    decode_text_token(fields[3], 512)
}

fn decode_text_token(token: &str, max_bytes: usize) -> Result<String> {
    let encoded = token
        .strip_prefix('T')
        .ok_or_else(|| anyhow!("DBP1505E Oracle Basic DBA spool metadata is not hex text"))?;
    if encoded.len() % 2 != 0 || encoded.len() > max_bytes.saturating_mul(2) {
        bail!("DBP1505E Oracle Basic DBA spool text exceeds its bound");
    }
    let bytes = hex::decode(encoded)
        .map_err(|_| anyhow!("DBP1505E Oracle Basic DBA spool has invalid hex text"))?;
    String::from_utf8(bytes)
        .map_err(|_| anyhow!("DBP1505E Oracle Basic DBA spool text is not UTF-8"))
}

fn decode_value_token(token: &str, max_bytes: usize) -> Result<OracleValue> {
    if token == "N" {
        return Ok(OracleValue::Null);
    }
    Ok(OracleValue::Text(decode_text_token(token, max_bytes)?))
}

fn value_bytes(value: &OracleValue) -> u64 {
    match value {
        OracleValue::Null => 0,
        OracleValue::Number(value)
        | OracleValue::Text(value)
        | OracleValue::Timestamp(value)
        | OracleValue::LobText(value) => value.len() as u64,
    }
}

fn decode_failure_class(
    value: &str,
    source: &str,
    error_number: u64,
) -> Result<OracleQueryFailure> {
    let class = match (value, source, error_number) {
        ("PERMISSION_DENIED", "ORA", 1031) => OracleQueryFailure::PermissionDenied,
        ("OBJECT_ABSENT", "ORA", 942) => OracleQueryFailure::ObjectAbsent,
        ("SESSION_LOST", "ORA", 28 | 3113 | 3114 | 3135 | 12537 | 12547) => {
            OracleQueryFailure::SessionLost
        }
        ("DATABASE_ERROR", "ORA", code)
            if !matches!(code, 28 | 942 | 1031 | 3113 | 3114 | 3135 | 12537 | 12547) =>
        {
            OracleQueryFailure::DatabaseError
        }
        ("TRANSIENT_VALUE_LIMIT", "LOCAL", 0) => OracleQueryFailure::TransientValueLimitExceeded,
        _ => bail!("DBP1505E Oracle Basic DBA spool failure class disagrees with its ORA number"),
    };
    Ok(class)
}

fn decode_abort(value: &str) -> Result<OracleCaptureAbort> {
    match value {
        "ROW_LIMIT" => Ok(OracleCaptureAbort::RowLimitExceeded),
        "BYTE_LIMIT" => Ok(OracleCaptureAbort::ByteLimitExceeded),
        "TRANSIENT_VALUE_LIMIT" => Ok(OracleCaptureAbort::TransientValueLimitExceeded),
        "DEADLINE" => Ok(OracleCaptureAbort::DeadlineExceeded),
        "SESSION_LOST" => Ok(OracleCaptureAbort::SessionLost),
        "UNCONFIRMED_CANCELLATION" => Ok(OracleCaptureAbort::UnconfirmedCancellation),
        _ => bail!("DBP1505E Oracle Basic DBA spool has an unknown abort class"),
    }
}

fn parse_dotted_version(value: &str) -> Result<OracleVersion> {
    let components = value
        .trim()
        .split('.')
        .map(|component| {
            if component.is_empty() || !component.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            component.parse::<u16>().ok()
        })
        .collect::<Option<Vec<_>>>()
        .and_then(|components| OracleVersion::from_components(&components))
        .ok_or_else(|| anyhow!("DBP1505E Oracle Basic DBA spool has an invalid server version"))?;
    Ok(components)
}

fn parse_sqlplus_release(value: &str) -> Option<OracleVersion> {
    let value = value.trim();
    if value.len() != 10 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let components = (0..5)
        .map(|index| value[index * 2..index * 2 + 2].parse::<u16>().ok())
        .collect::<Option<Vec<_>>>()?;
    OracleVersion::from_components(&components)
}

fn parse_owner_ordinal(value: &str) -> Result<Option<u32>> {
    let value = parse_u64(value, "owner ordinal")?;
    if value == 0 {
        Ok(None)
    } else {
        u32::try_from(value)
            .map(Some)
            .map_err(|_| anyhow!("DBP1505E Oracle Basic DBA spool owner ordinal is too large"))
    }
}

fn parse_u16(value: &str, label: &str) -> Result<u16> {
    value
        .parse::<u16>()
        .with_context(|| format!("DBP1505E invalid Oracle Basic DBA {label}"))
}

fn parse_usize(value: &str, label: &str) -> Result<usize> {
    value
        .parse::<usize>()
        .with_context(|| format!("DBP1505E invalid Oracle Basic DBA {label}"))
}

fn parse_u64(value: &str, label: &str) -> Result<u64> {
    value
        .parse::<u64>()
        .with_context(|| format!("DBP1505E invalid Oracle Basic DBA {label}"))
}

fn decode_digest(value: &str) -> Result<[u8; 32]> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("DBP1505E Oracle Basic DBA spool has an invalid manifest digest");
    }
    hex::decode(value)
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| {
            anyhow!("DBP1505E Oracle Basic DBA spool manifest digest has the wrong width")
        })
}

fn catalogue_set(outcomes: &[OracleQueryOutcome], read: bool) -> Vec<&'static str> {
    outcomes
        .iter()
        .filter(|outcome| {
            if read {
                matches!(outcome.status, OracleQueryStatus::Executed { .. })
            } else {
                matches!(outcome.status, OracleQueryStatus::Failed { .. })
            }
        })
        .map(|outcome| outcome.view)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn validate_spool_limits(
    rows: u64,
    bytes: u64,
    elapsed_ms: u64,
    limits: OracleProviderLimits,
    abort: Option<OracleCaptureAbort>,
    max_width: u64,
) -> Result<()> {
    // The script counts the one cell that proves a value exceeded the
    // transient limit before it stops the row. SQL*Plus bounds the transport
    // frame at 32,000 bytes, so the justified single-row overrun is the preceding
    // bounded cells plus one client-frame-sized cell, not merely
    // `width * transient_value_bytes`.
    let max_overrun_bytes = max_width
        .saturating_mul(limits.transient_value_bytes)
        .saturating_add(MAX_LINE_BYTES as u64);
    let rows_valid = match abort {
        Some(OracleCaptureAbort::RowLimitExceeded) => rows == limits.catalog_rows.saturating_add(1),
        Some(OracleCaptureAbort::UnconfirmedCancellation) => {
            rows <= limits.catalog_rows.saturating_add(1)
        }
        _ => rows <= limits.catalog_rows,
    };
    let bytes_valid = match abort {
        Some(OracleCaptureAbort::ByteLimitExceeded) => {
            bytes > limits.catalog_bytes
                && bytes <= limits.catalog_bytes.saturating_add(max_overrun_bytes)
        }
        Some(
            OracleCaptureAbort::TransientValueLimitExceeded
            | OracleCaptureAbort::UnconfirmedCancellation,
        ) => bytes <= limits.catalog_bytes.saturating_add(max_overrun_bytes),
        _ => bytes <= limits.catalog_bytes,
    };
    let elapsed_valid = match abort {
        Some(OracleCaptureAbort::DeadlineExceeded) => elapsed_ms >= limits.elapsed_ms,
        Some(OracleCaptureAbort::UnconfirmedCancellation) => true,
        _ => elapsed_ms <= limits.elapsed_ms,
    };
    if !rows_valid || !bytes_valid || !elapsed_valid {
        bail!("DBP1505E Oracle Basic DBA spool counters exceed their justified limits");
    }
    Ok(())
}

fn ensure_spool_row_can_be_retained(retained_rows: u64, row_limit: u64) -> Result<()> {
    if retained_rows >= row_limit.saturating_add(1) {
        bail!("DBP1505E Oracle Basic DBA spool exceeds its bounded retained-row population");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oracle_basic::{map_oracle_basic_capture, OracleBasicOptions};
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    fn text_token(value: &str) -> String {
        format!("T{}", hex::encode(value.as_bytes()))
    }

    fn fixture_spool(capture: &OracleCatalogCapture, version: OracleVersion) -> Vec<u8> {
        let mut out = String::new();
        let family = match version.components()[0] {
            12 if version.components().get(1) == Some(&1) => "oracle-12.1",
            12 if version.components().get(1) == Some(&2) => "oracle-12.2",
            19 => "oracle-19c",
            21 => "oracle-21c",
            23 => "oracle-26ai",
            major => panic!("unsupported fixture server major {major}"),
        };
        let server_release = version
            .components()
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(".");
        let client_release = version
            .components()
            .iter()
            .map(|component| format!("{component:02}"))
            .collect::<String>();
        out.push_str(&format!(
            "{PREFIX}|H|{SPOOL_CONTRACT_VERSION}|{family}|{}|{}\n",
            hex::encode(manifest_sha256(version)),
            hex::encode(oracle_basic_dba_script_digest(
                OracleDbaScriptFamily::parse(family).unwrap()
            ))
        ));
        for (name, value) in [
            ("CAPTURED_AT", "2026-09-28T00:00:00Z".to_string()),
            ("PRINCIPAL", "BPCOLLECTOR".to_string()),
            ("DATABASE", "XE/XEPDB1".to_string()),
            ("SERVER_VERSION", server_release),
            ("CLIENT_RELEASE", client_release),
        ] {
            out.push_str(&format!("{PREFIX}|M|{name}|{}\n", text_token(&value)));
        }
        out.push_str(&format!("{PREFIX}|O|1|{}\n", text_token("APP")));
        let mut sequence = 0_usize;
        let mut rows = 0_u64;
        let mut bytes = 0_u64;
        for query in queries_for_tier(OracleCaptureTier::Basic) {
            let owner_ordinal = query.owner_column.map(|_| 1_u32);
            let outcome = capture
                .outcomes
                .iter()
                .find(|outcome| {
                    outcome.query_id == query.query_id && outcome.owner_ordinal == owner_ordinal
                })
                .unwrap();
            sequence += 1;
            match outcome.status {
                OracleQueryStatus::Executed { .. } => {
                    let width = query.output_columns_for_version(version).len();
                    out.push_str(&format!(
                        "{PREFIX}|Q|{sequence}|{}|{}|{width}|B\n",
                        query.query_id, query.view
                    ));
                    for (row_index, row) in outcome.rows.iter().enumerate() {
                        out.push_str(&format!("{PREFIX}|R|{sequence}|{}", row_index + 1));
                        for value in &row.values {
                            let token = match value {
                                OracleValue::Null => "N".to_string(),
                                OracleValue::Number(value)
                                | OracleValue::Text(value)
                                | OracleValue::Timestamp(value)
                                | OracleValue::LobText(value) => {
                                    bytes += value.len() as u64;
                                    text_token(value)
                                }
                            };
                            out.push('|');
                            out.push_str(&token);
                        }
                        out.push_str("|Z\n");
                        rows += 1;
                    }
                    out.push_str(&format!("{PREFIX}|Q|{sequence}|E|{}\n", outcome.rows.len()));
                }
                OracleQueryStatus::Failed { class } => {
                    let (class, code) = match class {
                        OracleQueryFailure::PermissionDenied => ("PERMISSION_DENIED", 1031),
                        OracleQueryFailure::ObjectAbsent => ("OBJECT_ABSENT", 942),
                        OracleQueryFailure::SessionLost => ("SESSION_LOST", 3113),
                        _ => ("DATABASE_ERROR", 600),
                    };
                    out.push_str(&format!(
                        "{PREFIX}|Q|{sequence}|{}|{}|{}|B\n",
                        query.query_id,
                        query.view,
                        query.output_columns_for_version(version).len()
                    ));
                    out.push_str(&format!("{PREFIX}|Q|{sequence}|F|{class}|ORA|{code}|0\n"));
                }
                OracleQueryStatus::ResultDiscarded { reason } => {
                    let reason = match reason {
                        OracleCaptureAbort::DeadlineExceeded => "DEADLINE",
                        OracleCaptureAbort::RowLimitExceeded => "ROW_LIMIT",
                        OracleCaptureAbort::ByteLimitExceeded => "BYTE_LIMIT",
                        _ => "TRANSIENT_VALUE_LIMIT",
                    };
                    out.push_str(&format!(
                        "{PREFIX}|Q|{sequence}|{}|{}|{}|B\n",
                        query.query_id,
                        query.view,
                        query.output_columns_for_version(version).len()
                    ));
                    out.push_str(&format!("{PREFIX}|Q|{sequence}|A|{reason}|0\n"));
                }
                OracleQueryStatus::NotReached => {
                    out.push_str(&format!("{PREFIX}|Q|{sequence}|S|NOT_REACHED\n"));
                }
                OracleQueryStatus::SkippedVersion => {
                    out.push_str(&format!("{PREFIX}|Q|{sequence}|S|VERSION\n"));
                }
                OracleQueryStatus::SkippedDetail | OracleQueryStatus::OptionAbsent { .. } => {
                    panic!("fixture uses no detail or option skip")
                }
            }
        }
        let abort = match capture.abort {
            None => "NONE",
            Some(OracleCaptureAbort::DeadlineExceeded) => "DEADLINE",
            Some(OracleCaptureAbort::RowLimitExceeded) => "ROW_LIMIT",
            Some(OracleCaptureAbort::ByteLimitExceeded) => "BYTE_LIMIT",
            Some(_) => "TRANSIENT_VALUE_LIMIT",
        };
        out.push_str(&format!(
            "{PREFIX}|X|{sequence}|{rows}|{bytes}|{}|{abort}\n",
            capture.elapsed_ms.max(1)
        ));
        out.into_bytes()
    }

    #[test]
    fn generated_script_binds_hex_decoded_owners_and_carries_required_frames() {
        let script = render_oracle_basic_dba_script(OracleDbaScriptFamily::Oracle19c);
        let limits = crate::oracle_sqlplus::SQLPLUS_BASIC_MAX_LIMITS;
        assert!(script.contains("HEXTORAW(V_HEX)"));
        assert!(script.contains("V_INPUT CLOB := :DBWARP_BP_OWNER_HEX_LIST"));
        assert!(script.contains("CAST(REGEXP_SUBSTR(:DBWARP_BP_OWNER_HEX_LIST"));
        assert!(script.contains("HEX_COMPARE(V_PREVIOUS_HEX, V_HEX) >= 0"));
        assert!(!script.contains("DBMS_SQL"));
        assert!(!script.contains("DBMS_OUTPUT"));
        assert!(!script.contains("UTL_RAW.COMPARE"));
        assert!(script.contains("FUNCTION HEX_COMPARE"));
        assert!(!script.contains("owner = '&"));
        assert!(script.contains("SET ROWLIMIT OFF"));
        assert!(script.contains("SET SQLCASE MIXED"));
        assert!(script.contains("NLS_NUMERIC_CHARACTERS = '.,'"));
        assert!(script.contains("NLS_CALENDAR = 'GREGORIAN'"));
        assert!(script.contains("FROM SYS.ALL_USERS"));
        assert!(script.contains("RAISE_APPLICATION_ERROR(-20006, 'owner-not-found')"));
        assert!(script.contains("EXECUTE IMMEDIATE"));
        assert!(script.contains("IF SQLCODE NOT IN (-942, -1031) THEN RAISE; END IF;"));
        let strict = script.find("WHENEVER SQLERROR EXIT SQL.SQLCODE").unwrap();
        let nls = script.find("NLS_NUMERIC_CHARACTERS = '.,'").unwrap();
        let owner_proof = script.find("FROM SYS.ALL_USERS").unwrap();
        let spool = script
            .find("SPOOL dbwarp-blueprint-oracle-basic.raw")
            .unwrap();
        let degrade = script.find("WHENEVER SQLERROR CONTINUE NONE").unwrap();
        assert!(strict < nls && nls < degrade);
        assert!(owner_proof < spool);
        assert!(script.contains("DBWARP_BP_RAW|Q|1|"));
        assert!(script.contains("DBWARP_BP_RAW|X|"));
        assert!(script.contains(&format!("> {} THEN 'L'", limits.transient_value_bytes)));
        let unix_sanitizer = include_str!("../sql/capture/sanitize.awk");
        let windows_sanitizer = include_str!("../sql/capture/sanitize.ps1");
        assert!(unix_sanitizer.contains(&format!("max_rows = {}", limits.catalog_rows)));
        assert!(unix_sanitizer.contains(&format!("max_bytes = {}", limits.catalog_bytes)));
        assert!(windows_sanitizer.contains(&format!("totalRows -gt {}", limits.catalog_rows)));
        assert!(windows_sanitizer.contains(&format!("totalBytes -gt {}", limits.catalog_bytes)));
        for query in queries_for_tier(OracleCaptureTier::Basic) {
            assert!(script.contains(query.query_id));
            assert!(script.contains(query.view));
        }
    }

    #[test]
    fn portable_script_checksum_matches_posix_cksum_vectors() {
        assert_eq!(posix_cksum(b""), 4_294_967_295);
        assert_eq!(posix_cksum(b"abc"), 1_219_131_554);
        assert_eq!(posix_cksum(b"123456789"), 930_766_865);
        let script = render_oracle_basic_dba_script(OracleDbaScriptFamily::Oracle19c);
        assert!(!script.contains(&format!("-- POSIX cksum: {PORTABLE_CHECKSUM_PLACEHOLDER}")));
    }

    #[test]
    fn family_manifests_are_versioned() {
        let v12_1 = OracleVersion::from_components(&[12, 1]).unwrap();
        let v12_2 = OracleVersion::from_components(&[12, 2]).unwrap();
        let v19 = OracleVersion::from_components(&[19]).unwrap();
        let v21 = OracleVersion::from_components(&[21]).unwrap();
        assert_ne!(manifest_sha256(v12_1), manifest_sha256(v12_2));
        assert_ne!(manifest_sha256(v19), manifest_sha256(v21));
        let script_12_1 = render_oracle_basic_dba_script(OracleDbaScriptFamily::Oracle12cR1);
        let script_12_2 = render_oracle_basic_dba_script(OracleDbaScriptFamily::Oracle12cR2);
        assert!(script_12_1.contains("Server family: oracle-12.1"));
        assert!(script_12_1.contains("C_EXPECTED_MINOR CONSTANT PLS_INTEGER := 1;"));
        assert!(!script_12_1.contains("search_condition_vc"));
        assert!(script_12_2.contains("Server family: oracle-12.2"));
        assert!(script_12_2.contains("C_EXPECTED_MINOR CONSTANT PLS_INTEGER := 2;"));
        assert!(script_12_2.contains("search_condition_vc"));
        assert_eq!(OracleDbaScriptFamily::Oracle26ai.server_major(), 23);
        let script = render_oracle_basic_dba_script(OracleDbaScriptFamily::Oracle26ai);
        assert!(script.contains("Server family: oracle-26ai"));
        assert!(script.contains("C_EXPECTED_MAJOR CONSTANT PLS_INTEGER := 23;"));
        assert!(script.contains("C_EXPECTED_MINOR CONSTANT PLS_INTEGER := -1;"));
        assert!(OracleDbaScriptFamily::Oracle26ai
            .matches_server_version(OracleVersion::from_components(&[23, 0, 0, 0, 0]).unwrap()));
        let version = OracleVersion::from_components(&[23, 26, 3, 0, 0]).unwrap();
        let decoded = decode_oracle_basic_dba_spool(&fixture_spool(
            &crate::oracle_basic::tests::fixture(),
            version,
        ))
        .unwrap();
        assert_eq!(decoded.server_version, version);
    }

    #[test]
    fn family_matching_distinguishes_the_two_12c_catalogues() {
        let release_12_1 = OracleVersion::from_components(&[12, 1, 0, 2]).unwrap();
        let release_12_2 = OracleVersion::from_components(&[12, 2, 0, 1]).unwrap();
        assert!(OracleDbaScriptFamily::Oracle12cR1.matches_server_version(release_12_1));
        assert!(!OracleDbaScriptFamily::Oracle12cR1.matches_server_version(release_12_2));
        assert!(OracleDbaScriptFamily::Oracle12cR2.matches_server_version(release_12_2));
        assert!(!OracleDbaScriptFamily::Oracle12cR2.matches_server_version(release_12_1));
        assert_eq!(
            OracleDbaScriptFamily::parse("12c-r1"),
            Some(OracleDbaScriptFamily::Oracle12cR1)
        );
        assert_eq!(
            OracleDbaScriptFamily::parse("12.2"),
            Some(OracleDbaScriptFamily::Oracle12cR2)
        );
    }

    #[test]
    fn damaged_spool_cannot_retain_more_than_the_single_abort_row() {
        assert!(ensure_spool_row_can_be_retained(999, 1_000).is_ok());
        assert!(ensure_spool_row_can_be_retained(1_000, 1_000).is_ok());
        assert!(ensure_spool_row_can_be_retained(1_001, 1_000).is_err());
    }

    #[test]
    fn dba_spool_abort_reason_must_prove_its_threshold() {
        let limits = crate::oracle_sqlplus::SQLPLUS_BASIC_MAX_LIMITS;
        assert!(validate_spool_limits(
            limits.catalog_rows,
            0,
            1,
            limits,
            Some(OracleCaptureAbort::RowLimitExceeded),
            1,
        )
        .is_err());
        assert!(validate_spool_limits(
            limits.catalog_rows + 1,
            0,
            1,
            limits,
            Some(OracleCaptureAbort::RowLimitExceeded),
            1,
        )
        .is_ok());
        assert!(validate_spool_limits(
            0,
            limits.catalog_bytes,
            1,
            limits,
            Some(OracleCaptureAbort::ByteLimitExceeded),
            1,
        )
        .is_err());
        assert!(validate_spool_limits(
            0,
            limits.catalog_bytes + 1,
            1,
            limits,
            Some(OracleCaptureAbort::ByteLimitExceeded),
            1,
        )
        .is_ok());
        assert!(validate_spool_limits(
            0,
            0,
            limits.elapsed_ms - 1,
            limits,
            Some(OracleCaptureAbort::DeadlineExceeded),
            1,
        )
        .is_err());
        assert!(validate_spool_limits(
            0,
            0,
            limits.elapsed_ms,
            limits,
            Some(OracleCaptureAbort::DeadlineExceeded),
            1,
        )
        .is_ok());
    }

    #[test]
    fn shipped_script_pack_is_exact_registry_output() {
        let cases = [
            (
                OracleDbaScriptFamily::Oracle12cR1,
                include_str!("../sql/capture/oracle-12c/basic-12.1.sql"),
            ),
            (
                OracleDbaScriptFamily::Oracle12cR2,
                include_str!("../sql/capture/oracle-12c/basic-12.2.sql"),
            ),
            (
                OracleDbaScriptFamily::Oracle19c,
                include_str!("../sql/capture/oracle-19c/basic.sql"),
            ),
            (
                OracleDbaScriptFamily::Oracle21c,
                include_str!("../sql/capture/oracle-21c/basic.sql"),
            ),
            (
                OracleDbaScriptFamily::Oracle26ai,
                include_str!("../sql/capture/oracle-26ai/basic.sql"),
            ),
        ];
        for (family, committed) in cases {
            assert_eq!(committed, render_oracle_basic_dba_script(family));
        }
    }

    #[test]
    fn dispatcher_selects_every_shipped_server_family() {
        let dispatcher = include_str!("../sql/capture/run.sql");
        for path in [
            "oracle-12c/basic-12.1.sql",
            "oracle-12c/basic-12.2.sql",
            "oracle-19c/basic.sql",
            "oracle-21c/basic.sql",
            "oracle-26ai/basic.sql",
        ] {
            assert!(dispatcher.contains(path), "dispatcher omits {path}");
        }
        assert!(dispatcher.contains("FROM SYS.V_$INSTANCE"));
        assert!(dispatcher.contains("VARIABLE DBWARP_BP_OWNER_HEX_LIST CLOB"));
        assert!(dispatcher.contains("unsupported-server-family"));
        assert!(dispatcher.contains("@./dbwarp-blueprint-owner-bind.sql"));
        let reset = dispatcher.find("SET DEFINE ON").unwrap();
        let include = dispatcher.find("@@&DBWARP_BP_CAPTURE_SCRIPT").unwrap();
        assert!(reset < include);
        let fail_closed_exit = dispatcher.rfind("EXIT FAILURE").unwrap();
        assert!(include < fail_closed_exit);
    }

    #[test]
    fn windows_launcher_uses_private_files_and_the_same_sanitized_contract() {
        let launcher = include_str!("../sql/capture/capture.ps1");
        let sanitizer = include_str!("../sql/capture/sanitize.ps1");
        assert!(launcher.contains("SetAccessRuleProtection($true, $false)"));
        assert!(launcher.contains("dbwarp-blueprint-owner-bind.sql"));
        assert!(launcher.contains("sanitize.ps1"));
        assert!(launcher.contains("$env:SQLPATH"));
        assert!(launcher.contains("The executed capture script does not match"));
        assert!(!launcher.contains("@$PSScriptRoot\\run.sql\" $ownerHexList"));
        assert!(launcher.contains("Remove-IncompleteFinalRecord $rawPath"));
        assert!(launcher.contains("[Console]::Error.WriteLine($message)"));
        assert!(launcher.contains("Get-StartupFailureMessage $status $rawPath"));
        assert!(launcher.contains("$chunks = @(for ($offset = 0;"));
        assert!(launcher.contains("$Status -gt 255"));
        assert!(launcher.contains("Oracle rejected the collector credentials (ORA-01017)"));
        assert!(launcher.contains("owner input was malformed (ORA-20001)"));
        assert!(launcher.contains("session connected to CDB$ROOT (ORA-20005)"));
        assert!(launcher.contains("requested owner was not found (ORA-20006)"));
        assert!(sanitizer.contains("UTF8Encoding($false, $false)"));
        assert!(sanitizer.contains("UNCONFIRMED_CANCELLATION"));
        assert!(sanitizer.contains(
            "throw 'DBP1426E Capture output violated the DBWarp Blueprint framing contract.'"
        ));
        assert!(!sanitizer.contains("CANARY"));
    }

    #[cfg(unix)]
    #[test]
    fn unix_launcher_encodes_sorts_and_protects_the_spool() {
        use std::process::Command;

        let launcher = include_str!("../sql/capture/capture.sh");
        let sanitizer = include_str!("../sql/capture/sanitize.awk");
        assert!(!launcher.contains("mapfile"));
        assert!(!launcher.contains("connect_user^^"));
        assert!(!launcher.contains("sha256sum"));
        assert!(!launcher.contains("shasum"));
        assert!(launcher.contains("SQLPATH=\"$script_dir\""));
        assert!(!launcher.contains("SQLPATH=\"$PWD:"));
        assert!(launcher.contains("startup_failure_message"));
        assert!(launcher.contains("Oracle rejected the collector credentials (ORA-01017)"));
        assert!(launcher.contains("owner input was malformed (ORA-20001)"));
        assert!(launcher.contains("session connected to CDB\\$ROOT (ORA-20005)"));
        assert!(launcher.contains("requested owner was not found (ORA-20006)"));
        assert!(!sanitizer.contains("systime("));

        let nonce = format!(
            "oracle-capture-launcher-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tmp")
            .join(nonce);
        let bin = root.join("bin");
        let run = root.join("run with spaces");
        let pack = root.join("capture pack");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(&run).unwrap();
        std::fs::create_dir_all(pack.join("oracle-19c")).unwrap();
        for relative in ["capture.sh", "sanitize.awk", "run.sql"] {
            std::fs::copy(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("sql/capture")
                    .join(relative),
                pack.join(relative),
            )
            .unwrap();
        }
        std::fs::copy(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("sql/capture/oracle-19c/basic.sql"),
            pack.join("oracle-19c/basic.sql"),
        )
        .unwrap();
        let sqlplus = bin.join("sqlplus");
        let arguments = root.join("arguments");
        let raw = root.join("raw");
        std::fs::write(
            &raw,
            format!(
                "DBWARP_BP_RAW|H|2|oracle-19c|{}|{}|0\nDBWARP_BP_RAW|C|1\nDBWARP_BP_RAW|X|0|2\n",
                hex::encode(manifest_sha256(
                    OracleVersion::from_components(&[19]).unwrap()
                )),
                hex::encode(oracle_basic_dba_script_digest(
                    OracleDbaScriptFamily::Oracle19c
                ))
            ),
        )
        .unwrap();
        std::fs::write(
            &sqlplus,
            "#!/bin/sh\nprintf '%s\\n' \"$@\" >\"$DBWARP_CAPTURE_TEST_ARGS\"\nprintf 'SQLPATH=%s\\n' \"$SQLPATH\" >>\"$DBWARP_CAPTURE_TEST_ARGS\"\ncat dbwarp-blueprint-owner-bind.sql >>\"$DBWARP_CAPTURE_TEST_ARGS\"\ncp \"$DBWARP_CAPTURE_TEST_RAW\" dbwarp-blueprint-oracle-basic.raw\n",
        )
        .unwrap();
        std::fs::set_permissions(&sqlplus, std::fs::Permissions::from_mode(0o700)).unwrap();
        let inherited_path = std::env::var_os("PATH").unwrap_or_default();
        let path = format!("{}:{}", bin.display(), inherited_path.to_string_lossy());
        let owners = run.join("owners with spaces.txt");
        let mut owner_lines = vec!["Δ".to_string(), "APP".to_string()];
        owner_lines.extend((0..130).map(|index| format!("O{index:04}{}", "X".repeat(123))));
        std::fs::write(&owners, format!("{}\n", owner_lines.join("\n"))).unwrap();
        let output = Command::new("bash")
            .arg(pack.join("capture.sh"))
            .args(["BPCOLLECTOR@service", "--owners-file"])
            .arg(&owners)
            .current_dir(&run)
            .env("PATH", path)
            .env("DBWARP_CAPTURE_TEST_ARGS", &arguments)
            .env("DBWARP_CAPTURE_TEST_RAW", &raw)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let invoked = std::fs::read_to_string(arguments).unwrap();
        assert!(invoked.contains("BPCOLLECTOR@service"));
        assert!(invoked.contains("capture pack/run.sql"));
        assert!(invoked.contains(&format!("SQLPATH={}", pack.display())));
        assert!(!invoked.contains(&format!("SQLPATH={}", run.display())));
        assert!(invoked.contains("TO_CLOB('415050,"));
        assert!(invoked.contains("CE94';"));
        assert!(invoked.contains(":DBWARP_BP_OWNER_HEX_LIST := :DBWARP_BP_OWNER_HEX_LIST ||"));
        let spool = run.join("dbwarp-blueprint-oracle-basic.spool");
        assert_eq!(
            std::fs::metadata(spool).unwrap().permissions().mode() & 0o777,
            0o600
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn unix_launcher_maps_pre_frame_oracle_failure_without_echoing_native_text() {
        use std::os::unix::fs::PermissionsExt;
        use std::process::Command;

        let nonce = format!(
            "oracle-capture-startup-error-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tmp")
            .join(nonce);
        let bin = root.join("bin");
        let run = root.join("run");
        let pack = root.join("pack");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(&run).unwrap();
        std::fs::create_dir_all(&pack).unwrap();
        for relative in ["capture.sh", "sanitize.awk", "run.sql"] {
            std::fs::copy(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("sql/capture")
                    .join(relative),
                pack.join(relative),
            )
            .unwrap();
        }
        let sqlplus = bin.join("sqlplus");
        std::fs::write(
            &sqlplus,
            "#!/bin/sh\nprintf '%s\\n' 'ORA-01017: CANARY_PRIVATE_NAME' > dbwarp-blueprint-oracle-basic.raw\nexit 7\n",
        )
        .unwrap();
        std::fs::set_permissions(&sqlplus, std::fs::Permissions::from_mode(0o700)).unwrap();
        let inherited_path = std::env::var_os("PATH").unwrap_or_default();
        let path = format!("{}:{}", bin.display(), inherited_path.to_string_lossy());
        let output = Command::new("bash")
            .arg(pack.join("capture.sh"))
            .args(["BPCOLLECTOR@service", "APP"])
            .current_dir(&run)
            .env("PATH", &path)
            .output()
            .unwrap();
        assert!(!output.status.success());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert_eq!(stderr.lines().count(), 1, "{stderr}");
        assert!(stderr.contains("DBP1426E Oracle rejected the collector credentials"));
        assert!(!stderr.contains("CANARY_PRIVATE_NAME"));

        std::fs::write(&sqlplus, "#!/bin/sh\nexit \"$DBWARP_CAPTURE_TEST_EXIT\"\n").unwrap();
        for (exit_code, expected) in [
            ("223", "owner input was malformed (ORA-20001)"),
            ("219", "session connected to CDB$ROOT (ORA-20005)"),
            ("218", "requested owner was not found (ORA-20006)"),
        ] {
            let output = Command::new("bash")
                .arg(pack.join("capture.sh"))
                .args(["BPCOLLECTOR@service", "APP"])
                .current_dir(&run)
                .env("PATH", &path)
                .env("DBWARP_CAPTURE_TEST_EXIT", exit_code)
                .output()
                .unwrap();
            assert!(!output.status.success());
            let stderr = String::from_utf8(output.stderr).unwrap();
            assert_eq!(stderr.lines().count(), 1, "{stderr}");
            assert!(stderr.contains(expected), "{stderr}");
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn unix_launcher_sanitizes_the_completed_prefix_after_ctrl_c() {
        use std::process::{Command, Stdio};

        let nonce = format!(
            "oracle-capture-interrupt-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tmp")
            .join(nonce);
        let bin = root.join("bin");
        let run = root.join("run");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(&run).unwrap();
        let sqlplus = bin.join("sqlplus");
        let controller = root.join("controller.sh");
        let launcher_pid_path = run.join("launcher.pid");
        let digest = hex::encode(oracle_basic_dba_script_digest(
            OracleDbaScriptFamily::Oracle19c,
        ));
        std::fs::write(
            &sqlplus,
            format!(
                "#!/bin/sh\n\
                 trap 'exit 130' INT\n\
                 printf '%s\\n' \\
                   'DBWARP_BP_RAW|H|2|oracle-19c|{}|{}|1' \\
                   'DBWARP_BP_RAW|C|1' \\
                   'DBWARP_BP_RAW|Q|1|oracle-test|DBA_TEST|1|B' \\
                   > dbwarp-blueprint-oracle-basic.raw\n\
                 printf '%s' 'DBWARP_BP_RAW|R|1|1|T41' >> dbwarp-blueprint-oracle-basic.raw\n\
                 while :; do sleep 1; done\n",
                hex::encode(manifest_sha256(
                    OracleVersion::from_components(&[19]).unwrap()
                )),
                digest,
            ),
        )
        .unwrap();
        std::fs::set_permissions(&sqlplus, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::write(
            &controller,
            "#!/usr/bin/env bash\n\
             set -euo pipefail\n\
             launcher=$1\n\
             pid_path=$2\n\
             shift 2\n\
             (\n\
               trap - INT\n\
               exec \"$launcher\" \"$@\"\n\
             ) &\n\
             launcher_pid=$!\n\
             printf '%s\\n' \"$launcher_pid\" >\"$pid_path\"\n\
             wait \"$launcher_pid\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&controller, std::fs::Permissions::from_mode(0o700)).unwrap();
        let inherited_path = std::env::var_os("PATH").unwrap_or_default();
        let path = format!("{}:{}", bin.display(), inherited_path.to_string_lossy());
        let mut command = Command::new("bash");
        command
            .arg(&controller)
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/sql/capture/capture.sh"
            ))
            .arg(&launcher_pid_path)
            .args(["BPCOLLECTOR@service", "APP"])
            .current_dir(&run)
            .env("PATH", path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = command.spawn().unwrap();
        let raw = run.join("dbwarp-blueprint-oracle-basic.raw");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while std::time::Instant::now() < deadline {
            if std::fs::read_to_string(&raw).is_ok_and(|value| value.contains("DBWARP_BP_RAW|Q|1|"))
            {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(raw.is_file(), "mock SQLPlus did not open its raw spool");
        let launcher_pid = std::fs::read_to_string(&launcher_pid_path)
            .unwrap()
            .trim()
            .parse::<i32>()
            .unwrap();
        let result = unsafe { libc::kill(launcher_pid, libc::SIGINT) };
        assert_eq!(result, 0, "could not interrupt the background launcher");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "launcher did not finish after Ctrl-C"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        assert!(
            status.success(),
            "launcher discarded the interrupted prefix"
        );
        let spool =
            std::fs::read_to_string(run.join("dbwarp-blueprint-oracle-basic.spool")).unwrap();
        assert!(spool.contains("DBWARP_BP|Q|1|A|UNCONFIRMED_CANCELLATION|0"));
        assert!(!spool.contains("T41"));
        assert!(spool.ends_with("|UNCONFIRMED_CANCELLATION\n"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn sanitizer_preserves_a_real_partial_stream_without_oracle_message_text() {
        use std::process::Command;

        let version = OracleVersion::from_components(&[21, 3, 0, 0, 0]).unwrap();
        let complete = String::from_utf8(fixture_spool(
            &crate::oracle_basic::tests::fixture(),
            version,
        ))
        .unwrap();
        let query_count = queries_for_tier(OracleCaptureTier::Basic).count();
        let mut raw = String::new();
        for line in complete.lines() {
            let fields = line.split('|').collect::<Vec<_>>();
            if fields.get(1) == Some(&"H") {
                raw.push_str(&line.replacen(PREFIX, RAW_PREFIX, 1));
                raw.push('|');
                raw.push_str(&query_count.to_string());
                raw.push('\n');
            } else if fields.get(1) == Some(&"Q") && fields.get(2) == Some(&"22") {
                if fields.last() == Some(&"B") {
                    raw.push_str(&line.replacen(PREFIX, RAW_PREFIX, 1));
                    raw.push('\n');
                    raw.push_str("ORA-01013: CANARY_PRIVATE_TABLE_NAME\n");
                    raw.push_str("DBWARP_BP_RAW|Q|22|E\n");
                }
                break;
            } else if fields.get(1) == Some(&"Q") && fields.get(3) == Some(&"E") {
                raw.push_str(&fields[..4].join("|").replacen(PREFIX, RAW_PREFIX, 1));
                raw.push('\n');
            } else if fields.get(1) != Some(&"X") {
                raw.push_str(&line.replacen(PREFIX, RAW_PREFIX, 1));
                raw.push('\n');
            }
        }
        let nonce = format!(
            "oracle-capture-sanitize-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tmp")
            .join(nonce);
        std::fs::create_dir_all(&root).unwrap();
        let raw_path = root.join("capture.raw");
        let spool_path = root.join("capture.spool");
        std::fs::write(&raw_path, raw).unwrap();
        let output = Command::new("awk")
            .args(["-f"])
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/sql/capture/sanitize.awk"
            ))
            .arg(&raw_path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!output.stdout.windows(6).any(|window| window == b"CANARY"));
        std::fs::write(&spool_path, &output.stdout).unwrap();
        let decoded = decode_oracle_basic_dba_spool(&output.stdout).unwrap();
        assert_eq!(
            decoded.capture.abort,
            Some(OracleCaptureAbort::UnconfirmedCancellation)
        );
        let blueprint = map_oracle_basic_capture(
            &decoded.capture,
            &decoded.scope,
            decoded.server_version,
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: Some(decoded.captured_at.clone()),
                artifact_detail: decoded.artifact_detail,
            },
        )
        .expect("the completed floor before cancellation must still map");
        assert!(!blueprint.tables.is_empty());
        let denied_raw = format!(
            "DBWARP_BP_RAW|H|2|oracle-21c|{}|{}|1\nDBWARP_BP_RAW|C|1\nDBWARP_BP_RAW|Q|1|oracle-lobs|DBA_LOBS|11|B\n  FROM SYS.DBA_LOBS WHERE OWNER = 'CANARY_NATIVE_OWNER'\n       *\nERROR at line 1:\nORA-01031: CANARY_DENIED_NATIVE_NAME\nDBWARP_BP_RAW|Q|1|E\nDBWARP_BP_RAW|X|1|2\n",
            hex::encode(manifest_sha256(version)),
            hex::encode(oracle_basic_dba_script_digest(
                OracleDbaScriptFamily::Oracle21c
            ))
        );
        std::fs::write(&raw_path, denied_raw).unwrap();
        let denied = Command::new("awk")
            .args(["-f"])
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/sql/capture/sanitize.awk"
            ))
            .arg(&raw_path)
            .output()
            .unwrap();
        let denied = String::from_utf8(denied.stdout).unwrap();
        assert!(denied.contains("DBWARP_BP|Q|1|F|PERMISSION_DENIED|ORA|1031|0"));
        assert!(!denied.contains("CANARY"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn sanitizer_preserves_cancellation_between_query_frames() {
        use std::process::Command;

        let version = OracleVersion::from_components(&[21, 3, 0, 0, 0]).unwrap();
        let complete = String::from_utf8(fixture_spool(
            &crate::oracle_basic::tests::fixture(),
            version,
        ))
        .unwrap();
        let query_count = queries_for_tier(OracleCaptureTier::Basic).count();
        let mut raw = String::new();
        for line in complete.lines() {
            let fields = line.split('|').collect::<Vec<_>>();
            if fields.get(1) == Some(&"H") {
                raw.push_str(&line.replacen(PREFIX, RAW_PREFIX, 1));
                raw.push('|');
                raw.push_str(&query_count.to_string());
                raw.push('\n');
                raw.push_str("DBWARP_BP_RAW|C|1\n");
            } else if fields.get(1) == Some(&"Q") && fields.get(3) == Some(&"E") {
                raw.push_str(&fields[..4].join("|").replacen(PREFIX, RAW_PREFIX, 1));
                raw.push('\n');
                raw.push_str("DBWARP_BP_RAW|A|UNCONFIRMED_CANCELLATION\n");
                break;
            } else if fields.get(1) != Some(&"X") {
                raw.push_str(&line.replacen(PREFIX, RAW_PREFIX, 1));
                raw.push('\n');
            }
        }
        let nonce = format!(
            "oracle-capture-between-queries-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tmp")
            .join(nonce);
        std::fs::create_dir_all(&root).unwrap();
        let raw_path = root.join("capture.raw");
        std::fs::write(&raw_path, raw).unwrap();
        let output = Command::new("awk")
            .args(["-f"])
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/sql/capture/sanitize.awk"
            ))
            .arg(&raw_path)
            .output()
            .unwrap();
        assert!(output.status.success());
        let decoded = decode_oracle_basic_dba_spool(&output.stdout).unwrap();
        assert_eq!(
            decoded.capture.abort,
            Some(OracleCaptureAbort::UnconfirmedCancellation)
        );
        assert!(decoded
            .capture
            .outcomes
            .iter()
            .any(|outcome| matches!(outcome.status, OracleQueryStatus::NotReached)));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn unix_sanitizer_keeps_session_row_and_byte_limit_prefixes() {
        use std::process::Command;

        let nonce = format!(
            "oracle-capture-bounds-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tmp")
            .join(nonce);
        std::fs::create_dir_all(&root).unwrap();
        let sanitizer = concat!(env!("CARGO_MANIFEST_DIR"), "/sql/capture/sanitize.awk");
        let run = |name: &str, raw: &str, variables: &[(&str, &str)]| {
            let raw_path = root.join(format!("{name}.raw"));
            std::fs::write(&raw_path, raw).unwrap();
            let mut command = Command::new("awk");
            for (key, value) in variables {
                command.args(["-v", &format!("{key}={value}")]);
            }
            command.args(["-f", sanitizer]).arg(raw_path);
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "{name}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout).unwrap()
        };

        let row_limited = run(
            "rows",
            "DBWARP_BP_RAW|H|2|oracle-19c|m|s|1\n\
             DBWARP_BP_RAW|C|1000\n\
             DBWARP_BP_RAW|Q|1|oracle-test|DBA_TEST|1|B\n\
             DBWARP_BP_RAW|R|1|1|T41|Z\n\
             DBWARP_BP_RAW|R|1|2|T42|Z\n\
             DBWARP_BP_RAW|Q|1|E\n\
             DBWARP_BP_RAW|X|1|2000\n\
             DBWARP_BP_RAW|A|UNCONFIRMED_CANCELLATION\n",
            &[("max_rows", "1")],
        );
        assert!(row_limited.contains("DBWARP_BP|R|1|1|T41|Z"));
        assert!(!row_limited.contains("|R|1|2|T42|Z"));
        assert!(row_limited.contains("DBWARP_BP|Q|1|A|ROW_LIMIT|2"));
        assert!(row_limited.ends_with("|2|2|1000|ROW_LIMIT\n"));

        let byte_limited = run(
            "bytes",
            "DBWARP_BP_RAW|H|2|oracle-19c|m|s|1\n\
             DBWARP_BP_RAW|C|1000\n\
             DBWARP_BP_RAW|Q|1|oracle-test|DBA_TEST|1|B\n\
             DBWARP_BP_RAW|R|1|1|T4142|Z\n\
             DBWARP_BP_RAW|Q|1|E\n\
             DBWARP_BP_RAW|X|1|2000\n",
            &[("max_bytes", "1")],
        );
        assert!(byte_limited.contains("DBWARP_BP|Q|1|A|BYTE_LIMIT|1"));
        assert!(byte_limited.ends_with("|1|2|1000|BYTE_LIMIT\n"));

        let lost = run(
            "lost",
            "DBWARP_BP_RAW|H|2|oracle-19c|m|s|1\n\
             DBWARP_BP_RAW|C|1000\n\
             DBWARP_BP_RAW|Q|1|oracle-test|DBA_TEST|1|B\n\
             DBWARP_BP_RAW|R|1|1|T41|Z\n",
            &[],
        );
        assert!(lost.contains("DBWARP_BP|Q|1|A|SESSION_LOST|1"));
        assert!(lost.ends_with("|1|1|0|SESSION_LOST\n"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn sanitizer_refuses_malformed_framing_without_echoing_native_text() {
        use std::process::Command;

        let version = OracleVersion::from_components(&[21, 3, 0, 0, 0]).unwrap();
        let raw = format!(
            "DBWARP_BP_RAW|H|2|oracle-21c|{}|{}|1\n\
             DBWARP_BP_RAW|C|1\n\
             DBWARP_BP_RAW|Q|1|oracle-test|DBA_TEST|1|B\n\
             DBWARP_BP_RAW|R|1|1|CANARY_NATIVE_VALUE|Z\n",
            hex::encode(manifest_sha256(version)),
            hex::encode(oracle_basic_dba_script_digest(
                OracleDbaScriptFamily::Oracle21c
            ))
        );
        let nonce = format!(
            "oracle-capture-malformed-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tmp")
            .join(nonce);
        std::fs::create_dir_all(&root).unwrap();
        let raw_path = root.join("capture.raw");
        std::fs::write(&raw_path, raw).unwrap();
        let output = Command::new("awk")
            .args(["-f"])
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/sql/capture/sanitize.awk"
            ))
            .arg(&raw_path)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(!output.stdout.windows(6).any(|window| window == b"CANARY"));
        assert!(!output.stderr.windows(6).any(|window| window == b"CANARY"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn spool_failure_class_and_abort_boundary_are_cross_checked() {
        let version = OracleVersion::from_components(&[21, 3, 0, 0, 0]).unwrap();
        let mut denied = crate::oracle_basic::tests::fixture();
        let denied_outcome = denied
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-lobs")
            .unwrap();
        denied_outcome.status = OracleQueryStatus::Failed {
            class: OracleQueryFailure::PermissionDenied,
        };
        denied_outcome.rows.clear();
        let denied_spool = String::from_utf8(fixture_spool(&denied, version)).unwrap();
        let mismatched_class = denied_spool.replacen(
            "|F|PERMISSION_DENIED|ORA|1031|",
            "|F|OBJECT_ABSENT|ORA|1031|",
            1,
        );
        assert!(decode_oracle_basic_dba_spool(mismatched_class.as_bytes()).is_err());

        let mut partial = crate::oracle_basic::tests::fixture();
        let mut reached = false;
        for query in queries_for_tier(OracleCaptureTier::Basic) {
            let outcome = partial
                .outcomes
                .iter_mut()
                .find(|outcome| outcome.query_id == query.query_id)
                .unwrap();
            if query.query_id == "oracle-lobs" {
                outcome.status = OracleQueryStatus::ResultDiscarded {
                    reason: OracleCaptureAbort::DeadlineExceeded,
                };
                outcome.rows.clear();
                reached = true;
            } else if reached {
                outcome.status = OracleQueryStatus::NotReached;
                outcome.rows.clear();
            }
        }
        partial.abort = Some(OracleCaptureAbort::DeadlineExceeded);
        partial.elapsed_ms = crate::oracle_sqlplus::SQLPLUS_BASIC_MAX_LIMITS.elapsed_ms;
        let partial_spool = String::from_utf8(fixture_spool(&partial, version)).unwrap();
        let missing_trailer_abort = partial_spool.replacen("|DEADLINE\n", "|NONE\n", 1);
        assert!(decode_oracle_basic_dba_spool(missing_trailer_abort.as_bytes()).is_err());
    }

    #[test]
    fn spool_refuses_unproven_owner_and_cdb_root() {
        let version = OracleVersion::from_components(&[21, 3, 0, 0, 0]).unwrap();
        let complete = String::from_utf8(fixture_spool(
            &crate::oracle_basic::tests::fixture(),
            version,
        ))
        .unwrap();
        let wrong_owner = complete.replacen("DBWARP_BP|O|1|T415050", "DBWARP_BP|O|1|T617070", 1);
        let error = decode_oracle_basic_dba_spool(wrong_owner.as_bytes()).unwrap_err();
        assert!(
            format!("{error:#}").contains("outside the selected owner scope"),
            "{error:#}"
        );
        let root = complete.replacen(
            &format!("DBWARP_BP|M|DATABASE|{}", text_token("XE/XEPDB1")),
            &format!("DBWARP_BP|M|DATABASE|{}", text_token("XE/CDB$ROOT")),
            1,
        );
        let error = decode_oracle_basic_dba_spool(root.as_bytes()).unwrap_err();
        assert!(format!("{error:#}").contains("CDB$ROOT"), "{error:#}");

        let mut empty_scope = crate::oracle_basic::tests::fixture();
        for outcome in &mut empty_scope.outcomes {
            if outcome.owner_ordinal.is_some() || outcome.query_id == "oracle-users" {
                outcome.rows.clear();
                outcome.status = OracleQueryStatus::Executed { rows: 0 };
            }
        }
        let typo = String::from_utf8(fixture_spool(&empty_scope, version))
            .unwrap()
            .replacen("DBWARP_BP|O|1|T415050", "DBWARP_BP|O|1|T617070", 1);
        let error = decode_oracle_basic_dba_spool(typo.as_bytes()).unwrap_err();
        assert!(
            format!("{error:#}").contains("could not be proven"),
            "{error:#}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn private_dba_spool_file_is_streamed_and_hashed_without_a_full_file_buffer() {
        let version = OracleVersion::from_components(&[21, 3, 0, 0, 0]).unwrap();
        let bytes = fixture_spool(&crate::oracle_basic::tests::fixture(), version);
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tmp/tests")
            .join(format!("oracle-dba-streaming-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("capture.spool");
        std::fs::write(&path, &bytes).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();

        let decoded = crate::oracle_offline::read_oracle_basic_stream(&path).unwrap();
        assert_eq!(
            decoded.input_kind,
            crate::oracle_offline::OracleBasicOfflineInputKind::DbaSpool
        );
        let expected_digest: [u8; 32] = Sha256::digest(&bytes).into();
        assert_eq!(decoded.stream_sha256, expected_digest);
        assert!(decoded.capture.rows_consumed > 0);

        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn windows_crlf_dba_spool_keeps_the_same_bounded_contract() {
        let version = OracleVersion::from_components(&[21, 3, 0, 0, 0]).unwrap();
        let unix = String::from_utf8(fixture_spool(
            &crate::oracle_basic::tests::fixture(),
            version,
        ))
        .unwrap();
        let windows = unix.replace('\n', "\r\n");
        let decoded = decode_oracle_basic_dba_spool(windows.as_bytes()).unwrap();
        assert!(decoded.capture.rows_consumed > 0);
        assert_eq!(decoded.server_version, version);
    }

    #[test]
    fn complete_degraded_and_partial_spools_all_reach_the_basic_mapper() {
        let version = OracleVersion::from_components(&[21, 3, 0, 0, 0]).unwrap();
        let cases = [
            ("complete", crate::oracle_basic::tests::fixture()),
            ("denied-optional", {
                let mut capture = crate::oracle_basic::tests::fixture();
                let outcome = capture
                    .outcomes
                    .iter_mut()
                    .find(|outcome| outcome.query_id == "oracle-lobs")
                    .unwrap();
                outcome.status = OracleQueryStatus::Failed {
                    class: OracleQueryFailure::PermissionDenied,
                };
                outcome.rows.clear();
                capture
            }),
            ("partial", {
                let mut capture = crate::oracle_basic::tests::fixture();
                let mut reached = false;
                for query in queries_for_tier(OracleCaptureTier::Basic) {
                    let outcome = capture
                        .outcomes
                        .iter_mut()
                        .find(|outcome| outcome.query_id == query.query_id)
                        .unwrap();
                    if query.query_id == "oracle-lobs" {
                        outcome.status = OracleQueryStatus::ResultDiscarded {
                            reason: OracleCaptureAbort::DeadlineExceeded,
                        };
                        outcome.rows.clear();
                        reached = true;
                    } else if reached {
                        outcome.status = OracleQueryStatus::NotReached;
                        outcome.rows.clear();
                    }
                }
                capture.abort = Some(OracleCaptureAbort::DeadlineExceeded);
                capture.elapsed_ms = crate::oracle_sqlplus::SQLPLUS_BASIC_MAX_LIMITS.elapsed_ms;
                capture
            }),
        ];
        for (label, capture) in cases {
            let decoded = decode_oracle_basic_dba_spool(&fixture_spool(&capture, version))
                .unwrap_or_else(|error| panic!("{label} spool failed: {error:#}"));
            let blueprint = map_oracle_basic_capture(
                &decoded.capture,
                &decoded.scope,
                decoded.server_version,
                &OracleBasicOptions {
                    source_kind: "production".to_string(),
                    generated_at_pin: Some(decoded.captured_at),
                    artifact_detail: decoded.artifact_detail,
                },
            )
            .unwrap_or_else(|error| panic!("{label} mapper failed: {error:#}"));
            dbwarp_blueprint_core::validate_blueprint_contract(&blueprint)
                .unwrap_or_else(|error| panic!("{label} Blueprint failed: {error:#}"));
            assert_eq!(blueprint.tables.len(), 1, "{label}");
            let table = blueprint.tables.values().next().unwrap();
            assert!(!table.cols.is_empty(), "{label} lost the column floor");
            let statistics = table
                .statistics
                .as_ref()
                .unwrap_or_else(|| panic!("{label} lost row and size evidence"));
            assert_ne!(statistics.row_count_quality, "unavailable", "{label}");
            assert_ne!(statistics.size_quality, "unavailable", "{label}");
        }
    }

    #[test]
    fn a_complete_slow_dba_capture_is_not_refused_as_an_environment_failure() {
        let version = OracleVersion::from_components(&[21, 3, 0, 0, 0]).unwrap();
        let mut capture = crate::oracle_basic::tests::fixture();
        capture.elapsed_ms = crate::oracle_sqlplus::SQLPLUS_BASIC_MAX_LIMITS
            .elapsed_ms
            .saturating_add(1);
        let decoded = decode_oracle_basic_dba_spool(&fixture_spool(&capture, version)).unwrap();
        assert_eq!(decoded.capture.abort, None);
        assert_eq!(decoded.capture.elapsed_ms, capture.elapsed_ms);
        assert_eq!(decoded.limits.elapsed_ms, capture.elapsed_ms);
    }
}
