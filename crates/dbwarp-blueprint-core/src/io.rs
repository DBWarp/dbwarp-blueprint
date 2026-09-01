//! Blueprint and bundle input/output contract.
//!
//! Parsing accepts only the documented compatibility range, validates before
//! normalization, and never emits legacy identifiers. Serialization is
//! deterministic for an equivalent normalized model and is shared by live,
//! structured-file, batch, and bundle workflows.

use crate::{
    recompute_bundle_totals, BlueprintBundle, BlueprintFile, BlueprintSelector, BundleSource,
    BundleTotals, Totals, BUNDLE_KIND, BUNDLE_SCHEMA_VERSION, LEGACY_ARTIFACT_CONTRACT,
    LEGACY_BUNDLE_KIND, LEGACY_BUNDLE_SCHEMA_VERSION, LEGACY_IDENTIFIER_SCHEMA_VERSION,
    LEGACY_SAMPLE_ENCODING_TAG, MIN_SCHEMA_VERSION, PREVIOUS_BUNDLE_SCHEMA_VERSION,
    SAMPLE_ENCODING_TAG, SCHEMA_VERSION,
};
use anyhow::{bail, Context, Result};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

const STRUCTURED_SAMPLE_ENCODINGS: &[&str] = &[
    "parquet-column-chunks",
    "parquet-file",
    "avro-container",
    "avro-schema",
    "mixed-structured-file-provenance",
];

pub fn parse_blueprint_toml(text: &str) -> Result<BlueprintFile> {
    let mut blueprint: BlueprintFile = toml::from_str(text).context("parsing Blueprint TOML")?;
    validate_blueprint_contract(&blueprint)?;
    normalize_blueprint_identifiers(&mut blueprint);
    if blueprint.schema_version == 4 {
        blueprint.schema_version = LEGACY_IDENTIFIER_SCHEMA_VERSION;
        validate_blueprint_contract(&blueprint)?;
    }
    Ok(blueprint)
}

pub fn validate_blueprint_contract(blueprint: &BlueprintFile) -> Result<()> {
    if !(MIN_SCHEMA_VERSION..=SCHEMA_VERSION).contains(&blueprint.schema_version) {
        bail!(
            "unsupported Blueprint schema_version {}; supported range is {}..={}",
            blueprint.schema_version,
            MIN_SCHEMA_VERSION,
            SCHEMA_VERSION
        );
    }
    let computed_totals = computed_blueprint_totals(blueprint)?;
    let totals_required = blueprint.schema_version >= 2;
    validate_total(
        "table_count",
        blueprint.totals.table_count,
        computed_totals.table_count,
        totals_required,
    )?;
    validate_total(
        "row_count",
        blueprint.totals.row_count,
        computed_totals.row_count,
        totals_required,
    )?;
    validate_total(
        "table_bytes",
        blueprint.totals.table_bytes,
        computed_totals.table_bytes,
        totals_required,
    )?;
    validate_total(
        "index_bytes",
        blueprint.totals.index_bytes,
        computed_totals.index_bytes,
        totals_required,
    )?;

    for (table_id, table) in &blueprint.tables {
        let mut ordinals = BTreeSet::new();
        for (column_id, column) in &table.cols {
            if column.ordinal == 0 || !ordinals.insert(column.ordinal) {
                bail!(
                    "table '{table_id}' has missing or duplicate column ordinal {} at '{column_id}'",
                    column.ordinal
                );
            }
            if column.column_type.trim().is_empty() {
                bail!("table '{table_id}' column '{column_id}' has no canonical type");
            }
            validate_column_semantics(blueprint.schema_version, table_id, column_id, column)?;
            if let Some(fraction) = column.null_fraction {
                if !fraction.is_finite() || !(0.0..=1.0).contains(&fraction) {
                    bail!(
                        "table '{table_id}' column '{column_id}' has invalid null_fraction {fraction}"
                    );
                }
            }
            if column.numeric_precision > 0 && column.numeric_scale > column.numeric_precision {
                bail!(
                    "table '{table_id}' column '{column_id}' has scale {} above precision {}",
                    column.numeric_scale,
                    column.numeric_precision
                );
            }
            if column.bit_width > 64 {
                bail!(
                    "table '{table_id}' column '{column_id}' has unsupported bit width {}",
                    column.bit_width
                );
            }
            if column.length_p95_sample_rows > column.length_sample_rows {
                bail!(
                    "table '{table_id}' column '{column_id}' has p95 sample rows {} above total width sample rows {}",
                    column.length_p95_sample_rows,
                    column.length_sample_rows
                );
            }
            validate_compression(
                blueprint.schema_version,
                table_id,
                Some(column_id),
                column.compression.as_ref(),
            )?;
            validate_cardinality(table_id, column_id, column.cardinality.as_ref())?;
        }

        validate_table_semantics(blueprint, table_id, table, &ordinals)?;

        for (index_id, index) in &table.idxs {
            if blueprint.schema_version >= 2 && index.cols.is_empty() && !index.expression {
                bail!("table '{table_id}' index '{index_id}' has no key columns");
            }
            if !index.prefix_lengths.is_empty() && index.prefix_lengths.len() != index.cols.len() {
                bail!(
                    "table '{table_id}' index '{index_id}' has {} prefix lengths for {} columns",
                    index.prefix_lengths.len(),
                    index.cols.len()
                );
            }
            for ordinal in index.cols.iter().chain(index.include_cols.iter()) {
                if !ordinals.contains(ordinal) {
                    bail!(
                        "table '{table_id}' index '{index_id}' references missing column ordinal {ordinal}"
                    );
                }
            }
            if index.prefix_distinct_counts.len() > index.cols.len() {
                bail!(
                    "table '{table_id}' index '{index_id}' has {} prefix cardinalities for {} key columns",
                    index.prefix_distinct_counts.len(),
                    index.cols.len()
                );
            }
            let mut previous_known = 0_u64;
            for distinct in index
                .prefix_distinct_counts
                .iter()
                .copied()
                .filter(|distinct| *distinct > 0)
            {
                if distinct > table.rows || distinct < previous_known {
                    bail!(
                        "table '{table_id}' index '{index_id}' prefix cardinalities are outside the table row domain or not monotonic"
                    );
                }
                previous_known = distinct;
            }
            if blueprint.schema_version >= 3
                && index.prefix_distinct_counts.is_empty()
                    != index.cardinality_sample_method.is_empty()
            {
                bail!(
                    "table '{table_id}' index '{index_id}' prefix cardinalities and sample method must be supplied together"
                );
            }
        }
        validate_compression(
            blueprint.schema_version,
            table_id,
            None,
            table.compression.as_ref(),
        )?;
    }

    for (child_id, edges) in &blueprint.fk_edges {
        let child = blueprint
            .tables
            .get(child_id)
            .with_context(|| format!("foreign-key child table '{child_id}' is missing"))?;
        let child_ordinals = child
            .cols
            .values()
            .map(|column| column.ordinal)
            .collect::<BTreeSet<_>>();
        for edge in edges {
            let parent = blueprint.tables.get(&edge.to).with_context(|| {
                format!(
                    "foreign-key from '{child_id}' references missing parent table '{}'",
                    edge.to
                )
            })?;
            if edge.cols.is_empty() {
                bail!(
                    "foreign-key from '{child_id}' to '{}' has no columns",
                    edge.to
                );
            }
            if blueprint.schema_version >= 2 && edge.to_cols.len() != edge.cols.len() {
                bail!(
                    "schema-v2 foreign-key from '{child_id}' to '{}' has {} child columns and {} parent columns",
                    edge.to,
                    edge.cols.len(),
                    edge.to_cols.len()
                );
            }
            if !edge.to_cols.is_empty() && edge.to_cols.len() != edge.cols.len() {
                bail!(
                    "foreign-key from '{child_id}' to '{}' has mismatched column arity",
                    edge.to
                );
            }
            let parent_ordinals = parent
                .cols
                .values()
                .map(|column| column.ordinal)
                .collect::<BTreeSet<_>>();
            if edge
                .cols
                .iter()
                .any(|ordinal| !child_ordinals.contains(ordinal))
                || edge
                    .to_cols
                    .iter()
                    .any(|ordinal| !parent_ordinals.contains(ordinal))
            {
                bail!(
                    "foreign-key from '{child_id}' to '{}' references a missing column ordinal",
                    edge.to
                );
            }
            validate_foreign_key_semantics(child_id, edge)?;
            validate_relationship(child_id, edge.to.as_str(), edge.statistics.as_ref())?;
        }
    }
    if let Some(inventory) = blueprint.artifact_inventory.as_ref() {
        validate_artifact_inventory(blueprint, inventory)?;
    }
    validate_topology_and_dataset_scope(blueprint)?;
    Ok(())
}

fn validate_column_semantics(
    schema_version: u32,
    table_id: &str,
    column_id: &str,
    column: &crate::BlueprintColumn,
) -> Result<()> {
    let has_v6_fields = !column.value_source.is_empty()
        || column.has_default.is_some()
        || !column.default_kind.is_empty()
        || !column.type_kind.is_empty()
        || column.member_count.is_some()
        || column.domain_has_check.is_some()
        || column.hidden.is_some()
        || column.masked.is_some()
        || column.encrypted.is_some()
        || column.sparse.is_some()
        || column.has_check.is_some()
        || column.magnitude_min.is_some()
        || column.magnitude_max.is_some()
        || column.has_negative.is_some()
        || !column.time_span.is_empty()
        || column.time_recent_decade.is_some();
    if schema_version < 6 {
        if has_v6_fields {
            bail!(
                "table '{table_id}' column '{column_id}' uses column-semantics fields before Blueprint schema_version 6"
            );
        }
        return Ok(());
    }

    if !column.value_source.is_empty() {
        validate_token(
            "column value_source",
            &column.value_source,
            &[
                "identity-always",
                "identity-default",
                "auto-increment",
                "identity",
                "sequence-default",
                "generated-stored",
                "generated-virtual",
                "computed-persisted",
                "computed-virtual",
                "system-time",
                "rowversion",
            ],
        )?;
    }
    if !column.default_kind.is_empty() {
        validate_token(
            "column default_kind",
            &column.default_kind,
            &["constant", "function", "expression"],
        )?;
        if column.has_default != Some(true) {
            bail!(
                "table '{table_id}' column '{column_id}' has default_kind without has_default = true"
            );
        }
    }
    if !column.type_kind.is_empty() {
        validate_token(
            "column type_kind",
            &column.type_kind,
            &[
                "enum",
                "set",
                "domain",
                "composite",
                "array",
                "range",
                "alias",
            ],
        )?;
    }
    if matches!(column.type_kind.as_str(), "enum" | "set") {
        if !column.member_count.is_some_and(|count| count > 0) {
            bail!(
                "table '{table_id}' column '{column_id}' type_kind '{}' requires member_count greater than zero",
                column.type_kind
            );
        }
    } else if column.member_count.is_some() {
        bail!(
            "table '{table_id}' column '{column_id}' has member_count without enum or set type_kind"
        );
    }
    if column.domain_has_check.is_some() && column.type_kind != "domain" {
        bail!(
            "table '{table_id}' column '{column_id}' has domain_has_check without domain type_kind"
        );
    }

    match (
        column.magnitude_min,
        column.magnitude_max,
        column.has_negative,
    ) {
        (None, None, None) => {}
        (Some(minimum), Some(maximum), Some(_)) if minimum <= maximum => {}
        (Some(minimum), Some(maximum), Some(_)) => bail!(
            "table '{table_id}' column '{column_id}' has magnitude_min {minimum} above magnitude_max {maximum}"
        ),
        _ => bail!(
            "table '{table_id}' column '{column_id}' must supply magnitude_min, magnitude_max, and has_negative together"
        ),
    }

    match (column.time_span.is_empty(), column.time_recent_decade) {
        (true, None) => {}
        (false, Some(decade)) => {
            validate_token(
                "column time_span",
                &column.time_span,
                &["intraday", "days", "weeks", "months", "years", "decades"],
            )?;
            if decade % 10 != 0 {
                bail!(
                    "table '{table_id}' column '{column_id}' has non-decade time_recent_decade {decade}"
                );
            }
        }
        _ => bail!(
            "table '{table_id}' column '{column_id}' must supply time_span and time_recent_decade together"
        ),
    }

    Ok(())
}

fn validate_table_semantics(
    blueprint: &BlueprintFile,
    table_id: &str,
    table: &crate::BlueprintTable,
    ordinals: &BTreeSet<u32>,
) -> Result<()> {
    let has_v6_fields = !table.kind.is_empty()
        || table.unlogged.is_some()
        || !table.partition_strategy.is_empty()
        || table.partition_count.is_some()
        || !table.partition_key_cols.is_empty()
        || table.partition_rows_max.is_some()
        || !table.temporal_history.is_empty()
        || table.counted_in_totals.is_some()
        || table.check_count.is_some();
    if blueprint.schema_version < 6 {
        if has_v6_fields {
            bail!(
                "table '{table_id}' uses table-semantics fields before Blueprint schema_version 6"
            );
        }
        return Ok(());
    }

    if !table.kind.is_empty() {
        validate_token(
            "table kind",
            &table.kind,
            &[
                "partitioned",
                "materialized-view",
                "temporal-current",
                "temporal-history",
                "memory-optimized",
                "external",
                "graph-node",
                "graph-edge",
            ],
        )?;
    }
    if !table.partition_strategy.is_empty() {
        validate_token(
            "table partition_strategy",
            &table.partition_strategy,
            &["range", "list", "hash", "key", "linear-hash"],
        )?;
    }

    let has_partition_fields = !table.partition_strategy.is_empty()
        || table.partition_count.is_some()
        || !table.partition_key_cols.is_empty()
        || table.partition_rows_max.is_some();
    if table.kind == "partitioned" {
        if !table.partition_count.is_some_and(|count| count > 0) {
            bail!("table '{table_id}' is partitioned but has no positive partition_count");
        }
    } else if has_partition_fields {
        bail!("table '{table_id}' has partition fields without kind = 'partitioned'");
    }

    let mut partition_ordinals = BTreeSet::new();
    for ordinal in &table.partition_key_cols {
        if !ordinals.contains(ordinal) || !partition_ordinals.insert(*ordinal) {
            bail!(
                "table '{table_id}' partition_key_cols references a missing or duplicate column ordinal {ordinal}"
            );
        }
    }

    if table.kind == "temporal-current" {
        if table.temporal_history.is_empty() {
            bail!("table '{table_id}' is temporal-current but has no temporal_history reference");
        }
        let history = blueprint
            .tables
            .get(&table.temporal_history)
            .with_context(|| {
                format!(
                    "table '{table_id}' references missing temporal history table '{}'",
                    table.temporal_history
                )
            })?;
        if table.temporal_history == table_id || history.kind != "temporal-history" {
            bail!(
                "table '{table_id}' temporal_history must reference a different temporal-history table"
            );
        }
    } else if !table.temporal_history.is_empty() {
        bail!("table '{table_id}' has temporal_history without kind = 'temporal-current'");
    }

    match (table.kind.as_str(), table.counted_in_totals) {
        ("external", Some(false)) => {}
        ("external", _) => {
            bail!("table '{table_id}' is external and must set counted_in_totals = false")
        }
        (_, None) => {}
        (_, Some(true)) => bail!(
            "table '{table_id}' uses non-canonical counted_in_totals = true; omit the field instead"
        ),
        (_, Some(false)) => {
            bail!("table '{table_id}' sets counted_in_totals = false without kind = 'external'")
        }
    }

    let column_check_count = u64::try_from(
        table
            .cols
            .values()
            .filter(|column| column.has_check == Some(true))
            .count(),
    )
    .context("column CHECK count exceeds the supported u64 range")?;
    if column_check_count > table.check_count.unwrap_or(0) {
        bail!(
            "table '{table_id}' has {column_check_count} checked columns above its declared check_count"
        );
    }

    Ok(())
}

fn validate_topology_and_dataset_scope(blueprint: &BlueprintFile) -> Result<()> {
    if blueprint.schema_version < 6 {
        if blueprint.database_topology.is_some() || blueprint.dataset_scope.is_some() {
            bail!(
                "database_topology and dataset_scope require Blueprint schema_version 6 or newer"
            );
        }
        return Ok(());
    }

    let scope = blueprint
        .dataset_scope
        .as_ref()
        .context("schema-v6 Blueprint is missing dataset_scope")?;
    validate_dataset_scope(scope)?;

    let structured = matches!(blueprint.engine.as_str(), "parquet" | "avro");
    if structured {
        if blueprint.database_topology.is_some() {
            bail!("structured-file Blueprint must not contain database_topology");
        }
        if scope.layout != "structured-dataset" {
            bail!("structured-file Blueprint must use dataset_scope layout 'structured-dataset'");
        }
    } else {
        let topology = blueprint
            .database_topology
            .as_ref()
            .context("schema-v6 database Blueprint is missing database_topology")?;
        validate_database_topology(topology)?;
        if scope.layout == "structured-dataset" {
            bail!("database Blueprint must not use dataset_scope layout 'structured-dataset'");
        }
    }
    Ok(())
}

fn validate_database_topology(topology: &crate::DatabaseTopology) -> Result<()> {
    if topology.contract != crate::TOPOLOGY_CONTRACT {
        bail!(
            "unsupported database topology contract '{}'; expected '{}'",
            topology.contract,
            crate::TOPOLOGY_CONTRACT
        );
    }
    validate_token(
        "database topology deployment",
        &topology.deployment,
        &[
            "single-node",
            "replicated",
            "sharded",
            "distributed",
            "unknown",
        ],
    )?;
    validate_token(
        "database topology local_role",
        &topology.local_role,
        &[
            "standalone",
            "primary",
            "secondary",
            "coordinator",
            "worker",
            "member",
            "unknown",
        ],
    )?;
    validate_token(
        "database topology visibility",
        &topology.visibility,
        &["full", "partial", "unknown"],
    )?;
    if !topology.identifiers_redacted {
        bail!("database topology must assert identifiers_redacted = true");
    }

    const ROLES: &[&str] = &[
        "standalone",
        "primary",
        "secondary",
        "coordinator",
        "worker",
        "member",
        "unknown",
    ];
    let role_total =
        topology
            .role_counts
            .iter()
            .try_fold(0_u64, |total, (role, count)| -> Result<u64> {
                validate_token("database topology role", role, ROLES)?;
                total
                    .checked_add(*count)
                    .context("database topology role counts overflow u64")
            })?;
    if role_total > topology.member_count {
        bail!(
            "database topology role counts total {role_total} exceeds visible member_count {}",
            topology.member_count
        );
    }
    if topology.visibility == "full" {
        if topology.member_count == 0 || role_total != topology.member_count {
            bail!("full database topology visibility requires a complete nonzero role count");
        }
        if !topology.catalogs_unreadable.is_empty() {
            bail!("full database topology visibility cannot contain unreadable catalogs");
        }
        if topology.deployment == "unknown" {
            bail!("full database topology visibility cannot use deployment 'unknown'");
        }
    }
    if topology.deployment == "single-node"
        && (topology.visibility != "full"
            || topology.member_count != 1
            || topology.local_role != "standalone"
            || topology.role_counts.get("standalone") != Some(&1))
    {
        bail!("single-node database topology requires one fully visible standalone member");
    }
    if topology.local_role == "standalone" && topology.deployment != "single-node" {
        bail!("standalone local_role requires a single-node deployment");
    }

    validate_sorted_unique_tokens(
        "database topology feature",
        &topology.features,
        &[
            "citus",
            "mysql-asynchronous-replication",
            "mysql-galera",
            "mysql-group-replication",
            "mysql-ndb",
            "postgresql-streaming-replication",
            "sqlserver-availability-group",
            "vitess",
        ],
    )?;
    const CATALOGS: &[&str] = &[
        "citus-metadata",
        "citus-relation-size",
        "mysql-group-members",
        "mysql-replica-status",
        "mysql-server-identity",
        "mysql-storage-engines",
        "mysql-topology-capabilities",
        "mysql-vitess-identity",
        "mysql-wsrep-status",
        "pg-extension",
        "pg-is-in-recovery",
        "pg-stat-replication",
        "pg-stat-wal-receiver",
        "sqlserver-database-replica-states",
        "sqlserver-hadr-replica-states",
        "sqlserver-is-hadr-enabled",
    ];
    validate_sorted_unique_tokens(
        "database topology readable catalog",
        &topology.catalogs_read,
        CATALOGS,
    )?;
    validate_sorted_unique_tokens(
        "database topology unreadable catalog",
        &topology.catalogs_unreadable,
        CATALOGS,
    )?;
    if topology
        .catalogs_read
        .iter()
        .any(|catalog| topology.catalogs_unreadable.binary_search(catalog).is_ok())
    {
        bail!("database topology catalog cannot be both readable and unreadable");
    }
    Ok(())
}

fn validate_dataset_scope(scope: &crate::DatasetScope) -> Result<()> {
    if scope.contract != crate::DATASET_SCOPE_CONTRACT {
        bail!(
            "unsupported dataset scope contract '{}'; expected '{}'",
            scope.contract,
            crate::DATASET_SCOPE_CONTRACT
        );
    }
    validate_token(
        "dataset layout",
        &scope.layout,
        &[
            "full-copy",
            "sharded",
            "distributed",
            "structured-dataset",
            "unknown",
        ],
    )?;
    for (field, value) in [
        (
            "table inventory completeness",
            scope.table_inventory_completeness.as_str(),
        ),
        (
            "row count completeness",
            scope.row_count_completeness.as_str(),
        ),
        ("size completeness", scope.size_completeness.as_str()),
    ] {
        validate_token(field, value, &["complete", "incomplete", "unknown"])?;
    }
    validate_token(
        "dataset row count method",
        &scope.row_count_method,
        &[
            "postgres-planner-estimate",
            "mysql-table-statistics",
            "sqlserver-partition-counter",
            "parquet-footer",
            "avro-decoded-scan",
            "structured-dataset-aggregate",
            "distributed-aggregate",
            "unknown",
        ],
    )?;
    validate_token(
        "dataset size method",
        &scope.size_method,
        &[
            "postgres-local-relation-size",
            "mysql-information-schema",
            "sqlserver-partition-pages",
            "citus-distributed-relation-size",
            "parquet-footer",
            "avro-container",
            "structured-dataset-aggregate",
            "distributed-aggregate",
            "unknown",
        ],
    )?;
    validate_sorted_unique_tokens(
        "dataset limitation",
        &scope.limitations,
        &[
            "distributed-aggregate-unavailable",
            "distributed-row-count-unavailable",
            "distributed-size-unavailable",
            "external-data-unmeasured",
            "external-table-visibility-unknown",
            "failed-sources",
            "local-member-only",
            "replica-membership-unresolved",
            "row-counts-statistical",
            "selection-limited",
            "shard-membership-incomplete",
            "statistics-stale",
            "topology-unobserved",
            "topology-visibility-partial",
            "topology-visibility-unknown",
        ],
    )?;

    let incomplete = [
        scope.table_inventory_completeness.as_str(),
        scope.row_count_completeness.as_str(),
        scope.size_completeness.as_str(),
    ]
    .iter()
    .any(|value| *value != "complete");
    if incomplete && scope.limitations.is_empty() {
        bail!("incomplete or unknown dataset scope requires at least one limitation");
    }
    if scope.layout == "unknown"
        && ![
            scope.table_inventory_completeness.as_str(),
            scope.row_count_completeness.as_str(),
            scope.size_completeness.as_str(),
        ]
        .iter()
        .all(|value| *value == "unknown")
    {
        bail!("unknown dataset layout cannot claim complete or partially established coverage");
    }
    if scope.row_count_completeness == "complete" && scope.row_count_method == "unknown" {
        bail!("complete row-count coverage requires a known row_count_method");
    }
    if scope.size_completeness == "complete" && scope.size_method == "unknown" {
        bail!("complete size coverage requires a known size_method");
    }
    if matches!(scope.layout.as_str(), "sharded" | "distributed") {
        if scope.row_count_completeness == "complete"
            && scope.row_count_method != "distributed-aggregate"
        {
            bail!("complete distributed row coverage requires distributed-aggregate");
        }
        if scope.size_completeness == "complete"
            && !matches!(
                scope.size_method.as_str(),
                "citus-distributed-relation-size" | "distributed-aggregate"
            )
        {
            bail!("complete distributed size coverage requires a distributed size method");
        }
    }
    Ok(())
}

fn validate_sorted_unique_tokens(field: &str, values: &[String], allowed: &[&str]) -> Result<()> {
    let mut previous: Option<&str> = None;
    for value in values {
        validate_token(field, value, allowed)?;
        if previous.is_some_and(|previous| previous >= value.as_str()) {
            bail!("{field} values must be sorted and unique");
        }
        previous = Some(value);
    }
    Ok(())
}

fn validate_artifact_inventory(
    blueprint: &BlueprintFile,
    inventory: &crate::ArtifactInventory,
) -> Result<()> {
    if blueprint.schema_version < 4 {
        bail!("artifact_inventory requires Blueprint schema_version 4 or newer");
    }
    let legacy_contract =
        blueprint.schema_version == 4 && inventory.contract == LEGACY_ARTIFACT_CONTRACT;
    if inventory.contract != crate::ARTIFACT_CONTRACT && !legacy_contract {
        bail!(
            "unsupported artifact inventory contract '{}'; expected '{}'",
            inventory.contract,
            crate::ARTIFACT_CONTRACT
        );
    }
    validate_token(
        "artifact detail",
        &inventory.detail,
        &["none", "summary", "graph", "analyzed"],
    )?;
    validate_token(
        "artifact visibility",
        &inventory.visibility,
        &["full", "privilege_filtered", "unknown"],
    )?;
    let count_sum = inventory
        .counts_by_kind
        .values()
        .try_fold(0_u64, |total, count| total.checked_add(*count))
        .context("artifact counts_by_kind overflows u64")?;
    if count_sum != inventory.object_count {
        bail!(
            "artifact object_count {} does not match counts_by_kind sum {}",
            inventory.object_count,
            count_sum
        );
    }
    for kind in inventory.counts_by_kind.keys() {
        validate_artifact_kind(kind)?;
    }
    let external_sum = inventory
        .counts_by_external_class
        .values()
        .try_fold(0_u64, |total, count| total.checked_add(*count))
        .context("artifact counts_by_external_class overflows u64")?;
    if external_sum != inventory.external_prerequisite_count {
        bail!(
            "artifact external_prerequisite_count {} does not match class-count sum {}",
            inventory.external_prerequisite_count,
            external_sum
        );
    }
    for class in inventory.counts_by_external_class.keys() {
        validate_external_class(class)?;
    }
    for catalog in inventory
        .catalogs_read
        .iter()
        .chain(inventory.catalogs_unreadable.iter())
    {
        validate_catalog_label("artifact catalog", catalog)?;
    }
    for family in &inventory.families_not_inventoried {
        validate_closed_identifier("uninventoried artifact family", family)?;
    }
    if inventory.inventory_complete
        && (inventory.visibility != "full"
            || !inventory.catalogs_unreadable.is_empty()
            || !inventory.families_not_inventoried.is_empty())
    {
        bail!(
            "artifact inventory cannot be complete with filtered visibility or declared coverage gaps"
        );
    }
    if inventory.dependencies_complete && !inventory.catalogs_unreadable.is_empty() {
        bail!("artifact dependencies cannot be complete with unreadable catalogs");
    }
    if inventory.analysis_complete && inventory.detail != "analyzed" {
        bail!("artifact analysis_complete requires analyzed detail");
    }
    if inventory.detail == "none"
        && (inventory.object_count != 0
            || inventory.external_prerequisite_count != 0
            || !inventory.counts_by_kind.is_empty()
            || !inventory.counts_by_external_class.is_empty())
    {
        bail!("artifact detail none must not contain inventory counts");
    }
    if matches!(inventory.detail.as_str(), "none" | "summary") && !inventory.artifacts.is_empty() {
        bail!(
            "artifact detail '{}' must not contain a per-object graph",
            inventory.detail
        );
    }
    if matches!(inventory.detail.as_str(), "graph" | "analyzed")
        && inventory.artifacts.len() as u64 != inventory.object_count
    {
        bail!(
            "artifact graph contains {} records but object_count is {}",
            inventory.artifacts.len(),
            inventory.object_count
        );
    }

    let valid_ids = inventory
        .artifacts
        .keys()
        .chain(blueprint.tables.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut observed_by_kind = std::collections::BTreeMap::<String, u64>::new();
    let mut observed_external = std::collections::BTreeMap::<String, u64>::new();
    let mut observed_edges = 0_u64;
    for (artifact_id, artifact) in &inventory.artifacts {
        validate_artifact_kind(&artifact.kind)?;
        if !artifact_id.starts_with(&format!("{}-", artifact.kind)) {
            bail!(
                "artifact id '{artifact_id}' does not match kind '{}'",
                artifact.kind
            );
        }
        validate_closed_identifier("artifact subkind", &artifact.subkind)?;
        validate_token(
            "artifact tier",
            &artifact.tier,
            &[
                "declarative",
                "programmatic",
                "external",
                "physical",
                "security",
                "other",
            ],
        )?;
        validate_token(
            "artifact definition_visibility",
            &artifact.definition_visibility,
            &[
                "not_applicable",
                "not_read",
                "available",
                "withheld",
                "unavailable",
                "encrypted",
                "external_binary",
            ],
        )?;
        if !artifact.security_mode.is_empty() {
            validate_token(
                "artifact security_mode",
                &artifact.security_mode,
                &["invoker", "definer", "caller", "owner", "principal"],
            )?;
        }
        if !artifact.schema.is_empty() && !artifact.schema.starts_with("schema-") {
            bail!("artifact '{artifact_id}' has a non-anonymous schema id");
        }
        if !artifact.parent.is_empty() && !blueprint.tables.contains_key(&artifact.parent) {
            bail!(
                "artifact '{artifact_id}' references missing parent '{}'",
                artifact.parent
            );
        }
        for dependency in &artifact.dependencies {
            if !valid_ids.contains(dependency) {
                bail!("artifact '{artifact_id}' references missing dependency '{dependency}'");
            }
        }
        observed_edges = observed_edges
            .checked_add(artifact.dependencies.len() as u64)
            .context("artifact dependency edge count overflows u64")?;
        *observed_by_kind.entry(artifact.kind.clone()).or_default() += 1;
        if let Some(external) = artifact.external.as_ref() {
            validate_external(external)?;
            *observed_external.entry(external.class.clone()).or_default() += 1;
        }
        if let Some(analysis) = artifact.analysis.as_ref() {
            if inventory.detail != "analyzed" {
                bail!("artifact '{artifact_id}' has analysis outside analyzed detail");
            }
            validate_language_census(analysis)?;
            if inventory.analysis_complete && analysis.status != "complete" {
                bail!(
                    "artifact analysis cannot be complete while '{artifact_id}' has status '{}'",
                    analysis.status
                );
            }
        }
    }
    if !inventory.artifacts.is_empty() {
        if observed_by_kind != inventory.counts_by_kind {
            bail!("artifact graph kind counts do not match counts_by_kind");
        }
        if observed_external != inventory.counts_by_external_class {
            bail!("artifact graph external counts do not match counts_by_external_class");
        }
        if observed_edges != inventory.dependency_edge_count {
            bail!(
                "artifact graph has {observed_edges} dependency edges but dependency_edge_count is {}",
                inventory.dependency_edge_count
            );
        }
    }
    Ok(())
}

fn validate_artifact_kind(kind: &str) -> Result<()> {
    validate_token(
        "artifact kind",
        kind,
        &[
            "view",
            "materialized_view",
            "sequence",
            "synonym",
            "type",
            "default",
            "function",
            "procedure",
            "aggregate",
            "trigger",
            "event_trigger",
            "rule",
            "scheduled_job",
            "policy",
            "extension",
            "foreign_server",
            "external_table",
            "publication",
            "subscription",
            "assembly",
            "full_text",
            "partition_scheme",
            "physical_placement",
            "certificate",
            "encryption_key",
            "other",
        ],
    )
}

fn validate_external(external: &crate::BlueprintExternalPrerequisite) -> Result<()> {
    validate_external_class(&external.class)?;
    validate_closed_identifier("external deployment_scope", &external.deployment_scope)?;
    validate_token(
        "external binary_material",
        &external.binary_material,
        &["not_captured", "required_not_captured"],
    )?;
    validate_token(
        "external secret_material",
        &external.secret_material,
        &[
            "not_captured",
            "required_not_captured",
            "may_be_required_not_captured",
        ],
    )?;
    validate_token(
        "external endpoint_material",
        &external.endpoint_material,
        &[
            "not_captured",
            "required_not_captured",
            "may_be_required_not_captured",
        ],
    )?;
    validate_closed_identifier("external compatibility", &external.compatibility)
}

fn validate_external_class(class: &str) -> Result<()> {
    validate_token(
        "external class",
        class,
        &[
            "postgresql_extension",
            "postgresql_native_function",
            "mysql_loadable_udf",
            "sqlserver_clr_assembly",
            "foreign_endpoint",
            "replication_topology",
            "physical_storage",
            "server_feature",
            "certificate_material",
            "encryption_or_credential_material",
            "sqlserver_agent",
        ],
    )
}

fn validate_language_census(analysis: &crate::LanguageFeatureCensus) -> Result<()> {
    if analysis.contract != crate::LANGUAGE_CENSUS_CONTRACT {
        bail!(
            "unsupported language census contract '{}'; expected '{}'",
            analysis.contract,
            crate::LANGUAGE_CENSUS_CONTRACT
        );
    }
    validate_token(
        "language census status",
        &analysis.status,
        &["complete", "partial", "unavailable", "not_applicable"],
    )?;
    validate_token(
        "language census dialect",
        &analysis.dialect,
        &[
            "sql",
            "plpgsql",
            "plpython",
            "plperl",
            "mysql-sql-psm",
            "tsql",
            "clr",
            "c",
            "internal",
            "unknown",
        ],
    )?;
    validate_token(
        "language analyzer version",
        &analysis.analyzer_version,
        &["lexical-v1"],
    )?;
    validate_profile(&analysis.grammar_profile)?;
    if !analysis.compatibility_level.is_empty()
        && (analysis.compatibility_level.len() > 8
            || !analysis
                .compatibility_level
                .bytes()
                .all(|byte| byte.is_ascii_digit()))
    {
        bail!("language compatibility_level contains a non-canonical value");
    }
    if !analysis.ansi_nulls.is_empty() {
        validate_token(
            "language ansi_nulls",
            &analysis.ansi_nulls,
            &["on", "off", "unknown"],
        )?;
    }
    if !analysis.quoted_identifier.is_empty() {
        validate_token(
            "language quoted_identifier",
            &analysis.quoted_identifier,
            &["on", "off", "unknown"],
        )?;
    }
    const SQL_MODES: &[&str] = &[
        "ALLOW_INVALID_DATES",
        "ANSI",
        "ANSI_QUOTES",
        "ERROR_FOR_DIVISION_BY_ZERO",
        "HIGH_NOT_PRECEDENCE",
        "IGNORE_SPACE",
        "NO_AUTO_VALUE_ON_ZERO",
        "NO_BACKSLASH_ESCAPES",
        "NO_DIR_IN_CREATE",
        "NO_ENGINE_SUBSTITUTION",
        "NO_UNSIGNED_SUBTRACTION",
        "NO_ZERO_DATE",
        "NO_ZERO_IN_DATE",
        "ONLY_FULL_GROUP_BY",
        "PIPES_AS_CONCAT",
        "REAL_AS_FLOAT",
        "STRICT_ALL_TABLES",
        "STRICT_TRANS_TABLES",
        "TIME_TRUNCATE_FRACTIONAL",
    ];
    let mut previous_mode: Option<&str> = None;
    for mode in &analysis.sql_mode_flags {
        validate_token("language sql_mode", mode, SQL_MODES)?;
        if previous_mode.is_some_and(|previous| previous >= mode.as_str()) {
            bail!("language sql_mode_flags must be sorted and unique");
        }
        previous_mode = Some(mode);
    }
    for (field, value) in [
        (
            "definition_size_band",
            analysis.definition_size_band.as_str(),
        ),
        (
            "statement_count_band",
            analysis.statement_count_band.as_str(),
        ),
        ("token_count_band", analysis.token_count_band.as_str()),
        (
            "maximum_nesting_band",
            analysis.maximum_nesting_band.as_str(),
        ),
        (
            "cyclomatic_complexity_band",
            analysis.cyclomatic_complexity_band.as_str(),
        ),
        (
            "opaque_region_count_band",
            analysis.opaque_region_count_band.as_str(),
        ),
    ] {
        if !value.is_empty() {
            let allowed: &[&str] = if field == "definition_size_band" {
                &["0", "1-255", "256-1k", "1k-4k", "4k-16k", "16k-64k", "64k+"]
            } else {
                &["0", "1", "2-4", "5-8", "9-16", "17-32", "33+"]
            };
            validate_token(field, value, allowed)?;
        }
    }
    const FEATURES: &[&str] = &[
        "control.if",
        "control.case",
        "control.loop",
        "control.while",
        "control.repeat",
        "control.exception",
        "interface.cursor",
        "query.join",
        "query.subquery",
        "query.cte",
        "query.recursive",
        "query.aggregate",
        "query.window",
        "query.group_by",
        "query.set_operation",
        "query.order_by",
        "query.limit",
        "data.select",
        "data.insert",
        "data.update",
        "data.delete",
        "data.merge",
        "state.ddl",
        "state.temporary",
        "transaction.control",
        "dynamic.sql",
        "type.json",
        "type.xml",
        "type.spatial",
        "type.vector",
        "security.definer",
        "security.invoker",
        "security.impersonation",
    ];
    for (feature, band) in &analysis.features {
        validate_token("language feature", feature, FEATURES)?;
        validate_token(
            "language feature count band",
            band,
            &["0", "1", "2-4", "5-8", "9-16", "17-32", "33+"],
        )?;
    }
    Ok(())
}

fn validate_closed_identifier(field: &str, value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 96
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        bail!("{field} contains a non-canonical value");
    }
    Ok(())
}

fn validate_catalog_label(field: &str, value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'.')
        })
    {
        bail!("{field} contains a non-canonical value");
    }
    Ok(())
}

fn validate_profile(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 64
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'.' | b'-')
        })
    {
        bail!("language grammar_profile contains a non-canonical value");
    }
    Ok(())
}

fn validate_token(field: &str, value: &str, allowed: &[&str]) -> Result<()> {
    if !allowed.contains(&value) {
        bail!("{field} has unsupported value '{value}'");
    }
    Ok(())
}

fn validate_cardinality(
    table_id: &str,
    column_id: &str,
    cardinality: Option<&crate::BlueprintCardinality>,
) -> Result<()> {
    let Some(cardinality) = cardinality else {
        return Ok(());
    };
    if cardinality.non_null_rows > cardinality.sample_rows {
        bail!(
            "table '{table_id}' column '{column_id}' cardinality has {} non-NULL rows above {} sampled rows",
            cardinality.non_null_rows,
            cardinality.sample_rows
        );
    }
    if cardinality.observed_distinct_count > cardinality.non_null_rows {
        bail!(
            "table '{table_id}' column '{column_id}' cardinality has {} distinct values above {} non-NULL rows",
            cardinality.observed_distinct_count,
            cardinality.non_null_rows
        );
    }
    if cardinality.estimated_distinct_count > 0
        && cardinality.estimated_distinct_count < cardinality.observed_distinct_count
    {
        bail!(
            "table '{table_id}' column '{column_id}' estimated cardinality is below its observed cardinality"
        );
    }
    if cardinality.frequency_max > cardinality.non_null_rows {
        bail!(
            "table '{table_id}' column '{column_id}' maximum frequency {} exceeds {} non-NULL sampled rows",
            cardinality.frequency_max,
            cardinality.non_null_rows
        );
    }
    validate_fraction(
        table_id,
        format!("column '{column_id}' top_value_fraction").as_str(),
        cardinality.top_value_fraction,
    )?;
    validate_percentiles(
        table_id,
        format!("column '{column_id}' frequency").as_str(),
        cardinality.frequency_p50,
        cardinality.frequency_p95,
        cardinality.frequency_p99,
        cardinality.frequency_max,
    )?;
    validate_bias(
        table_id,
        format!("column '{column_id}' cardinality").as_str(),
        cardinality.sampled_with_bias,
        cardinality.bias_reason.as_str(),
    )
}

fn validate_relationship(
    child_id: &str,
    parent_id: &str,
    statistics: Option<&crate::BlueprintRelationship>,
) -> Result<()> {
    let Some(statistics) = statistics else {
        return Ok(());
    };
    let scope = format!("foreign key from '{child_id}' to '{parent_id}'");
    if statistics.non_null_rows > statistics.sample_rows {
        bail!(
            "{scope} has {} non-NULL rows above {} sampled rows",
            statistics.non_null_rows,
            statistics.sample_rows
        );
    }
    if statistics.distinct_parent_values > statistics.non_null_rows {
        bail!("{scope} has more distinct parent values than non-NULL sampled rows");
    }
    if statistics.orphan_rows > statistics.non_null_rows {
        bail!("{scope} has more orphan rows than non-NULL sampled rows");
    }
    if statistics.fanout_max > statistics.non_null_rows {
        bail!("{scope} has maximum fanout above its non-NULL sampled rows");
    }
    validate_fraction(
        child_id,
        format!("{scope} parent_coverage_fraction").as_str(),
        statistics.parent_coverage_fraction,
    )?;
    validate_percentiles(
        child_id,
        format!("{scope} fanout").as_str(),
        statistics.fanout_p50,
        statistics.fanout_p95,
        statistics.fanout_p99,
        statistics.fanout_max,
    )?;
    validate_bias(
        child_id,
        scope.as_str(),
        statistics.sampled_with_bias,
        statistics.bias_reason.as_str(),
    )
}

fn validate_foreign_key_semantics(child_id: &str, edge: &crate::FkEdge) -> Result<()> {
    let scope = format!("foreign key from '{child_id}' to '{}'", edge.to);
    for (field, action) in [
        ("on_update", edge.on_update.as_str()),
        ("on_delete", edge.on_delete.as_str()),
    ] {
        if !matches!(
            action,
            "" | "no-action" | "restrict" | "cascade" | "set-null" | "set-default"
        ) {
            bail!("{scope} has unsupported {field} action '{action}'");
        }
    }
    if !matches!(edge.match_type.as_str(), "" | "simple" | "full" | "partial") {
        bail!("{scope} has unsupported match_type '{}'", edge.match_type);
    }
    if edge.initially_deferred && !edge.deferrable {
        bail!("{scope} is initially deferred but not deferrable");
    }
    if edge.validated
        && edge
            .statistics
            .as_ref()
            .is_some_and(|statistics| statistics.orphan_rows > 0)
    {
        bail!("{scope} is validated but its statistics report orphan rows");
    }
    Ok(())
}

fn validate_fraction(table_id: &str, field: &str, value: f64) -> Result<()> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        bail!("table '{table_id}' {field} is outside 0.0..=1.0: {value}");
    }
    Ok(())
}

fn validate_percentiles(
    table_id: &str,
    field: &str,
    p50: u64,
    p95: u64,
    p99: u64,
    max: u64,
) -> Result<()> {
    if p50 > p95 || p95 > p99 || p99 > max {
        bail!("table '{table_id}' {field} percentiles are not monotonic");
    }
    Ok(())
}

fn validate_bias(
    table_id: &str,
    scope: &str,
    sampled_with_bias: bool,
    bias_reason: &str,
) -> Result<()> {
    if sampled_with_bias && bias_reason.trim().is_empty() {
        bail!("table '{table_id}' biased {scope} must include a bias_reason");
    }
    if !sampled_with_bias && !bias_reason.is_empty() {
        bail!("table '{table_id}' unbiased {scope} must not include a bias_reason");
    }
    Ok(())
}

/// Compatibility entry point for validating an external serializable model
/// against the canonical shared-core contract. Callers already holding a
/// `BlueprintFile` should use `validate_blueprint_contract` to avoid the
/// serialization boundary.
pub fn validate_blueprint<T>(blueprint: &T) -> Result<()>
where
    T: serde::Serialize + ?Sized,
{
    let encoded =
        toml::to_string(blueprint).context("serializing Blueprint for canonical validation")?;
    let canonical: BlueprintFile = toml::from_str(&encoded)
        .context("decoding Blueprint into the canonical validation model")?;
    validate_blueprint_contract(&canonical)
}

fn computed_blueprint_totals(blueprint: &BlueprintFile) -> Result<Totals> {
    Ok(Totals {
        table_count: u64::try_from(
            blueprint
                .tables
                .values()
                .filter(|table| table.counts_toward_totals())
                .count(),
        )
        .context("Blueprint counted table total exceeds the supported u64 range")?,
        row_count: checked_table_sum(blueprint, "row_count", |table| table.rows)?,
        table_bytes: checked_table_sum(blueprint, "table_bytes", |table| table.table_bytes)?,
        index_bytes: checked_table_sum(blueprint, "index_bytes", |table| table.index_bytes)?,
    })
}

fn checked_table_sum(
    blueprint: &BlueprintFile,
    field: &str,
    value: impl Fn(&crate::BlueprintTable) -> u64,
) -> Result<u64> {
    blueprint
        .tables
        .values()
        .filter(|table| table.counts_toward_totals())
        .try_fold(0u64, |total, table| {
            total.checked_add(value(table)).with_context(|| {
                format!("Blueprint {field} overflows u64 while summing table blocks")
            })
        })
}

fn validate_total(field: &str, declared: u64, computed: u64, required: bool) -> Result<()> {
    if declared != computed && (required || declared != 0) {
        bail!("Blueprint totals declare {declared} for {field} but table blocks compute to {computed}");
    }
    Ok(())
}

fn validate_compression(
    schema_version: u32,
    table_id: &str,
    column_id: Option<&str>,
    compression: Option<&crate::BlueprintCompression>,
) -> Result<()> {
    let Some(compression) = compression else {
        return Ok(());
    };
    if !compression.sample_encoding.is_empty()
        && compression.sample_encoding != SAMPLE_ENCODING_TAG
        && !STRUCTURED_SAMPLE_ENCODINGS.contains(&compression.sample_encoding.as_str())
        && !(schema_version == 4 && compression.sample_encoding == LEGACY_SAMPLE_ENCODING_TAG)
    {
        bail!(
            "table '{table_id}' uses unsupported sample_encoding '{}'",
            compression.sample_encoding
        );
    }
    if compression.sampled_with_bias && compression.bias_reason.trim().is_empty() {
        bail!("table '{table_id}' biased compression sample must include a non-empty bias_reason");
    }
    if !compression.sampled_with_bias && !compression.bias_reason.is_empty() {
        bail!("table '{table_id}' unbiased compression sample must not include a bias_reason");
    }
    for (name, ratio) in [
        ("ratio_zstd_3", compression.ratio_zstd_3),
        ("ratio_zstd_19", compression.ratio_zstd_19),
        ("ratio_stddev", compression.ratio_stddev),
        ("ratio_storage", compression.ratio_storage),
    ] {
        if !ratio.is_finite() || ratio < 0.0 {
            let scope = column_id
                .map(|column| format!("column '{column}'"))
                .unwrap_or_else(|| "table compression".to_string());
            bail!("table '{table_id}' {scope} has invalid {name} {ratio}");
        }
    }
    Ok(())
}

fn normalize_blueprint_identifiers(blueprint: &mut BlueprintFile) {
    if blueprint.schema_version != 4 {
        return;
    }
    if let Some(inventory) = blueprint.artifact_inventory.as_mut() {
        if inventory.contract == LEGACY_ARTIFACT_CONTRACT {
            inventory.contract = crate::ARTIFACT_CONTRACT.to_string();
        }
    }
    for table in blueprint.tables.values_mut() {
        normalize_compression_identifier(table.compression.as_mut());
        for column in table.cols.values_mut() {
            normalize_compression_identifier(column.compression.as_mut());
        }
    }
}

fn normalize_compression_identifier(compression: Option<&mut crate::BlueprintCompression>) {
    if let Some(compression) = compression {
        if compression.sample_encoding == LEGACY_SAMPLE_ENCODING_TAG {
            compression.sample_encoding = SAMPLE_ENCODING_TAG.to_string();
        }
    }
}

pub fn read_blueprint_toml(path: impl AsRef<Path>) -> Result<BlueprintFile> {
    let path = path.as_ref();
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    parse_blueprint_toml(&text).with_context(|| format!("parsing {}", path.display()))
}

pub fn parse_blueprint_bundle_toml(text: &str) -> Result<BlueprintBundle> {
    let mut bundle: BlueprintBundle =
        toml::from_str(text).context("parsing Blueprint bundle TOML")?;
    if bundle.schema_version == 0 {
        bundle.schema_version = BUNDLE_SCHEMA_VERSION;
    }
    let source_schema_version = bundle.schema_version;
    let legacy = source_schema_version == LEGACY_BUNDLE_SCHEMA_VERSION;
    let previous = source_schema_version == PREVIOUS_BUNDLE_SCHEMA_VERSION;
    if !legacy && !previous && source_schema_version != BUNDLE_SCHEMA_VERSION {
        bail!(
            "unsupported Blueprint bundle schema_version {}; expected {}, previous {}, or legacy {}",
            bundle.schema_version,
            BUNDLE_SCHEMA_VERSION,
            PREVIOUS_BUNDLE_SCHEMA_VERSION,
            LEGACY_BUNDLE_SCHEMA_VERSION
        );
    }
    if bundle.kind.is_empty() {
        bundle.kind = BUNDLE_KIND.to_string();
    } else if bundle.kind != BUNDLE_KIND && !(legacy && bundle.kind == LEGACY_BUNDLE_KIND) {
        bail!(
            "unsupported Blueprint bundle kind '{}'; expected '{}'",
            bundle.kind,
            BUNDLE_KIND
        );
    }
    for source in bundle.sources.values_mut() {
        if let Some(blueprint) = source.blueprint.as_mut() {
            normalize_blueprint_identifiers(blueprint);
            if blueprint.schema_version == 4 {
                blueprint.schema_version = LEGACY_IDENTIFIER_SCHEMA_VERSION;
            }
        }
    }
    if legacy || previous {
        validate_legacy_bundle_contract(&bundle)?;
        upgrade_legacy_bundle_relationships(&mut bundle)?;
        recompute_bundle_totals(&mut bundle)?;
    }
    bundle.schema_version = BUNDLE_SCHEMA_VERSION;
    bundle.kind = BUNDLE_KIND.to_string();
    validate_blueprint_bundle_contract(&bundle)?;
    Ok(bundle)
}

fn validate_legacy_bundle_contract(bundle: &BlueprintBundle) -> Result<()> {
    if !bundle.dataset_groups.is_empty()
        || !bundle.bundle_totals.aggregation.is_empty()
        || bundle.bundle_totals.logical_dataset_count != 0
        || !bundle.bundle_totals.limitations.is_empty()
    {
        bail!("legacy Blueprint bundle contains schema-v3 relationship fields");
    }
    let expected_failed_count = bundle.failed_sources.len() as u64;
    if bundle.failed_source_count != expected_failed_count
        || bundle.partial != (expected_failed_count > 0)
    {
        bail!("legacy Blueprint bundle failure summary is contradictory");
    }
    let mut failed = BTreeSet::new();
    for source_id in &bundle.failed_sources {
        if source_id.trim().is_empty()
            || !failed.insert(source_id)
            || bundle.sources.contains_key(source_id)
        {
            bail!("legacy Blueprint bundle contains an invalid failed source id");
        }
    }
    for (source_id, source) in &bundle.sources {
        if source_id.trim().is_empty()
            || !source.dataset_relationship.is_empty()
            || !source.dataset_group.is_empty()
            || !source.dataset_scope_completeness.is_empty()
        {
            bail!("legacy Blueprint bundle contains schema-v3 source fields");
        }
        if let Some(blueprint) = &source.blueprint {
            validate_blueprint_contract(blueprint).with_context(|| {
                format!("validating embedded Blueprint for source '{source_id}'")
            })?;
            validate_embedded_source_summary(source_id, source, blueprint)?;
        }
    }
    let expected = computed_legacy_bundle_totals(bundle)?;
    validate_legacy_bundle_totals(&bundle.bundle_totals, &expected)
}

fn upgrade_legacy_bundle_relationships(bundle: &mut BlueprintBundle) -> Result<()> {
    let mut source_ids: Vec<String> = bundle
        .sources
        .keys()
        .chain(bundle.failed_sources.iter())
        .cloned()
        .collect();
    source_ids.sort();
    source_ids.dedup();
    bundle.dataset_groups.clear();
    for (index, source_id) in source_ids.into_iter().enumerate() {
        let group_id = format!("legacy-dataset-{:03}", index + 1);
        bundle.dataset_groups.insert(
            group_id.clone(),
            crate::BundleDatasetGroup {
                relationship: "unknown".to_string(),
                members_complete: false,
                members: vec![source_id.clone()],
            },
        );
        if let Some(source) = bundle.sources.get_mut(&source_id) {
            source.dataset_relationship = "unknown".to_string();
            source.dataset_group = group_id;
            source.dataset_scope_completeness = source
                .blueprint
                .as_ref()
                .map(crate::blueprint_dataset_scope_completeness)
                .unwrap_or("unknown")
                .to_string();
        }
    }
    bundle.schema_version = BUNDLE_SCHEMA_VERSION;
    bundle.kind = BUNDLE_KIND.to_string();
    Ok(())
}

pub fn validate_blueprint_bundle_contract(bundle: &BlueprintBundle) -> Result<()> {
    if bundle.schema_version != BUNDLE_SCHEMA_VERSION {
        bail!(
            "unsupported Blueprint bundle schema_version {}; expected {}",
            bundle.schema_version,
            BUNDLE_SCHEMA_VERSION
        );
    }
    if bundle.kind != BUNDLE_KIND {
        bail!(
            "unsupported Blueprint bundle kind '{}'; expected '{}'",
            bundle.kind,
            BUNDLE_KIND
        );
    }

    let expected_failed_count = bundle.failed_sources.len() as u64;
    if bundle.failed_source_count != expected_failed_count {
        bail!(
            "Blueprint bundle failed_source_count is {} but {} failed source ids are present",
            bundle.failed_source_count,
            expected_failed_count
        );
    }
    let expected_partial = expected_failed_count > 0;
    if bundle.partial != expected_partial {
        bail!(
            "Blueprint bundle partial is {} but failed source state requires {}",
            bundle.partial,
            expected_partial
        );
    }

    let mut failed = BTreeSet::new();
    for source_id in &bundle.failed_sources {
        if source_id.trim().is_empty() || !failed.insert(source_id) {
            bail!("Blueprint bundle contains an empty or duplicate failed source id");
        }
        if bundle.sources.contains_key(source_id) {
            bail!("Blueprint bundle source '{source_id}' is both successful and failed");
        }
    }
    for (source_id, source) in &bundle.sources {
        if source_id.trim().is_empty() {
            bail!("Blueprint bundle contains an empty source id");
        }
        if let Some(blueprint) = &source.blueprint {
            validate_blueprint_contract(blueprint).with_context(|| {
                format!("validating embedded Blueprint for source '{source_id}'")
            })?;
            validate_embedded_source_summary(source_id, source, blueprint)?;
        }
    }

    validate_bundle_dataset_groups(bundle)?;

    let expected_totals = computed_bundle_totals(bundle)?;
    validate_bundle_totals(&bundle.bundle_totals, &expected_totals)?;
    Ok(())
}

fn validate_embedded_source_summary(
    source_id: &str,
    source: &BundleSource,
    blueprint: &BlueprintFile,
) -> Result<()> {
    let totals = computed_blueprint_totals(blueprint)?;
    for (field, declared, expected) in [
        ("table_count", source.table_count, totals.table_count),
        ("row_count", source.row_count, totals.row_count),
        ("table_bytes", source.table_bytes, totals.table_bytes),
        ("index_bytes", source.index_bytes, totals.index_bytes),
    ] {
        if declared != expected {
            bail!(
                "Blueprint bundle source '{source_id}' declares {declared} for {field} but its embedded Blueprint computes to {expected}"
            );
        }
    }
    for (field, declared, expected) in [
        ("engine", source.engine.as_str(), blueprint.engine.as_str()),
        (
            "engine_version",
            source.engine_version.as_str(),
            blueprint.engine_version.as_str(),
        ),
        (
            "source_kind",
            source.source_kind.as_str(),
            blueprint.source_kind.as_str(),
        ),
    ] {
        if declared != expected {
            bail!(
                "Blueprint bundle source '{source_id}' declares {field} '{declared}' but its embedded Blueprint declares '{expected}'"
            );
        }
    }
    if !source.dataset_scope_completeness.is_empty() {
        let expected = crate::blueprint_dataset_scope_completeness(blueprint);
        if source.dataset_scope_completeness != expected {
            bail!(
                "Blueprint bundle source '{source_id}' declares dataset_scope_completeness '{}' but its embedded Blueprint computes to '{expected}'",
                source.dataset_scope_completeness
            );
        }
    }
    Ok(())
}

fn validate_bundle_dataset_groups(bundle: &BlueprintBundle) -> Result<()> {
    if bundle.sources.is_empty() && bundle.failed_sources.is_empty() {
        bail!("Blueprint bundle contains no successful or failed sources");
    }
    let failed: BTreeSet<&String> = bundle.failed_sources.iter().collect();
    let mut memberships: std::collections::BTreeMap<&str, &str> = std::collections::BTreeMap::new();
    for (group_id, group) in &bundle.dataset_groups {
        if group_id.is_empty()
            || group_id.trim() != group_id
            || group_id.len() > 120
            || !group_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            bail!(
                "Blueprint bundle dataset group '{group_id}' is not a safe bundle-local identifier"
            );
        }
        if !matches!(
            group.relationship.as_str(),
            "independent" | "replica" | "shard" | "unknown"
        ) {
            bail!(
                "Blueprint bundle dataset group '{group_id}' has unsupported relationship '{}'",
                group.relationship
            );
        }
        if group.members.is_empty() {
            bail!("Blueprint bundle dataset group '{group_id}' has no members");
        }
        if group.relationship == "independent"
            && (group.members.len() != 1 || !group.members_complete)
        {
            bail!(
                "independent Blueprint bundle dataset group '{group_id}' requires exactly one complete member"
            );
        }
        if group.relationship == "unknown" && group.members_complete {
            bail!(
                "unknown Blueprint bundle dataset group '{group_id}' cannot claim complete membership"
            );
        }
        let mut previous: Option<&str> = None;
        for member in &group.members {
            if member.trim().is_empty() || previous.is_some_and(|value| value >= member.as_str()) {
                bail!(
                    "Blueprint bundle dataset group '{group_id}' members must be sorted, unique, and nonempty"
                );
            }
            previous = Some(member);
            if !bundle.sources.contains_key(member) && !failed.contains(member) {
                bail!(
                    "Blueprint bundle dataset group '{group_id}' names unknown source '{member}'"
                );
            }
            if let Some(previous_group) = memberships.insert(member, group_id) {
                bail!(
                    "Blueprint bundle source '{member}' belongs to both '{previous_group}' and '{group_id}'"
                );
            }
        }
    }

    for source_id in bundle.sources.keys().chain(bundle.failed_sources.iter()) {
        if !memberships.contains_key(source_id.as_str()) {
            bail!("Blueprint bundle source '{source_id}' has no dataset group");
        }
    }
    for (source_id, source) in &bundle.sources {
        if !matches!(
            source.dataset_scope_completeness.as_str(),
            "complete" | "incomplete" | "unknown"
        ) {
            bail!(
                "Blueprint bundle source '{source_id}' has invalid dataset_scope_completeness '{}'",
                source.dataset_scope_completeness
            );
        }
        let group = bundle
            .dataset_groups
            .get(&source.dataset_group)
            .with_context(|| {
                format!(
                    "Blueprint bundle source '{source_id}' names missing dataset group '{}'",
                    source.dataset_group
                )
            })?;
        if group.relationship != source.dataset_relationship {
            bail!(
                "Blueprint bundle source '{source_id}' relationship '{}' disagrees with dataset group '{}' relationship '{}'",
                source.dataset_relationship,
                source.dataset_group,
                group.relationship
            );
        }
        if memberships.get(source_id.as_str()).copied() != Some(source.dataset_group.as_str()) {
            bail!(
                "Blueprint bundle source '{source_id}' does not appear in its declared dataset group '{}'",
                source.dataset_group
            );
        }
    }
    Ok(())
}

fn computed_bundle_totals(bundle: &BlueprintBundle) -> Result<BundleTotals> {
    let mut computed = bundle.clone();
    recompute_bundle_totals(&mut computed)?;
    Ok(computed.bundle_totals)
}

fn validate_bundle_totals(declared: &BundleTotals, expected: &BundleTotals) -> Result<()> {
    if declared.aggregation != expected.aggregation {
        bail!(
            "Blueprint bundle totals declare aggregation '{}' but source relationships compute to '{}'",
            declared.aggregation,
            expected.aggregation
        );
    }
    for (field, declared, expected) in [
        ("source_count", declared.source_count, expected.source_count),
        (
            "logical_dataset_count",
            declared.logical_dataset_count,
            expected.logical_dataset_count,
        ),
        ("table_count", declared.table_count, expected.table_count),
        ("row_count", declared.row_count, expected.row_count),
        ("table_bytes", declared.table_bytes, expected.table_bytes),
        ("index_bytes", declared.index_bytes, expected.index_bytes),
    ] {
        if declared != expected {
            bail!(
                "Blueprint bundle totals declare {declared} for {field} but source summaries compute to {expected}"
            );
        }
    }
    if declared.limitations != expected.limitations {
        bail!(
            "Blueprint bundle totals declare limitations {:?} but source relationships compute to {:?}",
            declared.limitations,
            expected.limitations
        );
    }
    Ok(())
}

fn computed_legacy_bundle_totals(bundle: &BlueprintBundle) -> Result<BundleTotals> {
    let mut totals = BundleTotals {
        source_count: u64::try_from(bundle.sources.len())
            .context("legacy Blueprint bundle source count exceeds u64")?,
        ..Default::default()
    };
    for source in bundle.sources.values() {
        totals.table_count = totals
            .table_count
            .checked_add(source.table_count)
            .context("legacy Blueprint bundle table_count overflows u64")?;
        totals.row_count = totals
            .row_count
            .checked_add(source.row_count)
            .context("legacy Blueprint bundle row_count overflows u64")?;
        totals.table_bytes = totals
            .table_bytes
            .checked_add(source.table_bytes)
            .context("legacy Blueprint bundle table_bytes overflows u64")?;
        totals.index_bytes = totals
            .index_bytes
            .checked_add(source.index_bytes)
            .context("legacy Blueprint bundle index_bytes overflows u64")?;
    }
    Ok(totals)
}

fn validate_legacy_bundle_totals(declared: &BundleTotals, expected: &BundleTotals) -> Result<()> {
    for (field, declared, expected) in [
        ("source_count", declared.source_count, expected.source_count),
        ("table_count", declared.table_count, expected.table_count),
        ("row_count", declared.row_count, expected.row_count),
        ("table_bytes", declared.table_bytes, expected.table_bytes),
        ("index_bytes", declared.index_bytes, expected.index_bytes),
    ] {
        if declared != expected {
            bail!(
                "legacy Blueprint bundle totals declare {declared} for {field} but source summaries compute to {expected}"
            );
        }
    }
    Ok(())
}

pub fn read_blueprint_bundle_toml(path: impl AsRef<Path>) -> Result<BlueprintBundle> {
    let path = path.as_ref();
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    parse_blueprint_bundle_toml(&text).with_context(|| format!("parsing {}", path.display()))
}

/// Canonical comment header for every emitted Blueprint or Blueprint bundle.
///
/// The header describes the format's privacy boundary but deliberately does
/// not declare the file safe for a particular transfer channel or recipient.
pub const BLUEPRINT_TOML_HEADER: &str = "\
# dbwarp-blueprint v6
# Anonymous database Blueprint. Source object names and row values are excluded.
# Review under your organization's data-classification policy before sharing.
# https://github.com/DBWarp/dbwarp-blueprint

";

fn prepend_blueprint_header(body: String) -> String {
    let mut output = String::with_capacity(BLUEPRINT_TOML_HEADER.len() + body.len());
    output.push_str(BLUEPRINT_TOML_HEADER);
    output.push_str(&body);
    output
}

pub fn blueprint_to_toml(blueprint: &BlueprintFile) -> Result<String> {
    let mut blueprint = blueprint.clone();
    normalize_blueprint_identifiers(&mut blueprint);
    if blueprint.schema_version == 4 {
        blueprint.schema_version = LEGACY_IDENTIFIER_SCHEMA_VERSION;
    }
    validate_blueprint_contract(&blueprint)?;
    let body = toml::to_string_pretty(&blueprint).context("serializing Blueprint TOML")?;
    Ok(prepend_blueprint_header(body))
}

pub fn blueprint_bundle_to_toml(bundle: &BlueprintBundle) -> Result<String> {
    let mut bundle = bundle.clone();
    bundle.schema_version = BUNDLE_SCHEMA_VERSION;
    bundle.kind = BUNDLE_KIND.to_string();
    for source in bundle.sources.values_mut() {
        if let Some(blueprint) = source.blueprint.as_mut() {
            normalize_blueprint_identifiers(blueprint);
            if blueprint.schema_version == 4 {
                blueprint.schema_version = LEGACY_IDENTIFIER_SCHEMA_VERSION;
            }
        }
    }
    validate_blueprint_bundle_contract(&bundle)?;
    let body = toml::to_string_pretty(&bundle).context("serializing Blueprint bundle TOML")?;
    Ok(prepend_blueprint_header(body))
}

pub fn blueprint_bundle_with_embedded_blueprints(
    mut bundle: BlueprintBundle,
    bundle_path: impl AsRef<Path>,
) -> Result<BlueprintBundle> {
    let bundle_path = bundle_path.as_ref();
    let base = bundle_path.parent().unwrap_or_else(|| Path::new("."));
    for (source_id, source) in bundle.sources.iter_mut() {
        if source.blueprint.is_some() {
            continue;
        }
        let blueprint_path = source.blueprint_path.as_ref().with_context(|| {
            format!("bundle source '{source_id}' has neither blueprint nor blueprint_path")
        })?;
        let resolved = resolve_bundle_path_checked(base, blueprint_path)?;
        let blueprint = read_blueprint_toml(&resolved)
            .with_context(|| format!("reading Blueprint for bundle source '{source_id}'"))?;
        source.blueprint = Some(blueprint);
    }
    validate_blueprint_bundle_contract(&bundle)?;
    recompute_bundle_totals(&mut bundle)?;
    Ok(bundle)
}

/// Resolve an existing bundle child path without allowing the reference to
/// leave the canonical bundle directory.
pub fn resolve_bundle_path_checked(base: &Path, relative: &str) -> Result<PathBuf> {
    let relative_path = Path::new(relative);
    if relative.trim().is_empty() {
        bail!("bundle child path must not be empty");
    }
    if relative_path.is_absolute() {
        bail!("bundle child path '{relative}' must be relative to the bundle directory");
    }
    let mut has_normal_component = false;
    for component in relative_path.components() {
        match component {
            Component::Normal(_) => has_normal_component = true,
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                bail!(
                    "bundle child path '{relative}' contains a forbidden traversal or root component"
                )
            }
        }
    }
    if !has_normal_component {
        bail!("bundle child path '{relative}' does not identify a child object");
    }

    let canonical_base = fs::canonicalize(base)
        .with_context(|| format!("canonicalizing bundle directory {}", base.display()))?;
    if !canonical_base.is_dir() {
        bail!(
            "bundle base {} is not a directory",
            canonical_base.display()
        );
    }
    let joined = canonical_base.join(relative_path);
    let canonical_child = fs::canonicalize(&joined)
        .with_context(|| format!("canonicalizing bundle child path {}", joined.display()))?;
    if !canonical_child.starts_with(&canonical_base) || canonical_child == canonical_base {
        bail!(
            "bundle child path '{relative}' resolves outside bundle directory {}",
            canonical_base.display()
        );
    }
    Ok(canonical_child)
}

pub fn blueprint_uri_to_path(input: &str) -> Result<PathBuf> {
    if let Some(rest) = input.strip_prefix("blueprint://") {
        if rest.is_empty() {
            bail!("DBP1200E blueprint:// URI requires a path. Next: use blueprint://path/to/bundle.toml#source=ID.");
        }
        Ok(PathBuf::from(rest))
    } else if input.contains("://") {
        bail!(
            "DBP1200E unsupported Blueprint URI scheme. Next: use blueprint://path/to/blueprint.toml."
        )
    } else {
        Ok(Path::new(input).to_path_buf())
    }
}

pub fn split_blueprint_uri_selector(input: &str) -> (&str, Option<&str>) {
    match input.split_once('#') {
        Some((path, selector)) => (path, Some(selector)),
        None => (input, None),
    }
}

pub fn parse_blueprint_selector(input: &str) -> Result<BlueprintSelector> {
    let mut selector = BlueprintSelector::default();
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(selector);
    }
    for part in trimmed
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        let (key, value) = part
            .split_once('=')
            .with_context(|| {
                format!("DBP1200E selector part '{part}' must be key=value. Next: use source=ID, table=ID, engine=NAME, or tag=NAME.")
            })?;
        let value = value.trim();
        if value.is_empty() {
            bail!(
                "DBP1200E selector key '{}' has an empty value. Next: provide a non-empty selector value.",
                key.trim()
            );
        }
        match key.trim() {
            "source" => selector.source = Some(value.to_string()),
            "table" => selector.table = Some(value.to_string()),
            "engine" => selector.engine = Some(value.to_ascii_lowercase()),
            "tag" => selector.tag = Some(value.to_string()),
            other => {
                bail!(
                    "DBP1200E unsupported selector key '{other}'. Next: use source, table, engine, or tag."
                )
            }
        }
    }
    Ok(selector)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ArtifactInventory, BlueprintArtifact, BlueprintBundle, BlueprintColumn,
        BlueprintCompression, BlueprintIndex, BlueprintTable, BundleSource, FkEdge,
        LanguageFeatureCensus, Totals, ARTIFACT_CONTRACT, BUNDLE_KIND, BUNDLE_SCHEMA_VERSION,
        LANGUAGE_CENSUS_CONTRACT, SCHEMA_VERSION,
    };
    use std::collections::BTreeMap;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("tmp")
            .join("blueprint-core-tests")
            .join(format!("{name}-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn one_table_blueprint() -> BlueprintFile {
        let table = BlueprintTable {
            rows: 7,
            table_bytes: 70,
            index_bytes: 14,
            ..Default::default()
        };
        BlueprintFile {
            schema_version: SCHEMA_VERSION,
            engine: "postgresql".into(),
            engine_version: "18".into(),
            source_kind: "production".into(),
            totals: Totals {
                table_count: 1,
                row_count: 7,
                table_bytes: 70,
                index_bytes: 14,
            },
            database_topology: Some(crate::DatabaseTopology::unknown()),
            dataset_scope: Some(crate::DatasetScope::unknown_database(
                "postgres-planner-estimate",
                "postgres-local-relation-size",
            )),
            tables: BTreeMap::from([("table-001".into(), table)]),
            ..Default::default()
        }
    }

    fn embedded_bundle() -> BlueprintBundle {
        let blueprint = one_table_blueprint();
        let mut bundle = BlueprintBundle {
            schema_version: BUNDLE_SCHEMA_VERSION,
            kind: BUNDLE_KIND.into(),
            sources: BTreeMap::from([(
                "source-a".into(),
                BundleSource {
                    kind: "database".into(),
                    dataset_relationship: "independent".into(),
                    dataset_group: "dataset-a".into(),
                    blueprint: Some(blueprint),
                    ..Default::default()
                },
            )]),
            dataset_groups: BTreeMap::from([(
                "dataset-a".into(),
                crate::BundleDatasetGroup {
                    relationship: "independent".into(),
                    members_complete: true,
                    members: vec!["source-a".into()],
                },
            )]),
            ..Default::default()
        };
        recompute_bundle_totals(&mut bundle).expect("test bundle totals");
        bundle
    }

    fn empty_artifact_inventory() -> ArtifactInventory {
        ArtifactInventory {
            contract: ARTIFACT_CONTRACT.into(),
            detail: "summary".into(),
            visibility: "full".into(),
            ..Default::default()
        }
    }

    #[test]
    fn schema_v6_requires_scope_without_inventing_it_for_v5() {
        let mut current = one_table_blueprint();
        current.dataset_scope = None;
        assert!(validate_blueprint_contract(&current).is_err());

        current.schema_version = 5;
        current.database_topology = None;
        assert!(validate_blueprint_contract(&current).is_ok());

        current.dataset_scope = Some(crate::DatasetScope::unknown_database(
            "postgres-planner-estimate",
            "postgres-local-relation-size",
        ));
        assert!(validate_blueprint_contract(&current).is_err());
    }

    #[test]
    fn schema_v6_accepts_complete_full_copy_and_unknown_evidence() {
        let unknown = one_table_blueprint();
        assert!(validate_blueprint_contract(&unknown).is_ok());

        let mut complete = unknown;
        complete.database_topology = Some(crate::DatabaseTopology {
            contract: crate::TOPOLOGY_CONTRACT.into(),
            deployment: "single-node".into(),
            local_role: "standalone".into(),
            visibility: "full".into(),
            member_count: 1,
            identifiers_redacted: true,
            role_counts: BTreeMap::from([("standalone".into(), 1)]),
            catalogs_read: vec!["pg-extension".into(), "pg-is-in-recovery".into()],
            ..Default::default()
        });
        complete.dataset_scope = Some(crate::DatasetScope {
            contract: crate::DATASET_SCOPE_CONTRACT.into(),
            layout: "full-copy".into(),
            table_inventory_completeness: "complete".into(),
            row_count_completeness: "complete".into(),
            size_completeness: "complete".into(),
            row_count_method: "postgres-planner-estimate".into(),
            size_method: "postgres-local-relation-size".into(),
            limitations: vec!["row-counts-statistical".into()],
        });
        assert!(validate_blueprint_contract(&complete).is_ok());

        let encoded = blueprint_to_toml(&complete).unwrap();
        assert!(encoded.starts_with(BLUEPRINT_TOML_HEADER));
        let decoded = parse_blueprint_toml(&encoded).unwrap();
        assert_eq!(decoded.database_topology, complete.database_topology);
        assert_eq!(decoded.dataset_scope, complete.dataset_scope);
    }

    #[test]
    fn every_structured_serializer_emits_the_canonical_header() {
        let blueprint = one_table_blueprint();
        assert!(blueprint_to_toml(&blueprint)
            .unwrap()
            .starts_with(BLUEPRINT_TOML_HEADER));

        let bundle = embedded_bundle();
        assert!(blueprint_bundle_to_toml(&bundle)
            .unwrap()
            .starts_with(BLUEPRINT_TOML_HEADER));
    }

    #[test]
    fn measured_zero_compression_stddev_is_emitted_and_round_trips() {
        let mut blueprint = one_table_blueprint();
        blueprint.tables.get_mut("table-001").unwrap().compression = Some(BlueprintCompression {
            measured: true,
            sample_rows: 7,
            sample_bytes: 64,
            sample_method: "test bounded sample".into(),
            ratio_zstd_3: 2.5,
            ratio_stddev: 0.0,
            sample_encoding: SAMPLE_ENCODING_TAG.into(),
            ..Default::default()
        });

        let encoded = blueprint_to_toml(&blueprint).unwrap();
        assert!(encoded.contains("ratio_zstd_3 = 2.5"));
        assert!(encoded.contains("ratio_stddev = 0.0"));

        let decoded = parse_blueprint_toml(&encoded).unwrap();
        let compression = decoded.tables["table-001"].compression.as_ref().unwrap();
        assert!(compression.measured);
        assert_eq!(compression.ratio_zstd_3, 2.5);
        assert_eq!(compression.ratio_stddev, 0.0);

        // Older schema-v6 emitters could omit an exact zero. Preserve that
        // input compatibility even though current emitters must be explicit.
        let older = encoded.replacen("ratio_stddev = 0.0\n", "", 1);
        assert!(!older.contains("ratio_stddev"));
        let decoded_older = parse_blueprint_toml(&older).unwrap();
        assert_eq!(
            decoded_older.tables["table-001"]
                .compression
                .as_ref()
                .unwrap()
                .ratio_stddev,
            0.0
        );
    }

    #[test]
    fn schema_v6_rejects_contradictory_or_noncanonical_topology() {
        let mut blueprint = one_table_blueprint();
        let topology = blueprint.database_topology.as_mut().unwrap();
        topology.visibility = "full".into();
        topology.deployment = "replicated".into();
        topology.member_count = 2;
        topology.role_counts = BTreeMap::from([("primary".into(), 1)]);
        assert!(validate_blueprint_contract(&blueprint).is_err());

        let topology = blueprint.database_topology.as_mut().unwrap();
        topology.visibility = "partial".into();
        topology.features = vec!["customer-cluster-name".into()];
        assert!(validate_blueprint_contract(&blueprint).is_err());

        let topology = blueprint.database_topology.as_mut().unwrap();
        topology.features.clear();
        topology.catalogs_read = vec!["pg-stat-replication".into(), "pg-is-in-recovery".into()];
        assert!(validate_blueprint_contract(&blueprint).is_err());
    }

    #[test]
    fn schema_v6_rejects_local_methods_for_complete_distributed_data() {
        let mut blueprint = one_table_blueprint();
        let scope = blueprint.dataset_scope.as_mut().unwrap();
        scope.layout = "distributed".into();
        scope.table_inventory_completeness = "complete".into();
        scope.row_count_completeness = "complete".into();
        scope.size_completeness = "complete".into();
        scope.limitations = vec!["row-counts-statistical".into()];
        assert!(validate_blueprint_contract(&blueprint).is_err());

        let scope = blueprint.dataset_scope.as_mut().unwrap();
        scope.row_count_method = "distributed-aggregate".into();
        scope.size_method = "citus-distributed-relation-size".into();
        assert!(validate_blueprint_contract(&blueprint).is_ok());
    }

    #[test]
    fn schema_v6_structured_files_require_scope_but_forbid_database_topology() {
        let mut blueprint = one_table_blueprint();
        blueprint.engine = "parquet".into();
        blueprint.database_topology = None;
        blueprint.dataset_scope = Some(crate::DatasetScope::structured_dataset(
            "parquet-footer",
            "parquet-footer",
        ));
        assert!(validate_blueprint_contract(&blueprint).is_ok());

        blueprint.database_topology = Some(crate::DatabaseTopology::unknown());
        assert!(validate_blueprint_contract(&blueprint).is_err());
    }

    #[test]
    fn schema_v6_round_trip_preserves_locked_table_and_column_semantics() {
        let mut blueprint = one_table_blueprint();
        let partitioned: BlueprintTable = toml::from_str(
            r#"
rows = 7
table_bytes = 70
index_bytes = 14
kind = "partitioned"
unlogged = false
partition_strategy = "range"
partition_count = 4
partition_key_cols = [1]
partition_rows_max = 3
check_count = 1

[cols.col-1]
ordinal = 1
type = "integer"
value_source = "identity-always"
has_default = false
hidden = false
masked = false
encrypted = false
sparse = false
has_check = true
magnitude_min = -2
magnitude_max = 6
has_negative = true

[cols.col-2]
ordinal = 2
type = "text"
type_kind = "enum"
member_count = 3
has_default = true
default_kind = "constant"

[cols.col-3]
ordinal = 3
type = "user-defined"
type_kind = "domain"
domain_has_check = true

[cols.col-4]
ordinal = 4
type = "timestamp"
time_span = "years"
time_recent_decade = 2020
"#,
        )
        .expect("schema-v6 table fixture must deserialize");
        let current = BlueprintTable {
            rows: 5,
            table_bytes: 50,
            kind: "temporal-current".into(),
            temporal_history: "table-history".into(),
            ..Default::default()
        };
        let history = BlueprintTable {
            rows: 5,
            table_bytes: 50,
            kind: "temporal-history".into(),
            ..Default::default()
        };
        let external = BlueprintTable {
            rows: 99,
            table_bytes: 999,
            index_bytes: 99,
            kind: "external".into(),
            counted_in_totals: Some(false),
            check_count: Some(0),
            ..Default::default()
        };
        blueprint.tables = BTreeMap::from([
            ("table-001".into(), partitioned),
            ("table-current".into(), current),
            ("table-external".into(), external),
            ("table-history".into(), history),
        ]);
        blueprint.totals = Totals {
            table_count: 3,
            row_count: 17,
            table_bytes: 170,
            index_bytes: 14,
        };

        let encoded = blueprint_to_toml(&blueprint).unwrap();
        assert!(encoded.contains("unlogged = false"));
        assert!(encoded.contains("counted_in_totals = false"));
        assert!(encoded.contains("check_count = 0"));
        assert!(!encoded.contains("distinct_ratio"));

        let decoded = parse_blueprint_toml(&encoded).unwrap();
        let partitioned = &decoded.tables["table-001"];
        assert_eq!(partitioned.partition_count, Some(4));
        assert_eq!(partitioned.partition_key_cols, vec![1]);
        assert_eq!(partitioned.cols["col-1"].has_default, Some(false));
        assert_eq!(partitioned.cols["col-2"].member_count, Some(3));
        assert_eq!(partitioned.cols["col-4"].time_recent_decade, Some(2020));
        assert_eq!(decoded.tables["table-external"].check_count, Some(0));
        assert_eq!(decoded.totals.table_count, 3);
        assert_eq!(decoded.totals.row_count, 17);
    }

    #[test]
    fn table_and_column_semantics_are_schema_v6_only() {
        let mut blueprint = one_table_blueprint();
        blueprint.schema_version = 5;
        blueprint.database_topology = None;
        blueprint.dataset_scope = None;
        blueprint.tables.get_mut("table-001").unwrap().kind = "materialized-view".into();
        assert!(validate_blueprint_contract(&blueprint)
            .unwrap_err()
            .to_string()
            .contains("before Blueprint schema_version 6"));

        let table = blueprint.tables.get_mut("table-001").unwrap();
        table.kind.clear();
        table.cols.insert(
            "col-1".into(),
            BlueprintColumn {
                ordinal: 1,
                column_type: "text".into(),
                hidden: Some(false),
                ..Default::default()
            },
        );
        assert!(validate_blueprint_contract(&blueprint)
            .unwrap_err()
            .to_string()
            .contains("before Blueprint schema_version 6"));
    }

    #[test]
    fn schema_v6_semantic_invariants_fail_closed() {
        let mut blueprint = one_table_blueprint();
        blueprint.tables.get_mut("table-001").unwrap().kind = "partitioned".into();
        assert!(validate_blueprint_contract(&blueprint).is_err());

        let table = blueprint.tables.get_mut("table-001").unwrap();
        table.kind.clear();
        table.cols.insert(
            "col-1".into(),
            BlueprintColumn {
                ordinal: 1,
                column_type: "text".into(),
                type_kind: "enum".into(),
                member_count: Some(0),
                ..Default::default()
            },
        );
        assert!(validate_blueprint_contract(&blueprint).is_err());

        let column = blueprint
            .tables
            .get_mut("table-001")
            .unwrap()
            .cols
            .get_mut("col-1")
            .unwrap();
        column.type_kind.clear();
        column.member_count = None;
        column.has_default = Some(false);
        column.default_kind = "constant".into();
        assert!(validate_blueprint_contract(&blueprint).is_err());

        let column = blueprint
            .tables
            .get_mut("table-001")
            .unwrap()
            .cols
            .get_mut("col-1")
            .unwrap();
        column.has_default = None;
        column.default_kind.clear();
        column.magnitude_min = Some(2);
        column.magnitude_max = Some(1);
        column.has_negative = Some(false);
        assert!(validate_blueprint_contract(&blueprint).is_err());

        let column = blueprint
            .tables
            .get_mut("table-001")
            .unwrap()
            .cols
            .get_mut("col-1")
            .unwrap();
        column.magnitude_min = None;
        column.magnitude_max = None;
        column.has_negative = None;
        column.time_span = "years".into();
        column.time_recent_decade = Some(2026);
        assert!(validate_blueprint_contract(&blueprint).is_err());
    }

    #[test]
    fn external_tables_are_excluded_from_blueprint_and_bundle_totals() {
        let mut blueprint = one_table_blueprint();
        blueprint.tables.insert(
            "table-external".into(),
            BlueprintTable {
                rows: 900,
                table_bytes: 9_000,
                index_bytes: 900,
                kind: "external".into(),
                counted_in_totals: Some(false),
                ..Default::default()
            },
        );
        assert!(validate_blueprint_contract(&blueprint).is_ok());

        let mut bundle = embedded_bundle();
        bundle.sources.get_mut("source-a").unwrap().blueprint = Some(blueprint);
        recompute_bundle_totals(&mut bundle).unwrap();
        let source = &bundle.sources["source-a"];
        assert_eq!(source.table_count, 1);
        assert_eq!(source.row_count, 7);
        assert_eq!(source.table_bytes, 70);
        assert_eq!(source.index_bytes, 14);
        assert_eq!(bundle.bundle_totals.table_count, 1);
        assert_eq!(bundle.bundle_totals.row_count, 7);
    }

    #[test]
    fn artifact_completeness_claims_fail_closed() {
        let mut blueprint = one_table_blueprint();
        let mut inventory = empty_artifact_inventory();
        inventory.inventory_complete = true;
        inventory.visibility = "privilege_filtered".into();
        blueprint.artifact_inventory = Some(inventory.clone());
        assert!(validate_blueprint_contract(&blueprint).is_err());

        inventory.visibility = "full".into();
        inventory.catalogs_unreadable = vec!["sys.objects".into()];
        blueprint.artifact_inventory = Some(inventory.clone());
        assert!(validate_blueprint_contract(&blueprint).is_err());

        inventory.catalogs_unreadable.clear();
        inventory.families_not_inventoried = vec!["roles".into()];
        blueprint.artifact_inventory = Some(inventory);
        assert!(validate_blueprint_contract(&blueprint).is_err());
    }

    #[test]
    fn dependency_and_analysis_completeness_claims_require_matching_evidence() {
        let mut blueprint = one_table_blueprint();
        let mut inventory = empty_artifact_inventory();
        inventory.dependencies_complete = true;
        inventory.catalogs_unreadable = vec!["pg_depend".into()];
        blueprint.artifact_inventory = Some(inventory);
        assert!(validate_blueprint_contract(&blueprint).is_err());

        let mut inventory = empty_artifact_inventory();
        inventory.detail = "analyzed".into();
        inventory.analysis_complete = true;
        inventory.object_count = 1;
        inventory.counts_by_kind = BTreeMap::from([("view".into(), 1)]);
        inventory.artifacts = BTreeMap::from([(
            "view-001".into(),
            BlueprintArtifact {
                kind: "view".into(),
                subkind: "ordinary".into(),
                tier: "declarative".into(),
                definition_visibility: "available".into(),
                analysis: Some(LanguageFeatureCensus {
                    contract: LANGUAGE_CENSUS_CONTRACT.into(),
                    status: "partial".into(),
                    dialect: "sql".into(),
                    grammar_profile: "postgresql-18".into(),
                    analyzer_version: "lexical-v1".into(),
                    ..Default::default()
                }),
                ..Default::default()
            },
        )]);
        blueprint.artifact_inventory = Some(inventory);
        assert!(validate_blueprint_contract(&blueprint).is_err());
    }

    #[test]
    fn blueprint_round_trip_preserves_exact_length_and_index_metadata() {
        let mut blueprint = BlueprintFile {
            schema_version: SCHEMA_VERSION,
            generated_at: "2026-07-31T00:00:00Z".into(),
            engine: "mysql".into(),
            engine_version: "8.4".into(),
            source_kind: "production".into(),
            length_metadata: "exact".into(),
            declared_length_fidelity: "exact".into(),
            index_length_fidelity: "exact".into(),
            observed_length_fidelity: "exact".into(),
            totals: Totals {
                table_count: 1,
                ..Default::default()
            },
            database_topology: Some(crate::DatabaseTopology::unknown()),
            dataset_scope: Some(crate::DatasetScope::unknown_database(
                "mysql-table-statistics",
                "mysql-information-schema",
            )),
            ..BlueprintFile::default()
        };
        let mut table = BlueprintTable::default();
        table.cols.insert(
            "col-1".into(),
            BlueprintColumn {
                ordinal: 1,
                column_type: "varchar".into(),
                native_type: "varchar".into(),
                declared_max_chars: 191,
                declared_max_bytes: 764,
                charset: "utf8mb4".into(),
                collation: "utf8mb4_0900_ai_ci".into(),
                ..BlueprintColumn::default()
            },
        );
        table.idxs.insert(
            "idx-1".into(),
            BlueprintIndex {
                index_type: "btree".into(),
                unique: true,
                primary: true,
                cols: vec![1],
                prefix_lengths: vec![32],
                ..BlueprintIndex::default()
            },
        );
        blueprint.tables = BTreeMap::from([("table-001".into(), table)]);

        let encoded = blueprint_to_toml(&blueprint).unwrap();
        let decoded = parse_blueprint_toml(&encoded).unwrap();
        assert_eq!(decoded.length_metadata, "exact");
        assert_eq!(decoded.declared_length_fidelity, "exact");
        assert_eq!(decoded.index_length_fidelity, "exact");
        assert_eq!(decoded.observed_length_fidelity, "exact");
        let column = &decoded.tables["table-001"].cols["col-1"];
        assert_eq!(column.declared_max_chars, 191);
        assert_eq!(column.declared_max_bytes, 764);
        assert_eq!(decoded.tables["table-001"].idxs["idx-1"].cols, vec![1]);
        assert_eq!(
            decoded.tables["table-001"].idxs["idx-1"].prefix_lengths,
            vec![32]
        );
    }

    #[test]
    fn blueprint_uri_path_parses_plain_and_scheme_paths_and_rejects_former_shape_uri() {
        assert_eq!(
            blueprint_uri_to_path("/tmp/a.toml").unwrap(),
            PathBuf::from("/tmp/a.toml")
        );
        assert_eq!(
            blueprint_uri_to_path("blueprint:///tmp/a.toml").unwrap(),
            PathBuf::from("/tmp/a.toml")
        );
        assert_eq!(
            blueprint_uri_to_path("blueprint://relative.toml").unwrap(),
            PathBuf::from("relative.toml")
        );
        assert!(blueprint_uri_to_path("shape://fixture.toml").is_err());
    }

    #[test]
    fn split_blueprint_uri_selector_keeps_path_and_fragment_separate() {
        assert_eq!(
            split_blueprint_uri_selector("blueprint:///tmp/a.toml#source=erp,table=t1"),
            ("blueprint:///tmp/a.toml", Some("source=erp,table=t1"))
        );
        assert_eq!(
            split_blueprint_uri_selector("/tmp/a.toml"),
            ("/tmp/a.toml", None)
        );
    }

    #[test]
    fn parse_blueprint_selector_supports_source_table_engine_and_tag() {
        let selector =
            parse_blueprint_selector("source=erp, table=table-001, engine=Postgres, tag=hot")
                .expect("selector parses");
        assert_eq!(selector.source.as_deref(), Some("erp"));
        assert_eq!(selector.table.as_deref(), Some("table-001"));
        assert_eq!(selector.engine.as_deref(), Some("postgres"));
        assert_eq!(selector.tag.as_deref(), Some("hot"));
    }

    #[test]
    fn bundle_contract_rejects_unknown_fields_versions_and_kinds() {
        let unknown = r#"
schema_version = 1
kind = "dbwarp-shape-bundle"
unexpected = true
"#;
        assert!(parse_blueprint_bundle_toml(unknown).is_err());

        let version = r#"
schema_version = 99
kind = "dbwarp-blueprint-bundle"
"#;
        assert!(parse_blueprint_bundle_toml(version).is_err());

        let kind = r#"
schema_version = 1
kind = "other"
"#;
        assert!(parse_blueprint_bundle_toml(kind).is_err());
    }

    #[test]
    fn bundle_contract_normalizes_legacy_defaults_and_validates_embedded_blueprints() {
        assert!(parse_blueprint_bundle_toml("").is_err());

        let invalid_embedded = r#"
schema_version = 1
kind = "dbwarp-blueprint-bundle"

[sources.bad]
kind = "database"

[sources.bad.shape]
schema_version = 99
engine = "postgresql"
"#;
        assert!(parse_blueprint_bundle_toml(invalid_embedded).is_err());
    }

    #[test]
    fn legacy_bundle_identifiers_parse_and_reemit_only_blueprint_identifiers() {
        let legacy = r#"
schema_version = 1
kind = "dbwarp-shape-bundle"

[bundle_totals]
source_count = 1
table_count = 0
row_count = 0
table_bytes = 0
index_bytes = 0

[sources.source-a]
kind = "database"
shape_path = "blueprints/source-a.blueprint.toml"
table_count = 0
row_count = 0
table_bytes = 0
index_bytes = 0
"#;

        let parsed = parse_blueprint_bundle_toml(legacy).expect("legacy bundle parses");
        assert_eq!(parsed.schema_version, BUNDLE_SCHEMA_VERSION);
        assert_eq!(parsed.kind, BUNDLE_KIND);
        assert_eq!(
            parsed.sources["source-a"].blueprint_path.as_deref(),
            Some("blueprints/source-a.blueprint.toml")
        );

        let emitted = blueprint_bundle_to_toml(&parsed).expect("canonical bundle emits");
        assert!(emitted.contains("schema_version = 3"));
        assert!(emitted.contains("aggregation = \"suppressed\""));
        assert!(emitted.contains("relationship = \"unknown\""));
        assert!(emitted.contains("kind = \"dbwarp-blueprint-bundle\""));
        assert!(emitted.contains("blueprint_path = \"blueprints/source-a.blueprint.toml\""));
        assert!(!emitted.contains("dbwarp-shape-bundle"));
        assert!(!emitted.contains("shape_path"));
    }

    #[test]
    fn bundle_v2_upgrades_to_explicit_unknown_relationships_without_summing() {
        let canonical = blueprint_bundle_to_toml(&embedded_bundle()).unwrap();
        let mut previous: toml::Value = toml::from_str(&canonical).unwrap();
        let root = previous.as_table_mut().unwrap();
        root.insert("schema_version".into(), toml::Value::Integer(2));
        root.remove("dataset_groups");
        let totals = root["bundle_totals"].as_table_mut().unwrap();
        totals.remove("aggregation");
        totals.remove("logical_dataset_count");
        totals.remove("limitations");
        let source = root["sources"]["source-a"].as_table_mut().unwrap();
        source.remove("dataset_relationship");
        source.remove("dataset_group");
        source.remove("dataset_scope_completeness");
        let previous = toml::to_string_pretty(&previous).unwrap();

        let parsed = parse_blueprint_bundle_toml(&previous).expect("bundle v2 parses");
        assert_eq!(parsed.schema_version, BUNDLE_SCHEMA_VERSION);
        assert_eq!(parsed.bundle_totals.aggregation, "suppressed");
        assert_eq!(parsed.bundle_totals.source_count, 1);
        assert_eq!(parsed.bundle_totals.logical_dataset_count, 0);
        assert_eq!(parsed.bundle_totals.table_count, 0);
        assert_eq!(parsed.bundle_totals.row_count, 0);
        assert_eq!(parsed.sources["source-a"].dataset_relationship, "unknown");
        assert_eq!(
            parsed.sources["source-a"].dataset_group,
            "legacy-dataset-001"
        );
        assert_eq!(
            parsed.sources["source-a"].dataset_scope_completeness,
            "unknown"
        );
        assert_eq!(
            parsed.dataset_groups["legacy-dataset-001"].relationship,
            "unknown"
        );
        assert!(!parsed.dataset_groups["legacy-dataset-001"].members_complete);
    }

    #[test]
    fn legacy_embedded_field_reemits_as_blueprint() {
        let canonical = blueprint_bundle_to_toml(&embedded_bundle()).unwrap();
        let mut legacy: toml::Value = toml::from_str(&canonical).unwrap();
        let root = legacy.as_table_mut().unwrap();
        root.insert("schema_version".into(), toml::Value::Integer(1));
        root.insert(
            "kind".into(),
            toml::Value::String("dbwarp-shape-bundle".into()),
        );
        root.remove("dataset_groups");
        let totals = root["bundle_totals"].as_table_mut().unwrap();
        totals.remove("aggregation");
        totals.remove("logical_dataset_count");
        totals.remove("limitations");
        let source = root["sources"]["source-a"].as_table_mut().unwrap();
        source.remove("dataset_relationship");
        source.remove("dataset_group");
        source.remove("dataset_scope_completeness");
        let blueprint = source.remove("blueprint").unwrap();
        source.insert("shape".into(), blueprint);
        let legacy = toml::to_string_pretty(&legacy).unwrap();

        let parsed =
            parse_blueprint_bundle_toml(&legacy).expect("legacy embedded blueprint parses");
        assert!(parsed.sources["source-a"].blueprint.is_some());
        let emitted = blueprint_bundle_to_toml(&parsed).expect("canonical bundle emits");
        assert!(emitted.contains("[sources.source-a.blueprint]"));
        assert!(!emitted.contains("[sources.source-a.shape]"));
        assert!(!emitted.contains("dbwarp-shape-bundle"));
    }

    #[test]
    fn legacy_blueprint_tags_upgrade_but_current_documents_reject_them() {
        let mut legacy = one_table_blueprint();
        legacy.schema_version = 4;
        legacy.database_topology = None;
        legacy.dataset_scope = None;
        legacy.artifact_inventory = Some(ArtifactInventory {
            contract: LEGACY_ARTIFACT_CONTRACT.into(),
            detail: "summary".into(),
            visibility: "full".into(),
            ..Default::default()
        });
        legacy.tables.get_mut("table-001").unwrap().compression =
            Some(crate::BlueprintCompression {
                sample_encoding: LEGACY_SAMPLE_ENCODING_TAG.into(),
                ..Default::default()
            });

        let legacy_toml = toml::to_string_pretty(&legacy).unwrap();
        let parsed = parse_blueprint_toml(&legacy_toml).expect("legacy Blueprint parses");
        assert_eq!(parsed.schema_version, LEGACY_IDENTIFIER_SCHEMA_VERSION);
        assert!(parsed.database_topology.is_none());
        assert!(parsed.dataset_scope.is_none());
        assert_eq!(
            parsed.artifact_inventory.as_ref().unwrap().contract,
            ARTIFACT_CONTRACT
        );
        assert_eq!(
            parsed.tables["table-001"]
                .compression
                .as_ref()
                .unwrap()
                .sample_encoding,
            SAMPLE_ENCODING_TAG
        );

        let emitted = blueprint_to_toml(&parsed).expect("canonical Blueprint emits");
        assert!(!emitted.contains("dbwarp-shape-artifacts/v1"));
        assert!(!emitted.contains("dbwarp-shape-rowframe-v1"));
        assert!(emitted.contains("dbwarp-blueprint-artifacts/v1"));
        assert!(emitted.contains("dbwarp-blueprint-rowframe-v1"));

        let current_with_legacy_tags =
            legacy_toml.replacen("schema_version = 4", "schema_version = 5", 1);
        assert!(parse_blueprint_toml(&current_with_legacy_tags).is_err());
    }

    #[test]
    fn legacy_v1_indexes_without_ordinals_remain_readable_but_v2_refuses_them() {
        let legacy = r#"
schema_version = 1
engine = "mysql"

[tables.events]
rows = 10

[tables.events.idxs.pk]
type = "btree"
primary = true
unique = true
"#;
        assert!(parse_blueprint_toml(legacy).is_ok());
        let current = legacy.replacen("schema_version = 1", "schema_version = 2", 1);
        assert!(parse_blueprint_toml(&current).is_err());
    }

    #[test]
    fn expression_only_indexes_are_valid_in_v2_but_empty_ordinary_indexes_are_not() {
        let mut blueprint = one_table_blueprint();
        blueprint.tables.get_mut("table-001").unwrap().idxs.insert(
            "expression-index".into(),
            BlueprintIndex {
                index_type: "btree".into(),
                expression: true,
                ..Default::default()
            },
        );
        assert!(validate_blueprint_contract(&blueprint).is_ok());

        blueprint
            .tables
            .get_mut("table-001")
            .unwrap()
            .idxs
            .get_mut("expression-index")
            .unwrap()
            .expression = false;
        assert!(validate_blueprint_contract(&blueprint).is_err());
    }

    #[test]
    fn v1_may_omit_totals_but_cannot_supply_contradictory_totals() {
        let omitted = r#"
schema_version = 1

[tables.events]
rows = 5
table_bytes = 50
index_bytes = 10
"#;
        assert!(parse_blueprint_toml(omitted).is_ok());

        let contradictory = format!(
            "{omitted}\n[totals]\ntable_count = 1\nrow_count = 4\ntable_bytes = 50\nindex_bytes = 10\n"
        );
        assert!(parse_blueprint_toml(&contradictory).is_err());
    }

    #[test]
    fn v2_requires_exact_totals_for_every_aggregate() {
        let mut blueprint = one_table_blueprint();
        assert!(validate_blueprint(&blueprint).is_ok());
        for field in ["table_count", "row_count", "table_bytes", "index_bytes"] {
            let mut broken = blueprint.clone();
            match field {
                "table_count" => broken.totals.table_count = 0,
                "row_count" => broken.totals.row_count = 6,
                "table_bytes" => broken.totals.table_bytes = 69,
                "index_bytes" => broken.totals.index_bytes = 13,
                _ => unreachable!(),
            }
            assert!(
                validate_blueprint_contract(&broken).is_err(),
                "{field} mismatch must fail"
            );
        }
        assert!(validate_blueprint_contract(&blueprint).is_ok());
        blueprint.tables.clear();
        assert!(validate_blueprint_contract(&blueprint).is_err());
    }

    #[test]
    fn compression_bias_metadata_must_be_internally_consistent() {
        let mut blueprint = one_table_blueprint();
        blueprint.tables.get_mut("table-001").unwrap().compression = Some(BlueprintCompression {
            sampled_with_bias: true,
            ..Default::default()
        });
        assert!(validate_blueprint_contract(&blueprint).is_err());

        let compression = blueprint
            .tables
            .get_mut("table-001")
            .unwrap()
            .compression
            .as_mut()
            .unwrap();
        compression.sampled_with_bias = false;
        compression.bias_reason = "deterministic-first-n-rows".into();
        assert!(validate_blueprint_contract(&blueprint).is_err());

        let compression = blueprint
            .tables
            .get_mut("table-001")
            .unwrap()
            .compression
            .as_mut()
            .unwrap();
        compression.sampled_with_bias = true;
        assert!(validate_blueprint_contract(&blueprint).is_ok());
    }

    #[test]
    fn schema_v2_requires_parent_foreign_key_ordinals() {
        let mut blueprint = BlueprintFile {
            schema_version: SCHEMA_VERSION,
            engine: "postgresql".into(),
            database_topology: Some(crate::DatabaseTopology::unknown()),
            dataset_scope: Some(crate::DatasetScope::unknown_database(
                "postgres-planner-estimate",
                "postgres-local-relation-size",
            )),
            totals: Totals {
                table_count: 2,
                ..Default::default()
            },
            ..BlueprintFile::default()
        };
        let mut child = BlueprintTable::default();
        child.cols.insert(
            "child-id".into(),
            BlueprintColumn {
                ordinal: 1,
                column_type: "bigint".into(),
                ..BlueprintColumn::default()
            },
        );
        let mut parent = BlueprintTable::default();
        parent.cols.insert(
            "parent-id".into(),
            BlueprintColumn {
                ordinal: 1,
                column_type: "bigint".into(),
                ..BlueprintColumn::default()
            },
        );
        blueprint.tables.insert("child".into(), child);
        blueprint.tables.insert("parent".into(), parent);
        blueprint.fk_edges.insert(
            "child".into(),
            vec![FkEdge {
                to: "parent".into(),
                cols: vec![1],
                to_cols: vec![],
                ..Default::default()
            }],
        );

        let err = validate_blueprint_contract(&blueprint)
            .expect_err("v2 FK must identify parent columns");
        assert!(err.to_string().contains("schema-v2 foreign-key"));
    }

    #[test]
    fn schema_contract_rejects_unknown_fields_and_invalid_statistics() {
        let unknown = r#"
schema_version = 2
unexpected = "silent-drift"
"#;
        assert!(parse_blueprint_toml(unknown).is_err());

        let mut blueprint = BlueprintFile {
            schema_version: SCHEMA_VERSION,
            engine: "postgresql".into(),
            database_topology: Some(crate::DatabaseTopology::unknown()),
            dataset_scope: Some(crate::DatasetScope::unknown_database(
                "postgres-planner-estimate",
                "postgres-local-relation-size",
            )),
            totals: Totals {
                table_count: 1,
                ..Default::default()
            },
            ..BlueprintFile::default()
        };
        let mut table = BlueprintTable::default();
        table.cols.insert(
            "bad".into(),
            BlueprintColumn {
                ordinal: 1,
                column_type: "text".into(),
                null_fraction: Some(f64::NAN),
                ..BlueprintColumn::default()
            },
        );
        blueprint.tables.insert("table".into(), table);
        assert!(validate_blueprint_contract(&blueprint).is_err());
    }

    #[test]
    fn schema_v3_rejects_noncanonical_relationship_semantics() {
        let mut edge = FkEdge {
            to: "parent".into(),
            cols: vec![1],
            to_cols: vec![1],
            on_update: "no-action".into(),
            on_delete: "set-null".into(),
            match_type: "full".into(),
            deferrable: true,
            initially_deferred: true,
            ..Default::default()
        };
        assert!(validate_foreign_key_semantics("child", &edge).is_ok());

        edge.on_delete = "SET NULL".into();
        assert!(validate_foreign_key_semantics("child", &edge).is_err());
        edge.on_delete = "set-null".into();
        edge.deferrable = false;
        assert!(validate_foreign_key_semantics("child", &edge).is_err());

        edge.initially_deferred = false;
        edge.statistics = Some(crate::BlueprintRelationship {
            sample_rows: 10,
            non_null_rows: 10,
            orphan_rows: 1,
            ..Default::default()
        });
        assert!(validate_foreign_key_semantics("child", &edge).is_err());
    }

    #[test]
    fn schema_v3_rejects_frequency_counts_above_the_sample() {
        let cardinality = crate::BlueprintCardinality {
            measured: true,
            sample_rows: 10,
            non_null_rows: 8,
            observed_distinct_count: 2,
            estimated_distinct_count: 2,
            frequency_p50: 1,
            frequency_p95: 2,
            frequency_p99: 3,
            frequency_max: 9,
            ..Default::default()
        };
        assert!(validate_cardinality("table", "column", Some(&cardinality)).is_err());
    }

    #[test]
    fn schema_v3_index_prefix_cardinality_allows_unknown_slots_but_checks_known_values() {
        let mut blueprint = one_table_blueprint();
        let table = blueprint.tables.get_mut("table-001").unwrap();
        for ordinal in 1..=3 {
            table.cols.insert(
                format!("column-{ordinal}"),
                BlueprintColumn {
                    ordinal,
                    column_type: "integer".into(),
                    ..Default::default()
                },
            );
        }
        table.idxs.insert(
            "index-001".into(),
            BlueprintIndex {
                cols: vec![1, 2, 3],
                prefix_distinct_counts: vec![2, 0, 7],
                cardinality_sample_method: "bounded-test".into(),
                ..Default::default()
            },
        );
        assert!(validate_blueprint_contract(&blueprint).is_ok());

        blueprint
            .tables
            .get_mut("table-001")
            .unwrap()
            .idxs
            .get_mut("index-001")
            .unwrap()
            .prefix_distinct_counts = vec![3, 0, 2];
        assert!(validate_blueprint_contract(&blueprint).is_err());

        blueprint
            .tables
            .get_mut("table-001")
            .unwrap()
            .idxs
            .get_mut("index-001")
            .unwrap()
            .prefix_distinct_counts = vec![2, 0, 8];
        assert!(validate_blueprint_contract(&blueprint).is_err());
    }

    #[test]
    fn schema_v2_remains_readable_without_distribution_fields() {
        let text = r#"
schema_version = 2
engine = "postgresql"

[totals]
table_count = 1
row_count = 10
table_bytes = 100
index_bytes = 0

[tables.table-001]
rows = 10
table_bytes = 100

[tables.table-001.cols.col-001]
ordinal = 1
type = "bigint"
nullable = false
"#;
        let blueprint = parse_blueprint_toml(text).expect("schema v2 remains compatible");
        assert_eq!(blueprint.schema_version, 2);
        assert!(blueprint.tables["table-001"].cols["col-001"]
            .cardinality
            .is_none());
    }

    #[test]
    fn schema_v3_round_trip_preserves_distribution_and_relationship_statistics() {
        let mut blueprint = BlueprintFile {
            schema_version: SCHEMA_VERSION,
            engine: "postgresql".into(),
            database_topology: Some(crate::DatabaseTopology::unknown()),
            dataset_scope: Some(crate::DatasetScope::unknown_database(
                "postgres-planner-estimate",
                "postgres-local-relation-size",
            )),
            totals: Totals {
                table_count: 2,
                row_count: 120,
                table_bytes: 1_200,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut parent = BlueprintTable {
            rows: 20,
            table_bytes: 200,
            ..Default::default()
        };
        parent.cols.insert(
            "id".into(),
            BlueprintColumn {
                ordinal: 1,
                column_type: "bigint".into(),
                ..Default::default()
            },
        );
        parent.idxs.insert(
            "pk".into(),
            BlueprintIndex {
                primary: true,
                unique: true,
                cols: vec![1],
                prefix_distinct_counts: vec![20],
                cardinality_sample_method: "catalog-exact-unique".into(),
                ..Default::default()
            },
        );
        let mut child = BlueprintTable {
            rows: 100,
            table_bytes: 1_000,
            ..Default::default()
        };
        child.cols.insert(
            "parent-id".into(),
            BlueprintColumn {
                ordinal: 1,
                column_type: "bigint".into(),
                cardinality: Some(crate::BlueprintCardinality {
                    measured: true,
                    sample_rows: 100,
                    non_null_rows: 90,
                    observed_distinct_count: 10,
                    estimated_distinct_count: 10,
                    top_value_fraction: 0.25,
                    frequency_p50: 4,
                    frequency_p95: 10,
                    frequency_p99: 20,
                    frequency_max: 23,
                    sample_method: "test".into(),
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        blueprint.tables.insert("child".into(), child);
        blueprint.tables.insert("parent".into(), parent);
        blueprint.fk_edges.insert(
            "child".into(),
            vec![FkEdge {
                to: "parent".into(),
                cols: vec![1],
                to_cols: vec![1],
                on_delete: "cascade".into(),
                statistics: Some(crate::BlueprintRelationship {
                    measured: true,
                    sample_rows: 100,
                    non_null_rows: 90,
                    distinct_parent_values: 10,
                    parent_coverage_fraction: 0.5,
                    fanout_p50: 4,
                    fanout_p95: 10,
                    fanout_p99: 20,
                    fanout_max: 23,
                    sample_method: "test".into(),
                    ..Default::default()
                }),
                ..Default::default()
            }],
        );

        let encoded = blueprint_to_toml(&blueprint).unwrap();
        let decoded = parse_blueprint_toml(&encoded).unwrap();
        assert_eq!(
            decoded.tables["parent"].idxs["pk"].prefix_distinct_counts,
            vec![20]
        );
        assert_eq!(
            decoded.tables["child"].cols["parent-id"]
                .cardinality
                .as_ref()
                .unwrap()
                .estimated_distinct_count,
            10
        );
        assert_eq!(
            decoded.fk_edges["child"][0]
                .statistics
                .as_ref()
                .unwrap()
                .fanout_max,
            23
        );
        assert_eq!(decoded.fk_edges["child"][0].on_delete, "cascade");
    }

    #[test]
    fn bundle_contract_rejects_failure_partial_total_and_source_summary_contradictions() {
        let valid = embedded_bundle();
        assert!(validate_blueprint_bundle_contract(&valid).is_ok());

        let mut broken = valid.clone();
        broken.failed_sources.push("failed-a".into());
        assert!(validate_blueprint_bundle_contract(&broken).is_err());

        let mut broken = valid.clone();
        broken.partial = true;
        assert!(validate_blueprint_bundle_contract(&broken).is_err());

        let mut broken = valid.clone();
        broken.bundle_totals.row_count += 1;
        assert!(validate_blueprint_bundle_contract(&broken).is_err());

        let mut broken = valid.clone();
        broken.sources.get_mut("source-a").unwrap().row_count += 1;
        broken.bundle_totals.row_count += 1;
        assert!(validate_blueprint_bundle_contract(&broken).is_err());
    }

    fn summary_source(relationship: &str, group: &str, rows: u64) -> BundleSource {
        BundleSource {
            kind: "database".into(),
            engine: "postgresql".into(),
            dataset_relationship: relationship.into(),
            dataset_group: group.into(),
            dataset_scope_completeness: "complete".into(),
            table_count: 1,
            row_count: rows,
            table_bytes: rows.saturating_mul(10),
            index_bytes: rows.saturating_mul(2),
            ..Default::default()
        }
    }

    #[test]
    fn unknown_bundle_relationship_suppresses_every_aggregate_total() {
        let mut bundle = BlueprintBundle {
            schema_version: BUNDLE_SCHEMA_VERSION,
            kind: BUNDLE_KIND.into(),
            sources: BTreeMap::from([(
                "source-a".into(),
                summary_source("unknown", "dataset-a", 7),
            )]),
            dataset_groups: BTreeMap::from([(
                "dataset-a".into(),
                crate::BundleDatasetGroup {
                    relationship: "unknown".into(),
                    members_complete: false,
                    members: vec!["source-a".into()],
                },
            )]),
            ..Default::default()
        };
        recompute_bundle_totals(&mut bundle).unwrap();
        assert_eq!(bundle.bundle_totals.aggregation, "suppressed");
        assert_eq!(bundle.bundle_totals.source_count, 1);
        assert_eq!(bundle.bundle_totals.logical_dataset_count, 0);
        assert_eq!(bundle.bundle_totals.row_count, 0);
        assert_eq!(
            bundle.bundle_totals.limitations,
            vec!["unknown-dataset-relationship"]
        );
        validate_blueprint_bundle_contract(&bundle).unwrap();
    }

    #[test]
    fn matching_replicas_contribute_one_deterministic_copy() {
        let mut bundle = BlueprintBundle {
            schema_version: BUNDLE_SCHEMA_VERSION,
            kind: BUNDLE_KIND.into(),
            sources: BTreeMap::from([
                (
                    "replica-a".into(),
                    summary_source("replica", "dataset-a", 7),
                ),
                (
                    "replica-b".into(),
                    summary_source("replica", "dataset-a", 7),
                ),
            ]),
            dataset_groups: BTreeMap::from([(
                "dataset-a".into(),
                crate::BundleDatasetGroup {
                    relationship: "replica".into(),
                    members_complete: true,
                    members: vec!["replica-a".into(), "replica-b".into()],
                },
            )]),
            ..Default::default()
        };
        recompute_bundle_totals(&mut bundle).unwrap();
        assert_eq!(bundle.bundle_totals.aggregation, "complete");
        assert_eq!(bundle.bundle_totals.source_count, 2);
        assert_eq!(bundle.bundle_totals.logical_dataset_count, 1);
        assert_eq!(bundle.bundle_totals.row_count, 7);
        assert!(bundle.bundle_totals.limitations.is_empty());
        validate_blueprint_bundle_contract(&bundle).unwrap();
    }

    #[test]
    fn disagreeing_replicas_are_never_averaged_or_double_counted() {
        let mut bundle = BlueprintBundle {
            schema_version: BUNDLE_SCHEMA_VERSION,
            kind: BUNDLE_KIND.into(),
            sources: BTreeMap::from([
                (
                    "replica-a".into(),
                    summary_source("replica", "dataset-a", 7),
                ),
                (
                    "replica-b".into(),
                    summary_source("replica", "dataset-a", 9),
                ),
            ]),
            dataset_groups: BTreeMap::from([(
                "dataset-a".into(),
                crate::BundleDatasetGroup {
                    relationship: "replica".into(),
                    members_complete: false,
                    members: vec!["replica-a".into(), "replica-b".into()],
                },
            )]),
            ..Default::default()
        };
        recompute_bundle_totals(&mut bundle).unwrap();
        assert_eq!(bundle.bundle_totals.aggregation, "incomplete");
        assert_eq!(bundle.bundle_totals.row_count, 7);
        assert_eq!(
            bundle.bundle_totals.limitations,
            vec!["replica-group-disagreement"]
        );
    }

    #[test]
    fn complete_shards_sum_but_incomplete_shards_contribute_nothing() {
        let sources = BTreeMap::from([
            ("shard-a".into(), summary_source("shard", "dataset-a", 7)),
            ("shard-b".into(), summary_source("shard", "dataset-a", 9)),
        ]);
        let mut bundle = BlueprintBundle {
            schema_version: BUNDLE_SCHEMA_VERSION,
            kind: BUNDLE_KIND.into(),
            sources: sources.clone(),
            dataset_groups: BTreeMap::from([(
                "dataset-a".into(),
                crate::BundleDatasetGroup {
                    relationship: "shard".into(),
                    members_complete: true,
                    members: vec!["shard-a".into(), "shard-b".into()],
                },
            )]),
            ..Default::default()
        };
        recompute_bundle_totals(&mut bundle).unwrap();
        assert_eq!(bundle.bundle_totals.aggregation, "complete");
        assert_eq!(bundle.bundle_totals.logical_dataset_count, 1);
        assert_eq!(bundle.bundle_totals.row_count, 16);

        bundle.sources = sources;
        bundle
            .dataset_groups
            .get_mut("dataset-a")
            .unwrap()
            .members_complete = false;
        recompute_bundle_totals(&mut bundle).unwrap();
        assert_eq!(bundle.bundle_totals.aggregation, "incomplete");
        assert_eq!(bundle.bundle_totals.logical_dataset_count, 0);
        assert_eq!(bundle.bundle_totals.row_count, 0);
        assert_eq!(
            bundle.bundle_totals.limitations,
            vec!["shard-group-incomplete"]
        );
    }

    #[test]
    fn failed_shard_member_prevents_partial_shard_arithmetic() {
        let mut bundle = BlueprintBundle {
            schema_version: BUNDLE_SCHEMA_VERSION,
            kind: BUNDLE_KIND.into(),
            partial: true,
            failed_source_count: 1,
            failed_sources: vec!["shard-b".into()],
            sources: BTreeMap::from([("shard-a".into(), summary_source("shard", "dataset-a", 7))]),
            dataset_groups: BTreeMap::from([(
                "dataset-a".into(),
                crate::BundleDatasetGroup {
                    relationship: "shard".into(),
                    members_complete: true,
                    members: vec!["shard-a".into(), "shard-b".into()],
                },
            )]),
            ..Default::default()
        };
        recompute_bundle_totals(&mut bundle).unwrap();
        assert_eq!(bundle.bundle_totals.aggregation, "incomplete");
        assert_eq!(bundle.bundle_totals.row_count, 0);
        assert_eq!(
            bundle.bundle_totals.limitations,
            vec!["failed-sources", "shard-group-incomplete"]
        );
        validate_blueprint_bundle_contract(&bundle).unwrap();
    }

    #[test]
    fn bundle_total_recomputation_fails_closed_on_overflow() {
        let mut bundle = BlueprintBundle {
            schema_version: BUNDLE_SCHEMA_VERSION,
            kind: BUNDLE_KIND.into(),
            sources: BTreeMap::from([
                (
                    "source-a".into(),
                    BundleSource {
                        row_count: u64::MAX,
                        dataset_relationship: "shard".into(),
                        dataset_group: "dataset-a".into(),
                        dataset_scope_completeness: "complete".into(),
                        ..Default::default()
                    },
                ),
                (
                    "source-b".into(),
                    BundleSource {
                        row_count: 1,
                        dataset_relationship: "shard".into(),
                        dataset_group: "dataset-a".into(),
                        dataset_scope_completeness: "complete".into(),
                        ..Default::default()
                    },
                ),
            ]),
            dataset_groups: BTreeMap::from([(
                "dataset-a".into(),
                crate::BundleDatasetGroup {
                    relationship: "shard".into(),
                    members_complete: true,
                    members: vec!["source-a".into(), "source-b".into()],
                },
            )]),
            ..Default::default()
        };

        let error =
            recompute_bundle_totals(&mut bundle).expect_err("bundle row_count must not saturate");
        assert!(error.to_string().contains("row_count overflows u64"));
    }

    #[test]
    fn checked_bundle_paths_reject_absolute_parent_and_symlink_escapes() {
        let root = test_dir("bundle-paths");
        let child_dir = root.join("blueprints");
        fs::create_dir_all(&child_dir).unwrap();
        let child = child_dir.join("one.blueprint.toml");
        fs::write(&child, "invalid Blueprint fixture").unwrap();
        assert_eq!(
            resolve_bundle_path_checked(&root, "blueprints/one.blueprint.toml").unwrap(),
            fs::canonicalize(&child).unwrap()
        );
        assert!(resolve_bundle_path_checked(&root, child.to_str().unwrap()).is_err());
        assert!(resolve_bundle_path_checked(&root, "../outside.blueprint.toml").is_err());
        assert!(
            resolve_bundle_path_checked(&root, "blueprints/../blueprints/one.blueprint.toml")
                .is_err()
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let outside = root.with_file_name(format!(
                "{}-outside",
                root.file_name().unwrap().to_string_lossy()
            ));
            fs::create_dir_all(&outside).unwrap();
            let secret = outside.join("secret.blueprint.toml");
            fs::write(&secret, "secret").unwrap();
            symlink(&outside, root.join("escape")).unwrap();
            assert!(resolve_bundle_path_checked(&root, "escape/secret.blueprint.toml").is_err());
            fs::remove_dir_all(outside).unwrap();
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn embedding_blueprints_rejects_stale_source_summaries() {
        let root = test_dir("bundle-embed");
        let blueprint = one_table_blueprint();
        let blueprint_path = root.join("one.blueprint.toml");
        fs::write(&blueprint_path, blueprint_to_toml(&blueprint).unwrap()).unwrap();

        let mut bundle = embedded_bundle();
        let source = bundle.sources.get_mut("source-a").unwrap();
        source.blueprint = None;
        source.blueprint_path = Some("one.blueprint.toml".into());
        assert!(blueprint_bundle_with_embedded_blueprints(
            bundle.clone(),
            root.join("bundle.toml")
        )
        .is_ok());

        bundle.sources.get_mut("source-a").unwrap().row_count = 99;
        bundle.bundle_totals.row_count = 99;
        assert!(
            blueprint_bundle_with_embedded_blueprints(bundle, root.join("bundle.toml")).is_err()
        );
        fs::remove_dir_all(root).unwrap();
    }
}
