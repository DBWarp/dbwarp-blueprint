/// Build a `SecretSource` descriptor purely from CLI flags, without reading
/// the file or prompting the TTY. Used by `--dry-run` so pre-flight does not
/// expand the filesystem or TTY surface.
///
/// File mode is best-effort: this stats the file without reading it so the
/// summary can warn about a world-readable password file before consent.
fn describe_secret_source(cli: &Cli, has_uri_pw: bool) -> SecretSource {
    if matches!(cli.auth_mode, Some(AuthMode::Integrated)) {
        return SecretSource::IntegratedAuth;
    }
    if let Some(path) = &cli.azure_token_file {
        let mode = secret::file_mode(path);
        return SecretSource::EntraTokenFile {
            path: path.clone(),
            mode,
        };
    }
    if let Some(var) = &cli.azure_token_env {
        return SecretSource::EntraTokenEnv {
            var_name: var.clone(),
        };
    }
    if let Some(path) = &cli.password_file {
        let mode = secret::file_mode(path);
        return SecretSource::File {
            path: path.clone(),
            mode,
        };
    }
    if let Some(var) = &cli.password_env {
        return SecretSource::Env {
            var_name: var.clone(),
        };
    }
    if has_uri_pw {
        return SecretSource::ConnectionString;
    }
    SecretSource::Tty
}

fn acquire_secret(
    cli: &Cli,
    // Always None on the production path: URI-embedded passwords are
    // rejected before we get here. Kept on the signature so misuse
    // (passing a Some) is caught by the assertion below rather than
    // silently bypassing the security policy.
    embedded_pw: Option<String>,
    audit: &mut AuditLog,
) -> Result<(Secret, SecretSource)> {
    debug_assert!(
        embedded_pw.is_none(),
        "URI-embedded passwords must be rejected by the caller"
    );

    // Mutual-exclusion: Entra token flags conflict with password flags.
    // Reject loudly rather than silently preferring one over the other.
    let pw_set = cli.password_file.is_some() || cli.password_env.is_some();
    let token_set = cli.azure_token_file.is_some() || cli.azure_token_env.is_some();
    if pw_set && token_set {
        bail!(
            "DBP1604E --azure-token-file/--azure-token-env are mutually exclusive with \
             --password-file/--password-env. Pick exactly one credential source."
        );
    }
    if cli.azure_token_file.is_some() && cli.azure_token_env.is_some() {
        bail!("DBP1604E --azure-token-file and --azure-token-env are mutually exclusive");
    }

    // Entra ID token paths.
    if let Some(path) = &cli.azure_token_file {
        audit.record_file_read(&path.display().to_string());
        let mode = secret::file_mode(path);
        // Token files use the same sensitive-file mode check as
        // --password-file: refuse on group/other-readable Unix modes.
        let s = Secret::from_file(path)?;
        audit.connection.credential_actually_read = true;
        return Ok((
            s,
            SecretSource::EntraTokenFile {
                path: path.clone(),
                mode,
            },
        ));
    }
    if let Some(var) = &cli.azure_token_env {
        audit.record_env_var_read(var);
        let s = Secret::from_env(var)?;
        audit.connection.credential_actually_read = true;
        return Ok((
            s,
            SecretSource::EntraTokenEnv {
                var_name: var.clone(),
            },
        ));
    }

    // Password paths.
    if let Some(path) = &cli.password_file {
        // Record the attempted read BEFORE the fallible call. If the
        // file can't be read, the audit still shows what was attempted
        // — which is what a forensic reader wants ("the tool tried to
        // read X and failed").
        audit.record_file_read(&path.display().to_string());
        let mode = secret::file_mode(path);
        let s = Secret::from_file(path)?;
        // Mark "credential actually read" so the trust assertion in
        // finalize() fires only on runs where we hit this path (not
        // on --dry-run).
        audit.connection.credential_actually_read = true;
        return Ok((
            s,
            SecretSource::File {
                path: path.clone(),
                mode,
            },
        ));
    }
    if let Some(var) = &cli.password_env {
        // Record before the read attempt so the audit shows what was
        // tried even if the env var is unset.
        audit.record_env_var_read(var);
        let s = Secret::from_env(var)?;
        audit.connection.credential_actually_read = true;
        return Ok((
            s,
            SecretSource::Env {
                var_name: var.clone(),
            },
        ));
    }
    let s = Secret::from_tty_prompt(&format!("Database password: "))?;
    audit.connection.credential_actually_read = true;
    Ok((s, SecretSource::Tty))
}

#[derive(Debug, Clone, Copy)]
enum EngineKind {
    Postgresql,
    MySQL,
    Mssql,
}

/// Operator-facing authentication-method selector. `CloudToken` is explicit
/// because MySQL managed-service token authentication needs the
/// `mysql_clear_password` exchange enabled, which must never be switched on for
/// ordinary password connections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum AuthMode {
    /// Database username and password.
    SqlAuth,
    /// Microsoft Entra ID (Azure AD) OAuth access token.
    EntraToken,
    /// Externally generated PostgreSQL/MySQL managed-service token. Requires
    /// exactly one --password-file/-env and --tls-mode verify-full.
    CloudToken,
    /// SQL Server engine only. Kerberos (Linux) / SSPI (Windows).
    /// Requires a build compiled with --features
    /// integrated-auth-gssapi (Linux) or winauth (Windows). On vanilla
    /// builds this is rejected with a clear rebuild hint.
    Integrated,
}

impl AuthMode {
    fn audit_str(self) -> &'static str {
        match self {
            Self::SqlAuth => "sql-auth",
            Self::EntraToken => "entra-token",
            Self::CloudToken => "cloud-token",
            Self::Integrated => "integrated",
        }
    }
}

/// Resolve the authentication mode for the selected engine and validate the
/// credential-source contract before any secret is read.
fn resolve_auth_mode(cli: &Cli, engine: EngineKind) -> Result<AuthMode> {
    if matches!(engine, EngineKind::Mssql) {
        return resolve_mssql_auth_mode(cli);
    }

    let mode = cli.auth_mode.unwrap_or(AuthMode::SqlAuth);
    match mode {
        AuthMode::SqlAuth => Ok(mode),
        AuthMode::CloudToken => {
            let password_sources =
                usize::from(cli.password_file.is_some()) + usize::from(cli.password_env.is_some());
            if password_sources != 1 {
                bail!(
                    "DBP1604E --auth-mode=cloud-token requires exactly one externally generated token source: --password-file or --password-env"
                );
            }
            Ok(mode)
        }
        AuthMode::EntraToken | AuthMode::Integrated => {
            let engine_name = match engine {
                EngineKind::Postgresql => "PostgreSQL",
                EngineKind::MySQL => "MySQL",
                EngineKind::Mssql => unreachable!(),
            };
            bail!(
                "DBP1005E --auth-mode={} is unavailable for {engine_name}; use sql-auth or cloud-token",
                mode.audit_str()
            )
        }
    }
}

/// Cloud tokens are bearer credentials. Requiring certificate and hostname
/// verification prevents a malicious endpoint from collecting the token even
/// if a provider would accept a weaker encrypted transport.
fn validate_cloud_token_tls(mode: AuthMode, tls: &TlsParams) -> Result<()> {
    if matches!(mode, AuthMode::CloudToken) && tls.mode != TlsMode::VerifyFull {
        bail!(
            "DBP1604E --auth-mode=cloud-token requires --tls-mode=verify-full so the short-lived bearer token is sent only to a verified database endpoint"
        );
    }
    Ok(())
}

fn validate_expected_server_principal(cli: &Cli, engine: EngineKind) -> Result<()> {
    let Some(principal) = cli.expect_server_principal.as_deref() else {
        return Ok(());
    };
    if !matches!(engine, EngineKind::Mssql) {
        bail!("DBP1005E --expect-server-principal is available only for SQL Server live capture");
    }
    if principal.is_empty() || principal.trim() != principal {
        bail!(
            "DBP1606E --expect-server-principal must be non-empty and have no leading or trailing whitespace"
        );
    }
    if principal.chars().count() > 256
        || principal
            .chars()
            .any(|ch| ch.is_control() || i18n::is_forbidden_format_control(ch))
    {
        bail!(
            "DBP1606E --expect-server-principal must contain at most 256 characters and no control or bidirectional formatting characters"
        );
    }
    Ok(())
}

fn integrated_user_source() -> &'static str {
    #[cfg(windows)]
    {
        "windows-logon-session"
    }
    #[cfg(not(windows))]
    {
        "kerberos-credential-cache"
    }
}

/// Describe the configured database username without opening a file or reading
/// a named environment variable. This is used for pre-flight and dry-run
/// output; the live path resolves the value exactly once after consent.
fn preview_user(
    cli: &Cli,
    uri_user: &str,
    uri_user_was_explicit: bool,
    integrated: bool,
) -> Result<(String, &'static str)> {
    if integrated {
        return Ok((
            "(operating-system principal)".to_string(),
            integrated_user_source(),
        ));
    }
    if let Some(user) = &cli.user {
        if user.trim().is_empty() {
            bail!("DBP1603E --user value is empty");
        }
        return Ok((user.trim().to_string(), "flag"));
    }
    if cli.user_env.is_some() {
        return Ok(("(from --user-env)".to_string(), "env"));
    }
    if cli.user_file.is_some() {
        return Ok(("(from --user-file)".to_string(), "file"));
    }
    if uri_user_was_explicit {
        return Ok((uri_user.to_string(), "uri"));
    }
    Ok((uri_user.to_string(), "default"))
}

/// Resolve the database username and label its source for the audit log.
///
/// Precedence (highest to lowest):
///   1. --user FLAG
///   2. --user-env VAR_NAME
///   3. --user-file PATH
///   4. URI-embedded (postgresql://USER@host/db)
///   5. engine default (postgres / root / sa)
fn resolve_user(
    cli: &Cli,
    uri_user: &str,
    uri_user_was_explicit: bool,
    audit: &mut AuditLog,
) -> Result<(String, &'static str)> {
    if let Some(u) = &cli.user {
        if u.trim().is_empty() {
            bail!("DBP1603E --user value is empty");
        }
        return Ok((u.trim().to_string(), "flag"));
    }
    if let Some(var) = &cli.user_env {
        // Record before the read attempt so the audit shows what was
        // tried even if the var is unset.
        audit.record_env_var_read(var);
        let v =
            std::env::var(var).with_context(|| format!("DBP1603E reading --user-env '{var}'"))?;
        let v = v.trim().to_string();
        if v.is_empty() {
            bail!("DBP1603E --user-env '{var}' was empty");
        }
        return Ok((v, "env"));
    }
    if let Some(path) = &cli.user_file {
        // Record before the read attempt so the audit shows what
        // was tried even if the file can't be opened.
        audit.record_file_read(&path.display().to_string());
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("DBP1603E reading --user-file '{}'", path.display()))?;
        // Strip trailing whitespace only — usernames don't contain leading
        // whitespace in any real DB system, but they CAN contain internal
        // characters (e.g. `domain\user`, `app+monitor`).
        let v = raw.trim_end().trim_start_matches('\n').to_string();
        let v = v.trim_end_matches('\r').to_string();
        if v.is_empty() {
            bail!("DBP1603E --user-file '{}' was empty", path.display());
        }
        return Ok((v, "file"));
    }
    if uri_user_was_explicit {
        return Ok((uri_user.to_string(), "uri"));
    }
    Ok((uri_user.to_string(), "default"))
}

/// Resolve the operator's chosen authentication mode for the SQL Server
/// engine. Returns the effective AuthMode, validating that the supplied
/// credential flags match. Three error paths:
///
/// 1. Explicit `--auth-mode X` plus credential flags for a different
///    mode  → reject loudly.
/// 2. `--auth-mode entra-token` without `--azure-token-*`  → reject.
/// 3. `--auth-mode integrated` on a build without GSSAPI/SSPI feature
///    → reject with a rebuild hint.
///
/// When `--auth-mode` is omitted, infer from the supplied flags:
/// `--azure-token-file/-env` set → entra-token; otherwise sql-auth.
/// This keeps the simple two-flag invocation
/// (`--azure-token-file PATH` alone) working without forcing the
/// operator to also pass `--auth-mode entra-token`.
fn resolve_mssql_auth_mode(cli: &Cli) -> Result<AuthMode> {
    let token_set = cli.azure_token_file.is_some() || cli.azure_token_env.is_some();
    let pw_set = cli.password_file.is_some() || cli.password_env.is_some();

    let mode = match cli.auth_mode {
        Some(m) => m,
        None => {
            // Inference: if a token flag is present, the operator clearly
            // wants Entra; otherwise default to SQL auth.
            if token_set {
                AuthMode::EntraToken
            } else {
                AuthMode::SqlAuth
            }
        }
    };

    match mode {
        AuthMode::SqlAuth => {
            if token_set {
                bail!(
                    "DBP1604E --auth-mode=sql-auth conflicts with --azure-token-file/--azure-token-env. \
                     Drop the token flag or change to --auth-mode=entra-token."
                );
            }
        }
        AuthMode::EntraToken => {
            if !token_set {
                bail!(
                    "DBP1604E --auth-mode=entra-token requires --azure-token-file or --azure-token-env. \
                     Generate a token with: \
                     az account get-access-token --resource https://database.windows.net/ \
                     --query accessToken -o tsv > entra.token"
                );
            }
            if pw_set {
                bail!(
                    "DBP1604E --auth-mode=entra-token cannot be combined with --password-file/--password-env. \
                     Pick exactly one credential source."
                );
            }
        }
        AuthMode::CloudToken => {
            bail!(
                "DBP1005E --auth-mode=cloud-token is unavailable for SQL Server; use --auth-mode=entra-token for Azure SQL or a SQL Server credential"
            );
        }
        AuthMode::Integrated => {
            if token_set || pw_set {
                bail!(
                    "DBP1604E --auth-mode=integrated takes no credential flags. \
                     Kerberos/SSPI uses your existing TGT cache (Linux: kinit user@REALM first) \
                     or the current Windows session (Windows). \
                     Drop the --password-* / --azure-token-* flags."
                );
            }
            // The Integrated mode only works in a build compiled with
            // either tiberius's `integrated-auth-gssapi` (Linux) or
            // `winauth` (Windows) feature. The vanilla build excludes
            // these — error early with a clear rebuild hint.
            if !integrated_auth_available() {
                bail!(
                    "DBP1604E this build was compiled WITHOUT integrated authentication support. \
                     Rebuild with: \
                     `cargo build --features integrated-auth-gssapi --release` (Linux, requires libkrb5-dev) \
                     or `cargo build --features winauth --release` (Windows). See AUTH.md."
                );
            }
        }
    }
    Ok(mode)
}

/// Whether this build has the underlying tiberius feature for
/// integrated auth on the current target platform. False on the
/// vanilla build; true under `--features integrated-auth-gssapi`
/// (Linux) or `--features winauth` (Windows).
fn integrated_auth_available() -> bool {
    cfg!(any(
        all(unix, feature = "integrated-auth-gssapi"),
        all(windows, feature = "winauth")
    ))
}

fn engine_kind_for(uri: &str) -> Result<EngineKind> {
    let l = uri.trim_start().to_ascii_lowercase();
    if l.starts_with("postgresql://") || l.starts_with("postgres://") {
        Ok(EngineKind::Postgresql)
    } else if l.starts_with("mysql://") || l.starts_with("mariadb://") {
        Ok(EngineKind::MySQL)
    } else if l.starts_with("sqlserver://") || l.starts_with("mssql://") || l.starts_with("tds://")
    {
        Ok(EngineKind::Mssql)
    } else {
        // Echo only the scheme prefix (up to "://"), never the full URI —
        // it can carry an embedded password.
        let scheme_hint = match uri.find("://") {
            Some(end) => uri[..end].chars().take(64).collect::<String>(),
            None => "(no scheme)".to_string(),
        };
        Err(anyhow!(
            "DBP1002E --connect URI scheme not recognized; expected postgresql:// | mysql:// | sqlserver:// (got scheme: {scheme_hint}). Next: use a full database URI; short aliases such as pg:// are not accepted."
        ))
    }
}

fn print_preflight_for(
    cli: &Cli,
    redacted_uri: &str,
    host: &str,
    src: &SecretSource,
    mode: &str,
    engine_kind: EngineKind,
    resolved_user_source: &str,
) {
    let engine_str = match engine_kind {
        EngineKind::Postgresql => "postgresql",
        EngineKind::MySQL => "mysql",
        EngineKind::Mssql => "sqlserver",
    };
    eprintln!("{}", i18n::text("preflight.title"));
    preflight_line("preflight.engine", engine_str);
    preflight_line("preflight.connection", redacted_uri);
    preflight_line("preflight.host", host);
    preflight_line("preflight.mode", mode);
    if !cli.schema.is_empty() {
        preflight_line("preflight.schemas", cli.schema.join(", "));
    }
    let user_src_label = if matches!(cli.auth_mode, Some(AuthMode::Integrated)) {
        integrated_user_source().to_string()
    } else if cli.user.is_some() {
        "flag".to_string()
    } else if let Some(v) = &cli.user_env {
        format!("env:{v}")
    } else if let Some(p) = &cli.user_file {
        format!("file:{}", p.display())
    } else if resolved_user_source == "uri" {
        "uri".to_string()
    } else {
        "engine-default".to_string()
    };
    preflight_line("preflight.user_source", user_src_label);
    if let Some(expected) = &cli.expect_server_principal {
        preflight_line("preflight.expected_server_principal", expected);
    }
    preflight_line("preflight.password_source", src.audit_str());
    preflight_line("preflight.password_persisted", i18n::text("value.no"));
    preflight_line("preflight.output", cli.out.display());
    if let Some(dp) = &cli.deck {
        preflight_line(
            "preflight.deck",
            i18n::format(
                "value.generated_local",
                &[("path", dp.display().to_string())],
            ),
        );
        if let Some(level) = &cli.deck_confidentiality {
            preflight_line("preflight.deck_confidentiality", level.as_str());
        }
    }
    preflight_line("preflight.source_kind", &cli.source_kind);
    preflight_line("preflight.artifact_detail", cli.artifact_detail.as_str());
    preflight_line("preflight.tls_mode", &cli.tls_mode);
    if let Some(ca) = &cli.tls_ca {
        preflight_line("preflight.tls_ca", ca.display());
    }
    if let Some(cert) = &cli.tls_cert {
        preflight_line("preflight.tls_client_cert", cert.display());
    }
    if let Some(name) = &cli.tls_server_name {
        preflight_line("preflight.tls_server_name", name);
    }
    if cli.tls_skip_verify {
        preflight_line("preflight.tls_verify", i18n::text("value.tls_disabled"));
        if cli.i_know_what_im_doing {
            preflight_line(
                "preflight.tls_override",
                i18n::text("value.tls_override_ack"),
            );
        }
    }
    eprintln!();
    if cli.measure_compression {
        eprintln!("{}", i18n::text("tier2.heading"));
        preflight_bullet_format("tier2.rows", &[("rows", cli.sample_rows.to_string())]);
        preflight_bullet_format(
            "tier2.workers",
            &[(
                "workers",
                cli.compression_workers
                    .unwrap_or_else(default_compression_workers)
                    .to_string(),
            )],
        );
        preflight_bullet("tier2.compress");
        preflight_bullet("tier2.ratio");
        preflight_bullet("tier2.discard");
        preflight_bullet("tier2.block");
        preflight_bullet_format("tier2.wall", &[("seconds", cli.max_wall_secs.to_string())]);
        eprintln!();
    } else {
        eprintln!("{}", i18n::text("tier1.heading"));
        preflight_bullet_format("tier1.connection", &[("host", host.to_string())]);
        preflight_bullet("tier1.queries");
        preflight_bullet_format("tier1.write", &[("path", cli.out.display().to_string())]);
        eprintln!();
    }
    let length_fidelity = if cli.preserve_exact_lengths {
        LengthFidelity::Exact
    } else {
        cli.length_fidelity
    };
    if matches!(engine_kind, EngineKind::MySQL) {
        eprintln!(
            "{}",
            i18n::format(
                "mysql.length.heading",
                &[("mode", length_fidelity.label().to_string()),]
            )
        );
        match length_fidelity {
            LengthFidelity::Balanced => {
                preflight_bullet("mysql.length.balanced.declared");
                preflight_bullet("mysql.length.balanced.sampled");
            }
            LengthFidelity::Strict => {
                preflight_bullet("mysql.length.strict");
            }
            LengthFidelity::Exact => {
                preflight_bullet("mysql.length.exact");
                preflight_bullet("mysql.length.exact.warning");
            }
        }
        eprintln!();
    }
    if cli.dry_run {
        eprintln!("{}", i18n::text("dry.exit"));
    }
}

fn confirm_yes() -> Result<bool> {
    eprint!("{}", i18n::text("consent.prompt"));
    use std::io::{self, BufRead, Write};
    io::stderr().flush().ok();
    let stdin = io::stdin();
    let mut line = String::new();
    stdin
        .lock()
        .read_line(&mut line)
        .context("DBP1702E reading consent from stdin")?;
    let trimmed = line.trim().to_ascii_lowercase();
    Ok(matches!(trimmed.as_str(), "y" | "yes"))
}

fn unix_ms(t: SystemTime) -> u64 {
    t.duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
