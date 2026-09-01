#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_capture_timeout_is_created_inside_its_runtime() {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("build test runtime");
        let result =
            block_on_with_timeout(&runtime, std::time::Duration::from_secs(1), async { 42_u8 });
        assert_eq!(result.expect("immediate future must beat timeout"), 42);
    }

    #[test]
    #[ignore]
    fn dump_canonical_i18n_source_for_catalog_maintenance() {
        let mut command = Cli::command().term_width(0);
        command.build();
        let messages = i18n::MESSAGE_SPECS
            .iter()
            .map(|spec| {
                (
                    spec.code,
                    serde_json::json!({
                        "summary": spec.summary,
                        "cause": spec.cause,
                        "action": spec.action,
                    }),
                )
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        let text = i18n::UI_TEXT
            .iter()
            .copied()
            .collect::<std::collections::BTreeMap<_, _>>();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "schema_version": 1,
                "locale": "en",
                "messages": messages,
                "text": text,
                "help_phrases": canonical_help_phrases(&command),
            }))
            .unwrap()
        );
    }

    #[test]
    fn every_embedded_locale_exactly_covers_the_live_cli() {
        let mut command = Cli::command().term_width(0);
        command.build();
        i18n::validate_catalogs(&canonical_help_phrases(&command)).unwrap();
    }

    #[test]
    fn engine_kind_for_unknown_scheme_does_not_leak_password() {
        // Regression guard: a malformed URI with an embedded password must
        // not appear verbatim in the error message — only the scheme prefix
        // (truncated to 64 chars) is acceptable in user-facing output.
        let leak = "ftp://app:s3cretP@ss@db.internal/payments";
        let err = engine_kind_for(leak).unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            !msg.contains("s3cretP@ss"),
            "error message must not contain the embedded password; got: {msg}"
        );
        assert!(
            !msg.contains("app:s3cretP"),
            "error message must not contain user:password segment; got: {msg}"
        );
        assert!(
            msg.contains("ftp"),
            "error message should still hint at the offending scheme; got: {msg}"
        );
    }

    #[test]
    fn engine_kind_for_no_scheme_separator_uses_placeholder() {
        // Pathological input with no "://" must not panic and must not
        // echo the raw input (which could be anything, including a password).
        let err = engine_kind_for("just-some-garbage-with-no-scheme").unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("(no scheme)"),
            "expected placeholder; got: {msg}"
        );
        assert!(
            !msg.contains("just-some-garbage"),
            "raw input must not appear in error; got: {msg}"
        );
    }

    #[test]
    fn engine_kind_for_unknown_unicode_scheme_truncates_on_character_boundary() {
        let scheme = "é".repeat(40);
        let uri = format!("{scheme}://db.example/app");
        let error = engine_kind_for(&uri).expect_err("unknown scheme must be rejected");
        let message = format!("{error:#}");
        assert!(message.contains(&scheme));
    }

    #[test]
    fn anonymization_key_accepts_raw_and_hex_encodings() {
        assert_eq!(parse_anonymization_key(&[7_u8; 32]).unwrap(), [7_u8; 32]);
        assert_eq!(
            parse_anonymization_key(
                b"000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f\n"
            )
            .unwrap(),
            [
                0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18,
                19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31,
            ]
        );
        assert!(parse_anonymization_key(b"too-short").is_err());
    }

    #[test]
    fn operator_error_prefers_the_deepest_coded_inner_cause() {
        let error = anyhow!("DBP1204E reading bundle input failed")
            .context("uncoded filesystem wrapper")
            .context("DBP0001E generic outer wrapper");
        let rendered = render_operator_error("DBP0001E", "generic failure", &error);
        assert!(rendered.starts_with("DBP1204E "), "rendered:\n{rendered}");
        assert!(
            rendered.contains("uncoded filesystem wrapper"),
            "causal context was lost:\n{rendered}"
        );
        assert!(!rendered.starts_with("DBP0001E"));
    }

    #[test]
    fn audit_outcome_truncation_preserves_utf8_boundaries() {
        let mut message = format!("{}日本語", "x".repeat(239));
        truncate_audit_message(&mut message, 240);
        assert!(message.is_char_boundary(message.len()));
        assert_eq!(message, format!("{}…", "x".repeat(239)));
    }

    #[test]
    fn batch_input_paths_preserve_absolute_and_resolve_relative_locations() {
        let base = Path::new("/srv/dbwarp/manifests");
        assert_eq!(
            resolve_batch_input_path(base, "../fixtures/orders.parquet"),
            PathBuf::from("/srv/dbwarp/manifests/../fixtures/orders.parquet")
        );
        assert_eq!(
            resolve_batch_input_path(base, "/data/orders/*.parquet"),
            PathBuf::from("/data/orders/*.parquet")
        );
    }

    fn empty_cli() -> Cli {
        // Build a Cli stub with all credential paths unset for unit tests.
        Cli {
            lang: None,
            color: CliColorMode::Auto,
            banner: false,
            banner_mode: CliBannerMode::Auto,
            connect: Some("postgresql://app@localhost/db".to_string()),
            schema: Vec::new(),
            out: PathBuf::from("blueprint.toml"),
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
            source_kind: "production".to_string(),
            measure_compression: false,
            artifact_detail: ArtifactDetail::Summary,
            length_fidelity: LengthFidelity::Balanced,
            preserve_exact_lengths: false,
            yes: false,
            sample_rows: 1000,
            compression_workers: None,
            max_wall_secs: 300,
            no_rtt_probe: false,
            user: None,
            user_env: None,
            user_file: None,
            password_file: None,
            password_env: None,
            anonymization_key_file: None,
            azure_token_file: None,
            azure_token_env: None,
            auth_mode: None,
            expect_server_principal: None,
            audit_log: None,
            generated_at: None,
            dry_run: false,
            tls_mode: "verify-full".to_string(),
            tls_ca: None,
            tls_cert: None,
            tls_key: None,
            tls_server_name: None,
            tls_skip_verify: false,
            i_know_what_im_doing: false,
        }
    }

    #[test]
    fn live_schema_selector_is_repeatable_and_preserves_source_spelling() {
        let cli = Cli::try_parse_from([
            "dbwarp-blueprint",
            "--connect",
            "postgresql://app@localhost/db",
            "--schema",
            "app",
            "--schema",
            "Sales Data",
        ])
        .unwrap();
        assert_eq!(cli.schema, vec!["app", "Sales Data"]);
    }

    #[test]
    fn schema_selector_is_live_mode_only() {
        let error = Cli::try_parse_from([
            "dbwarp-blueprint",
            "--from-toml",
            "blueprint.toml",
            "--deck",
            "review.pptx",
            "--schema",
            "app",
        ])
        .unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    #[test]
    fn live_compression_workers_are_bounded_and_explicit() {
        assert_eq!(default_compression_workers(), 1);
        let cli = Cli::try_parse_from([
            "dbwarp-blueprint",
            "--connect",
            "postgresql://app@localhost/db",
            "--measure-compression",
            "--compression-workers",
            "4",
        ])
        .expect("live Tier 2 should accept an explicit bounded worker count");
        assert_eq!(cli.compression_workers, Some(4));

        for invalid in ["0", "33"] {
            assert!(Cli::try_parse_from([
                "dbwarp-blueprint",
                "--connect",
                "postgresql://app@localhost/db",
                "--measure-compression",
                "--compression-workers",
                invalid,
            ])
            .is_err());
        }
    }

    #[test]
    fn compression_workers_require_live_tier_two() {
        assert!(Cli::try_parse_from([
            "dbwarp-blueprint",
            "--connect",
            "postgresql://app@localhost/db",
            "--compression-workers",
            "2",
        ])
        .is_err());
        assert!(Cli::try_parse_from([
            "dbwarp-blueprint",
            "--from-parquet",
            "fixture.parquet",
            "--measure-compression",
            "--compression-workers",
            "2",
        ])
        .is_err());
    }

    #[test]
    fn postgres_cloud_token_requires_exactly_one_external_secret_source() {
        let mut cli = empty_cli();
        cli.auth_mode = Some(AuthMode::CloudToken);
        let error = resolve_auth_mode(&cli, EngineKind::Postgresql).unwrap_err();
        assert!(format!("{error:#}").contains("DBP1604E"));

        cli.password_env = Some("DBWARP_BLUEPRINT_TEST_CLOUD_TOKEN".to_string());
        assert_eq!(
            resolve_auth_mode(&cli, EngineKind::Postgresql).unwrap(),
            AuthMode::CloudToken
        );

        cli.password_file = Some(PathBuf::from("token.txt"));
        let error = resolve_auth_mode(&cli, EngineKind::Postgresql).unwrap_err();
        assert!(format!("{error:#}").contains("exactly one"));
    }

    #[test]
    fn cloud_token_requires_hostname_verified_tls() {
        let mut tls = TlsParams {
            mode: TlsMode::Require,
            ..TlsParams::default()
        };
        let error = validate_cloud_token_tls(AuthMode::CloudToken, &tls).unwrap_err();
        assert!(format!("{error:#}").contains("--tls-mode=verify-full"));

        tls.mode = TlsMode::VerifyFull;
        validate_cloud_token_tls(AuthMode::CloudToken, &tls).unwrap();
        validate_cloud_token_tls(AuthMode::SqlAuth, &TlsParams::default()).unwrap();
    }

    #[test]
    fn engine_specific_auth_modes_are_rejected_before_secret_acquisition() {
        let mut cli = empty_cli();
        cli.auth_mode = Some(AuthMode::EntraToken);
        let error = resolve_auth_mode(&cli, EngineKind::MySQL).unwrap_err();
        assert!(format!("{error:#}").contains("DBP1005E"));

        cli.auth_mode = Some(AuthMode::CloudToken);
        cli.password_env = Some("DBWARP_BLUEPRINT_TEST_CLOUD_TOKEN".to_string());
        let error = resolve_auth_mode(&cli, EngineKind::Mssql).unwrap_err();
        assert!(format!("{error:#}").contains("DBP1005E"));
    }

    #[test]
    fn expected_server_principal_is_sqlserver_only_and_audit_safe() {
        let mut cli = empty_cli();
        cli.connect = Some("sqlserver://db.example/inventory".to_string());
        cli.expect_server_principal = Some("DOMAIN\\svc-blueprint".to_string());
        assert!(validate_expected_server_principal(&cli, EngineKind::Mssql).is_ok());

        let error = validate_expected_server_principal(&cli, EngineKind::Postgresql).unwrap_err();
        assert!(format!("{error:#}").contains("DBP1005E"));

        cli.expect_server_principal = Some("DOMAIN\\svc\nforged".to_string());
        let error = validate_expected_server_principal(&cli, EngineKind::Mssql).unwrap_err();
        assert!(format!("{error:#}").contains("DBP1606E"));
    }

    #[test]
    fn expected_server_principal_requires_live_connection() {
        assert!(Cli::try_parse_from([
            "dbwarp-blueprint",
            "--from-parquet",
            "fixture.parquet",
            "--expect-server-principal",
            "DOMAIN\\svc-blueprint",
        ])
        .is_err());
        let parsed = Cli::try_parse_from([
            "dbwarp-blueprint",
            "--connect",
            "sqlserver://db.example/inventory",
            "--expect-server-principal",
            "DOMAIN\\svc-blueprint",
        ])
        .expect("live SQL Server CLI should accept a principal assertion");
        assert_eq!(
            parsed.expect_server_principal.as_deref(),
            Some("DOMAIN\\svc-blueprint")
        );
    }

    fn temp_test_dir(prefix: &str) -> PathBuf {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tmp/tests");
        path.push(format!(
            "{}-{}-{}",
            prefix,
            std::process::id(),
            unix_ms(SystemTime::now())
        ));
        path
    }

    fn write_owned_test_bundle(directory: &Path, generated_at: &str) -> BatchOwnerMarker {
        std::fs::create_dir_all(directory).unwrap();
        let mut bundle = dbwarp_blueprint_core::BlueprintBundle {
            schema_version: dbwarp_blueprint_core::BUNDLE_SCHEMA_VERSION,
            kind: dbwarp_blueprint_core::BUNDLE_KIND.to_string(),
            generated_at: generated_at.to_string(),
            sources: std::collections::BTreeMap::from([(
                "test-source".to_string(),
                dbwarp_blueprint_core::BundleSource {
                    kind: "database".to_string(),
                    engine: "postgresql".to_string(),
                    dataset_relationship: "independent".to_string(),
                    dataset_group: "dataset-test".to_string(),
                    dataset_scope_completeness: "complete".to_string(),
                    ..Default::default()
                },
            )]),
            dataset_groups: std::collections::BTreeMap::from([(
                "dataset-test".to_string(),
                dbwarp_blueprint_core::BundleDatasetGroup {
                    relationship: "independent".to_string(),
                    members_complete: true,
                    members: vec!["test-source".to_string()],
                },
            )]),
            ..Default::default()
        };
        dbwarp_blueprint_core::recompute_bundle_totals(&mut bundle).unwrap();
        std::fs::write(
            directory.join("bundle.toml"),
            dbwarp_blueprint_core::blueprint_bundle_to_toml(&bundle).unwrap(),
        )
        .unwrap();
        write_batch_owner_marker(directory).unwrap()
    }

    #[test]
    fn batch_publish_replaces_owned_output_as_one_directory() {
        let root = temp_test_dir("dbwarp-blueprint-batch-publish");
        let published = root.join("bundle");
        let staging = root.join("stage");
        write_owned_test_bundle(&published, "old");
        std::fs::write(published.join("stale.txt"), "old").unwrap();
        write_owned_test_bundle(&staging, "new");
        std::fs::create_dir_all(staging.join("blueprints")).unwrap();
        std::fs::write(staging.join("blueprints/new.blueprint.toml"), "new").unwrap();

        publish_batch_staging_dir(&staging, &published).unwrap();

        let bundle =
            dbwarp_blueprint_core::read_blueprint_bundle_toml(published.join("bundle.toml"))
                .unwrap();
        assert_eq!(bundle.generated_at, "new");
        assert!(published.join("blueprints/new.blueprint.toml").is_file());
        assert!(!published.join("stale.txt").exists());
        assert!(!staging.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn batch_publish_refuses_non_owned_nonempty_output() {
        let root = temp_test_dir("dbwarp-blueprint-batch-refuse");
        let published = root.join("customer-data");
        let staging = root.join("stage");
        std::fs::create_dir_all(&published).unwrap();
        std::fs::write(published.join("keep.txt"), "customer").unwrap();
        write_owned_test_bundle(&staging, "new");

        let error = publish_batch_staging_dir(&staging, &published).unwrap_err();

        assert!(format!("{error:#}").contains("refusing to replace"));
        assert_eq!(
            std::fs::read_to_string(published.join("keep.txt")).unwrap(),
            "customer"
        );
        assert!(staging.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn batch_publish_refuses_forged_or_empty_bundle_without_owner_marker() {
        let root = temp_test_dir("dbwarp-blueprint-batch-forged");
        let published = root.join("customer-data");
        let staging = root.join("stage");
        std::fs::create_dir_all(&published).unwrap();
        std::fs::write(published.join("bundle.toml"), "").unwrap();
        std::fs::write(published.join("keep.txt"), "customer").unwrap();
        write_owned_test_bundle(&staging, "new");

        let error = publish_batch_staging_dir(&staging, &published).unwrap_err();

        assert!(format!("{error:#}").contains("refusing to replace"));
        assert_eq!(
            std::fs::read_to_string(published.join("keep.txt")).unwrap(),
            "customer"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn interrupted_batch_publish_restores_owned_backup() {
        let root = temp_test_dir("dbwarp-blueprint-batch-recover");
        let published = root.join("bundle");
        let staging_id = "0123456789abcdef0123456789abcdef";
        let staging = root.join(format!(".bundle.dbwarp-stage-{staging_id}"));
        write_owned_test_bundle(&published, "old");
        let staging_marker = write_owned_test_bundle(&staging, "new");
        let generation_id = staging_marker.generation_id;
        let backup = root.join(format!(".bundle.dbwarp-backup-{generation_id}"));
        std::fs::rename(&published, &backup).unwrap();
        let journal = BatchPublishJournal {
            kind: "dbwarp-blueprint-bundle-publish".to_string(),
            version: 1,
            generation_id,
            staging_name: staging.file_name().unwrap().to_str().unwrap().to_string(),
            backup_name: backup.file_name().unwrap().to_str().unwrap().to_string(),
        };
        atomic_write_bytes(
            &batch_publish_journal_path(&published),
            toml::to_string(&journal).unwrap().as_bytes(),
        )
        .unwrap();

        recover_batch_publication(&published).unwrap();

        let bundle =
            dbwarp_blueprint_core::read_blueprint_bundle_toml(published.join("bundle.toml"))
                .unwrap();
        assert_eq!(bundle.generated_at, "old");
        assert!(!staging.exists());
        assert!(!backup.exists());
        assert!(!batch_publish_journal_path(&published).exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn interrupted_batch_publish_refuses_unrelated_owned_staging_directory() {
        let root = temp_test_dir("dbwarp-blueprint-batch-journal-scope");
        let published = root.join("bundle");
        let staging_id = "0123456789abcdef0123456789abcdef";
        let unrelated_staging = root.join(format!(".bundle.dbwarp-stage-{staging_id}"));
        write_owned_test_bundle(&published, "current");
        write_owned_test_bundle(&unrelated_staging, "unrelated");

        let journal_generation = "fedcba9876543210fedcba9876543210";
        let journal = BatchPublishJournal {
            kind: "dbwarp-blueprint-bundle-publish".to_string(),
            version: 1,
            generation_id: journal_generation.to_string(),
            staging_name: unrelated_staging
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string(),
            backup_name: format!(".bundle.dbwarp-backup-{journal_generation}"),
        };
        atomic_write_bytes(
            &batch_publish_journal_path(&published),
            toml::to_string(&journal).unwrap().as_bytes(),
        )
        .unwrap();

        let error = recover_batch_publication(&published).unwrap_err();

        assert!(format!("{error:#}").contains("does not match journal"));
        assert!(published.is_dir());
        assert!(unrelated_staging.is_dir());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn batch_manifest_rejects_unknown_fields_and_normalized_id_collisions() {
        let unknown = r#"
unexpected = true

[[source]]
id = "erp"
kind = "parquet"
path = "erp.parquet"
"#;
        assert!(toml::from_str::<BatchManifest>(unknown).is_err());

        let manifest = BatchManifest {
            defaults: BatchDefaults::default(),
            sources: vec![
                BatchSource {
                    id: "ERP/Orders".into(),
                    kind: "parquet".into(),
                    path: Some("one.parquet".into()),
                    ..BatchSource::default()
                },
                BatchSource {
                    id: "erp_orders".into(),
                    kind: "avro".into(),
                    path: Some("two.avro".into()),
                    ..BatchSource::default()
                },
            ],
        };
        let error = validate_batch_manifest_contract(&manifest).unwrap_err();
        assert!(format!("{error:#}").contains("DBP1109E"));
        assert!(format!("{error:#}").contains("both normalize"));
    }

    #[test]
    fn batch_manifest_dry_contract_rejects_cross_kind_and_sampling_fields() {
        let database_with_path = BatchManifest {
            defaults: BatchDefaults::default(),
            sources: vec![BatchSource {
                id: "erp".into(),
                kind: "postgresql".into(),
                connect_env: Some("ERP_DATABASE_URL".into()),
                path: Some("unexpected.parquet".into()),
                ..BatchSource::default()
            }],
        };
        assert!(format!(
            "{:#}",
            validate_batch_manifest_contract(&database_with_path).unwrap_err()
        )
        .contains("DBP1102E"));

        let zero_sample = BatchManifest {
            defaults: BatchDefaults::default(),
            sources: vec![BatchSource {
                id: "events".into(),
                kind: "parquet".into(),
                path: Some("events.parquet".into()),
                measure_compression: Some(true),
                sample_rows: Some(0),
                ..BatchSource::default()
            }],
        };
        assert!(format!(
            "{:#}",
            validate_batch_manifest_contract(&zero_sample).unwrap_err()
        )
        .contains("sample_rows=0"));
    }

    #[test]
    fn structured_dataset_compatibility_includes_unsigned_and_bit_width() {
        let table = |numeric_unsigned, bit_width| {
            let mut table = dbwarp_blueprint_core::BlueprintTable::default();
            table.cols.insert(
                "col-1".into(),
                dbwarp_blueprint_core::BlueprintColumn {
                    ordinal: 1,
                    column_type: "integer".into(),
                    native_type: "int".into(),
                    numeric_unsigned,
                    bit_width,
                    ..Default::default()
                },
            );
            table
        };
        let signed = table(false, 0);
        assert!(!structured_tables_compatible(&signed, &table(true, 0)));
        assert!(!structured_tables_compatible(&signed, &table(false, 8)));
        assert!(structured_tables_compatible(&signed, &table(false, 0)));
    }

    #[test]
    fn failed_batch_generation_preserves_previous_bundle() {
        let root = temp_test_dir("dbwarp-blueprint-batch-failure");
        let published = root.join("bundle");
        let manifest = root.join("batch.toml");
        write_owned_test_bundle(&published, "old");
        std::fs::write(published.join("sentinel.txt"), "old generation").unwrap();
        std::fs::write(
            &manifest,
            "[[source]]\nid = \"bad\"\nkind = \"unsupported\"\n",
        )
        .unwrap();
        let mut cli = empty_cli();
        cli.connect = None;
        cli.batch_manifest = Some(manifest.clone());
        cli.out_dir = Some(published.clone());
        cli.yes = true;
        let mut audit = AuditLog::new("blueprint-batch", 1_000);

        let error = run_batch_manifest(&cli, &mut audit, &manifest).unwrap_err();

        assert!(format!("{error:#}").contains("unsupported batch source kind"));
        let bundle =
            dbwarp_blueprint_core::read_blueprint_bundle_toml(published.join("bundle.toml"))
                .unwrap();
        assert_eq!(bundle.generated_at, "old");
        assert_eq!(
            std::fs::read_to_string(published.join("sentinel.txt")).unwrap(),
            "old generation"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    fn test_blueprint(
        engine: &str,
        table_name: &str,
        rows: u64,
    ) -> dbwarp_blueprint_core::BlueprintFile {
        let mut blueprint = dbwarp_blueprint_core::BlueprintFile {
            schema_version: dbwarp_blueprint_core::SCHEMA_VERSION,
            engine: engine.to_string(),
            source_kind: "test".to_string(),
            database_topology: (!matches!(engine, "parquet" | "avro"))
                .then(dbwarp_blueprint_core::DatabaseTopology::unknown),
            dataset_scope: Some(if matches!(engine, "parquet" | "avro") {
                dbwarp_blueprint_core::DatasetScope::structured_dataset(
                    "parquet-footer",
                    "parquet-footer",
                )
            } else {
                dbwarp_blueprint_core::DatasetScope::unknown_database("unknown", "unknown")
            }),
            ..Default::default()
        };
        let mut table = dbwarp_blueprint_core::BlueprintTable {
            rows,
            table_bytes: rows.saturating_mul(16),
            ..Default::default()
        };
        table.cols.insert(
            "col-1".to_string(),
            dbwarp_blueprint_core::BlueprintColumn {
                ordinal: 1,
                column_type: "int".to_string(),
                nullable: false,
                len_avg: 8,
                len_p95: 8,
                ..Default::default()
            },
        );
        blueprint.tables.insert(table_name.to_string(), table);
        recompute_blueprint_totals(&mut blueprint).expect("test Blueprint totals");
        blueprint
    }

    fn structured_test_blueprint(
        rows: u64,
        table_bytes: u64,
        storage_bytes: u64,
        codec: &str,
    ) -> dbwarp_blueprint_core::BlueprintFile {
        let mut blueprint = test_blueprint("parquet", "table-001", rows);
        let table = blueprint.tables.get_mut("table-001").unwrap();
        table.table_bytes = table_bytes;
        table.storage_bytes = storage_bytes;
        table.source_partitions = 1;
        table.row_group_count = 2;
        table.source_codec = codec.to_string();
        table.compression = Some(dbwarp_blueprint_core::BlueprintCompression {
            measured: true,
            sample_rows: rows,
            sample_bytes: table_bytes,
            sample_method: "parquet-footer".to_string(),
            ratio_storage: compression_ratio(table_bytes, storage_bytes),
            sample_encoding: "parquet-file".to_string(),
            ..Default::default()
        });
        let column = table.cols.get_mut("col-1").unwrap();
        column.null_fraction = Some(0.25);
        column.length_sample_rows = rows;
        column.length_sample_method = "parquet-decoded-row-sample".to_string();
        column.cardinality = Some(dbwarp_blueprint_core::BlueprintCardinality {
            measured: true,
            sample_rows: rows,
            non_null_rows: rows.saturating_mul(3) / 4,
            observed_distinct_count: rows.saturating_mul(3) / 8,
            estimated_distinct_count: rows.saturating_mul(3) / 8,
            top_value_fraction: 0.2,
            frequency_p50: 1,
            frequency_p95: 2,
            frequency_p99: 3,
            frequency_max: 4,
            sample_method: "structured-test-cardinality".to_string(),
            ..Default::default()
        });
        recompute_blueprint_totals(&mut blueprint).expect("structured test Blueprint totals");
        blueprint
    }

    #[test]
    fn structured_dataset_tables_and_columns_remain_anonymized() {
        let first = structured_test_blueprint(10, 1_000, 400, "zstd");
        let second = structured_test_blueprint(20, 3_000, 1_000, "snappy");
        let blueprint = blueprint_one_table_per_file(
            "parquet",
            vec![
                (PathBuf::from("customer-orders.parquet"), first),
                (PathBuf::from("secret-payroll.parquet"), second),
            ],
        )
        .unwrap();
        assert_eq!(
            blueprint.tables.keys().cloned().collect::<Vec<_>>(),
            vec!["table-001".to_string(), "table-002".to_string()]
        );
        let toml = dbwarp_blueprint_core::blueprint_to_toml(&blueprint).unwrap();
        assert!(!toml.contains("customer-orders"));
        assert!(!toml.contains("secret-payroll"));
    }

    #[test]
    fn structured_dataset_merge_aggregates_provenance_and_ignores_logical_name() {
        let first = structured_test_blueprint(10, 1_000, 400, "zstd");
        let second = structured_test_blueprint(20, 3_000, 1_000, "snappy");
        let merged = blueprint_merge_same_schema(
            "parquet",
            Some("customer_orders"),
            vec![
                (PathBuf::from("private-a.parquet"), first),
                (PathBuf::from("private-b.parquet"), second),
            ],
        )
        .unwrap();
        assert_eq!(merged.tables.len(), 1);
        assert!(merged.tables.contains_key("table-001"));
        assert!(!merged.tables.contains_key("customer_orders"));
        let table = &merged.tables["table-001"];
        assert_eq!(table.rows, 30);
        assert_eq!(table.table_bytes, 4_000);
        assert_eq!(table.storage_bytes, 1_400);
        assert_eq!(table.source_partitions, 2);
        assert_eq!(table.row_group_count, 4);
        assert_eq!(table.source_codec, "snappy,zstd");
        assert_eq!(table.cols["col-1"].length_sample_rows, 30);
        assert_eq!(table.cols["col-1"].null_fraction, Some(0.25));
        let cardinality = table.cols["col-1"].cardinality.as_ref().unwrap();
        assert_eq!(cardinality.sample_rows, 30);
        assert_eq!(cardinality.non_null_rows, 22);
        assert_eq!(cardinality.observed_distinct_count, 7);
        assert_eq!(cardinality.estimated_distinct_count, 10);
        assert_eq!(cardinality.top_value_fraction, 0.2);
        assert!(cardinality.sampled_with_bias);
        assert_eq!(
            cardinality.sample_method,
            "structured-file-dataset-cardinality-bounds-v1"
        );
        assert!(cardinality
            .bias_reason
            .contains("no values or per-value hashes"));
        assert_eq!(
            table.compression.as_ref().unwrap().ratio_storage,
            4_000.0 / 1_400.0
        );
    }

    #[test]
    fn structured_dataset_table_totals_fail_closed_on_overflow() {
        let first = test_blueprint("parquet", "table-001", u64::MAX);
        let second = test_blueprint("parquet", "table-001", 1);
        let error = blueprint_one_table_per_file(
            "parquet",
            vec![
                (PathBuf::from("part-a.parquet"), first),
                (PathBuf::from("part-b.parquet"), second),
            ],
        )
        .expect_err("aggregate row count must not wrap");
        let rendered = format!("{error:#}");
        assert!(rendered.contains("DBP1114E"), "rendered: {rendered}");
        assert!(
            rendered.contains("row count overflow"),
            "rendered: {rendered}"
        );
    }

    #[test]
    fn structured_dataset_member_merge_fails_closed_on_overflow() {
        let first = test_blueprint("parquet", "table-001", u64::MAX);
        let second = test_blueprint("parquet", "table-001", 1);
        let error = blueprint_merge_same_schema(
            "parquet",
            None,
            vec![
                (PathBuf::from("part-a.parquet"), first),
                (PathBuf::from("part-b.parquet"), second),
            ],
        )
        .expect_err("member row count must not saturate");
        let rendered = format!("{error:#}");
        assert!(rendered.contains("DBP1114E"), "rendered: {rendered}");
        assert!(
            rendered.contains("row count overflow"),
            "rendered: {rendered}"
        );
    }

    #[test]
    fn structured_dataset_merge_rejects_same_count_with_different_types() {
        let first = structured_test_blueprint(10, 1_000, 400, "zstd");
        let mut second = structured_test_blueprint(20, 3_000, 1_000, "zstd");
        second
            .tables
            .get_mut("table-001")
            .unwrap()
            .cols
            .get_mut("col-1")
            .unwrap()
            .column_type = "string".to_string();
        let error = blueprint_merge_same_schema(
            "parquet",
            None,
            vec![
                (PathBuf::from("first.parquet"), first),
                (PathBuf::from("second.parquet"), second),
            ],
        )
        .expect_err("different logical types must not merge");
        assert!(error.to_string().contains("DBP1114E"));
    }

    #[test]
    fn describe_secret_source_password_file_does_not_open_file() {
        // Regression guard: building the descriptor must NOT touch the
        // filesystem. We point at a non-existent path and assert that
        // describe_secret_source still returns SecretSource::File.
        let bogus = PathBuf::from("/nonexistent/never/should/exist/pw.txt");
        let mut cli = empty_cli();
        cli.password_file = Some(bogus.clone());
        let s = describe_secret_source(&cli, false);
        match s {
            SecretSource::File { path, .. } => assert_eq!(path, bogus),
            other => panic!("expected SecretSource::File; got {other:?}"),
        }
    }

    #[test]
    fn describe_secret_source_env_uses_var_name_only() {
        let mut cli = empty_cli();
        cli.password_env = Some("DBWARP_BLUEPRINT_TEST_PASS_VAR".to_string());
        let s = describe_secret_source(&cli, false);
        match s {
            SecretSource::Env { var_name } => {
                assert_eq!(var_name, "DBWARP_BLUEPRINT_TEST_PASS_VAR")
            }
            other => panic!("expected SecretSource::Env; got {other:?}"),
        }
    }

    #[test]
    fn describe_secret_source_uri_password_picked_up() {
        let cli = empty_cli();
        let s = describe_secret_source(&cli, true);
        assert!(
            matches!(s, SecretSource::ConnectionString),
            "uri-embedded password should map to ConnectionString"
        );
    }

    #[test]
    fn describe_secret_source_falls_through_to_tty() {
        let cli = empty_cli();
        let s = describe_secret_source(&cli, false);
        assert!(matches!(s, SecretSource::Tty), "no source → tty prompt");
    }

    #[test]
    fn from_toml_cli_does_not_require_connect_when_deck_is_set() {
        let cli = Cli::try_parse_from([
            "dbwarp-blueprint",
            "--from-toml",
            "blueprint.toml",
            "--deck",
            "blueprint.pptx",
        ])
        .expect("--from-toml + --deck should be a complete offline invocation");
        assert!(cli.connect.is_none());
        assert_eq!(cli.from_toml, Some(PathBuf::from("blueprint.toml")));
        assert_eq!(cli.deck, Some(PathBuf::from("blueprint.pptx")));
    }

    #[test]
    fn deck_confidentiality_supports_defaults_and_safe_custom_labels() {
        let cli = Cli::try_parse_from([
            "dbwarp-blueprint",
            "--from-toml",
            "blueprint.toml",
            "--deck",
            "blueprint.pptx",
            "--deck-confidentiality",
            "confidential",
        ])
        .expect("a supported deck confidentiality level should parse");
        assert_eq!(
            cli.deck_confidentiality,
            Some(DeckConfidentiality::Confidential)
        );

        let missing_deck = Cli::try_parse_from([
            "dbwarp-blueprint",
            "--connect",
            "postgresql://app@localhost/db",
            "--deck-confidentiality",
            "internal",
        ])
        .expect_err("deck confidentiality without --deck must fail");
        assert!(missing_deck.to_string().contains("--deck"));

        let custom = Cli::try_parse_from([
            "dbwarp-blueprint",
            "--from-toml",
            "blueprint.toml",
            "--deck",
            "blueprint.pptx",
            "--deck-confidentiality",
            "CLIENT // SENSITIVE",
        ])
        .expect("a safe customer-defined confidentiality label should parse");
        assert_eq!(
            custom.deck_confidentiality,
            Some(DeckConfidentiality::Custom(
                "CLIENT // SENSITIVE".to_string()
            ))
        );

        for unsafe_value in [
            " leading-space",
            "CLIENT\u{202e}LEAK",
            "THIS CUSTOMER CLASSIFICATION LABEL IS TOO LONG FOR THE MEASURED FOOTER ZONE",
        ] {
            let error = Cli::try_parse_from([
                "dbwarp-blueprint",
                "--from-toml",
                "blueprint.toml",
                "--deck",
                "blueprint.pptx",
                "--deck-confidentiality",
                unsafe_value,
            ])
            .expect_err("unsafe custom confidentiality labels must fail");
            assert!(error.to_string().contains("deck confidentiality label"));
        }
    }

    #[test]
    fn exact_length_mode_requires_explicit_consent() {
        let err = Cli::try_parse_from([
            "dbwarp-blueprint",
            "--connect",
            "mysql://app@localhost/db",
            "--preserve-exact-lengths",
        ])
        .expect_err("exact length metadata without --yes must fail");
        assert!(err.to_string().contains("--yes"));

        let cli = Cli::try_parse_from([
            "dbwarp-blueprint",
            "--connect",
            "mysql://app@localhost/db",
            "--preserve-exact-lengths",
            "--yes",
        ])
        .expect("explicitly consented exact length mode should parse");
        assert!(cli.preserve_exact_lengths);

        let mut cli = empty_cli();
        cli.length_fidelity = LengthFidelity::Exact;
        cli.dry_run = true;
        let mut audit = AuditLog::new("blueprint", 1_000);
        let err = run_with_audit(&cli, &mut audit)
            .expect_err("direct exact mode without --yes must fail");
        assert!(err.to_string().contains("DBP1009E"));
    }

    #[test]
    fn balanced_length_fidelity_is_the_default() {
        let cli =
            Cli::try_parse_from(["dbwarp-blueprint", "--connect", "mysql://app@localhost/db"])
                .expect("default MySQL invocation parses");
        assert_eq!(cli.length_fidelity, LengthFidelity::Balanced);
        assert!(!cli.preserve_exact_lengths);
    }

    #[test]
    fn exact_length_mode_rejects_non_mysql_live_capture() {
        let mut cli = empty_cli();
        cli.preserve_exact_lengths = true;
        cli.yes = true;
        cli.dry_run = true;
        let mut audit = AuditLog::new("blueprint", 1_000);
        let err = run_with_audit(&cli, &mut audit)
            .expect_err("exact length mode must not silently no-op for PostgreSQL");
        assert!(err.to_string().contains("DBP1007E"));
    }

    #[test]
    fn from_toml_cli_requires_deck() {
        let err = Cli::try_parse_from(["dbwarp-blueprint", "--from-toml", "blueprint.toml"])
            .expect_err("--from-toml without --deck must fail");
        let msg = err.to_string();
        assert!(
            msg.contains("--deck"),
            "error should explain that --deck is required; got: {msg}"
        );
    }

    #[test]
    fn from_toml_cli_rejects_live_collection_flags() {
        let err = Cli::try_parse_from([
            "dbwarp-blueprint",
            "--from-toml",
            "blueprint.toml",
            "--deck",
            "blueprint.pptx",
            "--password-file",
            "pw.txt",
        ])
        .expect_err("--from-toml must reject live credential flags");
        let msg = err.to_string();
        assert!(
            msg.contains("--from-toml") && msg.contains("--password-file"),
            "error should identify the conflicting flags; got: {msg}"
        );
    }

    #[test]
    fn from_parquet_cli_does_not_require_connect() {
        let cli = Cli::try_parse_from(["dbwarp-blueprint", "--from-parquet", "fixture.parquet"])
            .expect("--from-parquet should be a complete offline Blueprint invocation");
        assert!(cli.connect.is_none());
        assert_eq!(cli.from_parquet, Some(PathBuf::from("fixture.parquet")));
    }

    #[test]
    fn from_avro_cli_does_not_require_connect() {
        let cli = Cli::try_parse_from(["dbwarp-blueprint", "--from-avro", "fixture.avro"])
            .expect("--from-avro should be a complete offline Blueprint invocation");
        assert!(cli.connect.is_none());
        assert_eq!(cli.from_avro, Some(PathBuf::from("fixture.avro")));
    }

    #[test]
    fn from_parquet_cli_rejects_live_collection_flags() {
        let err = Cli::try_parse_from([
            "dbwarp-blueprint",
            "--from-parquet",
            "fixture.parquet",
            "--password-file",
            "pw.txt",
        ])
        .expect_err("--from-parquet must reject live credential flags");
        let msg = err.to_string();
        assert!(
            msg.contains("--from-parquet") && msg.contains("--password-file"),
            "error should identify the conflicting flags; got: {msg}"
        );
    }

    #[test]
    fn from_parquet_cli_accepts_decoded_measure_compression() {
        let cli = Cli::try_parse_from([
            "dbwarp-blueprint",
            "--from-parquet",
            "fixture.parquet",
            "--measure-compression",
            "--yes",
        ])
        .expect("--from-parquet should accept decoded file compression sampling");
        assert!(cli.measure_compression);
        assert!(cli.yes);
        assert_eq!(cli.from_parquet, Some(PathBuf::from("fixture.parquet")));
    }

    #[test]
    fn from_avro_cli_accepts_decoded_measure_compression() {
        let cli = Cli::try_parse_from([
            "dbwarp-blueprint",
            "--from-avro",
            "fixture.avro",
            "--measure-compression",
            "--yes",
        ])
        .expect("--from-avro should accept decoded file compression sampling");
        assert!(cli.measure_compression);
        assert!(cli.yes);
        assert_eq!(cli.from_avro, Some(PathBuf::from("fixture.avro")));
    }

    #[test]
    fn from_file_measure_compression_requires_yes_before_reading() {
        let cli = Cli::try_parse_from([
            "dbwarp-blueprint",
            "--from-parquet",
            "/definitely/not/present.parquet",
            "--measure-compression",
        ])
        .expect("consent for decoded sampling is checked at runtime");
        let mut audit = AuditLog::new("blueprint-from-file", 0);
        let err = run_with_audit(&cli, &mut audit).expect_err("must require explicit consent");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("--from-parquet --measure-compression requires --yes"),
            "got: {msg}"
        );
    }

    #[test]
    fn from_parquet_dry_run_reads_and_writes_nothing() {
        let out_dir = temp_test_dir("dbwarp-blueprint-from-parquet-dry-run-test");
        std::fs::create_dir_all(&out_dir).expect("create temp output dir");
        let out_path = out_dir.join("blueprint.toml");

        let mut cli = empty_cli();
        cli.connect = None;
        cli.from_parquet = Some(PathBuf::from("/nonexistent/fixture.parquet"));
        cli.out = out_path.clone();
        cli.dry_run = true;

        let mut audit = AuditLog::new("blueprint-from-file", 1_000);
        run_with_audit(&cli, &mut audit).expect("offline Parquet Blueprint dry-run should succeed");

        assert!(!out_path.exists(), "dry-run must not write Blueprint TOML");
        assert!(audit.files_read_local.is_empty());
        assert!(audit.files_written_local.is_empty());
        assert_eq!(audit.queries.len(), 0);
        assert_eq!(audit.connection.uri_redacted, "(offline: --from-parquet)");

        let _ = std::fs::remove_dir_all(out_dir);
    }

    #[test]
    fn bundle_extract_table_writes_single_blueprint() {
        let root = temp_test_dir("dbwarp-blueprint-bundle-extract-test");
        std::fs::create_dir_all(root.join("blueprints")).unwrap();
        let blueprint = test_blueprint("postgresql", "table-001", 123);
        std::fs::write(
            root.join("blueprints/erp.blueprint.toml"),
            dbwarp_blueprint_core::blueprint_to_toml(&blueprint).unwrap(),
        )
        .unwrap();

        let mut bundle = dbwarp_blueprint_core::BlueprintBundle {
            schema_version: dbwarp_blueprint_core::BUNDLE_SCHEMA_VERSION,
            kind: dbwarp_blueprint_core::BUNDLE_KIND.to_string(),
            generated_at: "2026-07-07T00:00:00Z".to_string(),
            ..Default::default()
        };
        bundle.sources.insert(
            "erp".to_string(),
            dbwarp_blueprint_core::BundleSource {
                kind: "database".to_string(),
                engine: "postgresql".to_string(),
                blueprint_path: Some("blueprints/erp.blueprint.toml".to_string()),
                table_count: 1,
                row_count: 123,
                tags: vec!["critical".to_string()],
                dataset_relationship: "independent".to_string(),
                dataset_group: "dataset-erp".to_string(),
                dataset_scope_completeness:
                    dbwarp_blueprint_core::blueprint_dataset_scope_completeness(&blueprint)
                        .to_string(),
                ..Default::default()
            },
        );
        bundle.dataset_groups.insert(
            "dataset-erp".to_string(),
            dbwarp_blueprint_core::BundleDatasetGroup {
                relationship: "independent".to_string(),
                members_complete: true,
                members: vec!["erp".to_string()],
            },
        );
        dbwarp_blueprint_core::recompute_bundle_totals(&mut bundle).unwrap();
        let bundle_path = root.join("bundle.toml");
        std::fs::write(
            &bundle_path,
            dbwarp_blueprint_core::blueprint_bundle_to_toml(&bundle).unwrap(),
        )
        .unwrap();

        let out = root.join("out.blueprint.toml");
        let cli = Cli::try_parse_from([
            "dbwarp-blueprint",
            "--bundle-extract",
            bundle_path.to_str().unwrap(),
            "--select",
            "source=erp,table=table-001",
            "--out",
            out.to_str().unwrap(),
        ])
        .unwrap();
        let mut audit = AuditLog::new("blueprint-bundle", 1_000);
        run_with_audit(&cli, &mut audit).unwrap();
        let extracted = dbwarp_blueprint_core::read_blueprint_toml(&out).unwrap();
        assert_eq!(extracted.tables.len(), 1);
        assert_eq!(extracted.totals.row_count, 123);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn bundle_pack_embeds_child_blueprints() {
        let root = temp_test_dir("dbwarp-blueprint-bundle-pack-test");
        std::fs::create_dir_all(root.join("blueprints")).unwrap();
        let blueprint = test_blueprint("mysql", "table-001", 77);
        std::fs::write(
            root.join("blueprints/app.blueprint.toml"),
            dbwarp_blueprint_core::blueprint_to_toml(&blueprint).unwrap(),
        )
        .unwrap();

        let mut bundle = dbwarp_blueprint_core::BlueprintBundle {
            schema_version: dbwarp_blueprint_core::BUNDLE_SCHEMA_VERSION,
            kind: dbwarp_blueprint_core::BUNDLE_KIND.to_string(),
            ..Default::default()
        };
        bundle.sources.insert(
            "app".to_string(),
            dbwarp_blueprint_core::BundleSource {
                kind: "database".to_string(),
                engine: "mysql".to_string(),
                source_kind: "test".to_string(),
                blueprint_path: Some("blueprints/app.blueprint.toml".to_string()),
                table_count: 1,
                row_count: 77,
                table_bytes: blueprint.totals.table_bytes,
                index_bytes: blueprint.totals.index_bytes,
                dataset_relationship: "independent".to_string(),
                dataset_group: "dataset-app".to_string(),
                dataset_scope_completeness:
                    dbwarp_blueprint_core::blueprint_dataset_scope_completeness(&blueprint)
                        .to_string(),
                ..Default::default()
            },
        );
        bundle.dataset_groups.insert(
            "dataset-app".to_string(),
            dbwarp_blueprint_core::BundleDatasetGroup {
                relationship: "independent".to_string(),
                members_complete: true,
                members: vec!["app".to_string()],
            },
        );
        dbwarp_blueprint_core::recompute_bundle_totals(&mut bundle).unwrap();
        std::fs::write(
            root.join("bundle.toml"),
            dbwarp_blueprint_core::blueprint_bundle_to_toml(&bundle).unwrap(),
        )
        .unwrap();

        let out = root.join("packed.toml");
        let cli = Cli::try_parse_from([
            "dbwarp-blueprint",
            "--bundle-pack",
            root.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ])
        .unwrap();
        let mut audit = AuditLog::new("blueprint-bundle", 1_000);
        run_with_audit(&cli, &mut audit).unwrap();
        let packed = dbwarp_blueprint_core::read_blueprint_bundle_toml(&out).unwrap();
        assert!(packed.sources["app"].blueprint.is_some());
        assert_eq!(packed.bundle_totals.row_count, 77);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn from_toml_mode_writes_deck_without_database_activity() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let input = PathBuf::from(format!(
            "{manifest_dir}/tests/fixtures/blueprint_format/pg_expected.toml"
        ));
        let out_dir = temp_test_dir("dbwarp-blueprint-from-toml-test");
        std::fs::create_dir_all(&out_dir).expect("create temp output dir");
        let deck_path = out_dir.join("blueprint.pptx");

        let mut cli = empty_cli();
        cli.connect = None;
        cli.from_toml = Some(input.clone());
        cli.deck = Some(deck_path.clone());

        let mut audit = AuditLog::new("deck-from-toml", 1_000);
        run_with_audit(&cli, &mut audit).expect("offline TOML deck generation should succeed");

        assert!(deck_path.exists(), "deck file should be written");
        assert!(
            std::fs::metadata(&deck_path).unwrap().len() > 0,
            "deck file should not be empty"
        );
        assert_eq!(audit.queries.len(), 0);
        assert_eq!(audit.bytes_read_from_server.catalog_wire_bytes, None);
        assert_eq!(audit.bytes_read_from_server.row_wire_bytes, None);
        assert_eq!(audit.bytes_read_from_server.encoded_sample_bytes, 0);
        assert!(audit
            .files_read_local
            .contains(&input.display().to_string()));
        assert_eq!(audit.files_written_local.len(), 1);
        assert_eq!(audit.files_written_local[0].path, deck_path);
        assert_eq!(audit.connection.uri_redacted, "(offline: --from-toml)");

        let _ = std::fs::remove_dir_all(out_dir);
    }

    #[test]
    fn from_toml_dry_run_reads_and_writes_nothing() {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let input = PathBuf::from(format!(
            "{manifest_dir}/tests/fixtures/blueprint_format/pg_expected.toml"
        ));
        let out_dir = temp_test_dir("dbwarp-blueprint-from-toml-dry-run-test");
        std::fs::create_dir_all(&out_dir).expect("create temp output dir");
        let deck_path = out_dir.join("blueprint.pptx");

        let mut cli = empty_cli();
        cli.connect = None;
        cli.from_toml = Some(input);
        cli.deck = Some(deck_path.clone());
        cli.dry_run = true;

        let mut audit = AuditLog::new("deck-from-toml", 1_000);
        run_with_audit(&cli, &mut audit).expect("offline TOML deck dry-run should succeed");

        assert!(!deck_path.exists(), "dry-run must not write the deck");
        assert!(audit.files_read_local.is_empty());
        assert!(audit.files_written_local.is_empty());
        assert_eq!(audit.queries.len(), 0);
        assert_eq!(audit.connection.uri_redacted, "(offline: --from-toml)");

        let _ = std::fs::remove_dir_all(out_dir);
    }

    /// Regression guard: URI-embedded passwords must be refused before any
    /// I/O. We exercise `run_with_audit` directly with a URI carrying an
    /// embedded password and assert that:
    ///   1. the function returns Err,
    ///   2. the error message names all three accepted alternatives,
    ///   3. the embedded password value does NOT appear in the message.
    #[test]
    fn run_with_audit_refuses_uri_embedded_password() {
        let mut cli = empty_cli();
        // Use a URI that would otherwise be valid, but with an embedded
        // password. Pointing at a non-listening port is fine — we should
        // bail before any connect attempt.
        cli.connect = Some("postgresql://app:supersecret123@127.0.0.1:9/postgres".to_string());
        cli.yes = true;
        let mut audit = AuditLog::new("tier-1", 0);
        let err = run_with_audit(&cli, &mut audit).expect_err("must refuse");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("refusing to use URI-embedded password"),
            "got: {msg}"
        );
        assert!(
            msg.contains("--password-file"),
            "missing --password-file alt; got: {msg}"
        );
        assert!(
            msg.contains("--password-env"),
            "missing --password-env alt; got: {msg}"
        );
        assert!(
            msg.contains("interactive TTY prompt"),
            "missing TTY alt; got: {msg}"
        );
        assert!(
            !msg.contains("supersecret123"),
            "the embedded password value must NEVER appear in the error; got: {msg}"
        );
    }

    /// Snapshot test for the SQL-only fallback's Python normalizer
    /// (`blueprint_format.py`). For each of the three engines, run the
    /// script on a checked-in JSON fixture with a pinned timestamp and a
    /// fixed, mode-protected test-only anonymization key, then assert
    /// byte-equal output to the committed `_expected.toml`. Re-parse that
    /// output through
    /// the canonical Rust `BlueprintFile` deserializer to prove the Python
    /// path produces a file the Rust estimator will accept.
    ///
    /// The script and the Rust binary are two encoders of the same TOML
    /// format. This test is the seam that catches drift in either.
    /// When the snapshot legitimately needs to change, regenerate it:
    ///
    /// ```text
    /// python3 blueprint_format.py --generated-at 2026-04-28T00:00:00Z \
    ///   --anonymization-key-file /path/to/mode-0600-test-key \
    ///   tests/fixtures/blueprint_format/<engine>_input.json \
    ///   > tests/fixtures/blueprint_format/<engine>_expected.toml
    /// ```
    fn blueprint_format_snapshot(engine: &str) {
        use std::process::Command;

        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let script = format!("{manifest_dir}/blueprint_format.py");
        let input = format!("{manifest_dir}/tests/fixtures/blueprint_format/{engine}_input.json");
        let expected_path =
            format!("{manifest_dir}/tests/fixtures/blueprint_format/{engine}_expected.toml");

        let key_dir = temp_test_dir(&format!("blueprint-format-{engine}"));
        std::fs::create_dir_all(&key_dir).expect("create formatter test directory");
        let key_path = key_dir.join("anonymization.key");
        std::fs::write(&key_path, "42".repeat(32)).expect("write formatter test key");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600))
                .expect("protect formatter test key");
        }

        let out = Command::new("python3")
            .args([
                script.as_str(),
                "--generated-at",
                "2026-04-28T00:00:00Z",
                "--anonymization-key-file",
                key_path.to_str().expect("UTF-8 formatter test key path"),
                input.as_str(),
            ])
            .output()
            .expect(
                "python3 must be available to run blueprint_format.py snapshot tests \
                 (the SQL-only fallback path is part of the shipped product)",
            );
        let _ = std::fs::remove_dir_all(&key_dir);

        if !out.status.success() {
            panic!(
                "blueprint_format.py exited {} on {} fixture; stderr:\n{}",
                out.status,
                engine,
                String::from_utf8_lossy(&out.stderr),
            );
        }

        let actual = String::from_utf8(out.stdout).expect("normalizer output must be UTF-8");
        let expected = std::fs::read_to_string(&expected_path)
            .unwrap_or_else(|e| panic!("read {expected_path}: {e}"));

        if actual != expected {
            panic!(
                "blueprint_format.py output drifted from the {engine} snapshot.\n\
                 Either the script or the fixture changed. To accept the new output:\n  \
                 python3 blueprint_format.py --generated-at 2026-04-28T00:00:00Z \\\n  \
                   --anonymization-key-file /path/to/mode-0600-test-key \\\n  \
                   {input} \\\n  \
                   > {expected_path}",
            );
        }

        // Independently confirm the Python's output round-trips through the
        // canonical Rust deserializer — that's what the dbwarp estimator uses,
        // so a parse failure here means the Python path silently produced a
        // file the customer can't actually consume.
        let body = actual.strip_prefix(format::FILE_HEADER).unwrap_or_else(|| {
            panic!(
                "Python output for {engine} fixture must start with the canonical \
                 dbwarp-blueprint v6 file header"
            )
        });
        let parsed: format::BlueprintFile = toml::from_str(body).unwrap_or_else(|e| {
            panic!(
                "Python output for {engine} fixture failed to parse via the canonical \
                 Rust BlueprintFile deserializer: {e}"
            )
        });
        assert!(
            (format::MIN_SCHEMA_VERSION..=format::SCHEMA_VERSION).contains(&parsed.schema_version),
            "{engine} snapshot schema version must remain readable"
        );
        assert!(
            !parsed.tables.is_empty(),
            "{engine} snapshot must yield at least one parsed table"
        );
        assert!(
            actual.contains(
                "# Producer: blueprint_format.py SQL fallback; anonymization key source: customer-key-file"
            ),
            "SQL fallback output must identify its producer and key provenance"
        );
    }

    #[test]
    fn blueprint_format_pg_snapshot() {
        blueprint_format_snapshot("pg");
    }

    #[test]
    fn blueprint_format_mysql_snapshot() {
        blueprint_format_snapshot("mysql");
    }

    #[test]
    fn blueprint_format_mssql_snapshot() {
        blueprint_format_snapshot("mssql");
    }

    #[test]
    fn blueprint_format_key_changes_anonymous_table_ordering() {
        use std::process::Command;

        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let script = format!("{manifest_dir}/blueprint_format.py");
        let input = format!("{manifest_dir}/tests/fixtures/blueprint_format/pg_input.json");
        let key_dir = temp_test_dir("blueprint-format-key-separation");
        std::fs::create_dir_all(&key_dir).expect("create formatter test directory");

        let run_with_key = |name: &str, byte: &str| {
            let key_path = key_dir.join(name);
            std::fs::write(&key_path, byte.repeat(32)).expect("write formatter test key");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600))
                    .expect("protect formatter test key");
            }
            let output = Command::new("python3")
                .args([
                    script.as_str(),
                    "--generated-at",
                    "2026-04-28T00:00:00Z",
                    "--anonymization-key-file",
                    key_path.to_str().expect("UTF-8 formatter test key path"),
                    input.as_str(),
                ])
                .output()
                .expect("run SQL fallback normalizer");
            assert!(
                output.status.success(),
                "normalizer failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            output.stdout
        };

        // These fixed keys deliberately put the two fixture tables in opposite
        // HMAC order. This prevents a regression to public, unkeyed ordering.
        let first = run_with_key("first.key", "42");
        let second = run_with_key("second.key", "24");
        let _ = std::fs::remove_dir_all(&key_dir);
        assert_ne!(first, second, "different secret keys must change anonymous ordering");
    }

    #[cfg(unix)]
    #[test]
    fn blueprint_format_refuses_group_readable_anonymization_key() {
        use std::os::unix::fs::PermissionsExt;
        use std::process::Command;

        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let script = format!("{manifest_dir}/blueprint_format.py");
        let input = format!("{manifest_dir}/tests/fixtures/blueprint_format/pg_input.json");
        let key_dir = temp_test_dir("blueprint-format-unsafe-key");
        std::fs::create_dir_all(&key_dir).expect("create formatter test directory");
        let key_path = key_dir.join("anonymization.key");
        let secret_value = "do-not-print-this-key-material-1";
        assert_eq!(secret_value.len(), 32);
        std::fs::write(&key_path, secret_value).expect("write formatter test key");
        std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o640))
            .expect("make formatter test key group-readable");

        let output = Command::new("python3")
            .args([
                script.as_str(),
                "--anonymization-key-file",
                key_path.to_str().expect("UTF-8 formatter test key path"),
                input.as_str(),
            ])
            .output()
            .expect("run SQL fallback normalizer");
        let _ = std::fs::remove_dir_all(&key_dir);

        assert!(!output.status.success(), "unsafe key mode must be refused");
        assert!(output.stdout.is_empty(), "refusal must not emit a Blueprint");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("group and other read bits must be clear"));
        assert!(
            !stderr.contains(secret_value),
            "anonymization key material must never reach diagnostics"
        );
    }
}
