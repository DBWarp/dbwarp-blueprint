//! Value-free, bounded relational statistics derived from column samples.
//!
//! This module never receives source values. It combines sanitized per-column
//! aggregates with catalog relationships and marks every composite estimate as
//! inferred so downstream generators and reports can distinguish it from a
//! direct tuple sample.

use std::collections::BTreeSet;

use crate::format::{BlueprintCardinality, BlueprintFile, BlueprintRelationship, BlueprintTable};

pub(crate) fn enrich_relational_statistics(blueprint: &mut BlueprintFile) {
    for table in blueprint.tables.values_mut() {
        enrich_index_prefix_statistics(table);
    }

    let child_ids = blueprint.fk_edges.keys().cloned().collect::<Vec<_>>();
    for child_id in child_ids {
        let Some(child) = blueprint.tables.get(&child_id) else {
            continue;
        };
        let inferred = blueprint
            .fk_edges
            .get(&child_id)
            .map(|edges| {
                edges
                    .iter()
                    .map(|edge| {
                        blueprint.tables.get(&edge.to).and_then(|parent| {
                            infer_relationship_statistics(
                                child,
                                parent,
                                edge.cols.as_slice(),
                                edge.validated,
                            )
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if let Some(edges) = blueprint.fk_edges.get_mut(&child_id) {
            for (edge, statistics) in edges.iter_mut().zip(inferred) {
                if edge.statistics.is_none() {
                    edge.statistics = statistics;
                }
            }
        }
    }
}

fn enrich_index_prefix_statistics(table: &mut BlueprintTable) {
    if table.rows == 0 {
        // Privacy rounding maps estimates below 50 rows to zero. A bounded
        // sample can still observe values in such a table, but emitting those
        // counts would place them outside the serialized table row domain and
        // violate the Blueprint invariant. Leave prefix cardinality unknown.
        return;
    }
    enrich_single_column_unique_cardinality(table);
    let cardinality_by_ordinal = table
        .cols
        .values()
        .filter_map(|column| {
            column
                .cardinality
                .as_ref()
                .map(|cardinality| (column.ordinal, cardinality.clone()))
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    for index in table.idxs.values_mut() {
        if index.expression || index.cols.is_empty() || !index.prefix_distinct_counts.is_empty() {
            continue;
        }
        let mut cumulative = 1_u64;
        let mut known = true;
        let mut prefixes = Vec::with_capacity(index.cols.len());
        for ordinal in &index.cols {
            let distinct = cardinality_by_ordinal
                .get(ordinal)
                .map(estimated_distinct)
                .filter(|distinct| *distinct > 0);
            match distinct {
                Some(distinct) if known => {
                    cumulative = cumulative.saturating_mul(distinct).min(table.rows.max(1));
                    prefixes.push(cumulative);
                }
                _ => {
                    known = false;
                    prefixes.push(0);
                }
            }
        }
        let all_key_columns_non_null = index.cols.iter().all(|ordinal| {
            table
                .cols
                .values()
                .find(|column| column.ordinal == *ordinal)
                .is_some_and(|column| !column.nullable)
        });
        if (index.primary || (index.unique && all_key_columns_non_null))
            && index.prefix_lengths.iter().all(|prefix| *prefix == 0)
            && !prefixes.is_empty()
        {
            if let Some(last) = prefixes.last_mut() {
                *last = table.rows;
            }
        }
        if prefixes.iter().any(|value| *value > 0) {
            index.prefix_distinct_counts = prefixes;
            index.cardinality_sample_method = "inferred-column-cardinality-product-v1".to_string();
        }
    }
}

fn enrich_single_column_unique_cardinality(table: &mut BlueprintTable) {
    let unique_ordinals = table
        .idxs
        .values()
        .filter(|index| {
            (index.primary || index.unique)
                && !index.expression
                && index.cols.len() == 1
                && index.prefix_lengths.first().copied().unwrap_or(0) == 0
        })
        .map(|index| index.cols[0])
        .collect::<BTreeSet<_>>();
    for ordinal in unique_ordinals {
        let Some(column) = table
            .cols
            .values_mut()
            .find(|column| column.ordinal == ordinal)
        else {
            continue;
        };
        let Some(cardinality) = column.cardinality.as_mut() else {
            // Catalog-only/basic captures intentionally do not contain sample
            // statistics. Do not introduce a Tier-2 block here.
            continue;
        };
        let estimated_non_null = if column.nullable {
            ((table.rows as f64) * (1.0 - column.null_fraction.unwrap_or(0.0))).round() as u64
        } else {
            table.rows
        };
        cardinality.estimated_distinct_count = estimated_non_null
            .max(cardinality.observed_distinct_count)
            .min(table.rows.max(cardinality.observed_distinct_count));
        cardinality.sample_method = format!(
            "{}; single-column unique constraint",
            cardinality.sample_method
        );
    }
}

fn infer_relationship_statistics(
    child: &BlueprintTable,
    parent: &BlueprintTable,
    child_ordinals: &[u32],
    validated: bool,
) -> Option<BlueprintRelationship> {
    if child_ordinals.is_empty() || child.rows == 0 || parent.rows == 0 {
        return None;
    }
    let unique_ordinals = child_ordinals.iter().copied().collect::<BTreeSet<_>>();
    if unique_ordinals.len() != child_ordinals.len() {
        return None;
    }
    let columns = child_ordinals
        .iter()
        .map(|ordinal| {
            child
                .cols
                .values()
                .find(|column| column.ordinal == *ordinal)
        })
        .collect::<Option<Vec<_>>>()?;
    let cardinalities = columns
        .iter()
        .map(|column| column.cardinality.as_ref())
        .collect::<Option<Vec<_>>>()?;

    let sample_rows = cardinalities
        .iter()
        .map(|cardinality| cardinality.sample_rows)
        .filter(|rows| *rows > 0)
        .min()?;
    let non_null_fraction = columns.iter().zip(cardinalities.iter()).fold(
        1.0_f64,
        |fraction, (column, cardinality)| {
            let sampled = if cardinality.sample_rows > 0 {
                cardinality.non_null_rows as f64 / cardinality.sample_rows as f64
            } else {
                1.0 - column.null_fraction.unwrap_or(0.0)
            };
            fraction * sampled.clamp(0.0, 1.0)
        },
    );
    let non_null_rows = quantize_count(
        ((sample_rows as f64) * non_null_fraction)
            .round()
            .clamp(0.0, sample_rows as f64) as u64,
    )
    .min(quantize_count(sample_rows));
    if non_null_rows == 0 {
        let sampled_with_bias = cardinalities.iter().any(|value| value.sampled_with_bias);
        return Some(BlueprintRelationship {
            measured: false,
            sample_rows: quantize_count(sample_rows),
            sample_method: "inferred-column-cardinality-v1".to_string(),
            sampled_with_bias,
            bias_reason: if sampled_with_bias {
                merged_bias_reason(cardinalities.as_slice())
            } else {
                String::new()
            },
            ..BlueprintRelationship::default()
        });
    }

    let observed_distinct = cardinalities.iter().fold(1_u64, |product, cardinality| {
        product.saturating_mul(cardinality.observed_distinct_count.max(1))
    });
    let estimated_distinct = cardinalities.iter().fold(1_u64, |product, cardinality| {
        product.saturating_mul(estimated_distinct(cardinality).max(1))
    });
    let source_non_null_rows = ((child.rows as f64) * non_null_fraction).round() as u64;
    let estimated_parent_values = estimated_distinct
        .min(source_non_null_rows.max(1))
        .min(parent.rows);
    let distinct_parent_values = quantize_count(observed_distinct.min(non_null_rows));
    let sample_average_fanout = non_null_rows.div_ceil(distinct_parent_values.max(1));
    let (fanout_p50, fanout_p95, fanout_p99, fanout_max) = if cardinalities.len() == 1 {
        let cardinality = cardinalities[0];
        (
            cardinality.frequency_p50.max(1),
            cardinality.frequency_p95.max(1),
            cardinality.frequency_p99.max(1),
            cardinality.frequency_max.max(1),
        )
    } else {
        (
            sample_average_fanout,
            sample_average_fanout,
            sample_average_fanout,
            sample_average_fanout,
        )
    };
    let sampled_with_bias = cardinalities.iter().any(|value| value.sampled_with_bias);
    Some(BlueprintRelationship {
        measured: false,
        sample_rows: quantize_count(sample_rows),
        non_null_rows,
        distinct_parent_values,
        parent_coverage_fraction: quantize_fraction(
            estimated_parent_values as f64 / parent.rows.max(1) as f64,
        ),
        fanout_p50: quantize_count(fanout_p50),
        fanout_p95: quantize_count(fanout_p95.max(fanout_p50)),
        fanout_p99: quantize_count(fanout_p99.max(fanout_p95).max(fanout_p50)),
        fanout_max: quantize_count(fanout_max.max(fanout_p99).max(fanout_p95).max(fanout_p50)),
        orphan_rows: 0,
        sample_method: if validated {
            "inferred-column-cardinality-valid-constraint-v1"
        } else {
            "inferred-column-cardinality-unvalidated-constraint-v1"
        }
        .to_string(),
        sampled_with_bias,
        bias_reason: if sampled_with_bias {
            merged_bias_reason(cardinalities.as_slice())
        } else {
            String::new()
        },
    })
}

fn estimated_distinct(cardinality: &BlueprintCardinality) -> u64 {
    cardinality
        .estimated_distinct_count
        .max(cardinality.observed_distinct_count)
}

fn merged_bias_reason(cardinalities: &[&BlueprintCardinality]) -> String {
    let reasons = cardinalities
        .iter()
        .filter(|cardinality| cardinality.sampled_with_bias)
        .map(|cardinality| cardinality.bias_reason.as_str())
        .filter(|reason| !reason.is_empty())
        .collect::<BTreeSet<_>>();
    if reasons.is_empty() {
        "derived-from-biased-column-sample".to_string()
    } else {
        reasons.into_iter().collect::<Vec<_>>().join("; ")
    }
}

fn quantize_count(value: u64) -> u64 {
    if value <= 32 {
        return value;
    }
    let magnitude = 1_u64 << (63 - value.leading_zeros());
    let bucket = (magnitude / 16).max(1);
    dbwarp_blueprint_core::round_to_bucket(value, bucket)
}

fn quantize_fraction(value: f64) -> f64 {
    (value.clamp(0.0, 1.0) * 200.0).round() / 200.0
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::format::{BlueprintColumn, BlueprintIndex, BlueprintTable, FkEdge, Totals};

    #[test]
    fn enrichment_adds_prefix_and_fk_statistics_without_values() {
        let cardinality = BlueprintCardinality {
            measured: true,
            sample_rows: 1_000,
            non_null_rows: 900,
            observed_distinct_count: 100,
            estimated_distinct_count: 200,
            top_value_fraction: 0.2,
            frequency_p50: 4,
            frequency_p95: 12,
            frequency_p99: 40,
            frequency_max: 180,
            sample_method: "bounded-test".into(),
            ..Default::default()
        };
        let mut parent = BlueprintTable {
            rows: 400,
            table_bytes: 4_000,
            ..Default::default()
        };
        parent.cols.insert(
            "id".into(),
            BlueprintColumn {
                ordinal: 1,
                ..Default::default()
            },
        );
        parent.idxs.insert(
            "pk".into(),
            BlueprintIndex {
                primary: true,
                unique: true,
                cols: vec![1],
                ..Default::default()
            },
        );
        let mut child = BlueprintTable {
            rows: 10_000,
            table_bytes: 100_000,
            ..Default::default()
        };
        child.cols.insert(
            "parent-id".into(),
            BlueprintColumn {
                ordinal: 1,
                nullable: true,
                cardinality: Some(cardinality),
                ..Default::default()
            },
        );
        child.idxs.insert(
            "ix-parent".into(),
            BlueprintIndex {
                cols: vec![1],
                ..Default::default()
            },
        );
        let mut blueprint = BlueprintFile {
            artifact_inventory: None,
            schema_version: 3,
            generated_at: "2026-08-04T00:00:00Z".into(),
            engine: "postgresql".into(),
            engine_version: "18".into(),
            source_kind: "synthetic".into(),
            length_metadata: "exact".into(),
            declared_length_fidelity: "exact".into(),
            index_length_fidelity: "exact".into(),
            observed_length_fidelity: "coarse-rounded-v1".into(),
            totals: Totals {
                table_count: 2,
                row_count: 10_400,
                table_bytes: 104_000,
                ..Default::default()
            },
            network: None,
            database_topology: None,
            dataset_scope: None,
            tables: BTreeMap::from([("child".into(), child), ("parent".into(), parent)]),
            fk_edges: BTreeMap::from([(
                "child".into(),
                vec![FkEdge {
                    to: "parent".into(),
                    cols: vec![1],
                    to_cols: vec![1],
                    ..Default::default()
                }],
            )]),
        };

        enrich_relational_statistics(&mut blueprint);
        assert_eq!(
            blueprint.tables["child"].idxs["ix-parent"].prefix_distinct_counts,
            vec![200]
        );
        let relationship = blueprint.fk_edges["child"][0].statistics.as_ref().unwrap();
        assert!(!relationship.measured);
        assert_eq!(relationship.sample_rows, 992);
        assert_eq!(relationship.non_null_rows, 896);
        assert_eq!(relationship.parent_coverage_fraction, 0.5);
        assert_eq!(relationship.fanout_max, 184);
    }

    #[test]
    fn prefix_statistics_keep_unknown_slots_and_do_not_overclaim_nullable_uniqueness() {
        let mut table = BlueprintTable {
            rows: 1_000,
            ..Default::default()
        };
        table.cols.insert(
            "first".into(),
            BlueprintColumn {
                ordinal: 1,
                cardinality: Some(BlueprintCardinality {
                    measured: true,
                    observed_distinct_count: 10,
                    estimated_distinct_count: 10,
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        table.cols.insert(
            "second".into(),
            BlueprintColumn {
                ordinal: 2,
                nullable: true,
                ..Default::default()
            },
        );
        table.idxs.insert(
            "uq".into(),
            BlueprintIndex {
                unique: true,
                cols: vec![1, 2],
                ..Default::default()
            },
        );

        enrich_index_prefix_statistics(&mut table);

        assert_eq!(table.idxs["uq"].prefix_distinct_counts, vec![10, 0]);
    }

    #[test]
    fn prefix_statistics_stay_unknown_when_privacy_rounding_hides_small_row_count() {
        let mut table = BlueprintTable {
            rows: 0,
            ..Default::default()
        };
        table.cols.insert(
            "id".into(),
            BlueprintColumn {
                ordinal: 1,
                cardinality: Some(BlueprintCardinality {
                    measured: true,
                    sample_rows: 8,
                    observed_distinct_count: 8,
                    estimated_distinct_count: 8,
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        table.idxs.insert(
            "pk".into(),
            BlueprintIndex {
                primary: true,
                unique: true,
                cols: vec![1],
                ..Default::default()
            },
        );

        enrich_index_prefix_statistics(&mut table);

        assert!(table.idxs["pk"].prefix_distinct_counts.is_empty());
        assert!(table.idxs["pk"].cardinality_sample_method.is_empty());
    }

    #[test]
    fn sampled_primary_key_uses_the_serialized_table_row_domain() {
        let mut table = BlueprintTable {
            rows: 1_000_000,
            ..Default::default()
        };
        table.cols.insert(
            "id".into(),
            BlueprintColumn {
                ordinal: 1,
                cardinality: Some(BlueprintCardinality {
                    measured: true,
                    sample_rows: 32,
                    non_null_rows: 32,
                    observed_distinct_count: 32,
                    estimated_distinct_count: 32,
                    sample_method: "bounded sample".into(),
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        table.idxs.insert(
            "pk".into(),
            BlueprintIndex {
                primary: true,
                unique: true,
                cols: vec![1],
                ..Default::default()
            },
        );

        enrich_index_prefix_statistics(&mut table);

        let cardinality = table.cols["id"].cardinality.as_ref().unwrap();
        assert_eq!(cardinality.estimated_distinct_count, 1_000_000);
        assert!(cardinality.sample_method.contains("unique constraint"));
    }
}
