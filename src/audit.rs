//! Audit log — deterministic per-run record of everything the tool did.
//!
//! Emitted to stderr on operational runs, and to `--audit-log PATH` if specified.
//! Help/version exits and failures before localization initialization do not
//! produce a full audit.
//! Format is plain-text, ordered, easy to grep, easy to archive.
//!
//! See ../AUDIT.md.

use std::fmt::Write as _;
use std::path::PathBuf;

use crate::format::{ArtifactInventory, DatabaseTopology, DatasetScope};
use crate::sample_compression::CompressionWorkReport;
use crate::secret::SecretSource;
use dbwarp_blueprint_core::BlueprintFidelityEstimate;

#[derive(Debug, Clone, Default)]
pub struct AuditLog {
    pub build_source_revision: String,
    pub build_source_dirty: String,
    pub build_toolchain: String,
    pub mode: String,
    /// Number of live `--schema` selectors. Source schema names are never
    /// copied into the audit.
    pub schema_selector_count: u64,
    pub started_at_unix_ms: u64,
    pub finished_at_unix_ms: u64,
    pub run_duration_ms: u64,
    pub connection: ConnectionAudit,
    pub queries: Vec<QueryAudit>,
    /// Identifier-free accounting for local Tier-2 compression and the
    /// conservative empty-table optimization. Database reads remain
    /// sequential regardless of the configured worker count.
    pub sampling_work: SamplingWorkAudit,
    pub bytes_read_from_server: BytesAudit,
    pub files_read_local: Vec<String>,
    pub files_written_local: Vec<FileWritten>,
    /// Non-fatal operator-visible degradations. Each entry begins with a
    /// stable DBP warning code so support can classify partial evidence.
    pub warnings: Vec<String>,
    pub network_egress: Vec<String>,
    pub env_vars_read: Vec<String>,
    pub trust_assertions: Vec<String>,
    /// MySQL length-fidelity mode selected for this run. This keeps the audit
    /// assertion aligned with the independent structural/observed policies.
    pub length_fidelity: Option<String>,
    /// Requested non-table artifact disclosure tier and the resulting
    /// bounded, name-free inventory summary.
    pub artifact_detail: Option<String>,
    pub artifact_inventory: Option<ArtifactInventoryAudit>,
    /// Bounded, name-free topology evidence and logical dataset coverage copied from
    /// the validated Blueprint. These structures intentionally have no fields
    /// capable of carrying member names, endpoints, or database identifiers.
    pub database_topology: Option<TopologyAudit>,
    pub dataset_scope: Option<DatasetScopeAudit>,
    /// Deterministic evidence-coverage estimate derived from the completed
    /// Blueprint. This is not a source-truth accuracy measurement.
    pub fidelity: Option<BlueprintFidelityEstimate>,
    /// Outcome of the run. `None` until finalize() is called. `Some(Ok)`
    /// on success, `Some(Err(message))` on any failure path. Rendered
    /// into the audit log on operational exits so customers have a
    /// forensic record of what stage failed and what the tool had
    /// already done by then.
    pub outcome: Option<Outcome>,
    /// When the customer pinned the Blueprint file's `generated_at` value
    /// via `--generated-at`, record both the fact that pinning happened
    /// AND the literal value. Empty when the flag wasn't passed (live
    /// timestamp used).
    pub generated_at_pin: Option<String>,
    /// Key provenance only; the HMAC key itself is never logged or emitted.
    pub anonymization_key_source: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Outcome {
    Ok,
    Err(String),
}

#[derive(Debug, Clone, Default)]
pub struct ConnectionAudit {
    pub uri_redacted: String,
    pub auth: String,
    pub user_source: Option<String>,
    /// Exact database identities observed on the established session. These
    /// belong only in the customer-local audit; they must never be copied into
    /// Blueprint, deck, or anonymous artifact output.
    pub authenticated_principal: Option<String>,
    pub effective_server_principal: Option<String>,
    pub database_principal: Option<String>,
    pub expected_server_principal: Option<String>,
    pub principal_assertion: Option<String>,
    pub password_source: Option<String>,
    pub password_persisted: bool,
    pub password_logged: bool,
    /// Distinguishes "the audit knows what the credential source WOULD
    /// have been" (set on dry-run via the `describe_secret_source`
    /// preview) from "we actually read a credential on this run" (set
    /// only by `acquire_secret` on success). The credential-handling
    /// trust assertion gates on this boolean — emitting it on a
    /// dry-run that never read anything would be a false claim.
    pub credential_actually_read: bool,
    pub tls_mode: String,
    pub tls_ca_path: Option<PathBuf>,
    pub tls_ca_only: bool,
    pub tls_client_cert: Option<PathBuf>,
    pub ssl_negotiated: String,
}

#[derive(Debug, Clone, Default)]
pub struct QueryAudit {
    pub seq: u32,
    pub elapsed_ms: u64,
    pub rows: Option<u64>,
    pub summary: String,
    pub outcome: QueryOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QueryOutcome {
    #[default]
    Succeeded,
    Failed,
}

impl QueryOutcome {
    fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct BytesAudit {
    /// Wire-byte totals are unavailable from the current database drivers.
    /// Keep them optional so the audit says `unknown` instead of presenting a
    /// fabricated zero or an encoded local buffer size as network evidence.
    pub catalog_wire_bytes: Option<u64>,
    pub row_wire_bytes: Option<u64>,
    /// Exact bytes in the local rowframe buffers passed to the compression
    /// routines. This is processing evidence, not a database wire-byte count.
    pub encoded_sample_bytes: u64,
}

#[derive(Debug, Clone, Default)]
pub struct SamplingWorkAudit {
    pub compression_workers: u64,
    /// Hard live-query payload ceiling per sampled table. Zero when Tier 2 was
    /// not enabled for this run.
    pub table_payload_limit_bytes: u64,
    pub compression_queue_capacity: u64,
    pub compression_jobs_submitted: u64,
    pub compression_jobs_completed: u64,
    pub compression_pipeline_wall_ms: u64,
    pub tables_skipped_proven_empty: u64,
    pub chunk_level_3_attempts: u64,
    pub table_level_3_attempts: u64,
    pub column_level_3_attempts: u64,
    /// Aggregate worker wall time. It may exceed pipeline wall time because
    /// workers overlap; it is not a process CPU counter.
    pub compression_worker_ms: u64,
}

#[derive(Debug, Clone, Default)]
pub struct ArtifactInventoryAudit {
    pub visibility: String,
    pub object_count: u64,
    pub dependency_edge_count: u64,
    pub external_prerequisite_count: u64,
    pub inventory_complete: bool,
    pub dependencies_complete: bool,
    pub analysis_complete: bool,
}

#[derive(Debug, Clone, Default)]
pub struct TopologyAudit {
    pub deployment: String,
    pub local_role: String,
    pub visibility: String,
    pub member_count: u64,
    pub identifiers_redacted: bool,
    pub role_counts: Vec<(String, u64)>,
    pub features: Vec<String>,
    pub catalogs_read: Vec<String>,
    pub catalogs_unreadable: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct DatasetScopeAudit {
    pub layout: String,
    pub table_inventory_completeness: String,
    pub row_count_completeness: String,
    pub size_completeness: String,
    pub row_count_method: String,
    pub size_method: String,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FileWritten {
    pub path: PathBuf,
    pub bytes: u64,
    pub sha256: String,
}

impl AuditLog {
    pub fn new(mode: &str, started_at_unix_ms: u64) -> Self {
        Self {
            build_source_revision: option_env!("DBWARP_BLUEPRINT_BUILD_REVISION")
                .unwrap_or("(not stamped at build time)")
                .to_string(),
            build_source_dirty: option_env!("DBWARP_BLUEPRINT_BUILD_DIRTY")
                .unwrap_or("unknown")
                .to_string(),
            build_toolchain: format!(
                "{} (vendored)",
                option_env!("DBWARP_BLUEPRINT_TOOLCHAIN").unwrap_or("rustc-unspecified")
            ),
            mode: mode.to_string(),
            started_at_unix_ms,
            ..Self::default()
        }
    }

    pub fn record_password_source(&mut self, source: &SecretSource) {
        self.connection.password_source = Some(source.audit_str());
        // `password_logged` is true if the source itself was insecure (e.g.
        // embedded in connection string visible to ps).
        self.connection.password_logged = matches!(source, SecretSource::ConnectionString);
    }

    pub fn record_database_principals(
        &mut self,
        authenticated: &str,
        effective_server: &str,
        database: &str,
        expected_server: Option<&str>,
        assertion: &str,
    ) {
        self.connection.authenticated_principal = Some(single_line_identity(authenticated));
        self.connection.effective_server_principal = Some(single_line_identity(effective_server));
        self.connection.database_principal = Some(single_line_identity(database));
        self.connection.expected_server_principal = expected_server.map(single_line_identity);
        self.connection.principal_assertion = Some(assertion.to_string());
    }

    pub fn record_query(&mut self, summary: &str, elapsed_ms: u64, rows: u64) {
        let seq = (self.queries.len() as u32) + 1;
        self.queries.push(QueryAudit {
            seq,
            elapsed_ms,
            rows: Some(rows),
            summary: summary.to_string(),
            outcome: QueryOutcome::Succeeded,
        });
    }

    /// Record a database operation that was attempted but did not complete.
    /// `summary` must remain identifier-free; raw driver errors belong in the
    /// localized terminal diagnostic only after they have been redacted.
    pub fn record_query_failure(&mut self, summary: &str, elapsed_ms: u64) {
        let seq = (self.queries.len() as u32) + 1;
        self.queries.push(QueryAudit {
            seq,
            elapsed_ms,
            rows: None,
            summary: summary.to_string(),
            outcome: QueryOutcome::Failed,
        });
    }

    pub fn configure_compression_workers(&mut self, workers: usize, queue_capacity: usize) {
        self.sampling_work.compression_workers = workers.try_into().unwrap_or(u64::MAX);
        self.sampling_work.compression_queue_capacity =
            queue_capacity.try_into().unwrap_or(u64::MAX);
        self.sampling_work.table_payload_limit_bytes =
            crate::engine_common::MAX_LIVE_TABLE_SAMPLE_BYTES as u64;
    }

    pub fn record_compression_job_submitted(&mut self) {
        self.sampling_work.compression_jobs_submitted = self
            .sampling_work
            .compression_jobs_submitted
            .saturating_add(1);
    }

    pub fn record_compression_job_completed(&mut self, work: &CompressionWorkReport) {
        let audit = &mut self.sampling_work;
        audit.compression_jobs_completed = audit.compression_jobs_completed.saturating_add(1);
        audit.chunk_level_3_attempts = audit
            .chunk_level_3_attempts
            .saturating_add(work.chunk_level_3_attempts);
        audit.table_level_3_attempts = audit
            .table_level_3_attempts
            .saturating_add(work.table_level_3_attempts);
        audit.column_level_3_attempts = audit
            .column_level_3_attempts
            .saturating_add(work.column_level_3_attempts);
        audit.compression_worker_ms = audit
            .compression_worker_ms
            .saturating_add(work.compression_ms);
    }

    pub fn record_compression_pipeline_wall(&mut self, elapsed_ms: u64) {
        self.sampling_work.compression_pipeline_wall_ms = self
            .sampling_work
            .compression_pipeline_wall_ms
            .saturating_add(elapsed_ms);
    }

    pub fn record_proven_empty_table_skipped(&mut self) {
        self.sampling_work.tables_skipped_proven_empty = self
            .sampling_work
            .tables_skipped_proven_empty
            .saturating_add(1);
    }

    /// Account for the exact local row-frame bytes without allowing a wrapped
    /// audit value to look smaller than the work actually performed.
    pub fn record_encoded_sample_bytes(&mut self, bytes: u64) -> anyhow::Result<()> {
        self.bytes_read_from_server.encoded_sample_bytes = self
            .bytes_read_from_server
            .encoded_sample_bytes
            .checked_add(bytes)
            .ok_or_else(|| anyhow::anyhow!("encoded sample-byte audit total exceeds u64"))?;
        Ok(())
    }

    pub fn record_file_read(&mut self, path: &str) {
        let s = path.to_string();
        if !self.files_read_local.contains(&s) {
            self.files_read_local.push(s);
        }
    }

    pub fn record_env_var_read(&mut self, var_name: &str) {
        let s = var_name.to_string();
        if !self.env_vars_read.contains(&s) {
            self.env_vars_read.push(s);
        }
    }

    /// Enumerate any TLS PEM files the tool will open. Called by
    /// each engine module just before constructing rustls config (or, for
    /// MySQL, just before `std::fs::read(ca)`). Idempotent — duplicates
    /// are filtered.
    pub fn record_tls_file_reads(
        &mut self,
        ca_bundle: Option<&PathBuf>,
        client_cert: Option<&PathBuf>,
        client_key: Option<&PathBuf>,
    ) {
        if let Some(p) = ca_bundle {
            self.record_file_read(&p.display().to_string());
        }
        if let Some(p) = client_cert {
            self.record_file_read(&p.display().to_string());
        }
        if let Some(p) = client_key {
            self.record_file_read(&p.display().to_string());
        }
    }

    pub fn record_file_written(&mut self, path: PathBuf, bytes: u64, sha256: String) {
        self.files_written_local.push(FileWritten {
            path,
            bytes,
            sha256,
        });
    }

    pub fn record_warning(&mut self, code: &str, detail: impl Into<String>) {
        let detail = detail.into().replace(['\r', '\n'], " ");
        let warning = if detail
            .strip_prefix(code)
            .is_some_and(|rest| rest.starts_with(char::is_whitespace))
        {
            detail
        } else {
            format!("{code} {detail}")
        };
        if !self.warnings.contains(&warning) {
            self.warnings.push(warning);
        }
    }

    pub fn record_artifact_inventory(&mut self, inventory: Option<&ArtifactInventory>) {
        self.artifact_inventory = inventory.map(|inventory| ArtifactInventoryAudit {
            visibility: inventory.visibility.clone(),
            object_count: inventory.object_count,
            dependency_edge_count: inventory.dependency_edge_count,
            external_prerequisite_count: inventory.external_prerequisite_count,
            inventory_complete: inventory.inventory_complete,
            dependencies_complete: inventory.dependencies_complete,
            analysis_complete: inventory.analysis_complete,
        });
    }

    pub fn record_sizing_scope(
        &mut self,
        topology: Option<&DatabaseTopology>,
        scope: Option<&DatasetScope>,
    ) {
        self.database_topology = topology.map(|topology| TopologyAudit {
            deployment: topology.deployment.clone(),
            local_role: topology.local_role.clone(),
            visibility: topology.visibility.clone(),
            member_count: topology.member_count,
            identifiers_redacted: topology.identifiers_redacted,
            role_counts: topology
                .role_counts
                .iter()
                .map(|(role, count)| (role.clone(), *count))
                .collect(),
            features: topology.features.clone(),
            catalogs_read: topology.catalogs_read.clone(),
            catalogs_unreadable: topology.catalogs_unreadable.clone(),
        });
        self.dataset_scope = scope.map(|scope| DatasetScopeAudit {
            layout: scope.layout.clone(),
            table_inventory_completeness: scope.table_inventory_completeness.clone(),
            row_count_completeness: scope.row_count_completeness.clone(),
            size_completeness: scope.size_completeness.clone(),
            row_count_method: scope.row_count_method.clone(),
            size_method: scope.size_method.clone(),
            limitations: scope.limitations.clone(),
        });
    }

    pub fn record_fidelity(&mut self, estimate: BlueprintFidelityEstimate) {
        self.fidelity = Some(estimate);
    }

    pub fn finalize(&mut self, finished_at_unix_ms: u64) {
        self.finished_at_unix_ms = finished_at_unix_ms;
        self.run_duration_ms = finished_at_unix_ms.saturating_sub(self.started_at_unix_ms);
        if self.outcome.is_none() {
            // Caller didn't explicitly set an outcome — assume success.
            self.outcome = Some(Outcome::Ok);
        }
        // Default trust assertions; engine module can append more.
        if self.mode == "tier-1" {
            self.trust_assertions
                .push("no row content was read".to_string());
        }
        self.trust_assertions
            .push("no telemetry was sent anywhere".to_string());
        match self.length_fidelity.as_deref() {
            Some("balanced") => self.trust_assertions.push(
                "length policy balanced: declared capacities and index prefixes exact; sampled lengths relatively rounded"
                    .to_string(),
            ),
            Some("exact") => self.trust_assertions.push(
                "length policy exact: declared, index-prefix, and sampled lengths preserved by explicit operator consent"
                    .to_string(),
            ),
            Some("strict") => self.trust_assertions.push(
                "length policy strict: declared, index-prefix, and sampled lengths coarsely rounded"
                    .to_string(),
            ),
            _ => self
                .trust_assertions
                .push("numeric statistics rounded to documented precision".to_string()),
        }
        match self.anonymization_key_source.as_deref() {
            Some("ephemeral-random") => self.trust_assertions.push(
                "identifier ordering uses domain-separated HMAC-SHA256 with a fresh process-local key; labels intentionally vary between runs"
                    .to_string(),
            ),
            Some("customer-key-file") => self.trust_assertions.push(
                "identifier ordering uses domain-separated HMAC-SHA256 with a customer-held key; labels are stable only when that key is reused"
                    .to_string(),
            ),
            _ => {}
        }
        if self.anonymization_key_source.is_some() {
            self.trust_assertions.push(
                "the anonymization key and source identifiers are not written to the Blueprint"
                    .to_string(),
            );
        }
        match self.artifact_detail.as_deref() {
            Some("summary") => self.trust_assertions.push(
                "artifact summary stores bounded counts and external-prerequisite classes; no object identities or definitions"
                    .to_string(),
            ),
            Some("graph") => self.trust_assertions.push(
                "artifact graph stores keyed anonymous identifiers and internally consistent edges; no source names or definitions"
                    .to_string(),
            ),
            Some("analyzed") => self.trust_assertions.push(
                "artifact definitions were analyzed transiently; only bounded feature bands and anonymous identifiers were stored"
                    .to_string(),
            ),
            Some("none") | None => {}
            Some(_) => self
                .trust_assertions
                .push("artifact capture used an unrecognized audit mode".to_string()),
        }
        if self
            .artifact_detail
            .as_deref()
            .is_some_and(|detail| detail != "none")
        {
            self.trust_assertions.push(
                "artifact output excludes source object names, SQL text, endpoints, credentials, keys, certificates, and binaries"
                .to_string(),
            );
        }
        if self.database_topology.is_some() || self.dataset_scope.is_some() {
            self.trust_assertions.push(
                "topology and dataset-scope evidence stores only closed tokens and counts; infrastructure identifiers were discarded"
                    .to_string(),
            );
        }
        // Only emit the credential-handling assertion when a credential
        // was *actually* read on this run. Gates on
        // `credential_actually_read` (set only by `acquire_secret` on
        // success) — NOT on `password_source.is_some()`, which is also
        // set on the `--dry-run` path via `describe_secret_source`
        // preview. Gating on `password_source` would fire the assertion
        // on dry-run even though no credential had been read.
        if self.connection.credential_actually_read {
            self.trust_assertions.push(
                "credential entered through the Secret wrapper and its buffer is zeroized on drop; \
                 driver APIs may retain copies as documented under 'Driver-owned credential copies' in SECURITY.md"
                    .to_string(),
            );
        }
    }

    /// Record a failure outcome. Caller still must call `finalize()` and
    /// `render()` to emit the audit on the error exit path.
    pub fn mark_failure(&mut self, stage_and_reason: impl Into<String>) {
        self.outcome = Some(Outcome::Err(stage_and_reason.into()));
    }

    /// Emit the audit log as plain text. Deterministic for the same inputs.
    pub fn render(&self) -> String {
        let mut s = String::with_capacity(2048);
        writeln!(s, "=== dbwarp-blueprint audit ===").ok();
        writeln!(
            s,
            "build_source_revision: {}",
            or_unset(&self.build_source_revision)
        )
        .ok();
        writeln!(
            s,
            "build_source_dirty:    {}",
            or_unset(&self.build_source_dirty)
        )
        .ok();
        writeln!(s, "build_toolchain:     {}", self.build_toolchain).ok();
        writeln!(s, "mode:                {}", self.mode).ok();
        writeln!(s, "started_at_unix_ms:  {}", self.started_at_unix_ms).ok();
        match self.outcome.as_ref() {
            Some(Outcome::Ok) => {
                writeln!(s, "outcome:             ok").ok();
            }
            Some(Outcome::Err(why)) => {
                writeln!(s, "outcome:             error: {}", why).ok();
            }
            None => {
                // finalize() should have set this; defensive only.
                writeln!(s, "outcome:             (not finalized)").ok();
            }
        }
        if let Some(pin) = &self.generated_at_pin {
            writeln!(s, "generated_at_pin:    {} (--generated-at)", pin).ok();
        }
        if let Some(source) = &self.anonymization_key_source {
            writeln!(s, "anonymization_key:   {source}").ok();
        }
        writeln!(s, "schema_selector_count: {}", self.schema_selector_count).ok();
        writeln!(s).ok();
        writeln!(s, "connection:").ok();
        writeln!(s, "  - {}", self.connection.uri_redacted).ok();
        writeln!(s, "    auth: {}", or_unset(&self.connection.auth)).ok();
        writeln!(s, "    tls: {}", or_unset(&self.connection.tls_mode)).ok();
        if let Some(ca) = &self.connection.tls_ca_path {
            writeln!(s, "    tls_ca_path: {}", ca.display()).ok();
        }
        writeln!(s, "    tls_ca_only: {}", self.connection.tls_ca_only).ok();
        if let Some(cc) = &self.connection.tls_client_cert {
            writeln!(s, "    tls_client_cert: {}", cc.display()).ok();
        }
        if !self.connection.ssl_negotiated.is_empty() {
            writeln!(s, "    ssl_negotiated: {}", self.connection.ssl_negotiated).ok();
        }
        writeln!(s).ok();
        writeln!(s, "auth:").ok();
        writeln!(
            s,
            "  user_source:        {}",
            self.connection
                .user_source
                .as_deref()
                .unwrap_or("(uri-or-default)")
        )
        .ok();
        writeln!(
            s,
            "  authenticated_principal: {}",
            self.connection
                .authenticated_principal
                .as_deref()
                .unwrap_or("(not observed)")
        )
        .ok();
        writeln!(
            s,
            "  effective_server_principal: {}",
            self.connection
                .effective_server_principal
                .as_deref()
                .unwrap_or("(not observed)")
        )
        .ok();
        writeln!(
            s,
            "  database_principal: {}",
            self.connection
                .database_principal
                .as_deref()
                .unwrap_or("(not observed)")
        )
        .ok();
        writeln!(
            s,
            "  expected_server_principal: {}",
            self.connection
                .expected_server_principal
                .as_deref()
                .unwrap_or("(not requested)")
        )
        .ok();
        writeln!(
            s,
            "  principal_assertion: {}",
            self.connection
                .principal_assertion
                .as_deref()
                .unwrap_or("not-observed")
        )
        .ok();
        writeln!(
            s,
            "  password_source:    {}",
            self.connection
                .password_source
                .as_deref()
                .unwrap_or("(none)")
        )
        .ok();
        writeln!(
            s,
            "  password_persisted: {}",
            self.connection.password_persisted
        )
        .ok();
        writeln!(
            s,
            "  password_logged:    {}",
            self.connection.password_logged
        )
        .ok();
        writeln!(s).ok();

        writeln!(s, "topology_and_scope:").ok();
        if let Some(topology) = &self.database_topology {
            writeln!(s, "  topology:").ok();
            writeln!(s, "    deployment: {}", topology.deployment).ok();
            writeln!(s, "    local_role: {}", topology.local_role).ok();
            writeln!(s, "    visibility: {}", topology.visibility).ok();
            writeln!(s, "    member_count: {}", topology.member_count).ok();
            writeln!(
                s,
                "    identifiers_redacted: {}",
                topology.identifiers_redacted
            )
            .ok();
            render_token_counts(&mut s, "    role_counts", &topology.role_counts);
            render_tokens(&mut s, "    features", &topology.features);
            render_tokens(&mut s, "    catalogs_read", &topology.catalogs_read);
            render_tokens(
                &mut s,
                "    catalogs_unreadable",
                &topology.catalogs_unreadable,
            );
        } else {
            writeln!(s, "  topology: (none)").ok();
        }
        if let Some(scope) = &self.dataset_scope {
            writeln!(s, "  dataset_scope:").ok();
            writeln!(s, "    layout: {}", scope.layout).ok();
            writeln!(
                s,
                "    table_inventory_completeness: {}",
                scope.table_inventory_completeness
            )
            .ok();
            writeln!(
                s,
                "    row_count_completeness: {}",
                scope.row_count_completeness
            )
            .ok();
            writeln!(s, "    size_completeness: {}", scope.size_completeness).ok();
            writeln!(s, "    row_count_method: {}", scope.row_count_method).ok();
            writeln!(s, "    size_method: {}", scope.size_method).ok();
            render_tokens(&mut s, "    limitations", &scope.limitations);
        } else {
            writeln!(s, "  dataset_scope: (none)").ok();
        }
        writeln!(s).ok();

        writeln!(s, "blueprint_fidelity_estimate:").ok();
        if let Some(fidelity) = &self.fidelity {
            writeln!(s, "  basis: evidence-coverage-v1").ok();
            writeln!(s, "  overall_score: {}/100", fidelity.overall_score).ok();
            writeln!(s, "  band: {}", fidelity.band).ok();
            writeln!(s, "  structure_score: {}/100", fidelity.structure_score).ok();
            writeln!(s, "  sizing_score: {}/100", fidelity.sizing_score).ok();
            writeln!(
                s,
                "  column_statistics_score: {}/100",
                fidelity.column_statistics_score
            )
            .ok();
            writeln!(
                s,
                "  relationship_score: {}/100",
                fidelity.relationship_score
            )
            .ok();
            writeln!(s, "  artifact_score: {}/100", fidelity.artifact_score).ok();
            render_tokens(&mut s, "  limitations", &fidelity.limitations);
            writeln!(
                s,
                "  qualification: evidence estimate, not source-truth accuracy or a confidence interval"
            )
            .ok();
        } else {
            writeln!(s, "  (not available for this operation)").ok();
        }
        writeln!(s).ok();

        writeln!(s, "artifact_inventory:").ok();
        writeln!(
            s,
            "  detail: {}",
            self.artifact_detail.as_deref().unwrap_or("(not requested)")
        )
        .ok();
        if let Some(inventory) = &self.artifact_inventory {
            writeln!(s, "  visibility: {}", inventory.visibility).ok();
            writeln!(s, "  objects: {}", inventory.object_count).ok();
            writeln!(s, "  dependency_edges: {}", inventory.dependency_edge_count).ok();
            writeln!(
                s,
                "  external_prerequisites: {}",
                inventory.external_prerequisite_count
            )
            .ok();
            writeln!(s, "  inventory_complete: {}", inventory.inventory_complete).ok();
            writeln!(
                s,
                "  dependencies_complete: {}",
                inventory.dependencies_complete
            )
            .ok();
            writeln!(s, "  analysis_complete: {}", inventory.analysis_complete).ok();
        } else {
            writeln!(s, "  (none)").ok();
        }
        writeln!(s).ok();

        writeln!(s, "database_operations_observed:").ok();
        if self.queries.is_empty() {
            writeln!(s, "  (none)").ok();
        }
        for q in &self.queries {
            match q.rows {
                Some(rows) => writeln!(
                    s,
                    "  {}. [{}, {}ms, {} rows]   {}",
                    q.seq,
                    q.outcome.as_str(),
                    q.elapsed_ms,
                    rows,
                    q.summary
                )
                .ok(),
                None => writeln!(
                    s,
                    "  {}. [{}, {}ms, rows unknown]   {}",
                    q.seq,
                    q.outcome.as_str(),
                    q.elapsed_ms,
                    q.summary
                )
                .ok(),
            };
        }
        writeln!(s).ok();

        writeln!(s, "wire_bytes_observed:").ok();
        writeln!(
            s,
            "  catalog_responses: {}",
            optional_human_bytes(self.bytes_read_from_server.catalog_wire_bytes)
        )
        .ok();
        writeln!(
            s,
            "  row_data:          {}",
            optional_human_bytes(self.bytes_read_from_server.row_wire_bytes)
        )
        .ok();
        writeln!(s, "local_sample_processing:").ok();
        writeln!(
            s,
            "  encoded_rowframe_bytes: {}",
            human_bytes(self.bytes_read_from_server.encoded_sample_bytes)
        )
        .ok();
        writeln!(s).ok();

        let work = &self.sampling_work;
        writeln!(s, "sampling_work:").ok();
        writeln!(
            s,
            "  table_payload_limit_bytes: {}",
            work.table_payload_limit_bytes
        )
        .ok();
        writeln!(s, "  compression_workers: {}", work.compression_workers).ok();
        writeln!(
            s,
            "  compression_queue_capacity: {}",
            work.compression_queue_capacity
        )
        .ok();
        writeln!(
            s,
            "  compression_jobs_submitted: {}",
            work.compression_jobs_submitted
        )
        .ok();
        writeln!(
            s,
            "  compression_jobs_completed: {}",
            work.compression_jobs_completed
        )
        .ok();
        writeln!(
            s,
            "  compression_pipeline_wall_ms: {}",
            work.compression_pipeline_wall_ms
        )
        .ok();
        writeln!(s, "  compression_worker_ms: {}", work.compression_worker_ms).ok();
        writeln!(
            s,
            "  tables_skipped_proven_empty: {}",
            work.tables_skipped_proven_empty
        )
        .ok();
        writeln!(
            s,
            "  chunk_level_3_attempts: {}",
            work.chunk_level_3_attempts
        )
        .ok();
        writeln!(
            s,
            "  table_level_3_attempts: {}",
            work.table_level_3_attempts
        )
        .ok();
        writeln!(
            s,
            "  column_level_3_attempts: {}",
            work.column_level_3_attempts
        )
        .ok();
        writeln!(s).ok();

        writeln!(s, "files_read_local:").ok();
        if self.files_read_local.is_empty() {
            writeln!(s, "  (none)").ok();
        }
        for f in &self.files_read_local {
            writeln!(s, "  - {f}").ok();
        }
        writeln!(s).ok();

        writeln!(s, "files_written_local:").ok();
        if self.files_written_local.is_empty() {
            writeln!(s, "  (none)").ok();
        }
        for f in &self.files_written_local {
            writeln!(
                s,
                "  - {}  ({} bytes, sha256: {})",
                f.path.display(),
                f.bytes,
                short_sha(&f.sha256)
            )
            .ok();
        }
        writeln!(s).ok();

        writeln!(s, "warnings:").ok();
        if self.warnings.is_empty() {
            writeln!(s, "  (none)").ok();
        }
        for warning in &self.warnings {
            writeln!(s, "  - {warning}").ok();
        }
        writeln!(s).ok();

        writeln!(s, "network_egress:").ok();
        if self.network_egress.is_empty() {
            if self.mode == "deck-from-toml" {
                writeln!(s, "  - none").ok();
            } else {
                writeln!(s, "  - none beyond DB connection").ok();
            }
        }
        for e in &self.network_egress {
            writeln!(s, "  - {e}").ok();
        }
        writeln!(s).ok();

        writeln!(s, "env_vars_read:").ok();
        if self.env_vars_read.is_empty() {
            writeln!(s, "  - (none)").ok();
        }
        for v in &self.env_vars_read {
            writeln!(s, "  - {v}").ok();
        }
        writeln!(s).ok();

        writeln!(s, "trust_assertions:").ok();
        for t in &self.trust_assertions {
            writeln!(s, "  - {t}").ok();
        }
        writeln!(s).ok();

        writeln!(s, "run_duration_ms:     {}", self.run_duration_ms).ok();
        writeln!(s, "finished_at_unix_ms: {}", self.finished_at_unix_ms).ok();
        writeln!(s, "=== end audit ===").ok();
        s
    }
}

fn or_unset(s: &str) -> &str {
    if s.is_empty() {
        "(unset)"
    } else {
        s
    }
}

fn single_line_identity(value: &str) -> String {
    let mut rendered = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\n' => rendered.push_str("\\n"),
            '\r' => rendered.push_str("\\r"),
            '\t' => rendered.push_str("\\t"),
            ch if ch.is_control() || crate::i18n::is_forbidden_format_control(ch) => {
                use std::fmt::Write as _;
                write!(rendered, "\\u{{{:x}}}", ch as u32).ok();
            }
            ch => rendered.push(ch),
        }
    }
    rendered
}

fn human_bytes(n: u64) -> String {
    if n == 0 {
        "0 B".to_string()
    } else if n < 1024 {
        format!("{n} B")
    } else if n < 1_048_576 {
        format!("{:.1} KiB", n as f64 / 1024.0)
    } else if n < 1_073_741_824 {
        format!("{:.1} MiB", n as f64 / 1_048_576.0)
    } else {
        format!("{:.2} GiB", n as f64 / 1_073_741_824.0)
    }
}

fn optional_human_bytes(n: Option<u64>) -> String {
    n.map(human_bytes)
        .unwrap_or_else(|| "unknown (driver does not expose wire-byte totals)".to_string())
}

fn short_sha(s: &str) -> String {
    if s.len() >= 12 {
        format!("{}...", &s[..12])
    } else {
        s.to_string()
    }
}

fn render_tokens(output: &mut String, label: &str, values: &[String]) {
    if values.is_empty() {
        writeln!(output, "{label}: (none)").ok();
    } else {
        writeln!(output, "{label}: {}", values.join(", ")).ok();
    }
}

fn render_token_counts(output: &mut String, label: &str, values: &[(String, u64)]) {
    if values.is_empty() {
        writeln!(output, "{label}: (none)").ok();
    } else {
        let rendered = values
            .iter()
            .map(|(token, count)| format!("{token}={count}"))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(output, "{label}: {rendered}").ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_render_minimal() {
        let mut a = AuditLog::new("tier-1", 1_000);
        a.connection.uri_redacted = "postgresql://app@example:5432/payments".to_string();
        a.connection.auth = "scram-sha-256".to_string();
        a.connection.tls_mode = "require".to_string();
        a.record_database_principals("app", "app", "app_reader", Some("app"), "matched");
        a.record_password_source(&SecretSource::Env {
            var_name: "DBPASS".to_string(),
        });
        a.record_query("SELECT ... FROM pg_class JOIN pg_namespace", 12, 28);
        a.bytes_read_from_server.catalog_wire_bytes = Some(12_345);
        a.finalize(2_000);
        let s = a.render();
        assert!(s.contains("=== dbwarp-blueprint audit ==="));
        assert!(s.contains("password_source:    env:DBPASS"));
        assert!(s.contains("authenticated_principal: app"));
        assert!(s.contains("effective_server_principal: app"));
        assert!(s.contains("database_principal: app_reader"));
        assert!(s.contains("expected_server_principal: app"));
        assert!(s.contains("principal_assertion: matched"));
        assert!(s.contains("[succeeded, 12ms, 28 rows]"));
        assert!(s.contains("=== end audit ==="));
        // No credential value ever appears.
        assert!(!s.contains("hunter2"));
    }

    #[test]
    fn database_principals_cannot_inject_audit_lines() {
        let mut audit = AuditLog::new("tier-1", 1_000);
        audit.record_database_principals(
            "DOMAIN\\svc\noutcome: ok",
            "DOMAIN\\svc\radmin",
            "reader\trole",
            Some("DOMAIN\\svc\u{202e}"),
            "matched",
        );
        let rendered = audit.render();
        assert!(rendered.contains("authenticated_principal: DOMAIN\\svc\\noutcome: ok"));
        assert!(!rendered.contains("\noutcome: ok\n"));
        assert!(rendered.contains("effective_server_principal: DOMAIN\\svc\\radmin"));
        assert!(rendered.contains("database_principal: reader\\trole"));
        assert!(rendered.contains("expected_server_principal: DOMAIN\\svc\\u{202e}"));
    }

    #[test]
    fn audit_does_not_turn_unobserved_wire_bytes_into_zero() {
        let mut audit = AuditLog::new("tier-2", 1_000);
        audit.bytes_read_from_server.encoded_sample_bytes = 4_096;
        audit.record_query_failure(
            "PostgreSQL live capture (hard wall-time limit reached)",
            500,
        );
        audit.finalize(1_500);
        let rendered = audit.render();
        assert!(rendered
            .contains("catalog_responses: unknown (driver does not expose wire-byte totals)"));
        assert!(rendered.contains("row_data:          unknown"));
        assert!(rendered.contains("encoded_rowframe_bytes: 4.0 KiB"));
        assert!(rendered.contains("[failed, 500ms, rows unknown]"));
    }

    #[test]
    fn encoded_sample_byte_accounting_rejects_overflow() {
        let mut audit = AuditLog::new("tier-2", 1_000);
        audit.bytes_read_from_server.encoded_sample_bytes = u64::MAX;
        let error = audit.record_encoded_sample_bytes(1).unwrap_err();
        assert!(error.to_string().contains("exceeds u64"));
        assert_eq!(audit.bytes_read_from_server.encoded_sample_bytes, u64::MAX);
    }

    #[test]
    fn sampling_work_is_identifier_free_and_distinguishes_worker_from_pipeline_time() {
        let mut audit = AuditLog::new("tier-2", 1_000);
        audit.configure_compression_workers(4, 4);
        audit.record_compression_job_submitted();
        audit.record_compression_job_completed(&CompressionWorkReport {
            chunk_level_3_attempts: 8,
            table_level_3_attempts: 1,
            column_level_3_attempts: 3,
            compression_ms: 25,
        });
        audit.record_compression_pipeline_wall(10);
        audit.record_proven_empty_table_skipped();

        let rendered = audit.render();
        assert!(rendered.contains("compression_workers: 4"));
        assert!(rendered.contains("compression_jobs_submitted: 1"));
        assert!(rendered.contains("compression_jobs_completed: 1"));
        assert!(rendered.contains("compression_pipeline_wall_ms: 10"));
        assert!(rendered.contains("compression_worker_ms: 25"));
        assert!(rendered.contains("tables_skipped_proven_empty: 1"));
        assert!(rendered.contains(
            "table_level_3_attempts: 1\n  column_level_3_attempts: 3\n\nfiles_read_local:"
        ));
    }

    /// The credential-handling trust assertion must NOT fire on a
    /// dry-run that recorded a password_source via
    /// `describe_secret_source` preview but never actually called
    /// `acquire_secret`. The gate is on `credential_actually_read`,
    /// not `password_source.is_some()`.
    #[test]
    fn dry_run_does_not_emit_credential_trust_assertion() {
        let mut a = AuditLog::new("tier-1", 1_000);
        a.connection.uri_redacted = "postgresql://app@example:5432/payments".to_string();
        // Simulate the dry-run preview path: password_source is set
        // (so the audit shows what the source WOULD have been) but
        // credential_actually_read remains false.
        a.record_password_source(&SecretSource::File {
            path: std::path::PathBuf::from("/etc/dbwarp/db.pass"),
            mode: Some(0o600),
        });
        assert!(!a.connection.credential_actually_read);
        a.finalize(2_000);
        let s = a.render();
        assert!(
            !s.contains("credential entered through the Secret wrapper"),
            "dry-run audit must not assert a credential was read; got:\n{s}"
        );
    }

    /// Mirror test: when credential_actually_read IS set (the
    /// production success path), the trust assertion DOES fire.
    #[test]
    fn real_run_emits_credential_trust_assertion() {
        let mut a = AuditLog::new("tier-1", 1_000);
        a.connection.uri_redacted = "postgresql://app@example:5432/payments".to_string();
        a.record_password_source(&SecretSource::File {
            path: std::path::PathBuf::from("/etc/dbwarp/db.pass"),
            mode: Some(0o600),
        });
        a.connection.credential_actually_read = true;
        a.finalize(2_000);
        let s = a.render();
        assert!(
            s.contains("credential entered through the Secret wrapper"),
            "real run audit must assert credential handling; got:\n{s}"
        );
    }

    #[test]
    fn anonymization_trust_assertions_match_key_provenance() {
        for (source, expected) in [
            (
                "ephemeral-random",
                "fresh process-local key; labels intentionally vary between runs",
            ),
            (
                "customer-key-file",
                "customer-held key; labels are stable only when that key is reused",
            ),
        ] {
            let mut audit = AuditLog::new("tier-1", 1_000);
            audit.anonymization_key_source = Some(source.to_string());
            audit.finalize(2_000);
            let rendered = audit.render();
            assert!(rendered.contains(expected), "audit was:\n{rendered}");
            assert!(rendered.contains(
                "the anonymization key and source identifiers are not written to the Blueprint"
            ));
            assert!(!rendered.contains("sha256-based"));
            assert!(!rendered.contains("no random or pseudorandom data in output"));
        }
    }

    #[test]
    fn warnings_are_coded_flattened_and_deduplicated() {
        let mut a = AuditLog::new("tier-2", 1_000);
        a.record_warning("DBP1407W", "DBP1407W sample failed\nfor table-001");
        a.record_warning("DBP1407W", "DBP1407W sample failed\r\nfor table-001");
        a.finalize(2_000);
        let rendered = a.render();
        assert_eq!(
            rendered
                .matches("DBP1407W sample failed for table-001")
                .count(),
            1
        );
        assert!(!rendered.contains("DBP1407W DBP1407W"));
    }

    #[test]
    fn topology_and_scope_audit_is_identifier_free() {
        let mut topology = DatabaseTopology::unknown();
        topology.deployment = "availability-group".to_string();
        topology.local_role = "secondary".to_string();
        topology.visibility = "partial".to_string();
        topology.member_count = 3;
        topology.role_counts.insert("primary".to_string(), 1);
        topology.role_counts.insert("secondary".to_string(), 2);
        topology.features = vec!["sqlserver-always-on".to_string()];
        topology.catalogs_read = vec!["sqlserver-hadr-replica-states".to_string()];

        let mut scope = DatasetScope::unknown_database(
            "sqlserver-dm-db-partition-stats",
            "sqlserver-dm-db-partition-stats",
        );
        scope.layout = "full-copy".to_string();
        scope.table_inventory_completeness = "complete".to_string();

        let mut audit = AuditLog::new("tier-1", 1_000);
        audit.record_sizing_scope(Some(&topology), Some(&scope));
        audit.finalize(2_000);
        let rendered = audit.render();

        assert!(rendered.contains("deployment: availability-group"));
        assert!(rendered.contains("role_counts: primary=1, secondary=2"));
        assert!(rendered.contains("row_count_method: sqlserver-dm-db-partition-stats"));
        assert!(rendered.contains("infrastructure identifiers were discarded"));
        assert!(!rendered.contains("primary.internal.example"));
        assert!(!rendered.contains("replica-01"));
    }

    #[test]
    fn fidelity_audit_is_dimensioned_and_explicitly_qualified() {
        let mut audit = AuditLog::new("tier-2", 1_000);
        audit.schema_selector_count = 2;
        audit.record_fidelity(BlueprintFidelityEstimate {
            overall_score: 81,
            band: "good",
            structure_score: 100,
            sizing_score: 100,
            column_statistics_score: 60,
            relationship_score: 75,
            artifact_score: 50,
            limitations: vec!["biased-column-sampling".to_string()],
        });
        audit.finalize(2_000);
        let rendered = audit.render();
        assert!(rendered.contains("schema_selector_count: 2"));
        assert!(rendered.contains("overall_score: 81/100"));
        assert!(rendered.contains("column_statistics_score: 60/100"));
        assert!(rendered.contains("limitations: biased-column-sampling"));
        assert!(rendered.contains("not source-truth accuracy or a confidence interval"));
    }

    #[test]
    fn human_bytes_formats() {
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(2048), "2.0 KiB");
        assert_eq!(human_bytes(2 * 1_048_576), "2.0 MiB");
    }
}
