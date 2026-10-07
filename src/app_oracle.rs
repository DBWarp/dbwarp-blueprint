//! Oracle Basic preview entry points.
//!
//! These paths intentionally expose only the Basic mapper through the
//! SQL*Plus direct-service adapter or a checksummed local capture. These paths
//! require the explicit `--acknowledge-oracle-preview` flag.

use super::*;
use crate::oracle_basic::{map_oracle_basic_capture, OracleBasicOptions};
use crate::oracle_offline::{
    read_oracle_basic_stream, write_oracle_basic_stream, OracleBasicOfflineInputKind,
};
use crate::oracle_provider::{
    negotiate_oracle_capabilities, oracle_offline_provider_contract, OracleAuthentication,
    OracleCaptureRequest, OracleCaptureTier, OracleEndpointMode, OracleLocalInput,
    OracleNegotiatedCapabilities, OracleProviderKind, OracleTransport, OracleTrustSource,
    ORACLE_PROVIDER_CONTRACT_VERSION,
};
use crate::oracle_scope::{
    resolve_oracle_owner_scope, OracleOwnerResolutionFailure, OracleOwnerScope,
    OracleOwnerSelection,
};
use crate::oracle_session::{
    run_catalog_capture_for_scope, OracleCatalogCapture, OracleQueryStatus, OracleValue,
};
use crate::oracle_sqlplus::{
    start_and_probe_sqlplus, OracleSqlplusConfig, SQLPLUS_BASIC_MAX_LIMITS,
    SQLPLUS_FORWARDED_LOADER_VARIABLES,
};
use std::collections::BTreeSet;
use std::time::Duration;

#[derive(Debug)]
struct OracleDirectService {
    host: String,
    port: u16,
    service: String,
}

impl OracleDirectService {
    fn parse(uri: &str) -> Result<Self> {
        let rest = uri
            .get(..9)
            .filter(|scheme| scheme.eq_ignore_ascii_case("oracle://"))
            .and_then(|_| uri.get(9..))
            .ok_or_else(|| {
                anyhow!("DBP1012E Oracle preview requires oracle://HOST[:PORT]/SERVICE")
            })?;
        let (authority, service) = crate::uri_authority::split_authority_and_path(rest);
        if authority.contains('@') {
            bail!(
                "DBP1001E refusing Oracle URI user information; pass the principal through --user, --user-env, or --user-file and the password through its protected source"
            );
        }
        if service.is_empty()
            || service.len() > 255
            || service.contains('/')
            || service.contains(['?', '#'])
            || !service
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
        {
            bail!("DBP1012E Oracle service name is empty or contains unsupported characters");
        }
        let (host, port) = crate::uri_authority::split_host_port(authority, 1521)
            .context("DBP1012E parsing Oracle direct-service host and port")?;
        if host.len() > 255
            || host
                .chars()
                .any(|ch| ch.is_control() || matches!(ch, '/' | '?' | '#' | '@'))
        {
            bail!("DBP1012E Oracle host is empty or contains unsupported characters");
        }
        Ok(Self {
            host,
            port,
            service: service.to_string(),
        })
    }

    fn connect_identifier(&self) -> String {
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        format!("//{host}:{}/{}", self.port, self.service)
    }

    fn redacted_uri(&self) -> String {
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        format!("oracle://{host}:{}/{}", self.port, self.service)
    }
}

pub(super) fn is_oracle_connect(uri: &str) -> bool {
    uri.get(..9)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("oracle://"))
}

pub(super) fn run_oracle_live(cli: &Cli, audit: &mut AuditLog, uri: &str) -> Result<()> {
    require_preview_acknowledgement(cli)?;
    // Reject embedded credentials before evaluating unrelated transport or
    // sampling options so a malformed URI cannot be masked by a later option check.
    let endpoint = OracleDirectService::parse(uri)?;
    validate_oracle_common_options(cli, false)?;
    let source_kind =
        SourceKind::parse(&cli.source_kind).context("DBP1013E validating --source-kind")?;
    audit.mode = "oracle-basic-sqlplus-preview".to_string();
    if let Some(path) = cli.oracle_basic_capture_out.as_deref() {
        validate_oracle_stream_path(cli, path, "DBP1426E")?;
    }
    let executable = cli.oracle_sqlplus.as_deref().ok_or_else(|| {
        anyhow!("DBP1426E Oracle SQL*Plus preview requires --oracle-sqlplus PATH")
    })?;
    if !executable.is_absolute() {
        bail!("DBP1426E --oracle-sqlplus must be an absolute path");
    }
    let network = cli.oracle_network_config_dir.as_deref().ok_or_else(|| {
        anyhow!("DBP1426E Oracle SQL*Plus preview requires --oracle-network-config-dir PATH")
    })?;
    if !network.is_absolute() {
        bail!("DBP1426E --oracle-network-config-dir must be an absolute path");
    }

    let (preview_user, preview_source) = preview_user(cli, "", false, false)?;
    if preview_user.is_empty() {
        bail!("DBP1603E Oracle preview requires --user, --user-env, or --user-file");
    }
    audit.connection.user_source = Some(preview_source.to_string());
    audit.connection.uri_redacted = format!("(not connected) {}", endpoint.redacted_uri());
    audit.connection.auth = "password".to_string();
    audit.connection.tls_mode = "plain-tcp (Oracle preview)".to_string();
    audit.artifact_detail = Some(cli.artifact_detail.as_str().to_string());
    audit.schema_selector_count = cli.schema.len() as u64;
    audit
        .trust_assertions
        .push("oracle_preview_acknowledged=true".to_string());

    if cli.dry_run {
        let preview = describe_secret_source(cli, false);
        audit.record_password_source(&preview);
        audit
            .network_egress
            .push("none (dry-run; no connection attempted)".to_string());
        print_oracle_preflight(cli, &endpoint, preview_source, &preview);
        eprintln!("{}", i18n::text("dry.live"));
        return Ok(());
    }

    if !cli.yes {
        let preview = describe_secret_source(cli, false);
        print_oracle_preflight(cli, &endpoint, preview_source, &preview);
        if !confirm_yes()? {
            bail!("DBP1701E aborted (no consent)");
        }
    }

    configure_anonymization_key(cli, audit)?;
    let blueprint_generated_at = format::generated_at_now(cli.generated_at.as_deref());
    let (username, user_source) = resolve_user(cli, "", false, audit)?;
    if username.is_empty() {
        bail!("DBP1603E Oracle preview requires a non-empty database principal");
    }
    audit.connection.user_source = Some(user_source.to_string());
    let (secret, secret_source) = acquire_secret(cli, None, audit)
        .context("DBP1601E acquiring the Oracle preview credential")?;
    audit.record_password_source(&secret_source);
    audit.record_file_read(&executable.display().to_string());
    audit.network_egress.push(format!(
        "Oracle direct TCP to {}:{} (SQL*Plus preview)",
        endpoint.host, endpoint.port
    ));

    // The executable fingerprint does not cover the client libraries that
    // these forwarded loader variables select, so record which were present.
    for variable in SQLPLUS_FORWARDED_LOADER_VARIABLES {
        if std::env::var_os(variable).is_some() {
            audit.record_env_var_read(variable);
        }
    }
    audit.trust_assertions.push(
        "oracle_sqlplus_client_libraries_selected_by_forwarded_loader_environment=true".to_string(),
    );

    let mut limits = SQLPLUS_BASIC_MAX_LIMITS;
    limits.elapsed_ms = limits
        .elapsed_ms
        .min(cli.max_wall_secs.saturating_mul(1_000));
    let connect_identifier = endpoint.connect_identifier();
    let mut probed = start_and_probe_sqlplus(
        OracleSqlplusConfig {
            executable,
            username: &username,
            connect_identifier: &connect_identifier,
            password: &secret,
            isolated_network_config_dir: network,
            startup_timeout: Duration::from_secs(cli.max_wall_secs.clamp(1, 120)),
        },
        limits,
    )
    .map_err(|failure| anyhow!("DBP1426E Oracle SQL*Plus startup or probe failed: {failure:?}"))?;
    drop(secret);
    audit.connection.uri_redacted = endpoint.redacted_uri();
    audit.record_database_principals(
        probed.effective_principal(),
        probed.effective_principal(),
        probed.effective_principal(),
        None,
        "SQL*Plus session principal observed before catalogue capture",
    );
    audit.record_database_identity(probed.database_identity());
    audit
        .trust_assertions
        .push("oracle_tns_admin_private_empty=true".to_string());

    let request = oracle_request(OracleProviderKind::Sqlplus, cli.artifact_detail);
    let negotiated =
        negotiate_oracle_capabilities(&request, probed.declaration(), probed.evidence()).map_err(
            |failure| anyhow!("DBP1426E Oracle SQL*Plus capability admission failed: {failure:?}"),
        )?;
    record_oracle_provider_proof(audit, &negotiated);
    let selection = if cli.schema.is_empty() {
        OracleOwnerSelection::Default
    } else {
        OracleOwnerSelection::Selected(cli.schema.clone())
    };
    let provisional = OracleOwnerScope::provisional(&selection, probed.effective_principal())
        .map_err(|failure| anyhow!("DBP1426E Oracle owner selection is invalid: {failure:?}"))?;
    // Capture provenance starts only after the live session and its effective
    // identity have been established. --generated-at pins deterministic
    // Blueprint output; it must never rewrite when the database was observed.
    let captured_at = format::generated_at_now(None);
    let capture = run_catalog_capture_for_scope(&negotiated, probed.adapter_mut(), &provisional);
    record_oracle_capture_audit(audit, &capture);
    let scope = resolve_scope_from_capture(
        &capture,
        selection,
        probed.effective_principal(),
        &provisional,
    )?;
    let blueprint = map_oracle_basic_capture(
        &capture,
        &scope,
        probed.server_version(),
        &OracleBasicOptions {
            source_kind: source_kind.as_str().to_string(),
            generated_at_pin: Some(blueprint_generated_at),
            artifact_detail: cli.artifact_detail,
        },
    )
    .context("DBP1426E mapping Oracle SQL*Plus Basic catalogue capture")?;
    dbwarp_blueprint_core::validate_blueprint_contract(&blueprint)
        .context("DBP1502E validating Oracle Basic Blueprint before publication")?;

    // The native capture stream is optional. Publish the validated anonymised
    // Blueprint first so a stream validation, filesystem, or ACL failure can
    // never turn a successful capture into no customer result.
    publish_oracle_blueprint(cli, audit, blueprint)?;

    if let Some(path) = cli.oracle_basic_capture_out.as_deref() {
        record_optional_oracle_stream(
            audit,
            path,
            write_oracle_basic_stream(
                path,
                &capture,
                &scope,
                probed.server_version(),
                cli.artifact_detail,
                negotiated.limits(),
                &captured_at,
                probed.effective_principal(),
                probed.database_identity(),
            ),
        );
    }
    Ok(())
}

pub(super) fn run_oracle_offline(cli: &Cli, audit: &mut AuditLog, path: &Path) -> Result<()> {
    require_preview_acknowledgement(cli)?;
    validate_oracle_common_options(cli, true)?;
    let source_kind =
        SourceKind::parse(&cli.source_kind).context("DBP1013E validating --source-kind")?;
    validate_oracle_stream_path(cli, path, "DBP1505E")?;
    audit.connection.uri_redacted = "(offline: --from-oracle-basic)".to_string();
    audit.connection.auth = "(not used)".to_string();
    audit.connection.tls_mode = "(not used)".to_string();
    audit.connection.user_source = Some("(not used)".to_string());
    audit.artifact_detail = Some(cli.artifact_detail.as_str().to_string());
    audit
        .trust_assertions
        .push("oracle_preview_acknowledged=true".to_string());
    audit
        .network_egress
        .push("none (checksummed Oracle Basic offline capture)".to_string());

    if cli.dry_run {
        eprintln!("{}", i18n::text("preflight.title"));
        preflight_line("preflight.mode", "oracle-basic-offline-preview");
        preflight_line("preflight.input_file", path.display());
        preflight_line("preflight.output_toml", cli.out.display());
        preflight_line("preflight.network", i18n::text("value.none"));
        eprintln!();
        eprintln!("{}", i18n::text("dry.file"));
        return Ok(());
    }

    configure_anonymization_key(cli, audit)?;
    audit.record_file_read(&path.display().to_string());
    let decoded = read_oracle_basic_stream(path)?;
    record_sensitive_file_mode_warning(audit, path, "Oracle Basic offline stream");
    let blueprint_generated_at =
        offline_blueprint_generated_at(cli.generated_at.as_deref(), &decoded.captured_at);
    if decoded.artifact_detail != cli.artifact_detail {
        bail!(
            "DBP1505E offline Oracle stream was captured with artifact-detail={} but this run requested {}",
            decoded.artifact_detail.as_str(),
            cli.artifact_detail.as_str()
        );
    }
    let (declaration, evidence) = oracle_offline_provider_contract(
        decoded.server_version,
        crate::oracle_offline::ORACLE_BASIC_QUERY_PACK_VERSION,
        decoded.query_pack_sha256,
        decoded.stream_sha256,
        decoded.limits,
    );
    let request = oracle_request(OracleProviderKind::Offline, decoded.artifact_detail);
    let negotiated =
        negotiate_oracle_capabilities(&request, &declaration, &evidence).map_err(|failure| {
            anyhow!("DBP1505E Oracle offline capability admission failed: {failure:?}")
        })?;
    record_oracle_provider_proof(audit, &negotiated);
    audit.trust_assertions.push(format!(
        "oracle_offline_stream_sha256={}",
        hex::encode(decoded.stream_sha256)
    ));
    audit.trust_assertions.push(format!(
        "oracle_offline_capture_timestamp={}",
        decoded.captured_at
    ));
    audit.trust_assertions.push(format!(
        "oracle_offline_input_kind={}",
        match decoded.input_kind {
            OracleBasicOfflineInputKind::LiveStream => "live-adapter-stream",
            OracleBasicOfflineInputKind::DbaSpool => "dba-sqlplus-spool",
        }
    ));
    audit.trust_assertions.push(format!(
        "oracle_recorded_sqlplus_release={}",
        decoded
            .recorded_client_version
            .map(format_oracle_version)
            .unwrap_or_else(|| "unknown".to_string())
    ));
    let converter_identity = local_converter_identity(audit);
    audit
        .trust_assertions
        .push(format!("oracle_offline_converter={converter_identity}"));
    audit.record_database_principals(
        &decoded.capturing_principal,
        &decoded.capturing_principal,
        &decoded.capturing_principal,
        None,
        "capturing Oracle session principal came from the offline capture; conversion used the separately recorded local process",
    );
    audit.record_database_identity(&decoded.database_identity);
    record_oracle_capture_audit(audit, &decoded.capture);
    let blueprint = map_oracle_basic_capture(
        &decoded.capture,
        &decoded.scope,
        decoded.server_version,
        &OracleBasicOptions {
            source_kind: source_kind.as_str().to_string(),
            generated_at_pin: Some(blueprint_generated_at),
            artifact_detail: decoded.artifact_detail,
        },
    )
    .context("DBP1505E mapping checksummed Oracle Basic offline capture")?;
    dbwarp_blueprint_core::validate_blueprint_contract(&blueprint)
        .context("DBP1502E validating offline Oracle Basic Blueprint before publication")?;
    publish_oracle_blueprint(cli, audit, blueprint)
}

fn offline_blueprint_generated_at(pinned: Option<&str>, captured_at: &str) -> String {
    pinned
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| captured_at.to_string())
}

fn format_oracle_version(version: crate::oracle_provider::OracleVersion) -> String {
    version
        .components()
        .iter()
        .map(u16::to_string)
        .collect::<Vec<_>>()
        .join(".")
}

fn require_preview_acknowledgement(cli: &Cli) -> Result<()> {
    if !cli.acknowledge_oracle_preview {
        bail!(
            "DBP1426E Oracle Basic is a preview in 1.6; rerun with --acknowledge-oracle-preview after reviewing the limitations"
        );
    }
    Ok(())
}

fn validate_oracle_common_options(cli: &Cli, offline: bool) -> Result<()> {
    if cli.measure_compression {
        bail!(
            "DBP1426E Oracle 1.6 preview is Basic catalogue-only and does not sample customer rows"
        );
    }
    if !matches!(
        cli.artifact_detail,
        ArtifactDetail::None | ArtifactDetail::Summary
    ) {
        bail!("DBP1426E Oracle 1.6 preview supports --artifact-detail none or summary only");
    }
    if cli.length_fidelity != LengthFidelity::Balanced || cli.preserve_exact_lengths {
        bail!("DBP1426E length-fidelity controls do not apply to Oracle Basic catalogue capture");
    }
    if cli.sample_rows != engine_common::DEFAULT_SAMPLE_ROWS
        || cli.compression_workers.is_some()
        || cli.no_rtt_probe
        || !cli.select.is_empty()
    {
        bail!("DBP1426E row-sampling, RTT, worker, and bundle-selection controls do not apply to Oracle Basic catalogue capture");
    }
    if cli.tls_ca.is_some()
        || cli.tls_cert.is_some()
        || cli.tls_key.is_some()
        || cli.tls_server_name.is_some()
        || cli.tls_skip_verify
        || cli.i_know_what_im_doing
    {
        bail!("DBP1426E Oracle 1.6 SQL*Plus preview accepts direct plain TCP only; encrypted transport and wallets are not available");
    }
    if cli.tls_mode_explicit {
        if offline {
            bail!("DBP1505E --tls-mode is not available for offline Oracle Basic ingestion");
        }
        bail!("DBP1426E --tls-mode is not available for Oracle 1.6 SQL*Plus preview; the preview transport is plain TCP");
    }
    if cli.auth_mode.is_some() || cli.azure_token_file.is_some() || cli.azure_token_env.is_some() {
        bail!("DBP1426E Oracle 1.6 preview supports password authentication only");
    }
    if cli.expect_server_principal.is_some() {
        bail!("DBP1426E --expect-server-principal is a SQL Server-only assertion");
    }
    if offline
        && (cli.yes
            || cli.max_wall_secs != engine_pg::DEFAULT_SAMPLE_TIMEOUT_SECS
            || cli.user.is_some()
            || cli.user_env.is_some()
            || cli.user_file.is_some()
            || cli.password_file.is_some()
            || cli.password_env.is_some()
            || cli.oracle_sqlplus.is_some()
            || cli.oracle_network_config_dir.is_some()
            || cli.oracle_basic_capture_out.is_some()
            || !cli.schema.is_empty())
    {
        bail!("DBP1505E offline Oracle Basic ingestion does not accept connection, credential, or schema-selection options");
    }
    Ok(())
}

pub(super) fn validate_oracle_script_options(cli: &Cli) -> Result<()> {
    validate_oracle_common_options(cli, true)?;
    if cli.artifact_detail != ArtifactDetail::Summary
        || cli.source_kind != "production"
        || cli.anonymization_key_file.is_some()
        || cli.generated_at.is_some()
        || cli.dry_run
        || cli.deck.is_some()
        || cli.deck_confidentiality.is_some()
        || cli.out != Path::new("blueprint.toml")
    {
        bail!("DBP1426E Blueprint-output, source-annotation, sampling, and dry-run controls do not apply to Oracle DBA script rendering");
    }
    Ok(())
}

fn print_oracle_preflight(
    cli: &Cli,
    endpoint: &OracleDirectService,
    user_source: &str,
    secret_source: &SecretSource,
) {
    eprintln!("{}", i18n::text("preflight.title"));
    preflight_line("preflight.mode", "oracle-basic-sqlplus-preview");
    preflight_line("preflight.database", endpoint.redacted_uri());
    preflight_line(
        "preflight.network",
        format!("{}:{}", endpoint.host, endpoint.port),
    );
    preflight_line(
        "preflight.schemas",
        if cli.schema.is_empty() {
            i18n::text("value.authenticated_owner").to_string()
        } else {
            cli.schema.join(", ")
        },
    );
    preflight_line("preflight.user_source", user_source);
    preflight_line("preflight.password_source", secret_source.audit_str());
    preflight_line("preflight.password_persisted", i18n::text("value.no"));
    preflight_line("preflight.tls_mode", "disable (plain TCP)");
    preflight_line("preflight.output_toml", cli.out.display());
    eprintln!();
}

fn oracle_request(provider: OracleProviderKind, detail: ArtifactDetail) -> OracleCaptureRequest {
    let offline = provider == OracleProviderKind::Offline;
    OracleCaptureRequest {
        contract_version: ORACLE_PROVIDER_CONTRACT_VERSION,
        provider,
        tier: OracleCaptureTier::Basic,
        artifact_detail: detail,
        endpoint_mode: if offline {
            OracleEndpointMode::Offline
        } else {
            OracleEndpointMode::DirectService
        },
        transport: if offline {
            OracleTransport::Offline
        } else {
            OracleTransport::Tcp
        },
        authentication: if offline {
            OracleAuthentication::Offline
        } else {
            OracleAuthentication::Password
        },
        trust_source: OracleTrustSource::NotApplicable,
        authorized_local_inputs: if offline {
            BTreeSet::from([OracleLocalInput::OfflineCapture])
        } else {
            BTreeSet::from([OracleLocalInput::ClientExecutable])
        },
        require_confirmed_server_cancel: false,
    }
}

fn validate_oracle_stream_path(cli: &Cli, stream: &Path, code: &str) -> Result<()> {
    if !stream.is_absolute() {
        bail!("{code} Oracle Basic capture stream paths must be absolute");
    }
    let stream = resolved_path_identity(stream, code, "the Oracle Basic stream")?;
    let candidates = [
        ("--out", Some(cli.out.as_path())),
        ("--audit-log", cli.audit_log.as_deref()),
        ("--deck", cli.deck.as_deref()),
        (
            "--anonymization-key-file",
            cli.anonymization_key_file.as_deref(),
        ),
        ("--user-file", cli.user_file.as_deref()),
        ("--password-file", cli.password_file.as_deref()),
        ("--oracle-sqlplus", cli.oracle_sqlplus.as_deref()),
        (
            "--oracle-basic-script-out",
            cli.oracle_basic_script_out.as_deref(),
        ),
        ("--batch-manifest", cli.batch_manifest.as_deref()),
        ("--out-dir", cli.out_dir.as_deref()),
        ("--bundle-list", cli.bundle_list.as_deref()),
        ("--bundle-extract", cli.bundle_extract.as_deref()),
        ("--bundle-pack", cli.bundle_pack.as_deref()),
        ("--from-toml", cli.from_toml.as_deref()),
        ("--from-parquet", cli.from_parquet.as_deref()),
        ("--from-avro", cli.from_avro.as_deref()),
    ];
    for (label, candidate) in candidates {
        let Some(candidate) = candidate else {
            continue;
        };
        if resolved_path_identity(candidate, code, label)? == stream {
            bail!("{code} the Oracle Basic capture stream must not share {label}'s path");
        }
    }
    Ok(())
}

fn resolved_path_identity(path: &Path, code: &str, label: &str) -> Result<PathBuf> {
    use std::path::Component;

    let absolute =
        std::path::absolute(path).with_context(|| format!("{code} resolving {label}"))?;
    let mut resolved = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(_) | Component::RootDir => {
                resolved.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if !resolved.pop() {
                    bail!("{code} {label} escapes its filesystem root");
                }
            }
            Component::Normal(name) => {
                let candidate = resolved.join(name);
                if candidate.exists() {
                    resolved = candidate
                        .canonicalize()
                        .with_context(|| format!("{code} resolving {label}"))?;
                } else {
                    resolved.push(name);
                }
            }
        }
    }
    Ok(resolved)
}

fn record_oracle_provider_proof(audit: &mut AuditLog, negotiated: &OracleNegotiatedCapabilities) {
    if let Some(digest) = negotiated.proof().provider_artifact_sha256() {
        audit.trust_assertions.push(format!(
            "oracle_provider_artifact_sha256={}",
            hex::encode(digest)
        ));
    }
    if let Some(digest) = negotiated.proof().probe_transcript_sha256() {
        audit.trust_assertions.push(format!(
            "oracle_probe_transcript_sha256={}",
            hex::encode(digest)
        ));
    }
    if let Some(digest) = negotiated.proof().session_binding_sha256() {
        audit.trust_assertions.push(format!(
            "oracle_session_binding_sha256={}",
            hex::encode(digest)
        ));
    }
}

fn redacted_oracle_stream_write_failure(_error: &anyhow::Error) -> &'static str {
    // The failed self-validation may have inspected native catalogue values.
    // The Blueprint remains usable, so expose only the stable failure class;
    // never copy the error chain into terminal or audit output.
    "private stream validation or atomic publication failed"
}

fn record_optional_oracle_stream(audit: &mut AuditLog, path: &Path, result: Result<(u64, String)>) {
    match result {
        Ok((bytes, sha256)) => {
            audit.record_file_written(path.to_path_buf(), bytes, sha256);
            record_sensitive_file_mode_warning(audit, path, "Oracle Basic offline stream");
        }
        Err(error) => audit.record_warning(
            "DBP1430W",
            format!(
                "Oracle Basic Blueprint was retained but its optional offline stream was not written: {}",
                redacted_oracle_stream_write_failure(&error)
            ),
        ),
    }
}

#[cfg(unix)]
fn local_converter_identity(_audit: &mut AuditLog) -> String {
    // SAFETY: geteuid has no pointer arguments or preconditions.
    format!("unix-euid:{}", unsafe { libc::geteuid() })
}

#[cfg(windows)]
fn local_converter_identity(audit: &mut AuditLog) -> String {
    audit.env_vars_read.push("USERNAME".to_string());
    audit.env_vars_read.push("USERDOMAIN".to_string());
    let username = std::env::var("USERNAME")
        .ok()
        .filter(|value| valid_local_identity_component(value));
    let domain = std::env::var("USERDOMAIN")
        .ok()
        .filter(|value| valid_local_identity_component(value));
    match (domain, username) {
        (Some(domain), Some(username)) => format!("windows-env-user:{domain}\\{username}"),
        (None, Some(username)) => format!("windows-env-user:{username}"),
        _ => "windows-process-identity-unavailable".to_string(),
    }
}

#[cfg(windows)]
fn valid_local_identity_component(value: &str) -> bool {
    !value.is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}

#[cfg(not(any(unix, windows)))]
fn local_converter_identity(_audit: &mut AuditLog) -> String {
    "local-process-identity-unavailable".to_string()
}

fn resolve_scope_from_capture(
    capture: &OracleCatalogCapture,
    selection: OracleOwnerSelection,
    session_user: &str,
    provisional: &OracleOwnerScope,
) -> Result<OracleOwnerScope> {
    let user_outcomes = capture
        .outcomes
        .iter()
        .filter(|outcome| outcome.query_id == "oracle-users")
        .collect::<Vec<_>>();
    if user_outcomes.len() == provisional.owners().len()
        && user_outcomes
            .iter()
            .all(|outcome| matches!(outcome.status, OracleQueryStatus::Executed { .. }))
    {
        let mut visible = Vec::with_capacity(user_outcomes.len());
        for (index, outcome) in user_outcomes.into_iter().enumerate() {
            if outcome.owner_ordinal != Some((index + 1) as u32) {
                bail!("DBP1426E Oracle ALL_USERS returned inconsistent owner evidence");
            }
            if outcome.rows.is_empty() {
                bail!(OWNER_NOT_FOUND);
            }
            if outcome.rows.len() != 1 {
                bail!("DBP1426E Oracle ALL_USERS returned inconsistent owner evidence");
            }
            match outcome.rows[0].values.first() {
                Some(OracleValue::Text(owner))
                    if owner == &provisional.owners()[index] && !owner.is_empty() =>
                {
                    visible.push(owner.clone());
                }
                _ => bail!("DBP1426E Oracle ALL_USERS returned malformed owner evidence"),
            }
        }
        return resolve_oracle_owner_scope(selection, session_user, visible)
            .map_err(owner_resolution_error);
    }

    // The minimum five-view grant intentionally does not depend on ALL_USERS.
    // For selected schemas, require positive DBA_OBJECTS evidence for every
    // provisional owner.  Default scope is already bound to the probed
    // SESSION_USER and therefore needs no second identity catalogue.
    if matches!(selection, OracleOwnerSelection::Selected(_)) {
        for ordinal in 1..=provisional.owners().len() {
            let proved = capture.outcomes.iter().any(|outcome| {
                outcome.query_id == "oracle-objects"
                    && outcome.owner_ordinal == Some(ordinal as u32)
                    && matches!(outcome.status, OracleQueryStatus::Executed { rows } if rows > 0)
            });
            if !proved {
                bail!("DBP1426E a selected Oracle owner could not be proven from the minimum catalogue floor. Owner names are matched exactly and are usually upper case (APP, not app); check the spelling, or the owner may have no objects.");
            }
        }
    }
    Ok(provisional.clone())
}

const OWNER_NOT_FOUND: &str = "DBP1426E a selected Oracle owner was not found. Owner names are matched exactly and are usually upper case (APP, not app); check the spelling and letter case, then retry.";

fn owner_resolution_error(failure: OracleOwnerResolutionFailure) -> anyhow::Error {
    match failure {
        OracleOwnerResolutionFailure::MissingRequestedOwner => anyhow!(OWNER_NOT_FOUND),
        other => anyhow!("DBP1426E Oracle owner selection did not resolve: {other:?}"),
    }
}

fn record_oracle_capture_audit(audit: &mut AuditLog, capture: &OracleCatalogCapture) {
    // The capture contract exposes one measured wall time for the bounded
    // catalogue pass, not per-query timings.  Record the measured aggregate
    // instead of fabricating zero-duration entries for each planned query.
    audit.record_query(
        "Oracle Basic bounded catalogue capture",
        capture.elapsed_ms,
        capture.rows_consumed,
    );
    if let Some(reason) = capture
        .client_version_attestation
        .and_then(|attestation| attestation.limitation_token())
    {
        audit.record_warning(
            "DBP1427W",
            format!("Oracle SQL*Plus client provenance is weaker: {reason}"),
        );
    }
    if let Some(crate::oracle_provider::OracleClientVersionAttestation::Attested(version)) =
        capture.client_version_attestation
    {
        let version = version
            .components()
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join(".");
        audit
            .trust_assertions
            .push(format!("oracle_attested_client_version={version}"));
    }
    audit.trust_assertions.push(format!(
        "oracle_capture_abort={}",
        capture.abort.map(oracle_abort_token).unwrap_or("none")
    ));
    audit.trust_assertions.push(format!(
        "oracle_session_discarded={}",
        capture.session_discarded
    ));
    audit.trust_assertions.push(format!(
        "oracle_cancellation_outcome={}",
        if capture.unconfirmed_server_work {
            "local-process-termination-server-work-unconfirmed"
        } else {
            "not-attempted"
        }
    ));
    let unavailable_optional = capture
        .outcomes
        .iter()
        .filter(|outcome| matches!(outcome.status, OracleQueryStatus::Failed { .. }))
        .count();
    if unavailable_optional > 0 {
        audit.record_warning(
            "DBP1429W",
            format!(
                "Oracle Basic retained its floor with {unavailable_optional} unavailable catalogue outcomes"
            ),
        );
    }
    if let Some(reason) = capture.abort {
        audit.record_warning(
            "DBP1428W",
            format!("Oracle Basic capture stopped early and retained its proven floor: {reason:?}"),
        );
    }
}

fn oracle_abort_token(reason: crate::oracle_session::OracleCaptureAbort) -> &'static str {
    use crate::oracle_session::OracleCaptureAbort;
    match reason {
        OracleCaptureAbort::CoreInventoryUnavailable => "core-inventory-unavailable",
        OracleCaptureAbort::OwnerScopeUnavailable => "owner-scope-unavailable",
        OracleCaptureAbort::NoLiveSession => "no-live-session",
        OracleCaptureAbort::RowLimitExceeded => "row-limit-exceeded",
        OracleCaptureAbort::ByteLimitExceeded => "byte-limit-exceeded",
        OracleCaptureAbort::TransientValueLimitExceeded => "transient-value-limit-exceeded",
        OracleCaptureAbort::LobLimitExceeded => "lob-limit-exceeded",
        OracleCaptureAbort::DeadlineExceeded => "deadline-exceeded",
        OracleCaptureAbort::SessionLost => "session-lost",
        OracleCaptureAbort::AdapterContractViolated => "adapter-contract-violated",
        OracleCaptureAbort::UnconfirmedCancellation => "unconfirmed-cancellation",
    }
}

fn publish_oracle_blueprint(
    cli: &Cli,
    audit: &mut AuditLog,
    blueprint: BlueprintFile,
) -> Result<()> {
    audit.record_fidelity(dbwarp_blueprint_core::estimate_blueprint_fidelity(
        &blueprint,
    ));
    audit.record_artifact_inventory(blueprint.artifact_inventory.as_ref());
    audit.record_sizing_scope(
        blueprint.database_topology.as_ref(),
        blueprint.dataset_scope.as_ref(),
    );
    let body = emit_toml(&blueprint).context("DBP1502E emitting Oracle Basic Blueprint")?;
    atomic_write_bytes(&cli.out, body.as_bytes())
        .with_context(|| format!("DBP1502E writing output {}", cli.out.display()))?;
    let sha = hex::encode(Sha256::digest(body.as_bytes()));
    audit.record_file_written(cli.out.clone(), body.len() as u64, sha);
    println!(
        "{}",
        i18n::format("status.wrote", &[("path", cli.out.display().to_string())])
    );
    if let Some(deck_path) = &cli.deck {
        write_deck(
            deck_path,
            &blueprint,
            cli.deck_confidentiality.as_ref(),
            audit,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oracle_session::{OracleQueryOutcome, OracleRow};

    #[test]
    fn direct_service_parser_rejects_userinfo_and_descriptors() {
        assert!(OracleDirectService::parse("oracle://app@db:1521/PDB").is_err());
        assert!(OracleDirectService::parse("oracle://db:1521/(DESCRIPTION=x)").is_err());
        let parsed = OracleDirectService::parse("oracle://[::1]:1522/PDB1").unwrap();
        assert_eq!(parsed.connect_identifier(), "//[::1]:1522/PDB1");
        assert!(is_oracle_connect("ORACLE://db:1521/PDB1"));
        assert!(OracleDirectService::parse("OrAcLe://db:1521/PDB1").is_ok());
    }

    #[test]
    fn offline_generation_defaults_to_capture_time_but_allows_a_document_pin() {
        let captured_at = "2026-09-28T12:34:56Z";
        assert_eq!(
            offline_blueprint_generated_at(None, captured_at),
            captured_at
        );
        assert_eq!(
            offline_blueprint_generated_at(Some("2026-09-28T00:00:00Z"), captured_at),
            "2026-09-28T00:00:00Z"
        );
        assert_eq!(
            offline_blueprint_generated_at(Some("   "), captured_at),
            captured_at
        );
    }

    #[test]
    fn owner_scope_is_reconciled_from_the_same_captured_all_users_row() {
        let selection = OracleOwnerSelection::Selected(vec!["APP".to_string()]);
        let provisional = OracleOwnerScope::provisional(&selection, "COLLECTOR").unwrap();
        let capture = OracleCatalogCapture {
            outcomes: vec![OracleQueryOutcome {
                query_id: "oracle-users",
                view: "ALL_USERS",
                owner_ordinal: Some(1),
                status: OracleQueryStatus::Executed { rows: 1 },
                rows: vec![OracleRow {
                    values: vec![
                        OracleValue::Text("APP".to_string()),
                        OracleValue::Text("N".to_string()),
                    ],
                }],
            }],
            catalogs_read: vec!["ALL_USERS"],
            catalogs_unreadable: Vec::new(),
            abort: None,
            session_discarded: false,
            unconfirmed_server_work: false,
            rows_consumed: 1,
            bytes_consumed: 4,
            lob_bytes_consumed: 0,
            elapsed_ms: 1,
            client_version_attestation: None,
        };
        let resolved =
            resolve_scope_from_capture(&capture, selection, "COLLECTOR", &provisional).unwrap();
        assert_eq!(resolved.owners(), &["APP"]);
    }

    #[test]
    fn missing_selected_owner_gets_an_exact_case_remedy() {
        let selection = OracleOwnerSelection::Selected(vec!["app".to_string()]);
        let provisional = OracleOwnerScope::provisional(&selection, "COLLECTOR").unwrap();
        let capture = OracleCatalogCapture {
            outcomes: vec![OracleQueryOutcome {
                query_id: "oracle-users",
                view: "ALL_USERS",
                owner_ordinal: Some(1),
                status: OracleQueryStatus::Executed { rows: 0 },
                rows: Vec::new(),
            }],
            catalogs_read: vec!["ALL_USERS"],
            catalogs_unreadable: Vec::new(),
            abort: None,
            session_discarded: false,
            unconfirmed_server_work: false,
            rows_consumed: 0,
            bytes_consumed: 0,
            lob_bytes_consumed: 0,
            elapsed_ms: 1,
            client_version_attestation: None,
        };
        let error = resolve_scope_from_capture(&capture, selection, "COLLECTOR", &provisional)
            .unwrap_err()
            .to_string();
        assert!(error.starts_with("DBP1426E a selected Oracle owner was not found"));
        assert!(error.contains("usually upper case"));
        assert!(!error.contains("inconsistent"));
    }

    #[test]
    fn minimum_grant_scope_requires_positive_object_evidence_when_users_is_denied() {
        let selection = OracleOwnerSelection::Selected(vec!["APP".to_string()]);
        let provisional = OracleOwnerScope::provisional(&selection, "COLLECTOR").unwrap();
        let mut capture = OracleCatalogCapture {
            outcomes: vec![
                OracleQueryOutcome {
                    query_id: "oracle-users",
                    view: "ALL_USERS",
                    owner_ordinal: Some(1),
                    status: OracleQueryStatus::Failed {
                        class: crate::oracle_session::OracleQueryFailure::PermissionDenied,
                    },
                    rows: Vec::new(),
                },
                OracleQueryOutcome {
                    query_id: "oracle-objects",
                    view: "DBA_OBJECTS",
                    owner_ordinal: Some(1),
                    status: OracleQueryStatus::Executed { rows: 0 },
                    rows: Vec::new(),
                },
            ],
            catalogs_read: vec!["DBA_OBJECTS"],
            catalogs_unreadable: vec!["ALL_USERS"],
            abort: None,
            session_discarded: false,
            unconfirmed_server_work: false,
            rows_consumed: 0,
            bytes_consumed: 0,
            lob_bytes_consumed: 0,
            elapsed_ms: 1,
            client_version_attestation: None,
        };
        assert!(
            resolve_scope_from_capture(&capture, selection.clone(), "COLLECTOR", &provisional)
                .is_err()
        );
        capture.outcomes[1].status = OracleQueryStatus::Executed { rows: 1 };
        capture.outcomes[1].rows = vec![OracleRow {
            values: vec![
                OracleValue::Text("APP".to_string()),
                OracleValue::Text("APP_TABLE".to_string()),
                OracleValue::Text("TABLE".to_string()),
                OracleValue::Text("VALID".to_string()),
                OracleValue::Text("N".to_string()),
                OracleValue::Text("N".to_string()),
                OracleValue::Text("N".to_string()),
                OracleValue::Text("N".to_string()),
            ],
        }];
        capture.rows_consumed = 1;
        assert!(resolve_scope_from_capture(&capture, selection, "COLLECTOR", &provisional).is_ok());
    }

    #[test]
    fn optional_catalogue_failure_is_a_specific_nonfatal_audit_warning() {
        let capture = OracleCatalogCapture {
            outcomes: vec![OracleQueryOutcome {
                query_id: "oracle-indexes",
                view: "DBA_INDEXES",
                owner_ordinal: Some(1),
                status: OracleQueryStatus::Failed {
                    class: crate::oracle_session::OracleQueryFailure::PermissionDenied,
                },
                rows: Vec::new(),
            }],
            catalogs_read: Vec::new(),
            catalogs_unreadable: vec!["DBA_INDEXES"],
            abort: None,
            session_discarded: false,
            unconfirmed_server_work: false,
            rows_consumed: 0,
            bytes_consumed: 0,
            lob_bytes_consumed: 0,
            elapsed_ms: 7,
            client_version_attestation: None,
        };
        let mut audit = AuditLog::new("oracle-test", 0);
        record_oracle_capture_audit(&mut audit, &capture);
        assert_eq!(audit.queries.len(), 1);
        assert_eq!(audit.queries[0].elapsed_ms, 7);
        assert!(audit
            .warnings
            .iter()
            .any(|value| value.starts_with("DBP1429W ")));
    }

    #[test]
    fn aborted_capture_audit_records_cancellation_and_client_attestation() {
        let version = crate::oracle_provider::OracleVersion::from_components(&[21, 3]).unwrap();
        let capture = OracleCatalogCapture {
            outcomes: Vec::new(),
            catalogs_read: Vec::new(),
            catalogs_unreadable: Vec::new(),
            abort: Some(crate::oracle_session::OracleCaptureAbort::DeadlineExceeded),
            session_discarded: true,
            unconfirmed_server_work: true,
            rows_consumed: 0,
            bytes_consumed: 0,
            lob_bytes_consumed: 0,
            elapsed_ms: 60_001,
            client_version_attestation: Some(
                crate::oracle_provider::OracleClientVersionAttestation::Attested(version),
            ),
        };
        let mut audit = AuditLog::new("oracle-test", 0);
        record_oracle_capture_audit(&mut audit, &capture);
        assert!(audit
            .trust_assertions
            .contains(&"oracle_attested_client_version=21.3".to_string()));
        assert!(audit
            .trust_assertions
            .contains(&"oracle_capture_abort=deadline-exceeded".to_string()));
        assert!(audit.trust_assertions.contains(
            &"oracle_cancellation_outcome=local-process-termination-server-work-unconfirmed"
                .to_string()
        ));
    }

    #[test]
    fn optional_stream_failure_is_redacted_and_does_not_become_an_error() {
        let mut audit = AuditLog::new("oracle-test", 0);
        record_optional_oracle_stream(
            &mut audit,
            Path::new("native-table-name-must-not-escape.capture"),
            Err(anyhow!("native-table-name-must-not-escape")),
        );
        assert!(audit.files_written_local.is_empty());
        assert_eq!(audit.warnings.len(), 1);
        assert!(audit.warnings[0].starts_with("DBP1430W "));
        assert!(!audit.warnings[0].contains("native-table-name-must-not-escape"));
    }
}
