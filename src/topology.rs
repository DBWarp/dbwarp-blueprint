//! Shared topology and dataset-scope contract helpers.
//!
//! Engine adapters retain only bounded facts and call these helpers for
//! canonical ordering and customer-visible degradation warnings.

use crate::audit::AuditLog;
use crate::format::{DatabaseTopology, DatasetScope};

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
