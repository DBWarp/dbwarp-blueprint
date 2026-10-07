use crate::{BlueprintColumn, BlueprintFile};
use anyhow::{Context, Result};

/// A deterministic estimate of the evidence contained in a Blueprint.
///
/// This is deliberately an evidence score, not an observed error bound or a
/// statistical confidence interval: the collector has no source ground truth
/// against which it could measure its own output during a normal run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlueprintFidelityEstimate {
    pub overall_score: u8,
    pub band: &'static str,
    pub structure_score: u8,
    pub sizing_score: u8,
    pub column_statistics_score: u8,
    pub relationship_score: u8,
    pub artifact_score: u8,
    pub limitations: Vec<String>,
}

/// Estimate fidelity for the canonical Blueprint model.
pub fn estimate_blueprint_fidelity(blueprint: &BlueprintFile) -> BlueprintFidelityEstimate {
    let mut limitations = Vec::new();

    let scope = blueprint.dataset_scope.as_ref();
    let table_inventory_score = scope
        .map(|scope| completeness_score(scope.table_inventory_completeness.as_str()))
        .unwrap_or(25);
    if table_inventory_score < 100 {
        limitations.push("table-inventory-not-complete".to_string());
    }

    let topology_score = if blueprint.engine == "parquet" || blueprint.engine == "avro" {
        table_inventory_score
    } else {
        match blueprint
            .database_topology
            .as_ref()
            .map(|topology| topology.visibility.as_str())
        {
            Some("full") => 100,
            Some("partial") => {
                limitations.push("topology-visibility-partial".to_string());
                60
            }
            _ => {
                limitations.push("topology-visibility-unknown".to_string());
                40
            }
        }
    };
    let structure_score = weighted_average(&[(table_inventory_score, 3), (topology_score, 1)]);

    let row_coverage_score = scope
        .map(|scope| completeness_score(scope.row_count_completeness.as_str()))
        .unwrap_or(25);
    let row_score = table_statistics_freshness_score(blueprint, &mut limitations)
        .map(|freshness| {
            // Freshness qualifies the evidence we have; it cannot compensate
            // for missing row-count coverage.
            let freshness_factor = weighted_average(&[(100, 3), (freshness, 1)]);
            rounded_ratio(
                u64::from(row_coverage_score) * u64::from(freshness_factor),
                100,
            )
        })
        .unwrap_or(row_coverage_score);
    let size_score = scope
        .map(|scope| completeness_score(scope.size_completeness.as_str()))
        .unwrap_or(25);
    let sizing_score = weighted_average(&[(row_score, 1), (size_score, 1)]);
    if row_coverage_score < 100 {
        limitations.push("row-count-coverage-not-complete".to_string());
    }
    if size_score < 100 {
        limitations.push("size-coverage-not-complete".to_string());
    }

    let mut column_score_total = 0u64;
    let mut column_count = 0u64;
    let mut sampled_columns = 0u64;
    let mut biased_columns = 0u64;
    let mut lower_bound_columns = 0u64;
    for table in blueprint.tables.values() {
        for column in table.cols.values() {
            column_count += 1;
            let mut score = 25u8; // canonical type, ordinal, and nullability
            let all_null = column.null_fraction == Some(1.0);
            if column.null_fraction.is_some() {
                score = score.saturating_add(20);
                sampled_columns += 1;
            }
            if all_null {
                // Cardinality and value length are not applicable when every
                // observed row is NULL; omission is truthful, not missing.
                score = score.saturating_add(40);
            } else if let Some(cardinality) =
                column.cardinality.as_ref().filter(|value| value.measured)
            {
                let lower_bound = (cardinality.sample_method.contains("lower-bound")
                    || cardinality.sample_method.contains("lower bound"))
                    && !cardinality.sample_method.contains("unique constraint");
                if cardinality.sampled_with_bias || lower_bound {
                    score = score.saturating_add(10);
                    biased_columns += u64::from(cardinality.sampled_with_bias);
                    lower_bound_columns += u64::from(lower_bound);
                } else {
                    score = score.saturating_add(25);
                }
            }
            if !all_null && (!is_variable_width(column) || column.length_sample_rows > 0) {
                score = score.saturating_add(15);
            }
            if column
                .compression
                .as_ref()
                .is_some_and(|compression| compression.measured)
            {
                score = score.saturating_add(15);
            }
            column_score_total = column_score_total.saturating_add(u64::from(score.min(100)));
        }
    }
    let column_statistics_score = if column_count == 0 {
        100
    } else {
        rounded_ratio(column_score_total, column_count)
    };
    if column_count > 0 && sampled_columns == 0 {
        limitations.push("column-statistics-not-sampled".to_string());
    } else if sampled_columns < column_count {
        limitations.push("column-statistics-partial".to_string());
    }
    if biased_columns > 0 {
        limitations.push("biased-column-sampling".to_string());
    }
    if lower_bound_columns > 0 {
        limitations.push("cardinality-lower-bounds".to_string());
    }

    let relationships = blueprint
        .fk_edges
        .values()
        .flat_map(|relationships| relationships.iter())
        .collect::<Vec<_>>();
    let relationship_score = if relationships.is_empty() {
        table_inventory_score
    } else {
        let evidence = relationships.iter().fold(0u64, |total, relationship| {
            total
                + if relationship.statistics.is_some() {
                    100
                } else {
                    50
                }
        });
        let score = rounded_ratio(evidence, relationships.len() as u64);
        if relationships
            .iter()
            .any(|relationship| relationship.statistics.is_none())
        {
            limitations.push("relationship-statistics-partial".to_string());
        }
        score
    };

    let artifact_score = match blueprint.artifact_inventory.as_ref() {
        None => {
            limitations.push("artifact-inventory-not-requested".to_string());
            0
        }
        Some(inventory) => {
            if !inventory.inventory_complete {
                limitations.push("artifact-inventory-incomplete".to_string());
            }
            match inventory.detail.as_str() {
                "summary" if inventory.inventory_complete => 50,
                "graph" if inventory.inventory_complete && inventory.dependencies_complete => 80,
                "analyzed"
                    if inventory.inventory_complete
                        && inventory.dependencies_complete
                        && inventory.analysis_complete =>
                {
                    100
                }
                "none" => 0,
                _ => 25,
            }
        }
    };

    limitations.sort();
    limitations.dedup();
    let overall_score = weighted_average(&[
        (structure_score, 30),
        (sizing_score, 25),
        (column_statistics_score, 30),
        (relationship_score, 10),
        (artifact_score, 5),
    ]);
    let band = match overall_score {
        90..=100 => "high",
        75..=89 => "good",
        50..=74 => "moderate",
        _ => "low",
    };

    BlueprintFidelityEstimate {
        overall_score,
        band,
        structure_score,
        sizing_score,
        column_statistics_score,
        relationship_score,
        artifact_score,
        limitations,
    }
}

/// Estimate fidelity for any value that serialises to the Blueprint model.
/// Values already held as `BlueprintFile` can use
/// `estimate_blueprint_fidelity` directly.
pub fn estimate_serializable_blueprint_fidelity<T>(
    blueprint: &T,
) -> Result<BlueprintFidelityEstimate>
where
    T: serde::Serialize + ?Sized,
{
    let encoded =
        toml::to_string(blueprint).context("serializing Blueprint for fidelity assessment")?;
    let canonical: BlueprintFile =
        toml::from_str(&encoded).context("decoding Blueprint for fidelity assessment")?;
    Ok(estimate_blueprint_fidelity(&canonical))
}

fn completeness_score(value: &str) -> u8 {
    match value {
        "complete" => 100,
        "incomplete" => 50,
        _ => 25,
    }
}

fn table_statistics_freshness_score(
    blueprint: &BlueprintFile,
    limitations: &mut Vec<String>,
) -> Option<u8> {
    let mut total = 0u64;
    let mut count = 0u64;
    for table in blueprint.tables.values() {
        let statistics_state = table
            .statistics
            .as_ref()
            .map(|evidence| evidence.statistics_state.as_str());
        let score = match statistics_state {
            Some("current") => 100,
            Some("possibly-stale") => {
                limitations.push("table-statistics-possibly-stale".to_string());
                80
            }
            Some("known-stale") => {
                limitations.push("table-statistics-stale".to_string());
                60
            }
            Some("never-analyzed") => {
                limitations.push("table-statistics-never-analyzed".to_string());
                30
            }
            Some("locked") => {
                limitations.push("table-statistics-locked".to_string());
                70
            }
            Some("user-supplied") => {
                limitations.push("table-statistics-user-supplied".to_string());
                75
            }
            // Unknown and not-applicable states must not be invented into a
            // freshness claim. Structured sources and counter-based engines
            // can legitimately have no optimizer freshness evidence.
            Some(_) => continue,
            None => match table.stats_freshness.as_str() {
                "fresh" => 100,
                "stale" => {
                    limitations.push("table-statistics-stale".to_string());
                    60
                }
                "never_analyzed" => {
                    limitations.push("table-statistics-never-analyzed".to_string());
                    30
                }
                // Empty is the schema-v6 representation for engines and
                // fallback paths that cannot establish freshness.
                _ => continue,
            },
        };
        total = total.saturating_add(score);
        count = count.saturating_add(1);
    }
    (count > 0).then(|| rounded_ratio(total, count))
}

fn is_variable_width(column: &BlueprintColumn) -> bool {
    matches!(
        column.column_type.as_str(),
        "text" | "string" | "binary" | "bytes" | "json" | "xml" | "unknown"
    ) || column.declared_max_chars > 0
        || column.declared_max_bytes > 0
}

fn rounded_ratio(total: u64, denominator: u64) -> u8 {
    if denominator == 0 {
        return 0;
    }
    ((total.saturating_add(denominator / 2) / denominator).min(100)) as u8
}

fn weighted_average(values: &[(u8, u64)]) -> u8 {
    let denominator = values.iter().map(|(_, weight)| *weight).sum::<u64>();
    let total = values
        .iter()
        .map(|(value, weight)| u64::from(*value) * *weight)
        .sum::<u64>();
    rounded_ratio(total, denominator)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ArtifactInventory, BlueprintCardinality, BlueprintCompression, BlueprintTable,
        DatabaseTopology, DatasetScope, FkEdge, TableStatisticsEvidence,
    };
    use std::collections::BTreeMap;

    fn complete_blueprint() -> BlueprintFile {
        let mut topology = DatabaseTopology::unknown();
        topology.visibility = "full".to_string();
        let mut scope = DatasetScope::unknown_database("catalog", "catalog");
        scope.table_inventory_completeness = "complete".to_string();
        scope.row_count_completeness = "complete".to_string();
        scope.size_completeness = "complete".to_string();
        let column = BlueprintColumn {
            ordinal: 1,
            column_type: "text".to_string(),
            nullable: true,
            null_fraction: Some(0.1),
            length_sample_rows: 1_000,
            compression: Some(BlueprintCompression {
                measured: true,
                sample_rows: 1_000,
                ..Default::default()
            }),
            cardinality: Some(BlueprintCardinality {
                measured: true,
                sample_rows: 1_000,
                non_null_rows: 900,
                observed_distinct_count: 100,
                estimated_distinct_count: 100,
                ..Default::default()
            }),
            ..Default::default()
        };
        let table = BlueprintTable {
            rows: 1_000,
            stats_freshness: "fresh".to_string(),
            cols: BTreeMap::from([("col-1".to_string(), column)]),
            ..Default::default()
        };
        BlueprintFile {
            schema_version: crate::SCHEMA_VERSION,
            engine: "postgresql".to_string(),
            database_topology: Some(topology),
            dataset_scope: Some(scope),
            tables: BTreeMap::from([("table-001".to_string(), table)]),
            artifact_inventory: Some(ArtifactInventory {
                detail: "analyzed".to_string(),
                inventory_complete: true,
                dependencies_complete: true,
                analysis_complete: true,
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn complete_sampled_evidence_scores_high_without_limitations() {
        let estimate = estimate_blueprint_fidelity(&complete_blueprint());
        assert_eq!(estimate.overall_score, 100);
        assert_eq!(estimate.band, "high");
        assert!(estimate.limitations.is_empty());
    }

    #[test]
    fn catalog_only_unknown_scope_is_conservatively_low() {
        let mut blueprint = complete_blueprint();
        blueprint.database_topology = Some(DatabaseTopology::unknown());
        blueprint.dataset_scope = Some(DatasetScope::unknown_database("unknown", "unknown"));
        blueprint.artifact_inventory = None;
        let column = blueprint.tables["table-001"].cols["col-1"].clone();
        blueprint.tables.get_mut("table-001").unwrap().cols.insert(
            "col-1".to_string(),
            BlueprintColumn {
                null_fraction: None,
                length_sample_rows: 0,
                compression: None,
                cardinality: None,
                ..column
            },
        );

        let estimate = estimate_blueprint_fidelity(&blueprint);
        assert!(estimate.overall_score < 50, "{estimate:?}");
        assert_eq!(estimate.band, "low");
        assert!(estimate
            .limitations
            .contains(&"column-statistics-not-sampled".to_string()));
        assert!(estimate
            .limitations
            .contains(&"artifact-inventory-not-requested".to_string()));
    }

    #[test]
    fn biased_lower_bound_and_missing_relationship_statistics_are_visible() {
        let mut blueprint = complete_blueprint();
        let cardinality = blueprint
            .tables
            .get_mut("table-001")
            .unwrap()
            .cols
            .get_mut("col-1")
            .unwrap()
            .cardinality
            .as_mut()
            .unwrap();
        cardinality.sampled_with_bias = true;
        cardinality.bias_reason = "deterministic-first-n-rows".to_string();
        cardinality.sample_method = "observed lower bound".to_string();
        blueprint.fk_edges.insert(
            "table-001".to_string(),
            vec![FkEdge {
                to: "table-002".to_string(),
                cols: vec![1],
                to_cols: vec![1],
                statistics: None,
                ..Default::default()
            }],
        );

        let estimate = estimate_blueprint_fidelity(&blueprint);
        assert_eq!(estimate.column_statistics_score, 85);
        assert_eq!(estimate.relationship_score, 50);
        assert!(estimate
            .limitations
            .contains(&"biased-column-sampling".to_string()));
        assert!(estimate
            .limitations
            .contains(&"cardinality-lower-bounds".to_string()));
        assert!(estimate
            .limitations
            .contains(&"relationship-statistics-partial".to_string()));
    }

    #[test]
    fn chao1_lower_bound_is_disclosed_but_a_unique_constraint_is_exact() {
        let mut blueprint = complete_blueprint();
        let cardinality = blueprint
            .tables
            .get_mut("table-001")
            .unwrap()
            .cols
            .get_mut("col-1")
            .unwrap()
            .cardinality
            .as_mut()
            .unwrap();
        cardinality.sample_method =
            "bounded sample; Chao1 lower-bound cardinality estimate".to_string();

        let lower_bound = estimate_blueprint_fidelity(&blueprint);
        assert!(lower_bound
            .limitations
            .contains(&"cardinality-lower-bounds".to_string()));

        blueprint
            .tables
            .get_mut("table-001")
            .unwrap()
            .cols
            .get_mut("col-1")
            .unwrap()
            .cardinality
            .as_mut()
            .unwrap()
            .sample_method
            .push_str("; single-column unique constraint");
        let exact = estimate_blueprint_fidelity(&blueprint);
        assert!(!exact
            .limitations
            .contains(&"cardinality-lower-bounds".to_string()));
    }

    #[test]
    fn compatibility_assessor_matches_canonical_assessor() {
        let blueprint = complete_blueprint();
        assert_eq!(
            estimate_serializable_blueprint_fidelity(&blueprint).unwrap(),
            estimate_blueprint_fidelity(&blueprint)
        );
    }

    #[test]
    fn all_null_columns_do_not_create_false_cardinality_or_length_gaps() {
        let mut blueprint = complete_blueprint();
        let column = blueprint
            .tables
            .get_mut("table-001")
            .unwrap()
            .cols
            .get_mut("col-1")
            .unwrap();
        column.null_fraction = Some(1.0);
        column.length_sample_rows = 0;
        column.cardinality = None;

        let estimate = estimate_blueprint_fidelity(&blueprint);
        assert_eq!(estimate.column_statistics_score, 100);
        assert!(!estimate
            .limitations
            .contains(&"column-statistics-partial".to_string()));
    }

    #[test]
    fn reported_stale_statistics_reduce_sizing_evidence() {
        let mut blueprint = complete_blueprint();
        blueprint
            .tables
            .get_mut("table-001")
            .unwrap()
            .stats_freshness = "stale".to_string();

        let stale = estimate_blueprint_fidelity(&blueprint);
        assert_eq!(stale.sizing_score, 95);
        assert!(stale
            .limitations
            .contains(&"table-statistics-stale".to_string()));

        blueprint
            .tables
            .get_mut("table-001")
            .unwrap()
            .stats_freshness = "never_analyzed".to_string();
        let never_analyzed = estimate_blueprint_fidelity(&blueprint);
        assert!(never_analyzed.sizing_score < stale.sizing_score);
        assert!(never_analyzed
            .limitations
            .contains(&"table-statistics-never-analyzed".to_string()));
    }

    #[test]
    fn schema_v7_statistics_state_drives_the_fidelity_estimate() {
        let mut blueprint = complete_blueprint();
        {
            let table = blueprint.tables.get_mut("table-001").unwrap();
            table.stats_freshness.clear();
            table.statistics = Some(TableStatisticsEvidence {
                statistics_state: "current".to_string(),
                ..Default::default()
            });
        }
        let current = estimate_blueprint_fidelity(&blueprint);

        blueprint
            .tables
            .get_mut("table-001")
            .unwrap()
            .statistics
            .as_mut()
            .unwrap()
            .statistics_state = "known-stale".to_string();
        let stale = estimate_blueprint_fidelity(&blueprint);
        assert!(stale.sizing_score < current.sizing_score);
        assert!(stale
            .limitations
            .contains(&"table-statistics-stale".to_string()));

        blueprint
            .tables
            .get_mut("table-001")
            .unwrap()
            .statistics
            .as_mut()
            .unwrap()
            .statistics_state = "never-analyzed".to_string();
        let never_analyzed = estimate_blueprint_fidelity(&blueprint);
        assert!(never_analyzed.sizing_score < stale.sizing_score);
        assert!(never_analyzed
            .limitations
            .contains(&"table-statistics-never-analyzed".to_string()));
    }

    #[test]
    fn unknown_freshness_does_not_penalize_counter_based_engines() {
        let mut blueprint = complete_blueprint();
        blueprint.engine = "sqlserver".to_string();
        blueprint
            .tables
            .get_mut("table-001")
            .unwrap()
            .stats_freshness = String::new();

        let estimate = estimate_blueprint_fidelity(&blueprint);
        assert_eq!(estimate.sizing_score, 100);
        assert!(!estimate
            .limitations
            .iter()
            .any(|limitation| limitation.starts_with("table-statistics-")));
    }

    #[test]
    fn freshness_never_compensates_for_missing_row_count_coverage() {
        for coverage in ["complete", "incomplete", "unknown"] {
            let mut blueprint = complete_blueprint();
            blueprint
                .dataset_scope
                .as_mut()
                .unwrap()
                .row_count_completeness = coverage.to_string();
            blueprint
                .tables
                .get_mut("table-001")
                .unwrap()
                .stats_freshness
                .clear();
            let baseline = estimate_blueprint_fidelity(&blueprint).sizing_score;
            for freshness in ["fresh", "stale", "never_analyzed"] {
                blueprint
                    .tables
                    .get_mut("table-001")
                    .unwrap()
                    .stats_freshness = freshness.to_string();
                let score = estimate_blueprint_fidelity(&blueprint).sizing_score;
                assert!(
                    score <= baseline,
                    "{coverage}/{freshness}: {score} > {baseline}"
                );
                if freshness == "fresh" {
                    assert_eq!(score, baseline);
                }
            }
        }
    }

    #[test]
    fn zero_serialized_rows_are_not_treated_as_proof_of_an_empty_source_table() {
        let mut blueprint = complete_blueprint();
        let table = blueprint.tables.get_mut("table-001").unwrap();
        table.rows = 0;
        table.cols.insert(
            "col-1".to_string(),
            BlueprintColumn {
                ordinal: 1,
                column_type: "text".to_string(),
                ..Default::default()
            },
        );

        let estimate = estimate_blueprint_fidelity(&blueprint);
        assert!(estimate.column_statistics_score < 100);
        assert!(estimate
            .limitations
            .contains(&"column-statistics-not-sampled".to_string()));
    }

    #[test]
    fn removing_evidence_never_improves_any_fidelity_dimension() {
        let complete = complete_blueprint();
        let baseline = estimate_blueprint_fidelity(&complete);
        let mut degraded = complete;
        degraded.database_topology.as_mut().unwrap().visibility = "partial".to_string();
        degraded
            .dataset_scope
            .as_mut()
            .unwrap()
            .row_count_completeness = "incomplete".to_string();
        let column = degraded
            .tables
            .get_mut("table-001")
            .unwrap()
            .cols
            .get_mut("col-1")
            .unwrap();
        column.null_fraction = None;
        column.cardinality = None;
        column.compression = None;
        column.length_sample_rows = 0;
        degraded.artifact_inventory = None;

        let estimate = estimate_blueprint_fidelity(&degraded);
        assert!(estimate.overall_score < baseline.overall_score);
        assert!(estimate.structure_score <= baseline.structure_score);
        assert!(estimate.sizing_score <= baseline.sizing_score);
        assert!(estimate.column_statistics_score <= baseline.column_statistics_score);
        assert!(estimate.relationship_score <= baseline.relationship_score);
        assert!(estimate.artifact_score <= baseline.artifact_score);
    }
}
