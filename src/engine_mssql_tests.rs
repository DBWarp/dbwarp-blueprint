#[cfg(test)]
mod tests {
    use super::*;

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
    fn visible_external_tables_make_local_partition_totals_incomplete() {
        let mut evidence = ordinary_mssql_evidence();
        evidence.external_table_count = 2;
        let assessment = classify_mssql_topology(&evidence);
        assert_eq!(
            assessment.dataset_scope.table_inventory_completeness,
            "incomplete"
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
    fn numeric_formatting() {
        use tiberius::numeric::Numeric;
        // 123.45 — value=12345, scale=2
        assert_eq!(
            format_tiberius_numeric(&Numeric::new_with_scale(12345, 2)),
            "123.45"
        );
        // 0.001234 — value=1234, scale=6
        assert_eq!(
            format_tiberius_numeric(&Numeric::new_with_scale(1234, 6)),
            "0.001234"
        );
        // 0 — zero scale
        assert_eq!(format_tiberius_numeric(&Numeric::new_with_scale(0, 0)), "0");
        // Negative — -12.34
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
}
