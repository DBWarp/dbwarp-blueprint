#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct BatchManifest {
    #[serde(default)]
    defaults: BatchDefaults,
    #[serde(default, rename = "source")]
    sources: Vec<BatchSource>,
}
#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct BatchDefaults {
    #[serde(default)]
    measure_compression: Option<bool>,
    #[serde(default)]
    sample_rows: Option<u64>,
    #[serde(default)]
    max_wall_secs: Option<u64>,
    #[serde(default)]
    continue_on_error: Option<bool>,
    #[serde(default)]
    source_kind: Option<String>,
}

#[derive(Debug, Deserialize, Default, Clone)]
#[serde(deny_unknown_fields)]
struct BatchSource {
    id: String,
    kind: String,
    #[serde(default)]
    connect: Option<String>,
    #[serde(default)]
    connect_env: Option<String>,
    #[serde(default)]
    connect_file: Option<PathBuf>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    paths: Vec<String>,
    #[serde(default)]
    dataset_mode: Option<String>,
    #[serde(default)]
    logical_table: Option<String>,
    #[serde(default)]
    user: Option<String>,
    #[serde(default)]
    user_env: Option<String>,
    #[serde(default)]
    user_file: Option<PathBuf>,
    #[serde(default)]
    password_file: Option<PathBuf>,
    #[serde(default)]
    password_env: Option<String>,
    #[serde(default)]
    azure_token_file: Option<PathBuf>,
    #[serde(default)]
    azure_token_env: Option<String>,
    #[serde(default)]
    auth_mode: Option<AuthMode>,
    #[serde(default)]
    measure_compression: Option<bool>,
    #[serde(default)]
    sample_rows: Option<u64>,
    #[serde(default)]
    max_wall_secs: Option<u64>,
    #[serde(default)]
    source_kind: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    dataset_relationship: Option<String>,
    #[serde(default)]
    dataset_group: Option<String>,
    #[serde(default)]
    dataset_group_complete: Option<bool>,
}

const BATCH_OWNER_FILE: &str = ".dbwarp-blueprint-bundle-owner.toml";
const BATCH_OWNER_KIND: &str = "dbwarp-blueprint-bundle-owner";
const BATCH_OWNER_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BatchOwnerMarker {
    kind: String,
    version: u32,
    generation_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BatchPublishJournal {
    kind: String,
    version: u32,
    generation_id: String,
    staging_name: String,
    backup_name: String,
}

fn validate_batch_manifest_contract(manifest: &BatchManifest) -> Result<()> {
    if manifest.sources.is_empty() {
        bail!("DBP1103E batch manifest has no [[source]] entries. Next: add at least one [[source]] block.");
    }

    let mut raw_ids = std::collections::BTreeSet::new();
    let mut normalized_ids = std::collections::BTreeMap::new();
    for source in &manifest.sources {
        if source.id.trim() != source.id || !raw_ids.insert(source.id.clone()) {
            bail!(
                "DBP1109E batch source id '{}' is empty, padded, or duplicated. Next: give every source one unique stable id without leading/trailing whitespace.",
                source.id
            );
        }
        let normalized = sanitize_source_id(&source.id)?;
        if normalized.len() > 120 {
            bail!(
                "DBP1109E batch source id '{}' normalizes to {} bytes, above the safe filename limit of 120. Next: use a shorter stable id.",
                source.id,
                normalized.len()
            );
        }
        if let Some(previous) = normalized_ids.insert(normalized.clone(), source.id.clone()) {
            bail!(
                "DBP1109E batch source ids '{}' and '{}' both normalize to '{}'. Next: choose ids that remain distinct after lowercase ASCII filename normalization.",
                previous,
                source.id,
                normalized
            );
        }

        let kind = normalized_batch_kind(&source.kind);
        let connection_sources = source.connect.is_some() as usize
            + source.connect_env.is_some() as usize
            + source.connect_file.is_some() as usize;
        let file_specs = source.path.is_some() as usize + source.paths.len();
        match kind.as_str() {
            "postgresql" | "mysql" | "sqlserver" => {
                if connection_sources != 1 {
                    bail!(
                        "DBP1110E database source '{}' requires exactly one of connect, connect_env, or connect_file. Next: set exactly one connection source.",
                        source.id
                    );
                }
                if file_specs > 0 || source.dataset_mode.is_some() || source.logical_table.is_some()
                {
                    bail!(
                        "DBP1102E database source '{}' contains structured-file fields. Next: remove path, paths, dataset_mode, and logical_table from database sources.",
                        source.id
                    );
                }
            }
            "parquet" | "avro" => {
                if connection_sources > 0 {
                    bail!(
                        "DBP1102E structured-file source '{}' contains database connection fields. Next: remove connect, connect_env, and connect_file.",
                        source.id
                    );
                }
                if file_specs == 0 {
                    bail!(
                        "DBP1107E file source '{}' requires path or paths. Next: add at least one path relative to the batch manifest.",
                        source.id
                    );
                }
                let mode = source.dataset_mode.as_deref().unwrap_or(if file_specs == 1 {
                    "single_file"
                } else {
                    "merge_same_schema"
                });
                if !matches!(
                    mode,
                    "single_file"
                        | "one_table_per_file"
                        | "merge_same_schema"
                        | "partitioned_dataset"
                ) {
                    bail!(
                        "DBP1108E unsupported dataset_mode '{mode}'. Next: use single_file, one_table_per_file, merge_same_schema, or partitioned_dataset."
                    );
                }
                if mode == "single_file" && file_specs != 1 {
                    bail!(
                        "DBP1108E dataset_mode 'single_file' for source '{}' has {} path specifications. Next: provide exactly one path or choose a multi-file dataset mode.",
                        source.id,
                        file_specs
                    );
                }
            }
            other => bail!(
                "DBP1106E unsupported batch source kind '{other}'. Next: use postgresql, mysql, sqlserver, parquet, or avro."
            ),
        }

        let measure = source
            .measure_compression
            .or(manifest.defaults.measure_compression)
            .unwrap_or(false);
        let sample_rows = source.sample_rows.or(manifest.defaults.sample_rows);
        let max_wall_secs = source.max_wall_secs.or(manifest.defaults.max_wall_secs);
        if measure && sample_rows == Some(0) {
            bail!(
                "DBP1102E source '{}' enables compression measurement with sample_rows=0. Next: set sample_rows to a positive bounded value.",
                source.id
            );
        }
        if measure && max_wall_secs == Some(0) {
            bail!(
                "DBP1102E source '{}' enables compression measurement with max_wall_secs=0. Next: set a positive wall-time budget.",
                source.id
            );
        }
    }
    build_batch_dataset_groups(manifest)?;
    Ok(())
}

fn build_batch_dataset_groups(
    manifest: &BatchManifest,
) -> Result<std::collections::BTreeMap<String, dbwarp_blueprint_core::BundleDatasetGroup>> {
    let mut groups = std::collections::BTreeMap::new();
    for source in &manifest.sources {
        let source_id = sanitize_source_id(&source.id)?;
        let relationship = source.dataset_relationship.as_deref().unwrap_or("unknown");
        if !matches!(
            relationship,
            "independent" | "replica" | "shard" | "unknown"
        ) {
            bail!(
                "DBP1102E source '{}' has unsupported dataset_relationship '{}'. Next: use independent, replica, shard, or unknown.",
                source.id,
                relationship
            );
        }
        if matches!(relationship, "replica" | "shard") && source.dataset_group.is_none() {
            bail!(
                "DBP1102E source '{}' uses dataset_relationship='{}' without dataset_group. Next: give every member of the logical dataset the same bundle-local group id.",
                source.id,
                relationship
            );
        }
        let group_id = source
            .dataset_group
            .clone()
            .unwrap_or_else(|| format!("dataset-{source_id}"));
        validate_batch_dataset_group_id(&source.id, &group_id)?;
        let members_complete = match relationship {
            "independent" => {
                if source.dataset_group_complete == Some(false) {
                    bail!(
                        "DBP1102E independent source '{}' cannot declare dataset_group_complete=false.",
                        source.id
                    );
                }
                true
            }
            "unknown" => {
                if source.dataset_group_complete == Some(true) {
                    bail!(
                        "DBP1102E unknown source '{}' cannot declare dataset_group_complete=true.",
                        source.id
                    );
                }
                false
            }
            _ => source.dataset_group_complete.unwrap_or(false),
        };
        let group = groups.entry(group_id.clone()).or_insert_with(|| {
            dbwarp_blueprint_core::BundleDatasetGroup {
                relationship: relationship.to_string(),
                members_complete,
                members: Vec::new(),
            }
        });
        if group.relationship != relationship || group.members_complete != members_complete {
            bail!(
                "DBP1102E dataset group '{}' has inconsistent relationship or completeness declarations. Next: use the same dataset_relationship and dataset_group_complete value on every member.",
                group_id
            );
        }
        group.members.push(source_id);
    }
    for (group_id, group) in groups.iter_mut() {
        group.members.sort();
        group.members.dedup();
        if group.relationship == "independent" && group.members.len() != 1 {
            bail!(
                "DBP1102E independent dataset group '{}' has {} members. Next: give each independent source its own dataset_group.",
                group_id,
                group.members.len()
            );
        }
    }
    Ok(groups)
}

fn validate_batch_dataset_group_id(source_id: &str, group_id: &str) -> Result<()> {
    if group_id.is_empty()
        || group_id.trim() != group_id
        || group_id.len() > 120
        || !group_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        bail!(
            "DBP1102E source '{}' has unsafe dataset_group '{}'. Next: use 1-120 ASCII letters, digits, '.', '_', or '-'.",
            source_id,
            group_id
        );
    }
    Ok(())
}

fn run_batch_manifest(cli: &Cli, audit: &mut AuditLog, manifest_path: &Path) -> Result<()> {
    let out_dir = cli
        .out_dir
        .clone()
        .unwrap_or_else(|| PathBuf::from("blueprint-bundle"));
    audit.connection.uri_redacted = "(offline: --batch-manifest)".to_string();
    audit.connection.auth = "(manifest-controlled)".to_string();
    audit.connection.tls_mode = "(per source)".to_string();
    audit.connection.user_source = Some("(per source)".to_string());

    let manifest_text = std::fs::read_to_string(manifest_path).with_context(|| {
        format!(
            "DBP1101E could not read batch manifest {}. Next: check the path and file permissions.",
            manifest_path.display()
        )
    })?;
    audit.record_file_read(&manifest_path.display().to_string());
    let manifest: BatchManifest = toml::from_str(&manifest_text).with_context(|| {
        format!(
            "DBP1102E could not parse batch manifest {}",
            manifest_path.display()
        )
    })?;
    validate_batch_manifest_contract(&manifest)?;

    if cli.dry_run {
        print_batch_preflight(&manifest, manifest_path, &out_dir);
        eprintln!("{}", i18n::text("dry.batch"));
        return Ok(());
    }
    if !cli.yes {
        bail!("DBP1104E --batch-manifest requires --yes for non-interactive multi-source collection. Next: run with --dry-run first, then rerun with --yes.");
    }

    recover_batch_publication(&out_dir)?;
    let staging_dir = create_batch_staging_dir(&out_dir)?;
    let generation =
        run_batch_manifest_into(cli, audit, manifest_path, &manifest, &staging_dir, &out_dir);
    let summary = match generation {
        Ok(summary) => summary,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&staging_dir);
            return Err(error);
        }
    };
    if let Err(error) = publish_batch_staging_dir(&staging_dir, &out_dir) {
        let _ = std::fs::remove_dir_all(&staging_dir);
        return Err(error);
    }
    println!(
        "{}",
        i18n::format(
            "status.wrote_bundle",
            &[("path", out_dir.join("bundle.toml").display().to_string()),]
        )
    );
    if summary.failed_sources > 0 {
        if summary.succeeded_sources == 0 {
            bail!(
                "DBP1115E every source in the batch failed; a diagnostic partial bundle and \
                 errors.txt were published, but no usable source Blueprint was produced."
            );
        }
        bail!(
            "DBP1116E the batch published a partial bundle with {} successful source(s) and {} \
             failed source(s). Inspect errors.txt and the child audits before using the result.",
            summary.succeeded_sources,
            summary.failed_sources
        );
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BatchRunSummary {
    succeeded_sources: usize,
    failed_sources: usize,
}

fn run_batch_manifest_into(
    cli: &Cli,
    audit: &mut AuditLog,
    manifest_path: &Path,
    manifest: &BatchManifest,
    staging_dir: &Path,
    published_dir: &Path,
) -> Result<BatchRunSummary> {
    let out_dir = staging_dir;

    let blueprints_dir = out_dir.join("blueprints");
    let audits_dir = out_dir.join("audits");
    std::fs::create_dir_all(&blueprints_dir).with_context(|| {
        format!(
            "DBP1113E creating batch Blueprint directory {}",
            blueprints_dir.display()
        )
    })?;
    std::fs::create_dir_all(&audits_dir).with_context(|| {
        format!(
            "DBP1113E creating batch audit directory {}",
            audits_dir.display()
        )
    })?;

    let mut bundle = dbwarp_blueprint_core::BlueprintBundle {
        schema_version: dbwarp_blueprint_core::BUNDLE_SCHEMA_VERSION,
        kind: dbwarp_blueprint_core::BUNDLE_KIND.to_string(),
        generated_at: cli.generated_at.clone().unwrap_or_else(|| {
            chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
        }),
        dataset_groups: build_batch_dataset_groups(manifest)?,
        ..Default::default()
    };

    let continue_on_error = manifest.defaults.continue_on_error.unwrap_or(false);
    let manifest_base = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let mut errors = Vec::new();
    let mut failed_source_ids = Vec::new();
    for source in &manifest.sources {
        let id = sanitize_source_id(&source.id)?;
        let result = run_batch_source(
            cli,
            &manifest.defaults,
            source,
            manifest_base,
            &blueprints_dir,
            &audits_dir,
        );
        match result {
            Ok(bundle_source) => {
                bundle.sources.insert(id, bundle_source);
            }
            Err(err) if continue_on_error => {
                failed_source_ids.push(id.clone());
                let _ = std::fs::remove_file(blueprints_dir.join(format!("{id}.blueprint.toml")));
                let _ = std::fs::remove_file(audits_dir.join(format!("{id}.audit.txt")));
                errors.push(format!("{}: {err:#}", source.id));
            }
            Err(err) => {
                return Err(err).with_context(|| {
                    format!(
                        "DBP1105E batch source '{}' failed. Next: inspect this source entry and rerun with --dry-run.",
                        source.id
                    )
                })
            }
        }
    }

    failed_source_ids.sort();
    bundle.partial = !failed_source_ids.is_empty();
    bundle.failed_source_count = failed_source_ids.len() as u64;
    bundle.failed_sources = failed_source_ids;
    dbwarp_blueprint_core::recompute_bundle_totals(&mut bundle)
        .context("DBP1113E computing batch bundle totals")?;
    record_bundle_aggregation_warnings(&bundle, audit);
    let bundle_path = out_dir.join("bundle.toml");
    let bundle_body = dbwarp_blueprint_core::blueprint_bundle_to_toml(&bundle)
        .context("DBP1113E serializing batch bundle output")?;
    write_bytes_with_parent(&bundle_path, bundle_body.as_bytes())
        .context("DBP1113E writing batch bundle output")?;
    audit.record_file_written(
        published_dir.join("bundle.toml"),
        bundle_body.len() as u64,
        sha256_bytes(bundle_body.as_bytes()),
    );
    if !errors.is_empty() {
        let errors_path = out_dir.join("errors.txt");
        let published_errors_path = published_dir.join("errors.txt");
        let errors_body = errors.join("\n");
        atomic_write_bytes(&errors_path, errors_body.as_bytes()).with_context(|| {
            format!(
                "DBP1113E writing batch error report {}",
                errors_path.display()
            )
        })?;
        audit.record_file_written(
            published_errors_path.clone(),
            errors_body.len() as u64,
            sha256_bytes(errors_body.as_bytes()),
        );
        eprintln!(
            "{}",
            i18n::format(
                "status.wrote_source_errors",
                &[
                    ("path", published_errors_path.display().to_string()),
                    ("count", errors.len().to_string()),
                ]
            )
        );
    }
    write_batch_owner_marker(out_dir)?;
    validate_batch_output_layout(out_dir)?;
    sync_batch_artifact_tree(out_dir)?;
    Ok(BatchRunSummary {
        succeeded_sources: bundle.sources.len(),
        failed_sources: errors.len(),
    })
}

fn record_bundle_aggregation_warnings(
    bundle: &dbwarp_blueprint_core::BlueprintBundle,
    audit: &mut AuditLog,
) {
    for (limitation, key, code) in [
        (
            "unknown-dataset-relationship",
            "bundle.relationship_unknown",
            "DBP1414W",
        ),
        (
            "replica-group-disagreement",
            "bundle.replica_disagreement",
            "DBP1415W",
        ),
        (
            "shard-group-incomplete",
            "bundle.shard_incomplete",
            "DBP1416W",
        ),
        (
            "source-dataset-scope-incomplete",
            "bundle.source_scope_incomplete",
            "DBP1418W",
        ),
    ] {
        if bundle
            .bundle_totals
            .limitations
            .iter()
            .any(|value| value == limitation)
        {
            let detail = i18n::format(key, &[("code", code.to_string())]);
            eprintln!("{detail}");
            audit.record_warning(code, detail);
        }
    }
    if bundle.bundle_totals.aggregation == "suppressed" {
        let detail = i18n::format(
            "bundle.aggregate_suppressed",
            &[("code", "DBP1417W".to_string())],
        );
        eprintln!("{detail}");
        audit.record_warning("DBP1417W", detail);
    }
}

fn create_batch_staging_dir(published_dir: &Path) -> Result<PathBuf> {
    let parent = published_dir.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)
        .with_context(|| format!("DBP1113E creating batch output parent {}", parent.display()))?;
    let name = published_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("blueprint-bundle");
    let staging_dir = parent.join(format!(
        ".{name}.dbwarp-stage-{}",
        fresh_generation_id(published_dir)
    ));
    std::fs::create_dir(&staging_dir).with_context(|| {
        format!(
            "DBP1113E creating batch staging directory {}",
            staging_dir.display()
        )
    })?;
    Ok(staging_dir)
}

fn publish_batch_staging_dir(staging_dir: &Path, published_dir: &Path) -> Result<()> {
    let marker = validate_batch_output_layout(staging_dir)?;
    recover_batch_publication(published_dir)?;
    if published_dir.symlink_metadata().is_ok() && !batch_output_is_owned(published_dir)? {
        bail!(
            "DBP1113E refusing to replace nonempty output directory {} because it is not an owned dbwarp-blueprint bundle. Next: choose an empty/new --out-dir or remove the directory explicitly.",
            published_dir.display()
        );
    }

    let parent = published_dir.parent().unwrap_or_else(|| Path::new("."));
    let name = published_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("blueprint-bundle");
    let backup = parent.join(format!(".{name}.dbwarp-backup-{}", marker.generation_id));
    let journal_path = batch_publish_journal_path(published_dir);
    let journal = BatchPublishJournal {
        kind: "dbwarp-blueprint-bundle-publish".to_string(),
        version: 1,
        generation_id: marker.generation_id.clone(),
        staging_name: sibling_file_name(staging_dir, "staging directory")?,
        backup_name: sibling_file_name(&backup, "backup directory")?,
    };
    let journal_body =
        toml::to_string(&journal).context("DBP1113E serializing batch publish journal")?;
    atomic_write_bytes(&journal_path, journal_body.as_bytes()).with_context(|| {
        format!(
            "DBP1113E durably recording batch publication journal {}",
            journal_path.display()
        )
    })?;
    let had_previous = published_dir.symlink_metadata().is_ok();
    if had_previous {
        std::fs::rename(published_dir, &backup).with_context(|| {
            format!(
                "DBP1113E moving prior batch output {} aside",
                published_dir.display()
            )
        })?;
    }
    sync_directory(parent)?;
    if let Err(error) = std::fs::rename(staging_dir, published_dir) {
        let restore_error = had_previous
            .then(|| std::fs::rename(&backup, published_dir).err())
            .flatten();
        return Err(error).with_context(|| {
            format!(
                "DBP1113E publishing completed batch output {}; rollback error: {:?}. Next: inspect the sibling backup directory before retrying.",
                published_dir.display(),
                restore_error
            )
        });
    }
    sync_directory(parent)?;
    if had_previous {
        remove_owned_or_empty_batch_dir(&backup)?;
    }
    std::fs::remove_file(&journal_path).with_context(|| {
        format!(
            "DBP1113E removing completed batch publish journal {}",
            journal_path.display()
        )
    })?;
    sync_directory(parent)?;
    Ok(())
}

fn fresh_generation_id(context: &Path) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    let mut hash = Sha256::new();
    hash.update(context.as_os_str().as_encoded_bytes());
    hash.update(std::process::id().to_le_bytes());
    hash.update(
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .to_le_bytes(),
    );
    hash.update(NEXT_ID.fetch_add(1, Ordering::Relaxed).to_le_bytes());
    hex::encode(&hash.finalize()[..16])
}

fn batch_publish_journal_path(published_dir: &Path) -> PathBuf {
    let name = published_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("blueprint-bundle");
    published_dir.with_file_name(format!(".{name}.dbwarp-publish-journal.toml"))
}

fn sibling_file_name(path: &Path, label: &str) -> Result<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty() && !name.contains(['/', '\\']))
        .map(str::to_string)
        .ok_or_else(|| anyhow!("DBP1113E invalid {label} name {}", path.display()))
}

fn write_batch_owner_marker(path: &Path) -> Result<BatchOwnerMarker> {
    let marker = BatchOwnerMarker {
        kind: BATCH_OWNER_KIND.to_string(),
        version: BATCH_OWNER_VERSION,
        generation_id: fresh_generation_id(path),
    };
    let body = toml::to_string(&marker).context("DBP1113E serializing batch ownership marker")?;
    atomic_write_bytes(&path.join(BATCH_OWNER_FILE), body.as_bytes()).with_context(|| {
        format!(
            "DBP1113E writing batch ownership marker under {}",
            path.display()
        )
    })?;
    Ok(marker)
}

fn read_batch_owner_marker(path: &Path) -> Result<BatchOwnerMarker> {
    let marker_path = path.join(BATCH_OWNER_FILE);
    let metadata = std::fs::symlink_metadata(&marker_path).with_context(|| {
        format!(
            "DBP1113E reading batch ownership marker {}",
            marker_path.display()
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        bail!(
            "DBP1113E batch ownership marker {} is not a regular file",
            marker_path.display()
        );
    }
    let text = std::fs::read_to_string(&marker_path).with_context(|| {
        format!(
            "DBP1113E reading batch ownership marker {}",
            marker_path.display()
        )
    })?;
    let marker: BatchOwnerMarker = toml::from_str(&text).with_context(|| {
        format!(
            "DBP1113E parsing batch ownership marker {}",
            marker_path.display()
        )
    })?;
    if marker.kind != BATCH_OWNER_KIND
        || marker.version != BATCH_OWNER_VERSION
        || marker.generation_id.len() != 32
        || !marker
            .generation_id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        bail!(
            "DBP1113E invalid batch ownership marker {}",
            marker_path.display()
        );
    }
    Ok(marker)
}

fn validate_batch_output_layout(path: &Path) -> Result<BatchOwnerMarker> {
    let marker = read_batch_owner_marker(path)?;
    let bundle_path = path.join("bundle.toml");
    let bundle =
        dbwarp_blueprint_core::read_blueprint_bundle_toml(&bundle_path).with_context(|| {
            format!(
                "DBP1113E validating completed batch bundle {}",
                bundle_path.display()
            )
        })?;
    if bundle.kind != dbwarp_blueprint_core::BUNDLE_KIND {
        bail!(
            "DBP1113E completed batch output {} has unexpected bundle kind '{}'",
            path.display(),
            bundle.kind
        );
    }
    for (source_id, source) in &bundle.sources {
        if let Some(blueprint_path) = &source.blueprint_path {
            let resolved = dbwarp_blueprint_core::resolve_bundle_path_checked(path, blueprint_path)
                .with_context(|| {
                    format!("DBP1113E validating Blueprint path for source '{source_id}'")
                })?;
            if !resolved.is_file() {
                bail!(
                    "DBP1113E batch source '{}' references missing Blueprint {}",
                    source_id,
                    resolved.display()
                );
            }
            dbwarp_blueprint_core::read_blueprint_toml(&resolved).with_context(|| {
                format!("DBP1113E validating Blueprint for source '{source_id}'")
            })?;
        }
        if let Some(audit_path) = &source.audit_path {
            let resolved = dbwarp_blueprint_core::resolve_bundle_path_checked(path, audit_path)
                .with_context(|| {
                    format!("DBP1113E validating audit path for source '{source_id}'")
                })?;
            if !resolved.is_file() {
                bail!(
                    "DBP1113E batch source '{}' references missing audit {}",
                    source_id,
                    resolved.display()
                );
            }
        }
    }
    Ok(marker)
}

fn remove_owned_or_empty_batch_dir(path: &Path) -> Result<()> {
    let mut entries = std::fs::read_dir(path).with_context(|| {
        format!(
            "DBP1113E inspecting retired batch directory {}",
            path.display()
        )
    })?;
    let empty = entries.next().is_none();
    if !empty {
        validate_batch_output_layout(path)?;
    }
    std::fs::remove_dir_all(path).with_context(|| {
        format!(
            "DBP1113E removing retired batch directory {}",
            path.display()
        )
    })
}

fn recover_batch_publication(published_dir: &Path) -> Result<()> {
    let journal_path = batch_publish_journal_path(published_dir);
    let metadata = match std::fs::symlink_metadata(&journal_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "DBP1113E inspecting batch publish journal {}",
                    journal_path.display()
                )
            });
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        bail!(
            "DBP1113E batch publish journal {} is not a regular file",
            journal_path.display()
        );
    }
    let journal_text = std::fs::read_to_string(&journal_path).with_context(|| {
        format!(
            "DBP1113E reading batch publish journal {}",
            journal_path.display()
        )
    })?;
    let journal: BatchPublishJournal = toml::from_str(&journal_text).with_context(|| {
        format!(
            "DBP1113E parsing batch publish journal {}",
            journal_path.display()
        )
    })?;
    if journal.kind != "dbwarp-blueprint-bundle-publish"
        || journal.version != 1
        || journal.generation_id.len() != 32
        || !journal
            .generation_id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        bail!(
            "DBP1113E invalid batch publish journal {}",
            journal_path.display()
        );
    }
    let parent = published_dir.parent().unwrap_or_else(|| Path::new("."));
    let published_name = published_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("blueprint-bundle");
    let staging_prefix = format!(".{published_name}.dbwarp-stage-");
    let staging_suffix = journal
        .staging_name
        .strip_prefix(&staging_prefix)
        .filter(|suffix| suffix.len() == 32 && suffix.bytes().all(|byte| byte.is_ascii_hexdigit()));
    let expected_backup = format!(".{published_name}.dbwarp-backup-{}", journal.generation_id);
    if Path::new(&journal.staging_name).components().count() != 1
        || staging_suffix.is_none()
        || Path::new(&journal.backup_name).components().count() != 1
        || journal.backup_name != expected_backup
    {
        bail!(
            "DBP1113E invalid sibling names in batch publish journal {}",
            journal_path.display()
        );
    }
    let staging = parent.join(&journal.staging_name);
    let backup = parent.join(&journal.backup_name);
    if staging.symlink_metadata().is_ok() {
        let marker = validate_batch_output_layout(&staging)?;
        if marker.generation_id != journal.generation_id {
            bail!(
                "DBP1113E interrupted staging generation does not match journal {}",
                journal_path.display()
            );
        }
    }
    if published_dir.symlink_metadata().is_ok() {
        if batch_output_is_owned(published_dir)? {
            if backup.symlink_metadata().is_ok() {
                remove_owned_or_empty_batch_dir(&backup)?;
            }
            if staging.symlink_metadata().is_ok() {
                remove_owned_or_empty_batch_dir(&staging)?;
            }
        } else {
            bail!(
                "DBP1113E interrupted publication found an unowned object at {}. Next: preserve the journal and inspect paths before retrying.",
                published_dir.display()
            );
        }
    } else if backup.symlink_metadata().is_ok() {
        if !batch_output_is_owned(&backup)? {
            bail!(
                "DBP1113E interrupted publication backup {} is not an owned bundle",
                backup.display()
            );
        }
        std::fs::rename(&backup, published_dir).with_context(|| {
            format!(
                "DBP1113E restoring interrupted batch publication {}",
                published_dir.display()
            )
        })?;
        if staging.symlink_metadata().is_ok() {
            remove_owned_or_empty_batch_dir(&staging)?;
        }
    } else if staging.symlink_metadata().is_ok() {
        std::fs::rename(&staging, published_dir).with_context(|| {
            format!(
                "DBP1113E completing interrupted batch publication {}",
                published_dir.display()
            )
        })?;
    } else {
        bail!(
            "DBP1113E batch publish journal {} has no recoverable published, backup, or staging directory",
            journal_path.display()
        );
    }
    std::fs::remove_file(&journal_path).with_context(|| {
        format!(
            "DBP1113E removing recovered batch publish journal {}",
            journal_path.display()
        )
    })?;
    sync_directory(parent)
}

fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        std::fs::File::open(path)
            .and_then(|directory| directory.sync_all())
            .with_context(|| format!("DBP1113E durably syncing directory {}", path.display()))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn sync_batch_artifact_tree(path: &Path) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path).with_context(|| {
        format!(
            "DBP1113E inspecting staged batch artifact {}",
            path.display()
        )
    })?;
    if metadata.file_type().is_symlink() {
        bail!(
            "DBP1113E staged batch artifact {} contains a symbolic link. Next: remove the unexpected link and regenerate the bundle.",
            path.display()
        );
    }
    if metadata.is_file() {
        std::fs::File::open(path)
            .and_then(|file| file.sync_all())
            .with_context(|| {
                format!(
                    "DBP1113E durably syncing staged batch artifact {}",
                    path.display()
                )
            })?;
        return Ok(());
    }
    if !metadata.is_dir() {
        bail!(
            "DBP1113E staged batch artifact {} is neither a regular file nor a directory",
            path.display()
        );
    }
    for entry in std::fs::read_dir(path).with_context(|| {
        format!(
            "DBP1113E enumerating staged batch directory {}",
            path.display()
        )
    })? {
        let entry = entry.with_context(|| {
            format!(
                "DBP1113E reading an entry under staged batch directory {}",
                path.display()
            )
        })?;
        sync_batch_artifact_tree(&entry.path())?;
    }
    #[cfg(unix)]
    std::fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .with_context(|| {
            format!(
                "DBP1113E durably syncing staged batch directory {}",
                path.display()
            )
        })?;
    Ok(())
}

fn batch_output_is_owned(path: &Path) -> Result<bool> {
    let metadata = match path.symlink_metadata() {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(true),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("DBP1113E inspecting batch output {}", path.display()));
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Ok(false);
    }
    let mut entries = std::fs::read_dir(path)
        .with_context(|| format!("DBP1113E reading batch output {}", path.display()))?;
    if entries.next().is_none() {
        return Ok(true);
    }
    Ok(validate_batch_output_layout(path).is_ok())
}

fn run_batch_source(
    parent: &Cli,
    defaults: &BatchDefaults,
    source: &BatchSource,
    manifest_base: &Path,
    blueprints_dir: &Path,
    audits_dir: &Path,
) -> Result<dbwarp_blueprint_core::BundleSource> {
    let id = sanitize_source_id(&source.id)?;
    let kind = normalized_batch_kind(&source.kind);
    let blueprint_path = blueprints_dir.join(format!("{id}.blueprint.toml"));
    let audit_path = audits_dir.join(format!("{id}.audit.txt"));
    match kind.as_str() {
        "postgresql" | "postgres" | "mysql" | "mariadb" | "sqlserver" | "mssql" | "tds" => {
            let connect = resolve_batch_connect(source, manifest_base)?;
            let mut source_cli = batch_child_cli(
                parent,
                defaults,
                source,
                blueprint_path.clone(),
                audit_path.clone(),
            );
            source_cli.connect = Some(connect);
            let mut source_audit = AuditLog::new(
                if source_cli.measure_compression {
                    "tier-2"
                } else {
                    "tier-1"
                },
                unix_ms(SystemTime::now()),
            );
            source_audit.generated_at_pin = source_cli.generated_at.clone();
            let result = run_with_audit(&source_cli, &mut source_audit);
            if let Err(err) = &result {
                source_audit.mark_failure(format!("{err}"));
            }
            source_audit.finalize(unix_ms(SystemTime::now()));
            let rendered = source_audit.render();
            atomic_write_bytes(&audit_path, rendered.as_bytes()).with_context(|| {
                format!("DBP1113E writing source audit {}", audit_path.display())
            })?;
            result?;
            let blueprint = dbwarp_blueprint_core::read_blueprint_toml(&blueprint_path).with_context(|| {
                format!("DBP1113E reading generated batch Blueprint {}", blueprint_path.display())
            })?;
            Ok(bundle_source_from_blueprint(
                source,
                &blueprint,
                "database",
                relative_bundle_path("blueprints", &blueprint_path),
                Some(relative_bundle_path("audits", &audit_path)),
            )?)
        }
        "parquet" | "avro" => {
            let started_unix_ms = unix_ms(SystemTime::now());
            let blueprint = batch_structured_file_blueprint(defaults, source, manifest_base, &kind)?;
            let body = dbwarp_blueprint_core::blueprint_to_toml(&blueprint)
                .context("DBP1113E serializing batch Blueprint output")?;
            write_bytes_with_parent(&blueprint_path, body.as_bytes())
                .context("DBP1113E writing batch Blueprint output")?;
            let mut source_audit = AuditLog::new("blueprint-batch-file", started_unix_ms);
            source_audit.connection.uri_redacted = format!("(offline: {kind})");
            source_audit.connection.auth = "(not used)".to_string();
            source_audit.connection.tls_mode = "(not used)".to_string();
            source_audit.connection.user_source = Some("(not used)".to_string());
            source_audit.record_fidelity(dbwarp_blueprint_core::estimate_blueprint_fidelity(
                &blueprint,
            ));
            for path in expand_batch_paths(source, manifest_base)? {
                source_audit.record_file_read(&path.display().to_string());
            }
            source_audit.record_file_written(
                blueprint_path.clone(),
                body.len() as u64,
                sha256_bytes(body.as_bytes()),
            );
            source_audit.finalize(unix_ms(SystemTime::now()));
            let audit_body = source_audit.render();
            write_bytes_with_parent(&audit_path, audit_body.as_bytes())
                .context("DBP1113E writing batch source audit")?;
            Ok(bundle_source_from_blueprint(
                source,
                &blueprint,
                if kind == "parquet" {
                    "parquet_dataset"
                } else {
                    "avro_dataset"
                },
                relative_bundle_path("blueprints", &blueprint_path),
                Some(relative_bundle_path("audits", &audit_path)),
            )?)
        }
        other => bail!(
            "DBP1106E unsupported batch source kind '{other}'. Next: use postgresql, mysql, sqlserver, parquet, or avro."
        ),
    }
}

fn batch_child_cli(
    parent: &Cli,
    defaults: &BatchDefaults,
    source: &BatchSource,
    out: PathBuf,
    audit_log: PathBuf,
) -> Cli {
    Cli {
        lang: parent.lang.clone(),
        color: parent.color,
        banner: false,
        banner_mode: CliBannerMode::Auto,
        connect: None,
        schema: Vec::new(),
        out,
        deck: None,
        deck_confidentiality: None,
        from_toml: None,
        from_parquet: None,
        from_avro: None,
        batch_manifest: None,
        out_dir: None,
        bundle_list: None,
        bundle_extract: None,
        bundle_pack: None,
        select: Vec::new(),
        source_kind: source
            .source_kind
            .clone()
            .or_else(|| defaults.source_kind.clone())
            .unwrap_or_else(|| parent.source_kind.clone()),
        measure_compression: source
            .measure_compression
            .or(defaults.measure_compression)
            .unwrap_or(parent.measure_compression),
        artifact_detail: parent.artifact_detail,
        length_fidelity: parent.length_fidelity,
        preserve_exact_lengths: parent.preserve_exact_lengths,
        yes: true,
        sample_rows: source
            .sample_rows
            .or(defaults.sample_rows)
            .unwrap_or(parent.sample_rows),
        compression_workers: parent.compression_workers,
        max_wall_secs: source
            .max_wall_secs
            .or(defaults.max_wall_secs)
            .unwrap_or(parent.max_wall_secs),
        no_rtt_probe: parent.no_rtt_probe,
        user: source.user.clone(),
        user_env: source.user_env.clone(),
        user_file: source.user_file.clone(),
        password_file: source.password_file.clone(),
        password_env: source.password_env.clone(),
        anonymization_key_file: parent.anonymization_key_file.clone(),
        azure_token_file: source.azure_token_file.clone(),
        azure_token_env: source.azure_token_env.clone(),
        auth_mode: source.auth_mode,
        expect_server_principal: None,
        audit_log: Some(audit_log),
        generated_at: parent.generated_at.clone(),
        dry_run: false,
        tls_mode: parent.tls_mode.clone(),
        tls_ca: parent.tls_ca.clone(),
        tls_cert: parent.tls_cert.clone(),
        tls_key: parent.tls_key.clone(),
        tls_server_name: parent.tls_server_name.clone(),
        tls_skip_verify: parent.tls_skip_verify,
        i_know_what_im_doing: parent.i_know_what_im_doing,
    }
}

fn batch_structured_file_blueprint(
    defaults: &BatchDefaults,
    source: &BatchSource,
    manifest_base: &Path,
    kind: &str,
) -> Result<dbwarp_blueprint_core::BlueprintFile> {
    let paths = expand_batch_paths(source, manifest_base)?;
    if paths.is_empty() {
        bail!(
            "DBP1107E source '{}' resolved no input files. Next: check path, paths, or glob entries relative to the manifest.",
            source.id
        );
    }
    let measure = source
        .measure_compression
        .or(defaults.measure_compression)
        .unwrap_or(false);
    let sample_rows = source.sample_rows.or(defaults.sample_rows).unwrap_or(1000);
    let max_wall_secs = source
        .max_wall_secs
        .or(defaults.max_wall_secs)
        .unwrap_or(300)
        .max(1);
    let options = if measure {
        dbwarp_blueprint_core::DecodedCompressionOptions::enabled(
            sample_rows,
            format!("{kind} batch decoded first {sample_rows} rows; rowframe-v1 zstd"),
            format!("{kind} batch decoded first {sample_rows} rows per column; rowframe-v1 zstd"),
        )
        .with_limits(
            dbwarp_blueprint_core::DEFAULT_MAX_SAMPLE_BYTES,
            std::time::Duration::from_secs(max_wall_secs),
        )
    } else {
        dbwarp_blueprint_core::DecodedCompressionOptions::disabled()
    };
    // One caller-owned deadline covers every file and every metadata,
    // sampling, and compression phase in this logical batch operation.
    let deadline = options.deadline();
    let mut blueprints = Vec::new();
    for path in &paths {
        let blueprint = match kind {
            "parquet" => {
                dbwarp_blueprint_core::parquet::parquet_blueprint_from_path_with_options_and_deadline(
                    path, &options, &deadline,
                )?
            }
            "avro" => dbwarp_blueprint_core::avro::avro_blueprint_from_path_with_options_and_deadline(
                path, &options, &deadline,
            )?,
            _ => unreachable!(),
        };
        blueprints.push((path.clone(), blueprint));
    }
    let mode = source
        .dataset_mode
        .as_deref()
        .unwrap_or(if paths.len() == 1 {
            "single_file"
        } else {
            "merge_same_schema"
        });
    let mut merged = match mode {
        "single_file" => {
            if blueprints.len() != 1 {
                bail!(
                    "DBP1108E dataset_mode 'single_file' for source '{}' resolved {} files. Next: provide exactly one file or use one_table_per_file/merge_same_schema.",
                    source.id,
                    blueprints.len()
                );
            }
            blueprints
                .into_iter()
                .next()
                .map(|(_, blueprint)| blueprint)
            .context("DBP1114E single_file source has no Blueprint")?
        }
        "one_table_per_file" => blueprint_one_table_per_file(kind, blueprints)?,
        "merge_same_schema" | "partitioned_dataset" => {
            blueprint_merge_same_schema(kind, source.logical_table.as_deref(), blueprints)?
        }
        other => bail!(
            "DBP1108E unsupported dataset_mode '{other}'. Next: use single_file, one_table_per_file, merge_same_schema, or partitioned_dataset."
        ),
    };
    merged.generated_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    if let Some(source_kind) = source
        .source_kind
        .as_ref()
        .or(defaults.source_kind.as_ref())
    {
        merged.source_kind = source_kind.clone();
    }
    Ok(merged)
}

fn blueprint_one_table_per_file(
    kind: &str,
    blueprints: Vec<(PathBuf, dbwarp_blueprint_core::BlueprintFile)>,
) -> Result<dbwarp_blueprint_core::BlueprintFile> {
    let mut out = dbwarp_blueprint_core::BlueprintFile {
        schema_version: dbwarp_blueprint_core::SCHEMA_VERSION,
        engine: kind.to_string(),
        source_kind: kind.to_string(),
        dataset_scope: Some(dbwarp_blueprint_core::DatasetScope::structured_dataset(
            "structured-dataset-aggregate",
            "structured-dataset-aggregate",
        )),
        ..Default::default()
    };
    for (idx, (path, blueprint)) in blueprints.into_iter().enumerate() {
        let table_name = format!("table-{:03}", idx + 1);
        let table = blueprint
            .tables
            .into_values()
            .next()
            .with_context(|| format!("DBP1114E {} had no table definition", path.display()))?;
        out.tables.insert(table_name, table);
    }
    recompute_blueprint_totals(&mut out)?;
    Ok(out)
}

fn blueprint_merge_same_schema(
    kind: &str,
    _logical_table: Option<&str>,
    blueprints: Vec<(PathBuf, dbwarp_blueprint_core::BlueprintFile)>,
) -> Result<dbwarp_blueprint_core::BlueprintFile> {
    let mut iter = blueprints.into_iter();
    let (first_path, mut out) = iter
        .next()
        .context("DBP1114E merge source has no Blueprints")?;
    let table_key =
        out.tables.keys().next().cloned().with_context(|| {
            format!("DBP1114E {} had no table definition", first_path.display())
        })?;
    // `logical_table` is useful manifest intent, but the emitted Blueprint follows
    // the same anonymized table-NNN contract as live database capture.
    let final_table_key = "table-001".to_string();
    if final_table_key != table_key {
        if let Some(table) = out.tables.remove(&table_key) {
            out.tables.insert(final_table_key.clone(), table);
        }
    }
    for (path, blueprint) in iter {
        let table = blueprint
            .tables
            .into_values()
            .next()
            .with_context(|| format!("DBP1114E {} had no table definition", path.display()))?;
        let base = out
            .tables
            .get_mut(&final_table_key)
            .context("DBP1114E merged table disappeared")?;
        if !structured_tables_compatible(base, &table) {
            bail!(
                "DBP1114E cannot merge {}; structured column layout differs from first file",
                path.display()
            );
        }
        merge_structured_table_observations(base, table)?;
    }
    out.engine = kind.to_string();
    out.source_kind = kind.to_string();
    out.dataset_scope = Some(dbwarp_blueprint_core::DatasetScope::structured_dataset(
        "structured-dataset-aggregate",
        "structured-dataset-aggregate",
    ));
    recompute_blueprint_totals(&mut out)?;
    Ok(out)
}

fn structured_tables_compatible(
    left: &dbwarp_blueprint_core::BlueprintTable,
    right: &dbwarp_blueprint_core::BlueprintTable,
) -> bool {
    if left.cols.len() != right.cols.len() {
        return false;
    }
    left.cols.iter().all(|(name, left_col)| {
        right.cols.get(name).is_some_and(|right_col| {
            left_col.ordinal == right_col.ordinal
                && left_col.column_type == right_col.column_type
                && left_col.nullable == right_col.nullable
                && left_col.native_type == right_col.native_type
                && left_col.declared_max_chars == right_col.declared_max_chars
                && left_col.declared_max_bytes == right_col.declared_max_bytes
                && left_col.numeric_precision == right_col.numeric_precision
                && left_col.numeric_scale == right_col.numeric_scale
                && left_col.numeric_unsigned == right_col.numeric_unsigned
                && left_col.bit_width == right_col.bit_width
                && left_col.datetime_precision == right_col.datetime_precision
                && left_col.charset == right_col.charset
                && left_col.collation == right_col.collation
                && left_col.source_semantics == right_col.source_semantics
                && left_col.style == right_col.style
        })
    })
}

fn merge_structured_table_observations(
    base: &mut dbwarp_blueprint_core::BlueprintTable,
    incoming: dbwarp_blueprint_core::BlueprintTable,
) -> Result<()> {
    let base_table_bytes = base.table_bytes;
    let incoming_table_bytes = incoming.table_bytes;
    base.rows = checked_dataset_add(base.rows, incoming.rows, "row count")?;
    base.table_bytes = checked_dataset_add(
        base.table_bytes,
        incoming.table_bytes,
        "logical table bytes",
    )?;
    base.storage_bytes = checked_dataset_add(
        base.storage_bytes,
        incoming.storage_bytes,
        "structured-file storage bytes",
    )?;
    base.index_bytes = checked_dataset_add(base.index_bytes, incoming.index_bytes, "index bytes")?;
    base.source_partitions = checked_dataset_add(
        base.source_partitions,
        incoming.source_partitions,
        "source partition count",
    )?;
    base.row_group_count = checked_dataset_add(
        base.row_group_count,
        incoming.row_group_count,
        "row-group/block count",
    )?;
    base.source_codec = merged_codec_set(&base.source_codec, &incoming.source_codec);

    merge_compression_observations(
        &mut base.compression,
        incoming.compression,
        base_table_bytes,
        incoming_table_bytes,
    )?;
    if let Some(compression) = base.compression.as_mut() {
        compression.ratio_storage = compression_ratio(base.table_bytes, base.storage_bytes);
    }

    for (name, incoming_col) in incoming.cols {
        let Some(base_col) = base.cols.get_mut(&name) else {
            continue;
        };
        let base_samples = base_col.length_sample_rows;
        let incoming_samples = incoming_col.length_sample_rows;
        let base_non_null = observed_non_null_samples(base_samples, base_col.null_fraction);
        let incoming_non_null =
            observed_non_null_samples(incoming_samples, incoming_col.null_fraction);
        base_col.len_avg = weighted_u64(
            base_col.len_avg,
            base_non_null,
            incoming_col.len_avg,
            incoming_non_null,
        );
        base_col.len_p95 = base_col.len_p95.max(incoming_col.len_p95);
        base_col.length_p95_sample_rows = checked_dataset_add(
            base_col.length_p95_sample_rows,
            incoming_col.length_p95_sample_rows,
            "column p95 sample rows",
        )?;
        base_col.null_fraction = merged_null_fraction(
            base_col.null_fraction,
            base_samples,
            incoming_col.null_fraction,
            incoming_samples,
        );
        base_col.length_sample_rows =
            checked_dataset_add(base_samples, incoming_samples, "column length sample rows")?;
        base_col.length_sample_method =
            "structured-file-dataset-weighted-average-max-shard-p95".to_string();
        let base_compression_bytes = base_col
            .compression
            .as_ref()
            .map(|sample| sample.sample_bytes)
            .unwrap_or_default();
        merge_compression_observations(
            &mut base_col.compression,
            incoming_col.compression,
            base_compression_bytes,
            0,
        )?;
        merge_cardinality_observations(
            &mut base_col.cardinality,
            incoming_col.cardinality,
            base.rows,
        )?;
    }
    Ok(())
}

fn merge_cardinality_observations(
    base: &mut Option<dbwarp_blueprint_core::BlueprintCardinality>,
    incoming: Option<dbwarp_blueprint_core::BlueprintCardinality>,
    merged_table_rows: u64,
) -> Result<()> {
    let Some(incoming) = incoming else {
        return Ok(());
    };
    let Some(current) = base.as_mut() else {
        *base = Some(incoming);
        return Ok(());
    };

    let current_non_null = current.non_null_rows;
    let incoming_non_null = incoming.non_null_rows;
    current.sample_rows = checked_dataset_add(
        current.sample_rows,
        incoming.sample_rows,
        "column cardinality sample rows",
    )?;
    current.non_null_rows = checked_dataset_add(
        current.non_null_rows,
        incoming.non_null_rows,
        "column cardinality non-null rows",
    )?;

    // Fingerprints are intentionally not retained, so distinct values cannot
    // be matched across dataset members. Preserve an observed lower bound and
    // a conservative estimated upper bound, and disclose that limitation.
    current.observed_distinct_count = current
        .observed_distinct_count
        .max(incoming.observed_distinct_count)
        .min(current.non_null_rows);
    current.estimated_distinct_count = checked_dataset_add(
        current.estimated_distinct_count,
        incoming.estimated_distinct_count,
        "column estimated distinct count",
    )?
    .min(merged_table_rows)
    .min(current.non_null_rows.max(current.observed_distinct_count));
    current.top_value_fraction = current
        .top_value_fraction
        .max(incoming.top_value_fraction)
        .clamp(0.0, 1.0);
    current.frequency_p50 = current.frequency_p50.max(incoming.frequency_p50);
    current.frequency_p95 = current.frequency_p95.max(incoming.frequency_p95);
    current.frequency_p99 = current.frequency_p99.max(incoming.frequency_p99);
    current.frequency_max = current.frequency_max.max(incoming.frequency_max);
    current.measured |= incoming.measured;
    current.sample_method = "structured-file-dataset-cardinality-bounds-v1".to_string();
    current.sampled_with_bias = true;
    current.bias_reason = format!(
        "cross-member value overlap is unavailable because samples retain no values or per-value hashes; member non-null weights were {} and {}",
        current_non_null, incoming_non_null
    );
    Ok(())
}

fn merge_compression_observations(
    base: &mut Option<dbwarp_blueprint_core::BlueprintCompression>,
    incoming: Option<dbwarp_blueprint_core::BlueprintCompression>,
    base_logical_bytes: u64,
    incoming_logical_bytes: u64,
) -> Result<()> {
    let Some(incoming) = incoming else {
        return Ok(());
    };
    let Some(current) = base.as_mut() else {
        *base = Some(incoming);
        return Ok(());
    };

    let base_weight = if current.sample_bytes > 0 {
        current.sample_bytes
    } else {
        base_logical_bytes
    };
    let incoming_weight = if incoming.sample_bytes > 0 {
        incoming.sample_bytes
    } else {
        incoming_logical_bytes
    };
    let same_encoding = current.sample_encoding == incoming.sample_encoding;
    current.measured |= incoming.measured;
    current.sample_rows = checked_dataset_add(
        current.sample_rows,
        incoming.sample_rows,
        "compression sample rows",
    )?;
    current.sample_bytes = checked_dataset_add(
        current.sample_bytes,
        incoming.sample_bytes,
        "compression sample bytes",
    )?;
    current.sampled_with_bias |= incoming.sampled_with_bias;
    if current.sample_method != incoming.sample_method {
        current.sample_method = "structured-file-dataset-aggregate".to_string();
    }
    if current.bias_reason != incoming.bias_reason {
        current.bias_reason = if current.sampled_with_bias {
            "one or more dataset members reported sampling bias".to_string()
        } else {
            String::new()
        };
    }
    if same_encoding {
        current.ratio_zstd_3 = weighted_ratio(
            current.ratio_zstd_3,
            base_weight,
            incoming.ratio_zstd_3,
            incoming_weight,
        );
        current.ratio_stddev = current.ratio_stddev.max(incoming.ratio_stddev);
        current.ratio_storage = weighted_ratio(
            current.ratio_storage,
            base_weight,
            incoming.ratio_storage,
            incoming_weight,
        );
    } else {
        current.sample_encoding = "mixed-structured-file-provenance".to_string();
        current.ratio_zstd_3 = 0.0;
        current.ratio_zstd_19 = 0.0;
        current.ratio_stddev = 0.0;
        current.ratio_storage = 0.0;
    }
    Ok(())
}

fn checked_dataset_add(left: u64, right: u64, field: &str) -> Result<u64> {
    left.checked_add(right).with_context(|| {
        format!(
            "DBP1114E structured-file dataset {field} overflow while merging members. Next: split the batch into smaller datasets or inspect the source metadata."
        )
    })
}

fn merged_codec_set(left: &str, right: &str) -> String {
    let mut codecs = left
        .split(',')
        .chain(right.split(','))
        .map(str::trim)
        .filter(|codec| !codec.is_empty())
        .collect::<Vec<_>>();
    codecs.sort_unstable();
    codecs.dedup();
    codecs.join(",")
}

fn observed_non_null_samples(rows: u64, null_fraction: Option<f64>) -> u64 {
    match null_fraction {
        Some(fraction) => ((rows as f64) * (1.0 - fraction.clamp(0.0, 1.0))).round() as u64,
        None => rows,
    }
}

fn merged_null_fraction(
    left: Option<f64>,
    left_rows: u64,
    right: Option<f64>,
    right_rows: u64,
) -> Option<f64> {
    match (left, right) {
        (Some(left), Some(right)) if (left_rows as u128 + right_rows as u128) > 0 => Some(
            ((left * left_rows as f64) + (right * right_rows as f64))
                / (left_rows as u128 + right_rows as u128) as f64,
        ),
        (Some(left), Some(_)) => Some(left),
        _ => None,
    }
}

fn weighted_u64(left: u64, left_weight: u64, right: u64, right_weight: u64) -> u64 {
    let total_weight = left_weight as u128 + right_weight as u128;
    if total_weight == 0 {
        return left.max(right);
    }
    (((left as u128 * left_weight as u128) + (right as u128 * right_weight as u128)) / total_weight)
        .min(u64::MAX as u128) as u64
}

fn weighted_ratio(left: f64, left_weight: u64, right: f64, right_weight: u64) -> f64 {
    if left <= 0.0 {
        return right;
    }
    if right <= 0.0 {
        return left;
    }
    let compressed = left_weight as f64 / left + right_weight as f64 / right;
    if compressed <= 0.0 {
        0.0
    } else {
        left_weight.saturating_add(right_weight) as f64 / compressed
    }
}

fn compression_ratio(logical: u64, stored: u64) -> f64 {
    if logical == 0 || stored == 0 {
        0.0
    } else {
        logical as f64 / stored as f64
    }
}

fn expand_batch_paths(source: &BatchSource, manifest_base: &Path) -> Result<Vec<PathBuf>> {
    let mut specs = source.paths.clone();
    if let Some(path) = &source.path {
        specs.push(path.clone());
    }
    if specs.is_empty() {
        bail!(
            "DBP1107E file source '{}' requires path or paths",
            source.id
        );
    }
    let mut out = Vec::new();
    for spec in specs {
        let spec_path = resolve_batch_input_path(manifest_base, &spec);
        let pattern = spec_path.to_string_lossy().to_string();
        let mut matched = false;
        for entry in glob::glob(&pattern)
            .with_context(|| format!("DBP1107E invalid glob pattern {pattern}"))?
        {
            let path =
                entry.with_context(|| format!("DBP1107E reading glob entry for {pattern}"))?;
            matched = true;
            out.push(path);
        }
        if !matched && !contains_glob_meta(&pattern) {
            out.push(spec_path);
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn contains_glob_meta(value: &str) -> bool {
    value.contains('*') || value.contains('?') || value.contains('[')
}

// Batch manifests are trusted local operator configuration and intentionally
// support absolute data paths and globs. Exported bundle child references use
// the confined shared-core resolver instead.
fn resolve_batch_input_path(manifest_base: &Path, value: impl AsRef<Path>) -> PathBuf {
    let path = value.as_ref();
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        manifest_base.join(path)
    }
}

fn bundle_source_from_blueprint(
    source: &BatchSource,
    blueprint: &dbwarp_blueprint_core::BlueprintFile,
    kind: &str,
    blueprint_path: String,
    audit_path: Option<String>,
) -> Result<dbwarp_blueprint_core::BundleSource> {
    let source_id = sanitize_source_id(&source.id)?;
    let dataset_relationship = source
        .dataset_relationship
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let dataset_group = source
        .dataset_group
        .clone()
        .unwrap_or_else(|| format!("dataset-{source_id}"));
    Ok(dbwarp_blueprint_core::BundleSource {
        kind: kind.to_string(),
        engine: blueprint.engine.clone(),
        engine_version: blueprint.engine_version.clone(),
        source_kind: blueprint.source_kind.clone(),
        blueprint_path: Some(blueprint_path),
        audit_path,
        tags: source.tags.clone(),
        dataset_relationship,
        dataset_group,
        dataset_scope_completeness: dbwarp_blueprint_core::blueprint_dataset_scope_completeness(
            blueprint,
        )
        .to_string(),
        table_count: u64::try_from(
            blueprint
                .tables
                .values()
                .filter(|table| table.counts_toward_totals())
                .count(),
        )
        .context("DBP1114E Blueprint table count exceeds the supported u64 range")?,
        row_count: if blueprint.totals.row_count > 0 {
            blueprint.totals.row_count
        } else {
            checked_blueprint_total(blueprint, "row count", |table| table.rows)?
        },
        table_bytes: if blueprint.totals.table_bytes > 0 {
            blueprint.totals.table_bytes
        } else {
            checked_blueprint_total(blueprint, "logical table bytes", |table| table.table_bytes)?
        },
        index_bytes: if blueprint.totals.index_bytes > 0 {
            blueprint.totals.index_bytes
        } else {
            checked_blueprint_total(blueprint, "index bytes", |table| table.index_bytes)?
        },
        blueprint: None,
    })
}
