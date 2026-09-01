async fn capture_artifacts(
    client: &mut Client<tokio_util::compat::Compat<TcpStream>>,
    detail: ArtifactDetail,
    engine_version: &str,
    schemas: &SchemaSelection,
    audit: &mut AuditLog,
) -> (Vec<RawArtifact>, CaptureCompleteness) {
    let analyze = detail.reads_definitions();
    let mut out = Vec::new();
    let mut completeness = CaptureCompleteness {
        visibility: "privilege_filtered".to_string(),
        inventory_complete: false,
        dependencies_complete: false,
        families_not_inventoried: vec![
            "grants".to_string(),
            "roles".to_string(),
            "service_broker".to_string(),
            "availability_groups".to_string(),
        ],
        ..CaptureCompleteness::default()
    };

    let visibility_sql = r#"
        SELECT CONVERT(nvarchar(10), COALESCE(IS_SRVROLEMEMBER('sysadmin'), 0)) AS is_sysadmin,
               CONVERT(nvarchar(10), COALESCE(IS_MEMBER('db_owner'), 0)) AS is_db_owner,
               CONVERT(nvarchar(10), COALESCE(HAS_PERMS_BY_NAME(DB_NAME(), 'DATABASE', 'VIEW DEFINITION'), 0)) AS can_view_definition
    "#;
    match mssql_artifact_rows(
        client,
        visibility_sql,
        "probe SQL Server artifact visibility",
        audit,
    )
    .await
    {
        Ok(rows) => {
            completeness
                .catalogs_read
                .push("permission_probe".to_string());
            if let Some(row) = rows.first() {
                let full = ["is_sysadmin", "is_db_owner", "can_view_definition"]
                    .iter()
                    .any(|column| mssql_string(row, column) == "1");
                if full {
                    completeness.visibility = "full".to_string();
                    completeness.inventory_complete = true;
                }
            }
        }
        Err(error) => mssql_catalog_unreadable(
            audit,
            &mut completeness,
            "permission_probe",
            "visibility probe",
            &error,
        ),
    }

    let compatibility_level = match mssql_artifact_rows(
        client,
        "SELECT CONVERT(nvarchar(10), compatibility_level) AS compatibility_level FROM sys.databases WHERE database_id = DB_ID()",
        "read SQL Server database compatibility level",
        audit,
    )
    .await
    {
        Ok(rows) => {
            completeness.catalogs_read.push("sys.databases".to_string());
            rows.first()
                .map(|row| mssql_string(row, "compatibility_level"))
                .unwrap_or_default()
        }
        Err(error) => {
            mssql_catalog_unreadable(
                audit,
                &mut completeness,
                "sys.databases",
                "database compatibility",
                &error,
            );
            String::new()
        }
    };
    let grammar_profile = artifacts::grammar_profile("sqlserver", engine_version);

    let definition_expr = if analyze {
        "m.definition"
    } else {
        "CAST(NULL AS nvarchar(max))"
    };
    let module_sql = format!(
        r#"
        SELECT CONVERT(nvarchar(30), o.object_id) AS native_id,
               s.name AS schema_name,
               o.name AS object_name,
               o.type AS type_code,
               COALESCE(ps.name, '') AS parent_schema,
               COALESCE(po.name, '') AS parent_name,
               {definition_expr} AS definition,
               CONVERT(nvarchar(10), COALESCE(m.uses_ansi_nulls, 0)) AS ansi_nulls,
               CONVERT(nvarchar(10), COALESCE(m.uses_quoted_identifier, 0)) AS quoted_identifier,
               CONVERT(nvarchar(20), COALESCE(m.execute_as_principal_id, 0)) AS execute_as_principal_id,
               CASE WHEN syn.object_id IS NULL THEN ''
                    WHEN PARSENAME(syn.base_object_name, 4) IS NOT NULL
                      OR PARSENAME(syn.base_object_name, 3) IS NOT NULL THEN 'remote_or_cross_database'
                    ELSE 'local_database' END AS synonym_scope,
               CONVERT(nvarchar(10), COALESCE(OBJECTPROPERTY(o.object_id, 'IsEncrypted'), 0)) AS is_encrypted
        FROM sys.objects o
        JOIN sys.schemas s ON s.schema_id = o.schema_id
        LEFT JOIN sys.sql_modules m ON m.object_id = o.object_id
        LEFT JOIN sys.objects po ON po.object_id = o.parent_object_id
        LEFT JOIN sys.schemas ps ON ps.schema_id = po.schema_id
        LEFT JOIN sys.synonyms syn ON syn.object_id = o.object_id
        WHERE o.is_ms_shipped = 0
          AND o.type IN ('V','P','PC','RF','FN','IF','TF','FS','FT','AF','TR','TA','D','C','R','SN','SO')
          {}
        ORDER BY o.object_id
        "#,
        schemas.and_sql("s.name")
    );
    let mut identity_by_native_id = BTreeMap::new();
    match mssql_artifact_rows(
        client,
        &module_sql,
        "read SQL Server modules and declarative objects",
        audit,
    )
    .await
    {
        Ok(rows) => {
            completeness.catalogs_read.push("sys.objects".to_string());
            completeness
                .catalogs_read
                .push("sys.sql_modules".to_string());
            completeness.catalogs_read.push("sys.synonyms".to_string());
            for row in rows {
                let native_id = mssql_string(&row, "native_id");
                let schema = mssql_string(&row, "schema_name");
                let name = mssql_string(&row, "object_name");
                let type_code = mssql_string(&row, "type_code");
                let Some((kind, subkind)) = mssql_artifact_kind(&type_code) else {
                    continue;
                };
                let type_code = type_code.trim();
                let identity = format!("sqlserver|{kind}|{schema}|{name}|{native_id}");
                identity_by_native_id.insert(native_id.clone(), identity.clone());
                let mut item = RawArtifact::new(identity, kind, subkind);
                item.schema_identity = Some(schema.clone());
                let parent_schema = mssql_string(&row, "parent_schema");
                let parent_name = mssql_string(&row, "parent_name");
                if !parent_schema.is_empty() && !parent_name.is_empty() {
                    item.parent_table_identity = Some(artifacts::table_identity(
                        "sqlserver",
                        &parent_schema,
                        &parent_name,
                    ));
                }
                let execute_as = mssql_string(&row, "execute_as_principal_id");
                item.security_mode = match execute_as.as_str() {
                    "0" | "" => "caller",
                    "-2" => "owner",
                    _ => "principal",
                };
                let is_clr = matches!(type_code, "PC" | "FS" | "FT" | "AF" | "TA");
                if is_clr {
                    item.external = Some(RawExternalPrerequisite::package(
                        "sqlserver_clr_assembly",
                        "target_clr_and_runtime_compatible",
                    ));
                }
                if type_code == "SN"
                    && mssql_string(&row, "synonym_scope") == "remote_or_cross_database"
                {
                    item.external = Some(RawExternalPrerequisite::infrastructure(
                        "foreign_endpoint",
                        "server_database_and_network",
                    ));
                }
                let definition: Option<String> = match row.try_get::<&str, _>("definition") {
                    Ok(value) => value.map(ToString::to_string),
                    Err(error) => {
                        mssql_catalog_unreadable(
                            audit,
                            &mut completeness,
                            "sys_modules",
                            "module definitions",
                            &error,
                        );
                        None
                    }
                };
                item.definition_visibility = if is_clr {
                    "external_binary"
                } else if !analyze {
                    "not_read"
                } else if definition.is_some() {
                    "available"
                } else if mssql_string(&row, "is_encrypted") == "1" {
                    "encrypted"
                } else {
                    "withheld"
                };
                if matches!(
                    kind,
                    "view"
                        | "procedure"
                        | "function"
                        | "aggregate"
                        | "trigger"
                        | "default"
                        | "rule"
                ) {
                    item.analysis = Some(RawLanguageAnalysis {
                        definition: definition.map(Zeroizing::new),
                        dialect: if is_clr { "clr" } else { "tsql" }.to_string(),
                        grammar_profile: grammar_profile.clone(),
                        compatibility_level: compatibility_level.clone(),
                        ansi_nulls: mssql_string(&row, "ansi_nulls"),
                        quoted_identifier: mssql_string(&row, "quoted_identifier"),
                        ..RawLanguageAnalysis::default()
                    });
                }
                out.push(item);
            }
        }
        Err(error) => {
            mssql_catalog_unreadable(audit, &mut completeness, "sys_modules", "modules", &error)
        }
    }

    capture_mssql_dependencies(
        client,
        schemas,
        audit,
        &mut out,
        &identity_by_native_id,
        &mut completeness,
    )
    .await;
    capture_mssql_types(client, schemas, audit, &mut out, &mut completeness).await;
    capture_mssql_external(client, schemas, audit, &mut out, &mut completeness).await;
    (out, completeness)
}

async fn capture_mssql_dependencies(
    client: &mut Client<tokio_util::compat::Compat<TcpStream>>,
    schemas: &SchemaSelection,
    audit: &mut AuditLog,
    out: &mut [RawArtifact],
    identity_by_native_id: &BTreeMap<String, String>,
    completeness: &mut CaptureCompleteness,
) {
    let sql = format!(
        r#"
        SELECT CONVERT(nvarchar(30), d.referencing_id) AS referencing_id,
               CONVERT(nvarchar(30), COALESCE(d.referenced_id, 0)) AS referenced_id,
               COALESCE(d.referenced_schema_name, '') AS referenced_schema_name,
               COALESCE(d.referenced_entity_name, '') AS referenced_entity_name,
               COALESCE(o.type, '') AS referenced_type,
               CASE WHEN d.referenced_server_name IS NOT NULL
                       OR d.referenced_database_name IS NOT NULL
                       OR d.is_ambiguous = 1
                       OR d.referenced_id IS NULL THEN '1' ELSE '0' END AS unresolved
        FROM sys.sql_expression_dependencies d
        JOIN sys.objects source_object ON source_object.object_id = d.referencing_id
        JOIN sys.schemas source_schema ON source_schema.schema_id = source_object.schema_id
        LEFT JOIN sys.objects o ON o.object_id = d.referenced_id
        WHERE 1 = 1
          {}
        ORDER BY d.referencing_id, d.referenced_id
    "#,
        schemas.and_sql("source_schema.name")
    );
    match mssql_artifact_rows(
        client,
        &sql,
        "read SQL Server expression dependencies",
        audit,
    )
    .await
    {
        Ok(rows) => {
            completeness
                .catalogs_read
                .push("sys.sql_expression_dependencies".to_string());
            let mut dependencies: BTreeMap<String, Vec<String>> = BTreeMap::new();
            let mut unresolved: BTreeMap<String, u64> = BTreeMap::new();
            for row in rows {
                let referencing = mssql_string(&row, "referencing_id");
                if mssql_string(&row, "unresolved") == "1" {
                    *unresolved.entry(referencing).or_default() += 1;
                    continue;
                }
                let referenced = mssql_string(&row, "referenced_id");
                let dependency =
                    if mssql_dependency_target_is_table(&mssql_string(&row, "referenced_type")) {
                        Some(artifacts::table_identity(
                            "sqlserver",
                            &mssql_string(&row, "referenced_schema_name"),
                            &mssql_string(&row, "referenced_entity_name"),
                        ))
                    } else {
                        identity_by_native_id.get(&referenced).cloned()
                    };
                if let Some(dependency) = dependency {
                    dependencies
                        .entry(referencing)
                        .or_default()
                        .push(dependency);
                } else {
                    *unresolved.entry(referencing).or_default() += 1;
                }
            }
            for item in out {
                if let Some(native_id) = item.identity.rsplit('|').next() {
                    if let Some(values) = dependencies.remove(native_id) {
                        item.dependencies.extend(values);
                    }
                    item.unresolved_dependency_count += unresolved.remove(native_id).unwrap_or(0);
                }
            }
        }
        Err(error) => mssql_catalog_unreadable(
            audit,
            completeness,
            "sys.sql_expression_dependencies",
            "module dependencies",
            &error,
        ),
    }
}

async fn capture_mssql_types(
    client: &mut Client<tokio_util::compat::Compat<TcpStream>>,
    schemas: &SchemaSelection,
    audit: &mut AuditLog,
    out: &mut Vec<RawArtifact>,
    completeness: &mut CaptureCompleteness,
) {
    let sql = format!(
        r#"
        SELECT CONVERT(nvarchar(30), t.user_type_id) AS native_id,
               s.name AS schema_name,
               t.name AS object_name,
               CASE WHEN t.is_assembly_type = 1 THEN 'clr_type'
                    WHEN t.is_table_type = 1 THEN 'table_type'
                    ELSE 'alias_type' END AS subkind,
               CONVERT(nvarchar(10), t.is_assembly_type) AS is_assembly_type
        FROM sys.types t
        JOIN sys.schemas s ON s.schema_id = t.schema_id
        WHERE t.is_user_defined = 1
          {}
        ORDER BY t.user_type_id
    "#,
        schemas.and_sql("s.name")
    );
    match mssql_artifact_rows(client, &sql, "read SQL Server user-defined types", audit).await {
        Ok(rows) => {
            completeness.catalogs_read.push("sys.types".to_string());
            for row in rows {
                let native_id = mssql_string(&row, "native_id");
                let schema = mssql_string(&row, "schema_name");
                let name = mssql_string(&row, "object_name");
                let mut item = RawArtifact::new(
                    format!("sqlserver|type|{schema}|{name}|{native_id}"),
                    "type",
                    match mssql_string(&row, "subkind").as_str() {
                        "clr_type" => "clr_type",
                        "table_type" => "table_type",
                        _ => "alias_type",
                    },
                );
                item.schema_identity = Some(schema);
                if mssql_string(&row, "is_assembly_type") == "1" {
                    item.external = Some(RawExternalPrerequisite::package(
                        "sqlserver_clr_assembly",
                        "target_clr_and_runtime_compatible",
                    ));
                }
                out.push(item);
            }
        }
        Err(error) => mssql_catalog_unreadable(
            audit,
            completeness,
            "sys.types",
            "user-defined types",
            &error,
        ),
    }
}

async fn capture_mssql_external(
    client: &mut Client<tokio_util::compat::Compat<TcpStream>>,
    schemas: &SchemaSelection,
    audit: &mut AuditLog,
    out: &mut Vec<RawArtifact>,
    completeness: &mut CaptureCompleteness,
) {
    let sql = r#"
        SELECT CONVERT(nvarchar(30), assembly_id) AS native_id,
               name AS object_name
        FROM sys.assemblies WHERE is_user_defined = 1 ORDER BY assembly_id
    "#;
    match mssql_artifact_rows(client, sql, "read SQL Server CLR assemblies", audit).await {
        Ok(rows) => {
            completeness
                .catalogs_read
                .push("sys.assemblies".to_string());
            for row in rows {
                let name = mssql_string(&row, "object_name");
                let native_id = mssql_string(&row, "native_id");
                let mut item = RawArtifact::new(
                    format!("sqlserver|assembly|{name}|{native_id}"),
                    "assembly",
                    "clr_assembly",
                );
                item.definition_visibility = "external_binary";
                item.external = Some(RawExternalPrerequisite::package(
                    "sqlserver_clr_assembly",
                    "target_clr_and_runtime_compatible",
                ));
                out.push(item);
            }
        }
        Err(error) => mssql_catalog_unreadable(
            audit,
            completeness,
            "sys.assemblies",
            "CLR assemblies",
            &error,
        ),
    }

    let external_sql = format!(
        r#"
        SELECT 'external_table' AS kind,
               CONVERT(nvarchar(30), et.object_id) AS native_id,
               s.name AS schema_name,
               et.name AS object_name,
               'external_table' AS subkind
        FROM sys.external_tables et JOIN sys.schemas s ON s.schema_id = et.schema_id
        WHERE 1 = 1
          {}
        UNION ALL
        SELECT 'foreign_server', CONVERT(nvarchar(30), data_source_id), '', name, 'external_data_source'
        FROM sys.external_data_sources
        UNION ALL
        SELECT 'foreign_server', CONVERT(nvarchar(30), file_format_id), '', name, 'external_file_format'
        FROM sys.external_file_formats
    "#,
        schemas.and_sql("s.name")
    );
    match mssql_artifact_rows(
        client,
        &external_sql,
        "read SQL Server external data objects without endpoint or credential values",
        audit,
    )
    .await
    {
        Ok(rows) => {
            completeness
                .catalogs_read
                .push("sys.external_tables".to_string());
            completeness
                .catalogs_read
                .push("sys.external_data_sources".to_string());
            completeness
                .catalogs_read
                .push("sys.external_file_formats".to_string());
            for row in rows {
                let kind = mssql_string(&row, "kind");
                let native_id = mssql_string(&row, "native_id");
                let schema = mssql_string(&row, "schema_name");
                let name = mssql_string(&row, "object_name");
                let mut item = RawArtifact::new(
                    format!("sqlserver|{kind}|{schema}|{name}|{native_id}"),
                    if kind == "external_table" {
                        "external_table"
                    } else {
                        "foreign_server"
                    },
                    match mssql_string(&row, "subkind").as_str() {
                        "external_data_source" => "external_data_source",
                        "external_file_format" => "external_file_format",
                        _ => "external_table",
                    },
                );
                if !schema.is_empty() {
                    item.schema_identity = Some(schema.clone());
                }
                item.external = Some(RawExternalPrerequisite::infrastructure(
                    "foreign_endpoint",
                    "server_database_storage_and_network",
                ));
                out.push(item);
            }
        }
        Err(error) => mssql_catalog_unreadable(
            audit,
            completeness,
            "sys_external_objects",
            "external data objects",
            &error,
        ),
    }

    let infrastructure_sql = r#"
        SELECT 'full_text' AS kind, CONVERT(nvarchar(30), fulltext_catalog_id) AS native_id,
               name AS object_name, 'full_text_catalog' AS subkind
        FROM sys.fulltext_catalogs
        UNION ALL
        SELECT 'partition_scheme', CONVERT(nvarchar(30), data_space_id), name, 'partition_scheme'
        FROM sys.partition_schemes
        UNION ALL
        SELECT 'partition_scheme', CONVERT(nvarchar(30), function_id), name, 'partition_function'
        FROM sys.partition_functions
        UNION ALL
        SELECT 'physical_placement', CONVERT(nvarchar(30), data_space_id), name, 'filegroup'
        FROM sys.filegroups WHERE name <> 'PRIMARY'
    "#;
    match mssql_artifact_rows(
        client,
        infrastructure_sql,
        "read SQL Server full-text, partitioning, and physical-placement inventory",
        audit,
    )
    .await
    {
        Ok(rows) => {
            completeness.catalogs_read.extend([
                "sys.fulltext_catalogs".to_string(),
                "sys.partition_schemes".to_string(),
                "sys.partition_functions".to_string(),
                "sys.filegroups".to_string(),
            ]);
            for row in rows {
                let kind = mssql_string(&row, "kind");
                let name = mssql_string(&row, "object_name");
                let native_id = mssql_string(&row, "native_id");
                let mut item = RawArtifact::new(
                    format!("sqlserver|{kind}|{name}|{native_id}"),
                    match kind.as_str() {
                        "full_text" => "full_text",
                        "partition_scheme" => "partition_scheme",
                        _ => "physical_placement",
                    },
                    match mssql_string(&row, "subkind").as_str() {
                        "full_text_catalog" => "full_text_catalog",
                        "partition_scheme" => "partition_scheme",
                        "partition_function" => "partition_function",
                        _ => "filegroup",
                    },
                );
                item.external = Some(RawExternalPrerequisite::infrastructure(
                    if kind == "physical_placement" {
                        "physical_storage"
                    } else {
                        "server_feature"
                    },
                    "server_or_managed_service",
                ));
                out.push(item);
            }
        }
        Err(error) => mssql_catalog_unreadable(
            audit,
            completeness,
            "sys_specialized_physical",
            "specialized and physical objects",
            &error,
        ),
    }

    capture_mssql_security_external(client, audit, out, completeness).await;
    capture_mssql_server_external(client, audit, out, completeness).await;
}

async fn capture_mssql_security_external(
    client: &mut Client<tokio_util::compat::Compat<TcpStream>>,
    audit: &mut AuditLog,
    out: &mut Vec<RawArtifact>,
    completeness: &mut CaptureCompleteness,
) {
    let sql = r#"
        SELECT 'certificate' AS kind, CONVERT(nvarchar(30), certificate_id) AS native_id,
               name AS object_name, 'database_certificate' AS subkind
        FROM sys.certificates WHERE name NOT LIKE '##%'
        UNION ALL
        SELECT 'encryption_key', CONVERT(nvarchar(30), symmetric_key_id), name, 'symmetric_key'
        FROM sys.symmetric_keys WHERE name NOT LIKE '##%'
        UNION ALL
        SELECT 'encryption_key', CONVERT(nvarchar(30), asymmetric_key_id), name, 'asymmetric_key'
        FROM sys.asymmetric_keys WHERE name NOT LIKE '##%'
        UNION ALL
        SELECT 'encryption_key', CONVERT(nvarchar(30), column_master_key_id), name, 'column_master_key'
        FROM sys.column_master_keys
        UNION ALL
        SELECT 'encryption_key', CONVERT(nvarchar(30), column_encryption_key_id), name, 'column_encryption_key'
        FROM sys.column_encryption_keys
        UNION ALL
        SELECT 'encryption_key', CONVERT(nvarchar(30), credential_id), name, 'database_scoped_credential'
        FROM sys.database_scoped_credentials
    "#;
    match mssql_artifact_rows(client, sql, "read SQL Server security prerequisites without key, certificate, identity, or secret material", audit).await {
        Ok(rows) => {
            completeness.catalogs_read.extend([
                "sys.certificates".to_string(),
                "sys.symmetric_keys".to_string(),
                "sys.asymmetric_keys".to_string(),
                "sys.column_master_keys".to_string(),
                "sys.column_encryption_keys".to_string(),
                "sys.database_scoped_credentials".to_string(),
            ]);
            for row in rows {
                let kind = mssql_string(&row, "kind");
                let name = mssql_string(&row, "object_name");
                let native_id = mssql_string(&row, "native_id");
                let mut item = RawArtifact::new(
                    format!("sqlserver|{kind}|{name}|{native_id}"),
                    if kind == "certificate" { "certificate" } else { "encryption_key" },
                    match mssql_string(&row, "subkind").as_str() {
                        "database_certificate" => "database_certificate",
                        "symmetric_key" => "symmetric_key",
                        "asymmetric_key" => "asymmetric_key",
                        "column_master_key" => "column_master_key",
                        "column_encryption_key" => "column_encryption_key",
                        _ => "database_scoped_credential",
                    },
                );
                item.external = Some(RawExternalPrerequisite {
                    class: if kind == "certificate" {
                        "certificate_material"
                    } else {
                        "encryption_or_credential_material"
                    },
                    deployment_scope: "database_and_external_key_store",
                    binary_material: "not_captured",
                    secret_material: "required_not_captured",
                    endpoint_material: "may_be_required_not_captured",
                    compatibility: "target_security_policy_specific",
                });
                out.push(item);
            }
        }
        Err(error) => mssql_catalog_unreadable(
            audit,
            completeness,
            "sys_security_objects",
            "certificates, keys, and credentials",
            &error,
        ),
    }
}

async fn capture_mssql_server_external(
    client: &mut Client<tokio_util::compat::Compat<TcpStream>>,
    audit: &mut AuditLog,
    out: &mut Vec<RawArtifact>,
    completeness: &mut CaptureCompleteness,
) {
    let linked_sql = "SELECT CONVERT(nvarchar(30), server_id) AS native_id, name AS object_name FROM sys.servers WHERE is_linked = 1 ORDER BY server_id";
    match mssql_artifact_rows(
        client,
        linked_sql,
        "read linked-server inventory without provider strings, endpoints, or credentials",
        audit,
    )
    .await
    {
        Ok(rows) => {
            completeness.catalogs_read.push("sys.servers".to_string());
            for row in rows {
                let name = mssql_string(&row, "object_name");
                let native_id = mssql_string(&row, "native_id");
                let mut item = RawArtifact::new(
                    format!("sqlserver|foreign_server|{name}|{native_id}"),
                    "foreign_server",
                    "linked_server",
                );
                item.external = Some(RawExternalPrerequisite::infrastructure(
                    "foreign_endpoint",
                    "server_database_and_network",
                ));
                out.push(item);
            }
        }
        Err(error) => {
            mssql_catalog_unreadable(audit, completeness, "sys.servers", "linked servers", &error)
        }
    }

    let jobs_sql = "SELECT CONVERT(nvarchar(40), job_id) AS native_id, name AS object_name, CONVERT(nvarchar(10), enabled) AS enabled FROM msdb.dbo.sysjobs ORDER BY job_id";
    match mssql_artifact_rows(
        client,
        jobs_sql,
        "read SQL Server Agent job inventory without commands, owners, schedules, or notifications",
        audit,
    )
    .await
    {
        Ok(rows) => {
            completeness
                .catalogs_read
                .push("msdb.dbo.sysjobs".to_string());
            for row in rows {
                let name = mssql_string(&row, "object_name");
                let native_id = mssql_string(&row, "native_id");
                let mut item = RawArtifact::new(
                    format!("sqlserver|scheduled_job|{name}|{native_id}"),
                    "scheduled_job",
                    if mssql_string(&row, "enabled") == "1" {
                        "enabled_agent_job"
                    } else {
                        "disabled_agent_job"
                    },
                );
                item.external = Some(RawExternalPrerequisite::infrastructure(
                    "sqlserver_agent",
                    "server_and_operating_environment",
                ));
                out.push(item);
            }
        }
        Err(error) => mssql_catalog_unreadable(
            audit,
            completeness,
            "msdb.dbo.sysjobs",
            "SQL Server Agent jobs",
            &error,
        ),
    }
}

async fn mssql_artifact_rows(
    client: &mut Client<tokio_util::compat::Compat<TcpStream>>,
    sql: &str,
    summary: &str,
    audit: &mut AuditLog,
) -> Result<Vec<tiberius::Row>> {
    let started = Instant::now();
    let rows = client.simple_query(sql).await?.into_first_result().await?;
    audit.record_query(summary, elapsed_ms(started), rows.len() as u64);
    Ok(rows)
}

fn mssql_string(row: &tiberius::Row, column: &str) -> String {
    row.get::<&str, _>(column).unwrap_or("").to_string()
}

fn mssql_dependency_target_is_table(type_code: &str) -> bool {
    type_code.trim() == "U"
}

fn mssql_artifact_kind(type_code: &str) -> Option<(&'static str, &'static str)> {
    match type_code.trim() {
        "V" => Some(("view", "ordinary")),
        "P" | "RF" => Some(("procedure", "stored_procedure")),
        "PC" => Some(("procedure", "clr_procedure")),
        "FN" => Some(("function", "scalar_function")),
        "IF" => Some(("function", "inline_table_function")),
        "TF" => Some(("function", "table_function")),
        "FS" => Some(("function", "clr_scalar_function")),
        "FT" => Some(("function", "clr_table_function")),
        "AF" => Some(("aggregate", "clr_aggregate")),
        "TR" => Some(("trigger", "table_trigger")),
        "TA" => Some(("trigger", "clr_trigger")),
        "D" => Some(("default", "default_constraint")),
        "C" => Some(("default", "check_constraint")),
        "R" => Some(("rule", "legacy_rule")),
        "SN" => Some(("synonym", "database_synonym")),
        "SO" => Some(("sequence", "integer_sequence")),
        _ => None,
    }
}

fn mssql_catalog_unreadable(
    audit: &mut AuditLog,
    completeness: &mut CaptureCompleteness,
    catalog: &'static str,
    family: &'static str,
    _error: &dyn std::fmt::Display,
) {
    artifacts::record_catalog_unreadable(audit, completeness, catalog, family);
}
