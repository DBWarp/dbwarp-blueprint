async fn capture_artifacts(
    conn: &mut mysql_async::Conn,
    detail: ArtifactDetail,
    engine_version: &str,
    schemas: &SchemaSelection,
    audit: &mut AuditLog,
) -> (Vec<RawArtifact>, CaptureCompleteness) {
    let analyze = detail.reads_definitions();
    let grammar_profile = artifacts::grammar_profile("mysql", engine_version);
    let mut out = Vec::new();
    let mut completeness = CaptureCompleteness {
        visibility: "privilege_filtered".to_string(),
        inventory_complete: false,
        dependencies_complete: false,
        families_not_inventoried: vec![
            "grants".to_string(),
            "roles".to_string(),
            "replication_channels".to_string(),
            "tablespaces".to_string(),
            "encryption_keys".to_string(),
        ],
        ..CaptureCompleteness::default()
    };

    // MySQL information_schema hides objects the current account cannot see.
    // Only claim full visibility when SHOW GRANTS proves a global ALL grant;
    // weaker grants remain useful evidence but cannot prove absence.
    let started = Instant::now();
    match conn.query::<String, _>("SHOW GRANTS").await {
        Ok(grants) => {
            audit.record_query(
                "SHOW GRANTS (artifact visibility probe)",
                elapsed_ms(started),
                grants.len() as u64,
            );
            completeness.catalogs_read.push("show_grants".to_string());
            let global_all = grants.iter().any(|grant| {
                let upper = grant.to_ascii_uppercase();
                upper.contains("GRANT ALL PRIVILEGES ON *.*") || upper.contains("GRANT ALL ON *.*")
            });
            if global_all {
                completeness.visibility = "full".to_string();
                completeness.inventory_complete = true;
            }
        }
        Err(error) => mysql_catalog_unreadable(
            audit,
            &mut completeness,
            "show_grants",
            "visibility probe",
            &error,
        ),
    }

    let definition = if analyze { "v.VIEW_DEFINITION" } else { "NULL" };
    let view_schema_predicate = schemas.and_sql("v.TABLE_SCHEMA");
    let view_sql = format!(
        r#"
        SELECT v.TABLE_SCHEMA, v.TABLE_NAME, {definition} AS definition,
               v.SECURITY_TYPE
        FROM information_schema.VIEWS v
        WHERE v.TABLE_SCHEMA NOT IN ('mysql','information_schema','performance_schema','sys')
          {view_schema_predicate}
        ORDER BY v.TABLE_SCHEMA, v.TABLE_NAME
        "#
    );
    let started = Instant::now();
    match conn
        .query_map(
            view_sql,
            |(schema, name, definition, security): (String, String, Option<String>, String)| {
                (schema, name, definition, security)
            },
        )
        .await
    {
        Ok(rows) => {
            audit.record_query(
                "SELECT views FROM information_schema.VIEWS",
                elapsed_ms(started),
                rows.len() as u64,
            );
            completeness
                .catalogs_read
                .push("information_schema.views".to_string());
            for (schema, name, definition, security) in rows {
                let mut item =
                    RawArtifact::new(mysql_identity("view", &schema, &name), "view", "ordinary");
                item.schema_identity = Some(schema);
                item.security_mode = if security.eq_ignore_ascii_case("DEFINER") {
                    "definer"
                } else {
                    "invoker"
                };
                item.definition_visibility = definition_visibility(analyze, &definition);
                item.analysis = Some(RawLanguageAnalysis {
                    definition: definition.map(Zeroizing::new),
                    dialect: "sql".to_string(),
                    grammar_profile: grammar_profile.clone(),
                    ..RawLanguageAnalysis::default()
                });
                out.push(item);
            }
        }
        Err(error) => mysql_catalog_unreadable(
            audit,
            &mut completeness,
            "information_schema.views",
            "views",
            &error,
        ),
    }

    let definition = if analyze {
        "r.ROUTINE_DEFINITION"
    } else {
        "NULL"
    };
    let routine_sql = format!(
        r#"
        SELECT r.ROUTINE_SCHEMA, r.ROUTINE_NAME, r.ROUTINE_TYPE,
               r.ROUTINE_BODY, {definition} AS definition,
               COALESCE(r.SQL_MODE, ''), r.SECURITY_TYPE
        FROM information_schema.ROUTINES r
        WHERE r.ROUTINE_SCHEMA NOT IN ('mysql','information_schema','performance_schema','sys')
          {}
        ORDER BY r.ROUTINE_SCHEMA, r.ROUTINE_NAME, r.ROUTINE_TYPE
        "#,
        schemas.and_sql("r.ROUTINE_SCHEMA")
    );
    let started = Instant::now();
    match conn
        .query_map(
            routine_sql,
            |(schema, name, routine_type, body, definition, sql_mode, security): (
                String,
                String,
                String,
                String,
                Option<String>,
                String,
                String,
            )| {
                (
                    schema,
                    name,
                    routine_type,
                    body,
                    definition,
                    sql_mode,
                    security,
                )
            },
        )
        .await
    {
        Ok(rows) => {
            audit.record_query(
                "SELECT routines FROM information_schema.ROUTINES",
                elapsed_ms(started),
                rows.len() as u64,
            );
            completeness
                .catalogs_read
                .push("information_schema.routines".to_string());
            for (schema, name, routine_type, body, definition, sql_mode, security) in rows {
                let (kind, subkind) = if routine_type.eq_ignore_ascii_case("PROCEDURE") {
                    ("procedure", "stored_procedure")
                } else {
                    ("function", "stored_function")
                };
                let mut item =
                    RawArtifact::new(mysql_identity(kind, &schema, &name), kind, subkind);
                item.schema_identity = Some(schema);
                item.security_mode = if security.eq_ignore_ascii_case("DEFINER") {
                    "definer"
                } else {
                    "invoker"
                };
                item.definition_visibility = definition_visibility(analyze, &definition);
                item.analysis = Some(RawLanguageAnalysis {
                    definition: definition.map(Zeroizing::new),
                    dialect: if body.eq_ignore_ascii_case("SQL") {
                        "mysql-sql-psm".to_string()
                    } else {
                        "unknown".to_string()
                    },
                    grammar_profile: grammar_profile.clone(),
                    sql_mode_flags: vec![sql_mode],
                    ..RawLanguageAnalysis::default()
                });
                out.push(item);
            }
        }
        Err(error) => mysql_catalog_unreadable(
            audit,
            &mut completeness,
            "information_schema.routines",
            "routines",
            &error,
        ),
    }

    let definition = if analyze {
        "t.ACTION_STATEMENT"
    } else {
        "NULL"
    };
    let trigger_sql = format!(
        r#"
        SELECT t.TRIGGER_SCHEMA, t.TRIGGER_NAME, t.EVENT_OBJECT_SCHEMA,
               t.EVENT_OBJECT_TABLE, {definition} AS definition,
               COALESCE(t.SQL_MODE, ''), t.ACTION_TIMING, t.EVENT_MANIPULATION
        FROM information_schema.TRIGGERS t
        WHERE t.TRIGGER_SCHEMA NOT IN ('mysql','information_schema','performance_schema','sys')
          {}
        ORDER BY t.TRIGGER_SCHEMA, t.TRIGGER_NAME
        "#,
        schemas.and_sql("t.TRIGGER_SCHEMA")
    );
    let started = Instant::now();
    match conn
        .query_map(
            trigger_sql,
            |(schema, name, table_schema, table, definition, sql_mode, timing, event): (
                String,
                String,
                String,
                String,
                Option<String>,
                String,
                String,
                String,
            )| {
                (
                    schema,
                    name,
                    table_schema,
                    table,
                    definition,
                    sql_mode,
                    timing,
                    event,
                )
            },
        )
        .await
    {
        Ok(rows) => {
            audit.record_query(
                "SELECT triggers FROM information_schema.TRIGGERS",
                elapsed_ms(started),
                rows.len() as u64,
            );
            completeness
                .catalogs_read
                .push("information_schema.triggers".to_string());
            for (schema, name, table_schema, table, definition, sql_mode, timing, event) in rows {
                let mut item = RawArtifact::new(
                    mysql_identity("trigger", &schema, &name),
                    "trigger",
                    match (timing.as_str(), event.as_str()) {
                        ("BEFORE", "INSERT") => "before_insert",
                        ("BEFORE", "UPDATE") => "before_update",
                        ("BEFORE", "DELETE") => "before_delete",
                        ("AFTER", "INSERT") => "after_insert",
                        ("AFTER", "UPDATE") => "after_update",
                        ("AFTER", "DELETE") => "after_delete",
                        _ => "table_trigger",
                    },
                );
                item.schema_identity = Some(schema);
                item.parent_table_identity =
                    Some(artifacts::table_identity("mysql", &table_schema, &table));
                item.definition_visibility = definition_visibility(analyze, &definition);
                item.analysis = Some(RawLanguageAnalysis {
                    definition: definition.map(Zeroizing::new),
                    dialect: "mysql-sql-psm".to_string(),
                    grammar_profile: grammar_profile.clone(),
                    sql_mode_flags: vec![sql_mode],
                    ..RawLanguageAnalysis::default()
                });
                out.push(item);
            }
        }
        Err(error) => mysql_catalog_unreadable(
            audit,
            &mut completeness,
            "information_schema.triggers",
            "triggers",
            &error,
        ),
    }

    let definition = if analyze {
        "e.EVENT_DEFINITION"
    } else {
        "NULL"
    };
    let event_sql = format!(
        r#"
        SELECT e.EVENT_SCHEMA, e.EVENT_NAME, {definition} AS definition,
               COALESCE(e.SQL_MODE, ''), e.STATUS
        FROM information_schema.EVENTS e
        WHERE e.EVENT_SCHEMA NOT IN ('mysql','information_schema','performance_schema','sys')
          {}
        ORDER BY e.EVENT_SCHEMA, e.EVENT_NAME
        "#,
        schemas.and_sql("e.EVENT_SCHEMA")
    );
    let started = Instant::now();
    match conn
        .query_map(
            event_sql,
            |(schema, name, definition, sql_mode, status): (
                String,
                String,
                Option<String>,
                String,
                String,
            )| { (schema, name, definition, sql_mode, status) },
        )
        .await
    {
        Ok(rows) => {
            audit.record_query(
                "SELECT scheduled events FROM information_schema.EVENTS",
                elapsed_ms(started),
                rows.len() as u64,
            );
            completeness
                .catalogs_read
                .push("information_schema.events".to_string());
            for (schema, name, definition, sql_mode, status) in rows {
                let mut item = RawArtifact::new(
                    mysql_identity("scheduled_job", &schema, &name),
                    "scheduled_job",
                    if status.eq_ignore_ascii_case("ENABLED") {
                        "enabled_event"
                    } else {
                        "disabled_event"
                    },
                );
                item.schema_identity = Some(schema);
                item.definition_visibility = definition_visibility(analyze, &definition);
                item.analysis = Some(RawLanguageAnalysis {
                    definition: definition.map(Zeroizing::new),
                    dialect: "mysql-sql-psm".to_string(),
                    grammar_profile: grammar_profile.clone(),
                    sql_mode_flags: vec![sql_mode],
                    ..RawLanguageAnalysis::default()
                });
                out.push(item);
            }
        }
        Err(error) => mysql_catalog_unreadable(
            audit,
            &mut completeness,
            "information_schema.events",
            "scheduled events",
            &error,
        ),
    }

    capture_mysql_view_dependencies(conn, schemas, audit, &mut out, &mut completeness).await;
    capture_mysql_external_artifacts(conn, schemas, audit, &mut out, &mut completeness).await;
    (out, completeness)
}

async fn capture_mysql_view_dependencies(
    conn: &mut mysql_async::Conn,
    schemas: &SchemaSelection,
    audit: &mut AuditLog,
    artifacts_out: &mut [RawArtifact],
    completeness: &mut CaptureCompleteness,
) {
    let started = Instant::now();
    let sql = format!(
        r#"
            SELECT VIEW_SCHEMA, VIEW_NAME, TABLE_SCHEMA, TABLE_NAME
            FROM information_schema.VIEW_TABLE_USAGE
            WHERE VIEW_SCHEMA NOT IN ('mysql','information_schema','performance_schema','sys')
              {}
            ORDER BY VIEW_SCHEMA, VIEW_NAME, TABLE_SCHEMA, TABLE_NAME
            "#,
        schemas.and_sql("VIEW_SCHEMA")
    );
    let rows: Result<Vec<(String, String, String, String)>> =
        conn.query_map(sql, |row| row).await.map_err(Into::into);
    match rows {
        Ok(rows) => {
            audit.record_query(
                "SELECT view dependencies FROM information_schema.VIEW_TABLE_USAGE",
                elapsed_ms(started),
                rows.len() as u64,
            );
            completeness
                .catalogs_read
                .push("information_schema.view_table_usage".to_string());
            let mut dependencies: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for (view_schema, view, table_schema, table) in rows {
                dependencies
                    .entry(mysql_identity("view", &view_schema, &view))
                    .or_default()
                    .push(artifacts::table_identity("mysql", &table_schema, &table));
            }
            for item in artifacts_out {
                if let Some(deps) = dependencies.remove(&item.identity) {
                    item.dependencies.extend(deps);
                }
            }
        }
        Err(error) => mysql_catalog_unreadable(
            audit,
            completeness,
            "information_schema.view_table_usage",
            "view dependencies",
            &error,
        ),
    }
}

async fn capture_mysql_external_artifacts(
    conn: &mut mysql_async::Conn,
    schemas: &SchemaSelection,
    audit: &mut AuditLog,
    out: &mut Vec<RawArtifact>,
    completeness: &mut CaptureCompleteness,
) {
    let started = Instant::now();
    let federated_sql = format!(
        r#"
            SELECT TABLE_SCHEMA, TABLE_NAME
            FROM information_schema.TABLES
            WHERE ENGINE = 'FEDERATED'
              AND TABLE_SCHEMA NOT IN ('mysql','information_schema','performance_schema','sys')
              {}
            ORDER BY TABLE_SCHEMA, TABLE_NAME
            "#,
        schemas.and_sql("TABLE_SCHEMA")
    );
    let federated: Result<Vec<(String, String)>> = conn
        .query_map(federated_sql, |row| row)
        .await
        .map_err(Into::into);
    match federated {
        Ok(rows) => {
            audit.record_query(
                "SELECT FEDERATED external tables FROM information_schema.TABLES (connection strings not selected)",
                elapsed_ms(started),
                rows.len() as u64,
            );
            for (schema, table) in rows {
                let mut item = RawArtifact::new(
                    mysql_identity("external_table", &schema, &table),
                    "external_table",
                    "federated_table",
                );
                item.schema_identity = Some(schema.clone());
                item.parent_table_identity =
                    Some(artifacts::table_identity("mysql", &schema, &table));
                item.external = Some(RawExternalPrerequisite::infrastructure(
                    "foreign_endpoint",
                    "network_and_database",
                ));
                out.push(item);
            }
        }
        Err(error) => mysql_catalog_unreadable(
            audit,
            completeness,
            "information_schema.tables",
            "FEDERATED external tables",
            &error,
        ),
    }

    let started = Instant::now();
    let udf_rows: Result<Vec<(String, String)>> = conn
        .query_map(
            "SELECT UDF_NAME, UDF_TYPE FROM performance_schema.user_defined_functions ORDER BY UDF_NAME",
            |row| row,
        )
        .await
        .map_err(Into::into);
    match udf_rows {
        Ok(rows) => {
            audit.record_query(
                "SELECT loadable UDF inventory FROM performance_schema.user_defined_functions (library names not selected)",
                elapsed_ms(started),
                rows.len() as u64,
            );
            completeness
                .catalogs_read
                .push("performance_schema.user_defined_functions".to_string());
            for (name, udf_type) in rows {
                let mut item = RawArtifact::new(
                    format!("mysql|function|loadable|{name}"),
                    if udf_type.eq_ignore_ascii_case("AGGREGATE") {
                        "aggregate"
                    } else {
                        "function"
                    },
                    "loadable_udf",
                );
                item.definition_visibility = "external_binary";
                item.external = Some(RawExternalPrerequisite::package(
                    "mysql_loadable_udf",
                    "source_server_abi",
                ));
                out.push(item);
            }
        }
        Err(error) => mysql_catalog_unreadable(
            audit,
            completeness,
            "performance_schema.user_defined_functions",
            "loadable UDFs",
            &error,
        ),
    }
}

fn mysql_identity(kind: &str, schema: &str, name: &str) -> String {
    format!("mysql|{kind}|{schema}|{name}")
}

fn definition_visibility(analyze: bool, definition: &Option<String>) -> &'static str {
    if !analyze {
        "not_read"
    } else if definition.is_some() {
        "available"
    } else {
        "withheld"
    }
}

fn mysql_catalog_unreadable(
    audit: &mut AuditLog,
    completeness: &mut CaptureCompleteness,
    catalog: &'static str,
    family: &'static str,
    _error: &dyn std::fmt::Display,
) {
    artifacts::record_catalog_unreadable(audit, completeness, catalog, family);
}
