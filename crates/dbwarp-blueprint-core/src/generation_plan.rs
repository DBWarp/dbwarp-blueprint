use crate::{
    generated_table_name, ordered_columns, scaled_row_count, BlueprintFile, BlueprintRelationship,
};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap, HashMap, HashSet};

/// Target-neutral table and relationship metadata needed to plan generation.
///
/// Frontends with a compatible but not identical Blueprint model can construct this
/// catalog without first serializing through the canonical TOML model.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GenerationCatalog {
    pub tables: Vec<GenerationCatalogTable>,
    pub foreign_keys: Vec<GenerationCatalogForeignKey>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationCatalogTable {
    pub source_name: String,
    pub rows: u64,
    pub column_ordinals: Vec<u32>,
    pub exact_unique_keys: Vec<Vec<u32>>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GenerationCatalogForeignKey {
    pub child_source_name: String,
    pub parent_source_name: String,
    pub child_ordinals: Vec<u32>,
    pub parent_ordinals: Vec<u32>,
    pub edge_ordinal: usize,
    pub on_update: String,
    pub on_delete: String,
    pub match_type: String,
    pub deferrable: bool,
    pub initially_deferred: bool,
    pub validated: bool,
    pub statistics: Option<BlueprintRelationship>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GenerationPlanOptions {
    pub selected_table: Option<String>,
    pub table_prefix: String,
    pub target_table: Option<String>,
    pub fixture_scale: f64,
    pub max_tables: Option<usize>,
    pub max_rows_per_table: Option<u64>,
    /// Retain relationship cycles when the caller creates every table first,
    /// loads all data, and adds constraints only after the complete load.
    /// Live per-table adapters must leave this false until they have a
    /// transfer-wide post-data finalization phase.
    pub retain_relationship_cycles: bool,
}

impl Default for GenerationPlanOptions {
    fn default() -> Self {
        Self {
            selected_table: None,
            table_prefix: "blueprint_".to_string(),
            target_table: None,
            fixture_scale: 1.0,
            max_tables: None,
            max_rows_per_table: None,
            retain_relationship_cycles: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationTablePlan {
    pub source_name: String,
    pub selection_index: usize,
    pub target_name: String,
    pub row_count: u64,
    pub column_ordinals: Vec<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GenerationForeignKeyPlan {
    pub child_selection_index: usize,
    pub parent_selection_index: usize,
    pub child_ordinals: Vec<u32>,
    pub parent_ordinals: Vec<u32>,
    pub edge_ordinal: usize,
    /// False means the source named parent columns but did not describe a
    /// matching exact primary/unique index. An adapter may synthesize a support
    /// index or reject the relationship according to its fidelity contract.
    pub parent_key_declared_exact: bool,
    pub on_update: String,
    pub on_delete: String,
    pub match_type: String,
    pub deferrable: bool,
    pub initially_deferred: bool,
    pub validated: bool,
    pub statistics: Option<BlueprintRelationship>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationPlanSummary {
    pub source_edges: usize,
    pub emitted: usize,
    pub omitted_external: usize,
    pub omitted_unsupported: usize,
    pub omitted_cycles: usize,
    pub retained_cycles: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct BlueprintGenerationPlan {
    /// Tables remain in selection order so stable generated identities do not
    /// change merely because relationship ordering changes.
    pub tables: Vec<GenerationTablePlan>,
    /// Selection indexes in deterministic parent-before-child execution order.
    pub load_order: Vec<usize>,
    pub foreign_keys: Vec<GenerationForeignKeyPlan>,
    pub summary: GenerationPlanSummary,
}

pub fn generation_catalog_from_blueprint(blueprint: &BlueprintFile) -> GenerationCatalog {
    let tables = blueprint
        .tables
        .iter()
        .map(|(source_name, table)| {
            let column_ordinals = ordered_columns(table)
                .into_iter()
                .map(|(_, column)| column.ordinal)
                .collect::<Vec<_>>();
            let known_ordinals = column_ordinals.iter().copied().collect::<HashSet<_>>();
            let exact_unique_keys = table
                .idxs
                .values()
                .filter(|index| index.primary || index.unique)
                .filter(|index| !index.cols.is_empty())
                .filter(|index| index.prefix_lengths.iter().all(|prefix| *prefix == 0))
                .map(|index| index.cols.clone())
                .filter(|ordinals| {
                    ordinals
                        .iter()
                        .all(|ordinal| known_ordinals.contains(ordinal))
                        && ordinals.iter().copied().collect::<HashSet<_>>().len() == ordinals.len()
                })
                .collect::<Vec<_>>();
            GenerationCatalogTable {
                source_name: source_name.clone(),
                rows: table.rows,
                column_ordinals,
                exact_unique_keys,
            }
        })
        .collect();
    let foreign_keys = blueprint
        .fk_edges
        .iter()
        .flat_map(|(child_source_name, edges)| {
            edges
                .iter()
                .enumerate()
                .map(|(edge_ordinal, edge)| GenerationCatalogForeignKey {
                    child_source_name: child_source_name.clone(),
                    parent_source_name: edge.to.clone(),
                    child_ordinals: edge.cols.clone(),
                    parent_ordinals: edge.to_cols.clone(),
                    edge_ordinal,
                    on_update: edge.on_update.clone(),
                    on_delete: edge.on_delete.clone(),
                    match_type: edge.match_type.clone(),
                    deferrable: edge.deferrable,
                    initially_deferred: edge.initially_deferred,
                    validated: edge.validated,
                    statistics: edge.statistics.clone(),
                })
        })
        .collect();
    GenerationCatalog {
        tables,
        foreign_keys,
    }
}

pub fn plan_blueprint_generation(
    blueprint: &BlueprintFile,
    options: &GenerationPlanOptions,
) -> Result<BlueprintGenerationPlan> {
    plan_generation_catalog(&generation_catalog_from_blueprint(blueprint), options)
}

pub fn plan_generation_catalog(
    catalog: &GenerationCatalog,
    options: &GenerationPlanOptions,
) -> Result<BlueprintGenerationPlan> {
    validate_options(options)?;
    let catalog_by_name = catalog_table_indexes(catalog)?;
    let mut selected_catalog_indexes = catalog
        .tables
        .iter()
        .enumerate()
        .filter_map(|(idx, table)| {
            options
                .selected_table
                .as_ref()
                .is_none_or(|wanted| wanted == &table.source_name)
                .then_some(idx)
        })
        .collect::<Vec<_>>();
    if let Some(max_tables) = options.max_tables {
        selected_catalog_indexes.truncate(max_tables);
    }
    if selected_catalog_indexes.is_empty() {
        if catalog.tables.is_empty()
            && options.selected_table.is_none()
            && options.target_table.is_none()
        {
            return Ok(BlueprintGenerationPlan::default());
        }
        bail!("no Blueprint tables matched the requested generation selection");
    }
    if options.target_table.is_some() && selected_catalog_indexes.len() != 1 {
        bail!("a target-table override requires exactly one selected Blueprint table");
    }

    let tables = selected_catalog_indexes
        .iter()
        .enumerate()
        .map(|(selection_index, catalog_index)| {
            let table = &catalog.tables[*catalog_index];
            GenerationTablePlan {
                source_name: table.source_name.clone(),
                selection_index,
                target_name: options.target_table.clone().unwrap_or_else(|| {
                    generated_table_name(options.table_prefix.as_str(), selection_index + 1)
                }),
                row_count: scaled_row_count(
                    table.rows,
                    options.fixture_scale,
                    options.max_rows_per_table,
                ),
                column_ordinals: table.column_ordinals.clone(),
            }
        })
        .collect::<Vec<_>>();
    let selected_by_name = tables
        .iter()
        .map(|table| (table.source_name.as_str(), table.selection_index))
        .collect::<HashMap<_, _>>();

    let mut summary = GenerationPlanSummary::default();
    let mut candidates = Vec::new();
    let mut claimed_child_values = HashMap::<(usize, u32), (usize, u32)>::new();

    for edge in &catalog.foreign_keys {
        let Some(&child_selection_index) = selected_by_name.get(edge.child_source_name.as_str())
        else {
            continue;
        };
        summary.source_edges += 1;
        let Some(&parent_selection_index) = selected_by_name.get(edge.parent_source_name.as_str())
        else {
            summary.omitted_external += 1;
            continue;
        };
        let child = &tables[child_selection_index];
        let parent = &tables[parent_selection_index];
        if child.row_count > 0 && parent.row_count == 0 {
            summary.omitted_unsupported += 1;
            continue;
        }
        let child_catalog = &catalog.tables[selected_catalog_indexes[child_selection_index]];
        let parent_catalog = &catalog.tables[selected_catalog_indexes[parent_selection_index]];
        if edge.child_ordinals.is_empty()
            || edge
                .child_ordinals
                .iter()
                .any(|ordinal| !child_catalog.column_ordinals.contains(ordinal))
        {
            summary.omitted_unsupported += 1;
            continue;
        }

        let (parent_ordinals, parent_key_declared_exact) =
            match resolve_parent_ordinals(parent_catalog, edge) {
                Some(mapping) => mapping,
                None => {
                    summary.omitted_unsupported += 1;
                    continue;
                }
            };
        if parent_ordinals.len() != edge.child_ordinals.len() {
            summary.omitted_unsupported += 1;
            continue;
        }

        let mappings = edge
            .child_ordinals
            .iter()
            .copied()
            .zip(parent_ordinals.iter().copied())
            .map(|(child_ordinal, parent_ordinal)| {
                (
                    (child_selection_index, child_ordinal),
                    (parent_selection_index, parent_ordinal),
                )
            })
            .collect::<Vec<_>>();
        if mappings.iter().any(|(child_key, parent_value)| {
            claimed_child_values
                .get(child_key)
                .is_some_and(|existing| existing != parent_value)
        }) {
            summary.omitted_unsupported += 1;
            continue;
        }
        for (child_key, parent_value) in mappings {
            claimed_child_values
                .entry(child_key)
                .or_insert(parent_value);
        }

        candidates.push(GenerationForeignKeyPlan {
            child_selection_index,
            parent_selection_index,
            child_ordinals: edge.child_ordinals.clone(),
            parent_ordinals,
            edge_ordinal: edge.edge_ordinal,
            parent_key_declared_exact,
            on_update: edge.on_update.clone(),
            on_delete: edge.on_delete.clone(),
            match_type: edge.match_type.clone(),
            deferrable: edge.deferrable,
            initially_deferred: edge.initially_deferred,
            validated: edge.validated,
            statistics: edge.statistics.clone(),
        });
    }

    let cyclic = cyclic_candidate_indexes(tables.len(), &candidates);
    let ordering_foreign_keys = candidates
        .iter()
        .enumerate()
        .filter_map(|(idx, candidate)| (!cyclic.contains(&idx)).then_some(candidate.clone()))
        .collect::<Vec<_>>();
    let foreign_keys = if options.retain_relationship_cycles {
        summary.retained_cycles = cyclic.len();
        candidates
    } else {
        summary.omitted_cycles = cyclic.len();
        ordering_foreign_keys.clone()
    };
    // Cyclic edges do not constrain data-generation order because synthetic
    // parent values are deterministic. They only require deferred DDL.
    let load_order = topological_order(tables.len(), &ordering_foreign_keys)?;
    summary.emitted = foreign_keys.len();

    // Ensure callers cannot accidentally pass a relationship catalog whose
    // table names differ only through duplicate entries hidden by a map.
    debug_assert_eq!(catalog_by_name.len(), catalog.tables.len());
    Ok(BlueprintGenerationPlan {
        tables,
        load_order,
        foreign_keys,
        summary,
    })
}

fn validate_options(options: &GenerationPlanOptions) -> Result<()> {
    if !options.fixture_scale.is_finite() || options.fixture_scale < 0.0 {
        bail!("fixture scale must be a non-negative finite number");
    }
    if options.max_tables == Some(0) {
        bail!("max tables must be greater than zero");
    }
    Ok(())
}

fn catalog_table_indexes(catalog: &GenerationCatalog) -> Result<BTreeMap<&str, usize>> {
    let mut indexes = BTreeMap::new();
    for (idx, table) in catalog.tables.iter().enumerate() {
        if table.source_name.is_empty() {
            bail!("generation catalog contains an empty source table name");
        }
        if indexes.insert(table.source_name.as_str(), idx).is_some() {
            bail!(
                "generation catalog contains duplicate source table name {}",
                table.source_name
            );
        }
    }
    Ok(indexes)
}

fn resolve_parent_ordinals(
    parent: &GenerationCatalogTable,
    edge: &GenerationCatalogForeignKey,
) -> Option<(Vec<u32>, bool)> {
    if !edge.parent_ordinals.is_empty() {
        if edge.parent_ordinals.len() != edge.child_ordinals.len()
            || edge
                .parent_ordinals
                .iter()
                .any(|ordinal| !parent.column_ordinals.contains(ordinal))
        {
            return None;
        }
        let exact = parent
            .exact_unique_keys
            .iter()
            .any(|key| key == &edge.parent_ordinals);
        return Some((edge.parent_ordinals.clone(), exact));
    }
    parent
        .exact_unique_keys
        .iter()
        .find(|key| key.len() == edge.child_ordinals.len())
        .cloned()
        .map(|ordinals| (ordinals, true))
}

fn cyclic_candidate_indexes(
    table_count: usize,
    candidates: &[GenerationForeignKeyPlan],
) -> HashSet<usize> {
    let mut graph = vec![Vec::new(); table_count];
    let mut reverse = vec![Vec::new(); table_count];
    for candidate in candidates {
        if candidate.parent_selection_index == candidate.child_selection_index {
            continue;
        }
        graph[candidate.parent_selection_index].push(candidate.child_selection_index);
        reverse[candidate.child_selection_index].push(candidate.parent_selection_index);
    }
    let finish_order = graph_finish_order(&graph);
    let mut component = vec![usize::MAX; table_count];
    let mut component_sizes = Vec::new();
    for root in finish_order.into_iter().rev() {
        if component[root] != usize::MAX {
            continue;
        }
        let component_id = component_sizes.len();
        let mut size = 0usize;
        let mut stack = vec![root];
        component[root] = component_id;
        while let Some(node) = stack.pop() {
            size += 1;
            for parent in &reverse[node] {
                if component[*parent] == usize::MAX {
                    component[*parent] = component_id;
                    stack.push(*parent);
                }
            }
        }
        component_sizes.push(size);
    }
    candidates
        .iter()
        .enumerate()
        .filter_map(|(idx, candidate)| {
            let component_id = component[candidate.parent_selection_index];
            (candidate.parent_selection_index != candidate.child_selection_index
                && component_id == component[candidate.child_selection_index]
                && component_sizes.get(component_id).copied().unwrap_or(0) > 1)
                .then_some(idx)
        })
        .collect()
}

fn graph_finish_order(graph: &[Vec<usize>]) -> Vec<usize> {
    let mut visited = vec![false; graph.len()];
    let mut order = Vec::with_capacity(graph.len());
    for root in 0..graph.len() {
        if visited[root] {
            continue;
        }
        visited[root] = true;
        let mut stack = vec![(root, 0usize)];
        while let Some((node, next_child)) = stack.last_mut() {
            if *next_child < graph[*node].len() {
                let child = graph[*node][*next_child];
                *next_child += 1;
                if !visited[child] {
                    visited[child] = true;
                    stack.push((child, 0));
                }
            } else {
                order.push(*node);
                stack.pop();
            }
        }
    }
    order
}

fn topological_order(
    table_count: usize,
    foreign_keys: &[GenerationForeignKeyPlan],
) -> Result<Vec<usize>> {
    let mut indegree = vec![0usize; table_count];
    let mut children = vec![Vec::new(); table_count];
    for foreign_key in foreign_keys {
        if foreign_key.child_selection_index == foreign_key.parent_selection_index {
            continue;
        }
        indegree[foreign_key.child_selection_index] += 1;
        children[foreign_key.parent_selection_index].push(foreign_key.child_selection_index);
    }
    let mut ready = indegree
        .iter()
        .enumerate()
        .filter_map(|(idx, degree)| (*degree == 0).then_some(Reverse(idx)))
        .collect::<BinaryHeap<_>>();
    let mut order = Vec::with_capacity(table_count);
    while let Some(Reverse(parent)) = ready.pop() {
        order.push(parent);
        for child in &children[parent] {
            indegree[*child] -= 1;
            if indegree[*child] == 0 {
                ready.push(Reverse(*child));
            }
        }
    }
    if order.len() != table_count {
        bail!("generation planner retained a relationship dependency cycle");
    }
    Ok(order)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BlueprintColumn, BlueprintIndex, BlueprintTable, FkEdge};

    fn table(rows: u64, ordinals: &[u32], exact_keys: &[&[u32]]) -> BlueprintTable {
        let mut table = BlueprintTable {
            rows,
            ..Default::default()
        };
        for ordinal in ordinals {
            table.cols.insert(
                format!("col-{ordinal}"),
                BlueprintColumn {
                    ordinal: *ordinal,
                    column_type: "bigint".to_string(),
                    ..Default::default()
                },
            );
        }
        for (idx, key) in exact_keys.iter().enumerate() {
            table.idxs.insert(
                format!("key-{idx}"),
                BlueprintIndex {
                    unique: true,
                    primary: idx == 0,
                    cols: key.to_vec(),
                    ..Default::default()
                },
            );
        }
        table
    }

    #[test]
    fn selection_scaling_and_target_override_are_stable() {
        let mut blueprint = BlueprintFile::default();
        blueprint.tables.insert("a".into(), table(101, &[1], &[]));
        blueprint.tables.insert("b".into(), table(200, &[1], &[]));
        let plan = plan_blueprint_generation(
            &blueprint,
            &GenerationPlanOptions {
                selected_table: Some("b".into()),
                table_prefix: "fixture_".into(),
                target_table: Some("chosen".into()),
                fixture_scale: 0.25,
                max_rows_per_table: Some(40),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(plan.tables.len(), 1);
        assert_eq!(plan.tables[0].source_name, "b");
        assert_eq!(plan.tables[0].target_name, "chosen");
        assert_eq!(plan.tables[0].row_count, 40);
        assert_eq!(plan.load_order, vec![0]);
    }

    #[test]
    fn composite_fk_loads_parent_before_lexically_earlier_child() {
        let mut blueprint = BlueprintFile::default();
        blueprint
            .tables
            .insert("a_child".into(), table(8, &[1, 2], &[]));
        blueprint
            .tables
            .insert("z_parent".into(), table(8, &[1, 2], &[&[1, 2]]));
        blueprint.fk_edges.insert(
            "a_child".into(),
            vec![FkEdge {
                to: "z_parent".into(),
                cols: vec![1, 2],
                to_cols: Vec::new(),
                ..Default::default()
            }],
        );
        let plan =
            plan_blueprint_generation(&blueprint, &GenerationPlanOptions::default()).unwrap();
        assert_eq!(plan.load_order, vec![1, 0]);
        assert_eq!(plan.summary.emitted, 1);
        assert_eq!(plan.foreign_keys[0].child_ordinals, vec![1, 2]);
        assert_eq!(plan.foreign_keys[0].parent_ordinals, vec![1, 2]);
        assert!(plan.foreign_keys[0].parent_key_declared_exact);
        assert_eq!(plan.tables[0].target_name, "blueprint_0001");
        assert_eq!(plan.tables[1].target_name, "blueprint_0002");
    }

    #[test]
    fn external_and_unsupported_relationships_are_counted() {
        let mut blueprint = BlueprintFile::default();
        blueprint.tables.insert("child".into(), table(4, &[1], &[]));
        blueprint.fk_edges.insert(
            "child".into(),
            vec![
                FkEdge {
                    to: "outside".into(),
                    cols: vec![1],
                    to_cols: vec![1],
                    ..Default::default()
                },
                FkEdge {
                    to: "child".into(),
                    cols: vec![99],
                    to_cols: vec![1],
                    ..Default::default()
                },
            ],
        );
        let plan =
            plan_blueprint_generation(&blueprint, &GenerationPlanOptions::default()).unwrap();
        assert_eq!(plan.summary.source_edges, 2);
        assert_eq!(plan.summary.omitted_external, 1);
        assert_eq!(plan.summary.omitted_unsupported, 1);
        assert_eq!(plan.summary.emitted, 0);
    }

    #[test]
    fn cycles_are_removed_without_dropping_downstream_relationships() {
        let mut blueprint = BlueprintFile::default();
        for name in ["a", "b", "c"] {
            blueprint
                .tables
                .insert(name.into(), table(4, &[1], &[&[1]]));
        }
        blueprint.fk_edges.insert(
            "a".into(),
            vec![FkEdge {
                to: "b".into(),
                cols: vec![1],
                to_cols: vec![1],
                ..Default::default()
            }],
        );
        blueprint.fk_edges.insert(
            "b".into(),
            vec![FkEdge {
                to: "a".into(),
                cols: vec![1],
                to_cols: vec![1],
                ..Default::default()
            }],
        );
        blueprint.fk_edges.insert(
            "c".into(),
            vec![FkEdge {
                to: "b".into(),
                cols: vec![1],
                to_cols: vec![1],
                ..Default::default()
            }],
        );
        let plan =
            plan_blueprint_generation(&blueprint, &GenerationPlanOptions::default()).unwrap();
        assert_eq!(plan.summary.omitted_cycles, 2);
        assert_eq!(plan.summary.emitted, 1);
        assert_eq!(plan.foreign_keys[0].child_selection_index, 2);
        assert_eq!(plan.foreign_keys[0].parent_selection_index, 1);
        let b_position = plan.load_order.iter().position(|idx| *idx == 1).unwrap();
        let c_position = plan.load_order.iter().position(|idx| *idx == 2).unwrap();
        assert!(b_position < c_position);
    }

    #[test]
    fn deferred_ddl_callers_can_retain_relationship_cycles() {
        let mut blueprint = BlueprintFile::default();
        for name in ["a", "b"] {
            blueprint
                .tables
                .insert(name.into(), table(4, &[1], &[&[1]]));
        }
        blueprint.fk_edges.insert(
            "a".into(),
            vec![FkEdge {
                to: "b".into(),
                cols: vec![1],
                to_cols: vec![1],
                ..Default::default()
            }],
        );
        blueprint.fk_edges.insert(
            "b".into(),
            vec![FkEdge {
                to: "a".into(),
                cols: vec![1],
                to_cols: vec![1],
                ..Default::default()
            }],
        );

        let plan = plan_blueprint_generation(
            &blueprint,
            &GenerationPlanOptions {
                retain_relationship_cycles: true,
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(plan.summary.emitted, 2);
        assert_eq!(plan.summary.omitted_cycles, 0);
        assert_eq!(plan.summary.retained_cycles, 2);
        assert_eq!(plan.foreign_keys.len(), 2);
        assert_eq!(plan.load_order, vec![0, 1]);
    }

    #[test]
    fn explicit_parent_columns_are_flagged_when_support_index_is_missing() {
        let mut blueprint = BlueprintFile::default();
        blueprint.tables.insert("child".into(), table(2, &[1], &[]));
        blueprint
            .tables
            .insert("parent".into(), table(2, &[1], &[]));
        blueprint.fk_edges.insert(
            "child".into(),
            vec![FkEdge {
                to: "parent".into(),
                cols: vec![1],
                to_cols: vec![1],
                ..Default::default()
            }],
        );
        let plan =
            plan_blueprint_generation(&blueprint, &GenerationPlanOptions::default()).unwrap();
        assert_eq!(plan.summary.emitted, 1);
        assert!(!plan.foreign_keys[0].parent_key_declared_exact);
    }

    #[test]
    fn an_empty_catalog_has_an_empty_plan() {
        let plan = plan_generation_catalog(
            &GenerationCatalog::default(),
            &GenerationPlanOptions::default(),
        )
        .unwrap();
        assert!(plan.tables.is_empty());
        assert!(plan.load_order.is_empty());
        assert_eq!(plan.summary, GenerationPlanSummary::default());
    }
}
