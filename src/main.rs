// dbwarp-blueprint — customer-side schema-and-compression Blueprint tool.
//
// See README.md, SECURITY.md, FORMAT.md, AUDIT.md.
//
// Live collection opens the configured database-driver session. DNS and
// explicitly configured identity or TLS infrastructure may also be contacted.
// The tool has no telemetry, licence-check, or upload path. Read SECURITY.md
// and grep src/secret.rs for `\.expose\(\)` to verify the credential boundary.

mod artifacts;
mod audit;
mod banner;
mod deck;
mod engine_common;
mod engine_mssql;
mod engine_mysql;
mod engine_pg;
mod format;
mod i18n;
mod sample_compression;
mod sample_encode;
mod schema_scope;
mod secret;
mod statistics;
mod style;
mod terminal_palette;
mod terminal_style;
mod tls;
mod topology;
mod uri_authority;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::SystemTime;

use anyhow::{anyhow, bail, Context, Result};
use clap::{CommandFactory, Parser, ValueEnum};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::artifacts::ArtifactDetail;
use crate::audit::AuditLog;
/// The Windows system heap services large-block free/alloc cycles with
/// kernel round trips, which penalizes the sampling buffers; mimalloc keeps
/// them in user space on every platform.
#[global_allocator]
static GLOBAL_ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;
static ANONYMIZATION_KEY_SOURCE: OnceLock<&'static str> = OnceLock::new();

use crate::engine_mssql::{MssqlConnectParams, MssqlRunOpts};
use crate::engine_mysql::{LengthFidelity, MyConnectParams, MyRunOpts};
use crate::engine_pg::{PgConnectParams, PgRunOpts, SourceKind};
use crate::format::{emit_toml, BlueprintFile};
use crate::secret::{Secret, SecretSource};
use crate::tls::{TlsMode, TlsParams};

#[derive(ValueEnum, Debug, Clone, Copy, Default, PartialEq, Eq)]
enum CliColorMode {
    #[default]
    Auto,
    Always,
    Never,
}

impl From<CliColorMode> for terminal_style::ColorMode {
    fn from(mode: CliColorMode) -> Self {
        match mode {
            CliColorMode::Auto => Self::Auto,
            CliColorMode::Always => Self::Always,
            CliColorMode::Never => Self::Never,
        }
    }
}

#[derive(ValueEnum, Debug, Clone, Copy, Default, PartialEq, Eq)]
enum CliBannerMode {
    #[default]
    Auto,
    Half,
    Blocks,
    Ascii,
    Braille,
}

impl From<CliBannerMode> for banner::Dialect {
    fn from(mode: CliBannerMode) -> Self {
        match mode {
            CliBannerMode::Auto => Self::Auto,
            CliBannerMode::Half => Self::Half,
            CliBannerMode::Blocks => Self::Blocks,
            CliBannerMode::Ascii => Self::Ascii,
            CliBannerMode::Braille => Self::Braille,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DeckConfidentiality {
    Public,
    Internal,
    Confidential,
    Restricted,
    Custom(String),
}

impl DeckConfidentiality {
    fn as_str(&self) -> &str {
        match self {
            Self::Public => "public",
            Self::Internal => "internal",
            Self::Confidential => "confidential",
            Self::Restricted => "restricted",
            Self::Custom(value) => value,
        }
    }

    fn label(&self) -> &str {
        match self {
            Self::Public => i18n::text("deck.confidentiality.public"),
            Self::Internal => i18n::text("deck.confidentiality.internal"),
            Self::Confidential => i18n::text("deck.confidentiality.confidential"),
            Self::Restricted => i18n::text("deck.confidentiality.restricted"),
            Self::Custom(value) => value,
        }
    }
}

/// Default Tier-2 compression worker count: preserve minimum host impact.
/// Explicit --compression-workers enables bounded local parallelism.
fn default_compression_workers() -> u64 {
    1
}

fn parse_deck_confidentiality(value: &str) -> std::result::Result<DeckConfidentiality, String> {
    if value.is_empty() {
        return Err("the deck confidentiality label must not be empty".to_string());
    }
    if value.trim() != value {
        return Err(
            "the deck confidentiality label must not have leading or trailing whitespace"
                .to_string(),
        );
    }
    if value
        .chars()
        .any(|ch| ch.is_control() || i18n::is_forbidden_format_control(ch))
    {
        return Err(
            "the deck confidentiality label must not contain control or bidirectional formatting characters"
                .to_string(),
        );
    }
    let display_units: usize = value
        .chars()
        .map(|ch| if ch.is_ascii() { 1 } else { 2 })
        .sum();
    if display_units > 48 {
        return Err(
            "the deck confidentiality label is too long for the footer (maximum 48 display-width units)"
                .to_string(),
        );
    }

    Ok(match value.to_ascii_lowercase().as_str() {
        "public" => DeckConfidentiality::Public,
        "internal" => DeckConfidentiality::Internal,
        "confidential" => DeckConfidentiality::Confidential,
        "restricted" => DeckConfidentiality::Restricted,
        _ => DeckConfidentiality::Custom(value.to_string()),
    })
}

/// Customer-side schema-and-compression Blueprint tool for dbwarp pre-flight estimation.
///
/// Connects to a database, captures an anonymized Blueprint, and produces a
/// reviewable plain-text TOML file. No row content is read (Tier 1) unless
/// --measure-compression is explicitly enabled (Tier 2). Sampled bytes are
/// discarded; only documented aggregate compression, null-density,
/// cardinality/frequency, length, and style measurements are emitted.
#[derive(Parser, Debug)]
#[command(
    name = "dbwarp-blueprint",
    version,
    about = "Anonymized database Blueprint for migration estimation",
    long_about = "Collect sanitized database or structured-file Blueprint metadata for DBWarp sizing, synthetic fixture generation, and migration planning.\n\nLive database modes read catalog/statistics metadata. Tier 2 compression measurement is opt-in with --measure-compression --yes; sampled bytes are encoded and compressed in memory, then discarded. Offline modes read local TOML, Parquet, Avro, or bundle files and do not connect to a database.",
    after_long_help = r#"Examples:
  Table-catalog-only PostgreSQL Blueprint:
    dbwarp-blueprint --connect postgresql://db.internal/app \
      --user-file /etc/dbwarp/db.user --password-file /etc/dbwarp/db.pass \
      --artifact-detail none \
      --tls-mode verify-full --tls-ca /etc/pki/root.pem \
      --out blueprint.toml --audit-log blueprint.audit.txt --yes

  Add bounded compression sampling:
    dbwarp-blueprint --connect mysql://db.internal/app \
      --password-file /etc/dbwarp/mysql.pass \
      --measure-compression --sample-rows 1000 --max-wall-secs 300 \
      --out blueprint.toml --audit-log blueprint.audit.txt --yes

  Blueprint local Parquet without database credentials:
    dbwarp-blueprint --from-parquet /data/orders.parquet --out orders.blueprint.toml

  Collect a multi-source customer bundle:
    dbwarp-blueprint --batch-manifest customer.batch.toml \
      --out-dir customer-blueprint-bundle --dry-run
    dbwarp-blueprint --batch-manifest customer.batch.toml \
      --out-dir customer-blueprint-bundle --yes

  Extract one source/table from a bundle:
    dbwarp-blueprint --bundle-extract customer-blueprint-bundle/bundle.toml \
      --select source=erp_pg,table=table-042 --out table-042.blueprint.toml

Notes:
  - Passwords in connection URIs are refused; use --password-file or --password-env.
  - --dry-run validates arguments and prints the planned action without connecting.
  - Bundle selectors support source=ID, table=ID, engine=postgresql|mysql|sqlserver|parquet|avro, and tag=NAME.
  - See docs/COOKBOOK.md and docs/BATCH_AND_BUNDLES.md for operator recipes."#
)]
struct Cli {
    /// Presentation language for help, prompts, diagnostics, progress, and decks.
    /// Command names, option names, values, URIs, identifiers, and output schemas
    /// remain canonical English tokens in every language.
    #[arg(
        long,
        value_name = "LANG",
        value_parser = ["en", "de", "fr", "es", "pl", "ja", "zh"]
    )]
    lang: Option<String>,

    /// Terminal colour policy for human output: auto, always, or never.
    #[arg(
        long,
        value_enum,
        default_value_t = CliColorMode::Auto,
        hide_short_help = true
    )]
    color: CliColorMode,

    /// Print the DBWarp terminal lockup and exit.
    #[arg(long, hide_short_help = true)]
    banner: bool,

    /// Lockup rendering: auto, half, blocks, ascii, or braille.
    #[arg(
        long,
        value_enum,
        default_value_t = CliBannerMode::Auto,
        hide_short_help = true
    )]
    banner_mode: CliBannerMode,

    /// Database connection URI (postgresql://[user[:password]@]host[:port]/database).
    /// Required unless an offline input mode is used.
    #[arg(
        long,
        value_name = "URI",
        required_unless_present_any = [
            "from_toml",
            "from_parquet",
            "from_avro",
            "batch_manifest",
            "bundle_list",
            "bundle_extract",
            "bundle_pack",
            "banner"
        ],
        conflicts_with_all = [
            "from_toml",
            "from_parquet",
            "from_avro",
            "batch_manifest",
            "bundle_list",
            "bundle_extract",
            "bundle_pack"
        ]
    )]
    connect: Option<String>,

    /// Limit live capture to one database schema. Repeat --schema to include
    /// multiple schemas. Matching uses the connected engine's native schema
    /// comparison. The selector limits table sampling and schema-owned
    /// artifacts; database-wide topology evidence remains in scope.
    #[arg(
        long,
        value_name = "NAME",
        action = clap::ArgAction::Append,
        value_parser = schema_scope::parse_schema_name,
        requires = "connect",
        conflicts_with_all = [
            "from_toml",
            "from_parquet",
            "from_avro",
            "batch_manifest",
            "bundle_list",
            "bundle_extract",
            "bundle_pack",
            "banner"
        ]
    )]
    schema: Vec<String>,

    /// Output file path. Default: ./blueprint.toml.
    #[arg(long, value_name = "PATH", default_value = "blueprint.toml")]
    out: PathBuf,

    /// Also write a branded PowerPoint deck (.pptx) summarising the Blueprint.
    /// Generated locally from the same in-memory Blueprint that produces --out:
    /// no extra database read, no network, no third-party libraries. Deck bytes
    /// are deterministic for the same Blueprint content and pinned
    /// --generated-at; live captures also need the same protected customer
    /// key, source state, and options.
    #[arg(long, value_name = "PATH", hide_short_help = true)]
    deck: Option<PathBuf>,

    /// Optional confidentiality label shown in deck footers. Built-in values
    /// public, internal, confidential, and restricted are localized; any other
    /// safe value is emitted verbatim. Omit for no label.
    #[arg(
        long,
        value_name = "LEVEL",
        value_parser = parse_deck_confidentiality,
        requires = "deck",
        hide_short_help = true
    )]
    deck_confidentiality: Option<DeckConfidentiality>,

    /// Build a PowerPoint deck from an existing dbwarp-blueprint TOML file,
    /// without connecting to a database. Must be paired with --deck.
    #[arg(
        long = "from-toml",
        value_name = "PATH",
        requires = "deck",
        hide_short_help = true,
        conflicts_with_all = [
            "measure_compression",
            "no_rtt_probe",
            "user",
            "user_env",
            "user_file",
            "password_file",
            "password_env",
            "azure_token_file",
            "azure_token_env",
            "auth_mode",
            "generated_at",
            "tls_ca",
            "tls_cert",
            "tls_key",
            "tls_server_name",
            "tls_skip_verify",
            "i_know_what_im_doing"
        ]
    )]
    from_toml: Option<PathBuf>,

    /// Build a Blueprint TOML from Parquet file metadata without connecting to a database.
    /// By default this reads Parquet footer/row-group metadata only. With
    /// --measure-compression --yes it also decodes a bounded row sample.
    #[arg(
        long = "from-parquet",
        value_name = "PATH",
        hide_short_help = true,
        conflicts_with_all = [
            "from_toml",
            "from_avro",
            "deck",
            "no_rtt_probe",
            "user",
            "user_env",
            "user_file",
            "password_file",
            "password_env",
            "azure_token_file",
            "azure_token_env",
            "auth_mode",
            "tls_ca",
            "tls_cert",
            "tls_key",
            "tls_server_name",
            "tls_skip_verify",
            "i_know_what_im_doing"
        ]
    )]
    from_parquet: Option<PathBuf>,

    /// Build a Blueprint TOML from an Avro object container without connecting to a database.
    /// Avro has no footer row count, so this walks the container to count records.
    /// With --measure-compression --yes it also decodes a bounded row sample.
    #[arg(
        long = "from-avro",
        value_name = "PATH",
        hide_short_help = true,
        conflicts_with_all = [
            "from_toml",
            "from_parquet",
            "deck",
            "no_rtt_probe",
            "user",
            "user_env",
            "user_file",
            "password_file",
            "password_env",
            "azure_token_file",
            "azure_token_env",
            "auth_mode",
            "tls_ca",
            "tls_cert",
            "tls_key",
            "tls_server_name",
            "tls_skip_verify",
            "i_know_what_im_doing"
        ]
    )]
    from_avro: Option<PathBuf>,

    /// Run multiple Blueprint captures from a manifest and write a bundle directory.
    /// Requires --out-dir. A non-dry-run batch requires --yes.
    #[arg(
        long = "batch-manifest",
        value_name = "PATH",
        hide_short_help = true,
        conflicts_with_all = [
            "from_toml",
            "from_parquet",
            "from_avro",
            "bundle_list",
            "bundle_extract",
            "bundle_pack",
            "deck",
            "connect"
        ]
    )]
    batch_manifest: Option<PathBuf>,

    /// Output directory for --batch-manifest. Receives bundle.toml, blueprints/, and audits/.
    #[arg(long = "out-dir", value_name = "DIR", hide_short_help = true)]
    out_dir: Option<PathBuf>,

    /// List bundle sources and matching table details from a bundle TOML.
    #[arg(
        long = "bundle-list",
        value_name = "PATH",
        hide_short_help = true,
        conflicts_with_all = [
            "from_toml",
            "from_parquet",
            "from_avro",
            "batch_manifest",
            "bundle_extract",
            "bundle_pack",
            "deck",
            "connect"
        ]
    )]
    bundle_list: Option<PathBuf>,

    /// Extract a selected source/table from a bundle TOML to --out.
    /// Use --select source=ID and optionally table=ID.
    #[arg(
        long = "bundle-extract",
        value_name = "PATH",
        hide_short_help = true,
        conflicts_with_all = [
            "from_toml",
            "from_parquet",
            "from_avro",
            "batch_manifest",
            "bundle_list",
            "bundle_pack",
            "deck",
            "connect"
        ]
    )]
    bundle_extract: Option<PathBuf>,

    /// Pack a bundle directory or bundle TOML into one embedded bundle TOML at --out.
    /// Review operator-supplied metadata and transfer only through an approved channel.
    #[arg(
        long = "bundle-pack",
        value_name = "PATH",
        hide_short_help = true,
        conflicts_with_all = [
            "from_toml",
            "from_parquet",
            "from_avro",
            "batch_manifest",
            "bundle_list",
            "bundle_extract",
            "deck",
            "connect"
        ]
    )]
    bundle_pack: Option<PathBuf>,

    /// Bundle selector: source=ID, table=ID, engine=postgresql, tag=NAME.
    /// Can be passed multiple times; all predicates are combined.
    #[arg(
        long = "select",
        value_name = "KEY=VALUE[,KEY=VALUE]",
        hide_short_help = true
    )]
    select: Vec<String>,

    /// Source kind annotation propagated to the report. Drives the
    /// estimator's compression_source_confidence on the dbwarp side.
    #[arg(
        long,
        value_name = "KIND",
        default_value = "production",
        hide_short_help = true
    )]
    source_kind: String,

    /// Enable Tier 2 by sampling rows locally. Adds documented aggregate
    /// compression, null-density, cardinality/frequency, length, and style
    /// measurements. Sampled values are never written to disk or output.
    #[arg(long)]
    measure_compression: bool,

    /// Non-table artifact capture: none, summary, graph, or analyzed.
    /// summary (default) emits bounded object counts without names or
    /// definitions. graph emits anonymous object/dependency records. analyzed
    /// also reads definitions transiently to emit bounded language-feature
    /// bands. graph/analyzed require --yes.
    #[arg(long, value_enum, default_value_t = ArtifactDetail::Summary)]
    artifact_detail: ArtifactDetail,

    /// Length metadata policy for live MySQL capture.
    /// balanced (default): exact schema/index lengths plus <=~3.2% relative
    /// rounding for sampled value lengths; strict: legacy coarse anonymization;
    /// exact: preserve all lengths exactly and require --yes.
    #[arg(
        long,
        value_enum,
        default_value_t = LengthFidelity::Balanced,
        hide_short_help = true
    )]
    length_fidelity: LengthFidelity,

    /// Compatibility alias for --length-fidelity exact.
    /// Requires --yes. Prefer --length-fidelity for new automation.
    #[arg(long, requires = "yes", hide_short_help = true)]
    preserve_exact_lengths: bool,

    /// Pre-answer the interactive consent prompt. Required for exact-length
    /// metadata and for non-interactive Tier 2 automation; an attached terminal
    /// may instead review the pre-flight plan and answer yes for Tier 2.
    #[arg(long)]
    yes: bool,

    /// Tier 2: rows per table to sample.
    #[arg(
        long,
        default_value_t = engine_common::DEFAULT_SAMPLE_ROWS,
        hide_short_help = true,
        value_parser = clap::value_parser!(u64).range(1..)
    )]
    sample_rows: u64,

    /// Live Tier 2: bounded local compression workers. Database sampling
    /// remains sequential. Default: 1; increase explicitly to use more local
    /// CPU.
    #[arg(
        long,
        value_name = "N",
        requires = "measure_compression",
        hide_short_help = true,
        conflicts_with_all = [
            "from_toml",
            "from_parquet",
            "from_avro",
            "batch_manifest",
            "bundle_list",
            "bundle_extract",
            "bundle_pack",
            "banner"
        ],
        value_parser = clap::value_parser!(u64).range(1..=32)
    )]
    compression_workers: Option<u64>,

    /// Hard wall-time limit for the complete live capture, including connect,
    /// catalog reads, RTT probes, and Tier 2 sampling, in seconds.
    #[arg(
        long,
        default_value_t = engine_pg::DEFAULT_SAMPLE_TIMEOUT_SECS,
        hide_short_help = true,
        value_parser = clap::value_parser!(u64).range(1..)
    )]
    max_wall_secs: u64,

    /// Skip the customer-side RTT probe (5× SELECT 1 after connect).
    /// By default the probe runs and emits a `[network]` block in the
    /// Blueprint file with `connect_total_ms`, `query_rtt_ms_p50`, and
    /// `query_rtt_ms_p95`.
    /// Disable if your DBA forbids any non-catalog queries against
    /// production. The probe never reads row data — each query
    /// returns the constant integer 1.
    #[arg(long = "no-rtt-probe", default_value_t = false, hide_short_help = true)]
    no_rtt_probe: bool,

    /// Database username (overrides any user in the --connect URI). Useful
    /// when the username contains characters awkward to URI-encode
    /// (`DOMAIN\user`, `app+monitor`, `user@corp.example.com`).
    #[arg(long, value_name = "USER")]
    user: Option<String>,

    /// Read username from a named environment variable. Tool reads ONLY the
    /// var you name.
    #[arg(long, value_name = "VAR", hide_short_help = true)]
    user_env: Option<String>,

    /// Read username from a file. Trailing whitespace stripped.
    #[arg(long, value_name = "PATH")]
    user_file: Option<PathBuf>,

    /// Read password from a file. Mode must not allow group/other read.
    #[arg(long, value_name = "PATH")]
    password_file: Option<PathBuf>,

    /// Read password from a named environment variable. Tool reads ONLY the
    /// var you name — no fallback to PGPASSWORD/MYSQL_PWD/etc.
    #[arg(long, value_name = "VAR")]
    password_env: Option<String>,

    /// Read a 32-byte or 64-hex-character customer-held HMAC key from a file.
    /// Reusing the same key preserves anonymous object IDs across runs. When
    /// omitted, a fresh process-local key prevents offline name guessing.
    #[arg(
        long,
        value_name = "PATH",
        hide_short_help = true,
        conflicts_with_all = [
            "from_toml",
            "from_parquet",
            "from_avro",
            "bundle_list",
            "bundle_extract",
            "bundle_pack",
            "banner"
        ]
    )]
    anonymization_key_file: Option<PathBuf>,

    /// Read a Microsoft Entra ID (Azure AD) access token from a file.
    /// SQL Server engine only. Mutually exclusive with --password-file
    /// and --password-env. The token is consumed once via the same
    /// zeroizing wrapper as a password. File mode must not allow
    /// group/other read on Unix. Generate with:
    ///   az account get-access-token --resource https://database.windows.net/ \
    ///       --query accessToken -o tsv > entra.token
    #[arg(long, value_name = "PATH", hide_short_help = true)]
    azure_token_file: Option<PathBuf>,

    /// Read a Microsoft Entra ID access token from a named environment
    /// variable. SQL Server engine only. Tool reads ONLY the var you
    /// name. Mutually exclusive with --password-file/-env.
    #[arg(long, value_name = "VAR", hide_short_help = true)]
    azure_token_env: Option<String>,

    /// Database authentication method. Optional — defaults to sql-auth,
    /// except SQL Server infers entra-token from --azure-token-*. Use
    /// cloud-token for an externally generated PostgreSQL/MySQL managed-service
    /// token supplied by exactly one --password-file/-env; this mode requires
    /// --tls-mode verify-full. integrated is SQL Server-only. Credential flags
    /// must match the selected mode.
    #[arg(long = "auth-mode", value_name = "MODE")]
    auth_mode: Option<AuthMode>,

    /// SQL Server only: require ORIGINAL_LOGIN() on the established session
    /// to match this principal before any catalog capture is attempted. The
    /// expected and observed identity values are written only to the local
    /// audit log.
    #[arg(
        long = "expect-server-principal",
        value_name = "PRINCIPAL",
        requires = "connect",
        hide_short_help = true,
        conflicts_with_all = [
            "from_toml",
            "from_parquet",
            "from_avro",
            "batch_manifest",
            "bundle_list",
            "bundle_extract",
            "bundle_pack",
            "banner"
        ]
    )]
    expect_server_principal: Option<String>,

    /// Write an atomic copy of the audit log to this file (in addition to stderr).
    #[arg(long, value_name = "PATH")]
    audit_log: Option<PathBuf>,

    /// Pin the Blueprint file's `generated_at` field to this value (e.g.
    /// `"2026-04-26T00:00:00Z"`). Byte-identical live captures additionally
    /// require the same protected customer key, source state, options, and
    /// producer. Without this flag the current UTC time at run start is used.
    /// Recorded in the audit log when set. This CLI flag is the only way to
    /// pin the timestamp — no environment variable is consulted, matching
    /// the "no env vars read by default" trust contract.
    #[arg(long, value_name = "ISO8601_UTC", hide_short_help = true)]
    generated_at: Option<String>,

    /// Print the pre-flight plan and exit without connecting.
    #[arg(long)]
    dry_run: bool,

    // ---- TLS ----
    /// TLS mode: disable | prefer | require | verify-ca | verify-full (default).
    /// prefer is loopback-only; non-verifying remote modes require explicit approval.
    #[arg(long, value_name = "MODE", default_value = "verify-full")]
    tls_mode: String,

    /// Path to trusted CA PEM. PostgreSQL/MySQL accept a bundle; SQL Server
    /// accepts exactly one certificate. The supplied file replaces default roots.
    #[arg(long, value_name = "PATH")]
    tls_ca: Option<PathBuf>,

    /// Path to a client TLS certificate (PEM). Used for PostgreSQL/MySQL mTLS —
    /// must be paired with --tls-key. SQL Server client certificates are
    /// unavailable.
    #[arg(long, value_name = "PATH", hide_short_help = true)]
    tls_cert: Option<PathBuf>,

    /// Path to a client TLS private key (PEM, PKCS8/RSA/SEC1). Paired with
    /// --tls-cert for PostgreSQL/MySQL; unavailable for SQL Server.
    #[arg(long, value_name = "PATH", hide_short_help = true)]
    tls_key: Option<PathBuf>,

    /// Reserved for a future release. Passing this currently fails loudly
    /// rather than being silently ignored by engine-specific drivers.
    #[arg(long, value_name = "NAME", hide_short_help = true)]
    tls_server_name: Option<String>,

    /// Disable certificate verification entirely. Loud — emits a stderr
    /// warning, recorded in audit. Refused on non-loopback addresses unless
    /// --i-know-what-im-doing is also set.
    #[arg(long, hide_short_help = true)]
    tls_skip_verify: bool,

    /// Override the loopback-only safety for an explicitly selected remote
    /// TLS policy that does not verify database identity.
    /// **DO NOT USE IN PRODUCTION.**
    #[arg(long, hide_short_help = true)]
    i_know_what_im_doing: bool,
}

const I18N_STARTUP_EXIT_CODE: u8 = 2;

const GENERATED_HELP_PHRASES: &[&str] = &[
    "Usage:",
    "Commands:",
    "Arguments:",
    "Options:",
    "Possible values:",
    "[default:",
    "[possible values:",
    "[alias:",
    "[aliases:",
];

fn argv_lang_hint(args: &[String]) -> Option<&str> {
    let mut index = 1usize;
    while index < args.len() {
        let arg = args[index].as_str();
        if arg == "--" {
            return None;
        }
        if let Some(value) = arg.strip_prefix("--lang=") {
            return Some(value);
        }
        if arg == "--lang" {
            return args.get(index + 1).map(String::as_str);
        }
        index += 1;
    }
    None
}

fn canonical_help_phrases(root: &clap::Command) -> std::collections::BTreeSet<String> {
    fn insert(output: &mut std::collections::BTreeSet<String>, value: Option<String>) {
        if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
            output.insert(value);
        }
    }

    fn collect(command: &clap::Command, output: &mut std::collections::BTreeSet<String>) {
        insert(output, command.get_about().map(ToString::to_string));
        insert(output, command.get_long_about().map(ToString::to_string));
        insert(output, command.get_before_help().map(ToString::to_string));
        insert(
            output,
            command.get_before_long_help().map(ToString::to_string),
        );
        insert(output, command.get_after_help().map(ToString::to_string));
        insert(
            output,
            command.get_after_long_help().map(ToString::to_string),
        );
        for argument in command.get_arguments() {
            insert(output, argument.get_help().map(ToString::to_string));
            insert(output, argument.get_long_help().map(ToString::to_string));
            insert(output, argument.get_help_heading().map(ToString::to_string));
            if let Some(values) = argument.get_value_parser().possible_values() {
                for value in values {
                    insert(output, value.get_help().map(ToString::to_string));
                }
            }
        }
        insert(
            output,
            command
                .get_subcommand_help_heading()
                .map(ToString::to_string),
        );
        for subcommand in command.get_subcommands() {
            collect(subcommand, output);
        }
    }

    let mut output = GENERATED_HELP_PHRASES
        .iter()
        .map(|source| (*source).to_string())
        .collect();
    collect(root, &mut output);
    output
}

fn localize_clap_argument(
    mut argument: clap::Arg,
    locale: i18n::Locale,
) -> std::result::Result<clap::Arg, String> {
    if let Some(help) = argument.get_help().map(ToString::to_string) {
        argument = argument.help(i18n::help_phrase(locale, &help)?);
    }
    if let Some(help) = argument.get_long_help().map(ToString::to_string) {
        argument = argument.long_help(i18n::help_phrase(locale, &help)?);
    }
    if let Some(heading) = argument.get_help_heading().map(ToString::to_string) {
        argument = argument.help_heading(i18n::help_phrase(locale, &heading)?);
    }
    let possible_values = argument.get_possible_values();
    if possible_values
        .iter()
        .any(|value| value.get_help().is_some())
    {
        let mut localized_values = Vec::with_capacity(possible_values.len());
        for mut value in possible_values {
            if let Some(help) = value.get_help().map(ToString::to_string) {
                value = value.help(i18n::help_phrase(locale, &help)?);
            }
            localized_values.push(value);
        }
        argument =
            argument.value_parser(clap::builder::PossibleValuesParser::new(localized_values));
    }
    Ok(argument)
}

fn localize_clap_command(
    mut command: clap::Command,
    locale: i18n::Locale,
) -> std::result::Result<clap::Command, String> {
    if let Some(value) = command.get_about().map(ToString::to_string) {
        command = command.about(i18n::help_phrase(locale, &value)?);
    }
    if let Some(value) = command.get_long_about().map(ToString::to_string) {
        command = command.long_about(i18n::help_phrase(locale, &value)?);
    }
    if let Some(value) = command.get_before_help().map(ToString::to_string) {
        command = command.before_help(i18n::help_phrase(locale, &value)?);
    }
    if let Some(value) = command.get_before_long_help().map(ToString::to_string) {
        command = command.before_long_help(i18n::help_phrase(locale, &value)?);
    }
    if let Some(value) = command.get_after_help().map(ToString::to_string) {
        command = command.after_help(i18n::help_phrase(locale, &value)?);
    }
    if let Some(value) = command.get_after_long_help().map(ToString::to_string) {
        command = command.after_long_help(i18n::help_phrase(locale, &value)?);
    }
    if let Some(value) = command
        .get_subcommand_help_heading()
        .map(ToString::to_string)
    {
        command = command.subcommand_help_heading(i18n::help_phrase(locale, &value)?);
    }

    let mut failure = None;
    command = command.mut_args(
        |argument| match localize_clap_argument(argument.clone(), locale) {
            Ok(localized) => localized,
            Err(error) => {
                failure.get_or_insert(error);
                argument
            }
        },
    );
    if let Some(error) = failure {
        return Err(error);
    }
    let mut failure = None;
    command = command.mut_subcommands(|subcommand| {
        match localize_clap_command(subcommand.clone(), locale) {
            Ok(localized) => localized,
            Err(error) => {
                failure.get_or_insert(error);
                subcommand
            }
        }
    });
    if let Some(error) = failure {
        return Err(error);
    }
    Ok(command)
}

fn render_localized_clap_help(args: &[String], locale: i18n::Locale) -> Result<String> {
    let mut command = Cli::command().term_width(0);
    command.build();
    let mut command =
        localize_clap_command(command, locale).map_err(|detail| anyhow!("DBP1010E {detail}"))?;
    let error = command
        .try_get_matches_from_mut(args)
        .expect_err("help/version invocation must terminate through Clap");
    if !matches!(
        error.kind(),
        clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
    ) {
        bail!("DBP1010E localized command rejected canonical help invocation: {error}");
    }
    i18n::localize_help_scaffolding(locale, &error.to_string())
        .map_err(|detail| anyhow!("DBP1010E {detail}"))
}

fn validate_startup_i18n() -> std::result::Result<(), String> {
    let mut command = Cli::command().term_width(0);
    command.build();
    i18n::validate_catalogs(&canonical_help_phrases(&command))
}

fn main() -> std::process::ExitCode {
    let raw_args = std::env::args().collect::<Vec<_>>();
    terminal_style::configure(terminal_style::mode_from_args(&raw_args));

    // Install rustls's default crypto provider once, before any TLS handshake.
    //
    // ring is selected via Cargo.toml feature ["ring"] and is the *active*
    // provider at runtime. aws-lc-rs / aws-lc-sys ARE present in the
    // vendored dep graph (pulled in transitively by `rustls` / `webpki`)
    // and are part of the audit surface, but their default-provider
    // installation is suppressed by this explicit ring install_default()
    // call — and `rustls/rustls-webpki` are configured to prefer the
    // pure-Rust implementations. Run `cargo tree -i aws-lc-rs` to see
    // the transitive presence; it does not run code in the live binary.
    let _ = rustls::crypto::ring::default_provider().install_default();

    if let Err(error) = validate_startup_i18n() {
        let message = format!("DBP1010E embedded localization validation failed: {error}\n");
        let rendered =
            terminal_style::render_status(&message, terminal_style::OutputStream::Stderr);
        let _ = terminal_style::write_stderr(&rendered);
        return std::process::ExitCode::from(I18N_STARTUP_EXIT_CODE);
    }

    match run_main() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            if format!("{error:#}").starts_with("DBP1011E") {
                let message = format!(
                    "{}\n",
                    render_operator_error("DBP1011E", "Command-line arguments are invalid", &error,)
                );
                let rendered =
                    terminal_style::render_status(&message, terminal_style::OutputStream::Stderr);
                let _ = terminal_style::write_stderr(&rendered);
                std::process::ExitCode::from(2)
            } else {
                std::process::ExitCode::FAILURE
            }
        }
    }
}

fn run_main() -> Result<()> {
    let raw_args = std::env::args().collect::<Vec<_>>();
    let locale = i18n::resolve_locale(argv_lang_hint(&raw_args));
    i18n::set_active_locale(locale);
    let cli = match Cli::try_parse_from(&raw_args) {
        Ok(cli) => cli,
        Err(error)
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) =>
        {
            let plain = if locale == i18n::Locale::En {
                error.to_string()
            } else {
                render_localized_clap_help(&raw_args, locale)?
            };
            let kind = if matches!(error.kind(), clap::error::ErrorKind::DisplayVersion) {
                banner::Kind::Lockup
            } else {
                banner::Kind::Help
            };
            let lockup = banner::render(
                kind,
                terminal_style::OutputStream::Stdout,
                banner::dialect_from_args(&raw_args),
            );
            let rendered =
                terminal_style::render_help(&plain, terminal_style::OutputStream::Stdout);
            terminal_style::write_stdout(&format!("{lockup}{rendered}"))
                .context("DBP1011E printing command-line help")?;
            return Ok(());
        }
        Err(error) => {
            emit_command_line_failure_audit(&raw_args);
            bail!("DBP1011E {error}")
        }
    };
    terminal_style::configure(cli.color.into());
    if cli.banner {
        let lockup = banner::render(
            banner::Kind::Help,
            terminal_style::OutputStream::Stdout,
            cli.banner_mode.into(),
        );
        terminal_style::write_stdout(&lockup).context("DBP1011E printing command-line help")?;
        return Ok(());
    }
    if terminal_style::is_terminal(terminal_style::OutputStream::Stdout) {
        let lockup = banner::render(
            banner::Kind::Lockup,
            terminal_style::OutputStream::Stdout,
            cli.banner_mode.into(),
        );
        // Branding is presentation-only. A terminal that disappears between
        // the is-terminal check and this write must not prevent a capture.
        let _ = terminal_style::write_stdout(&lockup);
    }
    let started = SystemTime::now();
    let started_unix_ms = unix_ms(started);

    // Construct the audit log immediately after successful CLI parsing so it
    // survives every subsequent operational exit path.
    // Even if `run_with_audit` fails partway through, we emit the
    // audit (with the error stage recorded) before propagating the
    // anyhow::Error to the OS. The customer always gets a forensic
    // record of what was attempted.
    let mode = if cli.from_toml.is_some() {
        "deck-from-toml"
    } else if cli.batch_manifest.is_some() {
        "blueprint-batch"
    } else if cli.bundle_list.is_some() || cli.bundle_extract.is_some() || cli.bundle_pack.is_some()
    {
        "blueprint-bundle"
    } else if cli.from_parquet.is_some() || cli.from_avro.is_some() {
        "blueprint-from-file"
    } else if cli.measure_compression {
        "tier-2"
    } else {
        "tier-1"
    };
    let mut audit = AuditLog::new(mode, started_unix_ms);
    audit.generated_at_pin = cli.generated_at.clone();

    let result = run_with_audit(&cli, &mut audit);

    // Stamp outcome before finalizing.
    if let Err(e) = &result {
        let plain_error = render_operator_error("DBP0001E", "dbwarp-blueprint failed", e);
        let terminal_error = terminal_style::render_status(
            &format!("{plain_error}\n"),
            terminal_style::OutputStream::Stderr,
        );
        let _ = terminal_style::write_stderr(&terminal_error);
        // Flatten to one short audit line. The terminal message above carries
        // the causal chain; the audit stays bounded and easy to scan.
        let mut msg = plain_error.replace('\n', " | ");
        // Truncate hard so a chatty error chain can't blow up the audit file.
        truncate_audit_message(&mut msg, 240);
        audit.mark_failure(msg);
    }
    record_runtime_env_reads(&mut audit);
    audit.finalize(unix_ms(SystemTime::now()));

    let rendered = audit.render();
    eprint!("{rendered}");
    if let Some(p) = &cli.audit_log {
        let write_result = atomic_write_bytes(p, rendered.as_bytes());
        // If we can't write the audit log, surface that as a warning but
        // do NOT clobber the underlying run result. A failed audit-log
        // write must not mask a successful run, and must not mask a
        // primary failure either.
        if let Err(e) = write_result {
            eprintln!(
                "{}",
                i18n::format(
                    "warning.audit_write",
                    &[
                        ("code", "DBP1504W".to_string()),
                        ("path", p.display().to_string()),
                        ("error", e.to_string()),
                    ]
                )
            );
        }
    }

    if result.is_ok() {
        if let Some(fidelity) = audit.fidelity.as_ref() {
            print_fidelity_summary(fidelity);
        }
    }

    result
}

fn print_fidelity_summary(fidelity: &dbwarp_blueprint_core::BlueprintFidelityEstimate) {
    let band = match fidelity.band {
        "high" => i18n::text("status.fidelity_band.high"),
        "good" => i18n::text("status.fidelity_band.good"),
        "moderate" => i18n::text("status.fidelity_band.moderate"),
        _ => i18n::text("status.fidelity_band.low"),
    };
    println!(
        "{}",
        i18n::format(
            "status.fidelity",
            &[
                ("overall", fidelity.overall_score.to_string()),
                ("band", band.to_string()),
                ("structure", fidelity.structure_score.to_string()),
                ("sizing", fidelity.sizing_score.to_string()),
                ("columns", fidelity.column_statistics_score.to_string()),
                ("relationships", fidelity.relationship_score.to_string()),
                ("artifacts", fidelity.artifact_score.to_string()),
            ]
        )
    );
    if !fidelity.limitations.is_empty() {
        println!(
            "{}",
            i18n::format(
                "status.fidelity_limitations",
                &[("limitations", fidelity.limitations.join(", "))]
            )
        );
    }
    println!("{}", i18n::text("status.fidelity_qualification"));
}

fn audit_log_path_hint(args: &[String]) -> Option<PathBuf> {
    let mut index = 1usize;
    while index < args.len() {
        if let Some(value) = args[index].strip_prefix("--audit-log=") {
            return (!value.is_empty()).then(|| PathBuf::from(value));
        }
        if args[index] == "--audit-log" {
            return args.get(index + 1).map(PathBuf::from);
        }
        index += 1;
    }
    None
}

fn record_runtime_env_reads(audit: &mut AuditLog) {
    for name in i18n::env_vars_read()
        .into_iter()
        .chain(terminal_palette::env_vars_read())
        .chain(banner::env_vars_read())
    {
        audit.record_env_var_read(name);
    }
}

fn emit_command_line_failure_audit(args: &[String]) {
    let started = unix_ms(SystemTime::now());
    let mut audit = AuditLog::new("command-line", started);
    audit.connection.uri_redacted = "(not parsed; no connection attempted)".to_string();
    audit.connection.auth = "(not acquired)".to_string();
    audit.connection.tls_mode = "(not applied)".to_string();
    audit.mark_failure("DBP1011E command-line arguments are invalid");
    record_runtime_env_reads(&mut audit);
    audit.finalize(unix_ms(SystemTime::now()));
    let rendered = audit.render();
    eprint!("{rendered}");
    if let Some(path) = audit_log_path_hint(args) {
        if let Err(error) = atomic_write_bytes(&path, rendered.as_bytes()) {
            eprintln!(
                "{}",
                i18n::format(
                    "warning.audit_write",
                    &[
                        ("code", "DBP1504W".to_string()),
                        ("path", path.display().to_string()),
                        ("error", error.to_string()),
                    ]
                )
            );
        }
    }
}

fn render_operator_error(default_code: &str, default_text: &str, err: &anyhow::Error) -> String {
    let chain = err
        .chain()
        .map(|cause| cause.to_string())
        .collect::<Vec<_>>();
    let Some(first) = chain.first() else {
        return format!("{default_code} {default_text}");
    };
    // An outer engine/orchestration code must not hide a more specific stable
    // code carried by an inner decision-boundary error. Prefer the deepest
    // coded cause while retaining every other cause in the rendered chain.
    let coded = chain
        .iter()
        .enumerate()
        .rev()
        .find(|(_, cause)| starts_with_message_code(cause, "DBP"));
    let (selected_index, code, detail) = if let Some((index, cause)) = coded {
        let code = cause.split_whitespace().next().unwrap_or(default_code);
        (Some(index), code, cause[code.len()..].trim())
    } else {
        (None, default_code, first.as_str())
    };
    let locale = i18n::active_locale();
    let summary = i18n::diagnostic_field(locale, code, i18n::DiagnosticField::Summary)
        .or_else(|| i18n::diagnostic_field(locale, default_code, i18n::DiagnosticField::Summary))
        .unwrap_or(default_text);
    let cause = i18n::diagnostic_field(locale, code, i18n::DiagnosticField::Cause)
        .or_else(|| i18n::diagnostic_field(locale, default_code, i18n::DiagnosticField::Cause));
    let action = i18n::diagnostic_field(locale, code, i18n::DiagnosticField::Action)
        .or_else(|| i18n::diagnostic_field(locale, default_code, i18n::DiagnosticField::Action));
    let mut out = format!("{code} {summary}");
    if let Some(cause) = cause {
        out.push_str(&format!("\n{}: {cause}", i18n::text("diag.cause")));
    }
    if let Some(action) = action {
        out.push_str(&format!("\n{}: {action}", i18n::text("diag.action")));
    }
    if !detail.is_empty() && detail != summary {
        out.push_str(&format!("\n{}: {detail}", i18n::text("diag.detail")));
    }
    for (index, cause) in chain.iter().enumerate() {
        if selected_index == Some(index) || (selected_index.is_none() && index == 0) {
            continue;
        }
        out.push_str(&format!("\n{}: ", i18n::text("diag.chain")));
        out.push_str(cause);
    }
    out
}

fn starts_with_message_code(text: &str, prefix: &str) -> bool {
    let Some(code) = text.split_whitespace().next() else {
        return false;
    };
    let bytes = code.as_bytes();
    bytes.len() == 8
        && code.starts_with(prefix)
        && bytes[3..7].iter().all(u8::is_ascii_digit)
        && matches!(bytes[7], b'E' | b'W' | b'I')
}

fn truncate_audit_message(message: &mut String, max_bytes: usize) {
    if message.len() <= max_bytes {
        return;
    }
    let mut boundary = max_bytes;
    while !message.is_char_boundary(boundary) {
        boundary -= 1;
    }
    message.truncate(boundary);
    message.push('…');
}

fn finish_live_capture(
    result: std::result::Result<Result<BlueprintFile>, tokio::time::error::Elapsed>,
    audit: &mut AuditLog,
    engine: &str,
    started: std::time::Instant,
    max_wall_secs: u64,
    context: &'static str,
) -> Result<BlueprintFile> {
    match result {
        Ok(Ok(blueprint)) => Ok(blueprint),
        Ok(Err(error)) => {
            audit.record_query_failure(
                &format!("{engine} live capture (driver detail redacted)"),
                started.elapsed().as_millis() as u64,
            );
            Err(error).context(context)
        }
        Err(_) => {
            audit.record_query_failure(
                &format!("{engine} live capture (hard wall-time limit reached)"),
                started.elapsed().as_millis() as u64,
            );
            bail!(
                "DBP1419E {engine} live capture exceeded the hard --max-wall-secs limit of \
                 {max_wall_secs} seconds. The client dropped its database connection; PostgreSQL \
                 and MySQL also enforce a server-side statement limit, while SQL Server enforces \
                 a server-side lock-wait limit. Rerun with a larger reviewed limit or investigate \
                 the database/network stall."
            )
        }
    }
}

fn block_on_with_timeout<F>(
    runtime: &tokio::runtime::Runtime,
    duration: std::time::Duration,
    future: F,
) -> std::result::Result<F::Output, tokio::time::error::Elapsed>
where
    F: std::future::Future,
{
    // Construct the Tokio timer only after block_on has entered this runtime.
    // Creating `timeout(...)` as a block_on argument panics because no reactor
    // is active while Rust evaluates the argument expression.
    runtime.block_on(async move { tokio::time::timeout(duration, future).await })
}

/// Fully resolved top-level operation. Clap validates option conflicts; this
/// enum makes the application dispatch exhaustive and prevents new modes from
/// being hidden in a long sequence of early-return conditionals.
enum CommandMode<'a> {
    Batch(&'a Path),
    BundleList(&'a Path),
    BundleExtract(&'a Path),
    BundlePack(&'a Path),
    DeckFromToml(&'a Path),
    StructuredFile(&'a Path, StructuredFileBlueprintKind),
    LiveCapture(&'a str),
}

fn command_mode(cli: &Cli) -> Result<CommandMode<'_>> {
    if let Some(path) = cli.batch_manifest.as_deref() {
        return Ok(CommandMode::Batch(path));
    }
    if let Some(path) = cli.bundle_list.as_deref() {
        return Ok(CommandMode::BundleList(path));
    }
    if let Some(path) = cli.bundle_extract.as_deref() {
        return Ok(CommandMode::BundleExtract(path));
    }
    if let Some(path) = cli.bundle_pack.as_deref() {
        return Ok(CommandMode::BundlePack(path));
    }
    if let Some(path) = cli.from_toml.as_deref() {
        return Ok(CommandMode::DeckFromToml(path));
    }
    if let Some(path) = cli.from_parquet.as_deref() {
        return Ok(CommandMode::StructuredFile(
            path,
            StructuredFileBlueprintKind::Parquet,
        ));
    }
    if let Some(path) = cli.from_avro.as_deref() {
        return Ok(CommandMode::StructuredFile(
            path,
            StructuredFileBlueprintKind::Avro,
        ));
    }
    cli.connect
        .as_deref()
        .map(CommandMode::LiveCapture)
        .ok_or_else(|| {
            anyhow!(
                "DBP1000E --connect is required unless an offline input mode is used. \
                 Next: pass --connect URI, or use --from-toml/--from-parquet/--from-avro/--batch-manifest/--bundle-*."
            )
        })
}

fn run_with_audit(cli: &Cli, audit: &mut AuditLog) -> Result<()> {
    let connect = match command_mode(cli)? {
        CommandMode::Batch(path) => return run_batch_manifest(cli, audit, path),
        CommandMode::BundleList(path) => return run_bundle_list(cli, audit, path),
        CommandMode::BundleExtract(path) => return run_bundle_extract(cli, audit, path),
        CommandMode::BundlePack(path) => return run_bundle_pack(cli, audit, path),
        CommandMode::DeckFromToml(path) => return run_deck_from_toml(cli, audit, path),
        CommandMode::StructuredFile(path, kind) => {
            return run_blueprint_from_structured_file(cli, audit, path, kind)
        }
        CommandMode::LiveCapture(connect) => connect,
    };

    let tls_params = TlsParams {
        mode: TlsMode::parse(&cli.tls_mode).context("DBP1602E parsing --tls-mode")?,
        ca_bundle: cli.tls_ca.clone(),
        client_cert: cli.tls_cert.clone(),
        client_key: cli.tls_key.clone(),
        server_name_override: cli.tls_server_name.clone(),
        skip_verify: cli.tls_skip_verify,
        override_safety: cli.i_know_what_im_doing,
    };
    // Record the selected TLS policy even on --dry-run. Paths describe the
    // planned inputs; they are not added to files_read_local until a live run
    // actually parses them.
    audit.connection.tls_mode = tls_params.mode.as_str().to_string();
    audit.connection.tls_ca_path = tls_params.ca_bundle.clone();
    audit.connection.tls_client_cert = tls_params.client_cert.clone();
    // Engine dispatch on URI scheme.
    let engine_kind = engine_kind_for(connect)?;
    if tls_params.server_name_override.is_some() {
        match engine_kind {
            EngineKind::Mssql => bail!(
                "DBP1003E --tls-server-name is not supported by this release; use a --connect hostname \
                 that matches the certificate. SQL Server validates that hostname in both verify-ca and verify-full modes."
            ),
            EngineKind::Postgresql | EngineKind::MySQL => bail!(
                "DBP1003E --tls-server-name is not supported by this release; use a --connect hostname \
                 that matches the certificate, or use --tls-mode=verify-ca if your policy \
                 permits CA validation without hostname validation."
            ),
        }
    }
    if matches!(engine_kind, EngineKind::Mssql)
        && (tls_params.client_cert.is_some() || tls_params.client_key.is_some())
    {
        bail!(
            "DBP1015E --tls-cert/--tls-key client-certificate authentication is unavailable for SQL Server. \
             Next: remove both options and use a SQL Server authentication mode plus verify-full server-certificate validation."
        );
    }
    validate_expected_server_principal(cli, engine_kind)?;
    audit.connection.expected_server_principal = cli.expect_server_principal.clone();
    let length_fidelity = if cli.preserve_exact_lengths {
        if cli.length_fidelity == LengthFidelity::Strict {
            bail!(
                "DBP1008E --preserve-exact-lengths conflicts with --length-fidelity strict. \
                 Next: remove the legacy alias or use --length-fidelity exact --yes."
            );
        }
        LengthFidelity::Exact
    } else {
        cli.length_fidelity
    };
    if length_fidelity == LengthFidelity::Exact && !cli.yes {
        bail!(
            "DBP1009E --length-fidelity exact requires --yes because exact sampled lengths may reveal more precise customer statistics. \
             Next: review the pre-flight contract and re-run with --yes."
        );
    }
    if (cli.preserve_exact_lengths || length_fidelity != LengthFidelity::Balanced)
        && !matches!(engine_kind, EngineKind::MySQL)
    {
        bail!(
            "DBP1007E explicit --length-fidelity modes currently apply to live MySQL capture only. \
             Next: remove the explicit mode for this engine; PostgreSQL and SQL Server exact declared-length support is not yet advertised."
        );
    }
    if matches!(
        cli.artifact_detail,
        ArtifactDetail::Graph | ArtifactDetail::Analyzed
    ) && !cli.yes
    {
        bail!(
            "DBP1014E --artifact-detail={} requires --yes because an anonymous dependency graph can fingerprint an application and analyzed mode transiently reads object definitions. Next: review the privacy contract and rerun with --yes, or use --artifact-detail=summary.",
            cli.artifact_detail.as_str()
        );
    }
    audit.artifact_detail = Some(cli.artifact_detail.as_str().to_string());

    // Reject Entra-token flags for non-MSSQL engines. PostgreSQL and
    // MySQL have their own cloud IAM token paths (AWS RDS IAM, Azure
    // Database for PG/MySQL, GCP Cloud SQL IAM) that use explicit
    // --auth-mode=cloud-token plus exactly one --password-file/-env source.
    // Entra-token-as-AAD is a SQL Server / tiberius concept
    // (`AuthMethod::aad_token`) and only applies here.
    if (cli.azure_token_file.is_some() || cli.azure_token_env.is_some())
        && !matches!(engine_kind, EngineKind::Mssql)
    {
        let scheme = match engine_kind {
            EngineKind::Postgresql => "postgresql",
            EngineKind::MySQL => "mysql",
            EngineKind::Mssql => unreachable!(),
        };
        bail!(
            "DBP1004E --azure-token-file/--azure-token-env is SQL Server only (tiberius \
             AAD token auth). For {} cloud IAM auth (AWS RDS, Azure Database, \
             GCP Cloud SQL), use --auth-mode=cloud-token, generate the token \
             externally, and pass it via exactly one --password-file/--password-env source.",
            scheme
        );
    }

    // Resolve and validate the authentication mode before reading any
    // credential. PostgreSQL/MySQL cloud tokens use the password secret
    // channel, but remain explicit so the MySQL cleartext plugin cannot be
    // enabled accidentally for a normal database-password connection.
    let resolved_auth_mode = resolve_auth_mode(cli, engine_kind)?;
    audit.connection.auth = resolved_auth_mode.audit_str().to_string();
    let source_kind =
        SourceKind::parse(&cli.source_kind).context("DBP1013E validating --source-kind")?;
    let requested_schemas = schema_scope::SchemaSelection::new(cli.schema.clone());
    audit.schema_selector_count = requested_schemas.len() as u64;

    let (redacted_uri, embedded_pw, host_for_preflight, tls_host, resolved_user_source): (
        String,
        Option<String>,
        String,
        String,
        &'static str,
    ) = match engine_kind {
        EngineKind::Postgresql => {
            let (p, pw) = PgConnectParams::parse(connect).with_context(|| {
                "DBP1012E parsing PostgreSQL --connect URI (value redacted to avoid logging embedded credentials)"
            })?;
            let host = format!("{}:{}", p.host, p.port);
            let (resolved_user, src) = preview_user(cli, &p.user, p.uri_user_was_explicit, false)?;
            let red = format!(
                "postgresql://{}@{}:{}/{}",
                resolved_user, p.host, p.port, p.database
            );
            (red, pw, host, p.host, src)
        }
        EngineKind::MySQL => {
            let (p, pw) = MyConnectParams::parse(connect).with_context(|| {
                "DBP1012E parsing MySQL --connect URI (value redacted to avoid logging embedded credentials)"
            })?;
            let host = format!("{}:{}", p.host, p.port);
            let (resolved_user, src) = preview_user(cli, &p.user, p.uri_user_was_explicit, false)?;
            let red = format!(
                "mysql://{}@{}:{}/{}",
                resolved_user, p.host, p.port, p.database
            );
            (red, pw, host, p.host, src)
        }
        EngineKind::Mssql => {
            let (p, pw) = MssqlConnectParams::parse(connect).with_context(|| {
                "DBP1012E parsing SQL Server --connect URI (value redacted to avoid logging embedded credentials)"
            })?;
            let host = format!("{}:{}", p.host, p.port);
            let (resolved_user, src) = preview_user(
                cli,
                &p.user,
                p.uri_user_was_explicit,
                matches!(resolved_auth_mode, AuthMode::Integrated),
            )?;
            let red = if matches!(resolved_auth_mode, AuthMode::Integrated) {
                format!("sqlserver://{}:{}/{}", p.host, p.port, p.database)
            } else {
                format!(
                    "sqlserver://{}@{}:{}/{}",
                    resolved_user, p.host, p.port, p.database
                )
            };
            (red, pw, host, p.host, src)
        }
    };
    audit.connection.user_source = Some(resolved_user_source.to_string());
    audit.connection.uri_redacted = format!("(planned; not connected) {redacted_uri}");

    // Validate policy, option combinations, paths, and sensitive-file modes
    // before dry-run. Do not parse PEM content yet: dry-run must not read a
    // credential or private key.
    tls::validate(&tls_params, &tls_host)
        .context("DBP1602E validating TLS policy and local file paths")?;
    validate_cloud_token_tls(resolved_auth_mode, &tls_params)?;

    // Refuse URI-embedded passwords entirely. The whole `--connect`
    // value is visible in `ps`, in process tracing, and in any shell
    // history that captured the command — there is no warning text loud
    // enough to defend that. The customer's alternatives are:
    //
    //   --password-file PATH   (recommended; mode 0600)
    //   --password-env VAR     (when the secret is already in the env)
    //   TTY prompt             (no flag — read once at startup)
    //
    // The error message names them all and points to AUTH.md / SECURITY.md.
    if embedded_pw.is_some() {
        bail!(
            "DBP1001E refusing to use URI-embedded password: the entire --connect \
             value is visible in `ps` and shell history. Re-run with one of:\n  \
               --password-file PATH   (recommended; mode 0600)\n  \
               --password-env VAR     (named env var; tool reads only the var you name)\n  \
               (no flag)              (interactive TTY prompt)\n\
             See AUTH.md and SECURITY.md for the full rationale."
        );
    }
    drop(embedded_pw); // explicit: no longer reachable in the success path

    let mode = audit.mode.clone();

    // Dry-run must NOT read username/credential files, named credential
    // variables, or prompt the TTY. Compute descriptors of the configured
    // sources, then short-circuit before any such access.
    if cli.dry_run {
        let preview = describe_secret_source(cli, false);
        audit.record_password_source(&preview);
        audit
            .network_egress
            .push("none (dry-run; no connection attempted)".to_string());
        print_preflight_for(
            cli,
            &redacted_uri,
            &host_for_preflight,
            &preview,
            &mode,
            engine_kind,
            resolved_user_source,
        );
        eprintln!("{}", i18n::text("dry.live"));
        return Ok(());
    }

    // Show the pre-flight contract and collect consent before opening TLS
    // material or acquiring a credential. An attached terminal can authorize
    // Tier 2; `--yes` is the non-interactive equivalent.
    let planned_secret_source = describe_secret_source(cli, false);
    if !cli.yes {
        print_preflight_for(
            cli,
            &redacted_uri,
            &host_for_preflight,
            &planned_secret_source,
            &mode,
            engine_kind,
            resolved_user_source,
        );
        if !confirm_yes()? {
            // Exit via Err so the audit still gets emitted with
            // outcome="error: aborted (no consent)". anyhow will print
            // "Error: aborted (no consent)" on stderr — that's fine;
            // the audit is the forensic artefact.
            bail!("DBP1701E aborted (no consent)");
        }
    }

    configure_anonymization_key(cli, audit)?;

    // Parse TLS material before entering an engine capture boundary so a bad
    // CA, certificate, or key is classified as DBP1602E rather than a generic
    // database-capture failure. Engine setup may parse it again when building
    // its connector; audit file-read recording is idempotent.
    audit.record_tls_file_reads(
        tls_params.ca_bundle.as_ref(),
        tls_params.client_cert.as_ref(),
        tls_params.client_key.as_ref(),
    );
    if matches!(engine_kind, EngineKind::Mssql) {
        tls::validate_mssql_ca(&tls_params)
            .context("DBP1602E parsing SQL Server TLS trust material")?;
    } else {
        tls::build_client_config(&tls_params)
            .context("DBP1602E parsing TLS trust and client-certificate material")?;
    }

    // Credential acquisition. embedded_pw is unconditionally None here
    // (the URI-embedded-password refusal above runs first). For
    // --auth-mode=integrated there is no credential to acquire — the
    // OS-level Kerberos TGT cache (Linux) or current Windows session
    // (Windows) supplies it; we use a synthetic placeholder Secret so
    // the engine signature stays uniform.
    let (secret, _secret_source) = if matches!(resolved_auth_mode, AuthMode::Integrated) {
        let placeholder = Secret::placeholder_for_integrated_auth();
        let src = SecretSource::IntegratedAuth;
        audit.record_password_source(&src);
        (placeholder, src)
    } else {
        let (s, src) = acquire_secret(cli, None, audit)
            .context("DBP1601E acquiring the configured credential source")?;
        audit.record_password_source(&src);
        (s, src)
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("DBP1801E building asynchronous runtime")?;

    let blueprint = match engine_kind {
        EngineKind::Postgresql => {
            let (mut params, _) = PgConnectParams::parse(connect)
                .context("DBP1012E re-parsing validated PostgreSQL --connect URI")?;
            let (resolved_user, src) =
                resolve_user(cli, &params.user, params.uri_user_was_explicit, audit)?;
            params.user = resolved_user;
            // Refresh redacted_uri so it reflects the resolved user.
            params.redacted_uri = format!(
                "postgresql://{}@{}:{}/{}",
                params.user, params.host, params.port, params.database
            );
            audit.connection.user_source = Some(src.to_string());
            let opts = PgRunOpts {
                measure_compression: cli.measure_compression,
                compression_workers: cli
                    .compression_workers
                    .unwrap_or_else(default_compression_workers)
                    as usize,
                sample_rows: cli.sample_rows,
                sample_timeout_secs: cli.max_wall_secs,
                source_kind: source_kind.clone(),
                tls: tls_params.clone(),
                generated_at_pin: cli.generated_at.clone(),
                rtt_probe: !cli.no_rtt_probe,
                cloud_token_auth: matches!(resolved_auth_mode, AuthMode::CloudToken),
                artifact_detail: cli.artifact_detail,
                schemas: requested_schemas.clone(),
            };
            let started = std::time::Instant::now();
            let result = block_on_with_timeout(
                &runtime,
                std::time::Duration::from_secs(cli.max_wall_secs),
                engine_pg::run(&params, &secret, &opts, audit),
            );
            finish_live_capture(
                result,
                audit,
                "PostgreSQL",
                started,
                cli.max_wall_secs,
                "DBP1401E PostgreSQL source capture failed",
            )?
        }
        EngineKind::MySQL => {
            let (mut params, _) = MyConnectParams::parse(connect)
                .context("DBP1012E re-parsing validated MySQL --connect URI")?;
            let (resolved_user, src) =
                resolve_user(cli, &params.user, params.uri_user_was_explicit, audit)?;
            params.user = resolved_user;
            params.redacted_uri = format!(
                "mysql://{}@{}:{}/{}",
                params.user, params.host, params.port, params.database
            );
            audit.connection.user_source = Some(src.to_string());
            let opts = MyRunOpts {
                measure_compression: cli.measure_compression,
                compression_workers: cli
                    .compression_workers
                    .unwrap_or_else(default_compression_workers)
                    as usize,
                length_fidelity,
                sample_rows: cli.sample_rows,
                sample_timeout_secs: cli.max_wall_secs,
                source_kind_str: source_kind.as_str().to_string(),
                tls: tls_params.clone(),
                generated_at_pin: cli.generated_at.clone(),
                rtt_probe: !cli.no_rtt_probe,
                cloud_token_auth: matches!(resolved_auth_mode, AuthMode::CloudToken),
                artifact_detail: cli.artifact_detail,
                schemas: requested_schemas.clone(),
            };
            let started = std::time::Instant::now();
            let result = block_on_with_timeout(
                &runtime,
                std::time::Duration::from_secs(cli.max_wall_secs),
                engine_mysql::run(&params, &secret, &opts, audit),
            );
            finish_live_capture(
                result,
                audit,
                "MySQL",
                started,
                cli.max_wall_secs,
                "DBP1402E MySQL source capture failed",
            )?
        }
        EngineKind::Mssql => {
            let (mut params, _) = MssqlConnectParams::parse(connect)
                .context("DBP1012E re-parsing validated SQL Server --connect URI")?;
            let (resolved_user, src) = if matches!(resolved_auth_mode, AuthMode::Integrated) {
                (params.user.clone(), integrated_user_source())
            } else {
                resolve_user(cli, &params.user, params.uri_user_was_explicit, audit)?
            };
            params.user = resolved_user;
            params.redacted_uri = if matches!(resolved_auth_mode, AuthMode::Integrated) {
                format!(
                    "sqlserver://{}:{}/{}",
                    params.host, params.port, params.database
                )
            } else {
                format!(
                    "sqlserver://{}@{}:{}/{}",
                    params.user, params.host, params.port, params.database
                )
            };
            audit.connection.user_source = Some(src.to_string());
            // Translate the operator-facing AuthMode → engine-facing
            // MssqlAuthMode. resolve_mssql_auth_mode rejects Integrated
            // on vanilla builds; on feature-enabled builds it passes
            // through and the engine's Integrated arm dispatches
            // tiberius's AuthMethod::Integrated.
            let auth_mode = match resolved_auth_mode {
                AuthMode::SqlAuth => engine_mssql::MssqlAuthMode::SqlAuth,
                AuthMode::EntraToken => engine_mssql::MssqlAuthMode::EntraToken,
                AuthMode::Integrated => engine_mssql::MssqlAuthMode::Integrated,
                AuthMode::CloudToken => {
                    bail!("DBP1005E --auth-mode=cloud-token is unavailable for SQL Server; use --auth-mode=entra-token for Azure SQL or a SQL Server credential")
                }
            };
            let opts = MssqlRunOpts {
                measure_compression: cli.measure_compression,
                compression_workers: cli
                    .compression_workers
                    .unwrap_or_else(default_compression_workers)
                    as usize,
                sample_rows: cli.sample_rows,
                sample_timeout_secs: cli.max_wall_secs,
                source_kind_str: source_kind.as_str().to_string(),
                tls: tls_params.clone(),
                generated_at_pin: cli.generated_at.clone(),
                rtt_probe: !cli.no_rtt_probe,
                auth_mode,
                expected_server_principal: cli.expect_server_principal.clone(),
                artifact_detail: cli.artifact_detail,
                schemas: requested_schemas.clone(),
            };
            let started = std::time::Instant::now();
            let result = block_on_with_timeout(
                &runtime,
                std::time::Duration::from_secs(cli.max_wall_secs),
                engine_mssql::run(&params, &secret, &opts, audit),
            );
            finish_live_capture(
                result,
                audit,
                "SQL Server",
                started,
                cli.max_wall_secs,
                "DBP1403E SQL Server source capture failed",
            )?
        }
    };

    // Drop secret immediately after connection establishment + sample read.
    // (engine_pg::run already finished by here; secret is in scope only this far.)
    drop(secret);

    dbwarp_blueprint_core::validate_blueprint_contract(&blueprint)
        .context("DBP1502E validating captured Blueprint before publication")?;
    let fidelity = dbwarp_blueprint_core::estimate_blueprint_fidelity(&blueprint);
    audit.record_fidelity(fidelity);
    audit.record_artifact_inventory(blueprint.artifact_inventory.as_ref());
    audit.record_sizing_scope(
        blueprint.database_topology.as_ref(),
        blueprint.dataset_scope.as_ref(),
    );

    // Render and write the TOML file.
    let body = emit_toml(&blueprint).context("DBP1502E emitting Blueprint file as TOML")?;
    let bytes = body.as_bytes();
    if let Some(parent) = cli.out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("DBP1502E creating output dir {}", parent.display()))?;
        }
    }
    atomic_write_bytes(&cli.out, bytes)
        .with_context(|| format!("DBP1502E writing output {}", cli.out.display()))?;
    let mut h = Sha256::new();
    h.update(bytes);
    let sha = hex::encode(h.finalize());
    audit.record_file_written(cli.out.clone(), bytes.len() as u64, sha);
    println!(
        "{}",
        i18n::format("status.wrote", &[("path", cli.out.display().to_string())])
    );

    // Optional presentation deck, generated locally from the same in-memory
    // Blueprint. No database access, no network, no third-party crate.
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

fn configure_anonymization_key(cli: &Cli, audit: &mut AuditLog) -> Result<()> {
    if format::anonymization_key_is_initialized() {
        audit.anonymization_key_source =
            ANONYMIZATION_KEY_SOURCE.get().copied().map(str::to_string);
        return Ok(());
    }

    let (key, source) = if let Some(path) = cli.anonymization_key_file.as_deref() {
        audit.record_file_read(&path.display().to_string());
        secret::check_sensitive_file_mode(path, "anonymization key file")
            .context("DBP1607E validating anonymization key file permissions")?;
        let bytes = Zeroizing::new(std::fs::read(path).with_context(|| {
            format!(
                "DBP1607E reading anonymization key file '{}'",
                path.display()
            )
        })?);
        (parse_anonymization_key(&bytes)?, "customer-key-file")
    } else {
        (format::generate_anonymization_key()?, "ephemeral-random")
    };
    format::install_anonymization_key(key)?;
    ANONYMIZATION_KEY_SOURCE
        .set(source)
        .map_err(|_| anyhow!("DBP1607E anonymization key source initialization raced"))?;
    audit.anonymization_key_source = Some(source.to_string());
    Ok(())
}

fn parse_anonymization_key(bytes: &[u8]) -> Result<[u8; 32]> {
    if bytes.len() == 32 {
        let mut key = [0_u8; 32];
        key.copy_from_slice(bytes);
        return Ok(key);
    }

    let trimmed = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    let trimmed = trimmed.strip_suffix(b"\r").unwrap_or(trimmed);
    if trimmed.len() != 64 || !trimmed.iter().all(u8::is_ascii_hexdigit) {
        bail!(
            "DBP1607E anonymization key file must contain exactly 32 raw bytes or 64 hexadecimal characters"
        );
    }
    let decoded = Zeroizing::new(
        hex::decode(trimmed).context("DBP1607E decoding hexadecimal anonymization key")?,
    );
    let mut key = [0_u8; 32];
    key.copy_from_slice(&decoded);
    Ok(key)
}

#[derive(Debug, Clone, Copy)]
enum StructuredFileBlueprintKind {
    Parquet,
    Avro,
}

impl StructuredFileBlueprintKind {
    fn label(self) -> &'static str {
        match self {
            Self::Parquet => "parquet",
            Self::Avro => "avro",
        }
    }
}

fn run_blueprint_from_structured_file(
    cli: &Cli,
    audit: &mut AuditLog,
    path: &Path,
    kind: StructuredFileBlueprintKind,
) -> Result<()> {
    audit.connection.uri_redacted = format!("(offline: --from-{})", kind.label());
    audit.connection.auth = "(not used)".to_string();
    audit.connection.tls_mode = "(not used)".to_string();
    audit.connection.user_source = Some("(not used)".to_string());

    if cli.dry_run {
        print_structured_file_preflight(
            path,
            &cli.out,
            kind,
            cli.measure_compression,
            cli.sample_rows,
        );
        eprintln!("{}", i18n::text("dry.file"));
        return Ok(());
    }

    if cli.measure_compression && !cli.yes {
        bail!(
            "DBP1006E --from-{} --measure-compression requires --yes (consent flag). \
             Re-run with both flags after reviewing the pre-flight summary.",
            kind.label()
        );
    }

    let compression_options = if cli.measure_compression {
        dbwarp_blueprint_core::DecodedCompressionOptions::enabled(
            cli.sample_rows,
            format!(
                "{} decoded first {} rows; rowframe-v1 zstd",
                kind.label(),
                cli.sample_rows
            ),
            format!(
                "{} decoded first {} rows per column; rowframe-v1 zstd",
                kind.label(),
                cli.sample_rows
            ),
        )
        .with_limits(
            dbwarp_blueprint_core::DEFAULT_MAX_SAMPLE_BYTES,
            std::time::Duration::from_secs(cli.max_wall_secs.max(1)),
        )
    } else {
        dbwarp_blueprint_core::DecodedCompressionOptions::disabled()
    };

    audit.record_file_read(&path.display().to_string());
    let mut blueprint = match kind {
        StructuredFileBlueprintKind::Parquet => {
            dbwarp_blueprint_core::parquet::parquet_blueprint_from_path_with_options(
                path,
                &compression_options,
            )
        }
        StructuredFileBlueprintKind::Avro => {
            dbwarp_blueprint_core::avro::avro_blueprint_from_path_with_options(
                path,
                &compression_options,
            )
        }
    }
    .context("DBP1501E reading or decoding structured-file Blueprint")?;
    blueprint.generated_at = cli
        .generated_at
        .clone()
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
    if !cli.source_kind.is_empty() {
        blueprint.source_kind = cli.source_kind.clone();
    }

    dbwarp_blueprint_core::validate_blueprint_contract(&blueprint)
        .context("DBP1502E validating structured-file Blueprint before publication")?;
    audit.record_fidelity(dbwarp_blueprint_core::estimate_blueprint_fidelity(
        &blueprint,
    ));

    let body = dbwarp_blueprint_core::blueprint_to_toml(&blueprint)
        .context("DBP1502E emitting structured-file Blueprint as TOML")?;
    let bytes = body.as_bytes();
    if let Some(parent) = cli.out.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("DBP1502E creating output dir {}", parent.display()))?;
        }
    }
    atomic_write_bytes(&cli.out, bytes)
        .with_context(|| format!("DBP1502E writing output {}", cli.out.display()))?;
    let mut h = Sha256::new();
    h.update(bytes);
    let sha = hex::encode(h.finalize());
    audit.record_file_written(cli.out.clone(), bytes.len() as u64, sha);
    println!(
        "{}",
        i18n::format("status.wrote", &[("path", cli.out.display().to_string())])
    );
    Ok(())
}

fn print_structured_file_preflight(
    path: &Path,
    out: &Path,
    kind: StructuredFileBlueprintKind,
    measure_compression: bool,
    sample_rows: u64,
) {
    eprintln!("{}", i18n::text("preflight.title"));
    preflight_line("preflight.mode", format!("blueprint-from-{}", kind.label()));
    preflight_line("preflight.database", i18n::text("value.none"));
    preflight_line("preflight.input_file", path.display());
    preflight_line("preflight.output_toml", out.display());
    preflight_line("preflight.network", i18n::text("value.none"));
    if measure_compression {
        preflight_line(
            "preflight.decoded_sampling",
            i18n::format(
                "value.enabled_first_rows",
                &[("rows", sample_rows.to_string())],
            ),
        );
    } else {
        preflight_line("preflight.decoded_sampling", i18n::text("value.disabled"));
    }
    eprintln!();
}

fn preflight_line(key: &str, value: impl std::fmt::Display) {
    eprintln!("  {}: {}", i18n::text(key), value);
}

fn preflight_bullet(key: &str) {
    eprintln!("  - {}", i18n::text(key));
}

fn preflight_bullet_format(key: &str, values: &[(&str, String)]) {
    eprintln!("  - {}", i18n::format(key, values));
}

include!("app_batch.rs");
include!("app_bundle.rs");
include!("app_deck.rs");
include!("app_auth.rs");
include!("main_tests.rs");
