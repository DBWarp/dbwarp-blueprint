//! Blueprint and bundle input/output contract.
//!
//! Parsing accepts only the documented compatibility range, validates the
//! result, and never emits superseded identifiers. Serialization is
//! deterministic for an equivalent normalized model and is shared by live,
//! structured-file, batch, and bundle workflows.

use crate::{
    recompute_bundle_totals, BlueprintBundle, BlueprintFile, BlueprintSelector, BundleSource,
    BundleTotals, Totals, BUNDLE_KIND, BUNDLE_SCHEMA_VERSION, LEGACY_ARTIFACT_CONTRACT,
    LEGACY_BUNDLE_KIND, LEGACY_BUNDLE_SCHEMA_VERSION, LEGACY_IDENTIFIER_SCHEMA_VERSION,
    LEGACY_SAMPLE_ENCODING_TAG, MIN_SCHEMA_VERSION, PREVIOUS_BUNDLE_SCHEMA_VERSION,
    PREVIOUS_SAMPLE_ENCODING_TAG, SAMPLE_ENCODING_TAG, SCHEMA_VERSION,
    TRANSFER_SAMPLE_ENCODING_TAG, TRANSFER_SAMPLE_STREAMING_CHUNKED_ENCODING_TAG,
    TRANSFER_SAMPLE_STREAMING_ENCODING_TAG,
};
use anyhow::{bail, Context, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

const STRUCTURED_SAMPLE_ENCODINGS: &[&str] = &[
    "parquet-column-chunks",
    "parquet-file",
    "avro-container",
    "avro-schema",
    "mixed-structured-file-provenance",
];

const LANGUAGE_CENSUS_DIALECTS: &[&str] = &[
    "sql",
    "plpgsql",
    "plpython",
    "plperl",
    "mysql-sql-psm",
    "plsql",
    "java",
    "tsql",
    "clr",
    "c",
    "internal",
    "unknown",
];

const LEXICAL_SUPPORTED_DIALECTS: &[&str] = &["sql", "plpgsql", "mysql-sql-psm", "plsql", "tsql"];

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
        if blueprint.schema_version >= 7 {
            validate_numeric_identifier(table_id, "table", 3, true)?;
            validate_artifact_schema_id(&table.schema)
                .with_context(|| format!("table '{table_id}' has a non-anonymous schema id"))?;
        }
        let mut ordinals = BTreeSet::new();
        for (column_id, column) in &table.cols {
            if blueprint.schema_version >= 7 {
                validate_numeric_identifier(column_id, "col", 1, false)?;
            }
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
            validate_numeric_semantics(
                blueprint.schema_version,
                &blueprint.engine,
                table_id,
                column_id,
                column,
            )?;
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
            let exact_table_population = table.statistics.as_ref().is_some_and(|statistics| {
                matches!(
                    statistics.row_count_quality.as_str(),
                    "exact-read" | "exact-counter"
                )
            });
            validate_cardinality(
                blueprint.schema_version,
                table_id,
                column_id,
                table.rows,
                exact_table_population,
                column.null_fraction,
                column.cardinality.as_ref(),
            )?;
        }

        validate_table_semantics(blueprint, table_id, table, &ordinals)?;
        validate_table_statistics(blueprint.schema_version, table_id, table)?;

        for (index_id, index) in &table.idxs {
            if blueprint.schema_version >= 7 {
                validate_numeric_identifier(index_id, "idx", 1, false)?;
            }
            let has_v7_index_fields = !index.partitioning.is_empty()
                || !index.visibility.is_empty()
                || !index.state.is_empty();
            if blueprint.schema_version < 7 && has_v7_index_fields {
                bail!("table '{table_id}' index '{index_id}' uses schema-v7 index evidence in an older Blueprint");
            }
            if blueprint.schema_version >= 7 {
                if !index.partitioning.is_empty() {
                    validate_token(
                        "index partitioning",
                        &index.partitioning,
                        &["none", "local", "global", "unknown"],
                    )?;
                }
                if !index.visibility.is_empty() {
                    validate_token(
                        "index visibility",
                        &index.visibility,
                        &["visible", "invisible", "unknown"],
                    )?;
                }
                if !index.state.is_empty() {
                    validate_token(
                        "index state",
                        &index.state,
                        &["usable", "unusable", "in-progress", "failed", "unknown"],
                    )?;
                }
            }
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
    if blueprint.schema_version >= 7 {
        validate_table_parent_cycles(blueprint)?;
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
    if blueprint.schema_version >= 7 {
        validate_v7_anonymous_identifier_sets(blueprint)?;
    }
    validate_topology_and_dataset_scope(blueprint)?;
    Ok(())
}

fn validate_table_parent_cycles(blueprint: &BlueprintFile) -> Result<()> {
    for table_id in blueprint.tables.keys() {
        let mut seen = BTreeSet::new();
        let mut current = table_id.as_str();
        while seen.insert(current) {
            let parent = blueprint.tables[current].parent_table.as_str();
            if parent.is_empty() {
                break;
            }
            current = parent;
        }
        if !blueprint.tables[current].parent_table.is_empty() {
            bail!("table parent hierarchy contains a cycle at '{current}'");
        }
    }
    Ok(())
}

fn validate_column_semantics(
    schema_version: u32,
    table_id: &str,
    column_id: &str,
    column: &crate::BlueprintColumn,
) -> Result<()> {
    let has_v7_fields = column.default_on_null.is_some()
        || column.invisible.is_some()
        || !column.length_semantics.is_empty()
        || column.lob_storage.is_some();
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
        if has_v6_fields || has_v7_fields {
            bail!(
                "table '{table_id}' column '{column_id}' uses column-semantics fields before Blueprint schema_version 6"
            );
        }
        return Ok(());
    }
    if schema_version < 7 && has_v7_fields {
        bail!("table '{table_id}' column '{column_id}' uses schema-v7 column evidence in an older Blueprint");
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
    if column.default_on_null == Some(true) && column.has_default != Some(true) {
        bail!(
            "table '{table_id}' column '{column_id}' has default_on_null without has_default = true"
        );
    }
    if !column.length_semantics.is_empty() {
        validate_token(
            "column length_semantics",
            &column.length_semantics,
            &["characters", "bytes", "not-applicable", "unknown"],
        )?;
    }
    if let Some(storage) = column.lob_storage.as_ref() {
        validate_lob_storage(table_id, column_id, storage)?;
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
        if column.member_count.is_none_or(|count| count == 0) {
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

fn validate_lob_storage(
    table_id: &str,
    column_id: &str,
    storage: &crate::LobStorageEvidence,
) -> Result<()> {
    let location = format!("table '{table_id}' column '{column_id}' LOB storage");
    validate_token(
        "LOB storage_class",
        &storage.storage_class,
        &["basicfile", "securefile", "external", "unknown"],
    )?;
    validate_token(
        "LOB compression",
        &storage.compression,
        &["none", "low", "medium", "high", "not-applicable", "unknown"],
    )?;
    validate_token(
        "LOB deduplication",
        &storage.deduplication,
        &["enabled", "disabled", "not-applicable", "unknown"],
    )?;
    validate_token(
        "LOB storage visibility",
        &storage.visibility,
        &["full", "partial", "unknown"],
    )?;
    if storage.storage_class == "external" {
        if storage.in_row.is_some()
            || storage.encrypted.is_some()
            || storage.compression != "not-applicable"
            || storage.deduplication != "not-applicable"
        {
            bail!("{location} declares in-database storage properties for external content");
        }
    } else if storage.compression == "not-applicable" || storage.deduplication == "not-applicable" {
        bail!("{location} marks in-database storage properties as not applicable");
    }
    if storage.visibility == "full"
        && (storage.storage_class == "unknown"
            || storage.compression == "unknown"
            || storage.deduplication == "unknown")
    {
        bail!("{location} claims full visibility with unknown properties");
    }
    Ok(())
}

fn validate_numeric_semantics(
    schema_version: u32,
    engine: &str,
    table_id: &str,
    column_id: &str,
    column: &crate::BlueprintColumn,
) -> Result<()> {
    let location = format!("table '{table_id}' column '{column_id}'");
    if schema_version < 7 {
        if !column.numeric_model.is_empty() || !column.numeric_precision_radix.is_empty() {
            bail!("{location} uses schema-v7 numeric semantics in an older Blueprint");
        }
        if column.numeric_precision == Some(0) && column.numeric_scale != Some(0) {
            bail!("{location} has a scale without a declared numeric precision");
        }
        if let (Some(precision), Some(scale)) = (column.numeric_precision, column.numeric_scale) {
            if scale < 0 || u64::try_from(scale).is_ok_and(|scale| scale > precision) {
                bail!("{location} has a legacy scale outside its declared precision");
            }
        }
        return Ok(());
    }

    validate_token(
        "column numeric_model",
        &column.numeric_model,
        &[
            "integer",
            "fixed-decimal",
            "unconstrained-decimal",
            "decimal-float",
            "binary-float",
            "not-applicable",
            "unknown",
        ],
    )?;
    if column.numeric_precision == Some(0) {
        bail!("{location} has a zero numeric_precision; omit an unknown declaration");
    }
    if !column.numeric_precision_radix.is_empty() {
        validate_token(
            "column numeric_precision_radix",
            &column.numeric_precision_radix,
            &["decimal", "binary"],
        )?;
    }
    if column
        .numeric_precision
        .is_some_and(|precision| precision > 1_000)
    {
        bail!("{location} has numeric_precision above the supported 1000-digit bound");
    }
    if let Some(precision) = column.numeric_precision {
        let engine_limit = match (engine, column.numeric_model.as_str()) {
            ("oracle", "fixed-decimal") => Some(38),
            ("oracle", "decimal-float") => Some(126),
            ("mysql", "fixed-decimal") => Some(65),
            ("sqlserver", "fixed-decimal") => Some(38),
            ("sqlserver", "binary-float") => Some(53),
            ("postgresql", "fixed-decimal") => Some(1_000),
            _ => None,
        };
        if engine_limit.is_some_and(|limit| precision > limit) {
            bail!(
                "{location} has numeric_precision above the {engine} {} limit",
                column.numeric_model
            );
        }
    }
    if let Some(scale) = column.numeric_scale {
        let in_engine_range = match engine {
            "oracle" => (-84..=127).contains(&scale),
            "postgresql" => (-1_000..=1_000).contains(&scale),
            "mysql" | "sqlserver" | "parquet" | "avro" => {
                scale >= 0
                    && column
                        .numeric_precision
                        .is_some_and(|precision| scale as u64 <= precision)
            }
            _ => (-1_000..=1_000).contains(&scale),
        };
        if !in_engine_range {
            bail!("{location} has numeric_scale outside the {engine} declaration range");
        }
    }

    match column.numeric_model.as_str() {
        "unknown" | "not-applicable" => {
            if column.numeric_precision.is_some()
                || column.numeric_scale.is_some()
                || !column.numeric_precision_radix.is_empty()
            {
                bail!(
                    "{location} declares numeric details with numeric_model = '{}'",
                    column.numeric_model
                );
            }
        }
        "unconstrained-decimal" => {
            if column.numeric_precision.is_some()
                || column.numeric_scale.is_some()
                || column.numeric_precision_radix != "decimal"
            {
                bail!("{location} has inconsistent unconstrained-decimal semantics");
            }
        }
        "fixed-decimal" => {
            if column.numeric_precision.is_none()
                || column.numeric_scale.is_none()
                || column.numeric_precision_radix != "decimal"
            {
                bail!("{location} has incomplete fixed-decimal semantics");
            }
        }
        "decimal-float" => {
            if column.numeric_precision.is_none()
                || column.numeric_scale.is_some()
                || column.numeric_precision_radix != "binary"
            {
                bail!("{location} has inconsistent decimal-float semantics");
            }
            if column
                .numeric_precision
                .is_some_and(|precision| precision > 126)
            {
                bail!("{location} has decimal-float precision above Oracle FLOAT(126)");
            }
        }
        "binary-float" => {
            if column.numeric_scale.is_some() || column.numeric_precision_radix != "binary" {
                bail!("{location} has inconsistent binary-float semantics");
            }
        }
        "integer" => {
            if column.numeric_scale.is_some_and(|scale| scale != 0) {
                bail!("{location} has a non-zero scale for numeric_model = 'integer'");
            }
            if !column.numeric_precision_radix.is_empty()
                && !matches!(
                    column.numeric_precision_radix.as_str(),
                    "decimal" | "binary"
                )
            {
                bail!("{location} has an invalid integer precision radix");
            }
        }
        _ => unreachable!("closed numeric_model vocabulary was validated above"),
    }
    Ok(())
}

fn validate_table_semantics(
    blueprint: &BlueprintFile,
    table_id: &str,
    table: &crate::BlueprintTable,
    ordinals: &BTreeSet<u32>,
) -> Result<()> {
    let has_v7_fields = !table.object_kind.is_empty()
        || !table.storage_organization.is_empty()
        || !table.partitioning.is_empty()
        || !table.segment_state.is_empty()
        || !table.parent_table.is_empty()
        || !table.child_tables.is_empty()
        || !table.table_features.is_empty()
        || table.statistics.is_some();
    let has_v6_fields = !table.kind.is_empty()
        || table.unlogged.is_some()
        || !table.partition_strategy.is_empty()
        || table.partition_count.is_some()
        || !table.partition_key_cols.is_empty()
        || table.partition_rows_max.is_some()
        || !table.temporal_history.is_empty()
        || !table.table_limitations.is_empty()
        || table.counted_in_totals.is_some()
        || table.check_count.is_some();
    if blueprint.schema_version < 6 {
        if has_v6_fields || has_v7_fields {
            bail!(
                "table '{table_id}' uses table-semantics fields before Blueprint schema_version 6"
            );
        }
        return Ok(());
    }

    if blueprint.schema_version < 7 && has_v7_fields {
        bail!("table '{table_id}' uses schema-v7 table evidence in an older Blueprint");
    }

    if blueprint.schema_version >= 7 {
        if !table.kind.is_empty() || !table.partition_strategy.is_empty() {
            bail!("table '{table_id}' emits legacy table kind fields in schema v7");
        }
        validate_token(
            "table object_kind",
            &table.object_kind,
            &[
                "ordinary-table",
                "materialized-view",
                "external-table",
                "temporary-table",
                "nested-table",
                "object-table",
            ],
        )?;
        validate_token(
            "table storage_organization",
            &table.storage_organization,
            &[
                "heap",
                "index-organized",
                "clustered",
                "external",
                "unknown",
            ],
        )?;
        validate_token(
            "table partitioning",
            &table.partitioning,
            &[
                "none",
                "range",
                "list",
                "hash",
                "interval",
                "reference",
                "composite",
                "system",
                "key",
                "linear-hash",
                "linear-key",
                "unknown",
            ],
        )?;
        validate_token(
            "table segment_state",
            &table.segment_state,
            &[
                "created",
                "deferred",
                "mixed",
                "mixed-table-and-index",
                "unavailable",
                "unknown",
            ],
        )?;
        validate_sorted_unique_tokens(
            "table feature",
            &table.table_features,
            &[
                "graph-edge",
                "graph-node",
                "memory-optimized",
                "temporal-current",
                "temporal-history",
            ],
        )?;
        validate_sorted_unique_tokens(
            "table limitation",
            &table.table_limitations,
            &[
                "column-inventory-unavailable",
                "dependent-structure-suppressed",
                "index-inventory-unavailable",
                "relationship-inventory-unavailable",
                "relationship-target-outside-selected-scope",
                "relationship-target-visibility-unknown",
                "row-security-filter-active",
                "row-security-visibility-unknown",
                "table-classification-unavailable",
                "temporal-history-outside-selected-scope",
                "temporal-history-visibility-unknown",
            ],
        )?;
        let partitioned = matches!(
            table.partitioning.as_str(),
            "range"
                | "list"
                | "hash"
                | "interval"
                | "reference"
                | "composite"
                | "system"
                | "key"
                | "linear-hash"
                | "linear-key"
        );
        let has_partition_details = table.partition_count.is_some()
            || !table.partition_key_cols.is_empty()
            || table.partition_rows_max.is_some();
        if partitioned && table.partition_count.is_none() {
            bail!("table '{table_id}' is partitioned but has no explicit partition_count");
        }
        let empty_logical_partition_root =
            table.object_kind == "ordinary-table" && table.segment_state == "unavailable";
        if partitioned && table.partition_count == Some(0) && !empty_logical_partition_root {
            bail!(
                "table '{table_id}' has partition_count = 0 without an unavailable ordinary logical root"
            );
        }
        if table.partitioning == "none" && has_partition_details {
            bail!("table '{table_id}' has partition details with partitioning = 'none'");
        }
        if table.partitioning == "unknown" && has_partition_details {
            bail!("table '{table_id}' has partition details with unknown partitioning");
        }
        if let Some(partition_rows_max) = table.partition_rows_max {
            if partition_rows_max > table.rows {
                bail!("table '{table_id}' partition_rows_max exceeds its serialized row count");
            }
            if table.rows > 0 && partition_rows_max == 0 {
                bail!("table '{table_id}' has a positive row count but an empty largest partition");
            }
        }
        if table.object_kind == "external-table" && table.storage_organization != "external" {
            bail!("table '{table_id}' is external but does not use external storage");
        }
        if table.storage_organization == "external" && table.object_kind != "external-table" {
            bail!("table '{table_id}' uses external storage without external object kind");
        }
        validate_sorted_unique_values("table child_tables", &table.child_tables)?;
        for child in &table.child_tables {
            let child_table = blueprint.tables.get(child).with_context(|| {
                format!("table '{table_id}' references missing child table '{child}'")
            })?;
            if child == table_id || child_table.parent_table != table_id {
                bail!("table '{table_id}' has a non-reciprocal child table '{child}'");
            }
        }
        if !table.parent_table.is_empty() {
            let parent = blueprint.tables.get(&table.parent_table).with_context(|| {
                format!(
                    "table '{table_id}' references missing parent table '{}'",
                    table.parent_table
                )
            })?;
            if table.parent_table == table_id
                || parent
                    .child_tables
                    .binary_search(&table_id.to_string())
                    .is_err()
            {
                bail!("table '{table_id}' has a non-reciprocal parent table");
            }
        }
    }

    if blueprint.schema_version < 7 && !table.kind.is_empty() {
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
    if blueprint.schema_version < 7 && !table.partition_strategy.is_empty() {
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
    if blueprint.schema_version < 7 && table.kind == "partitioned" {
        if table.partition_count.is_none_or(|count| count == 0) {
            bail!("table '{table_id}' is partitioned but has no positive partition_count");
        }
    } else if blueprint.schema_version < 7 && has_partition_fields {
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

    let temporal_current = if blueprint.schema_version >= 7 {
        table
            .table_features
            .binary_search(&"temporal-current".to_string())
            .is_ok()
    } else {
        table.kind == "temporal-current"
    };
    let history_outside_selected_scope = table
        .table_limitations
        .binary_search(&"temporal-history-outside-selected-scope".to_string())
        .is_ok();
    let history_visibility_unknown = table
        .table_limitations
        .binary_search(&"temporal-history-visibility-unknown".to_string())
        .is_ok();
    let relationship_target_outside_selected_scope = table
        .table_limitations
        .binary_search(&"relationship-target-outside-selected-scope".to_string())
        .is_ok();
    let relationship_target_visibility_unknown = table
        .table_limitations
        .binary_search(&"relationship-target-visibility-unknown".to_string())
        .is_ok();
    let table_classification_unavailable = table
        .table_limitations
        .binary_search(&"table-classification-unavailable".to_string())
        .is_ok();
    let column_inventory_unavailable = table
        .table_limitations
        .binary_search(&"column-inventory-unavailable".to_string())
        .is_ok();
    let dependent_structure_suppressed = table
        .table_limitations
        .binary_search(&"dependent-structure-suppressed".to_string())
        .is_ok();
    let index_inventory_unavailable = table
        .table_limitations
        .binary_search(&"index-inventory-unavailable".to_string())
        .is_ok();
    let relationship_inventory_unavailable = table
        .table_limitations
        .binary_search(&"relationship-inventory-unavailable".to_string())
        .is_ok();
    if table_classification_unavailable
        && blueprint
            .structure_scope
            .as_ref()
            .is_none_or(|scope| scope.table_inventory_completeness == "complete")
    {
        bail!(
            "table '{table_id}' uses table-classification-unavailable while table inventory is complete"
        );
    }
    if column_inventory_unavailable
        && blueprint
            .structure_scope
            .as_ref()
            .is_none_or(|scope| scope.column_inventory_completeness == "complete")
    {
        bail!(
            "table '{table_id}' uses column-inventory-unavailable while column inventory is complete"
        );
    }
    if dependent_structure_suppressed
        && blueprint.structure_scope.as_ref().is_none_or(|scope| {
            scope.index_inventory_completeness == "complete"
                || scope.relationship_inventory_completeness == "complete"
        })
    {
        bail!(
            "table '{table_id}' uses dependent-structure-suppressed while dependent inventories are complete"
        );
    }
    if index_inventory_unavailable
        && blueprint
            .structure_scope
            .as_ref()
            .is_none_or(|scope| scope.index_inventory_completeness == "complete")
    {
        bail!(
            "table '{table_id}' uses index-inventory-unavailable while index inventory is complete"
        );
    }
    if relationship_inventory_unavailable
        && blueprint
            .structure_scope
            .as_ref()
            .is_none_or(|scope| scope.relationship_inventory_completeness == "complete")
    {
        bail!(
            "table '{table_id}' uses relationship-inventory-unavailable while relationship inventory is complete"
        );
    }
    if relationship_target_outside_selected_scope {
        let selection_limited = blueprint.dataset_scope.as_ref().is_some_and(|scope| {
            scope
                .limitations
                .binary_search(&"selection-limited".to_string())
                .is_ok()
        });
        if !selection_limited {
            bail!(
                "table '{table_id}' uses relationship-target-outside-selected-scope without a selected capture"
            );
        }
    }
    if relationship_target_visibility_unknown
        && blueprint
            .structure_scope
            .as_ref()
            .is_none_or(|scope| scope.relationship_inventory_completeness == "complete")
    {
        bail!(
            "table '{table_id}' uses relationship-target-visibility-unknown while relationship inventory is complete"
        );
    }
    if history_outside_selected_scope {
        let selection_limited = blueprint.dataset_scope.as_ref().is_some_and(|scope| {
            scope
                .limitations
                .binary_search(&"selection-limited".to_string())
                .is_ok()
        });
        if !temporal_current || !selection_limited || !table.temporal_history.is_empty() {
            bail!(
                "table '{table_id}' uses temporal-history-outside-selected-scope without an unlinked temporal-current table in a selected capture"
            );
        }
    }
    if history_visibility_unknown && (!temporal_current || !table.temporal_history.is_empty()) {
        bail!(
            "table '{table_id}' uses temporal-history-visibility-unknown without an unlinked temporal-current table"
        );
    }
    if history_outside_selected_scope && history_visibility_unknown {
        bail!(
            "table '{table_id}' cannot classify the temporal history table as both outside scope and visibility unknown"
        );
    }
    if temporal_current {
        if table.temporal_history.is_empty() {
            if !history_outside_selected_scope && !history_visibility_unknown {
                bail!(
                    "table '{table_id}' is temporal-current but has no temporal_history reference"
                );
            }
        } else {
            let history = blueprint
                .tables
                .get(&table.temporal_history)
                .with_context(|| {
                    format!(
                        "table '{table_id}' references missing temporal history table '{}'",
                        table.temporal_history
                    )
                })?;
            let history_is_temporal = if blueprint.schema_version >= 7 {
                history
                    .table_features
                    .binary_search(&"temporal-history".to_string())
                    .is_ok()
            } else {
                history.kind == "temporal-history"
            };
            if table.temporal_history == table_id || !history_is_temporal {
                bail!(
                    "table '{table_id}' temporal_history must reference a different temporal-history table"
                );
            }
        }
    } else if !table.temporal_history.is_empty() {
        bail!("table '{table_id}' has temporal_history without kind = 'temporal-current'");
    }

    let excluded_unmeasured_or_derived = if blueprint.schema_version >= 7 {
        matches!(
            table.object_kind.as_str(),
            "external-table" | "materialized-view" | "temporary-table"
        ) || table
            .table_features
            .binary_search(&"memory-optimized".to_string())
            .is_ok()
    } else {
        table.kind == "external"
    };
    match (excluded_unmeasured_or_derived, table.counted_in_totals) {
        (true, Some(false)) => {}
        (true, _) => {
            bail!(
                "table '{table_id}' is external, derived, temporary, or unmeasured and must set counted_in_totals = false"
            )
        }
        (false, None) => {}
        (false, Some(true)) => bail!(
            "table '{table_id}' uses non-canonical counted_in_totals = true; omit the field instead"
        ),
        (false, Some(false)) => {
            bail!("table '{table_id}' sets counted_in_totals = false without an external, derived, temporary, or unmeasured classification")
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
        if blueprint.database_topology.is_some()
            || blueprint.dataset_scope.is_some()
            || blueprint.structure_scope.is_some()
            || blueprint.source_environment.is_some()
            || blueprint.statistics_evidence.is_some()
            || blueprint.activity_snapshot.is_some()
        {
            bail!(
                "topology, scope, environment, statistics, and activity evidence require a newer Blueprint schema"
            );
        }
        return Ok(());
    }

    let scope = blueprint
        .dataset_scope
        .as_ref()
        .context("schema-v6 Blueprint is missing dataset_scope")?;
    validate_dataset_scope(blueprint, scope)?;

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
        validate_database_topology(blueprint.schema_version, topology)?;
        if scope.layout == "structured-dataset" {
            bail!("database Blueprint must not use dataset_scope layout 'structured-dataset'");
        }
    }

    if blueprint.schema_version < 7 {
        if blueprint.structure_scope.is_some()
            || blueprint.source_environment.is_some()
            || blueprint.statistics_evidence.is_some()
            || blueprint.activity_snapshot.is_some()
        {
            bail!("schema-v7 evidence appears in an older Blueprint");
        }
        return Ok(());
    }

    let structure = blueprint
        .structure_scope
        .as_ref()
        .context("schema-v7 Blueprint is missing structure_scope")?;
    validate_structure_scope(blueprint, structure)?;
    let statistics = blueprint
        .statistics_evidence
        .as_ref()
        .context("schema-v7 Blueprint is missing statistics_evidence")?;
    validate_statistics_evidence(blueprint, statistics)?;
    blueprint
        .artifact_inventory
        .as_ref()
        .context("schema-v7 Blueprint is missing artifact_inventory")?;
    if structure.table_inventory_completeness != "complete"
        && scope.table_inventory_completeness == "complete"
    {
        bail!("dataset_scope cannot claim complete table inventory when structure_scope does not");
    }
    if structured {
        if blueprint.source_environment.is_some() || blueprint.activity_snapshot.is_some() {
            bail!("structured-file Blueprint must not contain environment or activity evidence");
        }
        if structure.visibility != "full"
            || [
                structure.table_inventory_completeness.as_str(),
                structure.column_inventory_completeness.as_str(),
                structure.index_inventory_completeness.as_str(),
                structure.relationship_inventory_completeness.as_str(),
            ]
            .iter()
            .any(|value| *value != "complete")
            || !structure.catalogs_unreadable.is_empty()
        {
            bail!("structured-file Blueprint requires complete full structure evidence");
        }
    } else {
        validate_source_environment(
            blueprint
                .source_environment
                .as_ref()
                .context("schema-v7 database Blueprint is missing source_environment")?,
        )?;
        if let Some(activity) = blueprint.activity_snapshot.as_ref() {
            validate_activity_snapshot(activity)?;
        }
    }
    Ok(())
}

fn validate_database_topology(
    schema_version: u32,
    topology: &crate::DatabaseTopology,
) -> Result<()> {
    let expected_contract = if schema_version >= 7 {
        crate::TOPOLOGY_CONTRACT
    } else {
        crate::PREVIOUS_TOPOLOGY_CONTRACT
    };
    if topology.contract != expected_contract {
        bail!(
            "unsupported database topology contract '{}'; expected '{}'",
            topology.contract,
            expected_contract
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
            "physical-standby",
            "logical-standby",
            "snapshot-standby",
            "unknown",
        ],
    )?;
    validate_token(
        "database topology visibility",
        &topology.visibility,
        &["full", "partial", "unknown"],
    )?;
    if schema_version >= 7 {
        validate_token(
            "database topology member_count_scope",
            &topology.member_count_scope,
            &[
                "deployment",
                "visible-subset",
                "connected-member",
                "unknown",
            ],
        )?;
    } else if !topology.member_count_scope.is_empty()
        || !topology.catalogs_not_applicable.is_empty()
    {
        bail!("schema-v6 database topology contains schema-v7 evidence");
    }
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
        "physical-standby",
        "logical-standby",
        "snapshot-standby",
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
        if schema_version >= 7 && topology.member_count_scope != "deployment" {
            bail!("full database topology visibility requires deployment-scoped member_count");
        }
    }
    if schema_version >= 7 {
        if topology.member_count == 0 && topology.member_count_scope != "unknown" {
            bail!("unknown member_count requires unknown member_count_scope");
        }
        if topology.member_count_scope == "connected-member" && topology.member_count != 1 {
            bail!("connected-member scope requires member_count = 1");
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
            "oracle-cdb",
            "oracle-data-guard",
            "oracle-non-cdb",
            "oracle-pdb",
            "oracle-rac",
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
        "oracle-cdb",
        "oracle-data-guard",
        "oracle-instance",
        "oracle-pdb",
        "oracle-rac",
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
    if schema_version >= 7 {
        validate_sorted_unique_tokens(
            "database topology inapplicable catalog",
            &topology.catalogs_not_applicable,
            CATALOGS,
        )?;
    }
    if topology.catalogs_read.iter().any(|catalog| {
        topology.catalogs_unreadable.binary_search(catalog).is_ok()
            || topology
                .catalogs_not_applicable
                .binary_search(catalog)
                .is_ok()
    }) || topology.catalogs_unreadable.iter().any(|catalog| {
        topology
            .catalogs_not_applicable
            .binary_search(catalog)
            .is_ok()
    }) {
        bail!("database topology catalog evidence sets must be disjoint");
    }
    Ok(())
}

fn validate_source_environment(environment: &crate::SourceEnvironment) -> Result<()> {
    if environment.contract != crate::SOURCE_ENVIRONMENT_CONTRACT {
        bail!(
            "unsupported source environment contract '{}'; expected '{}'",
            environment.contract,
            crate::SOURCE_ENVIRONMENT_CONTRACT
        );
    }
    validate_token(
        "source environment evidence_origin",
        &environment.evidence_origin,
        &[
            "database-endpoint",
            "provider-api",
            "orchestrator-api",
            "operator-attested",
            "mixed",
            "none",
        ],
    )?;
    validate_token(
        "source environment hosting_model",
        &environment.hosting_model,
        &["managed-service", "self-managed", "orchestrated", "unknown"],
    )?;
    validate_token(
        "source environment infrastructure_location",
        &environment.infrastructure_location,
        &["cloud", "on-premises", "hybrid", "unknown"],
    )?;
    validate_token(
        "source environment capacity_scope",
        &environment.capacity_scope,
        &[
            "connected-instance",
            "database-resource",
            "cluster-aggregate",
            "member-subset",
            "unknown",
        ],
    )?;
    validate_token(
        "source environment capacity_visibility",
        &environment.capacity_visibility,
        &["full", "partial", "unknown", "not-requested"],
    )?;
    validate_token(
        "source environment CPU band",
        &environment.cpu_capacity_band,
        &[
            "1", "2", "3-4", "5-8", "9-16", "17-32", "33-64", "65-128", "129-plus", "unknown",
        ],
    )?;
    validate_token(
        "source environment CPU basis",
        &environment.cpu_capacity_basis,
        &[
            "logical-cpu-limit",
            "database-resource-limit",
            "operating-system-visible",
            "physical-host",
            "unknown",
        ],
    )?;
    validate_token(
        "source environment memory band",
        &environment.memory_capacity_band,
        &[
            "under-2-gib",
            "2-4-gib",
            "4-8-gib",
            "8-16-gib",
            "16-32-gib",
            "32-64-gib",
            "64-128-gib",
            "128-256-gib",
            "256-512-gib",
            "512-gib-plus",
            "unknown",
        ],
    )?;
    validate_token(
        "source environment memory basis",
        &environment.memory_capacity_basis,
        &[
            "database-buffer-cache",
            "database-resource-limit",
            "operating-system-visible",
            "physical-host",
            "unknown",
        ],
    )?;
    if !environment.collector_machine_excluded {
        bail!("source environment must assert collector_machine_excluded = true");
    }
    validate_sorted_unique_tokens(
        "source environment feature",
        &environment.features,
        &[
            "autoscaling",
            "burstable",
            "container-limits-visible",
            "database-resource-governed",
            "serverless",
            "shared-host",
        ],
    )?;
    validate_sorted_unique_tokens(
        "source environment limitation",
        &environment.limitations,
        &[
            "oracle-client-version-below-tested-floor",
            "oracle-client-version-mismatch",
            "oracle-client-version-unreadable",
        ],
    )?;
    validate_environment_catalog_sets(
        "source environment",
        &environment.catalogs_read,
        &environment.catalogs_unreadable,
        &environment.catalogs_not_applicable,
        &[
            "kubernetes-resource-api",
            "mysql-capacity-variables",
            "oracle-capacity-parameters",
            "oracle-resource-manager",
            "pg-capacity-settings",
            "provider-resource-api",
            "sqlserver-engine-edition",
            "sqlserver-os-sys-info",
            "sqlserver-resource-governance",
        ],
    )?;
    let cpu_unknown = environment.cpu_capacity_band == "unknown";
    let memory_unknown = environment.memory_capacity_band == "unknown";
    if cpu_unknown != (environment.cpu_capacity_basis == "unknown")
        || memory_unknown != (environment.memory_capacity_basis == "unknown")
    {
        bail!("source environment capacity bands and evidence bases must agree");
    }
    if environment.capacity_visibility == "full"
        && (cpu_unknown || memory_unknown || !environment.catalogs_unreadable.is_empty())
    {
        bail!("full source environment visibility requires CPU and memory evidence");
    }
    if (!cpu_unknown || !memory_unknown) && environment.capacity_scope == "unknown" {
        bail!("known source capacity requires a known capacity scope");
    }
    if environment.evidence_origin == "none" {
        if environment.capacity_visibility != "not-requested"
            || !cpu_unknown
            || !memory_unknown
            || !environment.features.is_empty()
            || !environment.catalogs_read.is_empty()
            || !environment.catalogs_unreadable.is_empty()
        {
            bail!("source environment without an evidence origin must contain no evidence");
        }
    } else if environment.capacity_visibility == "not-requested" {
        const CAPACITY_CATALOGS: &[&str] = &[
            "kubernetes-resource-api",
            "mysql-capacity-variables",
            "oracle-capacity-parameters",
            "oracle-resource-manager",
            "pg-capacity-settings",
            "provider-resource-api",
            "sqlserver-os-sys-info",
            "sqlserver-resource-governance",
        ];
        let carries_capacity_catalog = environment
            .catalogs_read
            .iter()
            .chain(&environment.catalogs_unreadable)
            .chain(&environment.catalogs_not_applicable)
            .any(|catalog| CAPACITY_CATALOGS.contains(&catalog.as_str()));
        if !cpu_unknown
            || !memory_unknown
            || environment.capacity_scope != "unknown"
            || carries_capacity_catalog
        {
            bail!("not-requested capacity must contain no capacity evidence");
        }
    }
    Ok(())
}

fn validate_table_statistics(
    schema_version: u32,
    table_id: &str,
    table: &crate::BlueprintTable,
) -> Result<()> {
    if schema_version < 7 {
        if table.statistics.is_some() {
            bail!("table '{table_id}' uses statistics evidence before schema v7");
        }
        return Ok(());
    }
    if !table.stats_freshness.is_empty() {
        bail!("table '{table_id}' emits legacy stats_freshness in schema v7");
    }
    let evidence = table
        .statistics
        .as_ref()
        .with_context(|| format!("schema-v7 table '{table_id}' is missing statistics evidence"))?;
    validate_token(
        "table statistics row_count_method",
        &evidence.row_count_method,
        &[
            "postgres-planner-estimate",
            "mysql-table-statistics",
            "sqlserver-partition-counter",
            "oracle-table-statistics",
            "oracle-segment-statistics",
            "parquet-footer",
            "avro-decoded-scan",
            "structured-dataset-aggregate",
            "bounded-complete-read",
            "unknown",
        ],
    )?;
    validate_token(
        "table statistics row_count_quality",
        &evidence.row_count_quality,
        &[
            "exact-counter",
            "exact-read",
            "engine-counter",
            "engine-estimate",
            "cached-engine-estimate",
            "sample-extrapolation",
            "unavailable",
            "unknown",
        ],
    )?;
    if (evidence.row_count_method == "bounded-complete-read")
        != (evidence.row_count_quality == "exact-read")
    {
        bail!("table '{table_id}' must use bounded-complete-read and exact-read together");
    }
    if evidence.row_count_quality == "exact-read" && evidence.sample_fraction_band != "full" {
        bail!("table '{table_id}' exact-read evidence requires a full sample fraction");
    }
    validate_token(
        "table statistics state",
        &evidence.statistics_state,
        &[
            "current",
            "possibly-stale",
            "known-stale",
            "never-analyzed",
            "locked",
            "user-supplied",
            "not-applicable",
            "unknown",
        ],
    )?;
    validate_token(
        "table statistics refresh_age_band",
        &evidence.refresh_age_band,
        &[
            "under-1h",
            "1h-1d",
            "1-7d",
            "1-4w",
            "1-3m",
            "3m-plus",
            "unknown",
            "not-applicable",
        ],
    )?;
    validate_token(
        "table statistics modification_ratio_band",
        &evidence.modification_ratio_band,
        &[
            "none",
            "under-1pct",
            "1-5pct",
            "5-10pct",
            "10-20pct",
            "20-50pct",
            "over-50pct",
            "unknown",
            "not-applicable",
        ],
    )?;
    validate_token(
        "table statistics sample_fraction_band",
        &evidence.sample_fraction_band,
        &[
            "full",
            "75-99pct",
            "50-74pct",
            "25-49pct",
            "under-25pct",
            "unknown",
            "not-applicable",
        ],
    )?;
    validate_token(
        "table statistics scope",
        &evidence.statistics_scope,
        &[
            "global",
            "partition",
            "subpartition",
            "session",
            "local-member",
            "logical-dataset",
            "database-resource",
            "structured-dataset",
            "selected-object",
            "unknown",
        ],
    )?;
    validate_token(
        "table size method",
        &evidence.size_method,
        &[
            "postgres-local-relation-size",
            "mysql-information-schema",
            "sqlserver-partition-pages",
            "citus-distributed-relation-size",
            "distributed-aggregate",
            "oracle-segment-bytes",
            "oracle-table-logical-estimate",
            "parquet-footer",
            "avro-container",
            "structured-dataset-aggregate",
            "unknown",
        ],
    )?;
    validate_token(
        "table size quality",
        &evidence.size_quality,
        &[
            "exact-counter",
            "engine-counter",
            "engine-estimate",
            "cached-engine-estimate",
            "unavailable",
            "unknown",
        ],
    )?;
    validate_token(
        "table size scope",
        &evidence.size_scope,
        &[
            "table-only",
            "table-and-lob",
            "table-lob-and-index",
            "logical-dataset",
            "local-member",
            "selected-object",
            "unknown",
        ],
    )?;
    validate_token(
        "table size accounting",
        &evidence.size_accounting,
        &[
            "allocated-segment",
            "logical-estimate",
            "source-container",
            "unknown",
        ],
    )?;
    validate_token(
        "table size visibility",
        &evidence.size_visibility,
        &["full", "partial", "unavailable", "unknown"],
    )?;
    if evidence.size_method == "oracle-segment-bytes" {
        if evidence.size_quality != "exact-counter"
            || evidence.size_accounting != "allocated-segment"
            || !matches!(evidence.size_visibility.as_str(), "full" | "partial")
            || !matches!(
                table.segment_state.as_str(),
                "created" | "deferred" | "mixed" | "mixed-table-and-index"
            )
        {
            bail!(
                "table '{table_id}' Oracle segment-byte evidence requires attributed exact-counter provenance"
            );
        }
        if table.segment_state == "deferred" && (table.table_bytes > 0 || table.index_bytes > 0) {
            bail!(
                "table '{table_id}' deferred Oracle segment evidence cannot carry attributed bytes"
            );
        }
        if matches!(
            table.segment_state.as_str(),
            "mixed" | "mixed-table-and-index"
        ) && table.index_bytes != 0
        {
            bail!(
                "table '{table_id}' mixed Oracle segment evidence requires a rounded-zero index allocation"
            );
        }
        if table.segment_state == "mixed-table-and-index" && table.table_bytes != 0 {
            bail!(
                "table '{table_id}' mixed Oracle table/index evidence requires rounded-zero table and index allocations"
            );
        }
    }
    if evidence.size_method == "oracle-table-logical-estimate"
        && (evidence.size_quality != "engine-estimate"
            || evidence.size_accounting != "logical-estimate"
            || evidence.size_visibility != "partial"
            || table.segment_state != "unavailable")
    {
        bail!(
            "table '{table_id}' Oracle logical size evidence requires partial estimate provenance"
        );
    }
    if evidence.statistics_state == "not-applicable"
        && (evidence.refresh_age_band != "not-applicable"
            || evidence.modification_ratio_band != "not-applicable")
    {
        bail!("not-applicable statistics require not-applicable age and modification evidence");
    }
    if evidence.statistics_scope == "structured-dataset"
        && (evidence.statistics_state != "not-applicable"
            || evidence.sample_fraction_band != "not-applicable")
    {
        bail!("structured-dataset statistics must mark optimizer evidence not-applicable");
    }
    Ok(())
}

fn validate_statistics_evidence(
    blueprint: &BlueprintFile,
    evidence: &crate::StatisticsEvidence,
) -> Result<()> {
    if evidence.contract != crate::STATISTICS_EVIDENCE_CONTRACT {
        bail!(
            "unsupported statistics evidence contract '{}'; expected '{}'",
            evidence.contract,
            crate::STATISTICS_EVIDENCE_CONTRACT
        );
    }
    validate_token(
        "statistics evidence visibility",
        &evidence.visibility,
        &["full", "partial", "unknown"],
    )?;
    let expected = u64::try_from(blueprint.tables.len())
        .context("Blueprint table count exceeds statistics evidence u64 range")?;
    if evidence.table_count != expected {
        bail!("statistics evidence table_count does not match Blueprint tables");
    }
    let states = validate_count_map(
        "statistics state",
        &evidence.counts_by_statistics_state,
        &[
            "current",
            "possibly-stale",
            "known-stale",
            "never-analyzed",
            "locked",
            "user-supplied",
            "not-applicable",
            "unknown",
        ],
    )?;
    let row_quality = validate_count_map(
        "row-count quality",
        &evidence.counts_by_row_count_quality,
        &[
            "exact-counter",
            "exact-read",
            "engine-counter",
            "engine-estimate",
            "cached-engine-estimate",
            "sample-extrapolation",
            "unavailable",
            "unknown",
        ],
    )?;
    let size_quality = validate_count_map(
        "size quality",
        &evidence.counts_by_size_quality,
        &[
            "exact-counter",
            "engine-counter",
            "engine-estimate",
            "cached-engine-estimate",
            "unavailable",
            "unknown",
        ],
    )?;
    if states != expected || row_quality != expected || size_quality != expected {
        bail!("statistics aggregate maps must each cover every table");
    }
    let mut observed_states = BTreeMap::new();
    let mut observed_rows = BTreeMap::new();
    let mut observed_sizes = BTreeMap::new();
    let mut included_row_evidence_missing = false;
    let mut included_size_evidence_missing = false;
    let mut table_visibility_gap = false;
    let mut table_statistics_classification_gap = false;
    let mut included_table_seen = false;
    for table in blueprint.tables.values() {
        let table_evidence = table
            .statistics
            .as_ref()
            .context("schema-v7 table statistics evidence is missing")?;
        *observed_states
            .entry(table_evidence.statistics_state.clone())
            .or_insert(0_u64) += 1;
        *observed_rows
            .entry(table_evidence.row_count_quality.clone())
            .or_insert(0_u64) += 1;
        *observed_sizes
            .entry(table_evidence.size_quality.clone())
            .or_insert(0_u64) += 1;
        // Deliberate exclusion may make row and size evidence unavailable,
        // but it cannot turn a known malformed or missing statistics
        // classification into complete aggregate provenance.
        table_statistics_classification_gap |= table_evidence.statistics_state == "unknown";
        if table.counted_in_totals != Some(false) {
            included_table_seen = true;
            included_row_evidence_missing |= matches!(
                table_evidence.row_count_quality.as_str(),
                "unavailable" | "unknown"
            );
            included_size_evidence_missing |= matches!(
                table_evidence.size_quality.as_str(),
                "unavailable" | "unknown"
            );
            table_visibility_gap |= table_evidence.size_visibility != "full"
                || matches!(
                    table_evidence.row_count_quality.as_str(),
                    "unavailable" | "unknown"
                )
                || table_evidence.statistics_state == "unknown";
        }
    }
    if observed_states != evidence.counts_by_statistics_state
        || observed_rows != evidence.counts_by_row_count_quality
        || observed_sizes != evidence.counts_by_size_quality
    {
        bail!("statistics aggregate maps do not match table evidence");
    }
    if let Some(scope) = blueprint.dataset_scope.as_ref() {
        if scope.row_count_completeness == "complete" && included_row_evidence_missing {
            bail!("complete row-count coverage cannot include a counted table with unavailable evidence");
        }
        if scope.size_completeness == "complete" && included_size_evidence_missing {
            bail!(
                "complete size coverage cannot include a counted table with unavailable evidence"
            );
        }
    }
    validate_environment_catalog_sets(
        "statistics evidence",
        &evidence.catalogs_read,
        &evidence.catalogs_unreadable,
        &evidence.catalogs_not_applicable,
        &[
            "avro-container",
            "mysql-information-schema-tables",
            "mysql-innodb-table-stats",
            "oracle-tab-statistics",
            "oracle-segments",
            "parquet-footer",
            "pg-stat-all-tables",
            "sqlserver-dm-db-stats-properties",
            "sqlserver-partition-stats",
        ],
    )?;
    if evidence.visibility == "full" && !included_table_seen {
        bail!("full statistics visibility requires a non-empty counted population");
    }
    if evidence.visibility == "full" && table_visibility_gap {
        bail!("full statistics visibility cannot include counted table evidence gaps");
    }
    if evidence.visibility == "full" && table_statistics_classification_gap {
        bail!("full statistics visibility cannot include unclassified table statistics");
    }
    if evidence.visibility == "full" && !evidence.catalogs_unreadable.is_empty() {
        bail!("full statistics visibility cannot include unreadable catalogs");
    }
    validate_sorted_unique_tokens(
        "statistics evidence limitation",
        &evidence.limitations,
        &[
            "cached-statistics-expiry-unknown",
            "catalog-capture-truncated",
            "modification-evidence-unavailable",
            "optimizer-statistics-not-row-counter",
            "refresh-age-unavailable",
            "statistics-partial",
            "statistics-provenance-unclassified",
            "statistics-stale",
            "statistics-visibility-unknown",
        ],
    )?;
    Ok(())
}

fn validate_activity_snapshot(activity: &crate::ActivitySnapshot) -> Result<()> {
    if activity.contract != crate::ACTIVITY_SNAPSHOT_CONTRACT {
        bail!(
            "unsupported activity snapshot contract '{}'; expected '{}'",
            activity.contract,
            crate::ACTIVITY_SNAPSHOT_CONTRACT
        );
    }
    validate_token(
        "activity evidence_origin",
        &activity.evidence_origin,
        &["database-endpoint", "provider-api", "mixed"],
    )?;
    validate_token(
        "activity scope",
        &activity.scope,
        &[
            "connected-instance",
            "database-resource",
            "cluster-aggregate",
            "member-subset",
            "unknown",
        ],
    )?;
    validate_token(
        "activity observation window",
        &activity.observation_window_band,
        &["under-5s", "5-30s", "31-60s", "1-5m", "unknown"],
    )?;
    validate_token(
        "active connections",
        &activity.active_connections_band,
        &[
            "none",
            "1",
            "2-5",
            "6-20",
            "21-100",
            "101-500",
            "501-2000",
            "2001-plus",
            "unknown",
        ],
    )?;
    let rates = &[
        "none",
        "under-1",
        "1-10",
        "11-100",
        "101-1000",
        "1001-10000",
        "10000-plus",
        "unknown",
    ];
    for (field, value) in [
        ("transaction rate", activity.transaction_rate_band.as_str()),
        ("write rate", activity.write_rate_band.as_str()),
        (
            "log generation rate",
            activity.log_generation_rate_band.as_str(),
        ),
    ] {
        validate_token(field, value, rates)?;
    }
    for (field, value) in [
        ("CPU pressure", activity.cpu_pressure_band.as_str()),
        ("cache pressure", activity.cache_pressure_band.as_str()),
    ] {
        validate_token(
            field,
            value,
            &["low", "moderate", "high", "critical", "unknown"],
        )?;
    }
    validate_token(
        "activity reset semantics",
        &activity.reset_semantics,
        &[
            "interval-delta",
            "cumulative-since-engine-start",
            "stats-resettable",
            "unknown",
        ],
    )?;
    validate_environment_catalog_sets(
        "activity snapshot",
        &activity.catalogs_read,
        &activity.catalogs_unreadable,
        &activity.catalogs_not_applicable,
        &[
            "mysql-global-status",
            "oracle-system-event",
            "oracle-sysstat",
            "pg-stat-database",
            "provider-metrics-api",
            "sqlserver-performance-counters",
        ],
    )
}

fn validate_count_map(
    field: &str,
    counts: &BTreeMap<String, u64>,
    allowed: &[&str],
) -> Result<u64> {
    counts.iter().try_fold(0_u64, |total, (token, count)| {
        validate_token(field, token, allowed)?;
        total
            .checked_add(*count)
            .with_context(|| format!("{field} counts overflow u64"))
    })
}

fn validate_environment_catalog_sets(
    field: &str,
    readable: &[String],
    unreadable: &[String],
    not_applicable: &[String],
    allowed: &[&str],
) -> Result<()> {
    validate_sorted_unique_tokens(&format!("{field} readable catalog"), readable, allowed)?;
    validate_sorted_unique_tokens(&format!("{field} unreadable catalog"), unreadable, allowed)?;
    validate_sorted_unique_tokens(
        &format!("{field} inapplicable catalog"),
        not_applicable,
        allowed,
    )?;
    if readable.iter().any(|catalog| {
        unreadable.binary_search(catalog).is_ok() || not_applicable.binary_search(catalog).is_ok()
    }) || unreadable
        .iter()
        .any(|catalog| not_applicable.binary_search(catalog).is_ok())
    {
        bail!("{field} catalog evidence sets must be disjoint");
    }
    Ok(())
}

fn validate_dataset_scope(blueprint: &BlueprintFile, scope: &crate::DatasetScope) -> Result<()> {
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
            "oracle-table-statistics",
            "oracle-segment-statistics",
            "parquet-footer",
            "avro-decoded-scan",
            "structured-dataset-aggregate",
            "distributed-aggregate",
            "bounded-complete-read",
            "mixed-catalog-and-bounded-read",
            "not-applicable",
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
            "oracle-segment-bytes",
            "oracle-table-logical-estimate",
            "citus-distributed-relation-size",
            "parquet-footer",
            "avro-container",
            "structured-dataset-aggregate",
            "distributed-aggregate",
            "mixed",
            "not-applicable",
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
            "catalog-capture-truncated",
            "local-member-only",
            "logical-partition-root-unmeasured",
            "memory-optimized-data-unmeasured",
            "replica-membership-unresolved",
            "row-count-evidence-incomplete",
            "row-counts-statistical",
            "selection-limited",
            "shard-membership-incomplete",
            "size-evidence-incomplete",
            "statistics-stale",
            "table-inventory-visibility-unknown",
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
    if scope
        .limitations
        .binary_search(&"table-inventory-visibility-unknown".to_string())
        .is_ok()
        && scope.table_inventory_completeness == "complete"
    {
        bail!("table-inventory-visibility-unknown cannot accompany complete table inventory");
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
    if (scope.row_count_method == "not-applicable" || scope.size_method == "not-applicable")
        && (blueprint.engine != "oracle"
            || blueprint.tables.is_empty()
            || blueprint.totals.table_count != 0
            || scope.row_count_method != "not-applicable"
            || scope.size_method != "not-applicable"
            || scope.row_count_completeness != "complete"
            || scope.size_completeness != "complete")
    {
        bail!(
            "not-applicable Oracle row and size methods require a non-empty inventory, an empty copy-total population, and complete coverage"
        );
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

fn validate_structure_scope(
    blueprint: &BlueprintFile,
    scope: &crate::StructureScope,
) -> Result<()> {
    if scope.contract != crate::STRUCTURE_SCOPE_CONTRACT {
        bail!(
            "unsupported structure scope contract '{}'; expected '{}'",
            scope.contract,
            crate::STRUCTURE_SCOPE_CONTRACT
        );
    }
    validate_token(
        "structure visibility",
        &scope.visibility,
        &["full", "privilege-filtered", "unknown"],
    )?;
    for (field, value) in [
        (
            "table inventory",
            scope.table_inventory_completeness.as_str(),
        ),
        (
            "column inventory",
            scope.column_inventory_completeness.as_str(),
        ),
        (
            "index inventory",
            scope.index_inventory_completeness.as_str(),
        ),
        (
            "relationship inventory",
            scope.relationship_inventory_completeness.as_str(),
        ),
    ] {
        validate_token(field, value, &["complete", "incomplete", "unknown"])?;
    }
    const CATALOGS: &[&str] = &[
        "avro-schema",
        "mysql-information-schema-columns",
        "mysql-information-schema-foreign-keys",
        "mysql-information-schema-indexes",
        "mysql-information-schema-tables",
        "oracle-constraints",
        "oracle-constraint-columns",
        "oracle-identity-columns",
        "oracle-indexes",
        "oracle-tab-columns",
        "oracle-tables",
        "parquet-schema",
        "postgresql-columns",
        "postgresql-foreign-keys",
        "postgresql-indexes",
        "postgresql-tables",
        "sqlserver-columns",
        "sqlserver-foreign-keys",
        "sqlserver-indexes",
        "sqlserver-tables",
    ];
    validate_environment_catalog_sets(
        "structure",
        &scope.catalogs_read,
        &scope.catalogs_unreadable,
        &scope.catalogs_not_applicable,
        CATALOGS,
    )?;
    validate_sorted_unique_tokens(
        "structure limitation",
        &scope.limitations,
        &[
            "column-inventory-unavailable",
            "catalog-capture-truncated",
            "dependent-structure-suppressed",
            "index-inventory-unavailable",
            "metadata-visibility-privilege-filtered",
            "metadata-visibility-unknown",
            "relationship-inventory-unavailable",
            "selection-limited",
            "table-kinds-not-inventoried",
            "table-inventory-unavailable",
        ],
    )?;
    let dataset_selection_limited = blueprint.dataset_scope.as_ref().is_some_and(|dataset| {
        dataset
            .limitations
            .iter()
            .any(|limitation| limitation == "selection-limited")
    });
    let structure_selection_limited = scope
        .limitations
        .iter()
        .any(|limitation| limitation == "selection-limited");
    if dataset_selection_limited != structure_selection_limited {
        bail!("structure scope does not match the schema-selection evidence");
    }
    let completeness = [
        scope.table_inventory_completeness.as_str(),
        scope.column_inventory_completeness.as_str(),
        scope.index_inventory_completeness.as_str(),
        scope.relationship_inventory_completeness.as_str(),
    ];
    if (scope.visibility != "full" || completeness.iter().any(|value| *value != "complete"))
        && scope.limitations.is_empty()
    {
        bail!("partial structure scope requires at least one limitation");
    }
    if scope.table_inventory_completeness != "complete" && completeness[1..].contains(&"complete") {
        bail!("complete dependent structure requires complete table inventory");
    }
    if scope.column_inventory_completeness != "complete" && completeness[2..].contains(&"complete")
    {
        bail!("complete index or relationship inventory requires complete columns");
    }
    let expected = match blueprint.engine.as_str() {
        "postgresql" => Some([
            "postgresql-tables",
            "postgresql-columns",
            "postgresql-indexes",
            "postgresql-foreign-keys",
        ]),
        "mysql" => Some([
            "mysql-information-schema-tables",
            "mysql-information-schema-columns",
            "mysql-information-schema-indexes",
            "mysql-information-schema-foreign-keys",
        ]),
        "sqlserver" => Some([
            "sqlserver-tables",
            "sqlserver-columns",
            "sqlserver-indexes",
            "sqlserver-foreign-keys",
        ]),
        "oracle" => Some([
            "oracle-tables",
            "oracle-tab-columns",
            "oracle-indexes",
            "oracle-constraints",
        ]),
        "parquet" => Some(["parquet-schema"; 4]),
        "avro" => Some(["avro-schema"; 4]),
        _ => None,
    };
    if let Some(expected) = expected {
        for (complete, catalog) in completeness.into_iter().zip(expected) {
            if complete == "complete"
                && scope
                    .catalogs_read
                    .binary_search_by(|value| value.as_str().cmp(catalog))
                    .is_err()
            {
                bail!("complete structure evidence requires readable catalog '{catalog}'");
            }
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

fn validate_sorted_unique_values(field: &str, values: &[String]) -> Result<()> {
    let mut previous: Option<&str> = None;
    for value in values {
        if value.is_empty() {
            bail!("{field} contains an empty value");
        }
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
    let expected_contract = if blueprint.schema_version >= 7 {
        crate::ARTIFACT_CONTRACT
    } else {
        crate::PREVIOUS_ARTIFACT_CONTRACT
    };
    if inventory.contract != expected_contract && !legacy_contract {
        bail!(
            "unsupported artifact inventory contract '{}'; expected '{}'",
            inventory.contract,
            expected_contract
        );
    }
    if blueprint.schema_version < 7 {
        if !inventory.scope.is_empty()
            || !inventory.catalogs_not_applicable.is_empty()
            || inventory.requirements_complete
        {
            bail!(
                "artifact scope, catalogs_not_applicable and requirements_complete require schema v7"
            );
        }
    } else {
        validate_token(
            "artifact scope",
            &inventory.scope,
            &[
                "all-visible-schemas",
                "selected-schemas",
                "structured-source",
                "unknown",
            ],
        )?;
        let structured = matches!(blueprint.engine.as_str(), "parquet" | "avro");
        let selection_limited = blueprint.dataset_scope.as_ref().is_some_and(|scope| {
            scope
                .limitations
                .iter()
                .any(|limitation| limitation == "selection-limited")
        });
        if structured != (inventory.scope == "structured-source") {
            bail!("artifact scope does not match the Blueprint source kind");
        }
        if selection_limited != (inventory.scope == "selected-schemas") {
            bail!("artifact scope does not match the schema-selection evidence");
        }
    }
    validate_token(
        "artifact detail",
        &inventory.detail,
        &["none", "summary", "graph", "analyzed"],
    )?;
    if blueprint.schema_version < 7 && inventory.complexity.is_some() {
        bail!("artifact complexity requires Blueprint schema_version 7 or newer");
    }
    if matches!(inventory.detail.as_str(), "none" | "summary") && inventory.complexity.is_some() {
        bail!(
            "artifact detail '{}' must not contain a complexity assessment",
            inventory.detail
        );
    }
    if blueprint.schema_version >= 7
        && matches!(inventory.detail.as_str(), "graph" | "analyzed")
        && inventory.complexity.is_none()
    {
        bail!(
            "artifact detail '{}' requires a complexity assessment",
            inventory.detail
        );
    }
    if let Some(complexity) = inventory.complexity.as_ref() {
        validate_artifact_complexity(inventory, complexity)?;
    }
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
    for (field, catalogs) in [
        ("artifact readable catalog", &inventory.catalogs_read),
        (
            "artifact unreadable catalog",
            &inventory.catalogs_unreadable,
        ),
        (
            "artifact inapplicable catalog",
            &inventory.catalogs_not_applicable,
        ),
    ] {
        let mut previous: Option<&str> = None;
        for catalog in catalogs {
            validate_catalog_label(field, catalog)?;
            if previous.is_some_and(|previous| previous >= catalog.as_str()) {
                bail!("{field} values must be sorted and unique");
            }
            previous = Some(catalog);
        }
    }
    if inventory.catalogs_read.iter().any(|catalog| {
        inventory.catalogs_unreadable.binary_search(catalog).is_ok()
            || inventory
                .catalogs_not_applicable
                .binary_search(catalog)
                .is_ok()
    }) || inventory.catalogs_unreadable.iter().any(|catalog| {
        inventory
            .catalogs_not_applicable
            .binary_search(catalog)
            .is_ok()
    }) {
        bail!("artifact catalog evidence sets must be disjoint");
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
    if inventory.requirements_complete && !matches!(inventory.detail.as_str(), "graph" | "analyzed")
    {
        bail!("artifact requirements_complete requires graph or analyzed detail");
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
    let mut all_requirement_statuses_complete = true;
    for (artifact_id, artifact) in &inventory.artifacts {
        validate_artifact_kind(&artifact.kind)?;
        if blueprint.schema_version >= 7 {
            validate_artifact_id(artifact_id, &artifact.kind)?;
        } else if !artifact_id.starts_with(&format!("{}-", artifact.kind)) {
            bail!(
                "artifact id '{artifact_id}' does not match kind '{}'",
                artifact.kind
            );
        }
        if blueprint.schema_version >= 7 {
            validate_artifact_subkind(&artifact.subkind)?;
        } else {
            validate_closed_identifier("artifact subkind", &artifact.subkind)?;
        }
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
        if !artifact.schema.is_empty() {
            if blueprint.schema_version >= 7 {
                validate_artifact_schema_id(&artifact.schema).with_context(|| {
                    format!("artifact '{artifact_id}' has a non-anonymous schema id")
                })?;
            } else if !artifact.schema.starts_with("schema-") {
                bail!("artifact '{artifact_id}' has a non-anonymous schema id");
            }
        }
        if !artifact.parent.is_empty()
            && (artifact.parent == *artifact_id || !valid_ids.contains(&artifact.parent))
        {
            bail!(
                "artifact '{artifact_id}' references an invalid parent '{}'",
                artifact.parent
            );
        }
        let mut observed_unresolved = 0_u64;
        if blueprint.schema_version >= 7 {
            validate_token(
                "artifact requirement_status",
                &artifact.requirement_status,
                &["complete", "partial", "unavailable", "not_applicable"],
            )?;
            if !matches!(
                artifact.requirement_status.as_str(),
                "complete" | "not_applicable"
            ) {
                all_requirement_statuses_complete = false;
            }
            if artifact.requirement_status == "not_applicable"
                && (!artifact.requirements.is_empty() || artifact.external.is_some())
            {
                bail!(
                    "artifact '{artifact_id}' cannot carry requirements or an external prerequisite when requirement_status is not_applicable"
                );
            }
            if artifact.requirement_status == "unavailable"
                && (!artifact.requirements.is_empty() || artifact.external.is_some())
            {
                bail!(
                    "artifact '{artifact_id}' must use partial requirement_status when known requirement or external-prerequisite evidence is present"
                );
            }
            if !artifact.validity.is_empty() {
                validate_token(
                    "artifact validity",
                    &artifact.validity,
                    &["valid", "invalid", "not-applicable", "unknown"],
                )?;
            }
            if !artifact.dependencies.is_empty() || artifact.unresolved_dependency_count != 0 {
                bail!("schema-v7 artifact '{artifact_id}' must use typed relationships");
            }
            let mut previous: Option<(&str, &str, &str)> = None;
            for relationship in &artifact.relationships {
                validate_token(
                    "artifact relationship kind",
                    &relationship.kind,
                    &[
                        "calls-routine",
                        "fires-trigger",
                        "implemented-by",
                        "physically-placed-by",
                        "reads-table",
                        "references-object",
                        "references-table",
                        "scheduled-by",
                        "secured-by",
                        "uses-extension",
                        "uses-external-binary",
                        "uses-external-service",
                        "uses-remote-database",
                        "uses-remote-server",
                        "uses-type",
                        "writes-table",
                    ],
                )?;
                validate_artifact_evidence(
                    "artifact relationship evidence",
                    &relationship.evidence,
                )?;
                if !valid_ids.contains(&relationship.target) {
                    bail!(
                        "artifact '{artifact_id}' references missing target '{}'",
                        relationship.target
                    );
                }
                let current = (
                    relationship.kind.as_str(),
                    relationship.target.as_str(),
                    relationship.evidence.as_str(),
                );
                if previous.is_some_and(|previous| previous >= current) {
                    bail!("artifact '{artifact_id}' relationships must be sorted and unique");
                }
                previous = Some(current);
            }
            let mut previous: Option<(&str, &str, &str)> = None;
            for requirement in &artifact.requirements {
                validate_artifact_requirement_token(&requirement.token)?;
                validate_artifact_evidence("artifact requirement evidence", &requirement.evidence)?;
                validate_token(
                    "artifact requirement count_band",
                    &requirement.count_band,
                    &[
                        "1",
                        "2-4",
                        "5-9",
                        "10-24",
                        "25-49",
                        "50-99",
                        "100-249",
                        "250-499",
                        "500-999",
                        "1000-plus",
                    ],
                )?;
                let current = (
                    requirement.token.as_str(),
                    requirement.evidence.as_str(),
                    requirement.count_band.as_str(),
                );
                if previous.is_some_and(|previous| previous >= current) {
                    bail!("artifact '{artifact_id}' requirements must be sorted and unique");
                }
                previous = Some(current);
            }
            for (reason, count) in &artifact.unresolved_relationships {
                validate_token(
                    "artifact unresolved relationship reason",
                    reason,
                    &[
                        "ambiguous-binding",
                        "caller-dependent",
                        "catalog-not-tracked",
                        "catalog-unreadable",
                        "cross-database",
                        "cross-server",
                        "definition-withheld",
                        "dynamic-sql",
                        "encrypted-definition",
                        "incomplete-native-identity",
                        "missing-identity",
                        "outside-selected-schema",
                        "remote-reference",
                        "target-family-unmodeled",
                        "target-not-visible",
                        "unclassified-native-type",
                        "unknown",
                    ],
                )?;
                if *count == 0 {
                    bail!("artifact unresolved relationship counts must be positive");
                }
                observed_unresolved = observed_unresolved
                    .checked_add(*count)
                    .context("artifact unresolved relationship counts overflow u64")?;
            }
            observed_edges = observed_edges
                .checked_add(artifact.relationships.len() as u64)
                .context("artifact relationship edge count overflows u64")?;
        } else {
            if !artifact.relationships.is_empty()
                || !artifact.requirements.is_empty()
                || !artifact.requirement_status.is_empty()
                || !artifact.unresolved_relationships.is_empty()
                || !artifact.validity.is_empty()
                || artifact.enabled.is_some()
                || artifact.generated_by_engine.is_some()
                || artifact.temporary.is_some()
                || artifact.editioned.is_some()
                || artifact.secondary_object.is_some()
            {
                bail!("schema-v7 artifact evidence appears in an older Blueprint");
            }
            for dependency in &artifact.dependencies {
                if !valid_ids.contains(dependency) {
                    bail!("artifact '{artifact_id}' references missing dependency '{dependency}'");
                }
            }
            observed_edges = observed_edges
                .checked_add(artifact.dependencies.len() as u64)
                .context("artifact dependency edge count overflows u64")?;
            observed_unresolved = artifact.unresolved_dependency_count;
        }
        if inventory.dependencies_complete && observed_unresolved != 0 {
            bail!("complete artifact dependencies contain unresolved relationships");
        }
        *observed_by_kind.entry(artifact.kind.clone()).or_default() += 1;
        if let Some(external) = artifact.external.as_ref() {
            validate_external(external)?;
            *observed_external.entry(external.class.clone()).or_default() += 1;
        }
        if let Some(analysis) = artifact.analysis.as_ref() {
            if inventory.detail != "analyzed" {
                bail!("artifact '{artifact_id}' has analysis outside analyzed detail");
            }
            validate_language_census(blueprint.schema_version, analysis)?;
            if inventory.analysis_complete && analysis.status != "complete" {
                bail!(
                    "artifact analysis cannot be complete while '{artifact_id}' has status '{}'",
                    analysis.status
                );
            }
        }
        if blueprint.schema_version >= 7 {
            if inventory.detail != "analyzed"
                && matches!(
                    artifact.definition_visibility.as_str(),
                    "available" | "withheld" | "unavailable" | "encrypted"
                )
            {
                bail!(
                    "artifact '{artifact_id}' claims definition visibility outside analyzed detail"
                );
            }
            match (
                artifact.definition_visibility.as_str(),
                artifact.analysis.as_ref().map(|analysis| analysis.status.as_str()),
            ) {
                ("available", None) if inventory.detail == "analyzed" => {
                    bail!("artifact '{artifact_id}' has an available definition but no analysis")
                }
                ("not_applicable", Some(status)) if status != "not_applicable" => bail!(
                    "artifact '{artifact_id}' has a non-applicable definition but analysis status '{status}'"
                ),
                ("withheld" | "unavailable" | "encrypted" | "external_binary", Some(status))
                    if !matches!(status, "unavailable" | "not_applicable") =>
                {
                    bail!(
                        "artifact '{artifact_id}' has definition visibility '{}' but analysis status '{status}'",
                        artifact.definition_visibility
                    )
                }
                _ => {}
            }
        }
    }
    if inventory.requirements_complete && !all_requirement_statuses_complete {
        bail!(
            "artifact requirements_complete requires complete or not_applicable requirement_status on every artifact"
        );
    }
    if blueprint.schema_version >= 7 {
        for artifact_id in inventory.artifacts.keys() {
            let mut seen = BTreeSet::new();
            let mut current = artifact_id.as_str();
            while seen.insert(current) {
                let parent = inventory.artifacts[current].parent.as_str();
                if parent.is_empty() || blueprint.tables.contains_key(parent) {
                    break;
                }
                current = parent;
            }
            if inventory
                .artifacts
                .get(current)
                .is_some_and(|artifact| !artifact.parent.is_empty())
                && !blueprint
                    .tables
                    .contains_key(inventory.artifacts[current].parent.as_str())
            {
                bail!("artifact parent hierarchy contains a cycle at '{current}'");
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

fn validate_artifact_complexity(
    inventory: &crate::ArtifactInventory,
    complexity: &crate::ArtifactComplexity,
) -> Result<()> {
    if complexity.contract != crate::ARTIFACT_COMPLEXITY_CONTRACT {
        bail!(
            "unsupported artifact complexity contract '{}'; expected '{}'",
            complexity.contract,
            crate::ARTIFACT_COMPLEXITY_CONTRACT
        );
    }
    if complexity.assessor_version != crate::ARTIFACT_COMPLEXITY_ASSESSOR_VERSION {
        bail!(
            "unsupported artifact complexity assessor_version {}; expected {}",
            complexity.assessor_version,
            crate::ARTIFACT_COMPLEXITY_ASSESSOR_VERSION
        );
    }
    if complexity.scope != inventory.scope {
        bail!(
            "artifact complexity scope '{}' does not match inventory scope '{}'",
            complexity.scope,
            inventory.scope
        );
    }
    validate_token(
        "artifact complexity scope",
        &complexity.scope,
        &["all-visible-schemas", "selected-schemas", "unknown"],
    )?;
    if complexity.population_policy != crate::ARTIFACT_COMPLEXITY_POPULATION_POLICY {
        bail!(
            "unsupported artifact complexity population_policy '{}'; expected '{}'",
            complexity.population_policy,
            crate::ARTIFACT_COMPLEXITY_POPULATION_POLICY
        );
    }
    if inventory.requirements_complete && !complexity.assessment_population_complete {
        bail!(
            "artifact requirements_complete requires a complete assessment population for the selected scope"
        );
    }
    validate_token(
        "artifact complexity analyzer_version",
        &complexity.analyzer_version,
        &["lexical-v2", "not-applicable"],
    )?;
    validate_sorted_unique_tokens(
        "artifact complexity analysis_span",
        &complexity.analysis_spans,
        &["executable-body", "not-applicable", "unknown"],
    )?;
    validate_sorted_unique_tokens(
        "artifact complexity dialect",
        &complexity.dialects,
        LANGUAGE_CENSUS_DIALECTS,
    )?;
    validate_sorted_unique_values(
        "artifact complexity grammar_profile",
        &complexity.grammar_profiles,
    )?;
    for profile in &complexity.grammar_profiles {
        validate_profile(profile)?;
    }
    validate_overall_complexity_band("artifact complexity overall_band", &complexity.overall_band)?;
    if complexity.overall_score.is_some() {
        bail!("artifact complexity assessor v1 must not populate reserved overall_score");
    }
    validate_sorted_unique_tokens(
        "artifact complexity limitation",
        &complexity.limitations,
        &[
            "computation-failed",
            "computation-limit",
            "definition-analysis-not-requested",
            "definitions-withheld",
            "graph-incomplete",
            "outside-selected-scope",
            "requirements-incomplete",
            "unsupported-dialect",
            "wrapped-source",
        ],
    )?;

    let assessed = complexity
        .fully_assessed_object_count
        .checked_add(complexity.partially_assessed_object_count)
        .context("artifact complexity assessed object counts overflow u64")?;
    let classified = assessed
        .checked_add(complexity.unassessed_object_count)
        .context("artifact complexity assessment population overflows u64")?;
    if classified != complexity.eligible_object_count {
        bail!(
            "artifact complexity eligible_object_count {} does not match fully, partially and unassessed sum {}",
            complexity.eligible_object_count,
            classified
        );
    }
    let inventory_population = complexity
        .eligible_object_count
        .checked_add(complexity.excluded_object_count)
        .context("artifact complexity inventory population overflows u64")?;
    if inventory_population != inventory.object_count {
        bail!(
            "artifact complexity population {} does not match artifact object_count {}",
            inventory_population,
            inventory.object_count
        );
    }
    let expected_excluded = inventory
        .artifacts
        .values()
        .try_fold(0_u64, |count, artifact| {
            if artifact.generated_by_engine == Some(true) || artifact.secondary_object == Some(true)
            {
                count.checked_add(1)
            } else {
                Some(count)
            }
        })
        .context("artifact complexity excluded population overflows u64")?;
    if expected_excluded != complexity.excluded_object_count {
        bail!(
            "artifact complexity excluded_object_count {} does not match the recorded population policy result {}",
            complexity.excluded_object_count,
            expected_excluded
        );
    }

    if complexity.eligible_object_count == 0
        && (complexity.partially_assessed_object_count != 0
            || complexity.unassessed_object_count != 0)
    {
        bail!("an empty artifact complexity population cannot be partial or unassessed");
    }

    let mut expected_dialects = BTreeSet::new();
    let mut expected_profiles = BTreeSet::new();
    let mut expected_analysis_spans = BTreeSet::new();
    let mut expected_analyzer_versions = BTreeSet::new();
    let mut unsupported_dialect_observed = false;
    let mut definitions_withheld_observed = false;
    let mut wrapped_source_observed = false;
    let mut outside_selected_scope_observed = false;
    for artifact in inventory.artifacts.values().filter(|artifact| {
        artifact.generated_by_engine != Some(true) && artifact.secondary_object != Some(true)
    }) {
        definitions_withheld_observed |= artifact.definition_visibility == "withheld";
        wrapped_source_observed |= artifact
            .requirements
            .iter()
            .any(|requirement| requirement.token == "oracle.source.wrapped");
        outside_selected_scope_observed |= artifact
            .unresolved_relationships
            .contains_key("outside-selected-schema");
        let Some(analysis) = artifact.analysis.as_ref() else {
            continue;
        };
        wrapped_source_observed |= analysis.features.contains_key("source.wrapped");
        expected_dialects.insert(analysis.dialect.clone());
        expected_profiles.insert(analysis.grammar_profile.clone());
        expected_analysis_spans.insert(if analysis.analysis_span.is_empty() {
            "unknown".to_string()
        } else {
            analysis.analysis_span.clone()
        });
        expected_analyzer_versions.insert(analysis.analyzer_version.clone());
        if analysis.status == "unavailable"
            && artifact.definition_visibility == "available"
            && !LEXICAL_SUPPORTED_DIALECTS.contains(&analysis.dialect.as_str())
        {
            unsupported_dialect_observed = true;
        }
    }
    let actual_dialects = complexity.dialects.iter().cloned().collect::<BTreeSet<_>>();
    let actual_profiles = complexity
        .grammar_profiles
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let actual_analysis_spans = complexity
        .analysis_spans
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if actual_dialects != expected_dialects {
        bail!("artifact complexity dialects do not match the eligible language census records");
    }
    if actual_profiles != expected_profiles {
        bail!(
            "artifact complexity grammar_profiles do not match the eligible language census records"
        );
    }
    if actual_analysis_spans != expected_analysis_spans {
        bail!(
            "artifact complexity analysis_spans do not match the eligible language census records"
        );
    }
    if inventory.detail == "graph" {
        if complexity.analyzer_version != "not-applicable"
            || !complexity.dialects.is_empty()
            || !complexity.grammar_profiles.is_empty()
            || !complexity.analysis_spans.is_empty()
        {
            bail!(
                "graph artifact complexity requires not-applicable analyzer evidence and empty span/dialect/profile sets"
            );
        }
    } else {
        if complexity.analyzer_version != "lexical-v2" {
            bail!("analyzed artifact complexity requires lexical-v2 analyzer_version");
        }
        if expected_analyzer_versions
            .iter()
            .any(|version| version != &complexity.analyzer_version)
        {
            bail!(
                "artifact complexity analyzer_version does not match every eligible language census record"
            );
        }
    }

    let limitation_is_present = |expected: &str| {
        complexity
            .limitations
            .binary_search_by(|value| value.as_str().cmp(expected))
            .is_ok()
    };
    let has_limit = limitation_is_present("computation-limit");
    let has_failure = limitation_is_present("computation-failed");
    if has_limit && has_failure {
        bail!("artifact complexity cannot be both computation-limited and computation-failed");
    }
    if has_limit {
        bail!(
            "artifact complexity assessor v1 reserves computation-limit but does not yet define a recomputable limited result"
        );
    }
    let definition_not_requested = limitation_is_present("definition-analysis-not-requested");
    if inventory.detail == "graph" && !definition_not_requested {
        bail!("graph artifact complexity requires definition-analysis-not-requested limitation");
    }
    if inventory.detail == "analyzed" && definition_not_requested {
        bail!("analyzed artifact complexity must not claim definition-analysis-not-requested");
    }
    if inventory.detail == "graph"
        && complexity.limitations.iter().any(|limitation| {
            matches!(
                limitation.as_str(),
                "definitions-withheld" | "unsupported-dialect" | "wrapped-source"
            )
        })
    {
        bail!("graph artifact complexity cannot report definition-read limitations");
    }
    for (limitation, observed) in [
        (
            "definitions-withheld",
            inventory.detail == "analyzed" && definitions_withheld_observed,
        ),
        (
            "unsupported-dialect",
            inventory.detail == "analyzed" && unsupported_dialect_observed,
        ),
        (
            "wrapped-source",
            inventory.detail == "analyzed" && wrapped_source_observed,
        ),
        ("graph-incomplete", !inventory.dependencies_complete),
        ("requirements-incomplete", !inventory.requirements_complete),
        ("outside-selected-scope", outside_selected_scope_observed),
    ] {
        if limitation_is_present(limitation) != observed {
            bail!(
                "artifact complexity limitation '{limitation}' does not match the eligible artifact evidence"
            );
        }
    }
    if limitation_is_present("outside-selected-scope") && inventory.scope != "selected-schemas" {
        bail!("outside-selected-scope requires selected-schemas artifact scope");
    }

    validate_artifact_complexity_size_dimension(
        "volume",
        &complexity.dimensions.volume,
        complexity.eligible_object_count,
    )?;
    for (name, dimension) in [
        ("control_flow", &complexity.dimensions.control_flow),
        ("feature_breadth", &complexity.dimensions.feature_breadth),
        ("entanglement", &complexity.dimensions.entanglement),
        (
            "environment_coupling",
            &complexity.dimensions.environment_coupling,
        ),
        ("opacity", &complexity.dimensions.opacity),
        ("dialect_coupling", &complexity.dimensions.dialect_coupling),
    ] {
        validate_artifact_complexity_count_dimension(
            name,
            dimension,
            complexity.eligible_object_count,
        )?;
    }

    let any_dimension_unknown = complexity.dimensions.volume.histogram.unknown != 0
        || complexity.dimensions.control_flow.histogram.unknown != 0
        || complexity.dimensions.feature_breadth.histogram.unknown != 0
        || complexity.dimensions.entanglement.histogram.unknown != 0
        || complexity.dimensions.environment_coupling.histogram.unknown != 0
        || complexity.dimensions.opacity.histogram.unknown != 0
        || complexity.dimensions.dialect_coupling.histogram.unknown != 0;
    if !any_dimension_unknown
        && (complexity.partially_assessed_object_count != 0
            || complexity.unassessed_object_count != 0)
    {
        bail!(
            "artifact complexity object coverage cannot be partial or unassessed when every dimension is determined"
        );
    }
    let maximum_dimension_unknown = [
        complexity.dimensions.volume.histogram.unknown,
        complexity.dimensions.control_flow.histogram.unknown,
        complexity.dimensions.feature_breadth.histogram.unknown,
        complexity.dimensions.entanglement.histogram.unknown,
        complexity.dimensions.environment_coupling.histogram.unknown,
        complexity.dimensions.opacity.histogram.unknown,
        complexity.dimensions.dialect_coupling.histogram.unknown,
    ]
    .into_iter()
    .max()
    .unwrap_or(0);
    let objects_with_any_unknown = complexity
        .partially_assessed_object_count
        .checked_add(complexity.unassessed_object_count)
        .context("artifact complexity incomplete object count overflows u64")?;
    if maximum_dimension_unknown > objects_with_any_unknown {
        bail!(
            "artifact complexity per-dimension unknown counts exceed partial and unassessed object coverage"
        );
    }
    if maximum_dimension_unknown == 0 && objects_with_any_unknown != 0 {
        bail!(
            "artifact complexity cannot classify objects as partial or unassessed when every dimension is determined"
        );
    }
    let every_dimension_unknown = [
        complexity.dimensions.volume.histogram.unknown,
        complexity.dimensions.control_flow.histogram.unknown,
        complexity.dimensions.feature_breadth.histogram.unknown,
        complexity.dimensions.entanglement.histogram.unknown,
        complexity.dimensions.environment_coupling.histogram.unknown,
        complexity.dimensions.opacity.histogram.unknown,
        complexity.dimensions.dialect_coupling.histogram.unknown,
    ]
    .iter()
    .all(|unknown| *unknown == complexity.eligible_object_count);
    if complexity.eligible_object_count != 0
        && every_dimension_unknown
        && complexity.unassessed_object_count != complexity.eligible_object_count
    {
        bail!(
            "artifact complexity must classify a population with every dimension unknown as unassessed"
        );
    }
    if (has_limit || has_failure) && complexity.eligible_object_count != 0 && !any_dimension_unknown
    {
        bail!(
            "a limited or failed artifact complexity calculation must leave affected dimension evidence unknown"
        );
    }
    if wrapped_source_observed && complexity.dimensions.opacity.histogram.unknown == 0 {
        bail!("wrapped source must contribute to unknown opacity coverage");
    }

    if complexity.eligible_object_count == 0 && complexity.assessment_population_complete {
        if complexity.overall_band != "not-applicable" {
            bail!("an empty, complete artifact population requires not-applicable overall_band");
        }
    } else if complexity.eligible_object_count == 0 {
        if complexity.overall_band != "unknown" {
            bail!("an empty, incomplete artifact population requires unknown overall_band");
        }
    } else {
        if complexity.overall_band == "not-applicable" {
            bail!("non-empty artifact complexity cannot use not-applicable overall_band");
        }
        if inventory.detail == "graph" && complexity.overall_band != "unknown" {
            bail!("graph artifact complexity must use unknown overall_band");
        }
        if inventory.detail == "analyzed"
            && complexity.assessment_population_complete
            && complexity.fully_assessed_object_count == complexity.eligible_object_count
            && complexity.overall_band == "unknown"
        {
            bail!("a fully assessed artifact population requires a definitive overall_band");
        }
        if inventory.detail == "analyzed"
            && complexity.unassessed_object_count == complexity.eligible_object_count
            && complexity.overall_band != "unknown"
        {
            bail!("a wholly unassessed artifact population requires unknown overall_band");
        }
        if !complexity.assessment_population_complete
            && !matches!(complexity.overall_band.as_str(), "unknown" | "very-high")
        {
            bail!(
                "an incomplete artifact assessment population permits only unknown overall_band unless the known lower bound is already very-high"
            );
        }
    }

    if has_failure {
        let expected =
            crate::failed_artifact_complexity(inventory, complexity.assessment_population_complete)
                .context("recomputing fail-closed artifact complexity shape")?;
        if complexity != &expected {
            bail!(
                "failed artifact complexity does not match the required fail-closed unknown shape"
            );
        }
    } else {
        let expected =
            crate::assess_artifact_complexity(inventory, complexity.assessment_population_complete)
                .context("recomputing artifact complexity from the serialized graph")?;
        if complexity != &expected {
            bail!(
                "artifact complexity does not match assessor-v1 recomputation from the serialized artifact graph"
            );
        }
    }

    Ok(())
}

fn validate_artifact_complexity_size_dimension(
    name: &str,
    dimension: &crate::ArtifactComplexitySizeDimension,
    eligible_object_count: u64,
) -> Result<()> {
    let histogram = &dimension.histogram;
    validate_overall_complexity_band(&format!("artifact complexity {name} band"), &dimension.band)?;
    validate_artifact_complexity_dimension_coverage(
        name,
        &dimension.band,
        &dimension.coverage,
        &[
            histogram.zero,
            histogram.one_to_two_fifty_five,
            histogram.two_fifty_six_to_one_k,
            histogram.one_k_to_four_k,
            histogram.four_k_to_sixteen_k,
            histogram.sixteen_k_to_sixty_four_k,
            histogram.sixty_four_k_plus,
        ],
        histogram.not_applicable,
        histogram.unknown,
        eligible_object_count,
    )
}

fn validate_artifact_complexity_count_dimension(
    name: &str,
    dimension: &crate::ArtifactComplexityCountDimension,
    eligible_object_count: u64,
) -> Result<()> {
    let histogram = &dimension.histogram;
    validate_overall_complexity_band(&format!("artifact complexity {name} band"), &dimension.band)?;
    validate_artifact_complexity_dimension_coverage(
        name,
        &dimension.band,
        &dimension.coverage,
        &[
            histogram.zero,
            histogram.one,
            histogram.two_to_four,
            histogram.five_to_eight,
            histogram.nine_to_sixteen,
            histogram.seventeen_to_thirty_two,
            histogram.thirty_three_plus,
        ],
        histogram.not_applicable,
        histogram.unknown,
        eligible_object_count,
    )
}

fn validate_artifact_complexity_dimension_coverage(
    name: &str,
    band: &str,
    coverage: &str,
    assessed_bands: &[u64],
    not_applicable: u64,
    unknown: u64,
    eligible_object_count: u64,
) -> Result<()> {
    validate_token(
        &format!("artifact complexity {name} coverage"),
        coverage,
        &["complete", "partial", "not-applicable", "unknown"],
    )?;
    let assessed = assessed_bands
        .iter()
        .try_fold(0_u64, |sum, count| sum.checked_add(*count))
        .context("artifact complexity assessed histogram bands overflow u64")?;
    let total = assessed
        .checked_add(not_applicable)
        .and_then(|sum| sum.checked_add(unknown))
        .context("artifact complexity dimension histogram overflows u64")?;
    if total != eligible_object_count {
        bail!(
            "artifact complexity {name} histogram sum {total} does not match eligible_object_count {eligible_object_count}"
        );
    }
    match coverage {
        "complete" if unknown != 0 || not_applicable == eligible_object_count => {
            bail!(
                "complete artifact complexity {name} coverage requires determined applicable evidence"
            )
        }
        "partial" if unknown == 0 || unknown == eligible_object_count => {
            bail!(
                "partial artifact complexity {name} coverage requires both determined and unknown objects"
            )
        }
        "unknown" if eligible_object_count != 0 && unknown != eligible_object_count => {
            bail!("unknown artifact complexity {name} coverage must classify every object unknown")
        }
        "not-applicable"
            if not_applicable != eligible_object_count || assessed != 0 || unknown != 0 =>
        {
            bail!(
                "not-applicable artifact complexity {name} coverage must classify every object not-applicable"
            )
        }
        _ => {}
    }
    if coverage == "unknown" && band != "unknown" {
        bail!("unknown artifact complexity {name} coverage requires unknown band");
    }
    if coverage == "partial" && !matches!(band, "unknown" | "very-high") {
        bail!(
            "partial artifact complexity {name} coverage permits only unknown band unless the known lower bound is already very-high"
        );
    }
    if coverage == "complete" && matches!(band, "unknown" | "not-applicable") {
        bail!("complete artifact complexity {name} coverage requires a definitive band");
    }
    if (coverage == "not-applicable") != (band == "not-applicable") {
        bail!("not-applicable artifact complexity {name} coverage and band must agree");
    }
    Ok(())
}

fn validate_overall_complexity_band(field: &str, band: &str) -> Result<()> {
    validate_token(
        field,
        band,
        &[
            "trivial",
            "low",
            "moderate",
            "high",
            "very-high",
            "not-applicable",
            "unknown",
        ],
    )
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
            "package",
            "aggregate",
            "trigger",
            "event_trigger",
            "rule",
            "scheduled_job",
            "scheduler_program",
            "scheduler_schedule",
            "scheduler_chain",
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
            "database_link",
            "java",
            "library",
            "directory",
            "operator",
            "indextype",
            "domain",
            "annotation",
            "property_graph",
            "queue",
            "edition",
            "other",
        ],
    )
}

/// One engine-neutral vocabulary shared by every artifact kind. Keeping a
/// single list makes new providers reuse established distinctions instead of
/// creating near-synonyms that cannot be tightened later.
fn validate_artifact_subkind(subkind: &str) -> Result<()> {
    validate_token(
        "artifact subkind",
        subkind,
        &[
            "ordinary",
            "other",
            "materialized",
            "integer_sequence",
            "stored_procedure",
            "stored_function",
            "scalar_function",
            "inline_table_function",
            "table_function",
            "user_defined_aggregate",
            "table_trigger",
            "ddl_event_trigger",
            "before_insert",
            "before_update",
            "before_delete",
            "after_insert",
            "after_update",
            "after_delete",
            "generated_column",
            "column_default",
            "default_constraint",
            "check_constraint",
            "row_security",
            "rewrite_rule",
            "legacy_rule",
            "enum",
            "domain",
            "composite",
            "range",
            "alias_type",
            "table_type",
            "clr_type",
            "clr_procedure",
            "clr_scalar_function",
            "clr_table_function",
            "clr_aggregate",
            "clr_trigger",
            "clr_assembly",
            "server_extension",
            "loadable_udf",
            "foreign_data_wrapper_server",
            "foreign_table",
            "federated_table",
            "external_table",
            "external_data_source",
            "external_file_format",
            "logical_replication_publication",
            "logical_replication_subscription",
            "full_text_catalog",
            "partition_scheme",
            "partition_function",
            "tablespace",
            "filegroup",
            "database_certificate",
            "symmetric_key",
            "asymmetric_key",
            "column_master_key",
            "column_encryption_key",
            "database_scoped_credential",
            "linked_server",
            "enabled_event",
            "disabled_event",
            "enabled_agent_job",
            "disabled_agent_job",
            "database_synonym",
            "specification",
            "body",
            "package_member",
            "public",
            "private",
            "java_source",
            "java_class",
            "java_resource",
            "external_library",
            "user_defined_operator",
            "domain_indextype",
            "scheduler_job",
            "scheduler_program",
            "scheduler_schedule",
            "scheduler_chain",
            "advanced_queuing",
            "service_broker",
        ],
    )
}

fn validate_artifact_evidence(field: &str, evidence: &str) -> Result<()> {
    validate_token(
        field,
        evidence,
        &[
            "catalog-confirmed",
            "dependency-confirmed",
            "syntax-confirmed",
            "lexical-hint",
            "unresolved",
        ],
    )
}

fn validate_artifact_requirement_token(token: &str) -> Result<()> {
    validate_token(
        "artifact requirement token",
        token,
        &[
            "external.custom-binary-unknown",
            "external.custom-extension-unknown",
            "external.foreign-data-wrapper",
            "external.loadable-udf",
            "external.native-library",
            "external.procedural-language-runtime",
            "external.spatial-extension",
            "external.time-series-extension",
            "external.vector-extension",
            "mysql.event.recurring",
            "mysql.event.replica-disabled",
            "mysql.routine.modifies-sql-data",
            "mysql.routine.no-sql",
            "mysql.routine.reads-sql-data",
            "mysql.trigger.always-definer",
            "mysql.udf.aggregate",
            "mysql.udf.scalar",
            "mysql.view.non-updatable",
            "oracle.artifact.annotation",
            "oracle.artifact.database-link",
            "oracle.artifact.directory",
            "oracle.artifact.domain-index",
            "oracle.artifact.editioned",
            "oracle.artifact.external-library",
            "oracle.artifact.java",
            "oracle.artifact.property-graph",
            "oracle.artifact.scheduler",
            "oracle.language.autonomous-transaction",
            "oracle.language.bulk-collect",
            "oracle.language.conditional-compilation",
            "oracle.language.dbms-sql",
            "oracle.language.dynamic-sql",
            "oracle.language.forall",
            "oracle.language.package-initialization",
            "oracle.language.package-state",
            "oracle.language.percent-rowtype",
            "oracle.language.percent-type",
            "oracle.language.pragma",
            "oracle.language.ref-cursor",
            "oracle.language.sql-macro",
            "oracle.routine.aggregate",
            "oracle.routine.deterministic",
            "oracle.routine.interface",
            "oracle.routine.parallel",
            "oracle.routine.pipelined",
            "oracle.routine.polymorphic-table",
            "oracle.routine.result-cache",
            "oracle.source.wrapped",
            "oracle.trigger.compound",
            "oracle.type.collection",
            "oracle.type.domain",
            "oracle.type.json",
            "oracle.type.object",
            "oracle.type.spatial",
            "oracle.type.vector",
            "postgresql.aggregate.hypothetical-set",
            "postgresql.aggregate.moving",
            "postgresql.aggregate.ordered-set",
            "postgresql.function.leakproof",
            "postgresql.function.parallel-restricted",
            "postgresql.function.parallel-unsafe",
            "postgresql.function.security-definer",
            "postgresql.function.set-returning",
            "postgresql.function.volatile",
            "postgresql.function.window",
            "postgresql.policy.restrictive",
            "postgresql.trigger.constraint",
            "postgresql.trigger.transition-table",
            "postgresql.type.multirange",
            "postgresql.view.recursive",
            "postgresql.view.security-barrier",
            "postgresql.view.security-invoker",
            "sqlserver.clr.external-access",
            "sqlserver.clr.unsafe",
            "sqlserver.module.database-collation-dependent",
            "sqlserver.module.native-compiled",
            "sqlserver.module.recompile",
            "sqlserver.module.schema-bound",
            "sqlserver.procedure.startup",
            "sqlserver.scalar-udf.inlineable",
            "sqlserver.trigger.database-ddl",
            "sqlserver.trigger.server-ddl",
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
            "oracle_database_link",
            "oracle_directory",
            "oracle_external_library",
            "oracle_java_runtime",
            "oracle_scheduler",
        ],
    )
}

fn validate_language_census(
    schema_version: u32,
    analysis: &crate::LanguageFeatureCensus,
) -> Result<()> {
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
        LANGUAGE_CENSUS_DIALECTS,
    )?;
    if schema_version >= 7 {
        validate_token(
            "language analyzer version",
            &analysis.analyzer_version,
            &["lexical-v2"],
        )?;
        if !analysis.analysis_span.is_empty() {
            validate_token(
                "language analysis_span",
                &analysis.analysis_span,
                &["executable-body", "not-applicable", "unknown"],
            )?;
        }
        let effective_span = if analysis.analysis_span.is_empty() {
            "unknown"
        } else {
            analysis.analysis_span.as_str()
        };
        if analysis.status == "not_applicable" && effective_span != "not-applicable" {
            bail!("not-applicable language census requires not-applicable analysis_span");
        }
        if analysis.status != "not_applicable" && effective_span == "not-applicable" {
            bail!("not-applicable analysis_span requires not_applicable language census status");
        }
    } else {
        validate_token(
            "language analyzer version",
            &analysis.analyzer_version,
            &["lexical-v1"],
        )?;
        if !analysis.analysis_span.is_empty() {
            bail!("language analysis_span requires Blueprint schema_version 7 or newer");
        }
    }
    validate_profile(&analysis.grammar_profile)?;
    if !analysis.minimum_source_version.is_empty()
        && (analysis.minimum_source_version.len() > 32
            || !analysis
                .minimum_source_version
                .bytes()
                .all(|byte| byte.is_ascii_digit() || byte == b'.')
            || analysis.minimum_source_version.starts_with('.')
            || analysis.minimum_source_version.ends_with('.')
            || analysis.minimum_source_version.contains(".."))
    {
        bail!("language minimum_source_version contains a non-canonical value");
    }
    if analysis.minimum_version_complete && analysis.minimum_source_version.is_empty() {
        bail!("complete minimum source version evidence requires a version");
    }
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
        "control.goto",
        "control.raise",
        "binding.percent_type",
        "binding.percent_rowtype",
        "interface.cursor",
        "interface.ref_cursor",
        "interface.pipelined",
        "interface.polymorphic_table",
        "interface.sql_macro",
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
        "data.bulk_collect",
        "data.forall",
        "state.ddl",
        "state.temporary",
        "transaction.control",
        "transaction.autonomous",
        "dynamic.sql",
        "dynamic.dbms_sql",
        "package.state",
        "package.initialization",
        "trigger.compound",
        "compile.conditional",
        "compile.pragma",
        "external.java_call_spec",
        "external.stored_java",
        "external.library",
        "external.directory",
        "external.database_link",
        "scheduler.job",
        "messaging.aq",
        "source.wrapped",
        "source.opaque",
        "edition.editionable",
        "index.domain",
        "type.collection",
        "type.object",
        "type.domain",
        "type.rowid",
        "type.interval",
        "type.timezone",
        "type.boolean",
        "type.raw",
        "type.lob",
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

fn validate_artifact_id(artifact_id: &str, kind: &str) -> Result<()> {
    validate_numeric_identifier(artifact_id, kind, 3, true)
        .with_context(|| format!("artifact id '{artifact_id}' does not match kind '{kind}'"))
}

fn validate_numeric_identifier(
    identifier: &str,
    prefix: &str,
    minimum_width: usize,
    zero_padded: bool,
) -> Result<()> {
    let expected_prefix = format!("{prefix}-");
    let suffix = identifier
        .strip_prefix(&expected_prefix)
        .with_context(|| format!("identifier must start with '{expected_prefix}'"))?;
    if suffix.len() < minimum_width || !suffix.bytes().all(|byte| byte.is_ascii_digit()) {
        bail!("identifier must use a canonical decimal ordinal");
    }
    let ordinal = suffix
        .parse::<u64>()
        .context("identifier ordinal is out of range")?;
    let canonical = if zero_padded {
        format!("{ordinal:0minimum_width$}")
    } else {
        ordinal.to_string()
    };
    if ordinal == 0 || canonical != suffix {
        bail!("identifier is not a canonical positive ordinal");
    }
    Ok(())
}

fn validate_artifact_schema_id(schema_id: &str) -> Result<()> {
    let Some(suffix) = schema_id.strip_prefix("schema-") else {
        bail!("schema id must start with 'schema-'");
    };
    if suffix.is_empty() || !suffix.bytes().all(|byte| byte.is_ascii_uppercase()) {
        bail!("schema id must use uppercase alphabetic ordinals");
    }
    Ok(())
}

/// Validate the complete anonymous namespace, not only each identifier's
/// spelling. Dense canonical ordinals prevent a syntactically valid identifier
/// such as `schema-PRODUCTION` or `table-123456` from becoming a covert
/// source-derived label.
fn validate_v7_anonymous_identifier_sets(blueprint: &BlueprintFile) -> Result<()> {
    let observed_tables = blueprint.tables.keys().cloned().collect::<BTreeSet<_>>();
    let expected_tables = (1..=blueprint.tables.len())
        .map(|ordinal| format!("table-{ordinal:03}"))
        .collect::<BTreeSet<_>>();
    if observed_tables != expected_tables {
        bail!("schema-v7 table ids must be a dense canonical anonymous ordinal set");
    }

    let mut observed_schemas = BTreeSet::new();
    for (table_id, table) in &blueprint.tables {
        observed_schemas.insert(table.schema.clone());
        for (column_id, column) in &table.cols {
            if *column_id != format!("col-{}", column.ordinal) {
                bail!(
                    "table '{table_id}' column id '{column_id}' does not match ordinal {}",
                    column.ordinal
                );
            }
        }
        let observed_indexes = table.idxs.keys().cloned().collect::<BTreeSet<_>>();
        let expected_indexes = (1..=table.idxs.len())
            .map(|ordinal| format!("idx-{ordinal}"))
            .collect::<BTreeSet<_>>();
        if observed_indexes != expected_indexes {
            bail!("table '{table_id}' index ids must be a dense canonical anonymous ordinal set");
        }
    }

    if let Some(inventory) = blueprint.artifact_inventory.as_ref() {
        let mut observed_by_kind = BTreeMap::<String, BTreeSet<String>>::new();
        for (artifact_id, artifact) in &inventory.artifacts {
            if !artifact.schema.is_empty() {
                observed_schemas.insert(artifact.schema.clone());
            }
            observed_by_kind
                .entry(artifact.kind.clone())
                .or_default()
                .insert(artifact_id.clone());
        }
        for (kind, observed) in observed_by_kind {
            let expected = (1..=observed.len())
                .map(|ordinal| format!("{kind}-{ordinal:03}"))
                .collect::<BTreeSet<_>>();
            if observed != expected {
                bail!(
                    "schema-v7 artifact ids for kind '{kind}' must be a dense canonical anonymous ordinal set"
                );
            }
        }
    }

    let expected_schemas = (1..=observed_schemas.len())
        .map(canonical_schema_id)
        .collect::<BTreeSet<_>>();
    if observed_schemas != expected_schemas {
        bail!("schema-v7 schema ids must be a dense canonical anonymous ordinal set");
    }
    Ok(())
}

fn canonical_schema_id(mut ordinal: usize) -> String {
    debug_assert!(ordinal > 0);
    let mut suffix = String::new();
    while ordinal > 0 {
        let character = ((ordinal - 1) % 26) as u8 + b'A';
        suffix.insert(0, char::from(character));
        ordinal = (ordinal - 1) / 26;
    }
    format!("schema-{suffix}")
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
    schema_version: u32,
    table_id: &str,
    column_id: &str,
    table_rows: u64,
    exact_table_population: bool,
    null_fraction: Option<f64>,
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
    if schema_version >= 7 {
        if let Some(null_fraction) = null_fraction.filter(|_| cardinality.sample_rows > 0) {
            let expected = cardinality
                .sample_rows
                .saturating_sub(cardinality.non_null_rows) as f64
                / cardinality.sample_rows as f64;
            if (null_fraction - expected).abs() > 1e-12 {
                bail!(
                    "table '{table_id}' column '{column_id}' null_fraction disagrees with its cardinality population"
                );
            }
        }
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
    if cardinality.non_null_rows == 0 && cardinality.estimated_distinct_count > 0 {
        bail!(
            "table '{table_id}' column '{column_id}' estimates distinct values from an empty non-NULL census"
        );
    }
    if cardinality.non_null_rows == 0 && cardinality.top_value_fraction != 0.0 {
        bail!(
            "table '{table_id}' column '{column_id}' has a top-value fraction for an empty non-NULL census"
        );
    }
    if cardinality.complete_source_read
        && (!cardinality.measured
            || cardinality.sample_rows == 0
            || cardinality.sample_rows != table_rows
            || cardinality.estimated_distinct_count > table_rows)
    {
        bail!(
            "table '{table_id}' column '{column_id}' complete-read cardinality disagrees with the table's exact row population"
        );
    }
    if exact_table_population
        && (cardinality.sample_rows > table_rows
            || cardinality.estimated_distinct_count > table_rows)
    {
        bail!(
            "table '{table_id}' column '{column_id}' cardinality exceeds the table's exact row population"
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

/// Validate any value implementing `Serialize` against the canonical contract.
/// Values already held as `BlueprintFile` can use
/// `validate_blueprint_contract` to avoid the serialisation boundary.
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
        && compression.sample_encoding != PREVIOUS_SAMPLE_ENCODING_TAG
        && !(column_id.is_none() && compression.sample_encoding == TRANSFER_SAMPLE_ENCODING_TAG)
        && !(column_id.is_none()
            && compression.sample_encoding == TRANSFER_SAMPLE_STREAMING_ENCODING_TAG)
        && !(column_id.is_none()
            && compression.sample_encoding == TRANSFER_SAMPLE_STREAMING_CHUNKED_ENCODING_TAG)
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
    let accepts_former_contract = blueprint.schema_version == 4;
    if accepts_former_contract {
        if let Some(inventory) = blueprint.artifact_inventory.as_mut() {
            if inventory.contract == LEGACY_ARTIFACT_CONTRACT {
                inventory.contract = crate::PREVIOUS_ARTIFACT_CONTRACT.to_string();
            }
        }
    }
    for table in blueprint.tables.values_mut() {
        normalize_compression_identifier(table.compression.as_mut(), accepts_former_contract);
        for column in table.cols.values_mut() {
            normalize_compression_identifier(column.compression.as_mut(), accepts_former_contract);
            if blueprint.schema_version < 7 && column.numeric_precision == Some(0) {
                column.numeric_precision = None;
                if column.numeric_scale == Some(0) {
                    column.numeric_scale = None;
                }
            }
        }
    }
}

fn normalize_compression_identifier(
    compression: Option<&mut crate::BlueprintCompression>,
    accepts_former_contract: bool,
) {
    if let Some(compression) = compression {
        if compression.sample_encoding == PREVIOUS_SAMPLE_ENCODING_TAG
            || (accepts_former_contract
                && compression.sample_encoding == LEGACY_SAMPLE_ENCODING_TAG)
        {
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
/// not declare the file safe for a particular sharing channel or recipient.
pub const BLUEPRINT_TOML_HEADER: &str = "\
# dbwarp-blueprint v7
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
        ArtifactComplexity, ArtifactComplexityCountDimension, ArtifactComplexityCountHistogram,
        ArtifactComplexityDimensions, ArtifactComplexitySizeDimension,
        ArtifactComplexitySizeHistogram, ArtifactInventory, BlueprintArtifact, BlueprintBundle,
        BlueprintColumn, BlueprintCompression, BlueprintIndex, BlueprintTable, BundleSource,
        FkEdge, LanguageFeatureCensus, Totals, ARTIFACT_COMPLEXITY_ASSESSOR_VERSION,
        ARTIFACT_COMPLEXITY_CONTRACT, ARTIFACT_COMPLEXITY_POPULATION_POLICY, ARTIFACT_CONTRACT,
        BUNDLE_KIND, BUNDLE_SCHEMA_VERSION, LANGUAGE_CENSUS_CONTRACT, SCHEMA_VERSION,
    };
    use serde::Deserialize;
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
            schema: "schema-A".into(),
            rows: 7,
            table_bytes: 70,
            index_bytes: 14,
            ..Default::default()
        };
        let mut blueprint = BlueprintFile {
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
        };
        blueprint.initialize_v7_database_contract();
        blueprint
    }

    fn one_table_v6_blueprint() -> BlueprintFile {
        let mut blueprint = one_table_blueprint();
        blueprint.schema_version = 6;
        blueprint.structure_scope = None;
        blueprint.source_environment = None;
        blueprint.statistics_evidence = None;
        blueprint.activity_snapshot = None;
        blueprint.artifact_inventory = None;
        if let Some(topology) = blueprint.database_topology.as_mut() {
            topology.contract = crate::PREVIOUS_TOPOLOGY_CONTRACT.to_string();
            topology.member_count_scope.clear();
            topology.catalogs_not_applicable.clear();
        }
        for table in blueprint.tables.values_mut() {
            table.object_kind.clear();
            table.storage_organization.clear();
            table.partitioning.clear();
            table.segment_state.clear();
            table.parent_table.clear();
            table.child_tables.clear();
            table.table_features.clear();
            table.statistics = None;
            for column in table.cols.values_mut() {
                column.numeric_model.clear();
                column.numeric_precision_radix.clear();
            }
        }
        blueprint
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
            scope: "all-visible-schemas".into(),
            visibility: "full".into(),
            ..Default::default()
        }
    }

    fn blueprint_with_graph_complexity() -> BlueprintFile {
        let mut blueprint = one_table_blueprint();
        let mut inventory = ArtifactInventory {
            contract: ARTIFACT_CONTRACT.into(),
            detail: "graph".into(),
            scope: "all-visible-schemas".into(),
            visibility: "full".into(),
            inventory_complete: true,
            dependencies_complete: true,
            requirements_complete: true,
            object_count: 1,
            counts_by_kind: BTreeMap::from([("function".into(), 1)]),
            complexity: None,
            artifacts: BTreeMap::from([(
                "function-001".into(),
                BlueprintArtifact {
                    kind: "function".into(),
                    subkind: "ordinary".into(),
                    tier: "programmatic".into(),
                    requirement_status: "complete".into(),
                    definition_visibility: "not_read".into(),
                    ..Default::default()
                },
            )]),
            ..Default::default()
        };
        inventory.complexity = Some(
            crate::assess_artifact_complexity(&inventory, true)
                .expect("assess graph fixture complexity"),
        );
        blueprint.artifact_inventory = Some(inventory);
        blueprint
    }

    #[test]
    fn schema_v6_requires_scope_without_inventing_it_for_v5() {
        let mut current = one_table_v6_blueprint();
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
        let unknown = one_table_v6_blueprint();
        assert!(validate_blueprint_contract(&unknown).is_ok());

        let mut complete = unknown;
        complete.database_topology = Some(crate::DatabaseTopology {
            contract: crate::PREVIOUS_TOPOLOGY_CONTRACT.into(),
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
    fn v7_initializer_preserves_never_analyzed_statistics() {
        let mut blueprint = one_table_v6_blueprint();
        blueprint.schema_version = 7;
        blueprint
            .tables
            .get_mut("table-001")
            .unwrap()
            .stats_freshness = "never_analyzed".to_string();
        blueprint.initialize_v7_database_contract();

        assert_eq!(
            blueprint.tables["table-001"]
                .statistics
                .as_ref()
                .unwrap()
                .statistics_state,
            "never-analyzed"
        );
        assert!(validate_blueprint_contract(&blueprint).is_ok());
    }

    #[test]
    fn schema_v7_requires_explicit_structure_statistics_environment_and_artifacts() {
        let current = one_table_blueprint();
        validate_blueprint_contract(&current).unwrap();

        let mut missing = current.clone();
        missing.structure_scope = None;
        assert!(validate_blueprint_contract(&missing)
            .unwrap_err()
            .to_string()
            .contains("structure_scope"));

        let mut missing = current.clone();
        missing.statistics_evidence = None;
        assert!(validate_blueprint_contract(&missing)
            .unwrap_err()
            .to_string()
            .contains("statistics_evidence"));

        let mut missing = current.clone();
        missing.source_environment = None;
        assert!(validate_blueprint_contract(&missing)
            .unwrap_err()
            .to_string()
            .contains("source_environment"));

        let mut missing = current;
        missing.artifact_inventory = None;
        assert!(validate_blueprint_contract(&missing)
            .unwrap_err()
            .to_string()
            .contains("artifact_inventory"));
    }

    #[test]
    fn schema_v7_accepts_distributed_table_size_provenance() {
        for method in ["citus-distributed-relation-size", "distributed-aggregate"] {
            let mut blueprint = one_table_blueprint();
            let table = blueprint.tables.get_mut("table-001").unwrap();
            table.statistics.as_mut().unwrap().size_method = method.into();
            assert!(
                validate_blueprint_contract(&blueprint).is_ok(),
                "table-level size method {method} must match the dataset vocabulary"
            );
        }
    }

    #[test]
    fn schema_v7_represents_empty_and_linear_key_partitioning() {
        let mut empty = one_table_blueprint();
        let table = empty.tables.get_mut("table-001").unwrap();
        table.partitioning = "range".into();
        table.partition_count = Some(0);
        table.segment_state = "unavailable".into();
        validate_blueprint_contract(&empty)
            .expect("an empty PostgreSQL partition root is a valid catalog state");

        let mut invalid = one_table_blueprint();
        let table = invalid.tables.get_mut("table-001").unwrap();
        table.partitioning = "range".into();
        table.partition_count = Some(0);
        let error = validate_blueprint_contract(&invalid)
            .expect_err("a materialized segment cannot claim zero partitions");
        assert!(error.to_string().contains("partition_count = 0"));

        let mut non_logical = one_table_blueprint();
        let table = non_logical.tables.get_mut("table-001").unwrap();
        table.object_kind = "temporary-table".into();
        table.partitioning = "range".into();
        table.partition_count = Some(0);
        table.segment_state = "unavailable".into();
        let error = validate_blueprint_contract(&non_logical)
            .expect_err("non-ordinary storage must not inherit the logical-root exception");
        assert!(error.to_string().contains("partition_count = 0"));

        let mut linear_key = one_table_blueprint();
        let table = linear_key.tables.get_mut("table-001").unwrap();
        table.partitioning = "linear-key".into();
        table.partition_count = Some(8);
        validate_blueprint_contract(&linear_key)
            .expect("MySQL LINEAR KEY must not be collapsed into LINEAR HASH");

        let mut empty_largest_partition = one_table_blueprint();
        let table = empty_largest_partition.tables.get_mut("table-001").unwrap();
        table.partitioning = "range".into();
        table.partition_count = Some(2);
        table.rows = 100;
        table.partition_rows_max = Some(0);
        empty_largest_partition.totals.row_count = 100;
        let error = validate_blueprint_contract(&empty_largest_partition)
            .expect_err("a positive partitioned table cannot have an empty largest leaf");
        assert!(error.to_string().contains("empty largest partition"));

        let mut oversized_largest_partition = one_table_blueprint();
        let table = oversized_largest_partition
            .tables
            .get_mut("table-001")
            .unwrap();
        table.partitioning = "range".into();
        table.partition_count = Some(2);
        table.rows = 100;
        table.partition_rows_max = Some(200);
        oversized_largest_partition.totals.row_count = 100;
        let error = validate_blueprint_contract(&oversized_largest_partition)
            .expect_err("a largest leaf cannot exceed its table population");
        assert!(error
            .to_string()
            .contains("exceeds its serialized row count"));
    }

    #[test]
    fn parsers_reject_invalid_v7_cardinality_evidence() {
        let mut blueprint = one_table_blueprint();
        let table = blueprint.tables.get_mut("table-001").unwrap();
        table.statistics.as_mut().unwrap().row_count_quality = "engine-estimate".into();
        table.cols.insert(
            "col-1".into(),
            crate::BlueprintColumn {
                ordinal: 1,
                column_type: "text".into(),
                numeric_model: "not-applicable".into(),
                cardinality: Some(crate::BlueprintCardinality {
                    measured: true,
                    sample_rows: 7,
                    non_null_rows: 0,
                    observed_distinct_count: 0,
                    estimated_distinct_count: 2,
                    top_value_fraction: 0.5,
                    frequency_p50: 1,
                    frequency_p95: 1,
                    frequency_p99: 1,
                    frequency_max: 1,
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        blueprint
            .statistics_evidence
            .as_mut()
            .unwrap()
            .counts_by_row_count_quality = BTreeMap::from([("engine-estimate".into(), 1)]);

        let encoded = toml::to_string_pretty(&blueprint).unwrap();
        let error = parse_blueprint_toml(&encoded)
            .expect_err("invalid schema-v7 evidence must not be rewritten on input");
        assert!(error.to_string().contains("empty non-NULL census"));

        let mut bundle = embedded_bundle();
        bundle.sources.get_mut("source-a").unwrap().blueprint = Some(blueprint);
        let encoded_bundle = toml::to_string_pretty(&bundle).unwrap();
        let error = parse_blueprint_bundle_toml(&encoded_bundle)
            .expect_err("embedded invalid schema-v7 evidence must remain invalid");
        assert!(format!("{error:#}").contains("empty non-NULL census"));
    }

    #[test]
    fn hand_authored_complexity_corpus_catches_shared_assessor_mistakes() {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct OracleFixture {
            inventory: crate::ArtifactInventory,
            expected: crate::ArtifactComplexity,
        }

        let fixtures = [
            (
                "computation_failed.toml",
                include_str!("../tests/fixtures/artifact_complexity/computation_failed.toml"),
            ),
            (
                "empty_complete.toml",
                include_str!("../tests/fixtures/artifact_complexity/empty_complete.toml"),
            ),
            (
                "empty_incomplete.toml",
                include_str!("../tests/fixtures/artifact_complexity/empty_incomplete.toml"),
            ),
            (
                "excluded_population.toml",
                include_str!("../tests/fixtures/artifact_complexity/excluded_population.toml"),
            ),
            (
                "graph_nonempty.toml",
                include_str!("../tests/fixtures/artifact_complexity/graph_nonempty.toml"),
            ),
            (
                "not_applicable_sequence.toml",
                include_str!("../tests/fixtures/artifact_complexity/not_applicable_sequence.toml"),
            ),
            (
                "partial_coverage.toml",
                include_str!("../tests/fixtures/artifact_complexity/partial_coverage.toml"),
            ),
            (
                "population_incomplete.toml",
                include_str!("../tests/fixtures/artifact_complexity/population_incomplete.toml"),
            ),
            (
                "upper_tail.toml",
                include_str!("../tests/fixtures/artifact_complexity/upper_tail.toml"),
            ),
            (
                "wholly_unknown_wrapped.toml",
                include_str!("../tests/fixtures/artifact_complexity/wholly_unknown_wrapped.toml"),
            ),
        ];

        for (name, source) in fixtures {
            let fixture: OracleFixture = toml::from_str(source)
                .unwrap_or_else(|error| panic!("hand-authored fixture {name} must parse: {error}"));
            let actual = if fixture
                .expected
                .limitations
                .iter()
                .any(|limitation| limitation == "computation-failed")
            {
                crate::failed_artifact_complexity(
                    &fixture.inventory,
                    fixture.expected.assessment_population_complete,
                )
            } else {
                crate::assess_artifact_complexity(
                    &fixture.inventory,
                    fixture.expected.assessment_population_complete,
                )
            }
            .unwrap_or_else(|error| panic!("hand-authored fixture {name} must assess: {error}"));
            assert_eq!(actual, fixture.expected, "fixture {name}");
        }
    }

    #[test]
    fn schema_v7_rejects_noncanonical_artifact_and_schema_identifiers() {
        for (identifier, prefix, width, padded) in [
            ("orders", "table", 3, true),
            ("table-01", "table", 3, true),
            ("col-email", "col", 1, false),
            ("col-01", "col", 1, false),
            ("idx-primary", "idx", 1, false),
            ("idx-0", "idx", 1, false),
        ] {
            assert!(validate_numeric_identifier(identifier, prefix, width, padded).is_err());
        }
        for (identifier, prefix, width, padded) in [
            ("table-001", "table", 3, true),
            ("table-1000", "table", 3, true),
            ("col-1", "col", 1, false),
            ("idx-1", "idx", 1, false),
        ] {
            validate_numeric_identifier(identifier, prefix, width, padded)
                .unwrap_or_else(|error| panic!("{identifier} must be canonical: {error}"));
        }

        for invalid_id in [
            "function-payroll",
            "function-000",
            "function-01",
            "function-002",
        ] {
            let mut blueprint = blueprint_with_graph_complexity();
            let inventory = blueprint.artifact_inventory.as_mut().unwrap();
            let (_, artifact) = inventory.artifacts.pop_first().unwrap();
            inventory.artifacts.insert(invalid_id.into(), artifact);
            let error = validate_blueprint_contract(&blueprint)
                .expect_err("v7 artifact identifiers must use canonical ordinals");
            assert!(error.to_string().contains("artifact id"), "{error}");
        }

        for invalid_schema in [
            "schema-production",
            "schema-a",
            "schema-",
            "schema-PRODUCTION",
            "schema-Z",
        ] {
            let mut blueprint = blueprint_with_graph_complexity();
            blueprint
                .artifact_inventory
                .as_mut()
                .unwrap()
                .artifacts
                .get_mut("function-001")
                .unwrap()
                .schema = invalid_schema.into();
            let error = validate_blueprint_contract(&blueprint)
                .expect_err("v7 schema identifiers must use anonymous alphabetic ordinals");
            assert!(error.to_string().contains("schema id"), "{error}");
        }

        let mut sparse_artifact = blueprint_with_graph_complexity();
        let inventory = sparse_artifact.artifact_inventory.as_mut().unwrap();
        let (_, artifact) = inventory.artifacts.pop_first().unwrap();
        inventory.artifacts.insert("function-1000".into(), artifact);
        let error = validate_blueprint_contract(&sparse_artifact)
            .expect_err("syntactically valid but sparse artifact ordinals must fail");
        assert!(error.to_string().contains("dense canonical"), "{error}");

        let mut sparse_table = one_table_blueprint();
        let (_, table) = sparse_table.tables.pop_first().unwrap();
        sparse_table.tables.insert("table-002".into(), table);
        let error = validate_blueprint_contract(&sparse_table)
            .expect_err("syntactically valid but sparse table ordinals must fail");
        assert!(error.to_string().contains("table ids"), "{error}");

        let mut mismatched_column = one_table_blueprint();
        mismatched_column
            .tables
            .get_mut("table-001")
            .unwrap()
            .cols
            .insert(
                "col-2".into(),
                BlueprintColumn {
                    ordinal: 1,
                    column_type: "text".into(),
                    numeric_model: "not-applicable".into(),
                    ..Default::default()
                },
            );
        let error = validate_blueprint_contract(&mismatched_column)
            .expect_err("column identifiers must agree with their natural ordinal");
        assert!(
            error.to_string().contains("does not match ordinal"),
            "{error}"
        );

        let mut sparse_index = one_table_blueprint();
        sparse_index
            .tables
            .get_mut("table-001")
            .unwrap()
            .idxs
            .insert(
                "idx-2".into(),
                BlueprintIndex {
                    expression: true,
                    ..Default::default()
                },
            );
        let error = validate_blueprint_contract(&sparse_index)
            .expect_err("syntactically valid but sparse index ordinals must fail");
        assert!(error.to_string().contains("index ids"), "{error}");
    }

    #[test]
    fn schema_v7_temporal_history_may_be_outside_selected_scope_only() {
        let mut selected = one_table_blueprint();
        selected.tables.get_mut("table-001").unwrap().table_features =
            vec!["temporal-current".into()];
        selected
            .tables
            .get_mut("table-001")
            .unwrap()
            .table_limitations = vec!["temporal-history-outside-selected-scope".into()];
        selected
            .dataset_scope
            .as_mut()
            .unwrap()
            .limitations
            .push("selection-limited".into());
        selected.dataset_scope.as_mut().unwrap().limitations.sort();
        selected.initialize_v7_database_contract();
        validate_blueprint_contract(&selected)
            .expect("a selected schema may omit its temporal history table");

        let mut capture_wide_only = selected.clone();
        capture_wide_only
            .tables
            .get_mut("table-001")
            .unwrap()
            .table_limitations
            .clear();
        let error = validate_blueprint_contract(&capture_wide_only)
            .expect_err("capture-wide selection evidence must not waive a table invariant");
        assert!(error.to_string().contains("temporal_history"));

        let mut complete = one_table_blueprint();
        complete.tables.get_mut("table-001").unwrap().table_features =
            vec!["temporal-current".into()];
        let error = validate_blueprint_contract(&complete)
            .expect_err("an all-schema capture must link its temporal history table");
        assert!(error.to_string().contains("temporal_history"));

        let mut contradictory = selected;
        contradictory
            .tables
            .get_mut("table-001")
            .unwrap()
            .table_limitations
            .push("temporal-history-visibility-unknown".into());
        let error = validate_blueprint_contract(&contradictory)
            .expect_err("temporal absence evidence must have exactly one classification");
        assert!(error
            .to_string()
            .contains("both outside scope and visibility unknown"));
    }

    #[test]
    fn schema_v7_relationship_target_may_be_outside_selected_scope_only() {
        let mut selected = one_table_blueprint();
        selected
            .tables
            .get_mut("table-001")
            .unwrap()
            .table_limitations = vec!["relationship-target-outside-selected-scope".into()];
        selected
            .dataset_scope
            .as_mut()
            .unwrap()
            .limitations
            .push("selection-limited".into());
        selected.dataset_scope.as_mut().unwrap().limitations.sort();
        selected.initialize_v7_database_contract();
        validate_blueprint_contract(&selected)
            .expect("a selected schema may reference a relationship target outside scope");

        selected
            .dataset_scope
            .as_mut()
            .unwrap()
            .limitations
            .retain(|value| value != "selection-limited");
        let error = validate_blueprint_contract(&selected)
            .expect_err("an all-schema capture cannot omit a relationship target");
        assert!(error.to_string().contains("without a selected capture"));
    }

    #[test]
    fn schema_v7_relationship_visibility_unknown_requires_incomplete_inventory() {
        let mut blueprint = one_table_blueprint();
        let structure = blueprint.structure_scope.as_mut().unwrap();
        structure.relationship_inventory_completeness = "incomplete".into();
        structure.limitations = vec!["relationship-inventory-unavailable".into()];
        blueprint
            .tables
            .get_mut("table-001")
            .unwrap()
            .table_limitations = vec!["relationship-target-visibility-unknown".into()];
        validate_blueprint_contract(&blueprint)
            .expect("unknown structure visibility may explain an unresolved relationship");

        blueprint
            .structure_scope
            .as_mut()
            .unwrap()
            .relationship_inventory_completeness = "complete".into();
        let error = validate_blueprint_contract(&blueprint)
            .expect_err("complete relationship inventory contradicts an unresolved target");
        assert!(error
            .to_string()
            .contains("relationship inventory is complete"));
    }

    #[test]
    fn schema_v7_per_table_dependent_limitations_match_the_affected_family() {
        let mut index_gap = one_table_blueprint();
        index_gap
            .structure_scope
            .as_mut()
            .unwrap()
            .index_inventory_completeness = "incomplete".into();
        index_gap.structure_scope.as_mut().unwrap().limitations =
            vec!["index-inventory-unavailable".into()];
        index_gap
            .tables
            .get_mut("table-001")
            .unwrap()
            .table_limitations = vec!["index-inventory-unavailable".into()];
        validate_blueprint_contract(&index_gap)
            .expect("an index-only gap must not withdraw column or relationship coverage");

        let mut wrong_family = index_gap.clone();
        wrong_family
            .structure_scope
            .as_mut()
            .unwrap()
            .index_inventory_completeness = "complete".into();
        let error = validate_blueprint_contract(&wrong_family)
            .expect_err("an index gap cannot accompany complete index inventory");
        assert!(error.to_string().contains("index inventory is complete"));

        let mut generic = one_table_blueprint();
        let scope = generic.structure_scope.as_mut().unwrap();
        scope.index_inventory_completeness = "incomplete".into();
        scope.relationship_inventory_completeness = "complete".into();
        scope.limitations = vec!["dependent-structure-suppressed".into()];
        generic
            .tables
            .get_mut("table-001")
            .unwrap()
            .table_limitations = vec!["dependent-structure-suppressed".into()];
        let error = validate_blueprint_contract(&generic)
            .expect_err("generic dependent suppression requires both families incomplete");
        assert!(error
            .to_string()
            .contains("dependent inventories are complete"));
    }

    #[test]
    fn schema_v7_accepts_per_table_row_security_visibility_gap() {
        let mut blueprint = one_table_blueprint();
        blueprint
            .tables
            .get_mut("table-001")
            .unwrap()
            .table_limitations = vec!["row-security-visibility-unknown".into()];
        validate_blueprint_contract(&blueprint)
            .expect("a table may record that row-security metadata was not visible");
    }

    #[test]
    fn schema_v7_records_a_proven_active_row_security_filter() {
        let mut blueprint = one_table_blueprint();
        blueprint
            .tables
            .get_mut("table-001")
            .unwrap()
            .table_limitations = vec!["row-security-filter-active".into()];
        validate_blueprint_contract(&blueprint)
            .expect("a table may record a proven active row-security filter");
    }

    #[test]
    fn schema_v7_inventory_visibility_limitation_withdraws_completeness() {
        let mut blueprint = one_table_blueprint();
        let scope = blueprint.dataset_scope.as_mut().unwrap();
        scope
            .limitations
            .push("table-inventory-visibility-unknown".into());
        scope.limitations.sort();
        validate_blueprint_contract(&blueprint)
            .expect("unknown table inventory may carry visibility evidence");

        blueprint
            .dataset_scope
            .as_mut()
            .unwrap()
            .table_inventory_completeness = "complete".into();
        let error = validate_blueprint_contract(&blueprint)
            .expect_err("complete table inventory contradicts unknown visibility");
        assert!(error
            .to_string()
            .contains("cannot accompany complete table inventory"));
    }

    #[test]
    fn schema_v7_exact_read_provenance_is_atomic() {
        let mut blueprint = one_table_blueprint();
        let evidence = blueprint
            .tables
            .get_mut("table-001")
            .unwrap()
            .statistics
            .as_mut()
            .unwrap();
        evidence.row_count_method = "bounded-complete-read".into();
        evidence.row_count_quality = "exact-read".into();
        evidence.sample_fraction_band = "full".into();
        blueprint
            .statistics_evidence
            .as_mut()
            .unwrap()
            .counts_by_row_count_quality = BTreeMap::from([("exact-read".into(), 1)]);
        validate_blueprint_contract(&blueprint)
            .expect("a proven complete bounded read is valid exact row evidence");

        blueprint
            .tables
            .get_mut("table-001")
            .unwrap()
            .statistics
            .as_mut()
            .unwrap()
            .sample_fraction_band = "75-99pct".into();
        let error = validate_blueprint_contract(&blueprint)
            .expect_err("exact-read cannot describe a partial sample");
        assert!(error.to_string().contains("requires a full sample"));
    }

    #[test]
    fn schema_v7_oracle_exact_segment_bytes_require_attributed_segment_state() {
        let mut blueprint = one_table_blueprint();
        let table = blueprint.tables.get_mut("table-001").unwrap();
        table.segment_state = "unavailable".into();
        let evidence = table.statistics.as_mut().unwrap();
        evidence.size_method = "oracle-segment-bytes".into();
        evidence.size_quality = "exact-counter".into();
        evidence.size_scope = "table-only".into();
        evidence.size_accounting = "allocated-segment".into();
        evidence.size_visibility = "partial".into();
        blueprint
            .statistics_evidence
            .as_mut()
            .unwrap()
            .counts_by_size_quality = BTreeMap::from([("exact-counter".into(), 1)]);

        let error = validate_blueprint_contract(&blueprint)
            .expect_err("unattributed Oracle segment bytes cannot be an exact counter");
        assert!(error.to_string().contains("attributed exact-counter"));

        blueprint.tables.get_mut("table-001").unwrap().segment_state = "created".into();
        validate_blueprint_contract(&blueprint)
            .expect("attributed Oracle segment bytes may retain partial visibility");

        let table = blueprint.tables.get_mut("table-001").unwrap();
        table.table_bytes = 0;
        table.index_bytes = 0;
        blueprint.totals.table_bytes = 0;
        blueprint.totals.index_bytes = 0;
        validate_blueprint_contract(&blueprint).expect(
            "a positive raw segment counter may round below the first serialized byte bucket",
        );

        blueprint.tables.get_mut("table-001").unwrap().segment_state = "mixed".into();
        validate_blueprint_contract(&blueprint)
            .expect("mixed Oracle evidence may preserve a rounded-away index allocation");
        let table = blueprint.tables.get_mut("table-001").unwrap();
        table.index_bytes = 1024;
        blueprint.totals.index_bytes = 1024;
        let error = validate_blueprint_contract(&blueprint)
            .expect_err("mixed Oracle evidence is reserved for a rounded-zero index allocation");
        assert!(error.to_string().contains("rounded-zero index allocation"));
        let table = blueprint.tables.get_mut("table-001").unwrap();
        table.index_bytes = 0;
        blueprint.totals.index_bytes = 0;

        blueprint.tables.get_mut("table-001").unwrap().segment_state =
            "mixed-table-and-index".into();
        validate_blueprint_contract(&blueprint)
            .expect("mixed Oracle evidence may preserve rounded-away table and index allocations");
        let table = blueprint.tables.get_mut("table-001").unwrap();
        table.table_bytes = 1024;
        blueprint.totals.table_bytes = 1024;
        let error = validate_blueprint_contract(&blueprint)
            .expect_err("mixed table/index evidence is reserved for two rounded-zero allocations");
        assert!(error
            .to_string()
            .contains("rounded-zero table and index allocations"));
        let table = blueprint.tables.get_mut("table-001").unwrap();
        table.table_bytes = 0;
        blueprint.totals.table_bytes = 0;

        blueprint.tables.get_mut("table-001").unwrap().segment_state = "deferred".into();
        validate_blueprint_contract(&blueprint)
            .expect("a complete segment census can prove deferred zero-byte storage");

        let table = blueprint.tables.get_mut("table-001").unwrap();
        table.table_bytes = 1024;
        blueprint.totals.table_bytes = 1024;
        let error = validate_blueprint_contract(&blueprint)
            .expect_err("deferred storage cannot carry attributed bytes");
        assert!(error.to_string().contains("cannot carry attributed bytes"));
    }

    #[test]
    fn schema_v7_oracle_logical_estimate_requires_labelled_partial_provenance() {
        let mut blueprint = one_table_blueprint();
        let table = blueprint.tables.get_mut("table-001").unwrap();
        table.segment_state = "unavailable".into();
        let evidence = table.statistics.as_mut().unwrap();
        evidence.size_method = "oracle-table-logical-estimate".into();
        evidence.size_quality = "engine-estimate".into();
        evidence.size_scope = "table-only".into();
        evidence.size_accounting = "logical-estimate".into();
        evidence.size_visibility = "partial".into();
        blueprint
            .statistics_evidence
            .as_mut()
            .unwrap()
            .counts_by_size_quality = BTreeMap::from([("engine-estimate".into(), 1)]);
        validate_blueprint_contract(&blueprint)
            .expect("Oracle logical sizing may be emitted only as a partial estimate");

        blueprint
            .tables
            .get_mut("table-001")
            .unwrap()
            .statistics
            .as_mut()
            .unwrap()
            .size_quality = "exact-counter".into();
        blueprint
            .statistics_evidence
            .as_mut()
            .unwrap()
            .counts_by_size_quality = BTreeMap::from([("exact-counter".into(), 1)]);
        let error = validate_blueprint_contract(&blueprint)
            .expect_err("a logical estimate cannot masquerade as an exact counter");
        assert!(error.to_string().contains("partial estimate provenance"));
    }

    #[test]
    fn schema_v7_not_applicable_oracle_totals_require_an_excluded_only_population() {
        let mut blueprint = one_table_blueprint();
        blueprint.engine = "oracle".into();
        blueprint.structure_scope = Some(crate::StructureScope::complete_database("oracle"));
        let table = blueprint.tables.get_mut("table-001").unwrap();
        table.object_kind = "materialized-view".into();
        table.counted_in_totals = Some(false);
        blueprint.totals = crate::Totals::default();
        let scope = blueprint.dataset_scope.as_mut().unwrap();
        scope.layout = "full-copy".into();
        scope.table_inventory_completeness = "complete".into();
        scope.row_count_completeness = "complete".into();
        scope.size_completeness = "complete".into();
        scope.row_count_method = "not-applicable".into();
        scope.size_method = "not-applicable".into();
        validate_blueprint_contract(&blueprint).expect(
            "a non-empty excluded-only Oracle population has no aggregate measurement method",
        );

        blueprint.statistics_evidence.as_mut().unwrap().visibility = "full".into();
        let error = validate_blueprint_contract(&blueprint)
            .expect_err("an excluded-only population cannot earn full visibility vacuously");
        assert!(error
            .to_string()
            .contains("full statistics visibility requires a non-empty counted population"));
        blueprint.statistics_evidence.as_mut().unwrap().visibility = "unknown".into();

        blueprint.dataset_scope.as_mut().unwrap().size_completeness = "incomplete".into();
        let error = validate_blueprint_contract(&blueprint)
            .expect_err("not-applicable cannot describe incomplete coverage");
        assert!(error.to_string().contains("empty copy-total population"));

        let scope = blueprint.dataset_scope.as_mut().unwrap();
        scope.size_completeness = "complete".into();
        scope.row_count_method = "oracle-table-statistics".into();
        let error = validate_blueprint_contract(&blueprint)
            .expect_err("the row and size methods cannot disagree about an empty population");
        assert!(error.to_string().contains("row and size methods"));
    }

    #[test]
    fn schema_v7_unavailable_table_evidence_withdraws_complete_totals() {
        let mut blueprint = one_table_blueprint();
        let scope = blueprint.dataset_scope.as_mut().unwrap();
        scope.layout = "full-copy".into();
        scope.row_count_completeness = "complete".into();
        scope.size_completeness = "complete".into();
        let table = blueprint.tables.get_mut("table-001").unwrap();
        let evidence = table.statistics.as_mut().unwrap();
        evidence.row_count_quality = "unavailable".into();
        evidence.size_quality = "unavailable".into();
        let aggregate = blueprint.statistics_evidence.as_mut().unwrap();
        aggregate.counts_by_row_count_quality = BTreeMap::from([("unavailable".into(), 1)]);
        aggregate.counts_by_size_quality = BTreeMap::from([("unavailable".into(), 1)]);

        let error = validate_blueprint_contract(&blueprint)
            .expect_err("a numeric zero cannot repair unavailable included evidence");
        assert!(error
            .to_string()
            .contains("complete row-count coverage cannot include"));
    }

    #[test]
    fn schema_v7_full_statistics_visibility_rejects_table_evidence_gaps() {
        let mut blueprint = one_table_blueprint();
        blueprint.statistics_evidence.as_mut().unwrap().visibility = "full".into();

        let error = validate_blueprint_contract(&blueprint)
            .expect_err("full aggregate visibility cannot hide unknown table evidence");
        assert!(error
            .to_string()
            .contains("full statistics visibility cannot include counted table evidence gaps"));
    }

    #[test]
    fn schema_v7_full_statistics_visibility_rejects_unclassified_excluded_table() {
        let mut blueprint = one_table_blueprint();
        let counted = blueprint.tables.get_mut("table-001").unwrap();
        let counted_evidence = counted.statistics.as_mut().unwrap();
        counted_evidence.statistics_state = "current".into();
        counted_evidence.row_count_quality = "engine-estimate".into();
        counted_evidence.size_quality = "engine-counter".into();
        counted_evidence.size_visibility = "full".into();

        let mut excluded = counted.clone();
        excluded.object_kind = "materialized-view".into();
        excluded.counted_in_totals = Some(false);
        let excluded_evidence = excluded.statistics.as_mut().unwrap();
        excluded_evidence.statistics_state = "unknown".into();
        excluded_evidence.row_count_quality = "unavailable".into();
        excluded_evidence.size_quality = "unavailable".into();
        excluded_evidence.size_visibility = "partial".into();
        blueprint.tables.insert("table-002".into(), excluded);

        let aggregate = blueprint.statistics_evidence.as_mut().unwrap();
        aggregate.visibility = "full".into();
        aggregate.table_count = 2;
        aggregate.counts_by_statistics_state =
            BTreeMap::from([("current".into(), 1), ("unknown".into(), 1)]);
        aggregate.counts_by_row_count_quality =
            BTreeMap::from([("engine-estimate".into(), 1), ("unavailable".into(), 1)]);
        aggregate.counts_by_size_quality =
            BTreeMap::from([("engine-counter".into(), 1), ("unavailable".into(), 1)]);

        let error = validate_blueprint_contract(&blueprint)
            .expect_err("excluded policy cannot hide an unclassified statistics state");
        assert!(error
            .to_string()
            .contains("full statistics visibility cannot include unclassified table statistics"));
    }

    #[test]
    fn schema_v7_full_statistics_visibility_rejects_unreadable_catalogs_specifically() {
        let mut blueprint = one_table_blueprint();
        let table = blueprint.tables.get_mut("table-001").unwrap();
        let table_evidence = table.statistics.as_mut().unwrap();
        table_evidence.statistics_state = "current".into();
        table_evidence.row_count_quality = "engine-estimate".into();
        table_evidence.size_quality = "engine-counter".into();
        table_evidence.size_visibility = "full".into();
        let aggregate = blueprint.statistics_evidence.as_mut().unwrap();
        aggregate.visibility = "full".into();
        aggregate.counts_by_statistics_state = BTreeMap::from([("current".into(), 1)]);
        aggregate.counts_by_row_count_quality = BTreeMap::from([("engine-estimate".into(), 1)]);
        aggregate.counts_by_size_quality = BTreeMap::from([("engine-counter".into(), 1)]);
        aggregate.catalogs_unreadable = vec!["oracle-segments".into()];

        let error = validate_blueprint_contract(&blueprint)
            .expect_err("full visibility cannot coexist with an unreadable catalog");
        assert!(error
            .to_string()
            .contains("full statistics visibility cannot include unreadable catalogs"));
    }

    #[test]
    fn schema_v7_excludes_unmeasured_memory_optimized_tables_from_totals() {
        let mut blueprint = one_table_blueprint();
        let table = blueprint.tables.get_mut("table-001").unwrap();
        table.table_features = vec!["memory-optimized".into()];
        table.counted_in_totals = Some(false);
        blueprint.totals = crate::Totals::default();

        validate_blueprint_contract(&blueprint)
            .expect("an unmeasured memory-optimized table remains inventoried outside totals");

        blueprint
            .tables
            .get_mut("table-001")
            .unwrap()
            .counted_in_totals = None;
        let table = &blueprint.tables["table-001"];
        blueprint.totals = crate::Totals {
            table_count: 1,
            row_count: table.rows,
            table_bytes: table.table_bytes,
            index_bytes: table.index_bytes,
        };
        let error = validate_blueprint_contract(&blueprint)
            .expect_err("unmeasured memory-optimized data cannot enter aggregate totals");
        assert!(error
            .to_string()
            .contains("must set counted_in_totals = false"));
    }

    #[test]
    fn schema_v7_excludes_temporary_tables_from_copy_totals() {
        let mut blueprint = one_table_blueprint();
        let table = blueprint.tables.get_mut("table-001").unwrap();
        table.object_kind = "temporary-table".into();
        table.rows = 0;
        table.table_bytes = 0;
        table.index_bytes = 0;
        table.counted_in_totals = Some(false);
        blueprint.totals = crate::Totals::default();

        validate_blueprint_contract(&blueprint)
            .expect("session-scoped temporary data is inventoried outside copy totals");

        blueprint
            .tables
            .get_mut("table-001")
            .unwrap()
            .counted_in_totals = None;
        blueprint.totals.table_count = 1;
        let error = validate_blueprint_contract(&blueprint)
            .expect_err("temporary data cannot enter aggregate copy totals");
        assert!(error
            .to_string()
            .contains("must set counted_in_totals = false"));
    }

    #[test]
    fn schema_v7_round_trips_bounded_source_environment_and_activity() {
        let mut blueprint = one_table_blueprint();
        blueprint.source_environment = Some(crate::SourceEnvironment {
            contract: crate::SOURCE_ENVIRONMENT_CONTRACT.into(),
            evidence_origin: "database-endpoint".into(),
            hosting_model: "managed-service".into(),
            infrastructure_location: "cloud".into(),
            capacity_scope: "database-resource".into(),
            capacity_visibility: "full".into(),
            cpu_capacity_band: "5-8".into(),
            cpu_capacity_basis: "database-resource-limit".into(),
            memory_capacity_band: "16-32-gib".into(),
            memory_capacity_basis: "database-resource-limit".into(),
            collector_machine_excluded: true,
            member_capacity_uniform: Some(true),
            features: vec!["autoscaling".into()],
            limitations: vec![
                "oracle-client-version-below-tested-floor".into(),
                "oracle-client-version-unreadable".into(),
            ],
            catalogs_read: vec!["oracle-capacity-parameters".into()],
            ..Default::default()
        });
        blueprint.activity_snapshot = Some(crate::ActivitySnapshot {
            contract: crate::ACTIVITY_SNAPSHOT_CONTRACT.into(),
            evidence_origin: "database-endpoint".into(),
            scope: "database-resource".into(),
            observation_window_band: "5-30s".into(),
            active_connections_band: "21-100".into(),
            transaction_rate_band: "101-1000".into(),
            write_rate_band: "11-100".into(),
            log_generation_rate_band: "1001-10000".into(),
            cpu_pressure_band: "moderate".into(),
            cache_pressure_band: "low".into(),
            reset_semantics: "interval-delta".into(),
            catalogs_read: vec!["oracle-sysstat".into()],
            catalogs_not_applicable: vec!["oracle-system-event".into()],
            ..Default::default()
        });
        validate_blueprint_contract(&blueprint).unwrap();

        let encoded = blueprint_to_toml(&blueprint).unwrap();
        let decoded = parse_blueprint_toml(&encoded).unwrap();
        let decoded_environment = decoded.source_environment.unwrap();
        assert_eq!(
            decoded_environment.memory_capacity_basis,
            "database-resource-limit"
        );
        assert_eq!(
            decoded_environment.limitations,
            [
                "oracle-client-version-below-tested-floor",
                "oracle-client-version-unreadable"
            ]
        );
        assert_eq!(
            decoded.activity_snapshot.unwrap().active_connections_band,
            "21-100"
        );

        let mut invalid = blueprint;
        invalid
            .activity_snapshot
            .as_mut()
            .unwrap()
            .active_connections_band = "under-1".into();
        assert!(validate_blueprint_contract(&invalid).is_err());

        let mut invalid_limitation = one_table_blueprint();
        invalid_limitation.source_environment = Some(crate::SourceEnvironment {
            contract: crate::SOURCE_ENVIRONMENT_CONTRACT.into(),
            evidence_origin: "none".into(),
            hosting_model: "unknown".into(),
            infrastructure_location: "unknown".into(),
            capacity_scope: "unknown".into(),
            capacity_visibility: "not-requested".into(),
            cpu_capacity_band: "unknown".into(),
            cpu_capacity_basis: "unknown".into(),
            memory_capacity_band: "unknown".into(),
            memory_capacity_basis: "unknown".into(),
            collector_machine_excluded: true,
            limitations: vec!["unbounded-client-label".into()],
            ..Default::default()
        });
        assert!(validate_blueprint_contract(&invalid_limitation).is_err());
    }

    #[test]
    fn schema_v7_initializer_preserves_engine_source_environment_evidence() {
        let mut blueprint = one_table_blueprint();
        blueprint.source_environment = Some(crate::SourceEnvironment {
            contract: crate::SOURCE_ENVIRONMENT_CONTRACT.into(),
            evidence_origin: "database-endpoint".into(),
            hosting_model: "unknown".into(),
            infrastructure_location: "unknown".into(),
            capacity_scope: "connected-instance".into(),
            capacity_visibility: "partial".into(),
            cpu_capacity_band: "unknown".into(),
            cpu_capacity_basis: "unknown".into(),
            memory_capacity_band: "2-4-gib".into(),
            memory_capacity_basis: "database-buffer-cache".into(),
            collector_machine_excluded: true,
            catalogs_read: vec!["pg-capacity-settings".into()],
            ..Default::default()
        });

        blueprint.initialize_v7_database_contract();

        let environment = blueprint.source_environment.unwrap();
        assert_eq!(environment.evidence_origin, "database-endpoint");
        assert_eq!(environment.memory_capacity_band, "2-4-gib");
        assert_eq!(environment.catalogs_read, ["pg-capacity-settings"]);
    }

    #[test]
    fn schema_v7_not_requested_capacity_may_retain_non_capacity_classification() {
        let mut blueprint = one_table_blueprint();
        blueprint.source_environment = Some(crate::SourceEnvironment {
            contract: crate::SOURCE_ENVIRONMENT_CONTRACT.into(),
            evidence_origin: "database-endpoint".into(),
            hosting_model: "self-managed".into(),
            infrastructure_location: "unknown".into(),
            capacity_scope: "unknown".into(),
            capacity_visibility: "not-requested".into(),
            cpu_capacity_band: "unknown".into(),
            cpu_capacity_basis: "unknown".into(),
            memory_capacity_band: "unknown".into(),
            memory_capacity_basis: "unknown".into(),
            collector_machine_excluded: true,
            catalogs_read: vec!["sqlserver-engine-edition".into()],
            ..Default::default()
        });

        validate_blueprint_contract(&blueprint).unwrap();

        blueprint
            .source_environment
            .as_mut()
            .unwrap()
            .catalogs_read
            .push("sqlserver-os-sys-info".into());
        assert!(validate_blueprint_contract(&blueprint).is_err());
    }

    #[test]
    fn schema_v7_propagates_selected_schema_scope_to_structure_and_artifacts() {
        let mut blueprint = one_table_blueprint();
        blueprint
            .dataset_scope
            .as_mut()
            .unwrap()
            .limitations
            .push("selection-limited".into());
        blueprint.dataset_scope.as_mut().unwrap().limitations.sort();
        blueprint.initialize_v7_database_contract();

        assert!(blueprint
            .structure_scope
            .as_ref()
            .unwrap()
            .limitations
            .contains(&"selection-limited".to_string()));
        assert_eq!(
            blueprint.artifact_inventory.as_ref().unwrap().scope,
            "selected-schemas"
        );
        assert!(validate_blueprint_contract(&blueprint).is_ok());

        blueprint.artifact_inventory.as_mut().unwrap().scope = "all-visible-schemas".into();
        assert!(validate_blueprint_contract(&blueprint)
            .unwrap_err()
            .to_string()
            .contains("schema-selection evidence"));
    }

    #[test]
    fn schema_v7_round_trips_oracle_numeric_declarations_without_zero_sentinels() {
        let mut blueprint = one_table_blueprint();
        blueprint.engine = "oracle".into();
        blueprint.engine_version = "21.3.0.0.0".into();
        blueprint.database_topology = Some(crate::DatabaseTopology::unknown());
        blueprint.dataset_scope = Some(crate::DatasetScope::unknown_database(
            "oracle-table-statistics",
            "oracle-segment-bytes",
        ));
        blueprint.initialize_v7_database_contract();
        let table = blueprint.tables.get_mut("table-001").unwrap();
        table.segment_state = "created".into();
        let statistics = table.statistics.as_mut().unwrap();
        statistics.size_method = "oracle-segment-bytes".into();
        statistics.size_quality = "exact-counter".into();
        statistics.size_scope = "table-only".into();
        statistics.size_accounting = "allocated-segment".into();
        statistics.size_visibility = "full".into();
        table.cols = BTreeMap::from([
            (
                "col-1".into(),
                BlueprintColumn {
                    ordinal: 1,
                    column_type: "numeric".into(),
                    numeric_model: "unconstrained-decimal".into(),
                    numeric_precision_radix: "decimal".into(),
                    ..Default::default()
                },
            ),
            (
                "col-2".into(),
                BlueprintColumn {
                    ordinal: 2,
                    column_type: "numeric".into(),
                    numeric_model: "fixed-decimal".into(),
                    numeric_precision: Some(9),
                    numeric_scale: Some(-2),
                    numeric_precision_radix: "decimal".into(),
                    ..Default::default()
                },
            ),
            (
                "col-3".into(),
                BlueprintColumn {
                    ordinal: 3,
                    column_type: "numeric".into(),
                    numeric_model: "fixed-decimal".into(),
                    numeric_precision: Some(2),
                    numeric_scale: Some(5),
                    numeric_precision_radix: "decimal".into(),
                    ..Default::default()
                },
            ),
            (
                "col-4".into(),
                BlueprintColumn {
                    ordinal: 4,
                    column_type: "numeric".into(),
                    numeric_model: "decimal-float".into(),
                    numeric_precision: Some(126),
                    numeric_precision_radix: "binary".into(),
                    ..Default::default()
                },
            ),
            (
                "col-5".into(),
                BlueprintColumn {
                    ordinal: 5,
                    column_type: "float".into(),
                    numeric_model: "binary-float".into(),
                    numeric_precision_radix: "binary".into(),
                    bit_width: 32,
                    ..Default::default()
                },
            ),
            (
                "col-6".into(),
                BlueprintColumn {
                    ordinal: 6,
                    column_type: "double".into(),
                    numeric_model: "binary-float".into(),
                    numeric_precision_radix: "binary".into(),
                    bit_width: 64,
                    ..Default::default()
                },
            ),
        ]);
        blueprint
            .statistics_evidence
            .as_mut()
            .unwrap()
            .counts_by_size_quality = BTreeMap::from([("exact-counter".into(), 1)]);
        let encoded = blueprint_to_toml(&blueprint).unwrap();
        let decoded = parse_blueprint_toml(&encoded).unwrap();
        assert_eq!(
            decoded.tables["table-001"].cols["col-1"].numeric_precision,
            None
        );
        assert_eq!(
            decoded.tables["table-001"].cols["col-2"].numeric_scale,
            Some(-2)
        );
        assert_eq!(
            decoded.tables["table-001"].cols["col-3"].numeric_scale,
            Some(5)
        );
        assert_eq!(
            decoded.tables["table-001"].cols["col-4"].numeric_model,
            "decimal-float"
        );
        assert!(!encoded.contains("numeric_precision = 0"));
        assert!(!encoded.contains("numeric_scale = 0"));

        let mut invalid = blueprint.clone();
        invalid
            .tables
            .get_mut("table-001")
            .unwrap()
            .cols
            .get_mut("col-2")
            .unwrap()
            .numeric_scale = Some(-85);
        assert!(validate_blueprint_contract(&invalid).is_err());
        let mut invalid = blueprint;
        invalid
            .tables
            .get_mut("table-001")
            .unwrap()
            .cols
            .get_mut("col-4")
            .unwrap()
            .numeric_precision = Some(127);
        assert!(validate_blueprint_contract(&invalid).is_err());
    }

    #[test]
    fn schema_v7_numeric_scale_validation_is_engine_aware() {
        let mut postgres = one_table_blueprint();
        postgres.tables.get_mut("table-001").unwrap().cols.insert(
            "col-1".into(),
            BlueprintColumn {
                ordinal: 1,
                column_type: "numeric(1000,-1000)".into(),
                numeric_model: "fixed-decimal".into(),
                numeric_precision: Some(1_000),
                numeric_scale: Some(-1_000),
                numeric_precision_radix: "decimal".into(),
                ..Default::default()
            },
        );
        postgres.initialize_v7_database_contract();
        assert!(validate_blueprint_contract(&postgres).is_ok());

        let mut mysql = postgres;
        mysql.engine = "mysql".into();
        mysql
            .tables
            .get_mut("table-001")
            .unwrap()
            .cols
            .get_mut("col-1")
            .unwrap()
            .numeric_scale = Some(-1);
        assert!(validate_blueprint_contract(&mysql).is_err());

        let mut oracle = one_table_blueprint();
        oracle.engine = "oracle".into();
        oracle.tables.get_mut("table-001").unwrap().cols.insert(
            "col-1".into(),
            BlueprintColumn {
                ordinal: 1,
                column_type: "numeric(39,0)".into(),
                numeric_model: "fixed-decimal".into(),
                numeric_precision: Some(39),
                numeric_scale: Some(0),
                numeric_precision_radix: "decimal".into(),
                ..Default::default()
            },
        );
        oracle.initialize_v7_database_contract();
        assert!(validate_blueprint_contract(&oracle).is_err());

        let mut sqlserver = oracle;
        sqlserver.engine = "sqlserver".into();
        assert!(validate_blueprint_contract(&sqlserver).is_err());
    }

    #[test]
    fn schema_v7_keeps_oracle_column_lob_and_index_semantics_orthogonal() {
        let mut blueprint = one_table_blueprint();
        blueprint.engine = "oracle".into();
        let table = blueprint.tables.get_mut("table-001").unwrap();
        table.cols.insert(
            "col-1".into(),
            BlueprintColumn {
                ordinal: 1,
                column_type: "blob".into(),
                numeric_model: "not-applicable".into(),
                has_default: Some(true),
                default_kind: "expression".into(),
                default_on_null: Some(true),
                hidden: Some(false),
                invisible: Some(true),
                length_semantics: "not-applicable".into(),
                lob_storage: Some(crate::LobStorageEvidence {
                    storage_class: "securefile".into(),
                    compression: "medium".into(),
                    deduplication: "enabled".into(),
                    in_row: Some(false),
                    encrypted: Some(true),
                    visibility: "full".into(),
                }),
                ..Default::default()
            },
        );
        table.cols.insert(
            "col-2".into(),
            BlueprintColumn {
                ordinal: 2,
                column_type: "bfile".into(),
                numeric_model: "not-applicable".into(),
                lob_storage: Some(crate::LobStorageEvidence {
                    storage_class: "external".into(),
                    compression: "not-applicable".into(),
                    deduplication: "not-applicable".into(),
                    visibility: "full".into(),
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        table.idxs.insert(
            "idx-1".into(),
            BlueprintIndex {
                index_type: "domain".into(),
                cols: vec![1],
                partitioning: "local".into(),
                visibility: "invisible".into(),
                state: "usable".into(),
                ..Default::default()
            },
        );
        blueprint.initialize_v7_database_contract();
        validate_blueprint_contract(&blueprint).unwrap();

        let mut older = blueprint;
        older.schema_version = 6;
        assert!(validate_blueprint_contract(&older).is_err());
    }

    #[test]
    fn schema_v7_artifact_graph_preserves_oracle_container_and_requirement_evidence() {
        let mut blueprint = one_table_blueprint();
        blueprint.engine = "oracle".into();
        blueprint.initialize_v7_database_contract();
        let mut inventory = ArtifactInventory {
            contract: ARTIFACT_CONTRACT.into(),
            detail: "graph".into(),
            scope: "all-visible-schemas".into(),
            visibility: "full".into(),
            inventory_complete: true,
            dependencies_complete: true,
            requirements_complete: true,
            object_count: 3,
            dependency_edge_count: 2,
            counts_by_kind: BTreeMap::from([("package".into(), 2), ("procedure".into(), 1)]),
            catalogs_read: vec!["oracle.dependencies".into(), "oracle.objects".into()],
            artifacts: BTreeMap::from([
                (
                    "package-001".into(),
                    BlueprintArtifact {
                        kind: "package".into(),
                        subkind: "specification".into(),
                        tier: "programmatic".into(),
                        requirement_status: "complete".into(),
                        definition_visibility: "not_read".into(),
                        validity: "valid".into(),
                        generated_by_engine: Some(false),
                        temporary: Some(false),
                        editioned: Some(true),
                        secondary_object: Some(false),
                        relationships: vec![crate::ArtifactRelationship {
                            kind: "implemented-by".into(),
                            target: "package-002".into(),
                            evidence: "catalog-confirmed".into(),
                        }],
                        ..Default::default()
                    },
                ),
                (
                    "package-002".into(),
                    BlueprintArtifact {
                        kind: "package".into(),
                        subkind: "body".into(),
                        tier: "programmatic".into(),
                        requirement_status: "complete".into(),
                        parent: "package-001".into(),
                        definition_visibility: "not_read".into(),
                        validity: "valid".into(),
                        generated_by_engine: Some(false),
                        temporary: Some(false),
                        editioned: Some(true),
                        secondary_object: Some(false),
                        ..Default::default()
                    },
                ),
                (
                    "procedure-001".into(),
                    BlueprintArtifact {
                        kind: "procedure".into(),
                        subkind: "package_member".into(),
                        tier: "programmatic".into(),
                        requirement_status: "complete".into(),
                        parent: "package-001".into(),
                        relationships: vec![crate::ArtifactRelationship {
                            kind: "references-table".into(),
                            target: "table-001".into(),
                            evidence: "dependency-confirmed".into(),
                        }],
                        requirements: vec![crate::ArtifactRequirement {
                            token: "oracle.routine.pipelined".into(),
                            evidence: "catalog-confirmed".into(),
                            count_band: "1".into(),
                        }],
                        definition_visibility: "not_read".into(),
                        security_mode: "invoker".into(),
                        validity: "valid".into(),
                        enabled: Some(true),
                        ..Default::default()
                    },
                ),
            ]),
            ..Default::default()
        };
        inventory.complexity = Some(
            crate::assess_artifact_complexity(&inventory, true)
                .expect("assess Oracle graph fixture"),
        );
        blueprint.artifact_inventory = Some(inventory);
        validate_blueprint_contract(&blueprint).unwrap();
        let decoded = parse_blueprint_toml(&blueprint_to_toml(&blueprint).unwrap()).unwrap();
        assert_eq!(decoded.artifact_inventory.unwrap().dependency_edge_count, 2);
    }

    #[test]
    fn schema_v7_artifact_complexity_round_trips_fixed_one_dimensional_records() {
        let blueprint = blueprint_with_graph_complexity();
        validate_blueprint_contract(&blueprint).unwrap();

        let encoded = blueprint_to_toml(&blueprint).unwrap();
        assert!(encoded.contains("[artifact_inventory.complexity.dimensions.volume]"));
        assert!(encoded.contains("[artifact_inventory.complexity.dimensions.volume.histogram]"));
        assert!(!encoded.contains("overall_score"));
        let decoded = parse_blueprint_toml(&encoded).unwrap();
        assert_eq!(
            decoded.artifact_inventory.unwrap().complexity,
            blueprint.artifact_inventory.unwrap().complexity
        );
    }

    #[test]
    fn artifact_complexity_remains_per_source_in_a_bundle() {
        let mut bundle = embedded_bundle();
        bundle.sources.get_mut("source-a").unwrap().blueprint =
            Some(blueprint_with_graph_complexity());
        recompute_bundle_totals(&mut bundle).unwrap();

        let encoded = blueprint_bundle_to_toml(&bundle).unwrap();
        let value: toml::Value = toml::from_str(&encoded).unwrap();
        assert!(value.get("complexity").is_none());
        assert!(encoded.contains("[sources.source-a.blueprint.artifact_inventory.complexity]"));

        let decoded = parse_blueprint_bundle_toml(&encoded).unwrap();
        assert!(decoded.sources["source-a"]
            .blueprint
            .as_ref()
            .unwrap()
            .artifact_inventory
            .as_ref()
            .unwrap()
            .complexity
            .is_some());
    }

    #[test]
    fn schema_v7_rejects_complexity_below_graph_and_reserved_v1_score() {
        let mut summary = one_table_blueprint();
        let mut inventory = empty_artifact_inventory();
        inventory.complexity = Some(ArtifactComplexity::default());
        summary.artifact_inventory = Some(inventory);
        assert!(validate_blueprint_contract(&summary)
            .unwrap_err()
            .to_string()
            .contains("must not contain a complexity assessment"));

        let mut missing = blueprint_with_graph_complexity();
        missing.artifact_inventory.as_mut().unwrap().complexity = None;
        assert!(validate_blueprint_contract(&missing)
            .unwrap_err()
            .to_string()
            .contains("requires a complexity assessment"));

        let mut scored = blueprint_with_graph_complexity();
        scored
            .artifact_inventory
            .as_mut()
            .unwrap()
            .complexity
            .as_mut()
            .unwrap()
            .overall_score = Some(42);
        assert!(validate_blueprint_contract(&scored)
            .unwrap_err()
            .to_string()
            .contains("must not populate reserved overall_score"));

        let mut limited = blueprint_with_graph_complexity();
        let limitations = &mut limited
            .artifact_inventory
            .as_mut()
            .unwrap()
            .complexity
            .as_mut()
            .unwrap()
            .limitations;
        limitations.push("computation-limit".into());
        limitations.sort();
        assert!(validate_blueprint_contract(&limited)
            .unwrap_err()
            .to_string()
            .contains("does not yet define a recomputable limited result"));
    }

    #[test]
    fn schema_v7_complexity_distinguishes_unsupported_dialect_from_hidden_source() {
        let mut analyzed = blueprint_with_graph_complexity();
        let inventory = analyzed.artifact_inventory.as_mut().unwrap();
        inventory.detail = "analyzed".into();
        let artifact = inventory.artifacts.get_mut("function-001").unwrap();
        artifact.definition_visibility = "available".into();
        artifact.analysis = Some(LanguageFeatureCensus {
            contract: LANGUAGE_CENSUS_CONTRACT.into(),
            status: "unavailable".into(),
            dialect: "c".into(),
            grammar_profile: "postgresql-18".into(),
            analyzer_version: "lexical-v2".into(),
            analysis_span: "executable-body".into(),
            ..Default::default()
        });
        inventory.complexity = Some(
            crate::assess_artifact_complexity(inventory, true)
                .expect("assess unsupported dialect fixture"),
        );
        validate_blueprint_contract(&analyzed).unwrap();

        let inventory = analyzed.artifact_inventory.as_mut().unwrap();
        inventory
            .artifacts
            .get_mut("function-001")
            .unwrap()
            .analysis
            .as_mut()
            .unwrap()
            .analysis_span
            .clear();
        inventory.complexity = Some(
            crate::assess_artifact_complexity(inventory, true)
                .expect("reassess missing analysis span"),
        );
        validate_blueprint_contract(&analyzed).unwrap();

        let mut graph = blueprint_with_graph_complexity();
        graph
            .artifact_inventory
            .as_mut()
            .unwrap()
            .complexity
            .as_mut()
            .unwrap()
            .limitations = vec![
            "definition-analysis-not-requested".into(),
            "unsupported-dialect".into(),
        ];
        assert!(validate_blueprint_contract(&graph)
            .unwrap_err()
            .to_string()
            .contains("cannot report definition-read limitations"));
    }

    #[test]
    fn assessor_v1_uses_complete_and_partial_census_evidence_per_dimension() {
        let mut blueprint = blueprint_with_graph_complexity();
        let inventory = blueprint.artifact_inventory.as_mut().unwrap();
        inventory.detail = "analyzed".into();
        inventory.analysis_complete = true;
        let artifact = inventory.artifacts.get_mut("function-001").unwrap();
        artifact.definition_visibility = "available".into();
        artifact.analysis = Some(LanguageFeatureCensus {
            contract: LANGUAGE_CENSUS_CONTRACT.into(),
            status: "complete".into(),
            dialect: "sql".into(),
            grammar_profile: "postgresql-18".into(),
            analyzer_version: "lexical-v2".into(),
            analysis_span: "executable-body".into(),
            definition_size_band: "1-255".into(),
            statement_count_band: "1".into(),
            token_count_band: "2-4".into(),
            maximum_nesting_band: "0".into(),
            cyclomatic_complexity_band: "1".into(),
            opaque_region_count_band: "0".into(),
            ..Default::default()
        });
        inventory.complexity = Some(
            crate::assess_artifact_complexity(inventory, true)
                .expect("assess complete census fixture"),
        );
        let complete = inventory.complexity.as_ref().unwrap();
        assert_eq!(complete.fully_assessed_object_count, 1);
        assert_eq!(complete.partially_assessed_object_count, 0);
        assert_eq!(complete.dimensions.volume.coverage, "complete");
        assert_eq!(complete.dimensions.control_flow.coverage, "complete");
        validate_blueprint_contract(&blueprint).unwrap();

        let inventory = blueprint.artifact_inventory.as_mut().unwrap();
        inventory.analysis_complete = false;
        let analysis = inventory
            .artifacts
            .get_mut("function-001")
            .unwrap()
            .analysis
            .as_mut()
            .unwrap();
        analysis.status = "partial".into();
        analysis.cyclomatic_complexity_band.clear();
        inventory.complexity = Some(
            crate::assess_artifact_complexity(inventory, true)
                .expect("assess partial census fixture"),
        );
        let partial = inventory.complexity.as_ref().unwrap();
        assert_eq!(partial.fully_assessed_object_count, 0);
        assert_eq!(partial.partially_assessed_object_count, 1);
        assert_eq!(partial.dimensions.volume.coverage, "complete");
        assert_eq!(partial.dimensions.control_flow.coverage, "unknown");
        assert_eq!(partial.dimensions.control_flow.histogram.unknown, 1);
        validate_blueprint_contract(&blueprint).unwrap();
    }

    #[test]
    fn schema_v7_requirement_coverage_is_per_object_and_fail_closed() {
        let mut mixed = blueprint_with_graph_complexity();
        {
            let inventory = mixed.artifact_inventory.as_mut().unwrap();
            inventory.requirements_complete = false;
            let mut unavailable = inventory.artifacts["function-001"].clone();
            unavailable.requirement_status = "unavailable".into();
            inventory
                .artifacts
                .insert("function-002".into(), unavailable);
            inventory.object_count = 2;
            inventory.counts_by_kind.insert("function".into(), 2);
            inventory.complexity = Some(
                crate::assess_artifact_complexity(inventory, true)
                    .expect("assess mixed requirement coverage"),
            );
        }
        let complexity = mixed
            .artifact_inventory
            .as_ref()
            .unwrap()
            .complexity
            .as_ref()
            .unwrap();
        assert_eq!(
            complexity.dimensions.environment_coupling.coverage,
            "partial"
        );
        assert_eq!(complexity.dimensions.environment_coupling.histogram.zero, 1);
        assert_eq!(
            complexity.dimensions.environment_coupling.histogram.unknown,
            1
        );
        assert_eq!(complexity.dimensions.environment_coupling.band, "unknown");
        validate_blueprint_contract(&mixed).unwrap();

        let mut false_aggregate_claim = mixed.clone();
        let inventory = false_aggregate_claim.artifact_inventory.as_mut().unwrap();
        inventory.requirements_complete = true;
        inventory.complexity = Some(
            crate::assess_artifact_complexity(inventory, true)
                .expect("assess false aggregate requirement claim"),
        );
        assert!(validate_blueprint_contract(&false_aggregate_claim)
            .unwrap_err()
            .to_string()
            .contains("requires complete or not_applicable requirement_status"));

        let mut incomplete_population = blueprint_with_graph_complexity();
        let inventory = incomplete_population.artifact_inventory.as_mut().unwrap();
        inventory.complexity = Some(
            crate::assess_artifact_complexity(inventory, false)
                .expect("assess incomplete requirement population"),
        );
        assert!(validate_blueprint_contract(&incomplete_population)
            .unwrap_err()
            .to_string()
            .contains("requires a complete assessment population"));

        let mut unrelated_catalog_gap = blueprint_with_graph_complexity();
        let inventory = unrelated_catalog_gap.artifact_inventory.as_mut().unwrap();
        inventory.inventory_complete = false;
        inventory.dependencies_complete = false;
        inventory.catalogs_unreadable = vec!["oracle.scheduler".into()];
        inventory.complexity = Some(
            crate::assess_artifact_complexity(inventory, true)
                .expect("assess independently complete requirement evidence"),
        );
        validate_blueprint_contract(&unrelated_catalog_gap)
            .expect("an unrelated catalog gap must not erase requirement coverage");

        let mut missing_status = blueprint_with_graph_complexity();
        let inventory = missing_status.artifact_inventory.as_mut().unwrap();
        inventory.requirements_complete = false;
        inventory
            .artifacts
            .get_mut("function-001")
            .unwrap()
            .requirement_status
            .clear();
        inventory.complexity = Some(
            crate::assess_artifact_complexity(inventory, true)
                .expect("assess missing requirement coverage conservatively"),
        );
        assert!(validate_blueprint_contract(&missing_status)
            .unwrap_err()
            .to_string()
            .contains("artifact requirement_status has unsupported value"));

        let mut contradictory = blueprint_with_graph_complexity();
        let inventory = contradictory.artifact_inventory.as_mut().unwrap();
        let artifact = inventory.artifacts.get_mut("function-001").unwrap();
        artifact.requirement_status = "not_applicable".into();
        artifact.requirements = vec![crate::ArtifactRequirement {
            token: "postgresql.function.security-definer".into(),
            evidence: "catalog-confirmed".into(),
            count_band: "1".into(),
        }];
        inventory.complexity = Some(
            crate::assess_artifact_complexity(inventory, true)
                .expect("assess contradictory requirement evidence"),
        );
        assert!(validate_blueprint_contract(&contradictory)
            .unwrap_err()
            .to_string()
            .contains("cannot carry requirements"));

        let mut unavailable_with_evidence = blueprint_with_graph_complexity();
        let inventory = unavailable_with_evidence
            .artifact_inventory
            .as_mut()
            .unwrap();
        inventory.requirements_complete = false;
        let artifact = inventory.artifacts.get_mut("function-001").unwrap();
        artifact.requirement_status = "unavailable".into();
        artifact.requirements = vec![crate::ArtifactRequirement {
            token: "postgresql.function.security-definer".into(),
            evidence: "catalog-confirmed".into(),
            count_band: "1".into(),
        }];
        inventory.complexity = Some(
            crate::assess_artifact_complexity(inventory, true)
                .expect("assess unavailable requirement evidence conservatively"),
        );
        assert!(validate_blueprint_contract(&unavailable_with_evidence)
            .unwrap_err()
            .to_string()
            .contains("must use partial requirement_status"));
    }

    #[test]
    fn schema_v7_complexity_keeps_empty_distinct_from_trivial() {
        let size_not_applicable = ArtifactComplexitySizeDimension {
            band: "not-applicable".into(),
            coverage: "not-applicable".into(),
            histogram: ArtifactComplexitySizeHistogram::default(),
        };
        let count_not_applicable = ArtifactComplexityCountDimension {
            band: "not-applicable".into(),
            coverage: "not-applicable".into(),
            histogram: ArtifactComplexityCountHistogram::default(),
        };
        let mut blueprint = one_table_blueprint();
        blueprint.artifact_inventory = Some(ArtifactInventory {
            contract: ARTIFACT_CONTRACT.into(),
            detail: "graph".into(),
            scope: "all-visible-schemas".into(),
            visibility: "full".into(),
            inventory_complete: true,
            dependencies_complete: true,
            requirements_complete: true,
            complexity: Some(ArtifactComplexity {
                contract: ARTIFACT_COMPLEXITY_CONTRACT.into(),
                assessor_version: ARTIFACT_COMPLEXITY_ASSESSOR_VERSION,
                scope: "all-visible-schemas".into(),
                population_policy: ARTIFACT_COMPLEXITY_POPULATION_POLICY.into(),
                assessment_population_complete: true,
                analyzer_version: "not-applicable".into(),
                overall_band: "not-applicable".into(),
                limitations: vec!["definition-analysis-not-requested".into()],
                dimensions: ArtifactComplexityDimensions {
                    volume: size_not_applicable,
                    control_flow: count_not_applicable.clone(),
                    feature_breadth: count_not_applicable.clone(),
                    entanglement: count_not_applicable.clone(),
                    environment_coupling: count_not_applicable.clone(),
                    opacity: count_not_applicable.clone(),
                    dialect_coupling: count_not_applicable,
                },
                ..Default::default()
            }),
            ..Default::default()
        });
        validate_blueprint_contract(&blueprint).unwrap();
    }

    #[test]
    fn schema_v7_complexity_treats_not_applicable_as_assessed_per_dimension() {
        let mut blueprint = one_table_blueprint();
        let mut inventory = ArtifactInventory {
            contract: ARTIFACT_CONTRACT.into(),
            detail: "analyzed".into(),
            scope: "all-visible-schemas".into(),
            visibility: "full".into(),
            inventory_complete: true,
            dependencies_complete: true,
            requirements_complete: true,
            object_count: 1,
            counts_by_kind: BTreeMap::from([("sequence".into(), 1)]),
            complexity: None,
            artifacts: BTreeMap::from([(
                "sequence-001".into(),
                BlueprintArtifact {
                    kind: "sequence".into(),
                    subkind: "ordinary".into(),
                    tier: "declarative".into(),
                    requirement_status: "not_applicable".into(),
                    definition_visibility: "not_applicable".into(),
                    analysis: Some(LanguageFeatureCensus {
                        contract: LANGUAGE_CENSUS_CONTRACT.into(),
                        status: "not_applicable".into(),
                        dialect: "internal".into(),
                        grammar_profile: "postgresql-18".into(),
                        analyzer_version: "lexical-v2".into(),
                        analysis_span: "not-applicable".into(),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )]),
            ..Default::default()
        };
        inventory.complexity = Some(
            crate::assess_artifact_complexity(&inventory, true)
                .expect("assess not-applicable fixture"),
        );
        blueprint.artifact_inventory = Some(inventory);

        let complexity = blueprint
            .artifact_inventory
            .as_ref()
            .unwrap()
            .complexity
            .as_ref()
            .unwrap();
        assert_eq!(complexity.dimensions.volume.coverage, "not-applicable");
        assert_eq!(
            complexity.dimensions.control_flow.coverage,
            "not-applicable"
        );
        assert_eq!(
            complexity.dimensions.feature_breadth.coverage,
            "not-applicable"
        );
        assert_eq!(complexity.dimensions.opacity.coverage, "not-applicable");
        assert_eq!(complexity.dimensions.entanglement.coverage, "complete");

        // Broad inventory completeness covers every artifact family, while
        // assessment completeness covers only the declared assessment
        // population. They are intentionally independent claims.
        blueprint
            .artifact_inventory
            .as_mut()
            .unwrap()
            .inventory_complete = false;
        validate_blueprint_contract(&blueprint).unwrap();

        let inventory = blueprint.artifact_inventory.as_mut().unwrap();
        inventory.requirements_complete = false;
        inventory.complexity = Some(
            crate::assess_artifact_complexity(inventory, false)
                .expect("reassess incomplete population"),
        );
        assert_eq!(
            inventory.complexity.as_ref().unwrap().overall_band,
            "unknown"
        );
        validate_blueprint_contract(&blueprint).unwrap();
    }

    #[test]
    fn schema_v7_accepts_queue_and_edition_but_rejects_open_subkinds() {
        for (kind, subkind) in [("queue", "service_broker"), ("edition", "ordinary")] {
            let mut blueprint = blueprint_with_graph_complexity();
            let inventory = blueprint.artifact_inventory.as_mut().unwrap();
            inventory.counts_by_kind = BTreeMap::from([(kind.into(), 1)]);
            let (_, mut artifact) = inventory.artifacts.pop_first().unwrap();
            artifact.kind = kind.into();
            artifact.subkind = subkind.into();
            inventory.artifacts.insert(format!("{kind}-001"), artifact);
            validate_blueprint_contract(&blueprint).unwrap();
        }

        let mut blueprint = blueprint_with_graph_complexity();
        blueprint
            .artifact_inventory
            .as_mut()
            .unwrap()
            .artifacts
            .get_mut("function-001")
            .unwrap()
            .subkind = "provider_invented_spelling".into();
        assert!(validate_blueprint_contract(&blueprint)
            .unwrap_err()
            .to_string()
            .contains("artifact subkind has unsupported value"));
    }

    #[test]
    fn schema_v7_keeps_customer_external_binaries_in_the_assessment_population() {
        let mut blueprint = blueprint_with_graph_complexity();
        let artifact = blueprint
            .artifact_inventory
            .as_mut()
            .unwrap()
            .artifacts
            .get_mut("function-001")
            .unwrap();
        artifact.definition_visibility = "external_binary".into();
        artifact.generated_by_engine = Some(false);
        validate_blueprint_contract(&blueprint).unwrap();

        blueprint
            .artifact_inventory
            .as_mut()
            .unwrap()
            .artifacts
            .get_mut("function-001")
            .unwrap()
            .generated_by_engine = Some(true);
        assert!(validate_blueprint_contract(&blueprint)
            .unwrap_err()
            .to_string()
            .contains("excluded_object_count"));
    }

    #[test]
    fn schema_v7_graph_complexity_never_claims_an_overall_verdict() {
        let mut blueprint = blueprint_with_graph_complexity();
        let complexity = blueprint
            .artifact_inventory
            .as_mut()
            .unwrap()
            .complexity
            .as_mut()
            .unwrap();
        assert_eq!(complexity.dimensions.volume.coverage, "unknown");
        assert_eq!(complexity.dimensions.control_flow.coverage, "unknown");
        assert_eq!(complexity.dimensions.entanglement.coverage, "complete");
        assert_eq!(
            complexity.dimensions.environment_coupling.coverage,
            "complete"
        );
        complexity.overall_band = "trivial".into();

        assert!(validate_blueprint_contract(&blueprint)
            .unwrap_err()
            .to_string()
            .contains("graph artifact complexity must use unknown overall_band"));
    }

    #[test]
    fn schema_v7_wrapped_source_must_be_unknown_in_the_opacity_dimension() {
        let mut blueprint = blueprint_with_graph_complexity();
        let inventory = blueprint.artifact_inventory.as_mut().unwrap();
        inventory.detail = "analyzed".into();
        let artifact = inventory.artifacts.get_mut("function-001").unwrap();
        artifact.definition_visibility = "encrypted".into();
        artifact.requirements = vec![crate::ArtifactRequirement {
            token: "oracle.source.wrapped".into(),
            evidence: "catalog-confirmed".into(),
            count_band: "1".into(),
        }];
        inventory.complexity = Some(
            crate::assess_artifact_complexity(inventory, true)
                .expect("assess wrapped-source fixture"),
        );
        validate_blueprint_contract(&blueprint).unwrap();

        let opacity = &mut blueprint
            .artifact_inventory
            .as_mut()
            .unwrap()
            .complexity
            .as_mut()
            .unwrap()
            .dimensions
            .opacity;
        opacity.band = "not-applicable".into();
        opacity.coverage = "not-applicable".into();
        opacity.histogram.unknown = 0;
        opacity.histogram.not_applicable = 1;
        assert!(validate_blueprint_contract(&blueprint)
            .unwrap_err()
            .to_string()
            .contains("wrapped source must contribute to unknown opacity coverage"));
    }

    #[test]
    fn schema_v7_rejects_complexity_population_and_histogram_mismatches() {
        let mut bad_population = blueprint_with_graph_complexity();
        bad_population
            .artifact_inventory
            .as_mut()
            .unwrap()
            .complexity
            .as_mut()
            .unwrap()
            .excluded_object_count = 1;
        assert!(validate_blueprint_contract(&bad_population)
            .unwrap_err()
            .to_string()
            .contains("does not match artifact object_count"));

        let mut bad_histogram = blueprint_with_graph_complexity();
        bad_histogram
            .artifact_inventory
            .as_mut()
            .unwrap()
            .complexity
            .as_mut()
            .unwrap()
            .dimensions
            .volume
            .histogram
            .unknown = 2;
        assert!(validate_blueprint_contract(&bad_histogram)
            .unwrap_err()
            .to_string()
            .contains("histogram sum 2 does not match eligible_object_count 1"));

        // Preserve every structural sum and the resulting band while moving
        // the object into a false bucket. Arithmetic-only validation would
        // accept this.
        let mut wrong_bucket = blueprint_with_graph_complexity();
        let histogram = &mut wrong_bucket
            .artifact_inventory
            .as_mut()
            .unwrap()
            .complexity
            .as_mut()
            .unwrap()
            .dimensions
            .dialect_coupling
            .histogram;
        histogram.zero = 0;
        histogram.one = 1;
        assert!(validate_blueprint_contract(&wrong_bucket)
            .unwrap_err()
            .to_string()
            .contains("does not match assessor-v1 recomputation"));

        let mut overflowing = blueprint_with_graph_complexity();
        let complexity = overflowing
            .artifact_inventory
            .as_mut()
            .unwrap()
            .complexity
            .as_mut()
            .unwrap();
        complexity.fully_assessed_object_count = u64::MAX;
        complexity.partially_assessed_object_count = 1;
        assert!(validate_blueprint_contract(&overflowing)
            .unwrap_err()
            .to_string()
            .contains("assessed object counts overflow u64"));
    }

    #[test]
    fn schema_v7_requires_a_computation_failure_to_be_fully_fail_closed() {
        let mut blueprint = blueprint_with_graph_complexity();
        let inventory = blueprint.artifact_inventory.as_mut().unwrap();
        inventory.complexity = Some(
            crate::failed_artifact_complexity(inventory, true)
                .expect("construct fail-closed complexity fixture"),
        );
        validate_blueprint_contract(&blueprint).unwrap();

        let dimension = &mut blueprint
            .artifact_inventory
            .as_mut()
            .unwrap()
            .complexity
            .as_mut()
            .unwrap()
            .dimensions
            .environment_coupling;
        dimension.band = "trivial".into();
        dimension.coverage = "complete".into();
        dimension.histogram.unknown = 0;
        dimension.histogram.zero = 1;
        assert!(validate_blueprint_contract(&blueprint)
            .unwrap_err()
            .to_string()
            .contains("required fail-closed unknown shape"));
    }

    #[test]
    fn artifact_complexity_histogram_rejects_cross_tabulation_fields() {
        let text = r#"
"0" = 1
"1" = 0
"2-4" = 0
"5-8" = 0
"9-16" = 0
"17-32" = 0
"33+" = 0
not_applicable = 0
unknown = 0
kind_by_band = 1
"#;
        assert!(toml::from_str::<ArtifactComplexityCountHistogram>(text)
            .unwrap_err()
            .to_string()
            .contains("unknown field"));
    }

    #[test]
    fn older_schemas_reject_artifact_complexity() {
        let mut blueprint = one_table_v6_blueprint();
        blueprint.artifact_inventory = Some(ArtifactInventory {
            contract: crate::PREVIOUS_ARTIFACT_CONTRACT.into(),
            detail: "summary".into(),
            visibility: "full".into(),
            complexity: Some(ArtifactComplexity::default()),
            ..Default::default()
        });
        assert!(validate_blueprint_contract(&blueprint)
            .unwrap_err()
            .to_string()
            .contains("requires Blueprint schema_version 7"));
    }

    #[test]
    fn schema_v7_rejects_table_and_artifact_parent_cycles() {
        let mut table_cycle = one_table_blueprint();
        let mut second = table_cycle.tables["table-001"].clone();
        second.parent_table = "table-001".into();
        second.child_tables = vec!["table-001".into()];
        let first = table_cycle.tables.get_mut("table-001").unwrap();
        first.parent_table = "table-002".into();
        first.child_tables = vec!["table-002".into()];
        table_cycle.tables.insert("table-002".into(), second);
        table_cycle.totals = computed_blueprint_totals(&table_cycle).unwrap();
        table_cycle.initialize_v7_database_contract();
        assert!(validate_blueprint_contract(&table_cycle)
            .unwrap_err()
            .to_string()
            .contains("table parent hierarchy contains a cycle"));

        let mut artifact_cycle = one_table_blueprint();
        let artifact = |kind: &str, parent: &str| BlueprintArtifact {
            kind: kind.into(),
            subkind: "ordinary".into(),
            tier: "programmatic".into(),
            parent: parent.into(),
            requirement_status: "complete".into(),
            definition_visibility: "not_read".into(),
            ..Default::default()
        };
        let mut inventory = ArtifactInventory {
            contract: ARTIFACT_CONTRACT.into(),
            detail: "graph".into(),
            scope: "all-visible-schemas".into(),
            visibility: "full".into(),
            inventory_complete: true,
            dependencies_complete: true,
            requirements_complete: true,
            object_count: 2,
            counts_by_kind: BTreeMap::from([("function".into(), 1), ("procedure".into(), 1)]),
            artifacts: BTreeMap::from([
                ("function-001".into(), artifact("function", "procedure-001")),
                (
                    "procedure-001".into(),
                    artifact("procedure", "function-001"),
                ),
            ]),
            ..Default::default()
        };
        inventory.complexity = Some(
            crate::assess_artifact_complexity(&inventory, true)
                .expect("assess cyclic-parent fixture"),
        );
        artifact_cycle.artifact_inventory = Some(inventory);
        assert!(validate_blueprint_contract(&artifact_cycle)
            .unwrap_err()
            .to_string()
            .contains("artifact parent hierarchy contains a cycle"));
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

        // Schema-v6 input may omit an exact zero. Preserve that compatibility
        // even though current emitters must be explicit.
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
    fn columnar_transfer_probe_is_table_only_and_round_trips() {
        let mut blueprint = one_table_blueprint();
        blueprint.tables.get_mut("table-001").unwrap().compression = Some(BlueprintCompression {
            measured: true,
            sample_rows: 1_000,
            sample_bytes: 64_000,
            sample_method: "bounded neutral columnar transfer probe".into(),
            ratio_zstd_3: 4.25,
            ratio_stddev: 0.1,
            sample_encoding: TRANSFER_SAMPLE_ENCODING_TAG.into(),
            ..Default::default()
        });
        let encoded = blueprint_to_toml(&blueprint).unwrap();
        let decoded = parse_blueprint_toml(&encoded).unwrap();
        assert_eq!(
            decoded.tables["table-001"]
                .compression
                .as_ref()
                .unwrap()
                .sample_encoding,
            TRANSFER_SAMPLE_ENCODING_TAG
        );
        blueprint.tables.get_mut("table-001").unwrap().compression = Some(BlueprintCompression {
            measured: true,
            sample_rows: 1_000,
            sample_bytes: 64_000,
            sample_method: "bounded chunk-aware neutral columnar transfer probe".into(),
            ratio_zstd_3: 4.25,
            ratio_stddev: 0.1,
            sample_encoding: TRANSFER_SAMPLE_STREAMING_CHUNKED_ENCODING_TAG.into(),
            ..Default::default()
        });
        validate_blueprint_contract(&blueprint).unwrap();

        blueprint
            .tables
            .get_mut("table-001")
            .unwrap()
            .cols
            .insert("column-001".into(), crate::BlueprintColumn::default());
        blueprint
            .tables
            .get_mut("table-001")
            .unwrap()
            .cols
            .get_mut("column-001")
            .unwrap()
            .compression = Some(BlueprintCompression {
            sample_encoding: TRANSFER_SAMPLE_STREAMING_CHUNKED_ENCODING_TAG.into(),
            ..Default::default()
        });
        assert!(validate_blueprint_contract(&blueprint).is_err());
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
        let table_evidence = blueprint
            .tables
            .get_mut("table-001")
            .unwrap()
            .statistics
            .as_mut()
            .unwrap();
        table_evidence.row_count_quality = "engine-estimate".into();
        table_evidence.size_quality = "engine-counter".into();
        let aggregate = blueprint.statistics_evidence.as_mut().unwrap();
        aggregate.counts_by_row_count_quality = BTreeMap::from([("engine-estimate".into(), 1)]);
        aggregate.counts_by_size_quality = BTreeMap::from([("engine-counter".into(), 1)]);
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
        let mut blueprint = one_table_v6_blueprint();
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
    fn schema_v6_round_trip_preserves_table_and_column_semantics() {
        let mut blueprint = one_table_v6_blueprint();
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
        let mut blueprint = one_table_v6_blueprint();
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
        let mut blueprint = one_table_v6_blueprint();
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
            "table-002".into(),
            BlueprintTable {
                schema: "schema-A".into(),
                rows: 900,
                table_bytes: 9_000,
                index_bytes: 900,
                kind: "external".into(),
                counted_in_totals: Some(false),
                ..Default::default()
            },
        );
        blueprint.initialize_v7_database_contract();
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
                requirement_status: "unavailable".into(),
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
        let mut table = BlueprintTable {
            schema: "schema-A".into(),
            ..BlueprintTable::default()
        };
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
        blueprint.initialize_v7_database_contract();

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
    fn blueprint_uri_path_parses_plain_and_scheme_paths_and_rejects_other_uri_schemes() {
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
    fn bundle_contract_normalizes_v1_defaults_and_validates_embedded_blueprints() {
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
    fn v1_bundle_identifiers_parse_and_reemit_only_blueprint_identifiers() {
        let v1_input = r#"
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

        let parsed = parse_blueprint_bundle_toml(v1_input).expect("v1 bundle parses");
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
    fn v1_embedded_field_reemits_as_blueprint() {
        let canonical = blueprint_bundle_to_toml(&embedded_bundle()).unwrap();
        let mut v1_input: toml::Value = toml::from_str(&canonical).unwrap();
        let root = v1_input.as_table_mut().unwrap();
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
        let v1_input = toml::to_string_pretty(&v1_input).unwrap();

        let parsed = parse_blueprint_bundle_toml(&v1_input).expect("v1 embedded blueprint parses");
        assert!(parsed.sources["source-a"].blueprint.is_some());
        let emitted = blueprint_bundle_to_toml(&parsed).expect("canonical bundle emits");
        assert!(emitted.contains("[sources.source-a.blueprint]"));
        assert!(!emitted.contains("[sources.source-a.shape]"));
        assert!(!emitted.contains("dbwarp-shape-bundle"));
    }

    #[test]
    fn v4_blueprint_tags_upgrade_but_current_documents_reject_them() {
        let mut v4_input = one_table_v6_blueprint();
        v4_input.schema_version = 4;
        v4_input.database_topology = None;
        v4_input.dataset_scope = None;
        v4_input.artifact_inventory = Some(ArtifactInventory {
            contract: LEGACY_ARTIFACT_CONTRACT.into(),
            detail: "summary".into(),
            visibility: "full".into(),
            ..Default::default()
        });
        v4_input.tables.get_mut("table-001").unwrap().compression =
            Some(crate::BlueprintCompression {
                sample_encoding: LEGACY_SAMPLE_ENCODING_TAG.into(),
                ..Default::default()
            });

        let v4_toml = toml::to_string_pretty(&v4_input).unwrap();
        let parsed = parse_blueprint_toml(&v4_toml).expect("v4 Blueprint parses");
        assert_eq!(parsed.schema_version, LEGACY_IDENTIFIER_SCHEMA_VERSION);
        assert!(parsed.database_topology.is_none());
        assert!(parsed.dataset_scope.is_none());
        assert_eq!(
            parsed.artifact_inventory.as_ref().unwrap().contract,
            crate::PREVIOUS_ARTIFACT_CONTRACT
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
        assert!(emitted.contains(SAMPLE_ENCODING_TAG));
        assert!(!emitted.contains(PREVIOUS_SAMPLE_ENCODING_TAG));

        let current_with_v4_tags = v4_toml.replacen("schema_version = 4", "schema_version = 5", 1);
        assert!(parse_blueprint_toml(&current_with_v4_tags).is_err());
    }

    #[test]
    fn previous_probe_tag_is_input_only_for_current_blueprints() {
        let mut previous = one_table_blueprint();
        previous.tables.get_mut("table-001").unwrap().compression =
            Some(crate::BlueprintCompression {
                sample_encoding: PREVIOUS_SAMPLE_ENCODING_TAG.into(),
                ..Default::default()
            });

        let previous_toml = toml::to_string_pretty(&previous).unwrap();
        let parsed = parse_blueprint_toml(&previous_toml).expect("previous tag parses");
        assert_eq!(
            parsed.tables["table-001"]
                .compression
                .as_ref()
                .unwrap()
                .sample_encoding,
            SAMPLE_ENCODING_TAG
        );

        let emitted = blueprint_to_toml(&parsed).expect("canonical probe tag emits");
        assert!(emitted.contains(SAMPLE_ENCODING_TAG));
        assert!(!emitted.contains(PREVIOUS_SAMPLE_ENCODING_TAG));
    }

    #[test]
    fn v1_indexes_without_ordinals_remain_readable_but_v2_refuses_them() {
        let v1_input = r#"
schema_version = 1
engine = "mysql"

[tables.events]
rows = 10

[tables.events.idxs.pk]
type = "btree"
primary = true
unique = true
"#;
        assert!(parse_blueprint_toml(v1_input).is_ok());
        let current = v1_input.replacen("schema_version = 1", "schema_version = 2", 1);
        assert!(parse_blueprint_toml(&current).is_err());
    }

    #[test]
    fn expression_only_indexes_are_valid_in_v2_but_empty_ordinary_indexes_are_not() {
        let mut blueprint = one_table_blueprint();
        blueprint.tables.get_mut("table-001").unwrap().idxs.insert(
            "idx-1".into(),
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
            .get_mut("idx-1")
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
            schema_version: 2,
            engine: "postgresql".into(),
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
        assert!(
            validate_cardinality(3, "table", "column", 10, false, None, Some(&cardinality))
                .is_err()
        );
    }

    #[test]
    fn cardinality_cannot_estimate_values_from_an_empty_non_null_census() {
        let cardinality = crate::BlueprintCardinality {
            measured: true,
            sample_rows: 1_000,
            non_null_rows: 0,
            observed_distinct_count: 0,
            estimated_distinct_count: 2,
            top_value_fraction: 0.5,
            ..Default::default()
        };
        let error = validate_cardinality(
            7,
            "table",
            "column",
            1_000,
            false,
            Some(1.0),
            Some(&cardinality),
        )
        .expect_err("an empty non-NULL census cannot support a positive estimate");
        assert!(error.to_string().contains("empty non-NULL census"));

        let cardinality = crate::BlueprintCardinality {
            estimated_distinct_count: 0,
            ..cardinality
        };
        let error = validate_cardinality(
            7,
            "table",
            "column",
            1_000,
            false,
            Some(1.0),
            Some(&cardinality),
        )
        .expect_err("an empty non-NULL census cannot support a top-value fraction");
        assert!(error.to_string().contains("top-value fraction"));
    }

    #[test]
    fn complete_read_cardinality_cannot_exceed_exact_table_rows() {
        let cardinality = crate::BlueprintCardinality {
            measured: true,
            complete_source_read: true,
            sample_rows: 50,
            non_null_rows: 50,
            observed_distinct_count: 50,
            estimated_distinct_count: 50,
            sample_method: "LIMIT; complete bounded sample".into(),
            ..Default::default()
        };
        let error = validate_cardinality(
            7,
            "table",
            "column",
            49,
            true,
            Some(0.0),
            Some(&cardinality),
        )
        .expect_err("a complete-read sample cannot exceed its exact table population");
        assert!(error.to_string().contains("exact row population"));
    }

    #[test]
    fn complete_read_validation_uses_structured_evidence_not_method_prose() {
        let prose_only = crate::BlueprintCardinality {
            measured: true,
            sample_rows: 50,
            non_null_rows: 50,
            observed_distinct_count: 50,
            estimated_distinct_count: 50,
            sample_method: "complete bounded sample".into(),
            ..Default::default()
        };
        validate_cardinality(
            7,
            "table",
            "column",
            49,
            false,
            Some(0.0),
            Some(&prose_only),
        )
        .expect("human-readable method text must not define completeness");
    }

    #[test]
    fn exact_table_population_bounds_value_truncated_cardinality() {
        let cardinality = crate::BlueprintCardinality {
            measured: true,
            complete_source_read: false,
            sample_rows: 50,
            non_null_rows: 50,
            observed_distinct_count: 49,
            estimated_distinct_count: 50,
            sample_method: "complete rows; truncated values".into(),
            ..Default::default()
        };
        let error = validate_cardinality(
            7,
            "table",
            "column",
            49,
            true,
            Some(0.0),
            Some(&cardinality),
        )
        .expect_err("an exact table population bounds a value-truncated census");
        assert!(error.to_string().contains("exact row population"));
    }

    #[test]
    fn schema_v7_null_fraction_must_match_emitted_cardinality_counts() {
        let cardinality = crate::BlueprintCardinality {
            measured: true,
            sample_rows: 100,
            non_null_rows: 96,
            observed_distinct_count: 8,
            estimated_distinct_count: 8,
            ..Default::default()
        };
        validate_cardinality(
            7,
            "table",
            "column",
            100,
            false,
            Some(0.04),
            Some(&cardinality),
        )
        .expect("derived fraction");
        let error = validate_cardinality(
            7,
            "table",
            "column",
            100,
            false,
            Some(0.01),
            Some(&cardinality),
        )
        .expect_err("independently rounded fraction must be rejected");
        assert!(error.to_string().contains("null_fraction disagrees"));
        validate_cardinality(
            6,
            "table",
            "column",
            100,
            false,
            Some(0.01),
            Some(&cardinality),
        )
        .expect("schema-v6 input remains readable");
    }

    #[test]
    fn schema_v3_index_prefix_cardinality_allows_unknown_slots_but_checks_known_values() {
        let mut blueprint = one_table_v6_blueprint();
        blueprint.schema_version = 3;
        blueprint.database_topology = None;
        blueprint.dataset_scope = None;
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
            schema_version: 3,
            engine: "postgresql".into(),
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
