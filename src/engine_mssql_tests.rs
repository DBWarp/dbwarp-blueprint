#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiberius_uses_blueprints_single_wall_clock_deadline() {
        let mut config = Config::new();
        assert!(config.get_handshake_timeout().is_some());
        assert!(config.get_command_timeout().is_some());

        preserve_blueprint_timeout_policy(&mut config);

        assert_eq!(config.get_handshake_timeout(), None);
        assert_eq!(config.get_command_timeout(), None);
    }

    #[test]
    fn memory_optimized_storage_is_not_mislabeled_as_a_disk_heap() {
        assert_eq!(mssql_storage_organization(false, true, false), "unknown");
        assert_eq!(mssql_storage_organization(false, false, false), "heap");
        assert_eq!(mssql_storage_organization(false, false, true), "clustered");
        assert_eq!(mssql_storage_organization(true, false, false), "external");
    }

    #[test]
    fn numeric_contract_handles_parameterized_decimal_and_binary_float() {
        assert_eq!(
            mssql_numeric_contract("decimal(18,4)", 18, 4),
            (
                "fixed-decimal".to_string(),
                Some(18),
                Some(4),
                "decimal".to_string()
            )
        );
        assert_eq!(
            mssql_numeric_contract("float", 53, 0),
            (
                "binary-float".to_string(),
                Some(53),
                None,
                "binary".to_string()
            )
        );
        assert_eq!(
            mssql_numeric_contract("text", 0, 0).0,
            "not-applicable"
        );
    }

    #[test]
    fn unreadable_engine_edition_prevents_full_environment_visibility() {
        let mut environment = crate::environment::database_source_environment(
            Some(8),
            "operating-system-visible",
            Some(16 * 1024 * 1024 * 1024),
            "operating-system-visible",
            "connected-instance",
            vec!["sqlserver-os-sys-info".to_string()],
            Vec::new(),
        );
        assert_eq!(environment.capacity_visibility, "full");

        qualify_mssql_engine_edition(&mut environment, None);

        assert_eq!(environment.capacity_visibility, "partial");
        assert_eq!(
            environment.catalogs_unreadable,
            ["sqlserver-engine-edition"]
        );
    }

    #[test]
    fn os_capacity_is_never_read_for_managed_or_unclassified_editions() {
        for edition in [Some(5), Some(6), Some(8), Some(11), Some(99), None] {
            assert!(!mssql_os_capacity_probe_allowed(edition), "edition {edition:?}");
        }
        for edition in 1..=4 {
            assert!(mssql_os_capacity_probe_allowed(Some(edition)));
        }
    }

    #[test]
    fn os_capacity_is_requested_only_with_enhanced_artifact_detail() {
        assert!(!mssql_source_environment_requested(ArtifactDetail::None));
        assert!(!mssql_source_environment_requested(ArtifactDetail::Summary));
        assert!(mssql_source_environment_requested(ArtifactDetail::Graph));
        assert!(mssql_source_environment_requested(ArtifactDetail::Analyzed));
    }

    #[test]
    fn unrequested_capacity_retains_free_edition_evidence() {
        let self_managed = mssql_source_environment_not_requested(Some(3));
        assert_eq!(self_managed.evidence_origin, "database-endpoint");
        assert_eq!(self_managed.capacity_visibility, "not-requested");
        assert_eq!(self_managed.hosting_model, "self-managed");
        assert_eq!(
            self_managed.catalogs_read,
            ["sqlserver-engine-edition".to_string()]
        );
        assert!(self_managed.catalogs_unreadable.is_empty());

        let managed = mssql_source_environment_not_requested(Some(5));
        assert_eq!(managed.evidence_origin, "database-endpoint");
        assert_eq!(managed.capacity_visibility, "not-requested");
        assert_eq!(managed.hosting_model, "managed-service");
        assert_eq!(managed.infrastructure_location, "cloud");
        assert_eq!(
            managed.catalogs_read,
            ["sqlserver-engine-edition".to_string()]
        );

        let unreadable = mssql_source_environment_not_requested(None);
        assert_eq!(unreadable.evidence_origin, "database-endpoint");
        assert_eq!(unreadable.capacity_visibility, "not-requested");
        assert_eq!(
            unreadable.catalogs_unreadable,
            ["sqlserver-engine-edition".to_string()]
        );
    }

    #[test]
    fn managed_capacity_environment_remains_unknown() {
        let mut environment = crate::environment::database_source_environment(
            None,
            "unknown",
            None,
            "unknown",
            "unknown",
            Vec::new(),
            Vec::new(),
        );
        qualify_mssql_engine_edition(&mut environment, Some(5));
        assert_eq!(environment.hosting_model, "managed-service");
        assert_eq!(environment.capacity_visibility, "unknown");
        assert_eq!(environment.cpu_capacity_band, "unknown");
        assert_eq!(environment.memory_capacity_band, "unknown");
    }

    #[test]
    fn sampling_never_treats_unmeasured_storage_as_proven_empty() {
        assert!(mssql_sampling_allowed(false, false, true, false));
        assert!(!mssql_sampling_allowed(true, false, true, false));
        assert!(!mssql_sampling_allowed(false, true, true, false));
        assert!(!mssql_sampling_allowed(false, false, false, false));
        assert!(!mssql_sampling_allowed(false, false, true, true));
    }

    #[test]
    fn row_security_visibility_is_decided_per_table() {
        assert!(mssql_sampling_allowed(false, false, true, false));
        assert!(!mssql_sampling_allowed(false, false, false, false));

        let source = include_str!("engine_mssql.rs");
        assert!(source.contains("AS security_policy_catalog_complete"));
        assert!(source.contains("t.security_policy_catalog_complete"));
        assert!(source.contains("'ALTER ANY SECURITY POLICY'"));
        assert!(!source.contains("'OBJECT', 'SELECT'"));
        assert!(!source.contains("HAS_PERMS_BY_NAME(DB_NAME(), 'DATABASE', 'VIEW DEFINITION') (security-policy visibility)"));
        assert!(source.contains("row-security-visibility-unknown"));
        assert!(source.contains("row-security-filter-active"));
        assert!(source.contains("DBP1424W"));

        let mut audit = AuditLog::default();
        warn_security_policy_visibility_unavailable("table-007", &mut audit);
        assert_eq!(audit.warnings.len(), 1);
        assert!(audit.warnings[0].starts_with("DBP1424W "));
        assert!(audit.warnings[0].contains("table-007"));
    }

    #[test]
    fn complete_reads_cannot_erase_sqlserver_external_or_memory_optimized_gaps() {
        let mut scope = classify_mssql_topology(&ordinary_mssql_evidence()).dataset_scope;
        assert!(!mssql_row_scope_intrinsically_incomplete(&scope));
        scope.limitations.push("external-data-unmeasured".to_string());
        assert!(mssql_row_scope_intrinsically_incomplete(&scope));
        scope.limitations.clear();
        scope
            .limitations
            .push("memory-optimized-data-unmeasured".to_string());
        assert!(mssql_row_scope_intrinsically_incomplete(&scope));
    }

    #[test]
    fn missing_table_counters_lower_dataset_completeness() {
        let mut dataset = format::DatasetScope {
            row_count_completeness: "complete".to_string(),
            size_completeness: "complete".to_string(),
            ..format::DatasetScope::default()
        };
        let tables = vec![TableRow {
            object_id: 1,
            schema_name: "dbo".to_string(),
            table_name: "orders".to_string(),
            row_count: None,
            table_bytes: Some(8192),
            index_bytes: None,
            has_clustered_index: false,
            is_external: false,
            is_memory_optimized: false,
            temporal_type: 0,
            history_table_id: None,
            temporal_history_outside_scope: false,
            temporal_history_visibility_unknown: false,
            is_node: false,
            is_edge: false,
            partitioned: false,
            partition_count: 0,
            partition_rows_max: None,
            check_count: 0,
            has_security_filter: false,
            security_policy_catalog_complete: true,
        }];

        apply_table_counter_completeness(&mut dataset, &tables);

        assert_eq!(dataset.row_count_completeness, "incomplete");
        assert_eq!(dataset.size_completeness, "incomplete");
        assert_eq!(dataset.limitations, vec!["statistics-partial"]);
    }

    #[test]
    fn security_filter_prevents_a_short_sample_becoming_a_complete_read() {
        assert!(mssql_complete_row_read(49, 50, false));
        assert!(!mssql_complete_row_read(49, 50, true));
        assert!(!mssql_complete_row_read(50, 50, false));
        let source = include_str!("engine_mssql.rs");
        assert!(source.contains("FROM sys.security_predicates predicate"));
        assert!(source.contains("policy.is_enabled = 1"));
    }

    fn ordinary_mssql_evidence() -> MssqlTopologyEvidence {
        MssqlTopologyEvidence {
            hadr_capability_readable: true,
            hadr_enabled: Some(false),
            external_table_catalog_readable: true,
            ..MssqlTopologyEvidence::default()
        }
    }

    fn availability_group_evidence(local_role: &'static str) -> MssqlTopologyEvidence {
        let (primary_count, secondary_count, member_count) = if local_role == "primary" {
            (1, 2, 3)
        } else {
            (0, 1, 1)
        };
        MssqlTopologyEvidence {
            hadr_capability_readable: true,
            hadr_enabled: Some(true),
            database_replica_catalog_attempted: true,
            database_replica_catalog_readable: true,
            database_participates: true,
            local_role: Some(local_role),
            availability_replica_catalog_attempted: true,
            availability_replica_catalog_readable: true,
            visible_member_count: member_count,
            visible_primary_count: primary_count,
            visible_secondary_count: secondary_count,
            external_table_catalog_readable: true,
            ..MssqlTopologyEvidence::default()
        }
    }

    #[test]
    fn ordinary_sqlserver_keeps_data_scope_complete_without_claiming_single_node() {
        let assessment = classify_mssql_topology(&ordinary_mssql_evidence());
        assert_eq!(assessment.topology.deployment, "unknown");
        assert_eq!(assessment.topology.local_role, "unknown");
        assert_eq!(assessment.topology.visibility, "partial");
        assert_eq!(assessment.dataset_scope.layout, "full-copy");
        assert_eq!(
            assessment.dataset_scope.table_inventory_completeness,
            "complete"
        );
        assert_eq!(assessment.dataset_scope.row_count_completeness, "complete");
        assert_eq!(assessment.dataset_scope.size_completeness, "complete");
        assert!(assessment
            .dataset_scope
            .limitations
            .contains(&"topology-visibility-partial".to_string()));
    }

    #[test]
    fn partition_counter_preserves_known_positive_sub_bucket_tables() {
        assert_eq!(mssql_partition_counter_evidence(0), (0, "engine-counter"));
        assert_eq!(
            mssql_partition_counter_evidence(5),
            (100, "engine-estimate")
        );
        assert_eq!(
            mssql_partition_counter_evidence(50),
            (100, "engine-counter")
        );
        assert_eq!(
            mssql_partition_counter_evidence(150),
            (200, "engine-counter")
        );

        let estate = (0..50)
            .map(|_| mssql_partition_counter_evidence(5))
            .collect::<Vec<_>>();
        assert_eq!(estate.iter().map(|item| item.0).sum::<u64>(), 5_000);
        assert!(estate.iter().all(|item| item.1 == "engine-estimate"));
    }

    fn sqlserver_statistics_fixture(row_count_method: &str) -> BlueprintFile {
        let tables = ["table-001", "table-002"]
            .into_iter()
            .map(|table_id| {
                (
                    table_id.to_string(),
                    BlueprintTable {
                        object_kind: "ordinary-table".to_string(),
                        storage_organization: "heap".to_string(),
                        partitioning: "none".to_string(),
                        segment_state: "created".to_string(),
                        statistics: Some(
                            dbwarp_blueprint_core::TableStatisticsEvidence::unknown_database(
                                "sqlserver-table-statistics",
                                "sqlserver-partition-stats",
                            ),
                        ),
                        ..BlueprintTable::default()
                    },
                )
            })
            .collect();
        BlueprintFile {
            schema_version: SCHEMA_VERSION,
            engine: "sqlserver".to_string(),
            dataset_scope: Some(dbwarp_blueprint_core::DatasetScope {
                table_inventory_completeness: "complete".to_string(),
                row_count_completeness: "complete".to_string(),
                size_completeness: "complete".to_string(),
                row_count_method: row_count_method.to_string(),
                size_method: "sqlserver-partition-pages".to_string(),
                ..dbwarp_blueprint_core::DatasetScope::default()
            }),
            tables,
            ..BlueprintFile::default()
        }
    }

    #[test]
    fn qualification_wires_sub_bucket_ids_to_per_table_and_dataset_evidence() {
        let availability = BTreeMap::from([
            ("table-001".to_string(), (true, true)),
            ("table-002".to_string(), (true, true)),
        ]);
        let sub_bucket = BTreeSet::from(["table-001".to_string()]);
        let mut blueprint = sqlserver_statistics_fixture("sqlserver-partition-counter");

        qualify_v7_statistics(
            &mut blueprint,
            &availability,
            &BTreeSet::new(),
            &sub_bucket,
        )
        .unwrap();

        assert_eq!(
            blueprint.tables["table-001"]
                .statistics
                .as_ref()
                .unwrap()
                .row_count_quality,
            "engine-estimate"
        );
        assert_eq!(
            blueprint.tables["table-002"]
                .statistics
                .as_ref()
                .unwrap()
                .row_count_quality,
            "engine-counter"
        );
        assert!(blueprint
            .dataset_scope
            .as_ref()
            .unwrap()
            .limitations
            .contains(&"row-counts-statistical".to_string()));

        let mut unavailable = sqlserver_statistics_fixture("unknown");
        qualify_v7_statistics(
            &mut unavailable,
            &availability,
            &BTreeSet::new(),
            &sub_bucket,
        )
        .unwrap();
        assert_eq!(
            unavailable.tables["table-001"]
                .statistics
                .as_ref()
                .unwrap()
                .row_count_quality,
            "unavailable"
        );
        assert!(!unavailable
            .dataset_scope
            .as_ref()
            .unwrap()
            .limitations
            .contains(&"row-counts-statistical".to_string()));
    }

    #[test]
    fn sqlserver_primary_has_full_availability_group_visibility() {
        let assessment = classify_mssql_topology(&availability_group_evidence("primary"));
        assert_eq!(assessment.topology.deployment, "replicated");
        assert_eq!(assessment.topology.local_role, "primary");
        assert_eq!(assessment.topology.visibility, "full");
        assert_eq!(assessment.topology.member_count, 3);
        assert_eq!(assessment.topology.role_counts.get("primary"), Some(&1));
        assert_eq!(assessment.topology.role_counts.get("secondary"), Some(&2));
        assert!(assessment
            .topology
            .features
            .contains(&"sqlserver-availability-group".to_string()));
        assert!(assessment.dataset_scope.limitations.is_empty());
    }

    #[test]
    fn sqlserver_secondary_does_not_claim_full_group_visibility() {
        let assessment = classify_mssql_topology(&availability_group_evidence("secondary"));
        assert_eq!(assessment.topology.deployment, "replicated");
        assert_eq!(assessment.topology.local_role, "secondary");
        assert_eq!(assessment.topology.visibility, "partial");
        assert_eq!(assessment.topology.member_count, 1);
        assert!(assessment
            .dataset_scope
            .limitations
            .contains(&"replica-membership-unresolved".to_string()));
        assert_eq!(assessment.dataset_scope.size_completeness, "complete");
    }

    #[test]
    fn unreadable_hadr_state_remains_unknown_without_downgrading_local_pages() {
        let evidence = MssqlTopologyEvidence {
            hadr_capability_readable: true,
            hadr_enabled: Some(true),
            database_replica_catalog_attempted: true,
            external_table_catalog_readable: true,
            ..MssqlTopologyEvidence::default()
        };
        let assessment = classify_mssql_topology(&evidence);
        assert_eq!(assessment.topology.deployment, "unknown");
        assert!(assessment
            .topology
            .catalogs_unreadable
            .contains(&"sqlserver-database-replica-states".to_string()));
        assert_eq!(assessment.dataset_scope.row_count_completeness, "complete");
        assert_eq!(assessment.dataset_scope.size_completeness, "complete");
    }

    #[test]
    fn visible_external_tables_keep_inventory_complete_but_data_incomplete() {
        let mut evidence = ordinary_mssql_evidence();
        evidence.external_table_count = 2;
        let assessment = classify_mssql_topology(&evidence);
        assert_eq!(
            assessment.dataset_scope.table_inventory_completeness,
            "complete"
        );
        assert_eq!(
            assessment.dataset_scope.row_count_completeness,
            "incomplete"
        );
        assert_eq!(assessment.dataset_scope.size_completeness, "incomplete");
        assert!(assessment
            .dataset_scope
            .limitations
            .contains(&"external-data-unmeasured".to_string()));
    }

    #[test]
    fn unreadable_external_table_catalog_cannot_claim_complete_scope() {
        let mut evidence = ordinary_mssql_evidence();
        evidence.external_table_catalog_readable = false;
        let assessment = classify_mssql_topology(&evidence);
        assert_eq!(assessment.dataset_scope.size_completeness, "incomplete");
        assert!(assessment
            .dataset_scope
            .limitations
            .contains(&"external-table-visibility-unknown".to_string()));
    }

    #[test]
    fn dependency_table_detection_accepts_space_padded_type_codes() {
        assert!(mssql_dependency_target_is_table("U"));
        assert!(mssql_dependency_target_is_table("U "));
        assert!(!mssql_dependency_target_is_table("V "));
    }

    #[test]
    fn artifact_kind_accepts_space_padded_sys_object_codes() {
        assert_eq!(mssql_artifact_kind("V "), Some(("view", "ordinary")));
        assert_eq!(
            mssql_artifact_kind("P "),
            Some(("procedure", "stored_procedure"))
        );
        assert_eq!(
            mssql_artifact_kind("D "),
            Some(("default", "default_constraint"))
        );
        assert_eq!(
            mssql_artifact_kind("C "),
            Some(("default", "check_constraint"))
        );
        assert_eq!(
            mssql_artifact_kind("SN"),
            Some(("synonym", "database_synonym"))
        );
    }

    #[test]
    fn declarative_catalog_objects_do_not_inherit_module_semantics() {
        for type_code in ["C", "D", "SN", "SO"] {
            assert!(!mssql_has_module_security_context(type_code));
        }
        assert_eq!(
            mssql_definition_visibility("SN", true, false, false, false),
            "not_applicable"
        );
        assert_eq!(
            mssql_definition_visibility("SO", true, false, false, false),
            "not_applicable"
        );
        assert_eq!(
            mssql_definition_visibility("C", true, false, true, false),
            "available"
        );
        assert_eq!(
            mssql_definition_visibility("D", true, false, true, false),
            "available"
        );
        assert_eq!(
            mssql_definition_visibility("P", true, false, false, true),
            "encrypted"
        );
        assert_eq!(
            mssql_definition_span("C ", false),
            RawDefinitionSpan::ExecutableBody
        );
        assert_eq!(
            mssql_definition_span("D", false),
            RawDefinitionSpan::ExecutableBody
        );
        assert_eq!(
            mssql_definition_span("PC", true),
            RawDefinitionSpan::NotApplicable
        );
        assert_eq!(
            mssql_definition_span("P ", false),
            RawDefinitionSpan::SqlServerModule
        );
        assert!(mssql_has_module_security_context("P "));

        let source = include_str!("engine_mssql_artifacts.rs");
        assert!(source.contains("LEFT JOIN sys.check_constraints chk"));
        assert!(source.contains("WHEN o.type = 'C' THEN chk.definition"));
        assert!(source.contains("WHEN o.type = 'D' THEN def.definition"));
    }

    #[test]
    fn numeric_formatting() {
        use tiberius::numeric::Numeric;
        // 123.45: value=12345, scale=2
        assert_eq!(
            format_tiberius_numeric(&Numeric::new_with_scale(12345, 2)),
            "123.45"
        );
        // 0.001234: value=1234, scale=6
        assert_eq!(
            format_tiberius_numeric(&Numeric::new_with_scale(1234, 6)),
            "0.001234"
        );
        // 0: zero scale
        assert_eq!(format_tiberius_numeric(&Numeric::new_with_scale(0, 0)), "0");
        // Negative: -12.34
        assert_eq!(
            format_tiberius_numeric(&Numeric::new_with_scale(-1234, 2)),
            "-12.34"
        );
        // Whole number with scale 0
        assert_eq!(
            format_tiberius_numeric(&Numeric::new_with_scale(42, 0)),
            "42"
        );
    }

    #[test]
    fn sampled_variable_lengths_use_relative_privacy_buckets() {
        assert_eq!(
            sampled_mssql_column_length_stats(vec![9, 10, 11]),
            Some((10, 11))
        );
        assert_eq!(sampled_mssql_column_length_stats(Vec::new()), None);
    }

    #[test]
    fn variable_length_detection_excludes_fixed_width_types() {
        for native_type in ["varchar", "nvarchar", "varbinary", "text", "ntext", "image"] {
            assert!(is_variable_length_mssql(native_type), "{native_type}");
        }
        for native_type in ["char", "nchar", "binary", "int", "datetime2", "xml"] {
            assert!(!is_variable_length_mssql(native_type), "{native_type}");
        }
    }

    #[test]
    fn parse_uri_minimal() {
        let (p, pw) = MssqlConnectParams::parse("sqlserver://sa@db/master").unwrap();
        assert_eq!(p.host, "db");
        assert_eq!(p.port, 1433);
        assert_eq!(p.database, "master");
        assert_eq!(p.user, "sa");
        assert_eq!(pw, None);
    }

    #[test]
    fn parse_uri_comma_port() {
        let (p, _) = MssqlConnectParams::parse("sqlserver://sa@127.0.0.1,11433/master").unwrap();
        assert_eq!(p.host, "127.0.0.1");
        assert_eq!(p.port, 11433);
    }

    #[test]
    fn parse_uri_colon_port() {
        let (p, _) = MssqlConnectParams::parse("mssql://sa@db:1433/master").unwrap();
        assert_eq!(p.port, 1433);
    }

    #[test]
    fn parse_uri_full() {
        let (p, pw) =
            MssqlConnectParams::parse("sqlserver://app:hunter2@db.example,11433/payments").unwrap();
        assert_eq!(p.port, 11433);
        assert_eq!(pw.as_deref(), Some("hunter2"));
        assert!(!p.redacted_uri.contains("hunter2"));
    }

    #[test]
    fn parse_uri_tds_alias() {
        assert!(MssqlConnectParams::parse("tds://sa@h/d").is_ok());
    }

    // Regression guard: IPv6 with both `,port` and `:port` forms.

    #[test]
    fn parse_ipv6_comma_port() {
        let (p, _) = MssqlConnectParams::parse("sqlserver://sa@[::1],1433/master").unwrap();
        assert_eq!(p.host, "::1");
        assert_eq!(p.port, 1433);
    }

    #[test]
    fn parse_ipv6_colon_port() {
        let (p, _) = MssqlConnectParams::parse("sqlserver://sa@[::1]:1433/master").unwrap();
        assert_eq!(p.host, "::1");
        assert_eq!(p.port, 1433);
    }

    #[test]
    fn parse_ipv6_default_port() {
        let (p, _) = MssqlConnectParams::parse("sqlserver://sa@[::1]/master").unwrap();
        assert_eq!(p.host, "::1");
        assert_eq!(p.port, 1433);
    }

    #[test]
    fn format_mssql_type_examples() {
        assert_eq!(format_mssql_type("varchar", 255, 0, 0), "text");
        assert_eq!(format_mssql_type("varchar", -1, 0, 0), "text");
        assert_eq!(format_mssql_type("nvarchar", 510, 0, 0), "text");
        assert_eq!(format_mssql_type("decimal", 0, 18, 4), "decimal(18,4)");
        assert_eq!(format_mssql_type("datetime2", 0, 0, 7), "timestamp");
        assert_eq!(format_mssql_type("bit", 0, 0, 0), "boolean");
        assert_eq!(
            format_mssql_type("customer_money_type", 0, 0, 0),
            "user-defined"
        );
    }

    #[test]
    fn length_metadata_distinguishes_bounded_unicode_and_max_lobs() {
        assert_eq!(
            mssql_length_metadata("nvarchar", 510),
            (255, 510, String::new())
        );
        assert_eq!(
            mssql_length_metadata("nvarchar", -1),
            (0, 0, "unbounded-lob".to_string())
        );
        assert_eq!(
            mssql_length_metadata("image", 16),
            (0, 0, "unbounded-lob".to_string())
        );
    }

    #[test]
    fn mssql_charset_preserves_unicode_and_varchar_code_pages() {
        assert_eq!(mssql_charset("nvarchar", 1252), "utf-16le");
        assert_eq!(mssql_charset("varchar", 65001), "utf-8");
        assert_eq!(mssql_charset("varchar", 1252), "windows-1252");
        assert_eq!(mssql_charset("varchar", 932), "code-page-932");
        assert_eq!(mssql_charset("varbinary", 0), "");
    }

    fn sample_column(name: &str, native_type: &str, declared_max_bytes: u64) -> ColumnRow {
        ColumnRow {
            schema_name: "dbo".to_string(),
            table_name: "sample".to_string(),
            col_name: name.to_string(),
            ordinal: 1,
            col_type: if matches!(native_type, "binary" | "varbinary" | "image") {
                "binary".to_string()
            } else {
                "text".to_string()
            },
            native_type: native_type.to_string(),
            is_nullable: true,
            declared_max_chars: declared_max_bytes / 2,
            declared_max_bytes,
            numeric_precision: 0,
            numeric_scale: 0,
            datetime_precision: 0,
            charset: String::new(),
            collation: String::new(),
            source_semantics: String::new(),
        }
    }

    #[test]
    fn mssql_projection_budget_floors_prefixes_on_pathologically_wide_text_schemas() {
        let columns = (0..1_600)
            .map(|index| sample_column(format!("c{index}").as_str(), "nvarchar", 0))
            .collect::<Vec<_>>();
        let (rows, limits) = mssql_sample_projection_budget(4_096, &columns).unwrap();
        // The regime under test: the per-column budget is below one UTF-16LE
        // supplementary character, where an unfloored prefix becomes
        // LEFT(col, 0) and silently samples always-empty text. Post-floor,
        // every cell carries exactly the one-character byte cover and the
        // re-clamped row count keeps the floored row cost inside the probe
        // ceiling.
        assert!(limits.iter().all(|limit| limit.byte_limit == 4));
        assert!(limits.iter().all(|limit| limit.char_limit == 1));
        let row_bytes = limits
            .iter()
            .map(|limit| {
                limit.byte_limit as u64 + dbwarp_blueprint_core::TRANSFER_PROBE_CELL_OVERHEAD_BYTES
            })
            .sum::<u64>();
        assert!(rows >= 1);
        assert!(
            rows.saturating_mul(row_bytes)
                <= dbwarp_blueprint_core::TRANSFER_PROBE_MAX_SAMPLE_BYTES as u64
        );
    }

    #[test]
    fn mssql_initial_sample_plan_bounds_driver_rows_and_payload() {
        let columns = (0..16)
            .map(|index| sample_column(format!("c{index}").as_str(), "nvarchar", 1024))
            .collect::<Vec<_>>();
        let (rows, limits) = mssql_sample_projection_budget(131_072, &columns).unwrap();
        assert_eq!(rows, MSSQL_INITIAL_SAMPLE_MAX_ROWS);
        assert_eq!(limits.len(), columns.len());
        assert!(limits.iter().all(|limit| limit.byte_limit > 0));
        assert!(limits.iter().all(|limit| limit.char_limit > 0));
        let projected_payload = rows.saturating_mul(
            limits
                .iter()
                .map(|limit| limit.byte_limit as u64)
                .sum::<u64>(),
        );
        assert!(
            projected_payload
                <= (dbwarp_blueprint_core::TRANSFER_PROBE_MAX_SAMPLE_BYTES
                    / MSSQL_INITIAL_SAMPLE_PAYLOAD_DIVISOR) as u64
        );
    }

    #[test]
    fn mssql_sample_projection_carries_sampled_and_original_byte_lengths() {
        let columns = vec![
            sample_column("unicode]text", "nvarchar", 1024),
            sample_column("payload", "varbinary", 2048),
        ];
        let limits = vec![
            MssqlSampleProjectionLimit {
                byte_limit: 128,
                char_limit: 32,
            },
            MssqlSampleProjectionLimit {
                byte_limit: 256,
                char_limit: 64,
            },
        ];
        let projection = mssql_sample_projection(&columns, &limits);
        assert!(projection.contains("LEFT([unicode]]text], 32)"));
        assert!(projection.contains("SUBSTRING(CONVERT(varbinary(max), [payload]), 1, 256)"));
        assert_eq!(projection.matches("DATALENGTH(").count(), 4);
    }

    #[test]
    fn mssql_adaptive_projection_caps_single_and_combined_oversized_values() {
        let ceiling = dbwarp_blueprint_core::TRANSFER_PROBE_MAX_SAMPLE_BYTES as u64;
        for native in ["varbinary", "nvarchar", "varchar", "xml"] {
            for width in [1, ceiling - 4096, ceiling, ceiling + 4096, u64::MAX] {
                for count in [1, 2, 32] {
                    let columns = vec![sample_column("payload", native, 0); count];
                    for requested in [1, 32, u64::MAX] {
                        let (rows, limits) = mssql_adaptive_projection_budget(
                            &columns,
                            &vec![width; count],
                            requested,
                        )
                        .unwrap();
                        let row_bytes = limits
                            .iter()
                            .map(|limit| {
                                let payload = if native == "varbinary" {
                                    limit.byte_limit
                                } else {
                                    limit.char_limit * 4
                                };
                                assert!(payload <= limit.byte_limit);
                                payload as u64
                                    + dbwarp_blueprint_core::TRANSFER_PROBE_CELL_OVERHEAD_BYTES
                            })
                            .sum::<u64>();
                        assert!(rows > 0 && rows <= requested);
                        assert!(rows * row_bytes <= ceiling);
                    }
                }
            }
        }
    }

    #[test]
    fn mssql_text_retry_bounds_supplementary_characters_in_changed_rows() {
        let columns = [sample_column("payload", "nvarchar", 0)];
        let (rows, limits) = mssql_adaptive_projection_budget(&columns, &[1024], 32).unwrap();
        assert_eq!(rows, 32);
        assert_eq!(limits[0].char_limit, 1024);
        assert_eq!(limits[0].byte_limit, 4096);
    }
}
