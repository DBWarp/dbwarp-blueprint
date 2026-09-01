//! PostgreSQL engine — catalog reader (Tier 1) + compression sampler (Tier 2).
//!
//! Connects via `tokio-postgres`. Reads catalog tables only in Tier 1.
//! Tier 2 additionally runs `TABLESAMPLE SYSTEM(0.1) LIMIT N` per table,
//! zstd-compresses locally, records ratio + stddev, discards bytes.
//!
//! All identifiers in the output are anonymized via `format::table_id`,
//! `format::col_id`, `format::idx_id`, `format::schema_id`. Numeric
//! statistics are rounded via the `format::round_*` helpers.
//!
//! No row content is ever written to the output file. The style classifier
//! returns ONE LABEL per column.

use std::collections::BTreeMap;
use std::time::Instant;

use anyhow::{anyhow, bail, Context, Result};
use tokio_postgres::{Config as PgConfig, NoTls, SimpleQueryMessage};
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
use crate::tls::{self, TlsMode, TlsParams};
use crate::topology::{
    sort_dedup, sort_topology, warn_evidence_unavailable as warn_topology_unavailable,
    warn_incomplete_dataset_scope,
};

/// Classify a PG column type (catalog-string form) into a row-frame
/// TypeTag. The string is `format_type(atttypid, atttypmod)` — examples:
/// "integer", "bigint", "text", "character varying(64)",
/// "timestamp with time zone", "numeric(12,4)", "uuid", "jsonb", "bytea",
/// "boolean", "double precision", "real", "date".
///
/// We strip any "(...)" length/precision suffix before matching.
/// Unknown types fall back to `UnknownText` — the tool still samples
/// them via simple_query (which returns the textual representation
/// for any type), they just don't carry a precise type-tag to the
/// estimator.
fn type_tag_for_pg_str(type_str: &str) -> TypeTag {
    let head = type_str
        .split('(')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    match head.as_str() {
        "boolean" | "bool" => TypeTag::BoolText,
        "smallint" | "int2" | "integer" | "int4" | "bigint" | "int8" | "real" | "float4"
        | "double precision" | "float8" | "numeric" | "decimal" => TypeTag::NumberText,
        "date" => TypeTag::DateText,
        "time" | "time without time zone" | "time with time zone" | "timetz" => TypeTag::TimeText,
        "timestamp"
        | "timestamp without time zone"
        | "timestamp with time zone"
        | "timestamptz" => TypeTag::TimestampText,
        "uuid" => TypeTag::UuidText,
        "json" | "jsonb" => TypeTag::JsonText,
        "text" | "varchar" | "character varying" | "character" | "char" | "name" | "citext" => {
            TypeTag::TextUtf8
        }
        "bytea" => TypeTag::BinaryRaw,
        _ => TypeTag::UnknownText,
    }
}

fn normalized_pg_type(type_str: &str) -> String {
    let t = type_str.trim().to_ascii_lowercase();
    if let Some(inner) = t.strip_suffix("[]") {
        return format!("array<{}>", normalized_pg_type(inner));
    }
    let head = t.split('(').next().unwrap_or("").trim();
    match head {
        "boolean" | "bool" => "boolean".to_string(),
        "smallint" | "int2" => "smallint".to_string(),
        "integer" | "int4" => "integer".to_string(),
        "bigint" | "int8" => "bigint".to_string(),
        "real" | "float4" => "real".to_string(),
        "double precision" | "float8" => "double precision".to_string(),
        "numeric" | "decimal" => {
            if let Some(args) = t.strip_prefix(head).and_then(|s| s.strip_prefix('(')) {
                if let Some(args) = args.strip_suffix(')') {
                    if args
                        .chars()
                        .all(|c| c.is_ascii_digit() || c == ',' || c == ' ')
                    {
                        return format!("numeric({})", args.replace(' ', ""));
                    }
                }
            }
            "numeric".to_string()
        }
        "date" => "date".to_string(),
        "time" | "time without time zone" | "time with time zone" | "timetz" => "time".to_string(),
        "timestamp"
        | "timestamp without time zone"
        | "timestamp with time zone"
        | "timestamptz" => "timestamp".to_string(),
        "uuid" => "uuid".to_string(),
        "json" | "jsonb" => "json".to_string(),
        "text" | "varchar" | "character varying" | "character" | "char" | "name" | "citext" => {
            "text".to_string()
        }
        "bytea" => "binary".to_string(),
        _ => "user-defined".to_string(),
    }
}

/// Return PostgreSQL's declared character capacity when `format_type` exposes
/// one. PostgreSQL reports `character varying(n)` and `character(n)` limits in
/// characters, not bytes; the encoded byte ceiling therefore remains unknown.
/// Keep this parser deliberately narrow so a domain or user-defined type is
/// never assigned a capacity inferred from its name.
fn declared_pg_max_chars(type_str: &str) -> u64 {
    let mut normalized = type_str.trim().to_ascii_lowercase();
    while let Some(element) = normalized.strip_suffix("[]") {
        normalized = element.trim_end().to_string();
    }
    for prefix in ["character varying(", "varchar(", "character(", "char("] {
        let Some(value) = normalized
            .strip_prefix(prefix)
            .and_then(|rest| rest.strip_suffix(')'))
        else {
            continue;
        };
        if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) {
            return value.parse::<u64>().unwrap_or(0);
        }
    }
    0
}

fn normalized_index_method(method: &str) -> String {
    match method.trim().to_ascii_lowercase().as_str() {
        "btree" | "hash" | "gin" | "gist" | "spgist" | "brin" => method.trim().to_ascii_lowercase(),
        _ => "other".to_string(),
    }
}

/// Retain only PostgreSQL's numeric product version in the transferable
/// Blueprint. Packaged `server_version` values can append distribution or
/// build text; that transient producer detail is not needed for capability
/// selection and must not become an output string.
fn normalized_pg_version(raw: &str) -> String {
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

/// Tier-2 query deadline default.
pub const DEFAULT_SAMPLE_TIMEOUT_SECS: u64 = 300;

/// Style-classifier sample used only during Tier-2 row sampling. The bounded
/// source bytes are classified locally and are never emitted.
const STYLE_PEEK_BYTES: usize = 4096;

/// Source-kind annotation. Customer-declared, propagated to the report.
#[derive(Debug, Clone)]
pub enum SourceKind {
    Production,
    Staging,
    ScrubbedReplica,
    Synthetic,
}

impl SourceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::Staging => "staging",
            Self::ScrubbedReplica => "scrubbed-replica",
            Self::Synthetic => "synthetic",
        }
    }

    pub fn parse(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "production" | "prod" => Ok(Self::Production),
            "staging" | "stage" => Ok(Self::Staging),
            "scrubbed-replica" | "scrubbed" => Ok(Self::ScrubbedReplica),
            "synthetic" | "test" | "synth" => Ok(Self::Synthetic),
            other => bail!(
                "unknown source_kind '{other}'; expected one of \
                 production | staging | scrubbed-replica | synthetic"
            ),
        }
    }
}

/// Connection parameters extracted from the customer's --connect URI plus
/// optional flags (TLS, password source, etc.).
#[derive(Debug, Clone)]
pub struct PgConnectParams {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub uri_user_was_explicit: bool,
    pub redacted_uri: String,
}

impl PgConnectParams {
    /// Parse a `postgresql://[user[:password]@]host[:port]/db` URI.
    /// Returns the parts plus an embedded password if present (caller wraps
    /// in Secret if so). The `redacted_uri` is safe to log.
    pub fn parse(uri: &str) -> Result<(Self, Option<String>)> {
        let rest = uri
            .strip_prefix("postgresql://")
            .or_else(|| uri.strip_prefix("postgres://"))
            .ok_or_else(|| anyhow!("URI must start with postgresql:// or postgres://"))?;
        let (authority, dbpart) = crate::uri_authority::split_authority_and_path(rest);
        // Authority: [user[:password]@]host[:port]
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
            None => ("postgres".to_string(), None, false),
        };
        // Bracket-aware host:port split (IPv6-safe).
        let (host, port) = crate::uri_authority::split_host_port(hostport, 5432)?;
        // Strip query parameters from db.
        let database = match dbpart.find('?') {
            Some(i) => percent_decode(&dbpart[..i]),
            None => percent_decode(dbpart),
        };
        if database.is_empty() {
            bail!("URI is missing database (postgresql://user@host:port/DATABASE)");
        }
        let redacted_uri = format!("postgresql://{}@{}:{}/{}", user, host, port, database);
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
pub struct PgRunOpts {
    pub measure_compression: bool,
    pub compression_workers: usize,
    pub sample_rows: u64,
    pub sample_timeout_secs: u64,
    pub source_kind: SourceKind,
    pub tls: TlsParams,
    /// `--generated-at` CLI value, if the customer pinned a timestamp
    /// for byte-identical reproducibility runs.
    pub generated_at_pin: Option<String>,
    /// Run 5× SELECT 1 after connect to capture customer-side observed
    /// round-trip latency. Default true; `--no-rtt-probe` opts out.
    /// Adds ~5×RTT to wall time (~5–500 ms depending on network).
    pub rtt_probe: bool,
    /// An externally generated managed-service token is being supplied through
    /// PostgreSQL's ordinary password protocol inside verified TLS.
    pub cloud_token_auth: bool,
    pub artifact_detail: ArtifactDetail,
    pub schemas: SchemaSelection,
}

pub async fn run(
    params: &PgConnectParams,
    secret: &Secret,
    opts: &PgRunOpts,
    audit: &mut AuditLog,
) -> Result<BlueprintFile> {
    audit.connection.uri_redacted = params.redacted_uri.clone();
    audit.connection.tls_mode = opts.tls.mode.as_str().to_string();
    audit.connection.tls_ca_path = opts.tls.ca_bundle.clone();
    audit.connection.tls_client_cert = opts.tls.client_cert.clone();
    // Enumerate the PEM files we will read.
    audit.record_tls_file_reads(
        opts.tls.ca_bundle.as_ref(),
        opts.tls.client_cert.as_ref(),
        opts.tls.client_key.as_ref(),
    );

    tls::validate(&opts.tls, &params.host)?;
    if opts.cloud_token_auth && opts.tls.mode != TlsMode::VerifyFull {
        bail!("DBP1604E PostgreSQL cloud-token authentication requires --tls-mode=verify-full");
    }

    let mut cfg = PgConfig::new();
    cfg.host(&params.host)
        .port(params.port)
        .dbname(&params.database)
        .user(&params.user)
        .password(secret.expose())
        .application_name("dbwarp-blueprint");

    let connect_started = Instant::now();
    let conn_handle: tokio::task::JoinHandle<Option<(&'static str, String)>>;
    let client: tokio_postgres::Client;
    {
        let tls_cfg_opt = tls::build_client_config(&opts.tls)?;
        match (opts.tls.mode, tls_cfg_opt) {
            (TlsMode::Disable, _) => {
                let (c, connection) = cfg
                    .connect(NoTls)
                    .await
                    .with_context(|| format!("connecting plain to {}", params.redacted_uri))?;
                audit.connection.ssl_negotiated = "no (plaintext transport)".to_string();
                client = c;
                conn_handle = tokio::spawn(async move {
                    connection
                        .await
                        .err()
                        .map(|error| ("engine.pg.connection_error", error.to_string()))
                });
            }
            (mode, Some((rustls_cfg, ca_only))) => {
                use tokio_postgres_rustls::MakeRustlsConnect;
                let connector = MakeRustlsConnect::new(rustls_cfg.as_ref().clone());
                audit.connection.tls_ca_only = ca_only;
                match cfg.connect(connector.clone()).await {
                    Ok((c, connection)) => {
                        audit.connection.ssl_negotiated =
                            "yes (protocol version unavailable from driver)".to_string();
                        client = c;
                        conn_handle = tokio::spawn(async move {
                            connection
                                .await
                                .err()
                                .map(|error| ("engine.pg.tls_connection_error", error.to_string()))
                        });
                        let _ = mode;
                    }
                    Err(e) => {
                        if mode == TlsMode::Prefer {
                            // Fall back to plain.
                            let redacted = crate::i18n::format(
                                "engine.driver_detail_redacted",
                                &[("target", "TLS negotiation".to_string())],
                            );
                            let detail = crate::i18n::format(
                                "engine.pg.tls_fallback",
                                &[("code", "DBP1404W".to_string()), ("error", redacted)],
                            );
                            tracing_eprintln(detail.clone());
                            audit.record_warning("DBP1404W", detail);
                            let (c, connection) = cfg.connect(NoTls).await.with_context(|| {
                                format!(
                                    "connecting plain to {} (after TLS prefer failed)",
                                    params.redacted_uri
                                )
                            })?;
                            audit.connection.ssl_negotiated =
                                "no (loopback-only TLS prefer fallback)".to_string();
                            client = c;
                            conn_handle = tokio::spawn(async move {
                                connection
                                    .await
                                    .err()
                                    .map(|error| ("engine.pg.connection_error", error.to_string()))
                            });
                        } else {
                            return Err(anyhow!(
                                "TLS connection to {} failed: {e}",
                                params.redacted_uri
                            ));
                        }
                    }
                }
            }
            (TlsMode::Prefer, None) => {
                // Shouldn't happen — Prefer always returns Some — but be safe.
                let (c, connection) = cfg
                    .connect(NoTls)
                    .await
                    .with_context(|| format!("connecting plain to {}", params.redacted_uri))?;
                audit.connection.ssl_negotiated = "no (plaintext transport)".to_string();
                client = c;
                conn_handle = tokio::spawn(async move {
                    connection
                        .await
                        .err()
                        .map(|error| ("engine.pg.connection_error", error.to_string()))
                });
            }
            (_, None) => {
                bail!("internal: TLS config not built for non-Disable mode");
            }
        }
    }
    audit.connection.auth = if opts.cloud_token_auth {
        "cloud-token/postgresql-password-protocol".to_string()
    } else {
        "scram-sha-256-or-md5".to_string()
    };
    audit.network_egress.push(format!(
        "{}:{} (database-driver session; DNS may use the configured resolver)",
        params.host, params.port
    ));
    let connect_total = connect_started.elapsed();

    // The outer Tokio deadline bounds the client, but dropping a database
    // future is not proof that the server stopped executing it. PostgreSQL's
    // session GUC independently cancels every statement on the server.
    let timeout_ms = opts
        .sample_timeout_secs
        .saturating_mul(1000)
        .min(i32::MAX as u64);
    let timeout_started = Instant::now();
    client
        .batch_execute(&format!("SET statement_timeout = {timeout_ms}"))
        .await
        .context("setting PostgreSQL session statement_timeout")?;
    audit.record_query(
        "SET statement_timeout = <max-wall-secs> (session-local safety limit)",
        elapsed_ms(timeout_started),
        0,
    );

    // RTT probe — 5× SELECT 1 for customer-side observed round-trip
    // statistics. Captured BEFORE catalog queries so the timings are
    // not skewed by cache warmup.
    let network_probe = if opts.rtt_probe {
        match probe_rtt(&client, audit).await {
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

    // engine_version
    let engine_version = fetch_engine_version(&client, audit).await?;
    let schemas = resolve_pg_schemas(&client, &opts.schemas, audit).await?;

    // Establish the meaning of the local catalog totals before reading them.
    // In particular, ordinary PostgreSQL size functions materially undercount
    // distributed Citus tables on a coordinator.
    let topology_evidence = probe_pg_topology(&client, &schemas, audit).await;
    let mut sizing = classify_pg_topology(&topology_evidence);

    // Catalog walk.
    let mut table_capture = list_tables(&client, audit, sizing.table_size_mode).await?;
    table_capture
        .tables
        .retain(|table| schemas.includes(&table.schema_name));
    sizing.record_table_capture(&table_capture, audit);
    schemas.qualify_dataset_scope(&mut sizing.dataset_scope);
    warn_incomplete_dataset_scope(&sizing.dataset_scope, audit);
    let tables = table_capture.tables;
    let columns = list_columns(&client, audit).await?;
    let indexes = list_indexes(&client, audit).await?;
    let fks = list_foreign_keys(&client, audit).await?;

    // Anonymize: assign stable ordinals.
    let mut sorted = tables.clone();
    sorted.sort_by_key(|t| format::table_hash(&t.schema_name, &t.table_name));
    let mut id_by_oid: BTreeMap<u32, String> = BTreeMap::new();
    let mut schema_id_by_name: BTreeMap<String, String> = BTreeMap::new();
    {
        let mut schema_seen: Vec<String> = sorted
            .iter()
            .map(|t| t.schema_name.clone())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        schema_seen.sort_by_key(|s| format::schema_hash(s));
        for (i, name) in schema_seen.iter().enumerate() {
            schema_id_by_name.insert(name.clone(), format::schema_id(i + 1));
        }
    }
    for (i, t) in sorted.iter().enumerate() {
        id_by_oid.insert(t.oid, format::table_id(i + 1));
    }

    // Group columns + indexes by table OID.
    let mut cols_by_oid: BTreeMap<u32, Vec<ColumnRow>> = BTreeMap::new();
    for c in columns {
        cols_by_oid.entry(c.relid).or_default().push(c);
    }
    for v in cols_by_oid.values_mut() {
        v.sort_by_key(|c| c.attnum);
    }
    let mut idx_by_oid: BTreeMap<u32, Vec<IndexRow>> = BTreeMap::new();
    for idx in indexes {
        idx_by_oid.entry(idx.indrelid).or_default().push(idx);
    }
    for v in idx_by_oid.values_mut() {
        v.sort_by_key(|i| format::index_hash(&i.indexname));
    }

    // Tier-2 compression sampling, if enabled.
    let mut compression_by_oid: BTreeMap<u32, CompressionSample> = BTreeMap::new();
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
                for t in &sorted {
                    if Instant::now() >= deadline {
                        let detail = crate::i18n::format(
                            "engine.sample_budget",
                            &[("code", "DBP1406W".to_string())],
                        );
                        tracing_eprintln(detail.clone());
                        audit.record_warning("DBP1406W", detail);
                        break;
                    }
                    if t.sampling_empty_proven {
                        audit.record_proven_empty_table_skipped();
                        continue;
                    }
                    let cols_for_table: &[ColumnRow] =
                        cols_by_oid.get(&t.oid).map(|v| v.as_slice()).unwrap_or(&[]);
                    let table_id = id_by_oid
                        .get(&t.oid)
                        .map(String::as_str)
                        .unwrap_or("table-unknown");
                    match sample_compression(
                        &client,
                        t,
                        cols_for_table,
                        opts.sample_rows,
                        &compression_pool,
                        audit,
                    )
                    .await
                    {
                        Ok(Some(pending)) => {
                            if pipeline_started.is_none() {
                                pipeline_started = Some(pending.submitted_at);
                            }
                            pending_samples.push((t.oid, table_id.to_string(), pending));
                        }
                        Ok(None) => { /* table empty; skip */ }
                        Err(_) => {
                            warn_compression_unavailable(table_id, audit);
                        }
                    }
                }
                for (oid, table_id, pending) in pending_samples {
                    match pending.ticket.resolve() {
                        Ok(measurements) => {
                            audit.record_compression_job_completed(&measurements.work);
                            compression_by_oid.insert(
                                oid,
                                CompressionSample {
                                    table: measurements.table,
                                    columns: measurements.columns,
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

    // Style classification: a small bounded peek per text/jsonb/xml column,
    // bytes never leave the process. Only the label is emitted.
    // To keep Tier 1 catalog-only, skip style classification by default — only
    // run it under --measure-compression so behavior matches the consent prompt.
    let mut style_by_col: BTreeMap<(u32, i16), &'static str> = BTreeMap::new();
    if opts.measure_compression {
        let deadline =
            Instant::now() + std::time::Duration::from_secs(opts.sample_timeout_secs.max(1));
        for t in &sorted {
            if Instant::now() >= deadline {
                let detail = crate::i18n::format(
                    "engine.column_budget",
                    &[("code", "DBP1406W".to_string())],
                );
                tracing_eprintln(detail.clone());
                audit.record_warning("DBP1406W", detail);
                break;
            }
            if t.sampling_empty_proven {
                continue;
            }
            if let Some(cols) = cols_by_oid.get(&t.oid) {
                for (column_index, c) in cols.iter().enumerate() {
                    if !is_text_like(&c.type_str) {
                        continue;
                    }
                    match peek_column_style(&client, t, c).await {
                        Ok(label) if !label.is_empty() => {
                            style_by_col.insert((t.oid, c.attnum), label);
                        }
                        Ok(_) => {}
                        Err(_) => {
                            let table_id = id_by_oid
                                .get(&t.oid)
                                .map(String::as_str)
                                .unwrap_or("table-unknown");
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
                                &[("code", "DBP1408W".to_string()), ("error", redacted)],
                            );
                            tracing_eprintln(detail.clone());
                            audit.record_warning("DBP1408W", detail);
                        }
                    }
                }
            }
        }
    }

    // Build BlueprintFile.
    let mut tables_out: BTreeMap<String, BlueprintTable> = BTreeMap::new();
    let mut totals = Totals::default();
    for t in &sorted {
        let tid = id_by_oid.get(&t.oid).cloned().unwrap_or_default();
        let schema_anon = schema_id_by_name
            .get(&t.schema_name)
            .cloned()
            .unwrap_or_else(|| "schema-?".to_string());
        let compression_sample = compression_by_oid.remove(&t.oid);
        let mut col_map: BTreeMap<String, BlueprintColumn> = BTreeMap::new();
        if let Some(cs) = cols_by_oid.get(&t.oid) {
            for (col_pos, c) in cs.iter().enumerate() {
                let style_label = style_by_col.get(&(t.oid, c.attnum)).copied().unwrap_or("");
                let column_compression = compression_sample
                    .as_ref()
                    .and_then(|sample| sample.columns.get(col_pos))
                    .cloned()
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
                col_map.insert(
                    format::col_id(c.attnum as u32),
                    BlueprintColumn {
                        ordinal: c.attnum as u32,
                        column_type: normalized_pg_type(&c.type_str),
                        nullable: !c.not_null,
                        null_fraction,
                        native_type: String::new(),
                        declared_max_chars: declared_pg_max_chars(&c.type_str),
                        declared_max_bytes: 0,
                        numeric_precision: 0,
                        numeric_scale: 0,
                        datetime_precision: 0,
                        charset: String::new(),
                        collation: String::new(),
                        len_avg: format::round_len_avg(c.len_avg),
                        len_p95: format::round_len_p95(c.len_p95),
                        style: style_label.to_string(),
                        compression: column_compression,
                        cardinality: column_cardinality,
                        ..BlueprintColumn::default()
                    },
                );
            }
        }
        let mut idx_map: BTreeMap<String, BlueprintIndex> = BTreeMap::new();
        if let Some(is_) = idx_by_oid.get(&t.oid) {
            for (i, ix) in is_.iter().enumerate() {
                idx_map.insert(
                    format::idx_id((i + 1) as u32),
                    BlueprintIndex {
                        index_type: normalized_index_method(&ix.method),
                        primary: ix.is_primary,
                        unique: ix.is_unique,
                        cols: ix.col_ords.clone(),
                        prefix_lengths: Vec::new(),
                        include_cols: ix.include_ords.clone(),
                        expression: ix.has_expression,
                        filtered: ix.has_filter,
                        descending: ix.has_descending,
                        ..BlueprintIndex::default()
                    },
                );
            }
        }
        let table_blueprint = BlueprintTable {
            rows: format::round_rows(t.reltuples.max(0.0) as u64),
            table_bytes: format::round_bytes(t.table_bytes),
            index_bytes: format::round_bytes(t.index_bytes),
            schema: schema_anon,
            has_clustered_index: false, // PG always false (heap-only)
            stats_freshness: t.stats_freshness.clone(),
            cols: col_map,
            idxs: idx_map,
            compression: compression_sample.map(|sample| sample.table),
            ..BlueprintTable::default()
        };
        accumulate_table_totals(&mut totals, &table_blueprint)?;
        tables_out.insert(tid, table_blueprint);
    }
    totals.table_count = tables_out.len() as u64;

    // FK edges, anonymized.
    let mut fk_edges: BTreeMap<String, Vec<FkEdge>> = BTreeMap::new();
    for fk in &fks {
        let from = match id_by_oid.get(&fk.from_oid) {
            Some(s) => s.clone(),
            None => continue,
        };
        let to = match id_by_oid.get(&fk.to_oid) {
            Some(s) => s.clone(),
            None => continue,
        };
        fk_edges.entry(from).or_default().push(FkEdge {
            to,
            cols: fk.cols.clone(),
            to_cols: fk.to_cols.clone(),
            on_update: fk.on_update.clone(),
            on_delete: fk.on_delete.clone(),
            match_type: fk.match_type.clone(),
            deferrable: fk.deferrable,
            initially_deferred: fk.initially_deferred,
            validated: fk.validated,
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

    let table_artifact_ids: BTreeMap<String, String> = sorted
        .iter()
        .filter_map(|table| {
            id_by_oid.get(&table.oid).map(|id| {
                (
                    artifacts::table_identity("postgresql", &table.schema_name, &table.table_name),
                    id.clone(),
                )
            })
        })
        .collect();
    let artifact_inventory = if opts.artifact_detail == ArtifactDetail::None {
        None
    } else {
        let (mut raw_artifacts, completeness) = capture_artifacts(
            &client,
            opts.artifact_detail,
            &engine_version,
            &schemas,
            audit,
        )
        .await;
        raw_artifacts.retain(|item| {
            item.schema_identity
                .as_deref()
                .is_none_or(|schema| schemas.includes(schema))
        });
        Some(artifacts::build_inventory(
            opts.artifact_detail,
            raw_artifacts,
            &schema_id_by_name,
            &table_artifact_ids,
            completeness,
        ))
    };

    let mut blueprint = BlueprintFile {
        schema_version: SCHEMA_VERSION,
        // Pinning is via `--generated-at` CLI flag, never an env var.
        // See format::generated_at_now.
        generated_at: format::generated_at_now(opts.generated_at_pin.as_deref()),
        engine: "postgresql".to_string(),
        engine_version,
        source_kind: opts.source_kind.as_str().to_string(),
        length_metadata: "hybrid-v2".to_string(),
        declared_length_fidelity: "exact".to_string(),
        index_length_fidelity: "not-captured".to_string(),
        observed_length_fidelity: if opts.measure_compression {
            "coarse-rounded-v1"
        } else {
            "not-sampled"
        }
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

    // Wrap up the connection driver (it'll exit when the client is dropped).
    drop(client);
    let driver_warning = match conn_handle.await {
        Ok(warning) => warning,
        Err(error) => Some((
            "engine.pg.connection_error",
            format!("connection task join failed: {error}"),
        )),
    };
    if let Some((key, _error)) = driver_warning {
        let redacted = crate::i18n::format(
            "engine.driver_detail_redacted",
            &[(
                "target",
                "asynchronous PostgreSQL connection task".to_string(),
            )],
        );
        let detail = crate::i18n::format(
            key,
            &[("code", "DBP1409W".to_string()), ("error", redacted)],
        );
        tracing_eprintln(detail.clone());
        audit.record_warning("DBP1409W", detail);
    }
    Ok(blueprint)
}

// ---------------------------------------------------------------------------
// Catalog reads
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct TableRow {
    oid: u32,
    schema_name: String,
    table_name: String,
    reltuples: f64,
    table_bytes: u64,
    index_bytes: u64,
    stats_freshness: String,
    /// True only when statistics prove the table was analyzed after its last
    /// modification and the resulting row estimate is zero.
    sampling_empty_proven: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct PgTopologyEvidence {
    base_readable: bool,
    in_recovery: Option<bool>,
    citus_installed: Option<bool>,
    replication_catalog_readable: bool,
    direct_peer_count: Option<u64>,
    citus_metadata_readable: bool,
    distributed_table_count: Option<u64>,
    local_group_id: Option<i64>,
    registered_member_count: Option<u64>,
    coordinator_count: Option<u64>,
    worker_count: Option<u64>,
    local_member_registered: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PgTableSizeMode {
    Local,
    CitusAggregate,
    CitusLocalMember,
    Suppress,
}

#[derive(Debug, Clone)]
struct PgTableCapture {
    tables: Vec<TableRow>,
    distributed_size_complete: bool,
}

#[derive(Debug, Clone)]
struct PgSizingAssessment {
    topology: format::DatabaseTopology,
    dataset_scope: format::DatasetScope,
    table_size_mode: PgTableSizeMode,
}

impl PgSizingAssessment {
    fn record_table_capture(&mut self, capture: &PgTableCapture, audit: &mut AuditLog) {
        if capture
            .tables
            .iter()
            .any(|table| table.stats_freshness != "fresh")
        {
            self.dataset_scope
                .limitations
                .push("statistics-stale".to_string());
        }
        if self.table_size_mode != PgTableSizeMode::CitusAggregate {
            sort_dedup(&mut self.dataset_scope.limitations);
            return;
        }
        if capture.distributed_size_complete {
            self.topology
                .catalogs_read
                .push("citus-relation-size".to_string());
            self.dataset_scope.size_completeness = "complete".to_string();
            self.dataset_scope.size_method = "citus-distributed-relation-size".to_string();
        } else {
            self.topology
                .catalogs_unreadable
                .push("citus-relation-size".to_string());
            self.dataset_scope.size_completeness = "incomplete".to_string();
            self.dataset_scope.size_method = "unknown".to_string();
            self.dataset_scope
                .limitations
                .push("distributed-aggregate-unavailable".to_string());
            self.dataset_scope
                .limitations
                .push("distributed-size-unavailable".to_string());
            crate::topology::warn_distributed_size_unavailable(audit);
        }
        crate::topology::sort_dedup(&mut self.topology.catalogs_read);
        crate::topology::sort_dedup(&mut self.topology.catalogs_unreadable);
        crate::topology::sort_dedup(&mut self.dataset_scope.limitations);
    }
}

#[derive(Debug, Clone)]
struct ColumnRow {
    relid: u32,
    attnum: i16,
    attname: String,
    type_str: String,
    not_null: bool,
    len_avg: u64,
    len_p95: u64,
}

#[derive(Debug, Clone)]
struct IndexRow {
    indrelid: u32,
    indexname: String,
    method: String,
    is_primary: bool,
    is_unique: bool,
    col_ords: Vec<u32>,
    include_ords: Vec<u32>,
    has_expression: bool,
    has_filter: bool,
    has_descending: bool,
}

#[derive(Debug, Clone)]
struct FkRow {
    from_oid: u32,
    to_oid: u32,
    cols: Vec<u32>,
    to_cols: Vec<u32>,
    on_update: String,
    on_delete: String,
    match_type: String,
    deferrable: bool,
    initially_deferred: bool,
    validated: bool,
}

/// Run 5× `SELECT 1` round trips and return the median latency in
/// milliseconds. The 5 queries appear in the audit log as a single
/// summary entry rather than 5 individual rows — keeps the audit
/// terse while still being truthful.
async fn probe_rtt(client: &tokio_postgres::Client, audit: &mut AuditLog) -> Result<(u64, u64)> {
    let total_started = Instant::now();
    let mut samples_us: Vec<u64> = Vec::with_capacity(5);
    for _ in 0..5 {
        let started = Instant::now();
        client
            .query_one("SELECT 1::bigint", &[])
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

async fn fetch_engine_version(
    client: &tokio_postgres::Client,
    audit: &mut AuditLog,
) -> Result<String> {
    let started = Instant::now();
    let row = client
        .query_one("SELECT current_setting('server_version')", &[])
        .await
        .context("querying server_version")?;
    let v: String = row
        .try_get(0)
        .context("decoding PostgreSQL server_version")?;
    audit.record_query(
        "SELECT current_setting('server_version')",
        elapsed_ms(started),
        1,
    );
    Ok(normalized_pg_version(&v))
}

async fn resolve_pg_schemas(
    client: &tokio_postgres::Client,
    requested: &SchemaSelection,
    audit: &mut AuditLog,
) -> Result<SchemaSelection> {
    if !requested.is_active() {
        return Ok(SchemaSelection::default());
    }
    let sql = format!(
        "SELECT nspname FROM pg_namespace WHERE nspname NOT IN ('pg_catalog','information_schema') \
         AND nspname !~ '^pg_toast'{} ORDER BY nspname",
        requested.and_sql("nspname")
    );
    let started = Instant::now();
    let rows = client
        .query(&sql, &[])
        .await
        .context("resolving requested PostgreSQL schemas")?;
    audit.record_query(
        "SELECT requested schema visibility from pg_namespace (names discarded)",
        elapsed_ms(started),
        rows.len() as u64,
    );
    let names = rows
        .into_iter()
        .map(|row| row.try_get::<_, String>(0))
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("decoding requested PostgreSQL schema visibility")?;
    resolved_selection(requested, names, false)
}

async fn probe_pg_topology(
    client: &tokio_postgres::Client,
    schemas: &SchemaSelection,
    audit: &mut AuditLog,
) -> PgTopologyEvidence {
    let mut evidence = PgTopologyEvidence::default();
    let started = Instant::now();
    match client
        .query_one(
            "SELECT pg_is_in_recovery(), EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'citus')",
            &[],
        )
        .await
    {
        Ok(row) => {
            let (in_recovery, citus_installed) = match (
                row.try_get::<_, bool>(0),
                row.try_get::<_, bool>(1),
            ) {
                (Ok(in_recovery), Ok(citus_installed)) => (in_recovery, citus_installed),
                _ => {
                    warn_topology_unavailable(audit, "pg-is-in-recovery");
                    warn_topology_unavailable(audit, "pg-extension");
                    return evidence;
                }
            };
            evidence.base_readable = true;
            evidence.in_recovery = Some(in_recovery);
            evidence.citus_installed = Some(citus_installed);
            audit.record_query(
                "SELECT pg_is_in_recovery(), Citus extension presence (topology capability probe; no identifiers)",
                elapsed_ms(started),
                1,
            );
        }
        Err(_) => {
            warn_topology_unavailable(audit, "pg-is-in-recovery");
            warn_topology_unavailable(audit, "pg-extension");
            return evidence;
        }
    }

    let replication_catalog = if evidence.in_recovery == Some(true) {
        "pg-stat-wal-receiver"
    } else {
        "pg-stat-replication"
    };
    let replication_sql = if evidence.in_recovery == Some(true) {
        "SELECT count(*)::bigint FROM pg_stat_wal_receiver"
    } else {
        "SELECT count(*)::bigint FROM pg_stat_replication"
    };
    let started = Instant::now();
    match client.query_one(replication_sql, &[]).await {
        Ok(row) => {
            let count: i64 = match row.try_get(0) {
                Ok(count) => count,
                Err(_) => {
                    warn_topology_unavailable(audit, replication_catalog);
                    return evidence;
                }
            };
            evidence.replication_catalog_readable = true;
            evidence.direct_peer_count = Some(count.max(0) as u64);
            audit.record_query(
                if evidence.in_recovery == Some(true) {
                    "SELECT receiver count FROM pg_stat_wal_receiver (no endpoint or identity columns)"
                } else {
                    "SELECT sender count FROM pg_stat_replication (no endpoint or identity columns)"
                },
                elapsed_ms(started),
                1,
            );
        }
        Err(_) => warn_topology_unavailable(audit, replication_catalog),
    }

    if evidence.citus_installed != Some(true) {
        return evidence;
    }

    let started = Instant::now();
    let citus_schema_predicate = schemas.and_sql("n.nspname");
    let citus_sql = format!(
        r#"
        WITH local_group AS (
            SELECT groupid::bigint AS group_id
            FROM pg_dist_local_group
            LIMIT 1
        )
        SELECT (SELECT count(*)::bigint
                FROM pg_dist_partition p
                JOIN pg_class c ON c.oid = p.logicalrelid
                JOIN pg_namespace n ON n.oid = c.relnamespace
                WHERE TRUE{citus_schema_predicate}) AS distributed_tables,
               (SELECT group_id FROM local_group) AS local_group_id,
               (SELECT count(*)::bigint FROM pg_dist_node) AS registered_members,
               (SELECT count(*)::bigint FROM pg_dist_node WHERE groupid = 0) AS coordinators,
               (SELECT count(*)::bigint FROM pg_dist_node WHERE groupid <> 0) AS workers,
               EXISTS (
                   SELECT 1
                   FROM pg_dist_node n
                   JOIN local_group l ON l.group_id = n.groupid::bigint
               ) AS local_member_registered
    "#
    );
    match client.query_one(&citus_sql, &[]).await {
        Ok(row) => {
            let nonnegative = |value: i64| value.max(0) as u64;
            let parsed = (
                row.try_get::<_, i64>(0),
                row.try_get::<_, Option<i64>>(1),
                row.try_get::<_, i64>(2),
                row.try_get::<_, i64>(3),
                row.try_get::<_, i64>(4),
                row.try_get::<_, bool>(5),
            );
            let (distributed, local_group, members, coordinators, workers, local_registered) =
                match parsed {
                    (Ok(a), Ok(b), Ok(c), Ok(d), Ok(e), Ok(f)) => (a, b, c, d, e, f),
                    _ => {
                        warn_topology_unavailable(audit, "citus-metadata");
                        return evidence;
                    }
                };
            evidence.citus_metadata_readable = true;
            evidence.distributed_table_count = Some(nonnegative(distributed));
            evidence.local_group_id = local_group;
            evidence.registered_member_count = Some(nonnegative(members));
            evidence.coordinator_count = Some(nonnegative(coordinators));
            evidence.worker_count = Some(nonnegative(workers));
            evidence.local_member_registered = local_registered;
            audit.record_query(
                "SELECT Citus table/member/role counts from fixed metadata catalogs (identifiers discarded)",
                elapsed_ms(started),
                1,
            );
        }
        Err(_) => warn_topology_unavailable(audit, "citus-metadata"),
    }
    evidence
}

fn classify_pg_topology(evidence: &PgTopologyEvidence) -> PgSizingAssessment {
    if !evidence.base_readable {
        let mut topology = format::DatabaseTopology::unknown();
        topology.catalogs_unreadable =
            vec!["pg-extension".to_string(), "pg-is-in-recovery".to_string()];
        return PgSizingAssessment {
            topology,
            dataset_scope: format::DatasetScope::unknown_database("unknown", "unknown"),
            table_size_mode: PgTableSizeMode::Suppress,
        };
    }

    let in_recovery = evidence.in_recovery.unwrap_or(false);
    let mut topology = format::DatabaseTopology {
        contract: dbwarp_blueprint_core::TOPOLOGY_CONTRACT.to_string(),
        deployment: "unknown".to_string(),
        local_role: if in_recovery { "secondary" } else { "primary" }.to_string(),
        visibility: "partial".to_string(),
        member_count: 1,
        identifiers_redacted: true,
        role_counts: BTreeMap::from([(
            if in_recovery { "secondary" } else { "primary" }.to_string(),
            1,
        )]),
        features: Vec::new(),
        catalogs_read: vec!["pg-extension".to_string(), "pg-is-in-recovery".to_string()],
        catalogs_unreadable: Vec::new(),
    };

    let replication_catalog = if in_recovery {
        "pg-stat-wal-receiver"
    } else {
        "pg-stat-replication"
    };
    if evidence.replication_catalog_readable {
        topology.catalogs_read.push(replication_catalog.to_string());
        let peers = evidence.direct_peer_count.unwrap_or(0);
        if peers > 0 {
            topology.deployment = "replicated".to_string();
            topology
                .features
                .push("postgresql-streaming-replication".to_string());
            if in_recovery {
                topology.member_count = 2;
                topology.role_counts.insert("primary".to_string(), 1);
            } else {
                topology.member_count = 1_u64.saturating_add(peers);
                topology.role_counts.insert("secondary".to_string(), peers);
            }
        }
    } else {
        topology
            .catalogs_unreadable
            .push(replication_catalog.to_string());
    }

    if evidence.citus_installed != Some(true) {
        let mut limitations = vec![
            "row-counts-statistical".to_string(),
            "topology-visibility-partial".to_string(),
        ];
        if topology.deployment == "replicated" || in_recovery {
            limitations.push("replica-membership-unresolved".to_string());
        }
        sort_dedup(&mut limitations);
        sort_topology(&mut topology);
        return PgSizingAssessment {
            topology,
            dataset_scope: format::DatasetScope {
                contract: dbwarp_blueprint_core::DATASET_SCOPE_CONTRACT.to_string(),
                layout: "full-copy".to_string(),
                table_inventory_completeness: "complete".to_string(),
                row_count_completeness: "complete".to_string(),
                size_completeness: "complete".to_string(),
                row_count_method: "postgres-planner-estimate".to_string(),
                size_method: "postgres-local-relation-size".to_string(),
                limitations,
            },
            table_size_mode: PgTableSizeMode::Local,
        };
    }

    topology.features.push("citus".to_string());
    topology.deployment = "distributed".to_string();
    if !evidence.citus_metadata_readable {
        topology
            .catalogs_unreadable
            .push("citus-metadata".to_string());
        sort_topology(&mut topology);
        let mut scope = format::DatasetScope::unknown_database("unknown", "unknown");
        scope
            .limitations
            .push("distributed-aggregate-unavailable".to_string());
        scope
            .limitations
            .push("shard-membership-incomplete".to_string());
        sort_dedup(&mut scope.limitations);
        return PgSizingAssessment {
            topology,
            dataset_scope: scope,
            table_size_mode: PgTableSizeMode::Suppress,
        };
    }

    topology.catalogs_read.push("citus-metadata".to_string());
    let local_group = evidence.local_group_id;
    topology.local_role = match local_group {
        Some(0) => "coordinator",
        Some(_) => "worker",
        None => "unknown",
    }
    .to_string();
    topology.member_count = evidence.registered_member_count.unwrap_or(0);
    topology.role_counts.clear();
    let mut coordinators = evidence.coordinator_count.unwrap_or(0);
    let mut workers = evidence.worker_count.unwrap_or(0);
    if !evidence.local_member_registered {
        topology.member_count = topology.member_count.saturating_add(1);
        match topology.local_role.as_str() {
            "coordinator" => coordinators = coordinators.saturating_add(1),
            "worker" => workers = workers.saturating_add(1),
            _ => {}
        }
    }
    if coordinators > 0 {
        topology
            .role_counts
            .insert("coordinator".to_string(), coordinators);
    }
    if workers > 0 {
        topology.role_counts.insert("worker".to_string(), workers);
    }
    let distributed_tables = evidence.distributed_table_count.unwrap_or(0);
    let is_coordinator = topology.local_role == "coordinator";
    let (dataset_scope, table_size_mode) = if distributed_tables == 0 && is_coordinator {
        (
            format::DatasetScope {
                contract: dbwarp_blueprint_core::DATASET_SCOPE_CONTRACT.to_string(),
                layout: "full-copy".to_string(),
                table_inventory_completeness: "complete".to_string(),
                row_count_completeness: "complete".to_string(),
                size_completeness: "complete".to_string(),
                row_count_method: "postgres-planner-estimate".to_string(),
                size_method: "postgres-local-relation-size".to_string(),
                limitations: vec![
                    "row-counts-statistical".to_string(),
                    "topology-visibility-partial".to_string(),
                ],
            },
            PgTableSizeMode::Local,
        )
    } else if is_coordinator {
        (
            format::DatasetScope {
                contract: dbwarp_blueprint_core::DATASET_SCOPE_CONTRACT.to_string(),
                layout: "distributed".to_string(),
                table_inventory_completeness: "complete".to_string(),
                row_count_completeness: "incomplete".to_string(),
                size_completeness: "unknown".to_string(),
                row_count_method: "unknown".to_string(),
                size_method: "unknown".to_string(),
                limitations: vec![
                    "distributed-row-count-unavailable".to_string(),
                    "row-counts-statistical".to_string(),
                    "topology-visibility-partial".to_string(),
                ],
            },
            PgTableSizeMode::CitusAggregate,
        )
    } else {
        (
            format::DatasetScope {
                contract: dbwarp_blueprint_core::DATASET_SCOPE_CONTRACT.to_string(),
                layout: "distributed".to_string(),
                table_inventory_completeness: "incomplete".to_string(),
                row_count_completeness: "incomplete".to_string(),
                size_completeness: "incomplete".to_string(),
                row_count_method: "postgres-planner-estimate".to_string(),
                size_method: "postgres-local-relation-size".to_string(),
                limitations: vec![
                    "local-member-only".to_string(),
                    "row-counts-statistical".to_string(),
                    "shard-membership-incomplete".to_string(),
                    "topology-visibility-partial".to_string(),
                ],
            },
            PgTableSizeMode::CitusLocalMember,
        )
    };
    sort_topology(&mut topology);
    PgSizingAssessment {
        topology,
        dataset_scope,
        table_size_mode,
    }
}

async fn list_tables(
    client: &tokio_postgres::Client,
    audit: &mut AuditLog,
    mode: PgTableSizeMode,
) -> Result<PgTableCapture> {
    let local_sql = "
        SELECT c.oid::int8 AS oid,
               n.nspname AS schema_name,
               c.relname AS table_name,
               c.reltuples::float8 AS reltuples,
               pg_table_size(c.oid)::int8 AS table_bytes,
               pg_indexes_size(c.oid)::int8 AS index_bytes,
               COALESCE(s.last_analyze, s.last_autoanalyze) AS last_analyze,
               (COALESCE(s.last_analyze, s.last_autoanalyze) IS NOT NULL
                AND COALESCE(s.n_mod_since_analyze, 0) = 0
                AND c.reltuples = 0) AS sampling_empty_proven
        FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        LEFT JOIN pg_stat_all_tables s ON s.relid = c.oid
        WHERE c.relkind = 'r'
          AND n.nspname NOT IN ('pg_catalog','information_schema')
          AND n.nspname NOT LIKE 'pg_temp_%'
          AND n.nspname NOT LIKE 'pg_toast_temp_%'
        ORDER BY 1
    ";
    let citus_sql = "
        SELECT c.oid::int8 AS oid,
               n.nspname AS schema_name,
               c.relname AS table_name,
               CASE WHEN p.logicalrelid IS NULL THEN c.reltuples::float8 ELSE 0::float8 END AS reltuples,
               CASE WHEN p.logicalrelid IS NULL
                    THEN pg_table_size(c.oid)::int8
                    ELSE distributed_size.table_bytes
               END AS table_bytes,
               CASE WHEN p.logicalrelid IS NULL
                    THEN pg_indexes_size(c.oid)::int8
                    ELSE GREATEST(
                        distributed_size.total_bytes - distributed_size.table_bytes,
                        0::int8
                    )
               END AS index_bytes,
               COALESCE(s.last_analyze, s.last_autoanalyze) AS last_analyze,
               (p.logicalrelid IS NULL
                AND COALESCE(s.last_analyze, s.last_autoanalyze) IS NOT NULL
                AND COALESCE(s.n_mod_since_analyze, 0) = 0
                AND c.reltuples = 0) AS sampling_empty_proven
        FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        LEFT JOIN pg_stat_all_tables s ON s.relid = c.oid
        LEFT JOIN pg_dist_partition p ON p.logicalrelid = c.oid
        LEFT JOIN LATERAL (
            SELECT citus_table_size(p.logicalrelid)::int8 AS table_bytes,
                   citus_total_relation_size(p.logicalrelid)::int8 AS total_bytes
            WHERE p.logicalrelid IS NOT NULL
        ) distributed_size ON true
        WHERE c.relkind = 'r'
          AND n.nspname NOT IN ('pg_catalog','information_schema')
          AND n.nspname NOT LIKE 'pg_temp_%'
          AND n.nspname NOT LIKE 'pg_toast_temp_%'
        ORDER BY 1
    ";
    let citus_safe_sql = "
        SELECT c.oid::int8 AS oid,
               n.nspname AS schema_name,
               c.relname AS table_name,
               CASE WHEN p.logicalrelid IS NULL THEN c.reltuples::float8 ELSE 0::float8 END AS reltuples,
               CASE WHEN p.logicalrelid IS NULL THEN pg_table_size(c.oid)::int8 ELSE 0::int8 END AS table_bytes,
               CASE WHEN p.logicalrelid IS NULL THEN pg_indexes_size(c.oid)::int8 ELSE 0::int8 END AS index_bytes,
               COALESCE(s.last_analyze, s.last_autoanalyze) AS last_analyze,
               (p.logicalrelid IS NULL
                AND COALESCE(s.last_analyze, s.last_autoanalyze) IS NOT NULL
                AND COALESCE(s.n_mod_since_analyze, 0) = 0
                AND c.reltuples = 0) AS sampling_empty_proven
        FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        LEFT JOIN pg_stat_all_tables s ON s.relid = c.oid
        LEFT JOIN pg_dist_partition p ON p.logicalrelid = c.oid
        WHERE c.relkind = 'r'
          AND n.nspname NOT IN ('pg_catalog','information_schema')
          AND n.nspname NOT LIKE 'pg_temp_%'
          AND n.nspname NOT LIKE 'pg_toast_temp_%'
        ORDER BY 1
    ";
    let suppressed_sql = "
        SELECT c.oid::int8 AS oid,
               n.nspname AS schema_name,
               c.relname AS table_name,
               0::float8 AS reltuples,
               0::int8 AS table_bytes,
               0::int8 AS index_bytes,
               COALESCE(s.last_analyze, s.last_autoanalyze) AS last_analyze,
               false AS sampling_empty_proven
        FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        LEFT JOIN pg_stat_all_tables s ON s.relid = c.oid
        WHERE c.relkind = 'r'
          AND n.nspname NOT IN ('pg_catalog','information_schema')
          AND n.nspname NOT LIKE 'pg_temp_%'
          AND n.nspname NOT LIKE 'pg_toast_temp_%'
        ORDER BY 1
    ";

    let (rows, distributed_size_complete) = match mode {
        PgTableSizeMode::Local | PgTableSizeMode::CitusLocalMember => (
            query_table_rows(client, audit, local_sql, "local").await?,
            false,
        ),
        PgTableSizeMode::Suppress => (
            query_table_rows(client, audit, suppressed_sql, "suppressed").await?,
            false,
        ),
        PgTableSizeMode::CitusAggregate => {
            match query_table_rows(client, audit, citus_sql, "Citus aggregate").await {
                Ok(rows) => (rows, true),
                Err(_) => {
                    let rows = match query_table_rows(
                        client,
                        audit,
                        citus_safe_sql,
                        "Citus aggregate suppressed",
                    )
                    .await
                    {
                        Ok(rows) => rows,
                        Err(_) => {
                            query_table_rows(client, audit, suppressed_sql, "suppressed").await?
                        }
                    };
                    (rows, false)
                }
            }
        }
    };
    Ok(PgTableCapture {
        tables: rows,
        distributed_size_complete,
    })
}

async fn query_table_rows(
    client: &tokio_postgres::Client,
    audit: &mut AuditLog,
    sql: &str,
    size_evidence: &str,
) -> Result<Vec<TableRow>> {
    let started = Instant::now();
    let rows = client.query(sql, &[]).await.context("listing tables")?;
    audit.record_query(
        &format!(
            "SELECT ... FROM pg_class JOIN pg_namespace ... (table list; {size_evidence} size evidence)"
        ),
        elapsed_ms(started),
        rows.len() as u64,
    );
    let mut out = Vec::with_capacity(rows.len());
    let now = chrono::Utc::now();
    for r in rows {
        let oid: i64 = r.try_get("oid").context("decoding table oid")?;
        let schema_name: String = r.try_get("schema_name").context("decoding table schema")?;
        let table_name: String = r.try_get("table_name").context("decoding table name")?;
        let reltuples: f64 = r
            .try_get("reltuples")
            .context("decoding table row estimate")?;
        let table_bytes: i64 = r
            .try_get("table_bytes")
            .context("decoding table byte estimate")?;
        let index_bytes: i64 = r
            .try_get("index_bytes")
            .context("decoding index byte estimate")?;
        let last_analyze: Option<chrono::DateTime<chrono::Utc>> = r
            .try_get("last_analyze")
            .context("decoding table statistics timestamp")?;
        let sampling_empty_proven: bool = r
            .try_get("sampling_empty_proven")
            .context("decoding proven-empty sampling flag")?;
        let stats_freshness = if reltuples < 0.0 {
            "never_analyzed".to_string()
        } else {
            match last_analyze {
                None => "never_analyzed".to_string(),
                Some(t) => {
                    let age = now.signed_duration_since(t);
                    if age.num_days() <= 7 {
                        "fresh".to_string()
                    } else {
                        "stale".to_string()
                    }
                }
            }
        };
        out.push(TableRow {
            oid: oid as u32,
            schema_name,
            table_name,
            reltuples,
            table_bytes: table_bytes.max(0) as u64,
            index_bytes: index_bytes.max(0) as u64,
            stats_freshness,
            sampling_empty_proven,
        });
    }
    Ok(out)
}

async fn list_columns(
    client: &tokio_postgres::Client,
    audit: &mut AuditLog,
) -> Result<Vec<ColumnRow>> {
    // Per-attribute info plus rough avg/p95 length for variable-length types.
    // Length stats come from pg_stats (samples-based, ANALYZE-driven). Stats
    // may be missing — substitute 0 in that case.
    let sql = "
        SELECT a.attrelid::int8 AS relid,
               a.attnum::int2   AS attnum,
               a.attname        AS attname,
               format_type(a.atttypid, a.atttypmod) AS type_str,
               a.attnotnull     AS not_null,
               COALESCE(s.avg_width, 0)::int4 AS avg_width
        FROM pg_attribute a
        JOIN pg_class c ON c.oid = a.attrelid
        JOIN pg_namespace n ON n.oid = c.relnamespace
        LEFT JOIN pg_stats s
               ON s.schemaname = n.nspname
              AND s.tablename  = c.relname
              AND s.attname    = a.attname
        WHERE a.attnum > 0
          AND NOT a.attisdropped
          AND c.relkind = 'r'
          AND n.nspname NOT IN ('pg_catalog','information_schema')
        ORDER BY a.attrelid, a.attnum
    ";
    let started = Instant::now();
    let rows = client.query(sql, &[]).await.context("listing columns")?;
    audit.record_query(
        "SELECT format_type(...), avg_width FROM pg_attribute JOIN pg_stats (column list)",
        elapsed_ms(started),
        rows.len() as u64,
    );
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let relid: i64 = r.try_get("relid").context("decoding column table oid")?;
        let attnum: i16 = r.try_get("attnum").context("decoding column ordinal")?;
        let attname: String = r.try_get("attname").context("decoding column name")?;
        let type_str: String = r.try_get("type_str").context("decoding column type")?;
        let not_null: bool = r
            .try_get("not_null")
            .context("decoding column nullability")?;
        let avg_width: i32 = r
            .try_get("avg_width")
            .context("decoding column average width")?;
        out.push(ColumnRow {
            relid: relid as u32,
            attnum,
            attname,
            type_str,
            not_null,
            len_avg: avg_width.max(0) as u64,
            len_p95: 0, // PG doesn't store p95 directly in pg_stats; populated from sampling later if needed.
        });
    }
    Ok(out)
}

async fn list_indexes(
    client: &tokio_postgres::Client,
    audit: &mut AuditLog,
) -> Result<Vec<IndexRow>> {
    let sql = "
        SELECT i.indrelid::int8 AS indrelid,
               c.relname        AS indexname,
               am.amname        AS method,
               i.indisprimary   AS is_primary,
               i.indisunique    AS is_unique,
               i.indnkeyatts    AS key_att_count,
               i.indkey::int2[] AS all_ords,
               i.indoption::int2[] AS ind_options,
               i.indexprs IS NOT NULL AS has_expression,
               i.indpred IS NOT NULL AS has_filter
        FROM pg_index i
        JOIN pg_class c  ON c.oid  = i.indexrelid
        JOIN pg_am  am   ON am.oid = c.relam
        JOIN pg_class tc ON tc.oid = i.indrelid
        JOIN pg_namespace n ON n.oid = tc.relnamespace
        WHERE tc.relkind = 'r'
          AND n.nspname NOT IN ('pg_catalog','information_schema')
        ORDER BY i.indrelid, c.relname
    ";
    let started = Instant::now();
    let rows = client.query(sql, &[]).await.context("listing indexes")?;
    audit.record_query(
        "SELECT amname, indisprimary, indisunique, indnkeyatts, indkey, indoption FROM pg_index JOIN pg_class JOIN pg_am ... (index list)",
        elapsed_ms(started),
        rows.len() as u64,
    );
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let indrelid: i64 = r.try_get("indrelid").context("decoding index table oid")?;
        let indexname: String = r.try_get("indexname").context("decoding index name")?;
        let method: String = r.try_get("method").context("decoding index method")?;
        let is_primary: bool = r
            .try_get("is_primary")
            .context("decoding primary-index flag")?;
        let is_unique: bool = r
            .try_get("is_unique")
            .context("decoding unique-index flag")?;
        let key_att_count: i16 = r
            .try_get("key_att_count")
            .context("decoding index key-column count")?;
        let all_ords: Vec<i16> = r
            .try_get("all_ords")
            .context("decoding index column ordinals")?;
        let ind_options: Vec<i16> = r
            .try_get("ind_options")
            .context("decoding index column options")?;
        let has_expression: bool = r
            .try_get("has_expression")
            .context("decoding expression-index flag")?;
        let has_filter: bool = r
            .try_get("has_filter")
            .context("decoding partial-index flag")?;
        let mut col_ords = Vec::new();
        let mut include_ords = Vec::new();
        for (idx, ord) in all_ords.iter().enumerate() {
            if *ord <= 0 {
                continue;
            }
            if idx < key_att_count.max(0) as usize {
                col_ords.push(*ord as u32);
            } else {
                include_ords.push(*ord as u32);
            }
        }
        let has_descending = ind_options.iter().any(|option| (option & 1) != 0);
        out.push(IndexRow {
            indrelid: indrelid as u32,
            indexname,
            method,
            is_primary,
            is_unique,
            col_ords,
            include_ords,
            has_expression: has_expression || all_ords.iter().any(|ord| *ord <= 0),
            has_filter,
            has_descending,
        });
    }
    Ok(out)
}

async fn list_foreign_keys(
    client: &tokio_postgres::Client,
    audit: &mut AuditLog,
) -> Result<Vec<FkRow>> {
    let sql = "
        SELECT con.conrelid::int8   AS from_oid,
               con.confrelid::int8 AS to_oid,
               con.conkey::int2[]  AS cols,
               con.confkey::int2[] AS to_cols,
               con.confupdtype::text AS update_action,
               con.confdeltype::text AS delete_action,
               con.confmatchtype::text AS match_type,
               con.condeferrable,
               con.condeferred,
               con.convalidated
        FROM pg_constraint con
        JOIN pg_class c ON c.oid = con.conrelid
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE con.contype = 'f'
          AND n.nspname NOT IN ('pg_catalog','information_schema')
        ORDER BY con.conrelid, con.confrelid
    ";
    let started = Instant::now();
    let rows = client
        .query(sql, &[])
        .await
        .context("listing foreign keys")?;
    audit.record_query(
        "SELECT conrelid, confrelid, conkey, confkey FROM pg_constraint (FK list)",
        elapsed_ms(started),
        rows.len() as u64,
    );
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let from_oid: i64 = r
            .try_get("from_oid")
            .context("decoding foreign-key source oid")?;
        let to_oid: i64 = r
            .try_get("to_oid")
            .context("decoding foreign-key target oid")?;
        let cols: Vec<i16> = r
            .try_get("cols")
            .context("decoding foreign-key source columns")?;
        let to_cols: Vec<i16> = r
            .try_get("to_cols")
            .context("decoding foreign-key target columns")?;
        let update_action: String = r
            .try_get("update_action")
            .context("decoding foreign-key update action")?;
        let delete_action: String = r
            .try_get("delete_action")
            .context("decoding foreign-key delete action")?;
        let match_type: String = r
            .try_get("match_type")
            .context("decoding foreign-key match type")?;
        let deferrable: bool = r
            .try_get("condeferrable")
            .context("decoding foreign-key deferrable flag")?;
        let initially_deferred: bool = r
            .try_get("condeferred")
            .context("decoding foreign-key initially-deferred flag")?;
        let validated: bool = r
            .try_get("convalidated")
            .context("decoding foreign-key validation flag")?;
        out.push(FkRow {
            from_oid: from_oid as u32,
            to_oid: to_oid as u32,
            cols: cols.into_iter().map(|n| n as u32).collect(),
            to_cols: to_cols.into_iter().map(|n| n as u32).collect(),
            on_update: pg_fk_action(update_action.as_str()).to_string(),
            on_delete: pg_fk_action(delete_action.as_str()).to_string(),
            match_type: pg_fk_match(match_type.as_str()).to_string(),
            deferrable,
            initially_deferred,
            validated,
        });
    }
    Ok(out)
}

fn pg_fk_action(code: &str) -> &'static str {
    match code {
        "c" => "cascade",
        "r" => "restrict",
        "n" => "set-null",
        "d" => "set-default",
        _ => "no-action",
    }
}

fn pg_fk_match(code: &str) -> &'static str {
    match code {
        "f" => "full",
        "p" => "partial",
        _ => "simple",
    }
}

// ---------------------------------------------------------------------------
// Tier 2: compression sampling
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct CompressionSample {
    table: BlueprintCompression,
    columns: Vec<Option<BlueprintCompression>>,
    null_fractions: Vec<Option<f64>>,
    cardinalities: Vec<Option<format::BlueprintCardinality>>,
}

include!("engine_pg_sampling.rs");
fn tracing_eprintln(msg: String) {
    eprintln!("dbwarp-blueprint: {msg}");
}

include!("engine_pg_artifacts.rs");
include!("engine_pg_tests.rs");
