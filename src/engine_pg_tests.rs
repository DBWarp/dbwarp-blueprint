#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpopulated_materialized_views_are_not_sampled() {
        assert!(!pg_sampling_allowed("m", false, false));
        assert!(pg_sampling_allowed("m", true, false));
        assert!(!pg_sampling_allowed("p", true, false));
        assert!(!pg_sampling_allowed("f", true, false));
        assert!(pg_sampling_allowed("r", true, false));
        assert!(!pg_sampling_allowed("r", true, true));
    }

    #[test]
    fn numeric_contract_distinguishes_numeric_and_non_numeric_columns() {
        assert_eq!(
            pg_blueprint_numeric_contract("numeric"),
            (
                "unconstrained-decimal".to_string(),
                None,
                None,
                "decimal".to_string()
            )
        );
        assert_eq!(
            pg_blueprint_numeric_contract("double precision"),
            ("binary-float".to_string(), None, None, "binary".to_string())
        );
        // The declared spelling is the KNOWN case: fixed-decimal with its
        // facets parsed, where an earlier mapping said "unknown" and never
        // populated precision or scale at all.
        assert_eq!(
            pg_blueprint_numeric_contract("numeric(10,2)"),
            (
                "fixed-decimal".to_string(),
                Some(10),
                Some(2),
                "decimal".to_string()
            )
        );
        assert_eq!(
            pg_blueprint_numeric_contract("numeric(7)"),
            (
                "fixed-decimal".to_string(),
                Some(7),
                Some(0),
                "decimal".to_string()
            )
        );
        assert_eq!(
            pg_blueprint_numeric_contract("numeric(3,-2)"),
            (
                "fixed-decimal".to_string(),
                Some(3),
                Some(-2),
                "decimal".to_string()
            )
        );
        assert_eq!(
            pg_blueprint_numeric_contract("text").0,
            "not-applicable"
        );
    }

    fn ordinary_pg_evidence(in_recovery: bool, peer_count: u64) -> PgTopologyEvidence {
        PgTopologyEvidence {
            base_readable: true,
            in_recovery: Some(in_recovery),
            citus_installed: Some(false),
            replication_catalog_readable: true,
            direct_peer_count: Some(peer_count),
            ..PgTopologyEvidence::default()
        }
    }

    fn citus_coordinator_evidence() -> PgTopologyEvidence {
        PgTopologyEvidence {
            base_readable: true,
            in_recovery: Some(false),
            citus_installed: Some(true),
            replication_catalog_readable: true,
            direct_peer_count: Some(0),
            citus_metadata_readable: true,
            distributed_table_count: Some(3),
            local_group_id: Some(0),
            registered_member_count: Some(3),
            coordinator_count: Some(1),
            worker_count: Some(2),
            local_member_registered: true,
        }
    }

    #[test]
    fn ordinary_postgres_keeps_full_copy_sizing_without_claiming_full_topology() {
        let assessment = classify_pg_topology(&ordinary_pg_evidence(false, 0));
        assert_eq!(assessment.table_size_mode, PgTableSizeMode::Local);
        assert_eq!(assessment.topology.deployment, "unknown");
        assert_eq!(assessment.topology.local_role, "primary");
        assert_eq!(assessment.topology.visibility, "partial");
        assert_eq!(assessment.topology.member_count, 1);
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
            .contains(&"row-counts-statistical".to_string()));
    }

    #[test]
    fn complete_reads_cannot_erase_external_or_distributed_row_gaps() {
        let mut scope = classify_pg_topology(&ordinary_pg_evidence(false, 0)).dataset_scope;
        assert!(!pg_row_scope_intrinsically_incomplete(&scope));
        scope.limitations.push("external-data-unmeasured".to_string());
        assert!(pg_row_scope_intrinsically_incomplete(&scope));
        scope.limitations.clear();
        scope.limitations.push("local-member-only".to_string());
        assert!(pg_row_scope_intrinsically_incomplete(&scope));
        scope.limitations.clear();
        scope
            .limitations
            .push("distributed-row-count-unavailable".to_string());
        assert!(pg_row_scope_intrinsically_incomplete(&scope));
        scope.limitations.clear();
        scope
            .limitations
            .push("row-count-evidence-incomplete".to_string());
        assert!(!pg_row_scope_intrinsically_incomplete(&scope));
        scope
            .limitations
            .push("logical-partition-root-unmeasured".to_string());
        assert!(pg_row_scope_intrinsically_incomplete(&scope));
    }

    #[test]
    fn logical_partition_root_downgrades_dataset_rows_and_sizes() {
        let mut assessment = classify_pg_topology(&ordinary_pg_evidence(false, 0));
        let root = TableRow {
            oid: 1,
            schema_name: "app".to_string(),
            table_name: "events".to_string(),
            relkind: "p".to_string(),
            relpersistence: "p".to_string(),
            relispopulated: true,
            has_subclasses: true,
            parent_oid: None,
            partition_strategy: "r".to_string(),
            partition_count: 2,
            partition_key_cols: vec![1],
            partition_key_has_expression: false,
            check_count: 0,
            reltuples: Some(0.0),
            table_bytes: Some(0),
            index_bytes: Some(0),
            stats_freshness: "fresh".to_string(),
            sampling_empty_proven: false,
            sampling_allowed: false,
            row_security_active: false,
            sampling_blocked_by_ancestor_row_security: false,
        };
        assessment.record_table_capture(
            &PgTableCapture {
                tables: vec![root],
                distributed_size_complete: false,
            },
            &mut AuditLog::default(),
        );
        assert_eq!(assessment.dataset_scope.row_count_completeness, "incomplete");
        assert_eq!(assessment.dataset_scope.size_completeness, "incomplete");
        assert!(assessment
            .dataset_scope
            .limitations
            .contains(&"row-count-evidence-incomplete".to_string()));
        assert!(assessment
            .dataset_scope
            .limitations
            .contains(&"logical-partition-root-unmeasured".to_string()));
        assert!(assessment
            .dataset_scope
            .limitations
            .contains(&"size-evidence-incomplete".to_string()));
    }

    #[test]
    fn never_analyzed_table_downgrades_row_coverage_and_table_provenance() {
        let mut assessment = classify_pg_topology(&ordinary_pg_evidence(false, 0));
        let table = TableRow {
            oid: 1,
            schema_name: "app".to_string(),
            table_name: "events".to_string(),
            relkind: "r".to_string(),
            relpersistence: "p".to_string(),
            relispopulated: true,
            has_subclasses: false,
            parent_oid: None,
            partition_strategy: String::new(),
            partition_count: 0,
            partition_key_cols: Vec::new(),
            partition_key_has_expression: false,
            check_count: 0,
            reltuples: None,
            table_bytes: Some(0),
            index_bytes: Some(0),
            stats_freshness: "never_analyzed".to_string(),
            sampling_empty_proven: false,
            sampling_allowed: true,
            row_security_active: false,
            sampling_blocked_by_ancestor_row_security: false,
        };
        assessment.record_table_capture(
            &PgTableCapture {
                tables: vec![table],
                distributed_size_complete: false,
            },
            &mut AuditLog::default(),
        );
        assert_eq!(assessment.dataset_scope.row_count_completeness, "incomplete");
        assert!(assessment
            .dataset_scope
            .limitations
            .contains(&"row-count-evidence-incomplete".to_string()));

        let mut blueprint = BlueprintFile {
            schema_version: SCHEMA_VERSION,
            engine: "postgresql".to_string(),
            dataset_scope: Some(assessment.dataset_scope),
            tables: BTreeMap::from([(
                "table-001".to_string(),
                BlueprintTable {
                    object_kind: "ordinary-table".to_string(),
                    storage_organization: "heap".to_string(),
                    partitioning: "none".to_string(),
                    segment_state: "created".to_string(),
                    statistics: Some(dbwarp_blueprint_core::TableStatisticsEvidence {
                        statistics_state: "never-analyzed".to_string(),
                        ..dbwarp_blueprint_core::TableStatisticsEvidence::unknown_database(
                            "postgres-planner-estimate",
                            "postgres-local-relation-size",
                        )
                    }),
                    ..BlueprintTable::default()
                },
            )]),
            ..BlueprintFile::default()
        };
        qualify_v7_statistics(&mut blueprint, &BTreeSet::new()).unwrap();
        let evidence = blueprint.tables["table-001"].statistics.as_ref().unwrap();
        assert_eq!(evidence.row_count_method, "unknown");
        assert_eq!(evidence.row_count_quality, "unavailable");
        assert!(blueprint
            .statistics_evidence
            .as_ref()
            .unwrap()
            .limitations
            .contains(&"statistics-stale".to_string()));
    }

    #[test]
    fn row_security_does_not_hide_missing_postgres_row_evidence() {
        let mut assessment = classify_pg_topology(&ordinary_pg_evidence(false, 0));
        let table = TableRow {
            oid: 1,
            schema_name: "app".to_string(),
            table_name: "secured_events".to_string(),
            relkind: "r".to_string(),
            relpersistence: "p".to_string(),
            relispopulated: true,
            has_subclasses: false,
            parent_oid: None,
            partition_strategy: String::new(),
            partition_count: 0,
            partition_key_cols: Vec::new(),
            partition_key_has_expression: false,
            check_count: 0,
            // VACUUM can create a reltuples value without establishing
            // ANALYZE statistics. Presence alone must not erase the
            // per-table never-analyzed downgrade.
            reltuples: Some(0.0),
            table_bytes: Some(0),
            index_bytes: Some(0),
            stats_freshness: "never_analyzed".to_string(),
            sampling_empty_proven: false,
            sampling_allowed: false,
            row_security_active: true,
            sampling_blocked_by_ancestor_row_security: false,
        };
        assert!(!pg_catalog_row_estimate_available(&table));
        assessment.record_table_capture(
            &PgTableCapture {
                tables: vec![table],
                distributed_size_complete: false,
            },
            &mut AuditLog::default(),
        );
        assert_eq!(assessment.dataset_scope.row_count_completeness, "incomplete");
        assert!(assessment
            .dataset_scope
            .limitations
            .contains(&"row-count-evidence-incomplete".to_string()));
    }

    #[test]
    fn complete_bounded_read_restores_never_analyzed_table_provenance() {
        let mut assessment = classify_pg_topology(&ordinary_pg_evidence(false, 0));
        assessment
            .dataset_scope
            .limitations
            .push("row-count-evidence-incomplete".to_string());
        assessment.dataset_scope.row_count_completeness = "incomplete".to_string();
        apply_pg_bounded_row_scope(&mut assessment.dataset_scope, true, false, false);
        assert_eq!(
            assessment.dataset_scope.row_count_method,
            "bounded-complete-read"
        );
        assert_eq!(assessment.dataset_scope.row_count_completeness, "complete");
        assert!(!assessment
            .dataset_scope
            .limitations
            .contains(&"row-count-evidence-incomplete".to_string()));
        let mut blueprint = BlueprintFile {
            schema_version: SCHEMA_VERSION,
            engine: "postgresql".to_string(),
            dataset_scope: Some(assessment.dataset_scope),
            tables: BTreeMap::from([(
                "table-001".to_string(),
                BlueprintTable {
                    rows: format::round_rows(600),
                    object_kind: "ordinary-table".to_string(),
                    storage_organization: "heap".to_string(),
                    partitioning: "none".to_string(),
                    segment_state: "created".to_string(),
                    statistics: Some(dbwarp_blueprint_core::TableStatisticsEvidence {
                        statistics_state: "never-analyzed".to_string(),
                        ..dbwarp_blueprint_core::TableStatisticsEvidence::unknown_database(
                            "postgres-planner-estimate",
                            "postgres-local-relation-size",
                        )
                    }),
                    ..BlueprintTable::default()
                },
            )]),
            ..BlueprintFile::default()
        };
        qualify_v7_statistics(
            &mut blueprint,
            &BTreeSet::from(["table-001".to_string()]),
        )
        .unwrap();
        let evidence = blueprint.tables["table-001"].statistics.as_ref().unwrap();
        assert_eq!(evidence.row_count_method, "bounded-complete-read");
        assert_eq!(evidence.row_count_quality, "exact-read");
        assert_eq!(evidence.sample_fraction_band, "full");
        assert_eq!(blueprint.tables["table-001"].rows, 600);
    }

    #[test]
    fn complete_reads_cannot_restore_logical_partition_root_coverage() {
        let mut scope = classify_pg_topology(&ordinary_pg_evidence(false, 0)).dataset_scope;
        scope.row_count_completeness = "incomplete".to_string();
        scope
            .limitations
            .extend(["logical-partition-root-unmeasured".to_string(),
                     "row-count-evidence-incomplete".to_string()]);

        apply_pg_bounded_row_scope(&mut scope, true, false, false);

        assert_eq!(scope.row_count_completeness, "incomplete");
        assert_ne!(scope.row_count_method, "bounded-complete-read");
        assert!(scope
            .limitations
            .contains(&"logical-partition-root-unmeasured".to_string()));
    }

    #[test]
    fn active_row_security_prevents_a_short_sample_becoming_a_complete_read() {
        assert!(pg_complete_row_read(49, 50, false));
        assert!(!pg_complete_row_read(49, 50, true));
        assert!(!pg_complete_row_read(50, 50, false));
        assert!(include_str!("engine_pg.rs")
            .contains("pg_catalog.row_security_active(c.oid) AS row_security_active"));
        assert!(include_str!("engine_pg.rs").contains("ancestor_security ON true"));
    }

    #[test]
    fn streaming_primary_counts_only_directly_visible_standbys() {
        let assessment = classify_pg_topology(&ordinary_pg_evidence(false, 2));
        assert_eq!(assessment.topology.deployment, "replicated");
        assert_eq!(assessment.topology.member_count, 3);
        assert_eq!(assessment.topology.role_counts.get("primary"), Some(&1));
        assert_eq!(assessment.topology.role_counts.get("secondary"), Some(&2));
        assert_eq!(assessment.topology.visibility, "partial");
        assert!(assessment
            .dataset_scope
            .limitations
            .contains(&"replica-membership-unresolved".to_string()));
    }

    #[test]
    fn streaming_standby_reports_upstream_without_persisting_its_identity() {
        let assessment = classify_pg_topology(&ordinary_pg_evidence(true, 1));
        assert_eq!(assessment.topology.deployment, "replicated");
        assert_eq!(assessment.topology.local_role, "secondary");
        assert_eq!(assessment.topology.member_count, 2);
        assert_eq!(assessment.topology.role_counts.get("primary"), Some(&1));
        assert_eq!(assessment.topology.role_counts.get("secondary"), Some(&1));
        assert!(assessment.topology.identifiers_redacted);
    }

    #[test]
    fn citus_coordinator_requires_aggregate_sizes_and_refuses_shell_row_counts() {
        let mut assessment = classify_pg_topology(&citus_coordinator_evidence());
        assert_eq!(assessment.table_size_mode, PgTableSizeMode::CitusAggregate);
        assert_eq!(assessment.topology.deployment, "distributed");
        assert_eq!(assessment.topology.local_role, "coordinator");
        assert_eq!(assessment.topology.member_count, 3);
        assert_eq!(assessment.dataset_scope.layout, "distributed");
        assert_eq!(
            assessment.dataset_scope.row_count_completeness,
            "incomplete"
        );
        assert_eq!(assessment.dataset_scope.row_count_method, "unknown");
        assert_eq!(assessment.dataset_scope.size_completeness, "unknown");

        let mut audit = AuditLog::default();
        assessment.record_table_capture(
            &PgTableCapture {
                tables: Vec::new(),
                distributed_size_complete: true,
            },
            &mut audit,
        );
        assert_eq!(assessment.dataset_scope.size_completeness, "complete");
        assert_eq!(
            assessment.dataset_scope.size_method,
            "citus-distributed-relation-size"
        );
        assert!(assessment
            .topology
            .catalogs_read
            .contains(&"citus-relation-size".to_string()));
        assert!(audit.warnings.is_empty());
    }

    #[test]
    fn failed_citus_aggregate_is_explicit_and_coded() {
        let mut assessment = classify_pg_topology(&citus_coordinator_evidence());
        let mut audit = AuditLog::default();
        assessment.record_table_capture(
            &PgTableCapture {
                tables: Vec::new(),
                distributed_size_complete: false,
            },
            &mut audit,
        );
        warn_incomplete_dataset_scope(&assessment.dataset_scope, &mut audit);
        assert_eq!(assessment.dataset_scope.size_completeness, "incomplete");
        assert_eq!(assessment.dataset_scope.size_method, "unknown");
        assert!(assessment
            .dataset_scope
            .limitations
            .contains(&"distributed-size-unavailable".to_string()));
        assert!(assessment
            .topology
            .catalogs_unreadable
            .contains(&"citus-relation-size".to_string()));
        assert!(audit
            .warnings
            .iter()
            .any(|warning| warning.starts_with("DBP1412W ")));
        assert!(audit
            .warnings
            .iter()
            .any(|warning| warning.starts_with("DBP1413W ")));
    }

    #[test]
    fn citus_worker_is_local_member_only() {
        let mut evidence = citus_coordinator_evidence();
        evidence.local_group_id = Some(2);
        let assessment = classify_pg_topology(&evidence);
        assert_eq!(
            assessment.table_size_mode,
            PgTableSizeMode::CitusLocalMember
        );
        assert_eq!(assessment.topology.local_role, "worker");
        assert_eq!(
            assessment.dataset_scope.table_inventory_completeness,
            "incomplete"
        );
        assert_eq!(assessment.dataset_scope.size_completeness, "incomplete");
        assert!(assessment
            .dataset_scope
            .limitations
            .contains(&"local-member-only".to_string()));
    }

    #[test]
    fn unreadable_citus_metadata_suppresses_all_totals() {
        let evidence = PgTopologyEvidence {
            base_readable: true,
            in_recovery: Some(false),
            citus_installed: Some(true),
            replication_catalog_readable: true,
            direct_peer_count: Some(0),
            ..PgTopologyEvidence::default()
        };
        let assessment = classify_pg_topology(&evidence);
        assert_eq!(assessment.table_size_mode, PgTableSizeMode::Suppress);
        assert_eq!(assessment.dataset_scope.layout, "unknown");
        assert_eq!(assessment.dataset_scope.size_completeness, "unknown");
        assert!(assessment
            .topology
            .catalogs_unreadable
            .contains(&"citus-metadata".to_string()));
    }

    #[test]
    fn unreadable_base_topology_suppresses_unqualified_totals() {
        let assessment = classify_pg_topology(&PgTopologyEvidence::default());
        assert_eq!(assessment.table_size_mode, PgTableSizeMode::Suppress);
        assert_eq!(assessment.topology.visibility, "unknown");
        assert_eq!(assessment.dataset_scope.layout, "unknown");
        assert_eq!(assessment.dataset_scope.row_count_method, "unknown");
        assert_eq!(assessment.dataset_scope.size_method, "unknown");
    }

    #[test]
    fn artifact_catalog_maps_dependency_source_addresses() {
        assert_eq!(pg_artifact_catalog("view", "ordinary"), Some("pg_class"));
        assert_eq!(
            pg_artifact_catalog("function", "scalar_function"),
            Some("pg_proc")
        );
        assert_eq!(
            pg_artifact_catalog("default", "check_constraint"),
            Some("pg_constraint")
        );
        assert_eq!(
            pg_artifact_catalog("default", "column_default"),
            Some("pg_attrdef")
        );
        assert_eq!(pg_artifact_catalog("unknown", "unknown"), None);
    }

    #[test]
    fn pg13_subscription_inventory_avoids_the_protected_hidden_oid() {
        let sql = pg_subscription_inventory_sql("13.23 (Debian 13.23-1)");
        assert!(!sql.contains("oid::text AS native_id"));
        assert!(sql.contains("subdbid::text || ':' || subname::text"));
        assert!(sql.contains("current_database()"));

        let current_sql = pg_subscription_inventory_sql("14.24");
        assert!(current_sql.contains("oid::text AS native_id"));
        assert!(current_sql.contains("current_database()"));
    }

    #[test]
    fn parse_uri_minimal() {
        let (p, pw) = PgConnectParams::parse("postgresql://app@db.example/payments").unwrap();
        assert_eq!(p.host, "db.example");
        assert_eq!(p.port, 5432);
        assert_eq!(p.database, "payments");
        assert_eq!(p.user, "app");
        assert!(p.uri_user_was_explicit);
        assert_eq!(pw, None);
        assert!(p.redacted_uri.contains("payments"));
    }

    #[test]
    fn parse_uri_no_user_falls_back_to_default() {
        let (p, _) = PgConnectParams::parse("postgresql://db.example/payments").unwrap();
        assert_eq!(p.user, "postgres");
        assert!(!p.uri_user_was_explicit);
    }

    #[test]
    fn parse_uri_full() {
        let (p, pw) =
            PgConnectParams::parse("postgresql://app:hunter2@db.example:6432/db").unwrap();
        assert_eq!(p.host, "db.example");
        assert_eq!(p.port, 6432);
        assert_eq!(p.database, "db");
        assert_eq!(p.user, "app");
        assert_eq!(pw.as_deref(), Some("hunter2"));
        // redacted does NOT contain the password.
        assert!(!p.redacted_uri.contains("hunter2"));
    }

    #[test]
    fn parse_uri_pct_decoded() {
        let (p, pw) = PgConnectParams::parse("postgresql://app:%2A%23%21@db/payments").unwrap();
        assert_eq!(p.user, "app");
        assert_eq!(pw.as_deref(), Some("*#!"));
    }

    #[test]
    fn declared_character_capacity_is_exact_and_byte_capacity_is_not_inferred() {
        assert_eq!(declared_pg_max_chars("character varying(32)"), 32);
        assert_eq!(declared_pg_max_chars("character varying(1024)"), 1024);
        assert_eq!(declared_pg_max_chars("varchar(512)"), 512);
        assert_eq!(declared_pg_max_chars("character(7)"), 7);
        assert_eq!(declared_pg_max_chars("char(9)[]"), 9);
    }

    #[test]
    fn declared_character_capacity_is_not_guessed_for_unbounded_or_custom_types() {
        assert_eq!(declared_pg_max_chars("character varying"), 0);
        assert_eq!(declared_pg_max_chars("text"), 0);
        assert_eq!(declared_pg_max_chars("customer_code"), 0);
        assert_eq!(declared_pg_max_chars("numeric(12,4)"), 0);
        assert_eq!(declared_pg_max_chars("varchar(not-a-size)"), 0);
    }

    #[test]
    fn dense_float32_vector_metadata_is_neutral_and_exact() {
        assert_eq!(normalized_pg_type("vector(768)"), "vector");
        assert_eq!(type_tag_for_pg_str("vector(768)"), TypeTag::VectorBinary);
        assert_eq!(declared_pg_max_bytes("vector(768)"), 3_076);
        assert_eq!(declared_pg_max_bytes("vector"), 0);
        assert_eq!(declared_pg_max_bytes("vector(16001)"), 0);
        assert_eq!(declared_pg_max_bytes("customer_vector(768)"), 0);
    }

    #[test]
    fn parse_uri_rejects_non_postgres_scheme() {
        assert!(PgConnectParams::parse("mysql://x@db/d").is_err());
    }

    #[test]
    fn source_kind_parse() {
        assert!(matches!(
            SourceKind::parse("production").unwrap(),
            SourceKind::Production
        ));
        assert!(matches!(
            SourceKind::parse("STAGING").unwrap(),
            SourceKind::Staging
        ));
        assert!(matches!(
            SourceKind::parse("synth").unwrap(),
            SourceKind::Synthetic
        ));
        assert!(SourceKind::parse("garbage").is_err());
    }

    // IPv6 host forms must parse correctly.

    #[test]
    fn parse_ipv6_loopback_no_port() {
        let (p, _) = PgConnectParams::parse("postgresql://app@[::1]/payments").unwrap();
        assert_eq!(p.host, "::1");
        assert_eq!(p.port, 5432);
        assert_eq!(p.database, "payments");
    }

    #[test]
    fn parse_ipv6_loopback_with_port() {
        let (p, _) = PgConnectParams::parse("postgresql://app@[::1]:5433/payments").unwrap();
        assert_eq!(p.host, "::1");
        assert_eq!(p.port, 5433);
    }

    #[test]
    fn parse_ipv6_with_zone_id() {
        let (p, _) =
            PgConnectParams::parse("postgresql://app@[fe80::1%eth0]:5432/payments").unwrap();
        assert_eq!(p.host, "fe80::1%eth0");
        assert_eq!(p.port, 5432);
    }

    #[test]
    fn parse_ipv6_unbracketed_rejected() {
        // Bare IPv6 with embedded colons is ambiguous: must be bracketed.
        assert!(PgConnectParams::parse("postgresql://app@::1/payments").is_err());
    }

    #[test]
    fn style_sample_prefix_never_splits_utf8() {
        assert_eq!(utf8_prefix_bytes("abécd", 3), b"ab");
        assert_eq!(utf8_prefix_bytes("abécd", 4), "abé".as_bytes());
    }

    #[test]
fn postgres_binary_sample_hex_is_decoded_to_source_bytes() {
        assert_eq!(
            decode_pg_hex_sample("00ff89504e470d0a1a0a").unwrap(),
            b"\x00\xff\x89PNG\r\n\x1a\n"
        );
        assert!(decode_pg_hex_sample("abc").is_err());
        assert!(decode_pg_hex_sample("zz").is_err());
    }

    #[test]
    fn normalized_postgres_temporal_types_preserve_timezone_semantics() {
        assert_eq!(normalized_pg_type("timestamp without time zone"), "timestamp");
        assert_eq!(normalized_pg_type("timestamp with time zone"), "timestamptz");
        assert_eq!(normalized_pg_type("timestamptz"), "timestamptz");
        assert_eq!(normalized_pg_type("time without time zone"), "time");
        // TIME WITH TIME ZONE remains an explicitly unsupported output
        // distinction; do not conflate that limitation with TIMESTAMPTZ.
        assert_eq!(normalized_pg_type("time with time zone"), "time");
        assert_eq!(normalized_pg_type("timetz"), "time");
        assert_eq!(
            normalized_pg_type("timestamp with time zone[]"),
            "array<timestamptz>"
        );
    }

    #[test]
    fn adaptive_compression_sample_rate_avoids_moderate_table_underfill() {
        let percent = pg_table_sample_percent(375_000.0, 1_000);
        assert!((percent - 1.066_666_666).abs() < 0.000_001);
    }

    #[test]
    fn adaptive_compression_sample_rate_is_bounded() {
        assert_eq!(pg_table_sample_percent(30_000_000.0, 1_000), 0.1);
        assert_eq!(pg_table_sample_percent(500.0, 1_000), 100.0);
        assert_eq!(pg_table_sample_percent(f64::NAN, 1_000), 100.0);
        assert_eq!(pg_table_sample_percent(10_000.0, 0), 100.0);
    }

    #[test]
    fn reltuples_uses_one_rounded_domain_for_sampling_and_serialization() {
        assert_eq!(pg_reltuples_row_estimate(Some(40.4)), Some(40));
        assert_eq!(pg_reltuples_row_estimate(Some(40.6)), Some(41));
        assert_eq!(pg_reltuples_row_estimate(Some(-1.0)), None);
        assert_eq!(pg_reltuples_row_estimate(Some(f64::NAN)), None);
        assert_eq!(pg_reltuples_row_estimate(None), None);
    }

    #[test]
    fn complete_scan_skips_the_limit_fallback_without_underfill_bias() {
        assert!(pg_sample_scanned_complete_table(100.0, false));
        assert!(!pg_sample_scanned_complete_table(100.0, true));
        assert!(!pg_sample_scanned_complete_table(99.9, false));
        // A small table's adaptive rate always reaches the complete-scan
        // clamp, so its underfilled result is a census, not degradation.
        assert!(pg_sample_scanned_complete_table(
            pg_table_sample_percent(500.0, 1_000),
            false
        ));
        assert!(pg_sample_method_complete_scan().contains("complete scan"));
        assert!(!pg_sample_method_complete_scan().contains("fallback"));
    }

    #[test]
    fn stats_freshness_is_classified_against_the_run_reference_instant() {
        let utc = |raw: &str| {
            chrono::DateTime::parse_from_rfc3339(raw)
                .expect("test timestamp parses")
                .with_timezone(&chrono::Utc)
        };
        let analyzed = utc("2026-04-20T00:00:00Z");
        // Identical inputs classify identically on both sides of the
        // seven-day boundary; the wall clock plays no part.
        assert_eq!(
            pg_stats_freshness(Some(10.0), Some(analyzed), utc("2026-04-27T00:00:00Z")),
            "fresh"
        );
        assert_eq!(
            pg_stats_freshness(Some(10.0), Some(analyzed), utc("2026-04-28T00:00:01Z")),
            "stale"
        );
        assert_eq!(
            pg_stats_freshness(None, Some(analyzed), utc("2026-04-27T00:00:00Z")),
            "never_analyzed"
        );
        assert_eq!(
            pg_stats_freshness(Some(10.0), None, utc("2026-04-27T00:00:00Z")),
            "never_analyzed"
        );
    }

    #[test]
    fn emitted_version_excludes_packaging_and_build_text() {
        assert_eq!(
            normalized_pg_version("16.4 (Ubuntu 16.4-1.pgdg22.04+1)"),
            "16.4"
        );
        assert_eq!(normalized_pg_version("PostgreSQL 18.1-custom-host"), "18.1");
        assert_eq!(normalized_pg_version("custom-build"), "unknown");
    }
}

#[test]
fn postgres_sample_truncation_uses_server_side_octet_lengths() {
    assert_eq!(pg_sample_lengths(Some("8"), Some("8")).unwrap(), Some((8, 8)));
    assert_eq!(
        pg_sample_lengths(Some("4096"), Some("1024")).unwrap(),
        Some((4096, 1024))
    );
    assert_eq!(pg_sample_lengths(None, None).unwrap(), None);
    assert!(pg_sample_lengths(Some("8"), None).is_err());
    assert!(pg_sample_lengths(Some("invalid"), Some("8")).is_err());
}
