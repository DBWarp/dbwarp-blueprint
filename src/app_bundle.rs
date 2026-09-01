fn run_bundle_list(cli: &Cli, audit: &mut AuditLog, bundle_path: &Path) -> Result<()> {
    audit.connection.uri_redacted = "(offline: --bundle-list)".to_string();
    audit.connection.auth = "(not used)".to_string();
    audit.connection.tls_mode = "(not used)".to_string();
    audit.connection.user_source = Some("(not used)".to_string());
    audit.record_file_read(&bundle_path.display().to_string());
    let bundle = load_bundle_with_optional_blueprints(bundle_path, false)?;
    record_bundle_aggregation_warnings(&bundle, audit);
    let selector = combined_selector(&cli.select)?;
    println!("dbwarp-blueprint bundle: {}", bundle_path.display());
    println!(
        "aggregation={} sources={} logical_datasets={} tables={} rows={} table_bytes={} index_bytes={}",
        bundle.bundle_totals.aggregation,
        bundle.bundle_totals.source_count,
        bundle.bundle_totals.logical_dataset_count,
        bundle.bundle_totals.table_count,
        bundle.bundle_totals.row_count,
        bundle.bundle_totals.table_bytes,
        bundle.bundle_totals.index_bytes,
    );
    println!(
        "limitations={}",
        if bundle.bundle_totals.limitations.is_empty() {
            "none".to_string()
        } else {
            bundle.bundle_totals.limitations.join(",")
        }
    );
    for (group_id, group) in &bundle.dataset_groups {
        println!(
            "dataset_group={} relationship={} members_complete={} members={}",
            group_id,
            group.relationship,
            group.members_complete,
            group.members.join(",")
        );
    }
    for (source_id, source) in &bundle.sources {
        if !bundle_source_matches(source_id, source, &selector) {
            continue;
        }
        println!(
            "source={} kind={} engine={} dataset_relationship={} dataset_group={} dataset_scope={} tables={} rows={} tags={}",
            source_id,
            source.kind,
            source.engine,
            source.dataset_relationship,
            source.dataset_group,
            source.dataset_scope_completeness,
            source.table_count,
            source.row_count,
            source.tags.join(",")
        );
        if selector.table.is_some() || source.blueprint.is_some() {
            if let Some(blueprint) = load_source_blueprint(bundle_path, source_id, source)? {
                for (table_id, table) in &blueprint.tables {
                    if !table_matches(table_id, &selector) {
                        continue;
                    }
                    println!(
                        "  table={} rows={} bytes={} cols={} indexes={}",
                        table_id,
                        table.rows,
                        table.table_bytes,
                        table.cols.len(),
                        table.idxs.len()
                    );
                }
            }
        }
    }
    Ok(())
}
fn run_bundle_extract(cli: &Cli, audit: &mut AuditLog, bundle_path: &Path) -> Result<()> {
    audit.connection.uri_redacted = "(offline: --bundle-extract)".to_string();
    audit.connection.auth = "(not used)".to_string();
    audit.connection.tls_mode = "(not used)".to_string();
    audit.connection.user_source = Some("(not used)".to_string());
    audit.record_file_read(&bundle_path.display().to_string());
    let bundle = load_bundle_with_optional_blueprints(bundle_path, false)?;
    record_bundle_aggregation_warnings(&bundle, audit);
    let selector = combined_selector(&cli.select)?;
    let mut matches = Vec::new();
    for (source_id, source) in &bundle.sources {
        if bundle_source_matches(source_id, source, &selector) {
            matches.push((source_id, source));
        }
    }
    if matches.is_empty() {
        bail!(
            "DBP1201E bundle selector matched no sources. Next: run --bundle-list to inspect selectors. Available sources: {}",
            preview_bundle_sources(&bundle)
        );
    }
    if matches.len() > 1 && selector.table.is_none() {
        bail!(
            "DBP1202E bundle selector matched multiple sources: {}. Next: add --select source=ID, or use --bundle-list to inspect the bundle.",
            preview_matched_source_ids(&matches)
        );
    }

    let mut extracted: Option<dbwarp_blueprint_core::BlueprintFile> = None;
    let mut table_hints = Vec::new();
    for (source_id, source) in matches {
        let Some(mut blueprint) = load_source_blueprint(bundle_path, source_id, source)? else {
            continue;
        };
        if let Some(table_name) = &selector.table {
            let Some(table) = blueprint.tables.remove(table_name) else {
                table_hints.push(format!(
                    "{}: {}",
                    source_id,
                    preview_blueprint_tables(&blueprint)
                ));
                continue;
            };
            blueprint.tables.clear();
            blueprint.tables.insert(table_name.clone(), table);
            blueprint.fk_edges.retain(|child, edges| {
                if child != table_name {
                    return false;
                }
                edges.retain(|edge| edge.to == *table_name);
                !edges.is_empty()
            });
            recompute_blueprint_totals(&mut blueprint)?;
        }
        if extracted.is_some() {
            bail!(
                "DBP1202E bundle selector matched multiple extractable sources. Next: add --select source=ID."
            );
        }
        extracted = Some(blueprint);
    }
    let blueprint = extracted.ok_or_else(|| {
        let hint = if table_hints.is_empty() {
            "no loadable Blueprints were present for the selected sources".to_string()
        } else {
            format!("available tables for matched sources: {}", table_hints.join("; "))
        };
        anyhow!(
            "DBP1203E bundle selector matched no extractable Blueprint. Next: check --select source=ID,table=ID. {hint}"
        )
    })?;
    audit.record_fidelity(dbwarp_blueprint_core::estimate_blueprint_fidelity(
        &blueprint,
    ));
    let body = dbwarp_blueprint_core::blueprint_to_toml(&blueprint)
        .context("DBP1206E serializing extracted bundle Blueprint")?;
    write_bytes_with_parent(&cli.out, body.as_bytes())
        .context("DBP1206E writing extracted bundle Blueprint")?;
    audit.record_file_written(
        cli.out.clone(),
        body.len() as u64,
        sha256_bytes(body.as_bytes()),
    );
    println!(
        "{}",
        i18n::format("status.wrote", &[("path", cli.out.display().to_string())])
    );
    Ok(())
}

fn run_bundle_pack(cli: &Cli, audit: &mut AuditLog, bundle_input: &Path) -> Result<()> {
    audit.connection.uri_redacted = "(offline: --bundle-pack)".to_string();
    audit.connection.auth = "(not used)".to_string();
    audit.connection.tls_mode = "(not used)".to_string();
    audit.connection.user_source = Some("(not used)".to_string());
    let bundle_path = if bundle_input.is_dir() {
        bundle_input.join("bundle.toml")
    } else {
        bundle_input.to_path_buf()
    };
    audit.record_file_read(&bundle_path.display().to_string());
    let packed = load_bundle_with_optional_blueprints(&bundle_path, true)?;
    record_bundle_aggregation_warnings(&packed, audit);
    let body = dbwarp_blueprint_core::blueprint_bundle_to_toml(&packed)
        .context("DBP1206E serializing packed bundle")?;
    write_bytes_with_parent(&cli.out, body.as_bytes()).context("DBP1206E writing packed bundle")?;
    audit.record_file_written(
        cli.out.clone(),
        body.len() as u64,
        sha256_bytes(body.as_bytes()),
    );
    println!(
        "{}",
        i18n::format(
            "status.wrote_packed_bundle",
            &[("path", cli.out.display().to_string()),]
        )
    );
    Ok(())
}

fn load_bundle_with_optional_blueprints(
    bundle_path: &Path,
    embed: bool,
) -> Result<dbwarp_blueprint_core::BlueprintBundle> {
    let text = std::fs::read_to_string(bundle_path)
        .with_context(|| format!("DBP1204E reading bundle input {}", bundle_path.display()))?;
    let bundle = dbwarp_blueprint_core::parse_blueprint_bundle_toml(&text)
        .with_context(|| format!("DBP1205E parsing bundle input {}", bundle_path.display()))?;
    if embed {
        dbwarp_blueprint_core::blueprint_bundle_with_embedded_blueprints(bundle, bundle_path)
            .context("DBP1205E loading Blueprints referenced by bundle")
    } else {
        Ok(bundle)
    }
}

fn load_source_blueprint(
    bundle_path: &Path,
    source_id: &str,
    source: &dbwarp_blueprint_core::BundleSource,
) -> Result<Option<dbwarp_blueprint_core::BlueprintFile>> {
    if let Some(blueprint) = &source.blueprint {
        return Ok(Some(blueprint.clone()));
    }
    let Some(blueprint_path) = &source.blueprint_path else {
        return Ok(None);
    };
    let base = bundle_path.parent().unwrap_or_else(|| Path::new("."));
    let resolved = dbwarp_blueprint_core::resolve_bundle_path_checked(base, blueprint_path)
        .with_context(|| {
            format!("DBP1205E validating Blueprint path for bundle source '{source_id}'")
        })?;
    dbwarp_blueprint_core::read_blueprint_toml(&resolved)
        .with_context(|| format!("DBP1205E reading Blueprint for bundle source '{source_id}'"))
        .map(Some)
}

fn write_bytes_with_parent(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
    }
    atomic_write_bytes(path, bytes).with_context(|| format!("writing {}", path.display()))
}

fn atomic_write_bytes(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write as _;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("output");
    let partial = path.with_file_name(format!(
        ".{file_name}.dbwarp-partial-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| -> Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial)
            .with_context(|| format!("creating {}", partial.display()))?;
        file.write_all(bytes)
            .with_context(|| format!("writing {}", partial.display()))?;
        file.sync_all()
            .with_context(|| format!("syncing {}", partial.display()))?;
        std::fs::rename(&partial, path).with_context(|| {
            format!(
                "publishing {} atomically as {}",
                partial.display(),
                path.display()
            )
        })?;
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            sync_directory(parent)?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&partial);
    }
    result
}

fn combined_selector(values: &[String]) -> Result<dbwarp_blueprint_core::BlueprintSelector> {
    let mut out = dbwarp_blueprint_core::BlueprintSelector::default();
    for raw in values {
        let selector = dbwarp_blueprint_core::parse_blueprint_selector(raw)?;
        merge_selector_field(&mut out.source, selector.source, "source")?;
        merge_selector_field(&mut out.table, selector.table, "table")?;
        merge_selector_field(&mut out.engine, selector.engine, "engine")?;
        merge_selector_field(&mut out.tag, selector.tag, "tag")?;
    }
    Ok(out)
}

fn merge_selector_field(
    target: &mut Option<String>,
    value: Option<String>,
    name: &str,
) -> Result<()> {
    if let Some(value) = value {
        if let Some(existing) = target {
            if existing != &value {
                bail!(
                    "DBP1200E conflicting --select {name}= values: '{existing}' and '{value}'. Next: pass each selector key once."
                );
            }
        } else {
            *target = Some(value);
        }
    }
    Ok(())
}

fn bundle_source_matches(
    source_id: &str,
    source: &dbwarp_blueprint_core::BundleSource,
    selector: &dbwarp_blueprint_core::BlueprintSelector,
) -> bool {
    if let Some(wanted) = &selector.source {
        if source_id != wanted {
            return false;
        }
    }
    if let Some(wanted) = &selector.engine {
        if source.engine.to_ascii_lowercase() != *wanted {
            return false;
        }
    }
    if let Some(wanted) = &selector.tag {
        if !source.tags.iter().any(|tag| tag == wanted) {
            return false;
        }
    }
    true
}

fn table_matches(table_id: &str, selector: &dbwarp_blueprint_core::BlueprintSelector) -> bool {
    selector
        .table
        .as_ref()
        .map(|wanted| table_id == wanted)
        .unwrap_or(true)
}

fn preview_bundle_sources(bundle: &dbwarp_blueprint_core::BlueprintBundle) -> String {
    let mut items = bundle
        .sources
        .iter()
        .take(8)
        .map(|(source_id, source)| {
            let tags = if source.tags.is_empty() {
                "-".to_string()
            } else {
                source.tags.join(",")
            };
            format!(
                "{}(engine={}, tables={}, tags={})",
                source_id, source.engine, source.table_count, tags
            )
        })
        .collect::<Vec<_>>();
    if bundle.sources.len() > items.len() {
        items.push(format!("... {} more", bundle.sources.len() - items.len()));
    }
    if items.is_empty() {
        "(none)".to_string()
    } else {
        items.join(", ")
    }
}

fn preview_matched_source_ids(
    matches: &[(&String, &dbwarp_blueprint_core::BundleSource)],
) -> String {
    let items = matches
        .iter()
        .take(12)
        .map(|(source_id, _)| source_id.as_str())
        .collect::<Vec<_>>();
    let mut out = if items.is_empty() {
        "(none)".to_string()
    } else {
        items.join(", ")
    };
    if matches.len() > items.len() {
        out.push_str(&format!(", ... {} more", matches.len() - items.len()));
    }
    out
}

fn preview_blueprint_tables(blueprint: &dbwarp_blueprint_core::BlueprintFile) -> String {
    let items = blueprint
        .tables
        .keys()
        .take(12)
        .map(String::as_str)
        .collect::<Vec<_>>();
    let mut out = if items.is_empty() {
        "(none)".to_string()
    } else {
        items.join(", ")
    };
    if blueprint.tables.len() > items.len() {
        out.push_str(&format!(
            ", ... {} more",
            blueprint.tables.len() - items.len()
        ));
    }
    out
}

fn recompute_blueprint_totals(blueprint: &mut dbwarp_blueprint_core::BlueprintFile) -> Result<()> {
    blueprint.totals.table_count = u64::try_from(
        blueprint
            .tables
            .values()
            .filter(|table| table.counts_toward_totals())
            .count(),
    )
    .context("DBP1114E Blueprint table count exceeds the supported u64 range")?;
    blueprint.totals.row_count =
        checked_blueprint_total(blueprint, "row count", |table| table.rows)?;
    blueprint.totals.table_bytes =
        checked_blueprint_total(blueprint, "logical table bytes", |table| table.table_bytes)?;
    blueprint.totals.index_bytes =
        checked_blueprint_total(blueprint, "index bytes", |table| table.index_bytes)?;
    Ok(())
}

fn checked_blueprint_total(
    blueprint: &dbwarp_blueprint_core::BlueprintFile,
    field: &str,
    value: impl Fn(&dbwarp_blueprint_core::BlueprintTable) -> u64,
) -> Result<u64> {
    blueprint
        .tables
        .values()
        .filter(|table| table.counts_toward_totals())
        .try_fold(0_u64, |total, table| {
            total.checked_add(value(table)).with_context(|| {
                format!(
                    "DBP1114E Blueprint {field} overflow while aggregating tables. Next: split the batch or inspect the source metadata."
                )
            })
        })
}

fn resolve_batch_connect(source: &BatchSource, manifest_base: &Path) -> Result<String> {
    let mut set = 0;
    set += source.connect.is_some() as usize;
    set += source.connect_env.is_some() as usize;
    set += source.connect_file.is_some() as usize;
    if set != 1 {
        bail!(
            "DBP1110E database source '{}' requires exactly one of connect, connect_env, or connect_file. Next: set exactly one connection source in the manifest.",
            source.id
        );
    }
    if let Some(connect) = &source.connect {
        return Ok(connect.clone());
    }
    if let Some(var) = &source.connect_env {
        return std::env::var(var).with_context(|| {
            format!("DBP1111E could not read connect_env '{var}'. Next: export the variable or use connect_file.")
        });
    }
    let file = source.connect_file.as_ref().expect("checked above");
    let resolved = resolve_batch_input_path(manifest_base, file);
    std::fs::read_to_string(&resolved)
        .with_context(|| {
            format!(
                "DBP1112E could not read connect_file {}. Next: check the path relative to the batch manifest and file permissions.",
                resolved.display()
            )
        })
        .map(|s| s.trim().to_string())
}

fn normalized_batch_kind(kind: &str) -> String {
    match kind.trim().to_ascii_lowercase().as_str() {
        "postgres" | "postgresql" => "postgresql".to_string(),
        "mysql" | "mariadb" => "mysql".to_string(),
        "sqlserver" | "mssql" | "tds" => "sqlserver".to_string(),
        "parquet" | "parquet_dataset" => "parquet".to_string(),
        "avro" | "avro_dataset" => "avro".to_string(),
        other => other.to_string(),
    }
}

fn sanitize_source_id(id: &str) -> Result<String> {
    let sanitized = sanitize_identifier_for_table(id);
    if sanitized.is_empty() {
        bail!("DBP1109E source id must contain at least one ASCII letter or digit");
    }
    Ok(sanitized)
}

fn sanitize_identifier_for_table(input: impl AsRef<str>) -> String {
    let mut out = String::new();
    let mut prev_us = false;
    for ch in input.as_ref().chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            ch.to_ascii_lowercase()
        } else {
            '_'
        };
        if mapped == '_' {
            if !prev_us && !out.is_empty() {
                out.push('_');
            }
            prev_us = true;
        } else {
            out.push(mapped);
            prev_us = false;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    out
}

fn relative_bundle_path(prefix: &str, path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .map(|name| format!("{prefix}/{name}"))
        .unwrap_or_else(|| path.display().to_string())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

fn print_batch_preflight(manifest: &BatchManifest, manifest_path: &Path, out_dir: &Path) {
    eprintln!("{}", i18n::text("preflight.title"));
    preflight_line("preflight.mode", "blueprint-batch");
    preflight_line("preflight.manifest", manifest_path.display());
    preflight_line("preflight.output_directory", out_dir.display());
    preflight_line("preflight.sources", manifest.sources.len());
    for source in &manifest.sources {
        eprintln!(
            "    - id={} kind={} compression={:?} sample_rows={:?}",
            source.id, source.kind, source.measure_compression, source.sample_rows
        );
    }
}
