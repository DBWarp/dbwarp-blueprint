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

    #[test]
    fn numeric_primary_key_range_windows_span_the_source_domain() {
        assert_eq!(
            mysql_range_sample_thresholds(1, 60_000_000, 4),
            vec![1, 15_000_000, 30_000_000, 45_000_000]
        );
        assert_eq!(
            mysql_range_sample_thresholds(-100, 100, 4),
            vec![-100, -50, 0, 50]
        );
        assert_eq!(mysql_range_sample_thresholds(7, 7, 4), vec![7; 4]);
    }

    #[test]
    fn mysql_projection_budget_preserves_narrow_columns_and_funds_wide_text() {
        let columns = vec![
            ColumnRow {
                col_type: "integer".into(),
                native_type: "bigint".into(),
                numeric_precision: 19,
                ..ColumnRow::default()
            },
            ColumnRow {
                col_type: "text".into(),
                native_type: "varchar".into(),
                char_max_length: 120,
                char_octet_length: 480,
                character_set_name: "utf8mb4".into(),
                ..ColumnRow::default()
            },
            ColumnRow {
                col_type: "text".into(),
                native_type: "enum".into(),
                char_max_length: 5,
                char_octet_length: 20,
                character_set_name: "utf8mb4".into(),
                ..ColumnRow::default()
            },
            ColumnRow {
                col_type: "text".into(),
                native_type: "longtext".into(),
                char_max_length: u32::MAX.into(),
                char_octet_length: u32::MAX.into(),
                character_set_name: "utf8mb4".into(),
                ..ColumnRow::default()
            },
            ColumnRow {
                col_type: "binary".into(),
                native_type: "binary".into(),
                char_max_length: 16,
                char_octet_length: 16,
                ..ColumnRow::default()
            },
        ];

        let (rows, limits) = mysql_sample_projection_budget(4_096, &columns).unwrap();
        assert_eq!(rows, 4_096);
        assert_eq!(limits[1].char_limit, 120);
        assert_eq!(limits[2].char_limit, 5);
        assert!(limits[3].char_limit >= 291);
        assert_eq!(limits[4].byte_limit, 16);
    }

    #[test]
    fn mysql_projection_budget_reduces_rows_for_pathologically_wide_schemas() {
        let columns = (0..1_600)
            .map(|_| ColumnRow {
                col_type: "numeric".into(),
                numeric_precision: 65,
                ..ColumnRow::default()
            })
            .collect::<Vec<_>>();
        let (rows, limits) = mysql_sample_projection_budget(4_096, &columns).unwrap();
        assert!(rows < 4_096);
        assert_eq!(limits.len(), columns.len());
    }

    #[test]
    fn mysql_original_octet_length_is_independent_of_sampled_payload() {
        assert_eq!(mysql_observed_octet_length(&Value::UInt(291)), Some(291));
        assert_eq!(mysql_observed_octet_length(&Value::Int(226)), Some(226));
        assert_eq!(
            mysql_observed_octet_length(&Value::Bytes(b"154".to_vec())),
            Some(154)
        );
        assert_eq!(mysql_observed_octet_length(&Value::NULL), None);
    }

    #[test]
    fn mysql_adaptive_projection_caps_single_and_combined_oversized_values() {
        let ceiling = dbwarp_blueprint_core::TRANSFER_PROBE_MAX_SAMPLE_BYTES as u64;
        for native in ["binary", "text"] {
            for width in [1, ceiling - 4096, ceiling, ceiling + 4096, u64::MAX] {
                for count in [1, 2, 32] {
                    let columns = vec![
                        ColumnRow {
                            col_type: native.into(),
                            ..ColumnRow::default()
                        };
                        count
                    ];
                    for requested in [1, 32, u64::MAX] {
                        let (rows, limits) = mysql_adaptive_projection_budget(
                            &columns,
                            &vec![width; count],
                            requested,
                        )
                        .unwrap();
                        let row_bytes = limits
                            .iter()
                            .map(|limit| {
                                let payload = if native == "binary" {
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
    fn mysql_text_retry_reserves_for_connection_transcoding_and_changed_rows() {
        let columns = [ColumnRow {
            col_type: "text".into(),
            character_set_name: "latin1".into(),
            char_max_length: 1024,
            char_octet_length: 1024,
            ..ColumnRow::default()
        }];
        assert_eq!(mysql_sample_bytes_per_character(&columns[0]), 4);
        let (rows, limits) = mysql_adaptive_projection_budget(&columns, &[1024], 32).unwrap();
        assert_eq!(rows, 32);
        assert_eq!(limits[0].char_limit, 1024);
        assert_eq!(limits[0].byte_limit, 4096);
        let (rows, limits) = mysql_sample_projection_budget(u64::MAX, &columns).unwrap();
        assert!(
            rows * (limits[0].char_limit as u64 * 4 + 6)
                <= dbwarp_blueprint_core::TRANSFER_PROBE_MAX_SAMPLE_BYTES as u64
        );
    }

    #[test]
    fn mysql_projection_observes_truncation_before_result_charset_conversion() {
        let column = ColumnRow {
            col_name: "narrow_text".into(),
            col_type: "text".into(),
            character_set_name: "latin1".into(),
            ..ColumnRow::default()
        };
        let projection = mysql_sample_projection(
            &[column],
            &[MysqlSampleProjectionLimit {
                byte_limit: 4,
                char_limit: 1,
            }],
        );
        assert_eq!(projection, "LEFT(`narrow_text`, 1), OCTET_LENGTH(`narrow_text`), OCTET_LENGTH(LEFT(`narrow_text`, 1))");
    }

    #[test]
    fn mysql_generated_column_roles_are_closed_and_not_transfer_values() {
        assert_eq!(
            mysql_column_value_source("STORED GENERATED"),
            "generated-stored"
        );
        assert_eq!(
            mysql_column_value_source("VIRTUAL GENERATED"),
            "generated-virtual"
        );
        assert_eq!(
            mysql_column_value_source("auto_increment"),
            "auto-increment"
        );
        assert_eq!(mysql_column_value_source("DEFAULT_GENERATED"), "");

        let ordinary = ColumnRow::default();
        let generated = ColumnRow {
            value_source: "generated-stored".into(),
            ..ColumnRow::default()
        };
        assert!(mysql_column_is_transfer_value(&ordinary));
        assert!(!mysql_column_is_transfer_value(&generated));
    }

    #[test]
    fn sampled_column_results_expand_back_to_source_ordinals() {
        let expanded = expand_mysql_sampled_columns(vec![Some(10_u64), Some(30_u64)], &[0, 2], 3);
        assert_eq!(expanded, vec![Some(10), None, Some(30)]);
    }

    #[test]
    fn range_sampling_requires_the_first_numeric_primary_key_column() {
        let qual = ("app".to_string(), "events".to_string());
        let mut id = ColumnRow {
            schema_name: qual.0.clone(),
            table_name: qual.1.clone(),
            ordinal: 1,
            col_name: "event_id".into(),
            col_type: "numeric".into(),
            native_type: "bigint".into(),
            numeric_scale: 0,
            ..ColumnRow::default()
        };
        let payload = ColumnRow {
            schema_name: qual.0.clone(),
            table_name: qual.1.clone(),
            ordinal: 2,
            col_name: "payload".into(),
            col_type: "text".into(),
            native_type: "varchar".into(),
            ..ColumnRow::default()
        };
        let indexes = BTreeMap::from([(
            (qual.0.clone(), qual.1.clone(), "PRIMARY".into()),
            vec![(
                1,
                "event_id".into(),
                true,
                "BTREE".into(),
                true,
                false,
                0,
                false,
            )],
        )]);
        let columns = vec![id.clone(), payload];
        let plan = mysql_primary_range_sample_plan(&qual, &columns, &indexes)
            .expect("numeric leading primary key must support range sampling");
        assert_eq!(plan.range_column.ordinal, 1);
        assert_eq!(
            plan.order_columns
                .iter()
                .map(|column| column.col_name.as_str())
                .collect::<Vec<_>>(),
            vec!["event_id"]
        );

        id.native_type = "varchar".into();
        assert!(mysql_primary_range_sample_plan(&qual, &[id], &indexes).is_none());
    }

    #[test]
    fn range_sampling_orders_by_the_complete_composite_primary_key() {
        let qual = ("app".to_string(), "events".to_string());
        let columns = vec![
            ColumnRow {
                schema_name: qual.0.clone(),
                table_name: qual.1.clone(),
                ordinal: 1,
                col_name: "parent_id".into(),
                col_type: "numeric".into(),
                native_type: "bigint".into(),
                numeric_scale: 0,
                ..ColumnRow::default()
            },
            ColumnRow {
                schema_name: qual.0.clone(),
                table_name: qual.1.clone(),
                ordinal: 2,
                col_name: "position".into(),
                col_type: "numeric".into(),
                native_type: "int".into(),
                numeric_scale: 0,
                ..ColumnRow::default()
            },
        ];
        let indexes = BTreeMap::from([(
            (qual.0.clone(), qual.1.clone(), "PRIMARY".into()),
            vec![
                (
                    1,
                    "parent_id".into(),
                    true,
                    "BTREE".into(),
                    true,
                    false,
                    0,
                    false,
                ),
                (
                    2,
                    "position".into(),
                    true,
                    "BTREE".into(),
                    true,
                    false,
                    0,
                    false,
                ),
            ],
        )]);

        let plan = mysql_primary_range_sample_plan(&qual, &columns, &indexes)
            .expect("composite numeric primary key must support range sampling");
        assert_eq!(plan.range_column.col_name, "parent_id");
        assert_eq!(
            plan.order_columns
                .iter()
                .map(|column| column.col_name.as_str())
                .collect::<Vec<_>>(),
            vec!["parent_id", "position"]
        );
    }

    fn mysql_table(storage_engine: &str) -> TableRow {
        TableRow {
            schema_name: "app".to_string(),
            table_name: "events".to_string(),
            rows_estimate: 1_000,
            data_length: 65_536,
            index_length: 16_384,
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
        assert!(!assessment
            .dataset_scope
            .limitations
            .contains(&"statistics-stale".to_string()));
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
