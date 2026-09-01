//! SQL Server (TDS) engine — catalog reader (Tier 1) + compression sampler (Tier 2).
//!
//! Connects via `tiberius` (TDS 7.3, rustls TLS feature). Reads sys.* views
//! only in Tier 1. Tier 2 additionally runs `SELECT ... ORDER BY <pk>
//! OFFSET 0 ROWS FETCH NEXT N ROWS ONLY` per table (TABLESAMPLE SYSTEM
//! exists but is page-lumpy; the OFFSET/FETCH path is more predictable
//! and is flagged as biased).
//!
//! Auth modes (selected by `--auth-mode`):
//!   * `sql-auth`     — classic username + password. Always available.
//!   * `entra-token`  — Microsoft Entra ID (Azure AD) OAuth access token.
//!                      Always available; consumed via the same Secret
//!                      wrapper as a password.
//!   * `integrated`   — Kerberos (Linux) / SSPI (Windows). Available only
//!                      when the binary is built with the
//!                      `integrated-auth-gssapi` feature (Linux) or
//!                      `winauth` feature (Windows). Vanilla builds
//!                      reject this mode with a rebuild hint.
//!
//! See AUTH.md for the full per-mode customer-pasteable recipes.

use std::collections::BTreeMap;
use std::time::Instant;

use anyhow::{anyhow, bail, Context, Result};
use tiberius::{AuthMethod, Client, ColumnData, ColumnType, Config, EncryptionLevel};
use tokio::net::TcpStream;
use tokio_util::compat::TokioAsyncWriteCompatExt;
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

const STYLE_PEEK_BYTES: usize = 4096;

#[derive(Debug, Clone)]
pub struct MssqlConnectParams {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub user: String,
    pub uri_user_was_explicit: bool,
    pub redacted_uri: String,
}

impl MssqlConnectParams {
    /// Accepts sqlserver://[user[:password]@]host[:port]/db, mssql://, tds://.
    pub fn parse(uri: &str) -> Result<(Self, Option<String>)> {
        let rest = uri
            .strip_prefix("sqlserver://")
            .or_else(|| uri.strip_prefix("mssql://"))
            .or_else(|| uri.strip_prefix("tds://"))
            .ok_or_else(|| anyhow!("URI must start with sqlserver:// (or mssql://, tds://)"))?;
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
            None => ("sa".to_string(), None, false),
        };
        // Bracket-aware host:port split with SQL Server's
        // `host,port` comma alternative. IPv6-safe.
        let (host, port) = crate::uri_authority::split_host_port_mssql(hostport, 1433)?;
        let database = match dbpart.find('?') {
            Some(i) => percent_decode(&dbpart[..i]),
            None => percent_decode(dbpart),
        };
        if database.is_empty() {
            bail!("URI is missing database (sqlserver://user@host:port/DATABASE)");
        }
        let redacted_uri = format!("sqlserver://{}@{}:{}/{}", user, host, port, database);
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
pub struct MssqlRunOpts {
    pub measure_compression: bool,
    pub compression_workers: usize,
    pub sample_rows: u64,
    pub sample_timeout_secs: u64,
    pub source_kind_str: String,
    pub tls: TlsParams,
    pub generated_at_pin: Option<String>,
    /// Run 5× SELECT 1 after connect to capture customer-side observed
    /// round-trip latency. Default true; `--no-rtt-probe` opts out.
    pub rtt_probe: bool,
    /// Authentication method to dispatch. `SqlAuth` uses the supplied
    /// (user, password); `EntraToken` ignores the user field and
    /// passes the secret bytes verbatim as a Microsoft Entra ID OAuth
    /// access token via `tiberius::AuthMethod::aad_token`.
    pub auth_mode: MssqlAuthMode,
    /// Optional fail-closed assertion against ORIGINAL_LOGIN() on the same
    /// established session used for capture.
    pub expected_server_principal: Option<String>,
    pub artifact_detail: ArtifactDetail,
    pub schemas: SchemaSelection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MssqlAuthMode {
    SqlAuth,
    EntraToken,
    /// Kerberos (Linux: tiberius integrated-auth-gssapi feature) or
    /// SSPI (Windows: winauth feature). Only constructible when the
    /// matching feature is compiled in; main.rs's
    /// `resolve_mssql_auth_mode` enforces this by erroring before
    /// the dispatch ever sees an Integrated value on a vanilla build.
    Integrated,
}

pub async fn run(
    params: &MssqlConnectParams,
    secret: &Secret,
    opts: &MssqlRunOpts,
    audit: &mut AuditLog,
) -> Result<BlueprintFile> {
    crate::tls::validate(&opts.tls, &params.host)?;

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

    let mut config = Config::new();
    config.host(&params.host);
    config.port(params.port);
    config.database(&params.database);

    // Auth dispatch:
    //   SqlAuth     → AuthMethod::sql_server(user, password)
    //   EntraToken  → AuthMethod::aad_token(token)  — Microsoft Entra ID
    //                  (Azure AD) OAuth access token. Customer generates
    //                  the token externally with `az account
    //                  get-access-token --resource https://database.windows.net/`.
    //                  The token IS the credential; the username field
    //                  in the URI is ignored by tiberius for AAD.
    //   Integrated  → AuthMethod::Integrated — Kerberos on Linux
    //                  (integrated-auth-gssapi feature) or SSPI on
    //                  Windows (winauth feature). Vanilla builds reject
    //                  `--auth-mode integrated` upstream in main.rs.
    match opts.auth_mode {
        MssqlAuthMode::SqlAuth => {
            config.authentication(AuthMethod::sql_server(&params.user, secret.expose()));
            audit.connection.auth = "sql-auth".to_string();
        }
        MssqlAuthMode::EntraToken => {
            config.authentication(AuthMethod::aad_token(secret.expose()));
            audit.connection.auth = "aad-token".to_string();
        }
        MssqlAuthMode::Integrated => {
            // tiberius's `Integrated` auth uses the OS-level credential
            // cache: a Kerberos TGT (`KRB5CCNAME`, default
            // `/tmp/krb5cc_<uid>`) on Linux via libgssapi, or the
            // current Windows session via SSPI. The customer must run
            // `kinit user@REALM` before calling us. We never read or
            // store the credential ourselves — `secret` is unused on
            // this arm.
            #[cfg(any(
                all(unix, feature = "integrated-auth-gssapi"),
                all(windows, feature = "winauth")
            ))]
            {
                let _ = secret;
                config.authentication(AuthMethod::Integrated);
                audit.connection.password_persisted = false;
                audit.connection.password_logged = false;

                // Platform-specific credential-source recording. On
                // Windows, SSPI consults the LSASS-managed logon
                // session — there is no file we read. On Linux,
                // libgssapi consults a TGT cache file (KRB5CCNAME or
                // the libgssapi default). Be honest about which
                // applies.
                #[cfg(all(windows, feature = "winauth"))]
                {
                    audit.connection.auth = "integrated-sspi".to_string();
                    // No file is read; the Windows session credential
                    // lives in LSASS process memory, accessed via the
                    // SSPI API. This is part of the OS, not something
                    // dbwarp-blueprint touches directly.
                }
                #[cfg(all(unix, feature = "integrated-auth-gssapi"))]
                {
                    audit.connection.auth = "integrated-gssapi".to_string();
                    let krb5_cc = std::env::var("KRB5CCNAME").unwrap_or_else(|_| {
                        "(libgssapi default — see krb5.conf default_ccache_name; usually /tmp/krb5cc_<uid>)"
                            .to_string()
                    });
                    audit.files_read_local.push(format!(
                        "{} (Kerberos TGT cache, read by libgssapi)",
                        krb5_cc
                    ));
                    if std::env::var("KRB5CCNAME").is_ok() {
                        audit.env_vars_read.push("KRB5CCNAME".to_string());
                    }
                }
            }
            #[cfg(not(any(
                all(unix, feature = "integrated-auth-gssapi"),
                all(windows, feature = "winauth")
            )))]
            {
                let _ = secret;
                anyhow::bail!(
                    "MssqlAuthMode::Integrated reached the engine on a build without \
                     integrated-auth-gssapi (Linux) or winauth (Windows). This is a \
                     bug — main.rs's resolve_mssql_auth_mode should have rejected this \
                     mode before dispatch. Rebuild with the appropriate feature."
                );
            }
        }
    }

    // Encryption + trust. Tiberius uses native roots by default. An explicit
    // CA replaces that store, and its rustls connector validates the hostname
    // in both verify-ca and verify-full; the distinction cannot be weakened
    // without replacing the driver verifier.
    match opts.tls.mode {
        TlsMode::Disable => {
            config.encryption(EncryptionLevel::NotSupported);
        }
        TlsMode::Prefer => {
            config.encryption(EncryptionLevel::On);
            if opts.tls.ca_bundle.is_none() && !opts.tls.skip_verify {
                config.trust_cert();
            }
        }
        TlsMode::Require | TlsMode::VerifyCa | TlsMode::VerifyFull => {
            config.encryption(EncryptionLevel::Required);
            if matches!(opts.tls.mode, TlsMode::Require) && opts.tls.ca_bundle.is_none() {
                config.trust_cert();
            }
        }
    }
    if opts.tls.skip_verify {
        config.trust_cert();
    } else if let Some(ca) = &opts.tls.ca_bundle {
        config.trust_cert_ca(ca.to_string_lossy().to_string());
        audit.connection.tls_ca_only = true;
    }

    audit.network_egress.push(format!(
        "{}:{} (database-driver session; DNS may use the configured resolver)",
        params.host, params.port
    ));

    let connect_started = Instant::now();
    // Use the (host, port) tuple form rather than `format!("{}:{}", host,
    // port)`. The string form is ambiguous for IPv6 hosts ("::1:1433" vs
    // "[::1]:1433") whereas the tuple lets `ToSocketAddrs` resolve the
    // host string deterministically. Bracket-stripped IPv6 strings (e.g.
    // "::1") parse straight into SocketAddr; hostnames go through DNS.
    let tcp = TcpStream::connect((params.host.as_str(), params.port))
        .await
        .with_context(|| format!("TCP connect to {}", params.redacted_uri))?;
    tcp.set_nodelay(true).ok();
    let mut client = Client::connect(config, tcp.compat_write())
        .await
        .with_context(|| format!("tiberius connect to {}", params.redacted_uri))?;
    audit.connection.ssl_negotiated = if opts.tls.mode == TlsMode::Disable {
        "no (plaintext transport)".to_string()
    } else {
        "yes (protocol version unavailable from driver)".to_string()
    };
    let connect_total = connect_started.elapsed();

    // SQL Server exposes a session-local lock-wait timeout but no equivalent
    // session setting for total statement elapsed time. Bound lock stalls on
    // the server and retain the independent client wall deadline for all other
    // waits. We deliberately do not describe this as server-side cancellation.
    let timeout_ms = opts
        .sample_timeout_secs
        .saturating_mul(1000)
        .min(i32::MAX as u64);
    let timeout_started = Instant::now();
    client
        .simple_query(format!("SET LOCK_TIMEOUT {timeout_ms}"))
        .await
        .context("setting SQL Server session LOCK_TIMEOUT")?
        .into_results()
        .await
        .context("completing SQL Server session LOCK_TIMEOUT")?;
    audit.record_query(
        "SET LOCK_TIMEOUT <max-wall-secs> (session-local lock-wait safety limit)",
        elapsed_ms(timeout_started),
        0,
    );

    let expected_principal = opts.expected_server_principal.as_deref();
    match probe_mssql_principals(&mut client, expected_principal, audit).await {
        Ok(evidence) => {
            let assertion = if expected_principal.is_some() {
                if evidence.expected_match {
                    "matched"
                } else {
                    "mismatched"
                }
            } else {
                "not-requested"
            };
            audit.record_database_principals(
                &evidence.authenticated,
                &evidence.effective_server,
                &evidence.database,
                expected_principal,
                assertion,
            );
            if !evidence.expected_match {
                bail!(
                    "DBP1606E the authenticated SQL Server principal did not match --expect-server-principal. Exact observed and expected identities are recorded in the customer-local audit; no catalog capture was attempted."
                );
            }
        }
        Err(error) => {
            audit.connection.expected_server_principal = expected_principal.map(str::to_string);
            audit.connection.principal_assertion = Some("unavailable".to_string());
            if expected_principal.is_some() {
                bail!(
                    "DBP1606E could not verify the authenticated SQL Server principal against --expect-server-principal: {error}"
                );
            }
            let redacted = crate::i18n::format(
                "engine.driver_detail_redacted",
                &[("target", "SQL Server principal probe".to_string())],
            );
            let detail = crate::i18n::format(
                "engine.principal_failed",
                &[("code", "DBP1421W".to_string()), ("error", redacted)],
            );
            tracing_eprintln(detail.clone());
            audit.record_warning("DBP1421W", detail);
        }
    }

    // RTT probe — 5× SELECT 1 for customer-side observed round-trip
    // statistics. Captured BEFORE catalog queries so timings aren't
    // skewed by cache warmup.
    let network_probe = if opts.rtt_probe {
        match probe_rtt(&mut client, audit).await {
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

    // Engine version. SERVERPROPERTY returns sql_variant; cast to nvarchar
    // to avoid tiberius's "not implemented for SSVariant" panic.
    let started = Instant::now();
    let row = client
        .simple_query("SELECT CAST(SERVERPROPERTY('ProductVersion') AS NVARCHAR(128)) AS v")
        .await?
        .into_row()
        .await?
        .ok_or_else(|| anyhow!("ServerProperty returned no row"))?;
    let v: Option<&str> = row.get("v");
    let engine_version = v.unwrap_or("unknown").to_string();
    audit.record_query(
        "SELECT SERVERPROPERTY(...) AS NVARCHAR",
        elapsed_ms(started),
        1,
    );

    let schemas = resolve_mssql_schemas(&mut client, &opts.schemas, audit).await?;

    let topology_evidence = probe_mssql_topology(&mut client, &schemas, audit).await;
    let mut sizing = classify_mssql_topology(&topology_evidence);
    schemas.qualify_dataset_scope(&mut sizing.dataset_scope);
    crate::topology::sort_topology(&mut sizing.topology);
    crate::topology::sort_dedup(&mut sizing.dataset_scope.limitations);
    crate::topology::warn_incomplete_dataset_scope(&sizing.dataset_scope, audit);

    // Tables + sizes — sys.tables joined with dm_db_partition_stats /
    // sys.allocation_units to split heap-vs-secondary-index pages.
    // index_id IN (0,1) = heap or clustered index leaf = "table" data;
    // index_id  > 1     = nonclustered indexes = "index" data.
    let mut tables_in = list_tables(&mut client, audit).await?;
    tables_in.retain(|table| schemas.includes(&table.schema_name));
    let cols_in = list_columns(&mut client, audit).await?;
    let idx_in = list_indexes(&mut client, audit).await?;
    let fks_in = list_foreign_keys(&mut client, audit).await?;

    // Anonymize tables.
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

    // Group indexes by (schema, table, index_name).
    let mut idx_groups: BTreeMap<(String, String, String), Vec<IndexCol>> = BTreeMap::new();
    for r in idx_in {
        idx_groups
            .entry((
                r.schema_name.clone(),
                r.table_name.clone(),
                r.index_name.clone(),
            ))
            .or_default()
            .push(IndexCol {
                seq: r.seq,
                col_name: r.col_name,
                included: r.included,
                descending: r.descending,
                filtered: r.filtered,
                primary: r.primary,
                unique: r.unique,
                method: r.method,
            });
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
                    if t.row_count == 0 {
                        // sys.partitions.rows is an engine-maintained counter,
                        // unlike planner estimates that may use zero for
                        // unknown. It is safe to avoid all row/style reads.
                        audit.record_proven_empty_table_skipped();
                        continue;
                    }
                    let qual = (t.schema_name.clone(), t.table_name.clone());
                    let table_id = id_by_qual
                        .get(&qual)
                        .map(String::as_str)
                        .unwrap_or("table-unknown");
                    let cols_for_table: &[ColumnRow] =
                        cols_by_qual.get(&qual).map(Vec::as_slice).unwrap_or(&[]);
                    match sample_compression(
                        &mut client,
                        t,
                        table_id,
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
                            pending_samples.push((qual.clone(), table_id.to_string(), pending));
                        }
                        Ok(None) => { /* table became empty after catalog read */ }
                        Err(_) => warn_compression_unavailable(table_id, audit),
                    }
                    if let Some(cols) = cols_by_qual.get(&qual) {
                        for (column_index, c) in cols.iter().enumerate() {
                            if is_style_candidate_mssql(c) {
                                match peek_column_style_mssql(&mut client, t, c).await {
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
        let col_to_ord: BTreeMap<String, u32> = match cols_by_qual.get(&qual) {
            Some(cs) => cs
                .iter()
                .map(|c| (c.col_name.to_ascii_lowercase(), c.ordinal))
                .collect(),
            None => BTreeMap::new(),
        };
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
                let (len_avg, len_p95) = if is_variable_length_mssql(&c.native_type) {
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
                        declared_max_chars: c.declared_max_chars,
                        declared_max_bytes: c.declared_max_bytes,
                        numeric_precision: c.numeric_precision,
                        numeric_scale: c.numeric_scale,
                        datetime_precision: c.datetime_precision,
                        charset: c.charset.clone(),
                        collation: c.collation.clone(),
                        len_avg,
                        len_p95,
                        source_semantics: c.source_semantics.clone(),
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

        let mut idxs_for_table: Vec<((String, String, String), Vec<IndexCol>)> = idx_groups
            .iter()
            .filter(|((s, tn, _), _)| s == &t.schema_name && tn == &t.table_name)
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        idxs_for_table.sort_by_key(|((_, _, idx_name), _)| format::index_hash(idx_name));
        let mut idx_map: BTreeMap<String, BlueprintIndex> = BTreeMap::new();
        for (i, ((_, _, _), parts)) in idxs_for_table.iter().enumerate() {
            let mut sorted_parts = parts.clone();
            sorted_parts.sort_by_key(|p| p.seq);
            let primary = sorted_parts.first().map(|p| p.primary).unwrap_or(false);
            let unique = sorted_parts.first().map(|p| p.unique).unwrap_or(false);
            let filtered = sorted_parts.first().map(|p| p.filtered).unwrap_or(false);
            let descending = sorted_parts.iter().any(|p| p.descending);
            let method = sorted_parts
                .first()
                .map(|p| normalized_index_method(&p.method))
                .unwrap_or_else(|| "btree".to_string());
            let col_ords: Vec<u32> = sorted_parts
                .iter()
                .filter(|p| !p.included)
                .filter_map(|p| col_to_ord.get(&p.col_name.to_ascii_lowercase()).copied())
                .collect();
            let include_cols: Vec<u32> = sorted_parts
                .iter()
                .filter(|p| p.included)
                .filter_map(|p| col_to_ord.get(&p.col_name.to_ascii_lowercase()).copied())
                .collect();
            idx_map.insert(
                format::idx_id((i + 1) as u32),
                BlueprintIndex {
                    index_type: method,
                    primary,
                    unique,
                    cols: col_ords,
                    prefix_lengths: Vec::new(),
                    include_cols,
                    expression: false,
                    filtered,
                    descending,
                    ..BlueprintIndex::default()
                },
            );
        }

        let table_blueprint = BlueprintTable {
            rows: format::round_rows(t.row_count),
            table_bytes: format::round_bytes(t.table_bytes),
            index_bytes: format::round_bytes(t.index_bytes),
            schema: schema_anon,
            // SQL Server: table data lives in the clustered index leaf
            // (or heap if no clustered index). The split table_bytes /
            // index_bytes already reflects that.
            has_clustered_index: t.has_clustered_index,
            stats_freshness: String::new(),
            cols: col_map,
            idxs: idx_map,
            compression: compression_sample.map(|sample| sample.table),
            ..BlueprintTable::default()
        };
        accumulate_table_totals(&mut totals, &table_blueprint)?;
        tables_out.insert(tid, table_blueprint);
    }
    totals.table_count = tables_out.len() as u64;

    // FK edges.
    let mut fk_edges: BTreeMap<String, Vec<FkEdge>> = BTreeMap::new();
    let mut fk_groups: BTreeMap<(String, String, String, String, i32), Vec<FkRow>> =
        BTreeMap::new();
    for r in fks_in {
        let key = (
            r.from_schema.clone(),
            r.from_table.clone(),
            r.to_schema.clone(),
            r.to_table.clone(),
            r.constraint_id,
        );
        fk_groups.entry(key).or_default().push(r);
    }
    for ((fs, ft, ts, tt, _constraint_id), mut cols) in fk_groups {
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
            .map(|cs| {
                cs.iter()
                    .map(|c| (c.col_name.to_ascii_lowercase(), c.ordinal))
                    .collect()
            })
            .unwrap_or_default();
        let to_col_to_ord: BTreeMap<String, u32> = cols_by_qual
            .get(&(ts, tt))
            .map(|cs| {
                cs.iter()
                    .map(|c| (c.col_name.to_ascii_lowercase(), c.ordinal))
                    .collect()
            })
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
            match_type: "simple".to_string(),
            deferrable: false,
            initially_deferred: false,
            validated: cols[0].validated,
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
                artifacts::table_identity("sqlserver", schema, table),
                id.clone(),
            )
        })
        .collect();
    let artifact_inventory = if opts.artifact_detail == ArtifactDetail::None {
        None
    } else {
        let (mut raw_artifacts, completeness) = capture_artifacts(
            &mut client,
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
        generated_at: crate::format::generated_at_now(opts.generated_at_pin.as_deref()),
        engine: "sqlserver".to_string(),
        engine_version,
        source_kind: opts.source_kind_str.clone(),
        length_metadata: "hybrid-v2".to_string(),
        declared_length_fidelity: "exact".to_string(),
        index_length_fidelity: "exact".to_string(),
        observed_length_fidelity: if opts.measure_compression {
            "relative-rounded-v2".to_string()
        } else {
            "not-sampled".to_string()
        },
        totals,
        network: network_probe,
        database_topology: Some(sizing.topology),
        dataset_scope: Some(sizing.dataset_scope),
        tables: tables_out,
        fk_edges,
        artifact_inventory,
    };
    crate::statistics::enrich_relational_statistics(&mut blueprint);
    Ok(blueprint)
}

#[derive(Debug, Clone)]
struct TableRow {
    schema_name: String,
    table_name: String,
    row_count: u64,
    table_bytes: u64,
    index_bytes: u64,
    has_clustered_index: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct MssqlTopologyEvidence {
    hadr_capability_readable: bool,
    hadr_enabled: Option<bool>,
    database_replica_catalog_attempted: bool,
    database_replica_catalog_readable: bool,
    database_participates: bool,
    local_role: Option<&'static str>,
    availability_replica_catalog_attempted: bool,
    availability_replica_catalog_readable: bool,
    visible_member_count: u64,
    visible_primary_count: u64,
    visible_secondary_count: u64,
    visible_unknown_count: u64,
    external_table_catalog_readable: bool,
    external_table_count: u64,
}

#[derive(Debug, Clone)]
struct MssqlSizingAssessment {
    topology: format::DatabaseTopology,
    dataset_scope: format::DatasetScope,
}

#[derive(Debug, Clone)]
struct ColumnRow {
    schema_name: String,
    table_name: String,
    col_name: String,
    ordinal: u32,
    col_type: String,
    native_type: String,
    is_nullable: bool,
    declared_max_chars: u64,
    declared_max_bytes: u64,
    numeric_precision: u64,
    numeric_scale: u64,
    datetime_precision: u64,
    charset: String,
    collation: String,
    source_semantics: String,
}

#[derive(Debug, Clone)]
struct IndexRow {
    schema_name: String,
    table_name: String,
    index_name: String,
    method: String,
    primary: bool,
    unique: bool,
    seq: u32,
    col_name: String,
    included: bool,
    descending: bool,
    filtered: bool,
}

#[derive(Debug, Clone)]
struct IndexCol {
    seq: u32,
    col_name: String,
    included: bool,
    descending: bool,
    filtered: bool,
    primary: bool,
    unique: bool,
    method: String,
}

#[derive(Debug, Clone)]
struct FkRow {
    from_schema: String,
    from_table: String,
    to_schema: String,
    to_table: String,
    constraint_id: i32,
    position: i32,
    col_name: String,
    to_col_name: String,
    on_update: String,
    on_delete: String,
    validated: bool,
}

async fn resolve_mssql_schemas(
    client: &mut Client<tokio_util::compat::Compat<TcpStream>>,
    requested: &SchemaSelection,
    audit: &mut AuditLog,
) -> Result<SchemaSelection> {
    if !requested.is_active() {
        return Ok(SchemaSelection::default());
    }
    let sql = format!(
        "SELECT s.name AS schema_name FROM sys.schemas s \
         WHERE s.name NOT IN ('sys','INFORMATION_SCHEMA'){} ORDER BY s.name",
        requested.and_sql("s.name")
    );
    let started = Instant::now();
    let rows = client.simple_query(&sql).await?.into_first_result().await?;
    audit.record_query(
        "SELECT requested schema visibility from sys.schemas (names discarded)",
        elapsed_ms(started),
        rows.len() as u64,
    );
    resolved_selection(
        requested,
        rows.into_iter()
            .map(|row| row.get::<&str, _>("schema_name").unwrap_or("").to_string()),
        true,
    )
}

async fn probe_mssql_topology(
    client: &mut Client<tokio_util::compat::Compat<TcpStream>>,
    schemas: &SchemaSelection,
    audit: &mut AuditLog,
) -> MssqlTopologyEvidence {
    let mut evidence = MssqlTopologyEvidence::default();

    let started = Instant::now();
    let hadr_sql = r#"
        SELECT CONVERT(bigint,
                       COALESCE(CONVERT(int, SERVERPROPERTY('IsHadrEnabled')), 0))
               AS hadr_enabled
    "#;
    match client.simple_query(hadr_sql).await {
        Ok(stream) => match stream.into_row().await {
            Ok(Some(row)) => {
                let enabled: i64 = row.get("hadr_enabled").unwrap_or(0);
                evidence.hadr_capability_readable = true;
                evidence.hadr_enabled = Some(enabled != 0);
                audit.record_query(
                    "SELECT SERVERPROPERTY('IsHadrEnabled') (topology capability; no identifiers)",
                    elapsed_ms(started),
                    1,
                );
            }
            _ => crate::topology::warn_evidence_unavailable(audit, "sqlserver-is-hadr-enabled"),
        },
        Err(_) => crate::topology::warn_evidence_unavailable(audit, "sqlserver-is-hadr-enabled"),
    }

    if evidence.hadr_enabled == Some(true) {
        evidence.database_replica_catalog_attempted = true;
        let started = Instant::now();
        let database_state_sql = r#"
            SELECT COUNT_BIG(*) AS local_database_rows,
                   COALESCE(SUM(CONVERT(bigint,
                       CASE WHEN is_primary_replica = 1 THEN 1 ELSE 0 END)), 0)
                       AS local_primary_rows,
                   COALESCE(SUM(CONVERT(bigint,
                       CASE WHEN is_primary_replica = 0 THEN 1 ELSE 0 END)), 0)
                       AS local_secondary_rows
            FROM sys.dm_hadr_database_replica_states
            WHERE database_id = DB_ID() AND is_local = 1
        "#;
        match client.simple_query(database_state_sql).await {
            Ok(stream) => match stream.into_row().await {
                Ok(Some(row)) => {
                    let local_rows: i64 = row.get("local_database_rows").unwrap_or(0);
                    let primary_rows: i64 = row.get("local_primary_rows").unwrap_or(0);
                    let secondary_rows: i64 = row.get("local_secondary_rows").unwrap_or(0);
                    evidence.database_replica_catalog_readable = true;
                    evidence.database_participates = local_rows > 0;
                    evidence.local_role = if primary_rows > 0 {
                        Some("primary")
                    } else if secondary_rows > 0 {
                        Some("secondary")
                    } else if local_rows > 0 {
                        Some("unknown")
                    } else {
                        None
                    };
                    audit.record_query(
                        "SELECT current-database HADR participation and local role counts from sys.dm_hadr_database_replica_states (no identifiers)",
                        elapsed_ms(started),
                        1,
                    );
                }
                _ => crate::topology::warn_evidence_unavailable(
                    audit,
                    "sqlserver-database-replica-states",
                ),
            },
            Err(_) => crate::topology::warn_evidence_unavailable(
                audit,
                "sqlserver-database-replica-states",
            ),
        }

        if evidence.database_participates {
            evidence.availability_replica_catalog_attempted = true;
            let started = Instant::now();
            let replica_state_sql = r#"
                WITH local_group AS (
                    SELECT TOP (1) group_id
                    FROM sys.dm_hadr_database_replica_states
                    WHERE database_id = DB_ID() AND is_local = 1
                )
                SELECT COUNT_BIG(*) AS visible_members,
                       COALESCE(SUM(CONVERT(bigint,
                           CASE WHEN role = 1 THEN 1 ELSE 0 END)), 0)
                           AS visible_primaries,
                       COALESCE(SUM(CONVERT(bigint,
                           CASE WHEN role = 2 THEN 1 ELSE 0 END)), 0)
                           AS visible_secondaries,
                       COALESCE(SUM(CONVERT(bigint,
                           CASE WHEN role IS NULL OR role NOT IN (1, 2)
                                THEN 1 ELSE 0 END)), 0)
                           AS visible_unknown
                FROM sys.dm_hadr_availability_replica_states
                WHERE group_id = (SELECT group_id FROM local_group)
            "#;
            match client.simple_query(replica_state_sql).await {
                Ok(stream) => match stream.into_row().await {
                    Ok(Some(row)) => {
                        let nonnegative = |value: i64| value.max(0) as u64;
                        evidence.availability_replica_catalog_readable = true;
                        evidence.visible_member_count =
                            nonnegative(row.get("visible_members").unwrap_or(0));
                        evidence.visible_primary_count =
                            nonnegative(row.get("visible_primaries").unwrap_or(0));
                        evidence.visible_secondary_count =
                            nonnegative(row.get("visible_secondaries").unwrap_or(0));
                        evidence.visible_unknown_count =
                            nonnegative(row.get("visible_unknown").unwrap_or(0));
                        audit.record_query(
                            "SELECT current availability-group member/role counts from sys.dm_hadr_availability_replica_states (no identifiers)",
                            elapsed_ms(started),
                            1,
                        );
                    }
                    _ => crate::topology::warn_evidence_unavailable(
                        audit,
                        "sqlserver-hadr-replica-states",
                    ),
                },
                Err(_) => crate::topology::warn_evidence_unavailable(
                    audit,
                    "sqlserver-hadr-replica-states",
                ),
            }
        }
    }

    let started = Instant::now();
    let external_sql = format!(
        "SELECT COUNT_BIG(*) AS external_table_count \
         FROM sys.external_tables et \
         JOIN sys.schemas s ON s.schema_id = et.schema_id \
         WHERE 1 = 1{}",
        schemas.and_sql("s.name")
    );
    match client.simple_query(&external_sql).await {
        Ok(stream) => match stream.into_row().await {
            Ok(Some(row)) => {
                let count: i64 = row.get("external_table_count").unwrap_or(0);
                evidence.external_table_catalog_readable = true;
                evidence.external_table_count = count.max(0) as u64;
                audit.record_query(
                    "SELECT visible external-table count FROM sys.external_tables (no identifiers or endpoints)",
                    elapsed_ms(started),
                    1,
                );
            }
            _ => crate::topology::warn_evidence_unavailable(audit, "sqlserver-external-tables"),
        },
        Err(_) => crate::topology::warn_evidence_unavailable(audit, "sqlserver-external-tables"),
    }

    evidence
}

fn classify_mssql_topology(evidence: &MssqlTopologyEvidence) -> MssqlSizingAssessment {
    let mut topology = format::DatabaseTopology {
        contract: dbwarp_blueprint_core::TOPOLOGY_CONTRACT.to_string(),
        deployment: "unknown".to_string(),
        local_role: "unknown".to_string(),
        visibility: if evidence.hadr_capability_readable {
            "partial"
        } else {
            "unknown"
        }
        .to_string(),
        member_count: 1,
        identifiers_redacted: true,
        role_counts: BTreeMap::from([("unknown".to_string(), 1)]),
        features: Vec::new(),
        catalogs_read: Vec::new(),
        catalogs_unreadable: Vec::new(),
    };

    if evidence.hadr_capability_readable {
        topology
            .catalogs_read
            .push("sqlserver-is-hadr-enabled".to_string());
    } else {
        topology
            .catalogs_unreadable
            .push("sqlserver-is-hadr-enabled".to_string());
    }
    if evidence.database_replica_catalog_attempted {
        if evidence.database_replica_catalog_readable {
            topology
                .catalogs_read
                .push("sqlserver-database-replica-states".to_string());
        } else {
            topology
                .catalogs_unreadable
                .push("sqlserver-database-replica-states".to_string());
        }
    }

    if evidence.database_participates {
        topology.deployment = "replicated".to_string();
        topology.local_role = evidence.local_role.unwrap_or("unknown").to_string();
        topology.role_counts = BTreeMap::from([(topology.local_role.clone(), 1)]);
        topology
            .features
            .push("sqlserver-availability-group".to_string());
    }

    if evidence.availability_replica_catalog_attempted {
        if evidence.availability_replica_catalog_readable {
            topology
                .catalogs_read
                .push("sqlserver-hadr-replica-states".to_string());
            topology.member_count = evidence.visible_member_count.max(1);
            topology.role_counts.clear();
            if evidence.visible_primary_count > 0 {
                topology
                    .role_counts
                    .insert("primary".to_string(), evidence.visible_primary_count);
            }
            if evidence.visible_secondary_count > 0 {
                topology
                    .role_counts
                    .insert("secondary".to_string(), evidence.visible_secondary_count);
            }
            if evidence.visible_unknown_count > 0 {
                topology
                    .role_counts
                    .insert("unknown".to_string(), evidence.visible_unknown_count);
            }
            let classified = evidence
                .visible_primary_count
                .saturating_add(evidence.visible_secondary_count)
                .saturating_add(evidence.visible_unknown_count);
            if classified < topology.member_count {
                *topology
                    .role_counts
                    .entry("unknown".to_string())
                    .or_insert(0) += topology.member_count - classified;
            }
            // Microsoft documents full peer visibility only when this DMV is
            // queried on the primary replica. A secondary exposes local state.
            if topology.local_role == "primary" && topology.catalogs_unreadable.is_empty() {
                topology.visibility = "full".to_string();
            }
        } else {
            topology
                .catalogs_unreadable
                .push("sqlserver-hadr-replica-states".to_string());
        }
    }

    let mut limitations = vec![match topology.visibility.as_str() {
        "unknown" => "topology-visibility-unknown",
        _ => "topology-visibility-partial",
    }
    .to_string()];
    if topology.visibility == "full" {
        limitations.clear();
    } else if evidence.database_participates {
        limitations.push("replica-membership-unresolved".to_string());
    }

    let external_data_unmeasured =
        evidence.external_table_catalog_readable && evidence.external_table_count > 0;
    if external_data_unmeasured {
        limitations.push("external-data-unmeasured".to_string());
    } else if !evidence.external_table_catalog_readable {
        limitations.push("external-table-visibility-unknown".to_string());
    }
    crate::topology::sort_dedup(&mut limitations);

    let external_scope_complete =
        evidence.external_table_catalog_readable && evidence.external_table_count == 0;
    MssqlSizingAssessment {
        topology,
        dataset_scope: format::DatasetScope {
            contract: dbwarp_blueprint_core::DATASET_SCOPE_CONTRACT.to_string(),
            layout: "full-copy".to_string(),
            table_inventory_completeness: if external_scope_complete {
                "complete"
            } else {
                "incomplete"
            }
            .to_string(),
            row_count_completeness: if external_scope_complete {
                "complete"
            } else {
                "incomplete"
            }
            .to_string(),
            size_completeness: if external_scope_complete {
                "complete"
            } else {
                "incomplete"
            }
            .to_string(),
            row_count_method: "sqlserver-partition-counter".to_string(),
            size_method: "sqlserver-partition-pages".to_string(),
            limitations,
        },
    }
}

async fn list_tables(
    client: &mut Client<tokio_util::compat::Compat<TcpStream>>,
    audit: &mut AuditLog,
) -> Result<Vec<TableRow>> {
    let sql = r#"
        SELECT
            SCHEMA_NAME(t.schema_id) AS schema_name,
            t.name                   AS table_name,
            COALESCE(SUM(CASE WHEN p.index_id IN (0,1) THEN p.row_count ELSE 0 END), 0) AS row_count,
            COALESCE(SUM(CASE WHEN p.index_id IN (0,1) THEN p.used_page_count ELSE 0 END) * 8 * 1024, 0) AS table_bytes,
            COALESCE(SUM(CASE WHEN p.index_id  > 1     THEN p.used_page_count ELSE 0 END) * 8 * 1024, 0) AS index_bytes,
            CAST(MAX(CASE WHEN p.index_id = 1 THEN 1 ELSE 0 END) AS BIT) AS has_clustered_index
        FROM sys.tables t
        LEFT JOIN sys.dm_db_partition_stats p ON p.object_id = t.object_id
        WHERE t.is_ms_shipped = 0
        GROUP BY t.object_id, t.schema_id, t.name
        ORDER BY schema_name, table_name
    "#;
    let started = Instant::now();
    let stream = client.simple_query(sql).await?;
    let rows = stream.into_first_result().await?;
    audit.record_query(
        "SELECT FROM sys.tables JOIN dm_db_partition_stats (table list)",
        elapsed_ms(started),
        rows.len() as u64,
    );
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let schema_name: &str = r.get("schema_name").unwrap_or("");
        let table_name: &str = r.get("table_name").unwrap_or("");
        let row_count: i64 = r.get("row_count").unwrap_or(0);
        let table_bytes: i64 = r.get("table_bytes").unwrap_or(0);
        let index_bytes: i64 = r.get("index_bytes").unwrap_or(0);
        let has_clust: bool = r.get("has_clustered_index").unwrap_or(false);
        out.push(TableRow {
            schema_name: schema_name.to_string(),
            table_name: table_name.to_string(),
            row_count: row_count.max(0) as u64,
            table_bytes: table_bytes.max(0) as u64,
            index_bytes: index_bytes.max(0) as u64,
            has_clustered_index: has_clust,
        });
    }
    Ok(out)
}

async fn list_columns(
    client: &mut Client<tokio_util::compat::Compat<TcpStream>>,
    audit: &mut AuditLog,
) -> Result<Vec<ColumnRow>> {
    let sql = r#"
        SELECT
            SCHEMA_NAME(t.schema_id) AS schema_name,
            t.name                   AS table_name,
            c.name                   AS col_name,
            c.column_id              AS ordinal,
            CASE WHEN ty.is_user_defined = 1 THEN N'user-defined' ELSE ty.name END AS type_name,
            c.max_length             AS max_length,
            c.precision              AS [precision],
            c.scale                  AS scale,
            c.collation_name         AS collation_name,
            c.is_nullable            AS is_nullable
        FROM sys.tables t
        JOIN sys.columns c ON c.object_id = t.object_id
        JOIN sys.types ty ON ty.user_type_id = c.user_type_id
        WHERE t.is_ms_shipped = 0
        ORDER BY schema_name, table_name, c.column_id
    "#;
    let started = Instant::now();
    let stream = client.simple_query(sql).await?;
    let rows = stream.into_first_result().await?;
    audit.record_query(
        "SELECT FROM sys.columns JOIN sys.types (column list)",
        elapsed_ms(started),
        rows.len() as u64,
    );
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let schema_name: &str = r.get("schema_name").unwrap_or("");
        let table_name: &str = r.get("table_name").unwrap_or("");
        let col_name: &str = r.get("col_name").unwrap_or("");
        let ordinal: i32 = r.get("ordinal").unwrap_or(0);
        let type_name: &str = r.get("type_name").unwrap_or("");
        let max_length: i16 = r.get("max_length").unwrap_or(0);
        let precision: u8 = r.get("precision").unwrap_or(0);
        let scale: u8 = r.get("scale").unwrap_or(0);
        let collation: &str = r.get("collation_name").unwrap_or("");
        let is_nullable: bool = r.get("is_nullable").unwrap_or(true);
        // Format the type string with size/precision/scale where relevant.
        let col_type = format_mssql_type(type_name, max_length, precision, scale);
        let native_type = type_name.to_ascii_lowercase();
        let (declared_max_chars, declared_max_bytes, source_semantics) =
            mssql_length_metadata(&native_type, max_length);
        let datetime_precision = if matches!(
            native_type.as_str(),
            "time" | "datetime2" | "datetimeoffset"
        ) {
            u64::from(scale)
        } else {
            0
        };
        out.push(ColumnRow {
            schema_name: schema_name.to_string(),
            table_name: table_name.to_string(),
            col_name: col_name.to_string(),
            ordinal: ordinal.max(0) as u32,
            col_type,
            native_type: native_type.clone(),
            is_nullable,
            declared_max_chars,
            declared_max_bytes,
            numeric_precision: u64::from(precision),
            numeric_scale: u64::from(scale),
            datetime_precision,
            charset: if matches!(native_type.as_str(), "nvarchar" | "nchar" | "ntext") {
                "utf-16le".to_string()
            } else {
                String::new()
            },
            collation: collation.to_string(),
            source_semantics,
        });
    }
    Ok(out)
}

fn mssql_length_metadata(name: &str, max_length: i16) -> (u64, u64, String) {
    if max_length < 0 || matches!(name, "text" | "ntext" | "image" | "xml") {
        return (0, 0, "unbounded-lob".to_string());
    }
    let bytes = max_length as u64;
    let chars = match name {
        "nvarchar" | "nchar" => bytes / 2,
        "varchar" | "char" => bytes,
        _ => 0,
    };
    (chars, bytes, String::new())
}

fn format_mssql_type(name: &str, max_length: i16, precision: u8, scale: u8) -> String {
    let n = name.to_ascii_lowercase();
    match n.as_str() {
        "user-defined" => "user-defined".to_string(),
        "bigint" | "int" | "smallint" | "tinyint" => "integer".to_string(),
        "float" | "real" => "float".to_string(),
        "bit" => "boolean".to_string(),
        "varchar" | "char" | "varbinary" | "binary" => {
            if n == "varbinary" || n == "binary" {
                return "binary".to_string();
            }
            if max_length == -1 {
                "text".to_string()
            } else {
                "text".to_string()
            }
        }
        "nvarchar" | "nchar" | "text" | "ntext" | "xml" => "text".to_string(),
        "uniqueidentifier" => "uuid".to_string(),
        "date" => "date".to_string(),
        "datetime" | "datetime2" | "smalldatetime" | "datetimeoffset" => "timestamp".to_string(),
        "time" => "time".to_string(),
        "image" | "geography" | "geometry" | "hierarchyid" | "rowversion" | "timestamp" => {
            "binary".to_string()
        }
        "decimal" | "numeric" => format!("{n}({precision},{scale})"),
        "money" | "smallmoney" => "numeric".to_string(),
        _ => "user-defined".to_string(),
    }
}

fn normalized_index_method(method: &str) -> String {
    match method.trim().to_ascii_lowercase().as_str() {
        "heap"
        | "clustered"
        | "nonclustered"
        | "xml"
        | "spatial"
        | "clustered columnstore"
        | "nonclustered columnstore"
        | "hash" => method.trim().to_ascii_lowercase(),
        _ => "other".to_string(),
    }
}

async fn list_indexes(
    client: &mut Client<tokio_util::compat::Compat<TcpStream>>,
    audit: &mut AuditLog,
) -> Result<Vec<IndexRow>> {
    let sql = r#"
        SELECT
            SCHEMA_NAME(t.schema_id) AS schema_name,
            t.name                   AS table_name,
            i.name                   AS index_name,
            i.type_desc              AS index_type,
            i.is_primary_key         AS is_primary,
            i.is_unique              AS is_unique,
            i.has_filter             AS has_filter,
            ic.key_ordinal           AS seq,
            ic.is_included_column    AS is_included,
            ic.is_descending_key     AS is_descending,
            c.name                   AS col_name
        FROM sys.tables t
        JOIN sys.indexes i ON i.object_id = t.object_id
        JOIN sys.index_columns ic ON ic.object_id = i.object_id AND ic.index_id = i.index_id
        JOIN sys.columns c ON c.object_id = ic.object_id AND c.column_id = ic.column_id
        WHERE t.is_ms_shipped = 0
          AND i.index_id > 0   -- skip heap pseudo-index
          AND i.name IS NOT NULL
        ORDER BY schema_name, table_name, i.name, ic.is_included_column, ic.key_ordinal, ic.index_column_id
    "#;
    let started = Instant::now();
    let stream = client.simple_query(sql).await?;
    let rows = stream.into_first_result().await?;
    audit.record_query(
        "SELECT FROM sys.indexes JOIN sys.index_columns JOIN sys.columns (index list)",
        elapsed_ms(started),
        rows.len() as u64,
    );
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let schema_name: &str = r.get("schema_name").unwrap_or("");
        let table_name: &str = r.get("table_name").unwrap_or("");
        let index_name: &str = r.get("index_name").unwrap_or("");
        let method: &str = r.get("index_type").unwrap_or("BTREE");
        let primary: bool = r.get("is_primary").unwrap_or(false);
        let unique: bool = r.get("is_unique").unwrap_or(false);
        let filtered: bool = r.get("has_filter").unwrap_or(false);
        let seq: u8 = r.get("seq").unwrap_or(0);
        let included: bool = r.get("is_included").unwrap_or(false);
        let descending: bool = r.get("is_descending").unwrap_or(false);
        let col_name: &str = r.get("col_name").unwrap_or("");
        out.push(IndexRow {
            schema_name: schema_name.to_string(),
            table_name: table_name.to_string(),
            index_name: index_name.to_string(),
            method: method.to_string(),
            primary,
            unique,
            seq: seq as u32,
            col_name: col_name.to_string(),
            included,
            descending,
            filtered,
        });
    }
    Ok(out)
}

async fn list_foreign_keys(
    client: &mut Client<tokio_util::compat::Compat<TcpStream>>,
    audit: &mut AuditLog,
) -> Result<Vec<FkRow>> {
    let sql = r#"
        SELECT
            SCHEMA_NAME(ft.schema_id)        AS from_schema,
            ft.name                          AS from_table,
            SCHEMA_NAME(rt.schema_id)        AS to_schema,
            rt.name                          AS to_table,
            fk.object_id                     AS constraint_id,
            c.name                           AS col_name,
            rc.name                          AS to_col_name,
            fc.constraint_column_id          AS pos,
            fk.update_referential_action_desc AS update_action,
            fk.delete_referential_action_desc AS delete_action,
            fk.is_not_trusted                 AS is_not_trusted,
            fk.is_disabled                    AS is_disabled
        FROM sys.foreign_keys fk
        JOIN sys.foreign_key_columns fc ON fc.constraint_object_id = fk.object_id
        JOIN sys.tables ft ON ft.object_id = fk.parent_object_id
        JOIN sys.tables rt ON rt.object_id = fk.referenced_object_id
        JOIN sys.columns c ON c.object_id = fc.parent_object_id AND c.column_id = fc.parent_column_id
        JOIN sys.columns rc ON rc.object_id = fc.referenced_object_id AND rc.column_id = fc.referenced_column_id
        WHERE ft.is_ms_shipped = 0
        ORDER BY from_schema, from_table, fk.object_id, pos
    "#;
    let started = Instant::now();
    let stream = client.simple_query(sql).await?;
    let rows = stream.into_first_result().await?;
    audit.record_query(
        "SELECT FROM sys.foreign_keys JOIN sys.foreign_key_columns (FK list)",
        elapsed_ms(started),
        rows.len() as u64,
    );
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let from_schema: &str = r.get("from_schema").unwrap_or("");
        let from_table: &str = r.get("from_table").unwrap_or("");
        let to_schema: &str = r.get("to_schema").unwrap_or("");
        let to_table: &str = r.get("to_table").unwrap_or("");
        let constraint_id: i32 = r.get("constraint_id").unwrap_or(0);
        let position: i32 = r.get("pos").unwrap_or(0);
        let col_name: &str = r.get("col_name").unwrap_or("");
        let to_col_name: &str = r.get("to_col_name").unwrap_or("");
        let update_action: &str = r.get("update_action").unwrap_or("NO_ACTION");
        let delete_action: &str = r.get("delete_action").unwrap_or("NO_ACTION");
        let is_not_trusted: bool = r.get("is_not_trusted").unwrap_or(false);
        let is_disabled: bool = r.get("is_disabled").unwrap_or(false);
        out.push(FkRow {
            from_schema: from_schema.to_string(),
            from_table: from_table.to_string(),
            to_schema: to_schema.to_string(),
            to_table: to_table.to_string(),
            constraint_id,
            position,
            col_name: col_name.to_string(),
            to_col_name: to_col_name.to_string(),
            on_update: normalize_mssql_fk_action(update_action),
            on_delete: normalize_mssql_fk_action(delete_action),
            validated: !is_not_trusted && !is_disabled,
        });
    }
    Ok(out)
}

fn normalize_mssql_fk_action(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace('_', "-")
}

/// Render a single MSSQL ColumnData as (TypeTag, payload bytes) for
/// the row-frame encoder. The key load-bearing decision is for
/// `String` values: nvarchar/nchar/ntext columns are stored on the
/// wire (and on disk) as UTF-16LE, so we re-encode the Rust String
/// back to UTF-16LE before tagging — that preserves the byte-doubling
/// distribution that drives the (significantly higher) zstd ratios
/// MSSQL nvarchar exhibits in production.
///
/// (Var)Char columns use the database collation — typically a
/// single-byte SBCS like CP1252, but tiberius converts to UTF-8 on
/// receive. We tag those `TextUtf8` and accept that ratios may differ
/// slightly from the actual SBCS wire bytes; for ASCII-heavy content
/// the entropy and compression behavior of UTF-8 vs SBCS are very
/// similar.
fn encode_mssql_cell(col_type: ColumnType, data: &ColumnData<'_>) -> (TypeTag, Vec<u8>) {
    match data {
        ColumnData::U8(None)
        | ColumnData::I16(None)
        | ColumnData::I32(None)
        | ColumnData::I64(None)
        | ColumnData::F32(None)
        | ColumnData::F64(None)
        | ColumnData::Bit(None)
        | ColumnData::String(None)
        | ColumnData::Guid(None)
        | ColumnData::Binary(None)
        | ColumnData::Numeric(None)
        | ColumnData::Xml(None)
        | ColumnData::DateTime(None)
        | ColumnData::SmallDateTime(None)
        | ColumnData::Time(None)
        | ColumnData::Date(None)
        | ColumnData::DateTime2(None)
        | ColumnData::DateTimeOffset(None) => (TypeTag::Null, Vec::new()),
        ColumnData::U8(Some(v)) => (TypeTag::NumberText, v.to_string().into_bytes()),
        ColumnData::I16(Some(v)) => (TypeTag::NumberText, v.to_string().into_bytes()),
        ColumnData::I32(Some(v)) => (TypeTag::NumberText, v.to_string().into_bytes()),
        ColumnData::I64(Some(v)) => (TypeTag::NumberText, v.to_string().into_bytes()),
        ColumnData::F32(Some(v)) => (TypeTag::NumberText, format!("{v}").into_bytes()),
        ColumnData::F64(Some(v)) => (TypeTag::NumberText, format!("{v}").into_bytes()),
        ColumnData::Bit(Some(v)) => (
            TypeTag::BoolText,
            if *v { b"1".to_vec() } else { b"0".to_vec() },
        ),
        ColumnData::Numeric(Some(n)) => {
            // tiberius's `Display` for Numeric falls back to `Debug` =
            // "Numeric { value: 12345, scale: 2 }" — that constant
            // prefix repeats per row and is *extremely* zstd-compressible,
            // structurally inflating compression ratios for any table
            // with NUMERIC/DECIMAL columns. Format the value/scale
            // manually as canonical decimal text instead.
            (TypeTag::NumberText, format_tiberius_numeric(n).into_bytes())
        }
        ColumnData::Guid(Some(g)) => (TypeTag::UuidText, g.to_string().into_bytes()),
        ColumnData::Binary(Some(b)) => (TypeTag::BinaryRaw, b.to_vec()),
        ColumnData::Xml(Some(x)) => (TypeTag::TextUtf8, format!("{x:?}").into_bytes()),
        ColumnData::String(Some(s)) => {
            let is_unicode = matches!(
                col_type,
                ColumnType::NVarchar | ColumnType::NChar | ColumnType::NText
            );
            if is_unicode {
                let bytes: Vec<u8> = s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
                (TypeTag::TextUtf16Le, bytes)
            } else {
                (TypeTag::TextUtf8, s.as_bytes().to_vec())
            }
        }
        // DateTime variants: tiberius's Display impl falls back to Debug
        // for these too, producing repetitive `DateTime { days: ...,
        // seconds_fragments: ... }` strings that compress as constants.
        // Format as a stable iso-ish text using the public accessors.
        ColumnData::DateTime(Some(d)) => (
            TypeTag::TimestampText,
            format!("days={};frag={}", d.days(), d.seconds_fragments()).into_bytes(),
        ),
        ColumnData::SmallDateTime(Some(d)) => (
            TypeTag::TimestampText,
            format!("days={};frag={}", d.days(), d.seconds_fragments()).into_bytes(),
        ),
        ColumnData::Time(Some(t)) => (
            TypeTag::TimeText,
            format!("inc={};scale={}", t.increments(), t.scale()).into_bytes(),
        ),
        ColumnData::Date(Some(d)) => (TypeTag::DateText, format!("days={}", d.days()).into_bytes()),
        ColumnData::DateTime2(Some(d)) => (
            TypeTag::TimestampText,
            format!(
                "days={};inc={};scale={}",
                d.date().days(),
                d.time().increments(),
                d.time().scale()
            )
            .into_bytes(),
        ),
        ColumnData::DateTimeOffset(Some(d)) => (
            TypeTag::TimestampText,
            format!(
                "days={};inc={};scale={};off={}",
                d.datetime2().date().days(),
                d.datetime2().time().increments(),
                d.datetime2().time().scale(),
                d.offset()
            )
            .into_bytes(),
        ),
    }
}

/// Format a tiberius Numeric (value: i128, scale: u8) as canonical
/// decimal text — e.g. `Numeric { value: 12345, scale: 2 }` →
/// "123.45". Used by the Tier-2 row-frame encoder so NUMERIC/DECIMAL
/// columns contribute realistic byte distributions to the
/// compression sample (the tiberius Display-via-Debug fallback was
/// producing structurally inflated ratios on numeric-heavy tables).
fn format_tiberius_numeric(n: &tiberius::numeric::Numeric) -> String {
    let v = n.value();
    let scale = n.scale() as usize;
    let sign = if v < 0 { "-" } else { "" };
    let abs = v.unsigned_abs();
    let s = abs.to_string();
    if scale == 0 {
        return format!("{sign}{s}");
    }
    if s.len() > scale {
        let split = s.len() - scale;
        format!("{sign}{}.{}", &s[..split], &s[split..])
    } else {
        // Leading zeros: 0.001234 etc.
        format!("{sign}0.{}{}", "0".repeat(scale - s.len()), s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MssqlPrincipalEvidence {
    authenticated: String,
    effective_server: String,
    database: String,
    expected_match: bool,
}

/// Read the identities SQL Server assigned to the established capture
/// session. ORIGINAL_LOGIN() proves who authenticated, SUSER_SNAME() exposes
/// the current server security context, and USER_NAME() exposes the mapped
/// database principal. The optional assertion is evaluated by SQL Server
/// under its own principal-comparison semantics rather than by a client-side
/// case-folding approximation.
async fn probe_mssql_principals(
    client: &mut Client<tokio_util::compat::Compat<TcpStream>>,
    expected_server_principal: Option<&str>,
    audit: &mut AuditLog,
) -> Result<MssqlPrincipalEvidence> {
    const LABEL: &str =
        "SELECT ORIGINAL_LOGIN(), SUSER_SNAME(), USER_NAME() (session identity probe)";
    let started = Instant::now();
    let expected = expected_server_principal.unwrap_or("");
    let assertion_requested = i32::from(expected_server_principal.is_some());
    let result: Result<MssqlPrincipalEvidence> = async {
        let row = client
            .query(
                "SELECT \
                    CONVERT(nvarchar(256), ORIGINAL_LOGIN()) AS authenticated_principal, \
                    CONVERT(nvarchar(256), SUSER_SNAME()) AS effective_server_principal, \
                    CONVERT(nvarchar(256), USER_NAME()) AS database_principal, \
                    CONVERT(int, CASE WHEN @P2 = 0 OR ORIGINAL_LOGIN() = @P1 \
                                      THEN 1 ELSE 0 END) AS expected_match",
                &[&expected, &assertion_requested],
            )
            .await
            .context("querying SQL Server session identities")?
            .into_row()
            .await
            .context("reading SQL Server session identities")?
            .ok_or_else(|| anyhow!("SQL Server session identity query returned no row"))?;
        let authenticated = row
            .get::<&str, _>("authenticated_principal")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("SQL Server returned no authenticated principal"))?
            .to_string();
        let effective_server = row
            .get::<&str, _>("effective_server_principal")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("SQL Server returned no effective server principal"))?
            .to_string();
        let database = row
            .get::<&str, _>("database_principal")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| anyhow!("SQL Server returned no database principal"))?
            .to_string();
        let expected_match = row
            .get::<i32, _>("expected_match")
            .ok_or_else(|| anyhow!("SQL Server returned no principal assertion result"))?
            != 0;
        Ok(MssqlPrincipalEvidence {
            authenticated,
            effective_server,
            database,
            expected_match,
        })
    }
    .await;
    match result {
        Ok(evidence) => {
            audit.record_query(LABEL, elapsed_ms(started), 1);
            Ok(evidence)
        }
        Err(error) => {
            audit.record_query_failure(LABEL, elapsed_ms(started));
            Err(error)
        }
    }
}

/// Run 5× `SELECT 1` round trips and return the median latency in ms.
/// Recorded as one summary entry in the audit log for clarity.
async fn probe_rtt(
    client: &mut Client<tokio_util::compat::Compat<TcpStream>>,
    audit: &mut AuditLog,
) -> Result<(u64, u64)> {
    let total_started = Instant::now();
    let mut samples_us: Vec<u64> = Vec::with_capacity(5);
    for _ in 0..5 {
        let started = Instant::now();
        let _ = client
            .simple_query("SELECT 1")
            .await
            .context("RTT probe SELECT 1")?
            .into_row()
            .await
            .context("RTT probe SELECT 1 row read")?;
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

#[derive(Debug, Clone)]
struct CompressionSample {
    table: BlueprintCompression,
    columns: Vec<Option<BlueprintCompression>>,
    column_lengths: Vec<Option<(u64, u64)>>,
    null_fractions: Vec<Option<f64>>,
    cardinalities: Vec<Option<format::BlueprintCardinality>>,
}

include!("engine_mssql_sampling.rs");
fn tracing_eprintln(msg: String) {
    eprintln!("dbwarp-blueprint: {msg}");
}

include!("engine_mssql_artifacts.rs");
include!("engine_mssql_tests.rs");
