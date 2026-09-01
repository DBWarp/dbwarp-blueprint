/// Capture non-table source facts. Each catalog family is isolated so one
/// privilege-restricted optional catalog cannot erase otherwise visible
/// evidence. Definitions are requested only for `analyzed` and discarded
/// after the bounded language census is produced.
async fn capture_artifacts(
    client: &tokio_postgres::Client,
    detail: ArtifactDetail,
    engine_version: &str,
    schemas: &SchemaSelection,
    audit: &mut AuditLog,
) -> (Vec<RawArtifact>, CaptureCompleteness) {
    let analyze = detail.reads_definitions();
    let grammar_profile = artifacts::grammar_profile("postgresql", engine_version);
    let mut out = Vec::new();
    let mut completeness = CaptureCompleteness {
        visibility: "full".to_string(),
        inventory_complete: true,
        // PostgreSQL does not persist every dependency for string-bodied SQL
        // and procedural routines, so a complete dependency claim would be
        // false even with unrestricted catalogs.
        dependencies_complete: false,
        families_not_inventoried: vec![
            "grants".to_string(),
            "roles".to_string(),
            "large_objects".to_string(),
            "operator_classes".to_string(),
        ],
        ..CaptureCompleteness::default()
    };

    let started = Instant::now();
    let class_definition = if analyze {
        "CASE WHEN c.relkind IN ('v','m') THEN pg_get_viewdef(c.oid, true) ELSE NULL END"
    } else {
        "NULL::text"
    };
    let schema_predicate = schemas.and_sql("n.nspname");
    let class_sql = format!(
        r#"
        SELECT c.oid::text AS native_id,
               n.nspname AS schema_name,
               c.relname AS object_name,
               c.relkind::text AS relkind,
               {class_definition} AS definition
        FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE c.relkind IN ('v','m','S')
          AND n.nspname NOT IN ('pg_catalog','information_schema')
          AND n.nspname !~ '^pg_toast'
          {schema_predicate}
        ORDER BY c.oid
        "#
    );
    match client.query(&class_sql, &[]).await {
        Ok(rows) => {
            audit.record_query(
                "SELECT views, materialized views, and sequences FROM pg_class",
                elapsed_ms(started),
                rows.len() as u64,
            );
            completeness.catalogs_read.push("pg_class".to_string());
            for row in rows {
                let native_id: String = row.get("native_id");
                let schema: String = row.get("schema_name");
                let name: String = row.get("object_name");
                let relkind: String = row.get("relkind");
                let (kind, subkind) = match relkind.as_str() {
                    "v" => ("view", "ordinary"),
                    "m" => ("materialized_view", "materialized"),
                    "S" => ("sequence", "integer_sequence"),
                    _ => continue,
                };
                let mut item = RawArtifact::new(
                    format!("postgresql|{kind}|{schema}|{name}|{native_id}"),
                    kind,
                    subkind,
                );
                item.schema_identity = Some(schema);
                if matches!(kind, "view" | "materialized_view") {
                    let definition: Option<String> = row.get("definition");
                    item.definition_visibility = if analyze {
                        if definition.is_some() {
                            "available"
                        } else {
                            "withheld"
                        }
                    } else {
                        "not_read"
                    };
                    item.analysis = Some(RawLanguageAnalysis {
                        definition: definition.map(Zeroizing::new),
                        dialect: "sql".to_string(),
                        grammar_profile: grammar_profile.clone(),
                        ..RawLanguageAnalysis::default()
                    });
                }
                out.push(item);
            }
        }
        Err(error) => catalog_unreadable(
            audit,
            &mut completeness,
            "pg_class",
            "views/sequences",
            &error,
        ),
    }

    let started = Instant::now();
    let routine_definition = if analyze {
        "CASE WHEN p.prokind IN ('f','p') AND l.lanname NOT IN ('c','internal') THEN pg_get_functiondef(p.oid) ELSE NULL END"
    } else {
        "NULL::text"
    };
    let routine_sql = format!(
        r#"
        SELECT p.oid::text AS native_id,
               n.nspname AS schema_name,
               p.proname AS object_name,
               pg_get_function_identity_arguments(p.oid) AS identity_args,
               p.prokind::text AS prokind,
               l.lanname AS language_name,
               p.prosecdef,
               {routine_definition} AS definition
        FROM pg_proc p
        JOIN pg_namespace n ON n.oid = p.pronamespace
        JOIN pg_language l ON l.oid = p.prolang
        WHERE n.nspname NOT IN ('pg_catalog','information_schema')
          AND n.nspname !~ '^pg_toast'
          {schema_predicate}
          AND NOT EXISTS (
              SELECT 1 FROM pg_depend d
              WHERE d.classid = 'pg_proc'::regclass
                AND d.objid = p.oid
                AND d.deptype = 'e'
          )
        ORDER BY p.oid
        "#
    );
    match client.query(&routine_sql, &[]).await {
        Ok(rows) => {
            audit.record_query(
                "SELECT routines and aggregates FROM pg_proc",
                elapsed_ms(started),
                rows.len() as u64,
            );
            completeness.catalogs_read.push("pg_proc".to_string());
            for row in rows {
                let native_id: String = row.get("native_id");
                let schema: String = row.get("schema_name");
                let name: String = row.get("object_name");
                let args: String = row.get("identity_args");
                let prokind: String = row.get("prokind");
                let language: String = row.get("language_name");
                let (kind, subkind) = match prokind.as_str() {
                    "p" => ("procedure", "stored_procedure"),
                    "a" | "w" => ("aggregate", "user_defined_aggregate"),
                    _ => ("function", "stored_function"),
                };
                let mut item = RawArtifact::new(
                    format!("postgresql|{kind}|{schema}|{name}|{args}|{native_id}"),
                    kind,
                    subkind,
                );
                item.schema_identity = Some(schema);
                item.security_mode = if row.get::<_, bool>("prosecdef") {
                    "definer"
                } else {
                    "invoker"
                };
                let language_lower = language.to_ascii_lowercase();
                if matches!(language_lower.as_str(), "c" | "internal") {
                    item.external = Some(RawExternalPrerequisite::package(
                        "postgresql_native_function",
                        "source_server_abi",
                    ));
                }
                let definition: Option<String> = row.get("definition");
                item.definition_visibility = if analyze {
                    if definition.is_some() {
                        "available"
                    } else {
                        "unavailable"
                    }
                } else {
                    "not_read"
                };
                item.analysis = Some(RawLanguageAnalysis {
                    definition: definition.map(Zeroizing::new),
                    dialect: language_lower,
                    grammar_profile: grammar_profile.clone(),
                    ..RawLanguageAnalysis::default()
                });
                out.push(item);
            }
        }
        Err(error) => catalog_unreadable(audit, &mut completeness, "pg_proc", "routines", &error),
    }

    let started = Instant::now();
    let type_sql = format!(
        r#"
        SELECT t.oid::text AS native_id,
               n.nspname AS schema_name,
               t.typname AS object_name,
               t.typtype::text AS typtype
        FROM pg_type t
        JOIN pg_namespace n ON n.oid = t.typnamespace
        LEFT JOIN pg_class c ON c.oid = t.typrelid
        WHERE n.nspname NOT IN ('pg_catalog','information_schema')
          AND n.nspname !~ '^pg_toast'
          {schema_predicate}
          AND t.typtype IN ('e','d','c','r')
          AND (t.typrelid = 0 OR c.relkind = 'c')
          AND NOT EXISTS (
              SELECT 1 FROM pg_depend d
              WHERE d.classid = 'pg_type'::regclass
                AND d.objid = t.oid
                AND d.deptype = 'e'
          )
        ORDER BY t.oid
    "#
    );
    match client.query(&type_sql, &[]).await {
        Ok(rows) => {
            audit.record_query(
                "SELECT enum, domain, composite, and range types FROM pg_type",
                elapsed_ms(started),
                rows.len() as u64,
            );
            completeness.catalogs_read.push("pg_type".to_string());
            for row in rows {
                let schema: String = row.get("schema_name");
                let name: String = row.get("object_name");
                let native_id: String = row.get("native_id");
                let typtype: String = row.get("typtype");
                let subkind = match typtype.as_str() {
                    "e" => "enum",
                    "d" => "domain",
                    "c" => "composite",
                    "r" => "range",
                    _ => "other",
                };
                let mut item = RawArtifact::new(
                    format!("postgresql|type|{schema}|{name}|{native_id}"),
                    "type",
                    subkind,
                );
                item.schema_identity = Some(schema);
                out.push(item);
            }
        }
        Err(error) => catalog_unreadable(audit, &mut completeness, "pg_type", "types", &error),
    }

    let started = Instant::now();
    let trigger_definition = if analyze {
        "pg_get_triggerdef(t.oid, true)"
    } else {
        "NULL::text"
    };
    let trigger_sql = format!(
        r#"
        SELECT t.oid::text AS native_id,
               n.nspname AS schema_name,
               c.relname AS table_name,
               t.tgname AS object_name,
               {trigger_definition} AS definition
        FROM pg_trigger t
        JOIN pg_class c ON c.oid = t.tgrelid
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE NOT t.tgisinternal
          AND n.nspname NOT IN ('pg_catalog','information_schema')
          {schema_predicate}
        ORDER BY t.oid
        "#
    );
    match client.query(&trigger_sql, &[]).await {
        Ok(rows) => {
            audit.record_query(
                "SELECT user triggers FROM pg_trigger",
                elapsed_ms(started),
                rows.len() as u64,
            );
            completeness.catalogs_read.push("pg_trigger".to_string());
            for row in rows {
                let schema: String = row.get("schema_name");
                let table: String = row.get("table_name");
                let name: String = row.get("object_name");
                let native_id: String = row.get("native_id");
                let definition: Option<String> = row.get("definition");
                let mut item = RawArtifact::new(
                    format!("postgresql|trigger|{schema}|{table}|{name}|{native_id}"),
                    "trigger",
                    "table_trigger",
                );
                item.schema_identity = Some(schema.clone());
                item.parent_table_identity =
                    Some(artifacts::table_identity("postgresql", &schema, &table));
                item.definition_visibility = if analyze { "available" } else { "not_read" };
                item.analysis = Some(RawLanguageAnalysis {
                    definition: definition.map(Zeroizing::new),
                    dialect: "sql".to_string(),
                    grammar_profile: grammar_profile.clone(),
                    ..RawLanguageAnalysis::default()
                });
                out.push(item);
            }
        }
        Err(error) => {
            catalog_unreadable(audit, &mut completeness, "pg_trigger", "triggers", &error)
        }
    }

    let started = Instant::now();
    let default_definition = if analyze {
        "pg_get_expr(ad.adbin, ad.adrelid, true)"
    } else {
        "NULL::text"
    };
    let check_definition = if analyze {
        "pg_get_constraintdef(con.oid, true)"
    } else {
        "NULL::text"
    };
    let expression_sql = format!(
        r#"
        SELECT 'default'::text AS kind,
               ad.oid::text AS native_id,
               n.nspname AS schema_name,
               c.relname AS table_name,
               a.attname AS object_name,
               CASE WHEN a.attgenerated <> '' THEN 'generated_column' ELSE 'column_default' END AS subkind,
               {default_definition} AS definition
        FROM pg_attrdef ad
        JOIN pg_attribute a ON a.attrelid = ad.adrelid AND a.attnum = ad.adnum
        JOIN pg_class c ON c.oid = ad.adrelid
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname NOT IN ('pg_catalog','information_schema')
          {schema_predicate}
        UNION ALL
        SELECT 'default'::text AS kind,
               con.oid::text AS native_id,
               n.nspname AS schema_name,
               c.relname AS table_name,
               con.conname AS object_name,
               'check_constraint'::text AS subkind,
               {check_definition} AS definition
        FROM pg_constraint con
        JOIN pg_class c ON c.oid = con.conrelid
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE con.contype = 'c'
          AND n.nspname NOT IN ('pg_catalog','information_schema')
          {schema_predicate}
        "#
    );
    match client.query(&expression_sql, &[]).await {
        Ok(rows) => {
            audit.record_query(
                "SELECT defaults, generated expressions, and checks FROM pg_attrdef/pg_constraint",
                elapsed_ms(started),
                rows.len() as u64,
            );
            completeness.catalogs_read.push("pg_attrdef".to_string());
            completeness.catalogs_read.push("pg_constraint".to_string());
            for row in rows {
                let schema: String = row.get("schema_name");
                let table: String = row.get("table_name");
                let name: String = row.get("object_name");
                let native_id: String = row.get("native_id");
                let subkind: String = row.get("subkind");
                let definition: Option<String> = row.get("definition");
                let mut item = RawArtifact::new(
                    format!("postgresql|default|{schema}|{table}|{name}|{native_id}"),
                    "default",
                    match subkind.as_str() {
                        "generated_column" => "generated_column",
                        "check_constraint" => "check_constraint",
                        _ => "column_default",
                    },
                );
                item.schema_identity = Some(schema.clone());
                item.parent_table_identity =
                    Some(artifacts::table_identity("postgresql", &schema, &table));
                item.definition_visibility = if analyze { "available" } else { "not_read" };
                item.analysis = Some(RawLanguageAnalysis {
                    definition: definition.map(Zeroizing::new),
                    dialect: "sql".to_string(),
                    grammar_profile: grammar_profile.clone(),
                    ..RawLanguageAnalysis::default()
                });
                out.push(item);
            }
        }
        Err(error) => catalog_unreadable(
            audit,
            &mut completeness,
            "pg_defaults_constraints",
            "defaults/checks",
            &error,
        ),
    }

    let started = Instant::now();
    let policy_definition = if analyze {
        "concat_ws(' ', pg_get_expr(p.polqual, p.polrelid, true), pg_get_expr(p.polwithcheck, p.polrelid, true))"
    } else {
        "NULL::text"
    };
    let rule_definition = if analyze {
        "pg_get_ruledef(r.oid, true)"
    } else {
        "NULL::text"
    };
    let policy_rule_sql = format!(
        r#"
        SELECT 'policy'::text AS kind,
               p.oid::text AS native_id,
               n.nspname AS schema_name,
               c.relname AS table_name,
               p.polname AS object_name,
               {policy_definition} AS definition
        FROM pg_policy p
        JOIN pg_class c ON c.oid = p.polrelid
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname NOT IN ('pg_catalog','information_schema')
          {schema_predicate}
        UNION ALL
        SELECT 'rule'::text AS kind,
               r.oid::text AS native_id,
               n.nspname AS schema_name,
               c.relname AS table_name,
               r.rulename AS object_name,
               {rule_definition} AS definition
        FROM pg_rewrite r
        JOIN pg_class c ON c.oid = r.ev_class
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE r.rulename <> '_RETURN'
          AND n.nspname NOT IN ('pg_catalog','information_schema')
          {schema_predicate}
        "#
    );
    match client.query(&policy_rule_sql, &[]).await {
        Ok(rows) => {
            audit.record_query(
                "SELECT row policies and rules FROM pg_policy/pg_rewrite",
                elapsed_ms(started),
                rows.len() as u64,
            );
            completeness.catalogs_read.push("pg_policy".to_string());
            completeness.catalogs_read.push("pg_rewrite".to_string());
            for row in rows {
                let kind: String = row.get("kind");
                let schema: String = row.get("schema_name");
                let table: String = row.get("table_name");
                let name: String = row.get("object_name");
                let native_id: String = row.get("native_id");
                let definition: Option<String> = row.get("definition");
                let kind_static = if kind == "policy" { "policy" } else { "rule" };
                let mut item = RawArtifact::new(
                    format!("postgresql|{kind_static}|{schema}|{table}|{name}|{native_id}"),
                    kind_static,
                    if kind_static == "policy" {
                        "row_security"
                    } else {
                        "rewrite_rule"
                    },
                );
                item.schema_identity = Some(schema.clone());
                item.parent_table_identity =
                    Some(artifacts::table_identity("postgresql", &schema, &table));
                item.definition_visibility = if analyze { "available" } else { "not_read" };
                item.analysis = Some(RawLanguageAnalysis {
                    definition: definition.map(Zeroizing::new),
                    dialect: "sql".to_string(),
                    grammar_profile: grammar_profile.clone(),
                    ..RawLanguageAnalysis::default()
                });
                out.push(item);
            }
        }
        Err(error) => catalog_unreadable(
            audit,
            &mut completeness,
            "pg_policies_rewrites",
            "policies/rules",
            &error,
        ),
    }

    let started = Instant::now();
    let event_trigger_sql =
        "SELECT oid::text AS native_id, evtname AS object_name FROM pg_event_trigger ORDER BY oid";
    match client.query(event_trigger_sql, &[]).await {
        Ok(rows) => {
            audit.record_query(
                "SELECT database event triggers FROM pg_event_trigger",
                elapsed_ms(started),
                rows.len() as u64,
            );
            completeness
                .catalogs_read
                .push("pg_event_trigger".to_string());
            for row in rows {
                let native_id: String = row.get("native_id");
                let name: String = row.get("object_name");
                out.push(RawArtifact::new(
                    format!("postgresql|event_trigger|{name}|{native_id}"),
                    "event_trigger",
                    "ddl_event_trigger",
                ));
            }
        }
        Err(error) => catalog_unreadable(
            audit,
            &mut completeness,
            "pg_event_trigger",
            "event triggers",
            &error,
        ),
    }

    let started = Instant::now();
    let extension_sql = "SELECT oid::text AS native_id, extname AS object_name FROM pg_extension WHERE extname <> 'plpgsql' ORDER BY oid";
    match client.query(extension_sql, &[]).await {
        Ok(rows) => {
            audit.record_query(
                "SELECT installed extensions FROM pg_extension",
                elapsed_ms(started),
                rows.len() as u64,
            );
            completeness.catalogs_read.push("pg_extension".to_string());
            for row in rows {
                let native_id: String = row.get("native_id");
                let name: String = row.get("object_name");
                let mut item = RawArtifact::new(
                    format!("postgresql|extension|{name}|{native_id}"),
                    "extension",
                    "server_extension",
                );
                item.external = Some(RawExternalPrerequisite::package(
                    "postgresql_extension",
                    "target_compatible_package",
                ));
                out.push(item);
            }
        }
        Err(error) => catalog_unreadable(
            audit,
            &mut completeness,
            "pg_extension",
            "extensions",
            &error,
        ),
    }

    capture_pg_external_catalogs(
        client,
        engine_version,
        schemas,
        audit,
        &mut out,
        &mut completeness,
    )
    .await;
    capture_pg_dependencies(client, schemas, audit, &mut out, &mut completeness).await;
    (out, completeness)
}

async fn capture_pg_external_catalogs(
    client: &tokio_postgres::Client,
    engine_version: &str,
    schemas: &SchemaSelection,
    audit: &mut AuditLog,
    out: &mut Vec<RawArtifact>,
    completeness: &mut CaptureCompleteness,
) {
    let started = Instant::now();
    match client
        .query(
            "SELECT oid::text AS native_id, srvname AS object_name FROM pg_foreign_server ORDER BY oid",
            &[],
        )
        .await
    {
        Ok(rows) => {
            audit.record_query(
                "SELECT foreign servers FROM pg_foreign_server (names used transiently for anonymization)",
                elapsed_ms(started),
                rows.len() as u64,
            );
            completeness.catalogs_read.push("pg_foreign_server".to_string());
            for row in rows {
                let native_id: String = row.get("native_id");
                let name: String = row.get("object_name");
                let mut item = RawArtifact::new(
                    format!("postgresql|foreign_server|{name}|{native_id}"),
                    "foreign_server",
                    "foreign_data_wrapper_server",
                );
                item.external = Some(RawExternalPrerequisite::infrastructure(
                    "foreign_endpoint",
                    "network_and_database",
                ));
                out.push(item);
            }
        }
        Err(error) => catalog_unreadable(
            audit,
            completeness,
            "pg_foreign_server",
            "foreign servers",
            &error,
        ),
    }

    let started = Instant::now();
    let foreign_table_sql = format!(
        r#"
            SELECT c.oid::text AS native_id,
                   n.nspname AS schema_name,
                   c.relname AS object_name
            FROM pg_foreign_table ft
            JOIN pg_class c ON c.oid = ft.ftrelid
            JOIN pg_namespace n ON n.oid = c.relnamespace
            WHERE n.nspname NOT IN ('pg_catalog','information_schema')
              AND n.nspname !~ '^pg_toast'
              {}
            ORDER BY c.oid
            "#,
        schemas.and_sql("n.nspname")
    );
    match client.query(&foreign_table_sql, &[]).await {
        Ok(rows) => {
            audit.record_query(
                "SELECT foreign tables FROM pg_foreign_table (options and endpoints not selected)",
                elapsed_ms(started),
                rows.len() as u64,
            );
            completeness
                .catalogs_read
                .push("pg_foreign_table".to_string());
            for row in rows {
                let native_id: String = row.get("native_id");
                let schema: String = row.get("schema_name");
                let name: String = row.get("object_name");
                let mut item = RawArtifact::new(
                    format!("postgresql|external_table|{schema}|{name}|{native_id}"),
                    "external_table",
                    "foreign_table",
                );
                item.schema_identity = Some(schema.clone());
                item.parent_table_identity =
                    Some(artifacts::table_identity("postgresql", &schema, &name));
                item.external = Some(RawExternalPrerequisite::infrastructure(
                    "foreign_endpoint",
                    "network_database_or_external_storage",
                ));
                out.push(item);
            }
        }
        Err(error) => catalog_unreadable(
            audit,
            completeness,
            "pg_foreign_table",
            "foreign tables",
            &error,
        ),
    }

    let started = Instant::now();
    match client
        .query(
            "SELECT oid::text AS native_id, pubname AS object_name FROM pg_publication ORDER BY oid",
            &[],
        )
        .await
    {
        Ok(rows) => {
            audit.record_query(
                "SELECT logical replication publications FROM pg_publication",
                elapsed_ms(started),
                rows.len() as u64,
            );
            completeness.catalogs_read.push("pg_publication".to_string());
            for row in rows {
                let native_id: String = row.get("native_id");
                let name: String = row.get("object_name");
                let mut item = RawArtifact::new(
                    format!("postgresql|publication|{name}|{native_id}"),
                    "publication",
                    "logical_replication_publication",
                );
                item.external = Some(RawExternalPrerequisite::infrastructure(
                    "replication_topology",
                    "database_and_network",
                ));
                out.push(item);
            }
        }
        Err(error) => catalog_unreadable(
            audit,
            completeness,
            "pg_publication",
            "publications",
            &error,
        ),
    }

    let started = Instant::now();
    let subscription_sql = pg_subscription_inventory_sql(engine_version);
    match client.query(subscription_sql, &[]).await {
        Ok(rows) => {
            audit.record_query(
                "SELECT logical replication subscriptions from pg_subscription (connection data not selected)",
                elapsed_ms(started),
                rows.len() as u64,
            );
            completeness
                .catalogs_read
                .push("pg_subscription".to_string());
            for row in rows {
                let native_id: String = row.get("native_id");
                let name: String = row.get("object_name");
                let mut item = RawArtifact::new(
                    format!("postgresql|subscription|{name}|{native_id}"),
                    "subscription",
                    "logical_replication_subscription",
                );
                item.external = Some(RawExternalPrerequisite::infrastructure(
                    "replication_topology",
                    "database_and_network",
                ));
                out.push(item);
            }
        }
        Err(error) => catalog_unreadable(
            audit,
            completeness,
            "pg_subscription",
            "subscriptions",
            &error,
        ),
    }

    let started = Instant::now();
    match client
        .query(
            "SELECT oid::text AS native_id, spcname AS object_name FROM pg_tablespace WHERE spcname NOT IN ('pg_default','pg_global') ORDER BY oid",
            &[],
        )
        .await
    {
        Ok(rows) => {
            audit.record_query(
                "SELECT non-default physical placement from pg_tablespace (locations not selected)",
                elapsed_ms(started),
                rows.len() as u64,
            );
            completeness.catalogs_read.push("pg_tablespace".to_string());
            for row in rows {
                let native_id: String = row.get("native_id");
                let name: String = row.get("object_name");
                let mut item = RawArtifact::new(
                    format!("postgresql|physical_placement|{name}|{native_id}"),
                    "physical_placement",
                    "tablespace",
                );
                item.external = Some(RawExternalPrerequisite::infrastructure(
                    "physical_storage",
                    "host_or_managed_service",
                ));
                out.push(item);
            }
        }
        Err(error) => catalog_unreadable(
            audit,
            completeness,
            "pg_tablespace",
            "tablespaces",
            &error,
        ),
    }
}

async fn capture_pg_dependencies(
    client: &tokio_postgres::Client,
    schemas: &SchemaSelection,
    audit: &mut AuditLog,
    out: &mut [RawArtifact],
    completeness: &mut CaptureCompleteness,
) {
    // Restrict source addresses in SQL so a Blueprint run does not pull the whole
    // cluster's dependency catalog across the connection. View dependencies
    // belong to their generated _RETURN rule, so normalize those addresses
    // back to the owning pg_class object before matching anonymous artifacts.
    let schema_predicate = schemas.and_sql("n.nspname");
    let sql = format!(
        r#"
        WITH selected_source(source_catalog, source_id, classid, objid) AS (
            SELECT 'pg_class', c.oid, 'pg_class'::regclass, c.oid
            FROM pg_class c
            JOIN pg_namespace n ON n.oid = c.relnamespace
            WHERE c.relkind IN ('v','m','S','f')
              AND n.nspname NOT IN ('pg_catalog','information_schema')
              AND n.nspname !~ '^pg_toast'
              {schema_predicate}
            UNION ALL
            SELECT 'pg_proc', p.oid, 'pg_proc'::regclass, p.oid
            FROM pg_proc p
            JOIN pg_namespace n ON n.oid = p.pronamespace
            WHERE n.nspname NOT IN ('pg_catalog','information_schema')
              AND n.nspname !~ '^pg_toast'
              {schema_predicate}
            UNION ALL
            SELECT 'pg_type', t.oid, 'pg_type'::regclass, t.oid
            FROM pg_type t
            JOIN pg_namespace n ON n.oid = t.typnamespace
            WHERE t.typtype IN ('e','d','c','r')
              AND n.nspname NOT IN ('pg_catalog','information_schema')
              AND n.nspname !~ '^pg_toast'
              {schema_predicate}
            UNION ALL
            SELECT 'pg_trigger', t.oid, 'pg_trigger'::regclass, t.oid
            FROM pg_trigger t
            JOIN pg_class c ON c.oid = t.tgrelid
            JOIN pg_namespace n ON n.oid = c.relnamespace
            WHERE NOT t.tgisinternal
              AND n.nspname NOT IN ('pg_catalog','information_schema')
              {schema_predicate}
            UNION ALL
            SELECT 'pg_attrdef', ad.oid, 'pg_attrdef'::regclass, ad.oid
            FROM pg_attrdef ad
            JOIN pg_class c ON c.oid = ad.adrelid
            JOIN pg_namespace n ON n.oid = c.relnamespace
            WHERE n.nspname NOT IN ('pg_catalog','information_schema')
              {schema_predicate}
            UNION ALL
            SELECT 'pg_constraint', con.oid, 'pg_constraint'::regclass, con.oid
            FROM pg_constraint con
            JOIN pg_class c ON c.oid = con.conrelid
            JOIN pg_namespace n ON n.oid = c.relnamespace
            WHERE con.contype = 'c'
              AND n.nspname NOT IN ('pg_catalog','information_schema')
              {schema_predicate}
            UNION ALL
            SELECT 'pg_policy', p.oid, 'pg_policy'::regclass, p.oid
            FROM pg_policy p
            JOIN pg_class c ON c.oid = p.polrelid
            JOIN pg_namespace n ON n.oid = c.relnamespace
            WHERE n.nspname NOT IN ('pg_catalog','information_schema')
              {schema_predicate}
            UNION ALL
            SELECT 'pg_rewrite', r.oid, 'pg_rewrite'::regclass, r.oid
            FROM pg_rewrite r
            JOIN pg_class c ON c.oid = r.ev_class
            JOIN pg_namespace n ON n.oid = c.relnamespace
            WHERE r.rulename <> '_RETURN'
              AND n.nspname NOT IN ('pg_catalog','information_schema')
              {schema_predicate}
            UNION ALL
            SELECT 'pg_class', c.oid, 'pg_rewrite'::regclass, r.oid
            FROM pg_rewrite r
            JOIN pg_class c ON c.oid = r.ev_class
            JOIN pg_namespace n ON n.oid = c.relnamespace
            WHERE r.rulename = '_RETURN'
              AND c.relkind IN ('v','m')
              AND n.nspname NOT IN ('pg_catalog','information_schema')
              {schema_predicate}
            UNION ALL
            SELECT 'pg_event_trigger', e.oid, 'pg_event_trigger'::regclass, e.oid
            FROM pg_event_trigger e
            UNION ALL
            SELECT 'pg_extension', e.oid, 'pg_extension'::regclass, e.oid
            FROM pg_extension e WHERE e.extname <> 'plpgsql'
            UNION ALL
            SELECT 'pg_foreign_server', s.oid, 'pg_foreign_server'::regclass, s.oid
            FROM pg_foreign_server s
        ), dependency_edges AS (
            SELECT ss.source_catalog,
                   ss.source_id::text,
                   CASE d.refclassid
                     WHEN 'pg_class'::regclass THEN 'pg_class'
                     WHEN 'pg_proc'::regclass THEN 'pg_proc'
                     WHEN 'pg_type'::regclass THEN 'pg_type'
                     WHEN 'pg_trigger'::regclass THEN 'pg_trigger'
                     WHEN 'pg_attrdef'::regclass THEN 'pg_attrdef'
                     WHEN 'pg_constraint'::regclass THEN 'pg_constraint'
                     WHEN 'pg_policy'::regclass THEN 'pg_policy'
                     WHEN 'pg_rewrite'::regclass THEN 'pg_rewrite'
                     WHEN 'pg_event_trigger'::regclass THEN 'pg_event_trigger'
                     WHEN 'pg_extension'::regclass THEN 'pg_extension'
                     WHEN 'pg_foreign_server'::regclass THEN 'pg_foreign_server'
                     ELSE '' END AS target_catalog,
                   d.refobjid::text AS target_id,
                   COALESCE(rn.nspname, '') AS target_schema,
                   COALESCE(rc.relname, '') AS target_name,
                   COALESCE(rc.relkind::text, '') AS target_relkind
            FROM selected_source ss
            JOIN pg_depend d ON d.classid = ss.classid AND d.objid = ss.objid
            LEFT JOIN pg_class rc
              ON d.refclassid = 'pg_class'::regclass AND rc.oid = d.refobjid
            LEFT JOIN pg_namespace rn ON rn.oid = rc.relnamespace
            UNION ALL
            SELECT 'pg_publication', p.oid::text, 'pg_class', c.oid::text,
                   n.nspname, c.relname, c.relkind::text
            FROM pg_publication_rel pr
            JOIN pg_publication p ON p.oid = pr.prpubid
            JOIN pg_class c ON c.oid = pr.prrelid
            JOIN pg_namespace n ON n.oid = c.relnamespace
            WHERE TRUE {schema_predicate}
        )
        SELECT DISTINCT source_catalog, source_id, target_catalog, target_id,
                        target_schema, target_name, target_relkind
        FROM dependency_edges
        WHERE target_catalog <> ''
        ORDER BY source_catalog, source_id, target_catalog, target_id
    "#
    );
    let started = Instant::now();
    match client.query(&sql, &[]).await {
        Ok(rows) => {
            audit.record_query(
                "SELECT modeled artifact dependencies from pg_depend/pg_rewrite/pg_publication_rel",
                elapsed_ms(started),
                rows.len() as u64,
            );
            completeness.catalogs_read.push("pg_depend".to_string());
            completeness
                .catalogs_read
                .push("pg_publication_rel".to_string());

            let address_to_identity: BTreeMap<(String, String), String> = out
                .iter()
                .filter_map(|item| {
                    pg_artifact_catalog(item.kind, item.subkind).and_then(|catalog| {
                        item.identity.rsplit('|').next().map(|native_id| {
                            (
                                (catalog.to_string(), native_id.to_string()),
                                item.identity.clone(),
                            )
                        })
                    })
                })
                .collect();
            let mut dependencies: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for row in rows {
                let source_catalog: String = row.get("source_catalog");
                let source_id: String = row.get("source_id");
                let Some(source_identity) = address_to_identity
                    .get(&(source_catalog, source_id))
                    .cloned()
                else {
                    continue;
                };
                let target_catalog: String = row.get("target_catalog");
                let target_id: String = row.get("target_id");
                let target_schema: String = row.get("target_schema");
                let target_name: String = row.get("target_name");
                let target_relkind: String = row.get("target_relkind");
                let target = address_to_identity
                    .get(&(target_catalog.clone(), target_id))
                    .cloned()
                    .or_else(|| {
                        (target_catalog == "pg_class"
                            && matches!(target_relkind.as_str(), "r" | "p")
                            && !target_schema.is_empty()
                            && !target_name.is_empty())
                        .then(|| {
                            artifacts::table_identity("postgresql", &target_schema, &target_name)
                        })
                    });
                if let Some(target) = target {
                    if target != source_identity {
                        dependencies
                            .entry(source_identity.clone())
                            .or_default()
                            .push(target);
                    }
                }
            }
            for item in out {
                if let Some(values) = dependencies.remove(&item.identity) {
                    item.dependencies.extend(values);
                }
            }
        }
        Err(error) => catalog_unreadable(
            audit,
            completeness,
            "pg_depend",
            "artifact dependencies",
            &error,
        ),
    }
}

fn pg_subscription_inventory_sql(engine_version: &str) -> &'static str {
    let major = engine_version
        .split_once('.')
        .map_or(engine_version, |(major, _)| major)
        .parse::<u32>()
        .unwrap_or_default();
    if major >= 14 {
        "SELECT oid::text AS native_id, subname AS object_name \
         FROM pg_subscription \
         WHERE subdbid = (SELECT oid FROM pg_database WHERE datname = current_database()) \
         ORDER BY oid"
    } else {
        // PostgreSQL 13's pg_subscription OID is a hidden system column and
        // therefore is not covered by the catalog's safe PUBLIC column grants.
        // Selecting it requires relation-wide SELECT, which would also expose
        // subconninfo credentials. Use only the public, non-secret columns and
        // construct a database-scoped stable identity instead.
        "SELECT subdbid::text || ':' || subname::text AS native_id, \
                subname AS object_name \
         FROM pg_subscription \
         WHERE subdbid = (SELECT oid FROM pg_database WHERE datname = current_database()) \
         ORDER BY subname"
    }
}

fn pg_artifact_catalog(kind: &str, subkind: &str) -> Option<&'static str> {
    match kind {
        "view" | "materialized_view" | "sequence" | "external_table" => Some("pg_class"),
        "function" | "procedure" | "aggregate" => Some("pg_proc"),
        "type" => Some("pg_type"),
        "trigger" => Some("pg_trigger"),
        "default" if subkind == "check_constraint" => Some("pg_constraint"),
        "default" => Some("pg_attrdef"),
        "policy" => Some("pg_policy"),
        "rule" => Some("pg_rewrite"),
        "event_trigger" => Some("pg_event_trigger"),
        "extension" => Some("pg_extension"),
        "foreign_server" => Some("pg_foreign_server"),
        "publication" => Some("pg_publication"),
        "subscription" => Some("pg_subscription"),
        "physical_placement" => Some("pg_tablespace"),
        _ => None,
    }
}

fn catalog_unreadable(
    audit: &mut AuditLog,
    completeness: &mut CaptureCompleteness,
    catalog: &'static str,
    family: &'static str,
    _error: &dyn std::fmt::Display,
) {
    artifacts::record_catalog_unreadable(audit, completeness, catalog, family);
}
