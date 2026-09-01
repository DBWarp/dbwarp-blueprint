fn run_deck_from_toml(cli: &Cli, audit: &mut AuditLog, blueprint_path: &Path) -> Result<()> {
    let deck_path = cli.deck.as_ref().ok_or_else(|| {
        anyhow!("DBP1301E --from-toml requires --deck PATH. Next: pass --deck output.pptx.")
    })?;

    audit.connection.uri_redacted = "(offline: --from-toml)".to_string();
    audit.connection.auth = "(not used)".to_string();
    audit.connection.tls_mode = "(not used)".to_string();
    audit.connection.user_source = Some("(not used)".to_string());

    if cli.dry_run {
        print_from_toml_preflight(blueprint_path, deck_path, cli.deck_confidentiality.as_ref());
        eprintln!("{}", i18n::text("dry.deck"));
        return Ok(());
    }

    let source = std::fs::read_to_string(blueprint_path).with_context(|| {
        format!(
            "DBP1503E reading Blueprint TOML {}",
            blueprint_path.display()
        )
    })?;
    audit.record_file_read(&blueprint_path.display().to_string());
    if let Ok(probe) = source.parse::<toml::Value>() {
        if let Some(version) = probe
            .get("schema_version")
            .and_then(toml::Value::as_integer)
        {
            let supported = i64::from(dbwarp_blueprint_core::MIN_SCHEMA_VERSION)
                ..=i64::from(dbwarp_blueprint_core::SCHEMA_VERSION);
            if !supported.contains(&version) {
                bail!(
                    "DBP1302E unsupported Blueprint TOML schema_version {version}; supported range is {}..={}. Next: regenerate the Blueprint with a compatible dbwarp-blueprint release.",
                    dbwarp_blueprint_core::MIN_SCHEMA_VERSION,
                    dbwarp_blueprint_core::SCHEMA_VERSION
                );
            }
        }
    }
    let canonical = dbwarp_blueprint_core::parse_blueprint_toml(&source).with_context(|| {
        format!(
            "DBP1503E parsing Blueprint TOML {}",
            blueprint_path.display()
        )
    })?;
    audit.record_fidelity(dbwarp_blueprint_core::estimate_blueprint_fidelity(
        &canonical,
    ));
    let canonical_toml = toml::to_string(&canonical)
        .context("DBP1503E converting the canonical Blueprint model for deck generation")?;
    let blueprint: BlueprintFile = toml::from_str(&canonical_toml)
        .context("DBP1503E decoding the canonical Blueprint model for deck generation")?;

    write_deck(
        deck_path,
        &blueprint,
        cli.deck_confidentiality.as_ref(),
        audit,
    )
}

fn print_from_toml_preflight(
    blueprint_path: &Path,
    deck_path: &Path,
    deck_confidentiality: Option<&DeckConfidentiality>,
) {
    eprintln!("{}", i18n::text("preflight.title"));
    preflight_line("preflight.mode", "deck-from-toml");
    preflight_line("preflight.database", i18n::text("value.none"));
    preflight_line("preflight.input_toml", blueprint_path.display());
    preflight_line(
        "preflight.deck",
        i18n::format(
            "value.generated_local",
            &[("path", deck_path.display().to_string())],
        ),
    );
    if let Some(level) = deck_confidentiality {
        preflight_line("preflight.deck_confidentiality", level.as_str());
    }
    eprintln!();
}

fn write_deck(
    deck_path: &Path,
    blueprint: &BlueprintFile,
    deck_confidentiality: Option<&DeckConfidentiality>,
    audit: &mut AuditLog,
) -> Result<()> {
    if let Some(parent) = deck_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("DBP1503E creating deck dir {}", parent.display()))?;
        }
    }
    let deck_bytes = deck::build_pptx_with_confidentiality(
        blueprint,
        deck_confidentiality.map(DeckConfidentiality::label),
    );
    atomic_write_bytes(deck_path, &deck_bytes)
        .with_context(|| format!("DBP1503E writing deck {}", deck_path.display()))?;
    let mut dh = Sha256::new();
    dh.update(&deck_bytes);
    let dsha = hex::encode(dh.finalize());
    audit.record_file_written(deck_path.to_path_buf(), deck_bytes.len() as u64, dsha);
    println!(
        "{}",
        i18n::format(
            "status.wrote",
            &[("path", deck_path.display().to_string()),]
        )
    );
    Ok(())
}
