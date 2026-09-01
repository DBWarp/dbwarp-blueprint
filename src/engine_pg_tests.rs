#[cfg(test)]
mod tests {
    use super::*;

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
        // Bare IPv6 with embedded colons is ambiguous — must be bracketed.
        assert!(PgConnectParams::parse("postgresql://app@::1/payments").is_err());
    }

    #[test]
    fn style_sample_prefix_never_splits_utf8() {
        assert_eq!(utf8_prefix_bytes("abécd", 3), b"ab");
        assert_eq!(utf8_prefix_bytes("abécd", 4), "abé".as_bytes());
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
