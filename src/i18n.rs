//! Presentation-only localization for dbwarp-blueprint.
//!
//! Operational syntax is deliberately language-neutral. Stable DBP codes,
//! command/option names, possible values, URI schemes, environment variables,
//! identifiers, paths, audit keys, and Blueprint schemas are never translated.
//! Every advertised non-English catalog must exactly cover the live help tree,
//! diagnostics, and stable presentation labels or startup fails closed.

use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;
use unicode_normalization::UnicodeNormalization;

pub const DEFAULT_LOCALE: Locale = Locale::En;
pub const SUPPORTED_LOCALES: &[Locale] = &[
    Locale::En,
    Locale::De,
    Locale::Fr,
    Locale::Es,
    Locale::Pl,
    Locale::Ja,
    Locale::Zh,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Locale {
    En = 0,
    De = 1,
    Fr = 2,
    Es = 3,
    Pl = 4,
    Ja = 5,
    Zh = 6,
}

impl Locale {
    pub const fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::De => "de",
            Self::Fr => "fr",
            Self::Es => "es",
            Self::Pl => "pl",
            Self::Ja => "ja",
            Self::Zh => "zh",
        }
    }

    pub const fn bcp47(self) -> &'static str {
        match self {
            Self::En => "en-US",
            Self::De => "de-DE",
            Self::Fr => "fr-FR",
            Self::Es => "es-ES",
            Self::Pl => "pl-PL",
            Self::Ja => "ja-JP",
            Self::Zh => "zh-CN",
        }
    }

    pub fn from_language_tag(raw: &str) -> Option<Self> {
        let base = raw
            .trim()
            .split(['-', '_', '.', '@'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        match base.as_str() {
            "c" | "posix" | "en" => Some(Self::En),
            "de" => Some(Self::De),
            "fr" => Some(Self::Fr),
            "es" => Some(Self::Es),
            "pl" => Some(Self::Pl),
            "ja" => Some(Self::Ja),
            "zh" => Some(Self::Zh),
            _ => None,
        }
    }

    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::De,
            2 => Self::Fr,
            3 => Self::Es,
            4 => Self::Pl,
            5 => Self::Ja,
            6 => Self::Zh,
            _ => Self::En,
        }
    }
}

static ACTIVE_LOCALE: AtomicU8 = AtomicU8::new(Locale::En as u8);
static LOCALE_ENV_READS: AtomicU8 = AtomicU8::new(0);

pub fn set_active_locale(locale: Locale) {
    ACTIVE_LOCALE.store(locale as u8, Ordering::Release);
}

pub fn active_locale() -> Locale {
    Locale::from_u8(ACTIVE_LOCALE.load(Ordering::Acquire))
}

pub fn resolve_locale(cli: Option<&str>) -> Locale {
    if let Some(locale) = cli.and_then(Locale::from_language_tag) {
        return locale;
    }
    for (index, name) in ["DBWARP_BLUEPRINT_LANG", "LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .enumerate()
    {
        LOCALE_ENV_READS.fetch_or(1 << index, Ordering::AcqRel);
        if let Ok(raw) = std::env::var(name) {
            if let Some(locale) = Locale::from_language_tag(&raw) {
                return locale;
            }
        }
    }
    DEFAULT_LOCALE
}

pub fn env_vars_read() -> Vec<&'static str> {
    let bits = LOCALE_ENV_READS.load(Ordering::Acquire);
    ["DBWARP_BLUEPRINT_LANG", "LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .enumerate()
        .filter_map(|(index, name)| (bits & (1 << index) != 0).then_some(name))
        .collect()
}

#[derive(Debug, Clone, Copy)]
pub enum DiagnosticField {
    Summary,
    Cause,
    Action,
}

#[derive(Debug, Clone, Copy)]
pub struct MessageSpec {
    pub code: &'static str,
    pub summary: &'static str,
    pub cause: &'static str,
    pub action: &'static str,
}

macro_rules! message_specs {
    ($(($code:literal, $summary:literal, $cause:literal, $action:literal)),+ $(,)?) => {
        pub const MESSAGE_SPECS: &[MessageSpec] = &[
            $(MessageSpec { code: $code, summary: $summary, cause: $cause, action: $action }),+
        ];
    };
}

message_specs!(
    ("DBP0001E", "dbwarp-blueprint operation failed", "The requested operation failed before a more specific operator message could classify it.", "Review the technical detail and causal chain, correct the underlying condition, and retry."),
    ("DBP1000E", "Database connection URI is required", "No live database URI or offline input mode was supplied.", "Pass --connect URI or select one offline input mode."),
    ("DBP1001E", "Password in connection URI was refused", "Connection URIs are visible to process inspection and shell history, so embedded credentials are unsafe.", "Use --password-file, --password-env, or the interactive TTY prompt. See AUTH.md and SECURITY.md."),
    ("DBP1002E", "Database URI scheme is unsupported", "The --connect value did not use a supported PostgreSQL, MySQL, or SQL Server URI scheme.", "Use postgresql://, postgres://, mysql://, sqlserver://, mssql://, or tds://. mariadb:// is accepted only as a MySQL URI alias; MariaDB is not a separately qualified engine."),
    ("DBP1003E", "TLS server-name override is unavailable", "This release does not implement --tls-server-name.", "Use a --connect hostname matching the certificate. PostgreSQL and MySQL may use verify-ca when policy permits chain-only validation; SQL Server validates the hostname in both verifying modes."),
    ("DBP1004E", "Microsoft Entra token option used for another engine", "The Azure token options are specific to SQL Server authentication.", "For PostgreSQL or MySQL cloud IAM, use cloud-token, generate the token externally, and supply it through --password-file or --password-env."),
    ("DBP1005E", "Authentication mode is unavailable for the selected engine", "The selected --auth-mode is not implemented for the chosen database engine.", "Use sql-auth, cloud-token, entra-token, or integrated only with a database engine that supports that mode."),
    ("DBP1006E", "Structured-file compression sampling lacks explicit consent", "Structured-file modes cannot prompt interactively and therefore require --yes before reading bounded samples.", "Review the pre-flight contract and rerun with --measure-compression --yes."),
    ("DBP1007E", "Length-fidelity mode is unavailable for this engine", "Explicit length-fidelity controls currently apply only to live MySQL capture.", "Remove the explicit mode or use a supported MySQL source. Compatibility URI aliases do not expand the qualified engine matrix."),
    ("DBP1008E", "Length-fidelity options conflict", "The legacy exact-length alias was combined with strict length fidelity.", "Remove the legacy alias or use --length-fidelity exact --yes."),
    ("DBP1009E", "Exact length fidelity lacks explicit consent", "Exact sampled lengths may reveal more precise customer statistics and require --yes.", "Review the pre-flight contract and rerun with --length-fidelity exact --yes."),
    ("DBP1010E", "Embedded localization catalog is incomplete", "An advertised language does not exactly cover the active CLI, diagnostic, or presentation surface.", "Install a complete build or select English; maintainers must update every locale with each customer-visible string."),
    ("DBP1011E", "Command-line arguments are invalid", "Clap rejected an option, value, conflict, requirement, or positional argument.", "Correct the command using dbwarp-blueprint --help and retry."),
    ("DBP1012E", "Database connection URI is malformed", "The --connect value used a supported scheme but its authority, host, port, user information, or database path could not be parsed.", "Correct the URI structure, bracket IPv6 hosts, include a database name, and keep credentials outside the URI."),
    ("DBP1013E", "Source-kind annotation is invalid", "The --source-kind value is empty or is not one of the supported source classifications.", "Pass production, staging, scrubbed-replica, or synthetic."),
    ("DBP1014E", "Artifact detail lacks explicit consent", "Anonymous dependency graphs can fingerprint an application, and analyzed mode transiently reads object definitions.", "Review the artifact privacy contract and rerun with --yes, or use --artifact-detail summary."),
    ("DBP1015E", "TLS client certificates are unavailable for SQL Server", "The SQL Server driver does not implement --tls-cert or --tls-key client-certificate authentication.", "Remove both options and use a SQL Server authentication mode plus verify-full server-certificate validation."),
    ("DBP1101E", "Batch manifest could not be read", "The manifest path was unavailable or its permissions prevented reading.", "Check the path and file permissions, then retry."),
    ("DBP1102E", "Batch manifest could not be parsed", "The manifest is not valid for the supported TOML batch schema.", "Correct the TOML structure and values, then validate with --dry-run."),
    ("DBP1103E", "Batch manifest contains no sources", "The manifest has no [[source]] entries.", "Add at least one [[source]] block."),
    ("DBP1104E", "Batch collection lacks explicit consent", "Non-interactive multi-source collection requires --yes.", "Run with --dry-run first, review the plan, then rerun with --yes."),
    ("DBP1105E", "A batch source failed", "One source in a multi-source collection could not be captured as a Blueprint.", "Inspect the source-specific detail and rerun with --dry-run before retrying."),
    ("DBP1106E", "Batch source kind is unsupported", "A source kind was not a supported database engine or structured-file format.", "Use `postgresql`, `mysql`, `sqlserver`, `parquet`, or `avro`."),
    ("DBP1107E", "File source resolved no inputs", "The configured path, paths, or glob matched no structured files.", "Correct the source paths relative to the manifest."),
    ("DBP1108E", "File dataset mode is unsupported", "The `dataset_mode` value was not `single_file`, `one_table_per_file`, `merge_same_schema`, or `partitioned_dataset`.", "Choose a supported `dataset_mode`."),
    ("DBP1109E", "Batch source identifier is invalid", "A [[source]] id contains no ASCII letter or digit after safe normalization.", "Give every batch source a stable identifier containing at least one ASCII letter or digit."),
    ("DBP1110E", "Database source connection configuration is ambiguous", "The batch source did not specify exactly one of connect, connect_env, or connect_file.", "Set exactly one connection source."),
    ("DBP1111E", "Batch connection environment variable is unavailable", "The named connect_env variable was missing or unreadable.", "Export the variable or use connect_file."),
    ("DBP1112E", "Batch connection file is unavailable", "The connect_file path was missing or unreadable.", "Check the path relative to the batch manifest and its permissions."),
    ("DBP1113E", "Batch output could not be completed", "A batch Blueprint, source audit, error report, directory, or bundle output could not be encoded, created, read back, or written.", "Check the output path, permissions, free space, and filesystem health, then retry."),
    ("DBP1114E", "Structured-file dataset members are incompatible", "Files selected as one logical dataset had no table definition or did not share a compatible column layout.", "Inspect the selected files and use one_table_per_file or separate batch sources when their schemas differ."),
    ("DBP1115E", "Every batch source failed", "The diagnostic bundle was published, but it contains no usable source Blueprint.", "Inspect errors.txt and the child inputs or credentials, then rerun the failed sources."),
    ("DBP1116E", "Batch bundle is partial", "At least one batch source failed while at least one other source completed, so the published bundle is incomplete.", "Inspect errors.txt and all child audits; recapture failed sources before treating the bundle as complete."),
    ("DBP1200E", "Bundle selector is invalid", "A blueprint:// or --select expression was malformed or contradictory.", "Use source=ID, table=ID, engine=VALUE, or tag=NAME once per selector key."),
    ("DBP1201E", "Bundle selector matched no sources", "No bundle source satisfies the supplied selector predicates.", "Run --bundle-list to inspect available sources and selectors."),
    ("DBP1202E", "Bundle selector matched multiple sources", "The selector is not specific enough for a single-source operation.", "Add --select source=ID or another narrowing predicate."),
    ("DBP1203E", "Bundle selector matched no extractable Blueprint", "The selected source has no matching embedded or referenced Blueprint/table.", "Check source and table selectors with --bundle-list."),
    ("DBP1204E", "Bundle input could not be read", "The bundle path was unavailable, was not a regular readable input, or its permissions prevented reading.", "Check the bundle path and permissions, then retry."),
    ("DBP1205E", "Bundle content is invalid", "The bundle TOML or one of its referenced Blueprint files could not be parsed, validated, or loaded.", "Validate the bundle and referenced Blueprint files, then regenerate the bundle if necessary."),
    ("DBP1206E", "Bundle output could not be written", "An extracted or packed bundle output could not be serialized, created, or written.", "Check the destination path, permissions, and free space, then retry."),
    ("DBP1301E", "Deck output path is required", "--from-toml was supplied without --deck.", "Pass --deck output.pptx."),
    ("DBP1302E", "Blueprint TOML schema version is unsupported", "The input Blueprint uses a schema version this release cannot read.", "Regenerate the Blueprint with a compatible dbwarp-blueprint release."),
    ("DBP1401E", "PostgreSQL Blueprint capture failed", "A PostgreSQL connection, catalog query, RTT probe, sampling, or decoding operation failed.", "Review the technical detail, connectivity, privileges, TLS policy, and source health, then retry."),
    ("DBP1402E", "MySQL Blueprint capture failed", "A MySQL connection, catalog query, RTT probe, sampling, or decoding operation failed.", "Review the technical detail, connectivity, privileges, TLS policy, and source health, then retry."),
    ("DBP1403E", "SQL Server Blueprint capture failed", "A SQL Server connection, catalog query, RTT probe, sampling, or decoding operation failed.", "Review the technical detail, connectivity, privileges, TLS policy, and source health, then retry."),
    ("DBP1404W", "Loopback TLS preference fell back to plaintext", "A loopback PostgreSQL connection in TLS prefer mode could not negotiate TLS and continued over local plain TCP as explicitly permitted by that mode.", "Use --tls-mode verify-full or require when even loopback plaintext fallback is unacceptable."),
    ("DBP1405W", "Database RTT probe was unavailable", "The optional SELECT 1 latency probe failed while the primary catalog capture remained usable.", "Check probe permissions and connectivity, or use --no-rtt-probe when the DBA intentionally forbids the probe."),
    ("DBP1406W", "Sampling time budget was exhausted", "Tier 2 reached its configured wall-time budget before every table or text column could be sampled.", "Increase --max-wall-secs or accept partial sampling and inspect the audit warnings."),
    ("DBP1407W", "Compression sample was unavailable", "A bounded row sample query, stream, encoding, or compression operation failed for at least one table while Blueprint capture continued.", "Inspect the technical detail and table permissions; rerun Tier 2 if complete compression evidence is required."),
    ("DBP1408W", "Column-style sample was unavailable", "A bounded text-column style probe failed for at least one column while Blueprint capture continued.", "Inspect the technical detail and column permissions; generated style labels may be incomplete."),
    ("DBP1409W", "Database connection task reported an error", "A PostgreSQL connection driver task reported an asynchronous error during or after capture.", "Inspect the driver detail and verify that the resulting Blueprint and audit are complete before relying on them."),
    ("DBP1410W", "Artifact catalog was unavailable", "An optional artifact catalog could not be read, so the artifact inventory is explicitly marked incomplete.", "Inspect the technical detail and source privileges; rerun after granting catalog visibility when a complete inventory is required."),
    ("DBP1411W", "Database topology evidence was unavailable", "An optional topology catalog could not be read, so member visibility and related completeness claims were conservatively reduced.", "Review source privileges and topology health, then rerun when complete topology evidence is required."),
    ("DBP1412W", "Distributed size aggregation was unavailable", "A known distributed or sharded database could not provide aggregate relation sizes, so misleading gateway-local or member-local statistics were suppressed.", "Capture through a supported aggregate-aware endpoint with all required members available, then rerun before using the Blueprint for capacity sizing."),
    ("DBP1413W", "Dataset coverage is incomplete", "At least one table-inventory, row-count, or size-coverage dimension is incomplete or unknown, so the totals require qualification.", "Inspect dataset_scope and its limitations; capture through a topology-aware coordinator or restore the missing evidence before capacity planning."),
    ("DBP1414W", "Bundle dataset relationship is unknown", "At least one source was not declared as independent, replica, or shard, so cross-source arithmetic is unsafe.", "Set dataset_relationship and dataset_group for every source in the batch manifest, then rerun."),
    ("DBP1415W", "Bundle replicas disagree", "Two sources declared as replicas have different table, row, or byte summaries; one deterministic representative was retained without averaging.", "Inspect source freshness and scope, then recapture the replica group before using aggregate totals."),
    ("DBP1416W", "Bundle shard group is incomplete", "A shard group was not declared complete or at least one expected shard failed, so that group contributed no aggregate totals.", "List every expected shard, set dataset_group_complete=true only when the list is exhaustive, and rerun failed members."),
    ("DBP1417W", "Bundle aggregate totals were suppressed", "An unknown dataset relationship made the logical-dataset aggregate unsafe, so aggregate table, row, and byte totals were emitted as zero with aggregation=suppressed.", "Declare source relationships explicitly and rerun; per-source Blueprint evidence remains available."),
    ("DBP1418W", "Bundle source scope is incomplete", "At least one source included in bundle arithmetic has incomplete or unknown dataset coverage.", "Inspect that source Blueprint dataset_scope and restore complete topology-aware evidence before capacity planning."),
    ("DBP1419E", "Live capture exceeded its hard wall-time limit", "The database connect, catalog, RTT, or sampling operation did not complete before --max-wall-secs, so the client dropped the connection. PostgreSQL and MySQL also enforce server-side statement limits; SQL Server enforces only a server-side lock-wait limit.", "Investigate the database or network stall, confirm server work stopped, or rerun with a larger reviewed --max-wall-secs value."),
    ("DBP1420E", "Requested schema is not visible", "At least one --schema selector did not resolve through the connected database's native schema catalog and visibility rules.", "Verify the connected database, schema spelling, active role, and metadata grants, then retry; no Blueprint is written for an unresolved scope."),
    ("DBP1421W", "Database principal evidence was unavailable", "The connected SQL Server session could not report its authenticated, effective server, or database principal, so the identity fields in the audit are incomplete.", "Review connectivity and server behavior; rerun before relying on the audit as proof of the capture identity."),
    ("DBP1501E", "Structured-file Blueprint capture failed", "Parquet or Avro metadata, decoding, or bounded compression sampling failed.", "Check the input format, file permissions, and sampling limits, then retry."),
    ("DBP1502E", "Blueprint or bundle output failed", "A Blueprint, bundle, or associated output could not be encoded, created, or written.", "Check the destination path, permissions, free space, and input validity."),
    ("DBP1503E", "PowerPoint deck generation failed", "The input Blueprint could not be converted to or written as a PowerPoint deck.", "Check the input Blueprint, destination path, permissions, and free space."),
    ("DBP1504W", "Audit log could not be written", "The requested audit-log destination was unavailable after the primary operation completed or failed.", "Check the audit path, permissions, and free space; preserve the terminal audit output."),
    ("DBP1601E", "Credential acquisition failed", "The selected password, token, file, environment, TTY, or integrated-auth source could not be used safely.", "Correct the credential source and permissions without placing secrets in the connection URI."),
    ("DBP1602E", "TLS configuration failed", "The TLS mode, CA, certificate, key, hostname policy, or safety override was invalid.", "Correct the TLS settings and certificate material, then retry."),
    ("DBP1603E", "Database username acquisition failed", "The selected --user, --user-env, or --user-file source was missing, unreadable, or empty.", "Correct the username source and permissions, then retry without placing secrets in the URI."),
    ("DBP1604E", "Database authentication configuration is invalid", "The selected --auth-mode conflicts with its credential-source or transport-security requirements.", "Choose one compatible authentication mode and credential source; cloud-token requires exactly one --password-file/-env and verify-full TLS."),
    ("DBP1605W", "Sensitive-file permission enforcement is unavailable", "This platform does not expose the Unix mode-bit check used to reject broadly readable credential or private-key files.", "Verify the file ACL manually and restrict read access to the intended account before continuing."),
    ("DBP1606E", "Authenticated database principal assertion failed", "The SQL Server session identity was unavailable, unsafe to compare, or did not match --expect-server-principal.", "Verify the process identity, login mapping, connection endpoint, and expected principal; then rerun without broadening database permissions."),
    ("DBP1607E", "Anonymization key initialization failed", "The customer-held key file was unsafe, unreadable, malformed, or operating-system randomness was unavailable.", "Use a mode-restricted file containing exactly 32 raw bytes or 64 hexadecimal characters, or omit --anonymization-key-file for a fresh process-local key."),
    ("DBP1701E", "Operation cancelled before consent", "The interactive pre-flight prompt was declined or did not receive canonical y/yes consent.", "Review the plan and rerun, or pass --yes after explicit approval."),
    ("DBP1702E", "Consent response could not be read", "The interactive pre-flight prompt could not read from standard input.", "Run from an interactive terminal or pass --yes after independently reviewing and approving the operation."),
    ("DBP1801E", "Asynchronous runtime initialization failed", "The process could not initialize the runtime required for database connections and concurrent work.", "Check process resource limits and the installed build; collect the technical detail and contact support if the failure persists."),
);

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Translation {
    summary: String,
    cause: String,
    action: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MessageCatalog {
    schema_version: u32,
    locale: String,
    messages: BTreeMap<String, Translation>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UiCatalog {
    schema_version: u32,
    locale: String,
    text: BTreeMap<String, String>,
    help_phrases: BTreeMap<String, String>,
}

pub const UI_TEXT: &[(&str, &str)] = &[
    ("diag.cause", "Cause"),
    ("diag.action", "Action"),
    ("diag.detail", "Technical detail"),
    ("diag.chain", "Caused by"),
    ("status.wrote", "dbwarp-blueprint: wrote {path}"),
    ("status.wrote_bundle", "dbwarp-blueprint: wrote bundle {path}"),
    ("status.wrote_packed_bundle", "dbwarp-blueprint: wrote packed bundle {path}"),
    ("status.wrote_source_errors", "dbwarp-blueprint: wrote {path} with {count} source errors"),
    ("status.fidelity", "Blueprint fidelity estimate: {overall}/100 ({band}) · structure {structure} · sizing {sizing} · column statistics {columns} · relationships {relationships} · artifacts {artifacts}"),
    ("status.fidelity_limitations", "Evidence limitations: {limitations}"),
    ("status.fidelity_qualification", "Estimate of captured evidence coverage; not measured source-truth accuracy or a confidence interval."),
    ("status.fidelity_band.high", "high"),
    ("status.fidelity_band.good", "good"),
    ("status.fidelity_band.moderate", "moderate"),
    ("status.fidelity_band.low", "low"),
    ("warning.audit_write", "{code} could not write --audit-log {path}: {error}"),
    ("dry.live", "--dry-run: not connecting; not reading credentials."),
    ("dry.file", "--dry-run: not reading input file; not writing Blueprint TOML."),
    ("dry.batch", "--dry-run: not connecting; not reading structured files; not writing bundle."),
    ("dry.deck", "--dry-run: not reading TOML; not writing deck."),
    ("dry.exit", "--dry-run set: exiting without connecting."),
    ("preflight.title", "Pre-flight summary (review before continuing):"),
    ("preflight.engine", "Engine"),
    ("preflight.connection", "Connection (red.)"),
    ("preflight.host", "Host"),
    ("preflight.mode", "Mode"),
    ("preflight.schemas", "Selected schemas"),
    ("preflight.database", "Database"),
    ("preflight.input_file", "Input file"),
    ("preflight.input_toml", "Input TOML"),
    ("preflight.output_toml", "Output TOML"),
    ("preflight.output", "Output"),
    ("preflight.output_directory", "Output directory"),
    ("preflight.manifest", "Manifest"),
    ("preflight.sources", "Sources"),
    ("preflight.network", "Network"),
    ("preflight.user_source", "User source"),
    ("preflight.expected_server_principal", "Expected server principal"),
    ("preflight.password_source", "Password source"),
    ("preflight.password_persisted", "Password persisted"),
    ("preflight.deck", "Deck"),
    ("preflight.deck_confidentiality", "Deck confidentiality"),
    ("preflight.source_kind", "Source kind"),
    ("preflight.artifact_detail", "Artifact detail"),
    ("preflight.tls_mode", "TLS mode"),
    ("preflight.tls_ca", "TLS CA (only)"),
    ("preflight.tls_client_cert", "TLS client cert"),
    ("preflight.tls_server_name", "TLS server name"),
    ("preflight.tls_verify", "TLS verify"),
    ("preflight.tls_override", "TLS override"),
    ("preflight.decoded_sampling", "Decoded sampling"),
    ("value.none", "none"),
    ("value.no", "no"),
    ("value.disabled", "disabled"),
    ("value.enabled_first_rows", "enabled, first {rows} rows"),
    ("value.generated_local", "{path} (generated locally; no network)"),
    ("value.tls_disabled", "DISABLED (--tls-skip-verify)"),
    ("value.tls_override_ack", "--i-know-what-im-doing acknowledged"),
    ("tier2.heading", "--measure-compression is ENABLED (Tier 2). This will:"),
    ("tier2.rows", "read up to {rows} rows per table (configurable: --sample-rows N)"),
    ("tier2.workers", "use {workers} bounded local compression worker(s); database reads remain sequential"),
    ("tier2.compress", "zstd-compress them locally in memory"),
    ("tier2.ratio", "record aggregate compression, null-density, cardinality/frequency, length, and style measurements; never sampled values"),
    ("tier2.discard", "discard sampled bytes after aggregation; sampled values are not written to disk"),
    ("tier2.block", "write documented sample-derived aggregate blocks, including [compression], for sampled tables and columns"),
    ("tier2.wall", "wall-time budget: {seconds} seconds (--max-wall-secs)"),
    ("tier1.heading", "Tier 1 (catalog only). This will:"),
    ("tier1.connection", "open ONE connection to {host}"),
    ("tier1.queries", "run a small set of catalog queries (no row data read)"),
    ("tier1.write", "write {path}"),
    ("mysql.length.heading", "MySQL length fidelity: {mode}"),
    ("mysql.length.balanced.declared", "exact declared capacities and index-prefix lengths"),
    ("mysql.length.balanced.sampled", "sampled value lengths use bounded relative-error buckets"),
    ("mysql.length.strict", "declared, sampled, and index-prefix lengths use coarse privacy buckets"),
    ("mysql.length.exact", "declared, sampled, and index-prefix lengths are not bucketed"),
    ("mysql.length.exact.warning", "use only when the reviewed Blueprint may include precise statistics"),
    ("consent.prompt", "Continue? [y/N] "),
    ("engine.pg.connection_error", "{code} PostgreSQL connection error: {error}"),
    ("engine.pg.tls_connection_error", "{code} PostgreSQL TLS connection error: {error}"),
    ("engine.pg.tls_fallback", "{code} TLS attempt failed ({error}); falling back to plain TCP (--tls-mode=prefer)"),
    ("engine.rtt_failed", "{code} RTT probe failed (non-fatal): {error}"),
    ("engine.principal_failed", "{code} Database principal probe failed (non-fatal): {error}"),
    ("engine.sample_budget", "{code} Tier-2 sample wall budget exceeded; skipping remaining tables"),
    ("engine.column_budget", "{code} Tier-2 column sample wall budget exceeded; skipping remaining columns"),
    ("engine.compression_failed", "{code} Compression sample failed: {error}"),
    ("engine.style_failed", "{code} Style sample failed: {error}"),
    ("engine.topology_unavailable", "{code} Topology evidence source {catalog} was unavailable; completeness was reduced"),
    ("engine.distributed_size_unavailable", "{code} Distributed size aggregation was unavailable; gateway-local or member-local statistics were suppressed"),
    ("engine.dataset_scope_incomplete", "{code} Dataset coverage is incomplete (tables={tables}, rows={rows}, sizes={sizes}); totals require qualification"),
    ("bundle.relationship_unknown", "{code} Bundle dataset relationship is unknown; cross-source arithmetic is unsafe"),
    ("bundle.replica_disagreement", "{code} Declared replicas disagree; one deterministic copy was retained without averaging"),
    ("bundle.shard_incomplete", "{code} Shard group is incomplete; that group contributed no aggregate totals"),
    ("bundle.aggregate_suppressed", "{code} Bundle aggregate totals were suppressed; declare every source relationship"),
    ("bundle.source_scope_incomplete", "{code} A source included in bundle arithmetic has incomplete or unknown dataset coverage"),
    ("engine.sample_query_failed", "{code} Sample query failed for {table}: {error}"),
    ("engine.sample_stream_failed", "{code} Sample stream failed for {table}: {error}"),
    ("engine.driver_detail_redacted", "{target}; database driver detail redacted to protect source identifiers"),
    ("security.mode_check_noop", "{code} WARNING: {label} mode check is a no-op on this platform. Verify ACLs on '{path}' yourself; broad read access is a credential leak. (See SECURITY.md.)"),
    ("deck.brand_tagline", "Global Data · Local Speeds"),
    ("deck.website", "DBWarp.com"),
    ("deck.confidentiality.public", "Public"),
    ("deck.confidentiality.internal", "Internal"),
    ("deck.confidentiality.confidential", "Confidential"),
    ("deck.confidentiality.restricted", "Restricted"),
    ("deck.report", "Database Blueprint report"),
    ("deck.title_meta", "{name} {version}   ·   {source}   ·   {tables}   ·   generated {generated}"),
    ("deck.executive", "EXECUTIVE SUMMARY"),
    ("deck.executive.subtitle", "What this Blueprint says"),
    ("deck.scale_signal", "Migration scale"),
    ("deck.count.table.one", "{count} table"),
    ("deck.count.table.few", "{count} tables"),
    ("deck.count.table.other", "{count} tables"),
    ("deck.count.row.one", "{count} row"),
    ("deck.count.row.few", "{count} rows"),
    ("deck.count.row.other", "{count} rows"),
    ("deck.count.schema.one", "{count} schema"),
    ("deck.count.schema.few", "{count} schemas"),
    ("deck.count.schema.other", "{count} schemas"),
    ("deck.count.foreign_key_link.one", "{count} foreign-key link"),
    ("deck.count.foreign_key_link.few", "{count} foreign-key links"),
    ("deck.count.foreign_key_link.other", "{count} foreign-key links"),
    ("deck.metric.table.one", "Table"),
    ("deck.metric.table.few", "Tables"),
    ("deck.metric.table.other", "Tables"),
    ("deck.metric.foreign_key.one", "Foreign key"),
    ("deck.metric.foreign_key.few", "Foreign keys"),
    ("deck.metric.foreign_key.other", "Foreign keys"),
    ("deck.metric.index.one", "Index"),
    ("deck.metric.index.few", "Indexes"),
    ("deck.metric.index.other", "Indexes"),
    ("deck.scale_signal.body", "{tables}, {rows}, {data} table data; {schemas}."),
    ("deck.concentration_signal", "Data concentration"),
    ("deck.concentration_signal.body.one", "Largest table holds {share} of table data; plan the migration wave around it."),
    ("deck.concentration_signal.body.few", "Largest {count} tables hold {share} of table data; plan migration waves around them."),
    ("deck.concentration_signal.body", "Largest {count} tables hold {share} of table data; plan migration waves around them."),
    ("deck.relationship_signal", "Change complexity"),
    ("deck.relationship_signal.body", "{foreign_keys}; connected tables: {connected} of {total}."),
    ("deck.confidence_signal", "Share-ready evidence"),
    ("deck.confidence_signal.body", "Anonymised Blueprint only, generated locally, with no table or column names and no row data."),
    ("deck.overview", "OVERVIEW"),
    ("deck.overview.subtitle", "Database Blueprint at a glance"),
    ("deck.tables", "Tables"),
    ("deck.rows", "Rows"),
    ("deck.table_data", "Table data"),
    ("deck.columns", "Columns"),
    ("deck.indexes", "Indexes"),
    ("deck.schema", "Schema"),
    ("deck.schema_namespace", "independent namespace included in this Blueprint"),
    ("deck.schema_namespaces", "independent namespaces included in this Blueprint"),
    ("deck.primary_sizing_inputs", "PRIMARY SIZING INPUTS"),
    ("deck.structure", "STRUCTURE"),
    ("deck.catalog_table_groups", "catalog table groups"),
    ("deck.avg_cols_per_table", "Avg cols/table"),
    ("deck.column_density", "column density"),
    ("deck.complexity", "COMPLEXITY"),
    ("deck.load_order_links", "load-order links"),
    ("deck.secondary_objects", "secondary objects"),
    ("deck.secondary_structure", "Secondary structure"),
    ("deck.index_storage_size", "index storage size"),
    ("deck.anonymous_prefix", "Anonymised database Blueprint only"),
    ("deck.anonymous_suffix", " - no table names, column names, or row data in this report."),
    ("deck.overview.meta", "columns: {columns}   ·   indexes: {indexes}   ·   foreign keys: {foreign_keys}   ·   schemas: {schemas}"),
    ("deck.anonymous", "Anonymised database Blueprint only — no table or column names, no row data"),
    ("deck.tables.section", "TABLES"),
    ("deck.tables.sized", "Tables, sized"),
    ("deck.clustered", "clustered"),
    ("deck.heap", "heap"),
    ("deck.table.meta", "rows: {rows}  ·  cols: {columns}  ·  idx: {indexes}  ·  {layout}"),
    ("deck.table.sizes", "{data} data  ·  {indexes} idx"),
    ("deck.schema_table_meta", "rows: {rows}  ·  {layout}"),
    ("deck.largest", "Largest tables"),
    ("deck.row_and_bytes", "rows: {rows}  ·  {bytes}"),
    ("deck.more_tables", "+ more tables: {count}  ·  {bytes} additional data"),
    ("deck.composition", "COMPOSITION"),
    ("deck.composition.subtitle", "Schema composition"),
    ("deck.columns_by_type", "Columns by type"),
    ("deck.indexes_totals", "Indexes & totals"),
    ("deck.columns_total", "Columns total"),
    ("deck.indexes_total", "Indexes total"),
    ("deck.unique_nonunique", "Unique / non-unique"),
    ("deck.schemas", "Schemas"),
    ("deck.index_methods", "Index methods:  "),
    ("deck.unique", "unique "),
    ("deck.schema_map", "SCHEMA MAP"),
    ("deck.foreign_key_relationships", "Foreign-key relationships"),
    ("deck.relationships", "RELATIONSHIPS"),
    ("deck.foreign_keys", "foreign keys"),
    ("deck.connected", "connected tables: {connected} of {total}  ·  standalone: {standalone}"),
    ("deck.most_referenced", "Most referenced tables"),
    ("deck.refs", "refs: {count}"),
    ("deck.compression", "COMPRESSION"),
    ("deck.compression.subtitle", "Measured compression (Tier 2)"),
    ("deck.sampled_tables", "Sampled tables"),
    ("deck.weighted_zstd3", "Weighted zstd-3"),
    ("deck.projected_compressed", "Projected compressed"),
    ("deck.projected_reduction", "Projected reduction"),
    ("deck.sample_meta", "sampled rows: {rows}   ·   sampled bytes: {sample_bytes}   ·   raw table data covered: {raw_bytes}   ·   biased samples: {biased}"),
    ("deck.most_compressible", "Most compressible sampled tables"),
    ("deck.projection", "Projection uses each sampled table's rounded table_bytes divided by its measured zstd-3 ratio."),
    ("deck.sample_disposal", "Sample bytes are processed in memory and discarded; the Blueprint stores aggregate compression, null-density, cardinality/frequency, length, and style measurements, never sampled values."),
    ("deck.trust_model", "TRUST MODEL"),
    ("deck.trust.subtitle", "Verifiable by construction"),
    ("deck.no_phone_home", "No phone-home"),
    ("deck.no_phone_home.body", "Live collection opens the database-driver session; DNS and configured identity or TLS infrastructure may also be contacted. Deck generation makes no network connection. No telemetry, license check, or upload path; the audit records files, queries, and available byte evidence."),
    ("deck.one_leak", "Credential boundary"),
    ("deck.one_leak.body", "Credentials enter through the isolated Secret wrapper. Driver hand-off points are grep-able .expose() call sites; driver-owned copy limits are documented in SECURITY.md."),
    ("deck.no_hidden", "Explicit disclosure"),
    ("deck.no_hidden.body", "Blueprint writes only documented structural and statistical fields. Exact structural lengths may be present when catalog metadata or the selected length policy preserves them; source values and object names are not written."),
    ("deck.footer", "Open source  ·  reproducible builds  ·  an audit log on every run  ·  this deck built locally from the Blueprint, no network"),
];

macro_rules! catalog_slots {
    ($(($locale:ident, $message:literal, $ui:literal)),+ $(,)?) => {
        fn message_catalog_result(locale: Locale) -> Result<Option<&'static MessageCatalog>, String> {
            match locale {
                Locale::En => Ok(None),
                $(Locale::$locale => {
                    static CELL: OnceLock<Result<MessageCatalog, String>> = OnceLock::new();
                    CELL.get_or_init(|| parse_message_catalog(Locale::$locale, include_str!($message)))
                        .as_ref().map(Some).map_err(Clone::clone)
                }),+
            }
        }

        fn ui_catalog_result(locale: Locale) -> Result<Option<&'static UiCatalog>, String> {
            match locale {
                Locale::En => Ok(None),
                $(Locale::$locale => {
                    static CELL: OnceLock<Result<UiCatalog, String>> = OnceLock::new();
                    CELL.get_or_init(|| parse_ui_catalog(Locale::$locale, include_str!($ui)))
                        .as_ref().map(Some).map_err(Clone::clone)
                }),+
            }
        }
    };
}

catalog_slots!(
    (De, "../locales/messages.de.json", "../locales/ui.de.json"),
    (Fr, "../locales/messages.fr.json", "../locales/ui.fr.json"),
    (Es, "../locales/messages.es.json", "../locales/ui.es.json"),
    (Pl, "../locales/messages.pl.json", "../locales/ui.pl.json"),
    (Ja, "../locales/messages.ja.json", "../locales/ui.ja.json"),
    (Zh, "../locales/messages.zh.json", "../locales/ui.zh.json"),
);

fn validate_header(locale: Locale, schema: u32, actual: &str, kind: &str) -> Result<(), String> {
    if schema != 1 {
        return Err(format!("{} {kind} schema_version must be 1", locale.code()));
    }
    if actual != locale.code() {
        return Err(format!(
            "{} {kind} locale mismatch: {actual:?}",
            locale.code()
        ));
    }
    Ok(())
}

fn parse_message_catalog(locale: Locale, source: &str) -> Result<MessageCatalog, String> {
    let catalog: MessageCatalog = serde_json::from_str(source)
        .map_err(|error| format!("{} message catalog is invalid JSON: {error}", locale.code()))?;
    validate_header(
        locale,
        catalog.schema_version,
        &catalog.locale,
        "message catalog",
    )?;
    let expected = MESSAGE_SPECS
        .iter()
        .map(|spec| spec.code)
        .collect::<BTreeSet<_>>();
    let actual = catalog
        .messages
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(format!(
            "{} message catalog does not exactly cover DBP codes",
            locale.code()
        ));
    }
    let mut failures = Vec::new();
    for spec in MESSAGE_SPECS {
        let translated = &catalog.messages[spec.code];
        for (name, canonical, value) in [
            ("summary", spec.summary, translated.summary.as_str()),
            ("cause", spec.cause, translated.cause.as_str()),
            ("action", spec.action, translated.action.as_str()),
        ] {
            if let Err(error) = validate_translation(canonical, value) {
                failures.push(format!("{} {} {name}: {error}", locale.code(), spec.code));
            }
        }
    }
    if !failures.is_empty() {
        return Err(failures.join("; "));
    }
    Ok(catalog)
}

fn parse_ui_catalog(locale: Locale, source: &str) -> Result<UiCatalog, String> {
    let catalog: UiCatalog = serde_json::from_str(source)
        .map_err(|error| format!("{} UI catalog is invalid JSON: {error}", locale.code()))?;
    validate_header(
        locale,
        catalog.schema_version,
        &catalog.locale,
        "UI catalog",
    )?;
    let expected = UI_TEXT.iter().map(|(key, _)| *key).collect::<BTreeSet<_>>();
    let actual = catalog
        .text
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(format!(
            "{} UI catalog does not exactly cover stable text keys",
            locale.code()
        ));
    }
    let mut failures = Vec::new();
    for (key, canonical) in UI_TEXT {
        if let Err(error) = validate_translation(canonical, &catalog.text[*key]) {
            failures.push(format!("{} UI key {key}: {error}", locale.code()));
        }
    }
    for (canonical, translated) in &catalog.help_phrases {
        if let Err(error) = validate_translation(canonical, translated) {
            failures.push(format!(
                "{} help phrase {canonical:?}: {error}",
                locale.code()
            ));
        }
    }
    if !failures.is_empty() {
        return Err(failures.join("; "));
    }
    Ok(catalog)
}

fn canonical_ui(key: &str) -> Option<&'static str> {
    UI_TEXT
        .iter()
        .find_map(|(candidate, value)| (*candidate == key).then_some(*value))
}

pub fn text_for(locale: Locale, key: &str) -> Result<&'static str, String> {
    if locale == Locale::En {
        return canonical_ui(key).ok_or_else(|| format!("unknown canonical UI key {key}"));
    }
    ui_catalog_result(locale)?
        .and_then(|catalog| catalog.text.get(key))
        .map(String::as_str)
        .ok_or_else(|| format!("{} UI catalog missing key {key}", locale.code()))
}

pub fn text(key: &str) -> &'static str {
    text_for(active_locale(), key).expect("catalogs are validated before command execution")
}

pub fn format_for(locale: Locale, key: &str, values: &[(&str, String)]) -> Result<String, String> {
    render_template(text_for(locale, key)?, values)
}

pub fn format(key: &str, values: &[(&str, String)]) -> String {
    format_for(active_locale(), key, values)
        .expect("catalogs and template arguments are validated before command execution")
}

fn render_template(template: &str, values: &[(&str, String)]) -> Result<String, String> {
    let required = placeholders(template);
    let supplied = values.iter().map(|(key, _)| *key).collect::<BTreeSet<_>>();
    if required != supplied {
        return Err(format!(
            "template arguments mismatch: required {required:?}, supplied {supplied:?}"
        ));
    }
    let mut output = template.to_string();
    for (key, value) in values {
        output = output.replace(&format!("{{{key}}}"), value);
    }
    Ok(output)
}

fn placeholders(text: &str) -> BTreeSet<&str> {
    let mut output = BTreeSet::new();
    let bytes = text.as_bytes();
    let mut cursor = 0;
    while cursor < bytes.len() {
        let Some(open_rel) = text[cursor..].find('{') else {
            break;
        };
        let open = cursor + open_rel;
        let Some(close_rel) = text[open + 1..].find('}') else {
            break;
        };
        let close = open + 1 + close_rel;
        let key = &text[open + 1..close];
        if !key.is_empty()
            && key
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            output.insert(key);
        }
        cursor = close + 1;
    }
    output
}

pub fn diagnostic_field(
    locale: Locale,
    code: &str,
    field: DiagnosticField,
) -> Option<&'static str> {
    if locale == Locale::En {
        let spec = MESSAGE_SPECS.iter().find(|spec| spec.code == code)?;
        return Some(match field {
            DiagnosticField::Summary => spec.summary,
            DiagnosticField::Cause => spec.cause,
            DiagnosticField::Action => spec.action,
        });
    }
    let translated = message_catalog_result(locale)
        .ok()
        .flatten()?
        .messages
        .get(code)?;
    Some(match field {
        DiagnosticField::Summary => translated.summary.as_str(),
        DiagnosticField::Cause => translated.cause.as_str(),
        DiagnosticField::Action => translated.action.as_str(),
    })
}

pub fn help_phrase(locale: Locale, canonical: &str) -> Result<&'static str, String> {
    if locale == Locale::En {
        return Err("English help uses canonical source phrases".to_string());
    }
    ui_catalog_result(locale)?
        .and_then(|catalog| catalog.help_phrases.get(canonical))
        .map(String::as_str)
        .ok_or_else(|| format!("{} help catalog missing {canonical:?}", locale.code()))
}

pub fn validate_catalogs(required_help: &BTreeSet<String>) -> Result<(), String> {
    let mut failures = Vec::new();
    for locale in SUPPORTED_LOCALES
        .iter()
        .copied()
        .filter(|locale| *locale != Locale::En)
    {
        if let Err(error) = message_catalog_result(locale) {
            failures.push(error);
        }
        let catalog = match ui_catalog_result(locale) {
            Ok(Some(catalog)) => catalog,
            Ok(None) => {
                failures.push(format!("{} UI catalog missing", locale.code()));
                continue;
            }
            Err(error) => {
                failures.push(error);
                continue;
            }
        };
        let expected = required_help
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let actual = catalog
            .help_phrases
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if actual != expected {
            let missing = expected
                .difference(&actual)
                .take(5)
                .copied()
                .collect::<Vec<_>>();
            let extra = actual
                .difference(&expected)
                .take(5)
                .copied()
                .collect::<Vec<_>>();
            failures.push(format!(
                "{} help catalog mismatch; missing={missing:?}, extra={extra:?}",
                locale.code()
            ));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n"))
    }
}

pub fn localize_help_scaffolding(locale: Locale, canonical: &str) -> Result<String, String> {
    if locale == Locale::En {
        return Ok(canonical.to_string());
    }
    const LINE_LABELS: &[&str] = &[
        "Usage:",
        "Commands:",
        "Arguments:",
        "Options:",
        "Possible values:",
    ];
    const INLINE_LABELS: &[&str] = &["[default:", "[possible values:", "[alias:", "[aliases:"];
    let mut output = String::with_capacity(canonical.len());
    for line in canonical.split_inclusive('\n') {
        let indent_len = line.len() - line.trim_start_matches([' ', '\t']).len();
        let (indent, body) = line.split_at(indent_len);
        if let Some(source) = LINE_LABELS.iter().find(|source| body.starts_with(**source)) {
            output.push_str(indent);
            output.push_str(help_phrase(locale, source)?);
            output.push_str(&body[source.len()..]);
        } else {
            output.push_str(line);
        }
    }
    for source in INLINE_LABELS {
        output = output.replace(source, help_phrase(locale, source)?);
    }
    Ok(output)
}

fn validate_translation(canonical: &str, translated: &str) -> Result<(), String> {
    if translated.trim().is_empty() {
        return Err("translation is empty".to_string());
    }
    if translated.chars().any(is_forbidden_format_control) {
        return Err("translation contains an invisible or bidi format control".to_string());
    }
    if placeholders(canonical) != placeholders(translated) {
        return Err(format!(
            "placeholder mismatch: {:?} != {:?}",
            placeholders(canonical),
            placeholders(translated)
        ));
    }
    for command_line in canonical_command_lines(canonical) {
        if !translated.lines().any(|line| line == command_line) {
            return Err(format!(
                "localized example changed canonical command line {command_line:?}"
            ));
        }
    }
    let expected = operational_tokens(canonical);
    let actual = operational_tokens(translated);
    if expected != actual {
        return Err(format!(
            "canonical operational tokens changed: {expected:?} != {actual:?}"
        ));
    }
    Ok(())
}

fn canonical_command_lines(text: &str) -> Vec<&str> {
    let mut output = Vec::new();
    let mut in_command = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("dbwarp-blueprint ") && trimmed.contains(" --") {
            in_command = true;
            output.push(line);
            continue;
        }
        if in_command && trimmed.starts_with("--") {
            output.push(line);
            continue;
        }
        in_command = false;
    }
    output
}

pub(crate) fn is_forbidden_format_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{061c}'
            | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2060}'..='\u{2069}'
            | '\u{feff}'
    )
}

fn operational_tokens(text: &str) -> BTreeSet<String> {
    static TOKEN_RE: OnceLock<regex::Regex> = OnceLock::new();
    static ABSOLUTE_PATH_RE: OnceLock<regex::Regex> = OnceLock::new();
    static BACKTICK_RE: OnceLock<regex::Regex> = OnceLock::new();
    static CARGO_FEATURE_RE: OnceLock<regex::Regex> = OnceLock::new();
    let token_re = TOKEN_RE.get_or_init(|| {
        regex::Regex::new(
            r#"(?x)
              --[A-Za-z0-9_*][A-Za-z0-9_*-]*(?:/[A-Za-z0-9_*-]+)?
            | \bDBP[0-9]{4}[EWI]\b
            | \bDBWARP_[A-Z0-9_]+\b
            | \b[A-Za-z][A-Za-z0-9+.-]*://(?:[A-Za-z0-9._~:/?\#\[\]@!$&'*+=%-]*[A-Za-z0-9_~:/?\#\[\]@!$&'*+=%-])?
            | (?:source|table|engine|tag)=[A-Za-z0-9_.*|/-]*[A-Za-z0-9_*|/-]
            | [A-Za-z0-9_./-]+\.(?:md|toml|pptx|json|pem|txt|parquet|avro|crt|pass|user)
            | \b(?:verify-ca|verify-full|sql-auth|entra-token|cloud-token|single_file|file_set|balanced|strict|exact|y/yes)\b
            | \b(?:PGPASSWORD|MYSQL_PWD)\b
            | \baz\s+account\s+get-access-token\b
            | \[\[source\]\]
            | \[compression\]
            | \.expose\(\)
            "#,
        )
        .expect("operational-token regex must compile")
    });
    let absolute_path_re = ABSOLUTE_PATH_RE.get_or_init(|| {
        regex::Regex::new(r#"(?:^|[\s(>='\"])(?P<path>/(?:[A-Za-z0-9_.$-]+/)+[A-Za-z0-9_.$-]+)"#)
            .expect("absolute-path regex must compile")
    });
    let backtick_re = BACKTICK_RE.get_or_init(|| {
        regex::Regex::new(r"`[^`\r\n]+`").expect("backtick-token regex must compile")
    });
    let cargo_feature_re = CARGO_FEATURE_RE.get_or_init(|| {
        regex::Regex::new(r"--features(?:\s+|=)(?P<features>[A-Za-z0-9_-]+(?:,[A-Za-z0-9_-]+)*)")
            .expect("Cargo-feature regex must compile")
    });
    let normalized = text.nfkc().collect::<String>();
    let mut output = token_re
        .find_iter(&normalized)
        .map(|matched| matched.as_str().to_string())
        .collect::<BTreeSet<_>>();
    output.extend(
        backtick_re
            .find_iter(&normalized)
            .map(|matched| matched.as_str().to_string()),
    );
    output.extend(
        absolute_path_re
            .captures_iter(&normalized)
            .filter_map(|capture| capture.name("path"))
            .map(|matched| matched.as_str().to_string()),
    );
    output.extend(
        cargo_feature_re
            .captures_iter(&normalized)
            .filter_map(|capture| capture.name("features"))
            .flat_map(|matched| matched.as_str().split(','))
            .map(str::to_string),
    );
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Read every production Rust source in the binary crate. Keeping this
    /// inventory dynamic prevents a module split from silently weakening the
    /// UI-key and DBP-code coverage checks.
    fn production_sources() -> Vec<String> {
        let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut paths = std::fs::read_dir(&source_dir)
            .expect("read binary source directory")
            .map(|entry| entry.expect("read source-directory entry").path())
            .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("rs"))
            .filter(|path| {
                let name = path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("");
                name != "i18n.rs" && !name.ends_with("_tests.rs")
            })
            .collect::<Vec<_>>();
        paths.sort();
        paths
            .into_iter()
            .map(|path| {
                std::fs::read_to_string(&path)
                    .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
            })
            .collect()
    }

    #[test]
    fn locale_tags_resolve_without_translating_tokens() {
        assert_eq!(Locale::from_language_tag("de_CH.UTF-8"), Some(Locale::De));
        assert_eq!(Locale::from_language_tag("ja-JP"), Some(Locale::Ja));
        assert_eq!(Locale::from_language_tag("xx"), None);
    }

    #[test]
    fn templates_require_exact_placeholders() {
        assert!(render_template("wrote {path}", &[("path", "x".to_string())]).is_ok());
        assert!(render_template("wrote {path}", &[]).is_err());
    }

    #[test]
    fn translations_reject_token_changes_and_invisible_controls() {
        assert!(validate_translation(
            "Use --yes with postgresql://host",
            "Mit --yes und postgresql://host verwenden"
        )
        .is_ok());
        assert!(validate_translation("Use --yes", "Mit --ja verwenden").is_err());
        assert!(validate_translation("Use --yes", "Mit --yes\u{200b} verwenden").is_err());
        assert!(validate_translation(
            "Build with --features integrated-auth-gssapi or --features=winauth",
            "Mit --features integrated-auth-gssapi oder --features=winauth bauen"
        )
        .is_ok());
        assert!(validate_translation(
            "Build with --features integrated-auth-gssapi",
            "Mit --features Integrated-auth-gssapi bauen"
        )
        .is_err());
    }

    #[test]
    fn prose_slashes_are_translatable_but_paths_are_not() {
        assert!(validate_translation(
            "Extract a source/table from the bundle",
            "Eine Quelle/Tabelle aus dem Bundle extrahieren"
        )
        .is_ok());
        assert!(
            validate_translation("Read /etc/dbwarp/db.pass", "Lesen Sie /etc/dbwarp/passwort")
                .is_err()
        );
    }

    #[test]
    fn option_led_prose_is_not_mistaken_for_a_command() {
        assert!(validate_translation(
            "--dry-run: not connecting.",
            "--dry-run: keine Verbindung wird hergestellt."
        )
        .is_ok());
    }

    #[test]
    fn message_catalog_rejects_missing_codes_and_changed_placeholders() {
        let mut missing: serde_json::Value =
            serde_json::from_str(include_str!("../locales/messages.de.json")).unwrap();
        missing["messages"]
            .as_object_mut()
            .unwrap()
            .remove("DBP1001E");
        assert!(parse_message_catalog(Locale::De, &missing.to_string()).is_err());

        let mut changed: serde_json::Value =
            serde_json::from_str(include_str!("../locales/messages.de.json")).unwrap();
        changed["messages"]["DBP1504W"]["action"] =
            serde_json::Value::String("Prüfen Sie {ziel}.".to_string());
        assert!(parse_message_catalog(Locale::De, &changed.to_string()).is_err());
    }

    #[test]
    fn ui_catalog_rejects_missing_keys() {
        let mut catalog: serde_json::Value =
            serde_json::from_str(include_str!("../locales/ui.ja.json")).unwrap();
        catalog["text"]
            .as_object_mut()
            .unwrap()
            .remove("consent.prompt");
        assert!(parse_ui_catalog(Locale::Ja, &catalog.to_string()).is_err());
    }

    #[test]
    fn source_literals_reference_only_catalogued_ui_keys() {
        let sources = production_sources();
        let references = regex::Regex::new(
            r#"(?:i18n::(?:text|format)|crate::i18n::(?:text|format)|\btrf?|\bpreflight_(?:line|bullet|bullet_fmt))\s*\(\s*"([^"]+)""#,
        )
        .unwrap();
        let missing = sources
            .iter()
            .flat_map(|source| references.captures_iter(source))
            .filter_map(|capture| capture.get(1).map(|value| value.as_str()))
            .filter(|key| canonical_ui(key).is_none())
            .collect::<BTreeSet<_>>();
        assert!(missing.is_empty(), "uncatalogued UI keys: {missing:?}");
    }

    #[test]
    fn source_literals_reference_exactly_the_dbp_catalog() {
        let sources = production_sources();
        let references = regex::Regex::new(r"\bDBP[0-9]{4}[EWI]\b").unwrap();
        let actual = sources
            .iter()
            .flat_map(|source| references.find_iter(source))
            .map(|value| value.as_str())
            .collect::<BTreeSet<_>>();
        let expected = MESSAGE_SPECS
            .iter()
            .map(|message| message.code)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            actual, expected,
            "runtime DBP references drifted from catalog"
        );
    }

    #[test]
    fn orchestration_failure_literals_are_coded() {
        let production = include_str!("main.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let failure = regex::Regex::new(r#"\b(?:bail|anyhow)!\s*\(\s*"([^"]*)""#).unwrap();
        let uncoded = failure
            .captures_iter(production)
            .filter_map(|capture| capture.get(1).map(|value| value.as_str()))
            .filter(|literal| !literal.starts_with("DBP"))
            .collect::<Vec<_>>();
        assert!(
            uncoded.is_empty(),
            "orchestration failure literals need a DBP decision-boundary code: {uncoded:?}"
        );
    }

    #[test]
    fn operator_warning_templates_start_with_a_code() {
        for key in [
            "warning.audit_write",
            "engine.pg.connection_error",
            "engine.pg.tls_connection_error",
            "engine.pg.tls_fallback",
            "engine.rtt_failed",
            "engine.sample_budget",
            "engine.column_budget",
            "engine.compression_failed",
            "engine.style_failed",
            "engine.topology_unavailable",
            "engine.distributed_size_unavailable",
            "engine.dataset_scope_incomplete",
            "engine.sample_query_failed",
            "engine.sample_stream_failed",
            "security.mode_check_noop",
        ] {
            let value = canonical_ui(key).unwrap();
            assert!(
                value.starts_with("{code} "),
                "operator warning {key} must begin with its stable code: {value}"
            );
        }
    }
}
