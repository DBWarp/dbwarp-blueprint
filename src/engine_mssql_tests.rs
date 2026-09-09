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
