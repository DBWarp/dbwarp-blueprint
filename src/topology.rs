//! Shared topology and dataset-scope contract helpers.
//!
//! Engine modules retain only bounded facts and call these helpers for
//! canonical ordering and customer-visible degradation warnings.

use crate::audit::AuditLog;
use crate::format::{DatabaseTopology, DatasetScope};
use dbwarp_blueprint_core::StructureScope;

pub fn sort_dedup(values: &mut Vec<String>) {
    values.sort();
    values.dedup();
}

pub fn sort_topology(topology: &mut DatabaseTopology) {
    sort_dedup(&mut topology.features);
    sort_dedup(&mut topology.catalogs_read);
    sort_dedup(&mut topology.catalogs_unreadable);
}

pub fn warn_evidence_unavailable(audit: &mut AuditLog, catalog: &str) {
    let detail = crate::i18n::format(
        "engine.topology_unavailable",
        &[
            ("code", "DBP1411W".to_string()),
            ("catalog", catalog.to_string()),
        ],
    );
    eprintln!("dbwarp-blueprint: {detail}");
    audit.record_warning("DBP1411W", detail);
}

/// Retain a usable Blueprint when a non-core index or relationship catalog
/// fails after tables and columns were captured. The enum makes an engine-module
/// typo a compile-time error; driver text and source identifiers never reach
/// the warning or the emitted scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructureFamily {
    Index,
    Relationship,
}

pub fn mark_structure_unavailable(
    scope: &mut StructureScope,
    family: StructureFamily,
    catalog: &str,
    audit: &mut AuditLog,
) {
    scope.catalogs_read.retain(|value| value != catalog);
    scope.catalogs_unreadable.push(catalog.to_string());
    match family {
        StructureFamily::Index => {
            scope.index_inventory_completeness = "incomplete".to_string();
            scope
                .limitations
                .push("index-inventory-unavailable".to_string());
        }
        StructureFamily::Relationship => {
            scope.relationship_inventory_completeness = "incomplete".to_string();
            scope
                .limitations
                .push("relationship-inventory-unavailable".to_string());
        }
    }
    sort_dedup(&mut scope.catalogs_read);
    sort_dedup(&mut scope.catalogs_unreadable);
    sort_dedup(&mut scope.limitations);
    let detail = crate::i18n::format(
        "engine.structure_unavailable",
        &[
            ("code", "DBP1423W".to_string()),
            ("catalog", catalog.to_string()),
        ],
    );
    eprintln!("dbwarp-blueprint: {detail}");
    audit.record_warning("DBP1423W", detail);
}

pub fn warn_distributed_size_unavailable(audit: &mut AuditLog) {
    let detail = crate::i18n::format(
        "engine.distributed_size_unavailable",
        &[("code", "DBP1412W".to_string())],
    );
    eprintln!("dbwarp-blueprint: {detail}");
    audit.record_warning("DBP1412W", detail);
}

pub fn warn_incomplete_dataset_scope(scope: &DatasetScope, audit: &mut AuditLog) {
    if [
        scope.table_inventory_completeness.as_str(),
        scope.row_count_completeness.as_str(),
        scope.size_completeness.as_str(),
    ]
    .iter()
    .all(|value| *value == "complete")
    {
        return;
    }
    let detail = crate::i18n::format(
        "engine.dataset_scope_incomplete",
        &[
            ("code", "DBP1413W".to_string()),
            ("tables", scope.table_inventory_completeness.to_string()),
            ("rows", scope.row_count_completeness.to_string()),
            ("sizes", scope.size_completeness.to_string()),
        ],
    );
    eprintln!("dbwarp-blueprint: {detail}");
    audit.record_warning("DBP1413W", detail);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_core_structure_failure_is_retained_and_explicit() {
        let mut scope = StructureScope::complete_database("postgresql");
        let mut audit = AuditLog::new("test", 0);

        mark_structure_unavailable(
            &mut scope,
            StructureFamily::Index,
            "postgresql-indexes",
            &mut audit,
        );
        mark_structure_unavailable(
            &mut scope,
            StructureFamily::Relationship,
            "postgresql-foreign-keys",
            &mut audit,
        );

        assert_eq!(scope.table_inventory_completeness, "complete");
        assert_eq!(scope.column_inventory_completeness, "complete");
        assert_eq!(scope.index_inventory_completeness, "incomplete");
        assert_eq!(scope.relationship_inventory_completeness, "incomplete");
        assert_eq!(
            scope.catalogs_unreadable,
            vec!["postgresql-foreign-keys", "postgresql-indexes"]
        );
        assert!(scope
            .limitations
            .contains(&"index-inventory-unavailable".to_string()));
        assert!(scope
            .limitations
            .contains(&"relationship-inventory-unavailable".to_string()));
        assert_eq!(
            audit
                .warnings
                .iter()
                .filter(|warning| warning.starts_with("DBP1423W "))
                .count(),
            2
        );
    }
}
