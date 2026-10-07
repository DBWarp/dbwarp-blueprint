//! Customer examples must remain valid, useful illustrations of the live contract.

use dbwarp_blueprint_core::{blueprint_to_toml, parse_blueprint_toml};

const CURRENT: [(&str, &str, &str); 3] = [
    (
        "postgresql-v6-catalog.toml",
        "postgresql",
        include_str!("../samples/postgresql-v6-catalog.toml"),
    ),
    (
        "mysql-v6-sampled.toml",
        "mysql",
        include_str!("../samples/mysql-v6-sampled.toml"),
    ),
    (
        "sqlserver-v6-analyzed.toml",
        "sqlserver",
        include_str!("../samples/sqlserver-v6-analyzed.toml"),
    ),
];

#[test]
fn v6_samples_remain_compatible_and_round_trip_without_losing_fields() {
    for (name, engine, text) in CURRENT {
        let blueprint =
            parse_blueprint_toml(text).unwrap_or_else(|error| panic!("{name}: {error:#}"));
        assert_eq!(blueprint.schema_version, 6, "{name}");
        assert_eq!(blueprint.source_kind, "synthetic", "{name}");
        assert_eq!(blueprint.engine, engine, "{name}");
        let encoded = blueprint_to_toml(&blueprint).unwrap();
        let restored = parse_blueprint_toml(&encoded).unwrap();
        assert_eq!(
            serde_json::to_value(&blueprint).unwrap(),
            serde_json::to_value(&restored).unwrap()
        );
        assert_eq!(encoded, blueprint_to_toml(&restored).unwrap());
        for (table_id, table) in &blueprint.tables {
            assert!(table_id.starts_with("table-"));
            assert!(table.schema.starts_with("schema-"));
            assert!(table.cols.keys().all(|id| id.starts_with("col-")));
            if engine != "postgresql" {
                assert!(table.stats_freshness.is_empty(), "{name}");
            }
        }
    }
}

#[test]
fn catalog_sample_does_not_imply_row_sampling_or_complete_artifact_visibility() {
    let blueprint = parse_blueprint_toml(CURRENT[0].2).unwrap();
    for table in blueprint.tables.values() {
        assert!(table.compression.is_none());
        for column in table.cols.values() {
            assert!(column.compression.is_none());
            assert!(column.cardinality.is_none());
            assert!(column.null_fraction.is_none());
        }
    }
    let inventory = blueprint.artifact_inventory.unwrap();
    assert_eq!(inventory.detail, "summary");
    assert!(!inventory.inventory_complete);
    assert!(inventory.artifacts.is_empty());
}

#[test]
fn analyzed_sample_exposes_aggregate_and_external_dependency_limits() {
    let blueprint = parse_blueprint_toml(CURRENT[2].2).unwrap();
    assert!(!blueprint.fk_edges.is_empty());
    let column = &blueprint.tables["table-002"].cols["col-3"];
    assert!(column.null_fraction.is_some());
    assert!(column.cardinality.is_some());
    assert!(!column.style.is_empty());
    assert_eq!(column.compression.as_ref().unwrap().ratio_stddev, 0.0);
    let inventory = blueprint.artifact_inventory.unwrap();
    assert_eq!(inventory.detail, "analyzed");
    assert!(!inventory.analysis_complete);
    assert_eq!(inventory.external_prerequisite_count, 1);
    let analysis = inventory.artifacts["procedure-001"]
        .analysis
        .as_ref()
        .unwrap();
    assert_eq!(analysis.status, "partial");
    assert!(analysis.features.contains_key("dynamic.sql"));
}
