//! MySQL engine — catalog reader (Tier 1) + compression sampler (Tier 2).
//!
//! Connects via `mysql_async` (rustls TLS feature). Reads
//! information_schema only in Tier 1. Tier 2 additionally runs
//! `SELECT * FROM <table> LIMIT N` (MySQL has no native TABLESAMPLE),
//! flagged as biased. zstd-compresses locally, records ratio + stddev,
//! discards bytes.
//!
//! Anonymization + rounding identical to engine_pg.

use std::collections::BTreeMap;
use std::time::Instant;

use anyhow::{anyhow, bail, Context, Result};
use clap::ValueEnum;
use mysql_async::consts::ColumnType as MyColumnType;
use mysql_async::prelude::*;
use mysql_async::{OptsBuilder, Pool, SslOpts, Value};
use zeroize::Zeroizing;

use crate::artifacts::{
    self, ArtifactDetail, CaptureCompleteness, RawArtifact, RawExternalPrerequisite,
    RawLanguageAnalysis,
};
use crate::audit::AuditLog;
use crate::engine_common::{
    accumulate_table_totals, elapsed_ms, percent_decode, rtt_percentiles_ms,
    warn_compression_unavailable,
};
use crate::format::{
    self, BlueprintColumn, BlueprintCompression, BlueprintFile, BlueprintIndex, BlueprintTable,
    FkEdge, Totals, SCHEMA_VERSION,
};
use crate::sample_compression::{
    CompressionTicket, CompressionWorkerPool, PreparedCompressionSample,
};
use crate::sample_encode::{self, Cell, TypeTag};
use crate::schema_scope::{resolved_selection, SchemaSelection};
use crate::secret::Secret;
use crate::style;
use crate::tls::{TlsMode, TlsParams};
use crate::topology::{sort_dedup, sort_topology, warn_incomplete_dataset_scope};

/// MySQL `binary` charset id from `INFORMATION_SCHEMA.COLLATIONS` /
/// the wire protocol — when this charset is set on a column, the
/// bytes are not text in any encoding, they're raw binary.
const MYSQL_CHARSET_BINARY: u16 = 63;
const STYLE_PEEK_BYTES: usize = 4096;

/// Retain only the narrow numeric product version in the transferable
/// Blueprint. `VERSION()` banners may contain hostnames, distribution names,
/// build paths, or other producer-controlled text; callers that need the raw
/// banner for in-session feature detection keep it transiently.
fn normalized_mysql_version(raw: &str) -> String {
    let Some(start) = raw.find(|ch: char| ch.is_ascii_digit()) else {
        return "unknown".to_string();
    };
    let candidate: String = raw[start..]
        .chars()
        .take_while(|ch| ch.is_ascii_digit() || *ch == '.')
        .collect();
    let normalized = candidate.trim_end_matches('.');
    if normalized.is_empty() {
        "unknown".to_string()
    } else {
        normalized.to_string()
    }
}

/// Controls how length metadata is anonymized without conflating schema
/// fidelity with sampled-value fidelity.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum LengthFidelity {
    /// Preserve schema capacities and index prefixes exactly, while placing
    /// observed value lengths in bounded relative-error buckets.
    #[default]
    Balanced,
    /// Preserve the original coarse privacy bucketing for all lengths.
    Strict,
    /// Preserve both structural and observed lengths exactly.
    Exact,
}

impl LengthFidelity {
    pub fn label(self) -> &'static str {
        match self {
            Self::Balanced => "balanced",
            Self::Strict => "strict",
            Self::Exact => "exact",
        }
    }

    fn preserves_structure(self) -> bool {
        !matches!(self, Self::Strict)
    }

    fn legacy_marker(self) -> &'static str {
        match self {
            Self::Balanced => "hybrid-v2",
            Self::Strict => "rounded",
            Self::Exact => "exact",
        }
    }

    fn observed_marker(self, measured: bool) -> &'static str {
        if !measured {
            return "not-sampled";
        }
        match self {
            Self::Balanced => "relative-rounded-v2",
            Self::Strict => "coarse-rounded-v1",
            Self::Exact => "exact",
        }
    }
}

/// Distinguishes UTF-8 charsets from non-UTF-8 (latin1, cp1251, big5,
/// sjis, etc.) so the row-frame TypeTag is semantically correct.
/// Tagging non-UTF-8 charset bytes as `TextUtf8` weakens future
/// estimator work that relies on the tag to identify text encoding.
/// List sourced from MySQL's INFORMATION_SCHEMA.COLLATIONS where
/// character_set_name = 'utf8mb3' or
/// 'utf8mb4' or 'utf8'.
fn is_mysql_utf8_charset(id: u16) -> bool {
    matches!(
        id,
        // utf8mb3 (legacy "utf8")
        33 | 76 | 77 | 78 | 79 | 80 | 81 | 82 | 83
        // utf8mb4
        | 45 | 46 | 224 | 225 | 226 | 227 | 228 | 229 | 230 | 231
        | 232 | 233 | 234 | 235 | 236 | 237 | 238 | 239 | 240 | 241
        | 242 | 243 | 244 | 245 | 246 | 247
        | 255  // utf8mb4_0900_ai_ci (MySQL 8 default)
        | 256 | 257 | 258 | 259 | 260 | 261 | 262 | 263 | 264 | 265
        | 266 | 267 | 268 | 269 | 270 | 271 | 272 | 273 | 274 | 275
        | 276 | 277 | 278 | 279 | 280 | 281 | 282 | 283 | 284 | 285
        | 286 | 287 | 288 | 289 | 290 | 291 | 292 | 293 | 294 | 295
        | 296 | 297 | 298 | 299 | 300 | 301 | 302 | 303 | 304 | 305
        | 306 | 307 | 308 | 309
    )
}

/// Classify a MySQL Value + column metadata into a row-frame
/// (TypeTag, payload bytes).
///
/// MySQL's text protocol returns most values as `Value::Bytes` with
/// the textual decimal / ISO-style representation in the column's
/// charset. The Int / UInt / Float / Double / Date / Time variants
/// are reserved for the binary protocol (prepared statements) — we
/// handle them defensively for completeness, but in this code path
/// (`conn.query(...)`, text protocol) they shouldn't occur.
fn encode_mysql_cell(col_type: MyColumnType, charset: u16, value: &Value) -> (TypeTag, Vec<u8>) {
    match value {
        Value::NULL => (TypeTag::Null, Vec::new()),
        Value::Bytes(b) => {
            // Disambiguate text vs binary via column metadata.
            let is_binary_charset = charset == MYSQL_CHARSET_BINARY;
            let tag = match col_type {
                // Integer / numeric / float types → text decimal in
                // text protocol.
                MyColumnType::MYSQL_TYPE_TINY
                | MyColumnType::MYSQL_TYPE_SHORT
                | MyColumnType::MYSQL_TYPE_LONG
                | MyColumnType::MYSQL_TYPE_LONGLONG
                | MyColumnType::MYSQL_TYPE_INT24
                | MyColumnType::MYSQL_TYPE_FLOAT
                | MyColumnType::MYSQL_TYPE_DOUBLE
                | MyColumnType::MYSQL_TYPE_DECIMAL
                | MyColumnType::MYSQL_TYPE_NEWDECIMAL
                | MyColumnType::MYSQL_TYPE_BIT
                | MyColumnType::MYSQL_TYPE_YEAR => TypeTag::NumberText,
                MyColumnType::MYSQL_TYPE_TIMESTAMP
                | MyColumnType::MYSQL_TYPE_DATETIME
                | MyColumnType::MYSQL_TYPE_TIMESTAMP2
                | MyColumnType::MYSQL_TYPE_DATETIME2 => TypeTag::TimestampText,
                MyColumnType::MYSQL_TYPE_DATE | MyColumnType::MYSQL_TYPE_NEWDATE => {
                    TypeTag::DateText
                }
                MyColumnType::MYSQL_TYPE_TIME | MyColumnType::MYSQL_TYPE_TIME2 => TypeTag::TimeText,
                MyColumnType::MYSQL_TYPE_JSON => TypeTag::JsonText,
                // Variable-length text/binary. The `binary` charset
                // marks the column as raw bytes (BINARY, VARBINARY,
                // BLOB family with charset=binary). Otherwise the
                // bytes are text in the column's charset:
                //   - UTF-8 charsets → TextUtf8
                //   - Other charsets (latin1, cp1251, big5, sjis,
                //     etc.) → TextOther
                MyColumnType::MYSQL_TYPE_VARCHAR
                | MyColumnType::MYSQL_TYPE_VAR_STRING
                | MyColumnType::MYSQL_TYPE_STRING
                | MyColumnType::MYSQL_TYPE_TINY_BLOB
                | MyColumnType::MYSQL_TYPE_BLOB
                | MyColumnType::MYSQL_TYPE_MEDIUM_BLOB
                | MyColumnType::MYSQL_TYPE_LONG_BLOB => {
                    if is_binary_charset {
                        TypeTag::BinaryRaw
                    } else if is_mysql_utf8_charset(charset) {
                        TypeTag::TextUtf8
                    } else {
                        TypeTag::TextOther
                    }
                }
                MyColumnType::MYSQL_TYPE_GEOMETRY => TypeTag::BinaryRaw,
                MyColumnType::MYSQL_TYPE_ENUM | MyColumnType::MYSQL_TYPE_SET => TypeTag::TextUtf8,
                _ => TypeTag::UnknownText,
            };
            (tag, b.clone())
        }
        Value::Int(i) => (TypeTag::NumberText, i.to_string().into_bytes()),
        Value::UInt(u) => (TypeTag::NumberText, u.to_string().into_bytes()),
        Value::Float(f) => (TypeTag::NumberText, format!("{f}").into_bytes()),
        Value::Double(d) => (TypeTag::NumberText, format!("{d}").into_bytes()),
        Value::Date(y, mo, d, h, mi, s, _us) => (
            TypeTag::TimestampText,
            format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02}").into_bytes(),
        ),
        Value::Time(neg, days, h, mi, s, _us) => (
            TypeTag::TimeText,
            format!(
                "{}{days:03}d {h:02}:{mi:02}:{s:02}",
                if *neg { "-" } else { "" }
            )
            .into_bytes(),
        ),
    }
}

fn normalized_mysql_type(data_type: &str, bit_width: u64) -> String {
    match data_type.trim().to_ascii_lowercase().as_str() {
        "tinyint" | "smallint" | "mediumint" | "int" | "integer" | "bigint" => {
            "integer".to_string()
        }
        "decimal" | "numeric" => "numeric".to_string(),
        "float" | "real" => "float".to_string(),
        "double" => "double".to_string(),
        "bit" if bit_width > 1 => "binary".to_string(),
        "bit" | "bool" | "boolean" => "boolean".to_string(),
        "year" => "year".to_string(),
        "date" => "date".to_string(),
        "time" => "time".to_string(),
        "datetime" | "timestamp" => "timestamp".to_string(),
        "json" => "json".to_string(),
        "char" | "varchar" | "tinytext" | "text" | "mediumtext" | "longtext" | "enum" | "set" => {
            "text".to_string()
        }
        "binary" | "varbinary" | "tinyblob" | "blob" | "mediumblob" | "longblob" | "geometry"
        | "point" | "linestring" | "polygon" | "multipoint" | "multilinestring"
        | "multipolygon" | "geometrycollection" => "binary".to_string(),
        _ => "user-defined".to_string(),
    }
}

fn mysql_numeric_semantics(data_type: &str, column_type: &str) -> (bool, u64) {
    let data_type = data_type.trim().to_ascii_lowercase();
    let column_type = column_type.trim().to_ascii_lowercase();
    let unsigned = matches!(
        data_type.as_str(),
        "tinyint"
            | "smallint"
            | "mediumint"
            | "int"
            | "integer"
            | "bigint"
            | "decimal"
            | "numeric"
            | "float"
            | "double"
            | "real"
    ) && column_type
        .split_whitespace()
        .any(|part| part == "unsigned");
    let bit_width = if data_type == "bit" {
        column_type
            .strip_prefix("bit(")
            .and_then(|rest| rest.split_once(')'))
            .and_then(|(width, _)| width.parse::<u64>().ok())
            .filter(|width| (1..=64).contains(width))
            .unwrap_or(1)
    } else {
        0
    };
    (unsigned, bit_width)
}

fn normalized_index_method(method: &str) -> String {
    match method.trim().to_ascii_lowercase().as_str() {
        "btree" | "hash" | "fulltext" | "spatial" | "rtree" => method.trim().to_ascii_lowercase(),
        _ => "other".to_string(),
    }
}

#[derive(Debug, Clone)]
pub struct MyConnectParams {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub uri_user_was_explicit: bool,
    pub redacted_uri: String,
}

impl MyConnectParams {
    pub fn parse(uri: &str) -> Result<(Self, Option<String>)> {
        let rest = uri
            .strip_prefix("mysql://")
            .or_else(|| uri.strip_prefix("mariadb://"))
            .ok_or_else(|| anyhow!("URI must start with mysql:// or mariadb://"))?;
        let (authority, dbpart) = crate::uri_authority::split_authority_and_path(rest);
        let (userinfo, hostport) = match authority.rfind('@') {
            Some(i) => (Some(&authority[..i]), &authority[i + 1..]),
            None => (None, authority),
        };
        let (user, password, uri_user_was_explicit) = match userinfo {
            Some(ui) => match ui.find(':') {
                Some(i) => (
                    percent_decode(&ui[..i]),
                    Some(percent_decode(&ui[i + 1..])),
                    true,
                ),
                None => (percent_decode(ui), None, true),
            },
            None => ("root".to_string(), None, false),
        };
        // Bracket-aware host:port split (IPv6-safe).
        let (host, port) = crate::uri_authority::split_host_port(hostport, 3306)?;
        let database = match dbpart.find('?') {
            Some(i) => percent_decode(&dbpart[..i]),
            None => percent_decode(dbpart),
        };
        if database.is_empty() {
            bail!("URI is missing database (mysql://user@host:port/DATABASE)");
        }
        let redacted_uri = format!("mysql://{}@{}:{}/{}", user, host, port, database);
        Ok((
            Self {
                host,
                port,
                database,
                user,
                uri_user_was_explicit,
                redacted_uri,
            },
            password,
        ))
    }
}

#[derive(Debug, Clone)]
pub struct MyRunOpts {
    pub measure_compression: bool,
    pub compression_workers: usize,
    pub length_fidelity: LengthFidelity,
    pub sample_rows: u64,
    pub sample_timeout_secs: u64,
    pub source_kind_str: String,
    pub tls: TlsParams,
    pub generated_at_pin: Option<String>,
    /// Run 5× SELECT 1 after connect to capture customer-side observed
    /// round-trip latency. Default true; `--no-rtt-probe` opts out.
    pub rtt_probe: bool,
    /// An externally generated managed-service token is being supplied through
    /// the password secret channel. Enables `mysql_clear_password` only for
    /// this explicit mode and only inside verified TLS.
    pub cloud_token_auth: bool,
    pub artifact_detail: ArtifactDetail,
    pub schemas: SchemaSelection,
}

fn apply_mysql_auth_mode(builder: OptsBuilder, cloud_token_auth: bool) -> OptsBuilder {
    builder.enable_cleartext_plugin(cloud_token_auth)
}

pub async fn run(
    params: &MyConnectParams,
    secret: &Secret,
    opts: &MyRunOpts,
    audit: &mut AuditLog,
) -> Result<BlueprintFile> {
    crate::tls::validate(&opts.tls, &params.host)?;
    if opts.cloud_token_auth && opts.tls.mode != TlsMode::VerifyFull {
        bail!("DBP1604E MySQL cloud-token authentication requires --tls-mode=verify-full");
    }

    audit.connection.uri_redacted = params.redacted_uri.clone();
    audit.length_fidelity = Some(opts.length_fidelity.label().to_string());
    audit.connection.tls_mode = opts.tls.mode.as_str().to_string();
    audit.connection.tls_ca_path = opts.tls.ca_bundle.clone();
    audit.connection.tls_client_cert = opts.tls.client_cert.clone();
    // Enumerate the PEM files we will read.
    audit.record_tls_file_reads(
        opts.tls.ca_bundle.as_ref(),
        opts.tls.client_cert.as_ref(),
        opts.tls.client_key.as_ref(),
    );

    let mut builder = apply_mysql_auth_mode(OptsBuilder::default(), opts.cloud_token_auth)
        .ip_or_hostname(params.host.clone())
        .tcp_port(params.port)
        .user(Some(params.user.clone()))
        .pass(Some(secret.expose().to_string()))
        .db_name(Some(params.database.clone()));

    // Build SslOpts once for the single connection used by catalog capture
    // and Tier-2 sampling.
    let configured_ssl_opts: Option<SslOpts> = match opts.tls.mode {
        TlsMode::Disable => {
            // mysql_async with no SslOpts uses plain TCP.
            None
        }
        mode => {
            let mut ssl_opts = SslOpts::default();
            if let Some(ca) = &opts.tls.ca_bundle {
                ssl_opts = ssl_opts.with_root_certs(vec![std::fs::read(ca)
                    .with_context(|| format!("reading --tls-ca '{}'", ca.display()))?
                    .into()]);
                audit.connection.tls_ca_only = true;
            }
            if let (Some(cert), Some(key)) = (&opts.tls.client_cert, &opts.tls.client_key) {
                ssl_opts = ssl_opts.with_client_identity(Some(mysql_async::ClientIdentity::new(
                    cert.clone().into(),
                    key.clone().into(),
                )));
            }
            if matches!(mode, TlsMode::Prefer) {
                // mysql_async uses TLS whenever SSL options are configured.
            }
            if opts.tls.skip_verify {
                ssl_opts = ssl_opts.with_danger_accept_invalid_certs(true);
            }
            if !mode.verifies_hostname() {
                ssl_opts = ssl_opts.with_danger_skip_domain_validation(true);
            }
            Some(ssl_opts)
        }
    };
    if let Some(ssl) = configured_ssl_opts.as_ref() {
        builder = builder.ssl_opts(Some(ssl.clone()));
    }
    audit.connection.auth = if opts.cloud_token_auth {
        "cloud-token/mysql_clear_password".to_string()
    } else {
        "mysql_native_password / caching_sha2_password".to_string()
    };
    audit.network_egress.push(format!(
        "{}:{} (database-driver session; DNS may use the configured resolver)",
        params.host, params.port
    ));

    let pool = Pool::new(builder);

    // engine_version + tables/columns/indexes/fks
    let connect_started = Instant::now();
    let mut conn = pool
        .get_conn()
        .await
        .with_context(|| format!("connecting to {}", params.redacted_uri))?;
    audit.connection.ssl_negotiated = if opts.tls.mode == TlsMode::Disable {
        "no (plaintext transport)".to_string()
    } else {
        "yes (protocol version unavailable from driver)".to_string()
    };
    let connect_total = connect_started.elapsed();

    // MySQL applies max_execution_time to read-only SELECT statements, which
    // covers every catalog and sampling query issued by this collector. Keep
    // the independent client wall deadline as a connection-level backstop.
    let timeout_ms = opts
        .sample_timeout_secs
        .saturating_mul(1000)
        .min(u32::MAX as u64);
    let timeout_started = Instant::now();
    conn.query_drop(format!("SET SESSION max_execution_time = {timeout_ms}"))
        .await
        .context("setting MySQL session max_execution_time")?;
    audit.record_query(
        "SET SESSION max_execution_time = <max-wall-secs> (read-only SELECT safety limit)",
        elapsed_ms(timeout_started),
        0,
    );

    // RTT probe — 5× SELECT 1 for customer-side observed round-trip
    // statistics. Captured BEFORE catalog queries so timings aren't
    // skewed by cache warmup.
    let network_probe = if opts.rtt_probe {
        match probe_rtt(&mut conn, audit).await {
            Ok((p50, p95)) => Some(format::NetworkProbe {
                sample_count: 5,
                connect_total_ms: format::round_ms(connect_total),
                query_rtt_ms_p50: p50,
                query_rtt_ms_p95: p95,
            }),
            Err(_) => {
                let redacted = crate::i18n::format(
                    "engine.driver_detail_redacted",
                    &[("target", "RTT probe".to_string())],
                );
                let detail = crate::i18n::format(
                    "engine.rtt_failed",
                    &[("code", "DBP1405W".to_string()), ("error", redacted)],
                );
                tracing_eprintln(detail.clone());
                audit.record_warning("DBP1405W", detail);
                None
            }
        }
    } else {
        None
    };

    let started = Instant::now();
    let raw_version: String = conn
        .query_first("SELECT VERSION()")
        .await
        .context("querying VERSION()")?
        .unwrap_or_default();
    audit.record_query("SELECT VERSION()", elapsed_ms(started), 1);
    let engine_version = normalized_mysql_version(&raw_version);
    let schemas = resolve_mysql_schemas(&mut conn, &opts.schemas, audit).await?;
    let topology_evidence = probe_mysql_topology(&mut conn, &raw_version, audit).await;

    // Tables.
    let started = Instant::now();
    let mut tables_in: Vec<TableRow> = conn
        .query_map(
            r#"
            SELECT t.TABLE_SCHEMA  AS schema_name,
                   t.TABLE_NAME    AS table_name,
                   COALESCE(t.TABLE_ROWS, 0)  AS rows_estimate,
                   COALESCE(t.DATA_LENGTH, 0) AS data_length,
                   COALESCE(t.INDEX_LENGTH, 0) AS index_length,
                   t.UPDATE_TIME   AS update_time,
                   COALESCE(t.ENGINE, '') AS storage_engine
            FROM information_schema.TABLES t
            WHERE t.TABLE_TYPE = 'BASE TABLE'
              AND t.TABLE_SCHEMA NOT IN ('mysql','information_schema','performance_schema','sys')
            ORDER BY t.TABLE_SCHEMA, t.TABLE_NAME
            "#,
            |(
                schema_name,
                table_name,
                rows_estimate,
                data_length,
                index_length,
                update_time,
                storage_engine,
            ): (
                String,
                String,
                u64,
                u64,
                u64,
                Option<chrono::NaiveDateTime>,
                String,
            )| TableRow {
                schema_name,
                table_name,
                rows_estimate,
                data_length,
                index_length,
                update_time,
                storage_engine,
            },
        )
        .await
        .context("listing tables from information_schema")?;
    tables_in.retain(|table| schemas.includes(&table.schema_name));
    audit.record_query(
        "SELECT TABLE_SCHEMA,TABLE_NAME,TABLE_ROWS,DATA_LENGTH,INDEX_LENGTH,ENGINE FROM information_schema.TABLES",
        elapsed_ms(started),
        tables_in.len() as u64,
    );
    let mut sizing = classify_mysql_topology(&topology_evidence, &tables_in);
    sizing.qualify_table_statistics(&mut tables_in, audit);
    schemas.qualify_dataset_scope(&mut sizing.dataset_scope);
    warn_incomplete_dataset_scope(&sizing.dataset_scope, audit);

    // Columns.
    let started = Instant::now();
    let cols_in: Vec<ColumnRow> = conn
        .query_map(
            r#"
            SELECT c.TABLE_SCHEMA AS schema_name,
                   c.TABLE_NAME   AS table_name,
                   c.ORDINAL_POSITION AS ordinal,
                   c.COLUMN_NAME AS col_name,
                   CONCAT(c.DATA_TYPE, CHAR(31), c.COLUMN_TYPE) AS type_metadata,
                   c.IS_NULLABLE  AS is_nullable,
                   COALESCE(c.CHARACTER_MAXIMUM_LENGTH, 0) AS char_max_length,
                   COALESCE(c.CHARACTER_OCTET_LENGTH, 0) AS char_octet_length,
                   COALESCE(c.NUMERIC_PRECISION, 0) AS numeric_precision,
                   COALESCE(c.NUMERIC_SCALE, 0) AS numeric_scale,
                   COALESCE(c.DATETIME_PRECISION, 0) AS datetime_precision,
                   CONCAT(COALESCE(c.CHARACTER_SET_NAME, ''), CHAR(31),
                          COALESCE(c.COLLATION_NAME, '')) AS character_metadata
            FROM information_schema.COLUMNS c
            WHERE c.TABLE_SCHEMA NOT IN ('mysql','information_schema','performance_schema','sys')
            ORDER BY c.TABLE_SCHEMA, c.TABLE_NAME, c.ORDINAL_POSITION
            "#,
            |(
                schema_name,
                table_name,
                ordinal,
                col_name,
                type_metadata,
                is_nullable,
                char_max_length,
                char_octet_length,
                numeric_precision,
                numeric_scale,
                datetime_precision,
                character_metadata,
            ): (
                String,
                String,
                u32,
                String,
                String,
                String,
                u64,
                u64,
                u64,
                u64,
                u64,
                String,
            )| {
                let (data_type, column_type) = type_metadata
                    .split_once('\u{1f}')
                    .unwrap_or((type_metadata.as_str(), type_metadata.as_str()));
                let (character_set_name, collation_name) = character_metadata
                    .split_once('\u{1f}')
                    .unwrap_or((character_metadata.as_str(), ""));
                let (numeric_unsigned, bit_width) =
                    mysql_numeric_semantics(&data_type, &column_type);
                ColumnRow {
                    schema_name,
                    table_name,
                    ordinal,
                    col_name,
                    col_type: normalized_mysql_type(data_type, bit_width),
                    native_type: data_type.to_ascii_lowercase(),
                    numeric_unsigned,
                    bit_width,
                    is_nullable: is_nullable == "YES",
                    char_max_length,
                    char_octet_length,
                    numeric_precision,
                    numeric_scale,
                    datetime_precision,
                    character_set_name: character_set_name.to_string(),
                    collation_name: collation_name.to_string(),
                }
            },
        )
        .await
        .context("listing columns from information_schema")?;
    audit.record_query(
        "SELECT ... FROM information_schema.COLUMNS",
        elapsed_ms(started),
        cols_in.len() as u64,
    );

    // Indexes (deduplicated by INDEX_NAME, ordered by SEQ_IN_INDEX).
    let started = Instant::now();
    let supports_index_expression = conn
        .query_first::<u64, _>(
            r#"
            SELECT COUNT(*)
            FROM information_schema.COLUMNS
            WHERE TABLE_SCHEMA = 'information_schema'
              AND TABLE_NAME = 'STATISTICS'
              AND COLUMN_NAME = 'EXPRESSION'
            "#,
        )
        .await
        .context("detecting MySQL functional-index metadata support")?
        .unwrap_or(0)
        > 0;
    let index_sql = if supports_index_expression {
        r#"
            SELECT s.TABLE_SCHEMA  AS schema_name,
                   s.TABLE_NAME    AS table_name,
                   s.INDEX_NAME    AS index_name,
                   s.NON_UNIQUE    AS non_unique,
                   COALESCE(s.INDEX_TYPE, 'BTREE') AS index_type,
                   s.SEQ_IN_INDEX  AS seq,
                   s.COLUMN_NAME   AS col_name,
                   COALESCE(s.SUB_PART, 0) AS sub_part,
                   COALESCE(s.COLLATION, 'A') AS collation,
                   CASE WHEN s.EXPRESSION IS NULL THEN 0 ELSE 1 END AS is_expression
            FROM information_schema.STATISTICS s
            WHERE s.TABLE_SCHEMA NOT IN ('mysql','information_schema','performance_schema','sys')
            ORDER BY s.TABLE_SCHEMA, s.TABLE_NAME, s.INDEX_NAME, s.SEQ_IN_INDEX
        "#
    } else {
        r#"
            SELECT s.TABLE_SCHEMA  AS schema_name,
                   s.TABLE_NAME    AS table_name,
                   s.INDEX_NAME    AS index_name,
                   s.NON_UNIQUE    AS non_unique,
                   COALESCE(s.INDEX_TYPE, 'BTREE') AS index_type,
                   s.SEQ_IN_INDEX  AS seq,
                   s.COLUMN_NAME   AS col_name,
                   COALESCE(s.SUB_PART, 0) AS sub_part,
                   COALESCE(s.COLLATION, 'A') AS collation,
                   0 AS is_expression
            FROM information_schema.STATISTICS s
            WHERE s.TABLE_SCHEMA NOT IN ('mysql','information_schema','performance_schema','sys')
            ORDER BY s.TABLE_SCHEMA, s.TABLE_NAME, s.INDEX_NAME, s.SEQ_IN_INDEX
        "#
    };
    let idx_in: Vec<IndexRow> = conn
        .query_map(
            index_sql,
            |(
                schema_name,
                table_name,
                index_name,
                non_unique,
                index_type,
                seq,
                col_name,
                sub_part,
                collation,
                is_expression,
            ): (
                String,
                String,
                String,
                u32,
                String,
                u32,
                Option<String>,
                u64,
                Option<String>,
                u8,
            )| IndexRow {
                schema_name,
                table_name,
                index_name,
                non_unique: non_unique != 0,
                index_type,
                seq,
                col_name: col_name.unwrap_or_default(),
                prefix_length: sub_part,
                descending: collation
                    .as_deref()
                    .map(|value| value.eq_ignore_ascii_case("D"))
                    .unwrap_or(false),
                expression: is_expression != 0,
            },
        )
        .await
        .context("listing indexes from information_schema")?;
    audit.record_query(
        "SELECT ... FROM information_schema.STATISTICS",
        elapsed_ms(started),
        idx_in.len() as u64,
    );

    // FKs.
    let started = Instant::now();
    let fks_in: Vec<FkRow> = conn
        .query_map(
            r#"
            SELECT k.TABLE_SCHEMA           AS from_schema,
                   k.TABLE_NAME             AS from_table,
                   k.REFERENCED_TABLE_SCHEMA AS to_schema,
                   k.REFERENCED_TABLE_NAME   AS to_table,
                   k.CONSTRAINT_NAME         AS constraint_name,
                   k.ORDINAL_POSITION        AS position,
                   k.COLUMN_NAME             AS col_name,
                   k.REFERENCED_COLUMN_NAME  AS to_col_name,
                   rc.UPDATE_RULE            AS update_rule,
                   rc.DELETE_RULE            AS delete_rule,
                   rc.MATCH_OPTION           AS match_option
            FROM information_schema.KEY_COLUMN_USAGE k
            JOIN information_schema.REFERENTIAL_CONSTRAINTS rc
              ON rc.CONSTRAINT_SCHEMA = k.CONSTRAINT_SCHEMA
             AND rc.CONSTRAINT_NAME = k.CONSTRAINT_NAME
             AND rc.TABLE_NAME = k.TABLE_NAME
            WHERE k.REFERENCED_TABLE_NAME IS NOT NULL
              AND k.TABLE_SCHEMA NOT IN ('mysql','information_schema','performance_schema','sys')
            ORDER BY k.TABLE_SCHEMA, k.TABLE_NAME, k.CONSTRAINT_NAME, k.ORDINAL_POSITION
            "#,
            |(
                from_schema,
                from_table,
                to_schema,
                to_table,
                constraint_name,
                position,
                col_name,
                to_col_name,
                update_rule,
                delete_rule,
                match_option,
            ): (
                String,
                String,
                Option<String>,
                Option<String>,
                String,
                u32,
                String,
                Option<String>,
                String,
                String,
                String,
            )| FkRow {
                from_schema,
                from_table,
                to_schema: to_schema.unwrap_or_default(),
                to_table: to_table.unwrap_or_default(),
                constraint_name,
                position,
                col_name,
                to_col_name: to_col_name.unwrap_or_default(),
                on_update: normalize_fk_rule(update_rule.as_str()),
                on_delete: normalize_fk_rule(delete_rule.as_str()),
                match_type: normalize_mysql_fk_match(match_option.as_str()),
            },
        )
        .await
        .context("listing FKs from information_schema")?;
    audit.record_query(
        "SELECT ... FROM information_schema.KEY_COLUMN_USAGE (FK list)",
        elapsed_ms(started),
        fks_in.len() as u64,
    );

    let artifact_capture = if opts.artifact_detail == ArtifactDetail::None {
        None
    } else {
        let (mut raw, completeness) = capture_artifacts(
            &mut conn,
            opts.artifact_detail,
            &raw_version,
            &schemas,
            audit,
        )
        .await;
        raw.retain(|item| {
            item.schema_identity
                .as_deref()
                .is_none_or(|schema| schemas.includes(schema))
        });
        Some((raw, completeness))
    };

    // ----- Anonymize + build -----
    let mut tables_sorted = tables_in.clone();
    tables_sorted.sort_by_key(|t| format::table_hash(&t.schema_name, &t.table_name));
    let mut id_by_qual: BTreeMap<(String, String), String> = BTreeMap::new();
    for (i, t) in tables_sorted.iter().enumerate() {
        id_by_qual.insert(
            (t.schema_name.clone(), t.table_name.clone()),
            format::table_id(i + 1),
        );
    }
    let mut schema_seen: Vec<String> = tables_sorted
        .iter()
        .map(|t| t.schema_name.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    schema_seen.sort_by_key(|s| format::schema_hash(s));
    let mut schema_id_by_name: BTreeMap<String, String> = BTreeMap::new();
    for (i, name) in schema_seen.iter().enumerate() {
        schema_id_by_name.insert(name.clone(), format::schema_id(i + 1));
    }

    // Group columns by qualified name.
    let mut cols_by_qual: BTreeMap<(String, String), Vec<ColumnRow>> = BTreeMap::new();
    for c in cols_in {
        cols_by_qual
            .entry((c.schema_name.clone(), c.table_name.clone()))
            .or_default()
            .push(c);
    }
    for v in cols_by_qual.values_mut() {
        v.sort_by_key(|c| c.ordinal);
    }

    // Group indexes by (schema, table, index_name) → list of (seq, col_name).
    let mut idx_groups: BTreeMap<(String, String, String), Vec<IndexPart>> = BTreeMap::new();
    for r in idx_in {
        let primary = r.index_name.eq_ignore_ascii_case("PRIMARY");
        idx_groups
            .entry((r.schema_name, r.table_name, r.index_name))
            .or_default()
            .push((
                r.seq,
                r.col_name,
                !r.non_unique,
                r.index_type,
                primary,
                r.descending,
                r.prefix_length,
                r.expression,
            ));
    }

    // Tier 2 sampling.
    let mut style_by_qual_ordinal: BTreeMap<(String, String, u32), &'static str> = BTreeMap::new();
    let mut compression_by_qual: BTreeMap<(String, String), CompressionSample> = BTreeMap::new();
    if opts.measure_compression {
        match CompressionWorkerPool::new(opts.compression_workers) {
            Ok(compression_pool) => {
                audit.configure_compression_workers(
                    compression_pool.worker_count(),
                    compression_pool.queue_capacity(),
                );
                let deadline = Instant::now()
                    + std::time::Duration::from_secs(opts.sample_timeout_secs.max(1));
                let mut pending_samples = Vec::new();
                let mut pipeline_started = None;
                for t in &tables_sorted {
                    if Instant::now() >= deadline {
                        let detail = crate::i18n::format(
                            "engine.sample_budget",
                            &[("code", "DBP1406W".to_string())],
                        );
                        tracing_eprintln(detail.clone());
                        audit.record_warning("DBP1406W", detail);
                        break;
                    }
                    let qual = (t.schema_name.clone(), t.table_name.clone());
                    let table_id = id_by_qual
                        .get(&qual)
                        .map(String::as_str)
                        .unwrap_or("table-unknown");
                    let cols_for_table: &[ColumnRow] =
                        cols_by_qual.get(&qual).map(Vec::as_slice).unwrap_or(&[]);
                    match sample_compression(
                        &mut conn,
                        t,
                        cols_for_table,
                        opts.sample_rows,
                        opts.length_fidelity,
                        &compression_pool,
                        audit,
                    )
                    .await
                    {
                        Ok(Some(pending)) => {
                            if pipeline_started.is_none() {
                                pipeline_started = Some(pending.submitted_at);
                            }
                            pending_samples.push((qual.clone(), table_id.to_string(), pending));
                        }
                        Ok(None) => { /* table empty */ }
                        Err(_) => warn_compression_unavailable(table_id, audit),
                    }

                    if let Some(cols) = cols_by_qual.get(&qual) {
                        for (column_index, c) in cols.iter().enumerate() {
                            if is_style_candidate_mysql(c) {
                                if c.col_type == "json" {
                                    style_by_qual_ordinal.insert(
                                        (t.schema_name.clone(), t.table_name.clone(), c.ordinal),
                                        "json",
                                    );
                                } else {
                                    match peek_column_style(&mut conn, t, c).await {
                                        Ok(label) if !label.is_empty() => {
                                            style_by_qual_ordinal.insert(
                                                (
                                                    t.schema_name.clone(),
                                                    t.table_name.clone(),
                                                    c.ordinal,
                                                ),
                                                label,
                                            );
                                        }
                                        Ok(_) => {}
                                        Err(_) => {
                                            let redacted = crate::i18n::format(
                                                "engine.driver_detail_redacted",
                                                &[(
                                                    "target",
                                                    format!(
                                                        "{table_id}/{}",
                                                        format::col_id((column_index + 1) as u32)
                                                    ),
                                                )],
                                            );
                                            let detail = crate::i18n::format(
                                                "engine.style_failed",
                                                &[
                                                    ("code", "DBP1408W".to_string()),
                                                    ("error", redacted),
                                                ],
                                            );
                                            tracing_eprintln(detail.clone());
                                            audit.record_warning("DBP1408W", detail);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                for (qual, table_id, pending) in pending_samples {
                    match pending.ticket.resolve() {
                        Ok(measurements) => {
                            audit.record_compression_job_completed(&measurements.work);
                            compression_by_qual.insert(
                                qual,
                                CompressionSample {
                                    table: measurements.table,
                                    columns: measurements.columns,
                                    column_lengths: pending.column_lengths,
                                    null_fractions: pending.null_fractions,
                                    cardinalities: pending.cardinalities,
                                },
                            );
                        }
                        Err(_) => warn_compression_unavailable(&table_id, audit),
                    }
                }
                if let Some(started) = pipeline_started {
                    audit.record_compression_pipeline_wall(elapsed_ms(started));
                }
            }
            Err(_) => warn_compression_unavailable("worker-pool", audit),
        }
    }

    // Build BlueprintFile.
    let mut tables_out: BTreeMap<String, BlueprintTable> = BTreeMap::new();
    let mut totals = Totals::default();
    for t in &tables_sorted {
        let qual = (t.schema_name.clone(), t.table_name.clone());
        let tid = id_by_qual.get(&qual).cloned().unwrap_or_default();
        let schema_anon = schema_id_by_name
            .get(&t.schema_name)
            .cloned()
            .unwrap_or_else(|| "schema-?".to_string());
        let compression_sample = compression_by_qual.remove(&qual);
        let mut col_map: BTreeMap<String, BlueprintColumn> = BTreeMap::new();
        if let Some(cs) = cols_by_qual.get(&qual) {
            for (col_pos, c) in cs.iter().enumerate() {
                let column_compression = compression_sample
                    .as_ref()
                    .and_then(|sample| sample.columns.get(col_pos))
                    .cloned()
                    .flatten();
                let column_lengths = compression_sample
                    .as_ref()
                    .and_then(|sample| sample.column_lengths.get(col_pos))
                    .copied()
                    .flatten();
                let null_fraction = compression_sample
                    .as_ref()
                    .and_then(|sample| sample.null_fractions.get(col_pos))
                    .copied()
                    .flatten();
                let column_cardinality = compression_sample
                    .as_ref()
                    .and_then(|sample| sample.cardinalities.get(col_pos))
                    .cloned()
                    .flatten();
                let variable_length = matches!(
                    c.col_type.as_str(),
                    "text" | "json" | "binary" | "array" | "user-defined"
                );
                let (len_avg, len_p95) = if variable_length {
                    column_lengths.unwrap_or((0, 0))
                } else {
                    (0, 0)
                };
                col_map.insert(
                    format::col_id(c.ordinal),
                    BlueprintColumn {
                        ordinal: c.ordinal,
                        column_type: c.col_type.clone(),
                        nullable: c.is_nullable,
                        null_fraction,
                        native_type: c.native_type.clone(),
                        declared_max_chars: blueprint_length(
                            c.char_max_length,
                            opts.length_fidelity,
                        ),
                        declared_max_bytes: blueprint_length(
                            c.char_octet_length,
                            opts.length_fidelity,
                        ),
                        numeric_precision: c.numeric_precision,
                        numeric_scale: c.numeric_scale,
                        numeric_unsigned: c.numeric_unsigned,
                        bit_width: c.bit_width,
                        datetime_precision: c.datetime_precision,
                        charset: c.character_set_name.clone(),
                        collation: c.collation_name.clone(),
                        len_avg,
                        len_p95,
                        style: style_by_qual_ordinal
                            .get(&(t.schema_name.clone(), t.table_name.clone(), c.ordinal))
                            .copied()
                            .unwrap_or("")
                            .to_string(),
                        compression: column_compression,
                        cardinality: column_cardinality,
                        ..BlueprintColumn::default()
                    },
                );
            }
        }
        // Anonymize indexes for this table.
        let col_to_ord: BTreeMap<String, u32> = cols_by_qual
            .get(&qual)
            .map(|cs| cs.iter().map(|c| (c.col_name_lower(), c.ordinal)).collect())
            .unwrap_or_default();
        let mut idxs_for_table: Vec<((String, String, String), Vec<IndexPart>)> = idx_groups
            .iter()
            .filter(|((s, tn, _), _)| s == &t.schema_name && tn == &t.table_name)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        idxs_for_table.sort_by_key(|((_, _, idx_name), _)| format::index_hash(idx_name));
        let mut idx_map: BTreeMap<String, BlueprintIndex> = BTreeMap::new();
        for (i, ((_, _, _), parts)) in idxs_for_table.iter().enumerate() {
            idx_map.insert(
                format::idx_id((i + 1) as u32),
                index_blueprint_from_parts(parts, &col_to_ord, opts.length_fidelity),
            );
        }

        // Stats freshness from update_time (UPDATE_TIME) — heuristic: > 7d = stale.
        let stats_freshness = match t.update_time {
            None => "never_analyzed".to_string(),
            Some(t_naive) => {
                let now = chrono::Utc::now().naive_utc();
                let age = now.signed_duration_since(t_naive);
                if age.num_days() <= 7 {
                    "fresh".to_string()
                } else {
                    "stale".to_string()
                }
            }
        };

        let table_blueprint = BlueprintTable {
            rows: format::round_rows(t.rows_estimate),
            table_bytes: format::round_bytes(t.data_length),
            index_bytes: format::round_bytes(t.index_length),
            schema: schema_anon,
            has_clustered_index: false,
            stats_freshness,
            cols: col_map,
            idxs: idx_map,
            compression: compression_sample.map(|sample| sample.table),
            ..BlueprintTable::default()
        };
        accumulate_table_totals(&mut totals, &table_blueprint)?;
        tables_out.insert(tid, table_blueprint);
    }
    totals.table_count = tables_out.len() as u64;

    // FKs.
    let mut fk_edges: BTreeMap<String, Vec<FkEdge>> = BTreeMap::new();
    // Constraint identity is retained only while collecting. The emitted Blueprint
    // remains anonymized but distinct FKs between the same tables stay distinct.
    let mut fk_groups: BTreeMap<(String, String, String, String, String), Vec<FkRow>> =
        BTreeMap::new();
    for r in fks_in {
        let key = (
            r.from_schema.clone(),
            r.from_table.clone(),
            r.to_schema.clone(),
            r.to_table.clone(),
            r.constraint_name.clone(),
        );
        fk_groups.entry(key).or_default().push(r);
    }
    for ((fs, ft, ts, tt, _constraint_name), mut cols) in fk_groups {
        cols.sort_by_key(|row| row.position);
        let from_id = match id_by_qual.get(&(fs.clone(), ft.clone())) {
            Some(s) => s.clone(),
            None => continue,
        };
        let to_id = match id_by_qual.get(&(ts.clone(), tt.clone())) {
            Some(s) => s.clone(),
            None => continue,
        };
        let col_to_ord: BTreeMap<String, u32> = cols_by_qual
            .get(&(fs.clone(), ft.clone()))
            .map(|cs| cs.iter().map(|c| (c.col_name_lower(), c.ordinal)).collect())
            .unwrap_or_default();
        let to_col_to_ord: BTreeMap<String, u32> = cols_by_qual
            .get(&(ts, tt))
            .map(|cs| cs.iter().map(|c| (c.col_name_lower(), c.ordinal)).collect())
            .unwrap_or_default();
        let col_ords: Vec<u32> = cols
            .iter()
            .filter_map(|row| col_to_ord.get(&row.col_name.to_ascii_lowercase()).copied())
            .collect();
        let to_col_ords: Vec<u32> = cols
            .iter()
            .filter_map(|row| {
                to_col_to_ord
                    .get(&row.to_col_name.to_ascii_lowercase())
                    .copied()
            })
            .collect();
        if col_ords.len() != cols.len() || to_col_ords.len() != cols.len() {
            continue;
        }
        fk_edges.entry(from_id).or_default().push(FkEdge {
            to: to_id,
            cols: col_ords,
            to_cols: to_col_ords,
            on_update: cols[0].on_update.clone(),
            on_delete: cols[0].on_delete.clone(),
            match_type: cols[0].match_type.clone(),
            deferrable: false,
            initially_deferred: false,
            validated: true,
            statistics: None,
        });
    }
    for v in fk_edges.values_mut() {
        v.sort_by(|a, b| {
            a.to.cmp(&b.to)
                .then(a.cols.cmp(&b.cols))
                .then(a.to_cols.cmp(&b.to_cols))
        });
    }

    let table_artifact_ids: BTreeMap<String, String> = id_by_qual
        .iter()
        .map(|((schema, table), id)| {
            (
                artifacts::table_identity("mysql", schema, table),
                id.clone(),
            )
        })
        .collect();
    let artifact_inventory = artifact_capture.map(|(raw_artifacts, completeness)| {
        artifacts::build_inventory(
            opts.artifact_detail,
            raw_artifacts,
            &schema_id_by_name,
            &table_artifact_ids,
            completeness,
        )
    });

    let mut blueprint = BlueprintFile {
        schema_version: SCHEMA_VERSION,
        // Pinning is via `--generated-at` CLI flag, never an env var.
        generated_at: crate::format::generated_at_now(opts.generated_at_pin.as_deref()),
        engine: "mysql".to_string(),
        engine_version,
        source_kind: opts.source_kind_str.clone(),
        length_metadata: opts.length_fidelity.legacy_marker().to_string(),
        declared_length_fidelity: if opts.length_fidelity.preserves_structure() {
            "exact"
        } else {
            "coarse-rounded-v1"
        }
        .to_string(),
        index_length_fidelity: if opts.length_fidelity.preserves_structure() {
            "exact"
        } else {
            "rounded-down-v1"
        }
        .to_string(),
        observed_length_fidelity: opts
            .length_fidelity
            .observed_marker(opts.measure_compression)
            .to_string(),
        totals,
        network: network_probe,
        database_topology: Some(sizing.topology),
        dataset_scope: Some(sizing.dataset_scope),
        tables: tables_out,
        fk_edges,
        artifact_inventory,
    };
    crate::statistics::enrich_relational_statistics(&mut blueprint);
    drop(conn);
    pool.disconnect().await.ok();
    Ok(blueprint)
}

#[derive(Debug, Clone)]
struct TableRow {
    schema_name: String,
    table_name: String,
    rows_estimate: u64,
    data_length: u64,
    index_length: u64,
    update_time: Option<chrono::NaiveDateTime>,
    storage_engine: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct MysqlTopologyEvidence {
    server_identity_readable: bool,
    vitess_gateway: bool,
    capability_catalog_readable: bool,
    group_catalog_present: bool,
    group_catalog_readable: bool,
    group_member_count: u64,
    group_primary_count: u64,
    group_secondary_count: u64,
    local_group_role: Option<&'static str>,
    replica_catalog_present: bool,
    replica_catalog_readable: bool,
    replica_channel_count: u64,
    wsrep_catalog_attempted: bool,
    wsrep_catalog_readable: bool,
    galera_active: bool,
    galera_member_count: u64,
}

#[derive(Debug, Clone)]
struct MysqlSizingAssessment {
    topology: format::DatabaseTopology,
    dataset_scope: format::DatasetScope,
    suppress_table_statistics: bool,
    distributed_size_unavailable: bool,
}

impl MysqlSizingAssessment {
    fn qualify_table_statistics(&mut self, tables: &mut [TableRow], audit: &mut AuditLog) {
        let now = chrono::Utc::now().naive_utc();
        if tables.iter().any(|table| {
            table
                .update_time
                .map(|updated| now.signed_duration_since(updated).num_days() > 7)
                .unwrap_or(true)
        }) {
            self.dataset_scope
                .limitations
                .push("statistics-stale".to_string());
        }
        if self.suppress_table_statistics {
            for table in tables {
                table.rows_estimate = 0;
                table.data_length = 0;
                table.index_length = 0;
            }
        }
        if self.distributed_size_unavailable {
            crate::topology::warn_distributed_size_unavailable(audit);
        }
        sort_dedup(&mut self.dataset_scope.limitations);
    }
}

fn classify_mysql_topology(
    evidence: &MysqlTopologyEvidence,
    tables: &[TableRow],
) -> MysqlSizingAssessment {
    let mut catalogs_read = Vec::new();
    let mut catalogs_unreadable = Vec::new();
    if evidence.server_identity_readable {
        catalogs_read.push("mysql-server-identity".to_string());
    } else {
        catalogs_unreadable.push("mysql-server-identity".to_string());
    }
    if evidence.capability_catalog_readable {
        catalogs_read.push("mysql-topology-capabilities".to_string());
    } else if !evidence.vitess_gateway {
        catalogs_unreadable.push("mysql-topology-capabilities".to_string());
    }
    if evidence.group_catalog_present {
        if evidence.group_catalog_readable {
            catalogs_read.push("mysql-group-members".to_string());
        } else {
            catalogs_unreadable.push("mysql-group-members".to_string());
        }
    }
    if evidence.replica_catalog_present {
        if evidence.replica_catalog_readable {
            catalogs_read.push("mysql-replica-status".to_string());
        } else if !(evidence.group_catalog_readable && evidence.group_member_count > 0) {
            catalogs_unreadable.push("mysql-replica-status".to_string());
        }
    }
    if evidence.wsrep_catalog_attempted {
        if evidence.wsrep_catalog_readable {
            catalogs_read.push("mysql-wsrep-status".to_string());
        } else {
            catalogs_unreadable.push("mysql-wsrep-status".to_string());
        }
    }

    let mut topology = format::DatabaseTopology {
        contract: dbwarp_blueprint_core::TOPOLOGY_CONTRACT.to_string(),
        deployment: "unknown".to_string(),
        local_role: "unknown".to_string(),
        visibility: "partial".to_string(),
        member_count: 1,
        identifiers_redacted: true,
        role_counts: BTreeMap::from([("unknown".to_string(), 1)]),
        features: Vec::new(),
        catalogs_read,
        catalogs_unreadable,
    };

    if evidence.group_catalog_readable && evidence.group_member_count > 0 {
        topology.deployment = "replicated".to_string();
        topology.local_role = evidence.local_group_role.unwrap_or("unknown").to_string();
        topology.member_count = evidence.group_member_count;
        topology.role_counts.clear();
        if evidence.group_primary_count > 0 {
            topology
                .role_counts
                .insert("primary".to_string(), evidence.group_primary_count);
        }
        if evidence.group_secondary_count > 0 {
            topology
                .role_counts
                .insert("secondary".to_string(), evidence.group_secondary_count);
        }
        let classified = evidence
            .group_primary_count
            .saturating_add(evidence.group_secondary_count);
        if classified < evidence.group_member_count {
            topology.role_counts.insert(
                "unknown".to_string(),
                evidence.group_member_count - classified,
            );
        }
        topology
            .features
            .push("mysql-group-replication".to_string());
        if topology.catalogs_unreadable.is_empty() {
            topology.visibility = "full".to_string();
        }
    } else if evidence.galera_active && evidence.galera_member_count > 0 {
        topology.deployment = "replicated".to_string();
        topology.local_role = "member".to_string();
        topology.member_count = evidence.galera_member_count;
        topology.role_counts =
            BTreeMap::from([("member".to_string(), evidence.galera_member_count)]);
        topology.features.push("mysql-galera".to_string());
        if topology.catalogs_unreadable.is_empty() {
            topology.visibility = "full".to_string();
        }
    } else if evidence.replica_catalog_readable && evidence.replica_channel_count > 0 {
        topology.deployment = "replicated".to_string();
        topology.local_role = "secondary".to_string();
        topology.member_count = 1_u64.saturating_add(evidence.replica_channel_count);
        topology.role_counts = BTreeMap::from([
            ("primary".to_string(), evidence.replica_channel_count),
            ("secondary".to_string(), 1),
        ]);
        topology
            .features
            .push("mysql-asynchronous-replication".to_string());
    }

    let has_ndb = tables.iter().any(|table| {
        matches!(
            table.storage_engine.to_ascii_uppercase().as_str(),
            "NDB" | "NDBCLUSTER"
        )
    });
    let distributed = evidence.vitess_gateway || has_ndb;
    if evidence.vitess_gateway {
        topology.deployment = "sharded".to_string();
        topology.local_role = "coordinator".to_string();
        topology.visibility = "unknown".to_string();
        topology.member_count = 0;
        topology.role_counts.clear();
        topology.features.push("vitess".to_string());
        topology
            .catalogs_read
            .push("mysql-vitess-identity".to_string());
    } else if has_ndb {
        topology.deployment = "distributed".to_string();
        topology.local_role = "coordinator".to_string();
        topology.visibility = "partial".to_string();
        topology.features.push("mysql-ndb".to_string());
        topology
            .catalogs_read
            .push("mysql-storage-engines".to_string());
    }
    sort_topology(&mut topology);

    let dataset_scope = if distributed {
        let layout = if evidence.vitess_gateway {
            "sharded"
        } else {
            "distributed"
        };
        format::DatasetScope {
            contract: dbwarp_blueprint_core::DATASET_SCOPE_CONTRACT.to_string(),
            layout: layout.to_string(),
            table_inventory_completeness: if evidence.vitess_gateway {
                "unknown"
            } else {
                "complete"
            }
            .to_string(),
            row_count_completeness: "incomplete".to_string(),
            size_completeness: "incomplete".to_string(),
            row_count_method: "unknown".to_string(),
            size_method: "unknown".to_string(),
            limitations: {
                let mut limitations = vec![
                    "distributed-aggregate-unavailable".to_string(),
                    "distributed-row-count-unavailable".to_string(),
                    "distributed-size-unavailable".to_string(),
                    "shard-membership-incomplete".to_string(),
                    if evidence.vitess_gateway {
                        "topology-visibility-unknown"
                    } else {
                        "topology-visibility-partial"
                    }
                    .to_string(),
                ];
                sort_dedup(&mut limitations);
                limitations
            },
        }
    } else {
        let mut limitations = vec!["row-counts-statistical".to_string()];
        if topology.visibility == "partial" {
            limitations.push("topology-visibility-partial".to_string());
        } else if topology.visibility == "unknown" {
            limitations.push("topology-visibility-unknown".to_string());
        }
        if topology.deployment == "replicated" && topology.visibility != "full" {
            limitations.push("replica-membership-unresolved".to_string());
        }
        sort_dedup(&mut limitations);
        format::DatasetScope {
            contract: dbwarp_blueprint_core::DATASET_SCOPE_CONTRACT.to_string(),
            layout: "full-copy".to_string(),
            table_inventory_completeness: "complete".to_string(),
            row_count_completeness: "complete".to_string(),
            size_completeness: "complete".to_string(),
            row_count_method: "mysql-table-statistics".to_string(),
            size_method: "mysql-information-schema".to_string(),
            limitations,
        }
    };

    MysqlSizingAssessment {
        topology,
        dataset_scope,
        suppress_table_statistics: distributed,
        distributed_size_unavailable: distributed,
    }
}

#[derive(Debug, Clone, Default)]
struct ColumnRow {
    schema_name: String,
    table_name: String,
    ordinal: u32,
    col_name: String,
    col_type: String,
    native_type: String,
    is_nullable: bool,
    char_max_length: u64,
    char_octet_length: u64,
    numeric_precision: u64,
    numeric_scale: u64,
    numeric_unsigned: bool,
    bit_width: u64,
    datetime_precision: u64,
    character_set_name: String,
    collation_name: String,
}

impl ColumnRow {
    fn col_name_lower(&self) -> String {
        self.col_name.to_ascii_lowercase()
    }
}

#[derive(Debug, Clone)]
struct IndexRow {
    schema_name: String,
    table_name: String,
    index_name: String,
    non_unique: bool,
    index_type: String,
    seq: u32,
    col_name: String,
    prefix_length: u64,
    descending: bool,
    expression: bool,
}

type IndexPart = (u32, String, bool, String, bool, bool, u64, bool);

fn index_blueprint_from_parts(
    parts: &[IndexPart],
    col_to_ord: &BTreeMap<String, u32>,
    length_fidelity: LengthFidelity,
) -> BlueprintIndex {
    let unique = parts.first().map(|part| part.2).unwrap_or(false);
    let primary = parts.first().map(|part| part.4).unwrap_or(false);
    let descending = parts.iter().any(|part| part.5);
    let expression = parts.iter().any(|part| part.7);
    let method = parts
        .first()
        .map(|part| normalized_index_method(&part.3))
        .unwrap_or_else(|| "btree".to_string());
    let mut col_ords = Vec::with_capacity(parts.len());
    let mut prefix_lengths = Vec::with_capacity(parts.len());
    let mut sorted_parts = parts.to_vec();
    sorted_parts.sort_by_key(|part| part.0);
    for (_, name, _, _, _, _, prefix_length, _) in sorted_parts {
        if let Some(ordinal) = col_to_ord.get(&name.to_ascii_lowercase()) {
            col_ords.push(*ordinal);
            prefix_lengths.push(blueprint_prefix_length(prefix_length, length_fidelity));
        }
    }
    if prefix_lengths.iter().all(|length| *length == 0) {
        prefix_lengths.clear();
    }
    BlueprintIndex {
        index_type: method,
        primary,
        unique,
        cols: col_ords,
        prefix_lengths,
        include_cols: Vec::new(),
        expression,
        filtered: false,
        descending,
        ..BlueprintIndex::default()
    }
}

#[derive(Debug, Clone)]
struct FkRow {
    from_schema: String,
    from_table: String,
    to_schema: String,
    to_table: String,
    constraint_name: String,
    position: u32,
    col_name: String,
    to_col_name: String,
    on_update: String,
    on_delete: String,
    match_type: String,
}

fn normalize_fk_rule(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(' ', "-")
}

fn normalize_mysql_fk_match(value: &str) -> String {
    match value.trim().to_ascii_uppercase().as_str() {
        "" | "NONE" | "SIMPLE" => "simple".to_string(),
        "FULL" => "full".to_string(),
        "PARTIAL" => "partial".to_string(),
        other => other.to_ascii_lowercase(),
    }
}

/// Run 5× `SELECT 1` round trips and return the median latency in ms.
/// Recorded as one summary entry in the audit log for clarity.
async fn probe_rtt(conn: &mut mysql_async::Conn, audit: &mut AuditLog) -> Result<(u64, u64)> {
    let total_started = Instant::now();
    let mut samples_us: Vec<u64> = Vec::with_capacity(5);
    for _ in 0..5 {
        let started = Instant::now();
        let _: Option<i64> = conn
            .query_first("SELECT 1")
            .await
            .context("RTT probe SELECT 1")?;
        samples_us.push(started.elapsed().as_micros() as u64);
    }
    let percentiles = rtt_percentiles_ms(&mut samples_us);
    audit.record_query(
        "5x SELECT 1 (RTT probe; constant integer 1, no row data)",
        elapsed_ms(total_started),
        5,
    );
    Ok(percentiles)
}

async fn resolve_mysql_schemas(
    conn: &mut mysql_async::Conn,
    requested: &SchemaSelection,
    audit: &mut AuditLog,
) -> Result<SchemaSelection> {
    if !requested.is_active() {
        return Ok(SchemaSelection::default());
    }
    let sql = format!(
        "SELECT SCHEMA_NAME FROM information_schema.SCHEMATA \
         WHERE SCHEMA_NAME NOT IN ('mysql','information_schema','performance_schema','sys'){} \
         ORDER BY SCHEMA_NAME",
        requested.and_sql("SCHEMA_NAME")
    );
    let started = Instant::now();
    let names: Vec<String> = conn
        .query(sql)
        .await
        .context("resolving requested MySQL schemas")?;
    audit.record_query(
        "SELECT requested schema visibility from information_schema.SCHEMATA (names discarded)",
        elapsed_ms(started),
        names.len() as u64,
    );
    resolved_selection(requested, names, true)
}

async fn probe_mysql_topology(
    conn: &mut mysql_async::Conn,
    version: &str,
    audit: &mut AuditLog,
) -> MysqlTopologyEvidence {
    let mut evidence = MysqlTopologyEvidence {
        server_identity_readable: true,
        vitess_gateway: version.to_ascii_lowercase().contains("vitess"),
        ..MysqlTopologyEvidence::default()
    };
    if evidence.vitess_gateway {
        return evidence;
    }

    let started = Instant::now();
    let capabilities: Result<Option<(u64, u64)>> = conn
        .query_first(
            r#"
            SELECT CAST(COALESCE(SUM(TABLE_NAME = 'replication_group_members'), 0) AS UNSIGNED),
                   CAST(COALESCE(SUM(TABLE_NAME = 'replication_connection_status'), 0) AS UNSIGNED)
            FROM information_schema.TABLES
            WHERE TABLE_SCHEMA = 'performance_schema'
              AND TABLE_NAME IN ('replication_group_members', 'replication_connection_status')
            "#,
        )
        .await
        .context("probing MySQL topology catalog capabilities");
    match capabilities {
        Ok(Some((group, replica))) => {
            evidence.capability_catalog_readable = true;
            evidence.group_catalog_present = group > 0;
            evidence.replica_catalog_present = replica > 0;
            audit.record_query(
                "SELECT fixed Performance Schema topology-table capability counts (no identifiers)",
                elapsed_ms(started),
                1,
            );
        }
        Ok(None) => {
            evidence.capability_catalog_readable = true;
            audit.record_query(
                "SELECT fixed Performance Schema topology-table capability counts (no identifiers)",
                elapsed_ms(started),
                0,
            );
        }
        Err(_) => {
            crate::topology::warn_evidence_unavailable(audit, "mysql-group-members");
            crate::topology::warn_evidence_unavailable(audit, "mysql-replica-status");
        }
    }

    if evidence.group_catalog_present {
        let started = Instant::now();
        let group: Result<Option<(u64, u64, u64, u64, u64)>> = conn
            .query_first(
                r#"
                SELECT COUNT(*),
                       CAST(COALESCE(SUM(MEMBER_ROLE = 'PRIMARY'), 0) AS UNSIGNED),
                       CAST(COALESCE(SUM(MEMBER_ROLE = 'SECONDARY'), 0) AS UNSIGNED),
                       CAST(COALESCE(SUM(MEMBER_ID = @@server_uuid AND MEMBER_ROLE = 'PRIMARY'), 0) AS UNSIGNED),
                       CAST(COALESCE(SUM(MEMBER_ID = @@server_uuid AND MEMBER_ROLE = 'SECONDARY'), 0) AS UNSIGNED)
                FROM performance_schema.replication_group_members
                "#,
            )
            .await
            .context("reading aggregate MySQL Group Replication topology");
        match group {
            Ok(Some((members, primaries, secondaries, local_primary, local_secondary))) => {
                evidence.group_catalog_readable = true;
                evidence.group_member_count = members;
                evidence.group_primary_count = primaries;
                evidence.group_secondary_count = secondaries;
                evidence.local_group_role = if local_primary > 0 {
                    Some("primary")
                } else if local_secondary > 0 {
                    Some("secondary")
                } else {
                    None
                };
                audit.record_query(
                    "SELECT Group Replication member/role counts (member identities discarded server-side)",
                    elapsed_ms(started),
                    1,
                );
            }
            Ok(None) => evidence.group_catalog_readable = true,
            Err(_) => crate::topology::warn_evidence_unavailable(audit, "mysql-group-members"),
        }
        if evidence.group_catalog_readable && evidence.group_member_count > 0 {
            return evidence;
        }
    }

    if evidence.replica_catalog_present {
        let started = Instant::now();
        let channels: Result<Option<u64>> = conn
            .query_first(
                r#"
                SELECT COUNT(*)
                FROM performance_schema.replication_connection_status
                WHERE CHANNEL_NAME NOT IN ('group_replication_applier', 'group_replication_recovery')
                "#,
            )
            .await
            .context("reading aggregate MySQL replica channel count");
        match channels {
            Ok(count) => {
                evidence.replica_catalog_readable = true;
                evidence.replica_channel_count = count.unwrap_or(0);
                audit.record_query(
                    "SELECT asynchronous replica channel count (channel/source identities not selected)",
                    elapsed_ms(started),
                    1,
                );
            }
            Err(_) => crate::topology::warn_evidence_unavailable(audit, "mysql-replica-status"),
        }
    }

    evidence.wsrep_catalog_attempted = true;
    let started = Instant::now();
    let wsrep: Result<Option<(String, String)>> = conn
        .query_first("SHOW GLOBAL VARIABLES LIKE 'wsrep_on'")
        .await
        .context("probing Galera wsrep capability");
    match wsrep {
        Ok(value) => {
            evidence.wsrep_catalog_readable = true;
            audit.record_query(
                "SHOW GLOBAL VARIABLES LIKE 'wsrep_on' (fixed variable only)",
                elapsed_ms(started),
                u64::from(value.is_some()),
            );
            let active = value
                .as_ref()
                .map(|(_, value)| matches!(value.to_ascii_uppercase().as_str(), "ON" | "1"))
                .unwrap_or(false);
            if active {
                let started = Instant::now();
                let status: Result<Vec<(String, String)>> = conn
                    .query(
                        r#"
                        SHOW GLOBAL STATUS
                        WHERE Variable_name IN (
                            'wsrep_cluster_size',
                            'wsrep_cluster_status',
                            'wsrep_local_state_comment',
                            'wsrep_ready'
                        )
                        "#,
                    )
                    .await
                    .context("reading bounded Galera topology status");
                match status {
                    Ok(rows) => {
                        let mut size = 0_u64;
                        let mut primary_component = false;
                        let mut ready = false;
                        for (name, value) in &rows {
                            match name.as_str() {
                                "wsrep_cluster_size" => {
                                    size = value.parse::<u64>().unwrap_or(0);
                                }
                                "wsrep_cluster_status" => {
                                    primary_component = value.eq_ignore_ascii_case("primary");
                                }
                                "wsrep_ready" => {
                                    ready =
                                        matches!(value.to_ascii_uppercase().as_str(), "ON" | "1");
                                }
                                _ => {}
                            }
                        }
                        evidence.galera_active = primary_component && ready && size > 0;
                        evidence.galera_member_count = size;
                        audit.record_query(
                            "SHOW fixed Galera count/health status variables (identities excluded)",
                            elapsed_ms(started),
                            rows.len() as u64,
                        );
                    }
                    Err(_) => {
                        evidence.wsrep_catalog_readable = false;
                        crate::topology::warn_evidence_unavailable(audit, "mysql-wsrep-status");
                    }
                }
            }
        }
        Err(_) => crate::topology::warn_evidence_unavailable(audit, "mysql-wsrep-status"),
    }
    evidence
}

#[derive(Debug, Clone)]
struct CompressionSample {
    table: BlueprintCompression,
    columns: Vec<Option<BlueprintCompression>>,
    column_lengths: Vec<Option<(u64, u64)>>,
    null_fractions: Vec<Option<f64>>,
    cardinalities: Vec<Option<format::BlueprintCardinality>>,
}

include!("engine_mysql_sampling.rs");
fn tracing_eprintln(msg: String) {
    eprintln!("dbwarp-blueprint: {msg}");
}

include!("engine_mysql_artifacts.rs");
include!("engine_mysql_tests.rs");
