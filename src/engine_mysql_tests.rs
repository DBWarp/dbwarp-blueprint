#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloud_token_mode_is_the_only_path_that_enables_cleartext_auth() {
        let normal: mysql_async::Opts = apply_mysql_auth_mode(OptsBuilder::default(), false).into();
        let cloud_token: mysql_async::Opts =
            apply_mysql_auth_mode(OptsBuilder::default(), true).into();
        assert!(!normal.enable_cleartext_plugin());
        assert!(cloud_token.enable_cleartext_plugin());
    }

    #[test]
    fn emitted_version_excludes_the_server_banner() {
        assert_eq!(
            normalized_mysql_version("8.0.46-commercial-build@customer-host"),
            "8.0.46"
        );
        assert_eq!(normalized_mysql_version("5.7.9-Vitess"), "5.7.9");
        assert_eq!(normalized_mysql_version("MySQL 9.7.0 preview"), "9.7.0");
        assert_eq!(normalized_mysql_version("private-build"), "unknown");
    }

    fn mysql_table(storage_engine: &str) -> TableRow {
        TableRow {
            schema_name: "app".to_string(),
            table_name: "events".to_string(),
            rows_estimate: 1_000,
            data_length: 65_536,
            index_length: 16_384,
            update_time: Some(chrono::Utc::now().naive_utc()),
            storage_engine: storage_engine.to_string(),
        }
    }

    fn ordinary_mysql_evidence() -> MysqlTopologyEvidence {
        MysqlTopologyEvidence {
            server_identity_readable: true,
            capability_catalog_readable: true,
            replica_catalog_present: true,
            replica_catalog_readable: true,
            wsrep_catalog_attempted: true,
            wsrep_catalog_readable: true,
            ..MysqlTopologyEvidence::default()
        }
    }

    #[test]
    fn ordinary_mysql_is_full_copy_without_inventing_cluster_membership() {
        let assessment =
            classify_mysql_topology(&ordinary_mysql_evidence(), &[mysql_table("InnoDB")]);
        assert_eq!(assessment.topology.deployment, "unknown");
        assert_eq!(assessment.topology.local_role, "unknown");
        assert_eq!(assessment.topology.visibility, "partial");
        assert_eq!(assessment.dataset_scope.layout, "full-copy");
        assert_eq!(assessment.dataset_scope.row_count_completeness, "complete");
        assert_eq!(assessment.dataset_scope.size_completeness, "complete");
        assert!(!assessment.suppress_table_statistics);
    }

    #[test]
    fn asynchronous_replica_counts_sources_without_reading_channel_identity() {
        let mut evidence = ordinary_mysql_evidence();
        evidence.replica_channel_count = 2;
        let assessment = classify_mysql_topology(&evidence, &[mysql_table("InnoDB")]);
        assert_eq!(assessment.topology.deployment, "replicated");
        assert_eq!(assessment.topology.local_role, "secondary");
        assert_eq!(assessment.topology.member_count, 3);
        assert_eq!(assessment.topology.role_counts.get("primary"), Some(&2));
        assert_eq!(assessment.topology.role_counts.get("secondary"), Some(&1));
        assert!(assessment
            .topology
            .features
            .contains(&"mysql-asynchronous-replication".to_string()));
    }

    #[test]
    fn group_replication_has_full_count_and_role_visibility() {
        let mut evidence = ordinary_mysql_evidence();
        evidence.group_catalog_present = true;
        evidence.group_catalog_readable = true;
        evidence.group_member_count = 3;
        evidence.group_primary_count = 1;
        evidence.group_secondary_count = 2;
        evidence.local_group_role = Some("secondary");
        let assessment = classify_mysql_topology(&evidence, &[mysql_table("InnoDB")]);
        assert_eq!(assessment.topology.visibility, "full");
        assert_eq!(assessment.topology.member_count, 3);
        assert_eq!(assessment.topology.local_role, "secondary");
        assert!(!assessment
            .dataset_scope
            .limitations
            .contains(&"topology-visibility-partial".to_string()));
    }

    #[test]
    fn galera_cluster_is_a_full_copy_member_set() {
        let mut evidence = ordinary_mysql_evidence();
        evidence.galera_active = true;
        evidence.galera_member_count = 5;
        let assessment = classify_mysql_topology(&evidence, &[mysql_table("InnoDB")]);
        assert_eq!(assessment.topology.deployment, "replicated");
        assert_eq!(assessment.topology.local_role, "member");
        assert_eq!(assessment.topology.visibility, "full");
        assert_eq!(assessment.topology.role_counts.get("member"), Some(&5));
    }

    #[test]
    fn vitess_gateway_suppresses_unqualified_shard_statistics() {
        let evidence = MysqlTopologyEvidence {
            server_identity_readable: true,
            vitess_gateway: true,
            ..MysqlTopologyEvidence::default()
        };
        let assessment = classify_mysql_topology(&evidence, &[mysql_table("InnoDB")]);
        assert_eq!(assessment.topology.deployment, "sharded");
        assert_eq!(assessment.topology.local_role, "coordinator");
        assert_eq!(assessment.dataset_scope.layout, "sharded");
        assert_eq!(
            assessment.dataset_scope.table_inventory_completeness,
            "unknown"
        );
        assert!(assessment.suppress_table_statistics);
        assert!(assessment.distributed_size_unavailable);
    }

    #[test]
    fn ndb_sql_node_suppresses_local_member_statistics() {
        let assessment =
            classify_mysql_topology(&ordinary_mysql_evidence(), &[mysql_table("NDBCLUSTER")]);
        assert_eq!(assessment.topology.deployment, "distributed");
        assert!(assessment
            .topology
            .features
            .contains(&"mysql-ndb".to_string()));
        assert_eq!(assessment.dataset_scope.layout, "distributed");
        assert!(assessment.suppress_table_statistics);
    }

    #[test]
    fn suppressed_mysql_statistics_are_zero_and_coded() {
        let evidence = MysqlTopologyEvidence {
            server_identity_readable: true,
            vitess_gateway: true,
            ..MysqlTopologyEvidence::default()
        };
        let mut assessment = classify_mysql_topology(&evidence, &[mysql_table("InnoDB")]);
        let mut tables = vec![mysql_table("InnoDB")];
        let mut audit = AuditLog::default();
        assessment.qualify_table_statistics(&mut tables, &mut audit);
        assert_eq!(tables[0].rows_estimate, 0);
        assert_eq!(tables[0].data_length, 0);
        assert_eq!(tables[0].index_length, 0);
        assert!(audit
            .warnings
            .iter()
            .any(|warning| warning.starts_with("DBP1412W ")));
    }

    /// `is_mysql_utf8_charset` correctly classifies common UTF-8
    /// charset IDs and rejects everything else. The encoder uses
    /// this to decide TextUtf8 vs TextOther for non-binary
    /// string/blob columns.
    #[test]
    fn mysql_style_candidates_are_textual_only() {
        let text = ColumnRow {
            schema_name: "s".into(),
            table_name: "t".into(),
            ordinal: 1,
            col_name: "body".into(),
            col_type: "text".into(),
            is_nullable: false,
            char_octet_length: 100,
            ..ColumnRow::default()
        };
        let json = ColumnRow {
            col_type: "json".into(),
            ..text.clone()
        };
        let binary = ColumnRow {
            col_type: "binary".into(),
            ..text.clone()
        };
        assert!(is_style_candidate_mysql(&text));
        assert!(is_style_candidate_mysql(&json));
        assert!(!is_style_candidate_mysql(&binary));
    }

    #[test]
    fn mysql_identifier_quoting_escapes_backticks() {
        assert_eq!(quote_mysql_ident("a`b"), "`a``b`");
    }

    #[test]
    fn mysql_none_match_option_is_canonical_simple() {
        assert_eq!(normalize_mysql_fk_match("NONE"), "simple");
        assert_eq!(normalize_mysql_fk_match("simple"), "simple");
        assert_eq!(normalize_mysql_fk_match("FULL"), "full");
    }

    #[test]
    fn mysql_native_numeric_semantics_preserve_unsigned_bit_and_year() {
        assert_eq!(
            mysql_numeric_semantics("bigint", "bigint unsigned"),
            (true, 0)
        );
        assert_eq!(mysql_numeric_semantics("bit", "bit(13)"), (false, 13));
        assert_eq!(normalized_mysql_type("bit", 13), "binary");
        assert_eq!(normalized_mysql_type("bit", 1), "boolean");
        assert_eq!(normalized_mysql_type("double", 0), "double");
        assert_eq!(normalized_mysql_type("year", 0), "year");
    }

    #[test]
    fn functional_index_is_preserved_without_leaking_expression_text() {
        let parts = vec![(
            1,
            String::new(),
            true,
            "BTREE".to_string(),
            false,
            false,
            0,
            true,
        )];
        let blueprint =
            index_blueprint_from_parts(&parts, &BTreeMap::new(), LengthFidelity::Balanced);
        assert!(blueprint.expression);
        assert!(blueprint.unique);
        assert!(blueprint.cols.is_empty());
        assert!(blueprint.prefix_lengths.is_empty());
    }

    #[test]
    fn exact_length_mode_preserves_observed_and_prefix_lengths() {
        assert_eq!(
            sampled_column_length_stats(vec![9, 10, 11], LengthFidelity::Exact),
            Some((10, 11))
        );
        assert_eq!(blueprint_length(191, LengthFidelity::Exact), 191);
        assert_eq!(blueprint_prefix_length(191, LengthFidelity::Exact), 191);
    }

    #[test]
    fn balanced_length_mode_preserves_structure_and_short_observations() {
        assert_eq!(
            sampled_column_length_stats(vec![9, 10, 11], LengthFidelity::Balanced),
            Some((10, 11))
        );
        assert_eq!(blueprint_length(191, LengthFidelity::Balanced), 191);
        assert_eq!(blueprint_prefix_length(191, LengthFidelity::Balanced), 191);
        assert_eq!(blueprint_prefix_length(9, LengthFidelity::Balanced), 9);
    }

    #[test]
    fn strict_length_mode_retains_coarse_share_safe_buckets() {
        assert_eq!(
            sampled_column_length_stats(vec![9, 10, 11], LengthFidelity::Strict),
            Some((10, 10))
        );
        assert_eq!(blueprint_length(191, LengthFidelity::Strict), 190);
        assert_eq!(blueprint_prefix_length(191, LengthFidelity::Strict), 190);
        assert_eq!(blueprint_prefix_length(9, LengthFidelity::Strict), 1);
    }

    #[test]
    fn mysql_utf8_charset_classification() {
        // UTF-8 charsets that should map to TextUtf8.
        assert!(is_mysql_utf8_charset(33), "utf8mb3 utf8_general_ci");
        assert!(is_mysql_utf8_charset(45), "utf8mb4_general_ci");
        assert!(is_mysql_utf8_charset(46), "utf8mb4_bin");
        assert!(
            is_mysql_utf8_charset(255),
            "utf8mb4_0900_ai_ci (MySQL 8 default)"
        );
        // Non-UTF-8 charsets that should NOT map to TextUtf8.
        assert!(!is_mysql_utf8_charset(8), "latin1_swedish_ci");
        assert!(!is_mysql_utf8_charset(47), "latin1_bin");
        assert!(
            !is_mysql_utf8_charset(63),
            "binary (caller checks separately)"
        );
        assert!(!is_mysql_utf8_charset(51), "cp1251_general_ci");
        assert!(!is_mysql_utf8_charset(13), "big5_chinese_ci");
        assert!(!is_mysql_utf8_charset(13), "sjis_japanese_ci");
        assert!(!is_mysql_utf8_charset(0), "unset / unknown");
    }

    #[test]
    fn parse_uri_minimal() {
        let (p, pw) = MyConnectParams::parse("mysql://app@db.example/payments").unwrap();
        assert_eq!(p.host, "db.example");
        assert_eq!(p.port, 3306);
        assert_eq!(p.database, "payments");
        assert_eq!(p.user, "app");
        assert_eq!(pw, None);
    }

    #[test]
    fn parse_uri_full() {
        let (p, pw) = MyConnectParams::parse("mysql://app:hunter2@db.example:6033/db").unwrap();
        assert_eq!(p.port, 6033);
        assert_eq!(pw.as_deref(), Some("hunter2"));
        assert!(!p.redacted_uri.contains("hunter2"));
    }

    #[test]
    fn parse_mariadb_scheme() {
        let r = MyConnectParams::parse("mariadb://r@h/d");
        assert!(r.is_ok());
    }

    // Regression guard: IPv6 forms must parse correctly.

    #[test]
    fn parse_ipv6_loopback_no_port() {
        let (p, _) = MyConnectParams::parse("mysql://app@[::1]/payments").unwrap();
        assert_eq!(p.host, "::1");
        assert_eq!(p.port, 3306);
    }

    #[test]
    fn parse_ipv6_loopback_with_port() {
        let (p, _) = MyConnectParams::parse("mysql://app@[::1]:3307/payments").unwrap();
        assert_eq!(p.host, "::1");
        assert_eq!(p.port, 3307);
    }

    /// Build-time guard: the vendored mysql_async TLS path is patched
    /// to skip webpki_roots when user roots are supplied. If someone
    /// re-runs `cargo vendor` and clobbers the patch, this test fails
    /// loudly so reviewers notice before a release ships with the
    /// trust regression. The patch comment string is unique enough
    /// that no upstream version of mysql_async would naturally
    /// contain it.
    #[test]
    fn vendored_mysql_async_restricts_to_user_roots_when_supplied() {
        let src = include_str!("../vendor/mysql_async/src/io/tls/rustls_io.rs");
        assert!(
            src.contains("DBWarp Blueprint patch:"),
            "vendor/mysql_async/src/io/tls/rustls_io.rs is missing the dbwarp-blueprint \
             trust-restriction patch — `--tls-ca` will silently fall back to the \
             upstream behavior (system + webpki_roots also trusted)."
        );
        assert!(
            src.contains("if user_roots.is_empty()"),
            "the user-roots guard is missing from the patched mysql_async TLS path; \
             check vendor/ contents"
        );
    }
}
