//! Deterministic synthetic-row generation from a validated Blueprint.
//!
//! Generation projects captured nullability, cardinality, length, type, and
//! entropy evidence onto the requested output row count. Identical Blueprint,
//! options, table/column coordinates, and row index produce identical bytes;
//! callers must validate the Blueprint contract before generation.

use crate::{BlueprintColumn, BlueprintRelationship, BlueprintTable};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy)]
pub struct SyntheticOptions {
    pub max_value_bytes: u64,
    pub null_percent: u8,
}

impl Default for SyntheticOptions {
    fn default() -> Self {
        Self {
            max_value_bytes: 64 * 1024,
            null_percent: 3,
        }
    }
}

pub fn ordered_columns(table: &BlueprintTable) -> Vec<(&String, &BlueprintColumn)> {
    let mut columns = table.cols.iter().collect::<Vec<_>>();
    columns.sort_by_key(|(name, col)| (col.ordinal, name.as_str()));
    columns
}

pub fn generated_table_name(prefix: &str, one_based_idx: usize) -> String {
    format!("{prefix}{one_based_idx:04}")
}

pub fn generated_column_name(one_based_idx: usize) -> String {
    format!("c{one_based_idx:03}")
}

pub fn scaled_row_count(rows: u64, scale: f64, max_rows_per_table: Option<u64>) -> u64 {
    let scaled = ((rows as f64) * scale).round() as u64;
    let scaled = if rows > 0 && scale > 0.0 {
        scaled.max(1)
    } else {
        scaled
    };
    max_rows_per_table.map_or(scaled, |max_rows| scaled.min(max_rows))
}

pub fn blueprint_row_value(
    table: &BlueprintTable,
    column: &BlueprintColumn,
    table_idx: u64,
    row_idx: u64,
    col_idx: u64,
    options: SyntheticOptions,
) -> Option<Vec<u8>> {
    blueprint_row_value_with_entropy(table, column, table_idx, row_idx, col_idx, options, None)
}

pub fn blueprint_row_value_with_entropy(
    table: &BlueprintTable,
    column: &BlueprintColumn,
    table_idx: u64,
    row_idx: u64,
    col_idx: u64,
    options: SyntheticOptions,
    entropy_override: Option<f64>,
) -> Option<Vec<u8>> {
    blueprint_row_value_for_generated_rows_with_entropy(
        table,
        column,
        table.rows,
        table_idx,
        row_idx,
        col_idx,
        options,
        entropy_override,
    )
}

/// Generate a value while projecting captured source cardinality onto the
/// actual generated table size. Callers applying a fixture scale should use
/// this entry point; the legacy helpers retain source-row-count semantics.
pub fn blueprint_row_value_for_generated_rows_with_entropy(
    table: &BlueprintTable,
    column: &BlueprintColumn,
    generated_row_count: u64,
    table_idx: u64,
    row_idx: u64,
    col_idx: u64,
    options: SyntheticOptions,
    entropy_override: Option<f64>,
) -> Option<Vec<u8>> {
    let null_seed = synthetic_seed(table_idx, row_idx, col_idx);
    let ty = normalized_type(column.column_type.as_str());
    if is_null_type(&ty) || should_emit_null(column, null_seed, options.null_percent) {
        return None;
    }
    let value_row_idx = statistical_value_row_index_for_generated_rows(
        table,
        column,
        generated_row_count,
        row_idx,
        col_idx,
    );
    let seed = synthetic_seed(table_idx, value_row_idx, col_idx);
    let entropy = entropy_override
        .filter(|entropy| entropy.is_finite())
        .map(|entropy| entropy.clamp(0.0, 1.0))
        .unwrap_or_else(|| entropy_for_column(table, column));
    if is_binary_type(&ty) {
        let len = generated_value_len(
            table,
            column,
            value_row_idx,
            col_idx,
            options.max_value_bytes,
        );
        return Some(generated_binary(seed, len, entropy));
    }
    let value = if is_year_column(&ty, column) {
        generated_year(seed)
    } else if is_bit_column(&ty, column) {
        generated_bit(seed, column.bit_width)
    } else if is_boolean_type(&ty) {
        if generated_bool(seed, entropy) {
            "1".to_string()
        } else {
            "0".to_string()
        }
    } else if is_integer_type(&ty) {
        generated_integer(seed, &ty, column)
    } else if is_numeric_type(&ty) {
        generated_numeric(seed, column)
    } else if matches!(
        ty.as_str(),
        "float" | "float4" | "real" | "double" | "float8" | "double precision"
    ) {
        format!("{:.6}", (seed % 10_000_000) as f64 / 97.0)
    } else if ty == "date" {
        generated_date(seed)
    } else if ty == "time" {
        generated_time(seed)
    } else if is_temporal_type(&ty) {
        format!("{} {}", generated_date(seed), generated_time(seed))
    } else if ty == "uuid" {
        generated_uuid(seed)
    } else {
        generated_text_value(
            table,
            column,
            value_row_idx,
            col_idx,
            options.max_value_bytes,
            entropy,
        )
    };
    Some(value.into_bytes())
}

pub fn append_synthetic_value_bytes(
    out: &mut Vec<u8>,
    table: &BlueprintTable,
    column: &BlueprintColumn,
    table_idx: u64,
    row_idx: u64,
    col_idx: u64,
    options: SyntheticOptions,
) {
    append_synthetic_value_bytes_with_entropy(
        out, table, column, table_idx, row_idx, col_idx, options, None,
    );
}

pub fn append_synthetic_value_bytes_with_entropy(
    out: &mut Vec<u8>,
    table: &BlueprintTable,
    column: &BlueprintColumn,
    table_idx: u64,
    row_idx: u64,
    col_idx: u64,
    options: SyntheticOptions,
    entropy_override: Option<f64>,
) {
    match blueprint_row_value_with_entropy(
        table,
        column,
        table_idx,
        row_idx,
        col_idx,
        options,
        entropy_override,
    ) {
        None => out.extend_from_slice(&u32::MAX.to_le_bytes()),
        Some(value) => {
            let len = value.len().min(u32::MAX as usize) as u32;
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(&value);
        }
    }
}

/// Append one generated row using the same canonical typed row-frame encoding
/// used by dbwarp-blueprint compression sampling. This lets every generator
/// calibrate synthetic entropy against the captured ratio without retaining
/// or reconstructing source values.
pub fn append_synthetic_rowframe_row_with_entropy(
    out: &mut Vec<u8>,
    table: &BlueprintTable,
    columns: &[&BlueprintColumn],
    table_idx: u64,
    row_idx: u64,
    options: SyntheticOptions,
    entropy_override: Option<f64>,
) {
    append_synthetic_rowframe_row_for_generated_rows_with_entropy(
        out,
        table,
        columns,
        table.rows,
        table_idx,
        row_idx,
        options,
        entropy_override,
    );
}

/// Append one canonical row-frame row using the actual generated table size
/// for source-cardinality projection.
pub fn append_synthetic_rowframe_row_for_generated_rows_with_entropy(
    out: &mut Vec<u8>,
    table: &BlueprintTable,
    columns: &[&BlueprintColumn],
    generated_row_count: u64,
    table_idx: u64,
    row_idx: u64,
    options: SyntheticOptions,
    entropy_override: Option<f64>,
) {
    for (col_idx, column) in columns.iter().enumerate() {
        match blueprint_row_value_for_generated_rows_with_entropy(
            table,
            column,
            generated_row_count,
            table_idx,
            row_idx,
            col_idx as u64,
            options,
            entropy_override,
        ) {
            None => append_rowframe_cell(out, 0x00, None),
            Some(value) => append_rowframe_cell(
                out,
                synthetic_rowframe_type_tag(column.column_type.as_str()),
                Some(value.as_slice()),
            ),
        }
    }
}

/// Append a canonical row-frame row while applying a table-level calibration
/// as an offset to each column's own measured entropy. This preserves
/// per-column differences while still allowing the aggregate table ratio to be
/// matched.
pub fn append_synthetic_rowframe_row_for_generated_rows_with_table_entropy_calibration(
    out: &mut Vec<u8>,
    table: &BlueprintTable,
    columns: &[&BlueprintColumn],
    generated_row_count: u64,
    table_idx: u64,
    row_idx: u64,
    options: SyntheticOptions,
    calibrated_table_entropy: f64,
) {
    for (col_idx, column) in columns.iter().enumerate() {
        let column_entropy =
            entropy_for_column_with_table_calibration(table, column, calibrated_table_entropy);
        match blueprint_row_value_for_generated_rows_with_entropy(
            table,
            column,
            generated_row_count,
            table_idx,
            row_idx,
            col_idx as u64,
            options,
            Some(column_entropy),
        ) {
            None => append_rowframe_cell(out, 0x00, None),
            Some(value) => append_rowframe_cell(
                out,
                synthetic_rowframe_type_tag(column.column_type.as_str()),
                Some(value.as_slice()),
            ),
        }
    }
}

/// Return the canonical `dbwarp-blueprint-rowframe-v1` tag for a normalized SQL
/// type. Frontends with their own Blueprint model use this rather than duplicating
/// the compression-sampling wire contract.
pub fn synthetic_rowframe_type_tag(column_type: &str) -> u8 {
    let ty = normalized_type(column_type);
    if is_boolean_type(&ty) {
        0x05
    } else if is_numeric_type(&ty) {
        0x04
    } else if ty == "date" {
        0x07
    } else if ty == "time" {
        0x08
    } else if is_temporal_type(&ty) {
        0x06
    } else if ty == "uuid" {
        0x09
    } else if ty.contains("json") {
        0x0f
    } else if is_binary_type(&ty) {
        0x10
    } else if is_text_type(&ty) {
        0x01
    } else {
        0xfe
    }
}

/// Append one canonical compression-sampling cell. The caller owns value
/// generation; this function owns the stable tag/length framing.
pub fn append_rowframe_cell(out: &mut Vec<u8>, type_tag: u8, value: Option<&[u8]>) {
    let Some(value) = value else {
        out.push(0x00);
        return;
    };
    out.push(type_tag);
    let len = value.len().min(u32::MAX as usize);
    write_rowframe_varint(out, len as u32);
    out.extend_from_slice(&value[..len]);
}

fn write_rowframe_varint(out: &mut Vec<u8>, mut value: u32) {
    while value >= 0x80 {
        out.push((value as u8) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EntropyCalibration {
    pub target_ratio: f64,
    pub observed_ratio: f64,
    pub relative_error: f64,
    pub entropy: f64,
    pub observations: u32,
    pub matched: bool,
}

/// Search the continuous entropy domain without assuming fixture-specific
/// buckets. `observe` must return the compression ratio produced at the given
/// entropy. The highest-quality observation is returned even when the target
/// is outside the generator's achievable range.
pub fn calibrate_entropy<F>(
    target_ratio: f64,
    initial_entropy: f64,
    iterations: u32,
    tolerance: f64,
    mut observe: F,
) -> Result<EntropyCalibration>
where
    F: FnMut(f64) -> Result<f64>,
{
    if !target_ratio.is_finite() || target_ratio < 1.0 {
        bail!("compression calibration target ratio must be finite and at least 1.0");
    }
    if !initial_entropy.is_finite() {
        bail!("compression calibration initial entropy must be finite");
    }
    if iterations == 0 {
        bail!("compression calibration iterations must be greater than zero");
    }
    if !tolerance.is_finite() || tolerance <= 0.0 {
        bail!("compression calibration tolerance must be finite and positive");
    }

    let mut best = EntropyCalibration {
        target_ratio,
        relative_error: f64::INFINITY,
        entropy: initial_entropy.clamp(0.0, 1.0),
        ..Default::default()
    };
    let mut observations = 0_u32;
    let low_ratio = record_entropy_observation(
        0.0,
        target_ratio,
        &mut observe,
        &mut best,
        &mut observations,
    )?;
    let high_ratio = record_entropy_observation(
        1.0,
        target_ratio,
        &mut observe,
        &mut best,
        &mut observations,
    )?;
    record_entropy_observation(
        initial_entropy,
        target_ratio,
        &mut observe,
        &mut best,
        &mut observations,
    )?;
    let decreasing = low_ratio >= high_ratio;
    let mut low = 0.0;
    let mut high = 1.0;
    for _ in 0..iterations {
        if best.relative_error <= tolerance {
            break;
        }
        let entropy = (low + high) / 2.0;
        let ratio = record_entropy_observation(
            entropy,
            target_ratio,
            &mut observe,
            &mut best,
            &mut observations,
        )?;
        if (ratio > target_ratio) == decreasing {
            low = entropy;
        } else {
            high = entropy;
        }
    }
    best.observations = observations;
    best.matched = best.relative_error <= tolerance;
    Ok(best)
}

fn record_entropy_observation<F>(
    entropy: f64,
    target_ratio: f64,
    observe: &mut F,
    best: &mut EntropyCalibration,
    observations: &mut u32,
) -> Result<f64>
where
    F: FnMut(f64) -> Result<f64>,
{
    let entropy = entropy.clamp(0.0, 1.0);
    let ratio = observe(entropy)?;
    if !ratio.is_finite() || ratio <= 0.0 {
        bail!("compression calibration observer returned an invalid ratio {ratio}");
    }
    *observations = (*observations).saturating_add(1);
    let error = (ratio - target_ratio).abs() / target_ratio;
    if error < best.relative_error {
        best.observed_ratio = ratio;
        best.relative_error = error;
        best.entropy = entropy;
    }
    Ok(ratio)
}

pub fn generated_value_len(
    table: &BlueprintTable,
    column: &BlueprintColumn,
    row_idx: u64,
    col_idx: u64,
    max_value_bytes: u64,
) -> usize {
    let variable_columns = table
        .cols
        .values()
        .filter(|candidate| {
            let ty = normalized_type(candidate.column_type.as_str());
            is_text_type(&ty) || is_binary_type(&ty)
        })
        .count()
        .max(1) as u64;
    let avg_row_bytes = if table.rows > 0 && table.table_bytes > 0 {
        table.table_bytes / table.rows.max(1)
    } else {
        estimate_row_bytes_from_columns(table)
    };
    let shared_budget = (avg_row_bytes / variable_columns).max(8);
    let mut len = sane_len_hint(column).unwrap_or(shared_budget);
    // A little over five percent of generated values must land in the p95
    // band. Using fewer than five percent makes the independently measured
    // nearest-rank p95 collapse back to the average-length band.
    if row_idx.checked_rem(19) == Some(0) {
        len = sane_len_hint_p95(column).unwrap_or(len.saturating_mul(2));
    } else if row_idx.wrapping_add(col_idx).checked_rem(11) == Some(0) {
        len = (len / 2).max(1);
    }
    let declared_cap = match (column.declared_max_chars, column.declared_max_bytes) {
        (0, 0) => u64::MAX,
        (chars, 0) => chars,
        (0, bytes) => bytes,
        (chars, bytes) => chars.min(bytes),
    };
    len.min(declared_cap).min(max_value_bytes.max(1)).max(1) as usize
}

pub fn entropy_from_column(column: &BlueprintColumn) -> f64 {
    let ratio = column
        .compression
        .as_ref()
        .filter(|compression| compression.sample_encoding == crate::SAMPLE_ENCODING_TAG)
        .map(|compression| compression.ratio_zstd_3)
        .filter(|ratio| ratio.is_finite() && *ratio >= 1.0)
        .unwrap_or(3.0);
    default_entropy_for_ratio(ratio)
}

pub fn entropy_for_column(table: &BlueprintTable, column: &BlueprintColumn) -> f64 {
    column
        .compression
        .as_ref()
        .filter(|compression| compression.sample_encoding == crate::SAMPLE_ENCODING_TAG)
        .map(|compression| compression.ratio_zstd_3)
        .filter(|ratio| ratio.is_finite() && *ratio >= 1.0)
        .map(default_entropy_for_ratio)
        .unwrap_or_else(|| default_entropy_for_table(table))
}

/// Preserve a column's measured entropy difference from the table baseline
/// while applying the aggregate table calibration selected by a bounded
/// closed-loop search.
pub fn entropy_for_column_with_table_calibration(
    table: &BlueprintTable,
    column: &BlueprintColumn,
    calibrated_table_entropy: f64,
) -> f64 {
    let table_baseline = default_entropy_for_table(table);
    (entropy_for_column(table, column) + calibrated_table_entropy.clamp(0.0, 1.0) - table_baseline)
        .clamp(0.0, 1.0)
}

/// Project a source-domain distribution onto a generated row. The returned
/// index is deterministic and contains no source value material. Exact unique
/// keys are handled by target adapters and deliberately bypass this helper.
pub fn statistical_value_row_index(
    table: &BlueprintTable,
    column: &BlueprintColumn,
    row_idx: u64,
    col_idx: u64,
) -> u64 {
    statistical_value_row_index_for_generated_rows(table, column, table.rows, row_idx, col_idx)
}

/// Project source cardinality onto `generated_row_count` while preserving the
/// observed distinct-to-row ratio. This keeps scaled fixtures representative
/// instead of accidentally retaining the source table's absolute domain size.
pub fn statistical_value_row_index_for_generated_rows(
    table: &BlueprintTable,
    column: &BlueprintColumn,
    generated_row_count: u64,
    row_idx: u64,
    col_idx: u64,
) -> u64 {
    let Some(cardinality) = column
        .cardinality
        .as_ref()
        .filter(|cardinality| cardinality.measured)
    else {
        return row_idx;
    };
    project_scaled_cardinality_index(
        table.rows,
        cardinality.observed_distinct_count,
        cardinality.estimated_distinct_count,
        cardinality.top_value_fraction,
        [
            cardinality.frequency_p50,
            cardinality.frequency_p95,
            cardinality.frequency_p99,
        ],
        generated_row_count,
        row_idx,
        col_idx,
    )
}

/// Project privacy-safe cardinality aggregates from a source row domain onto
/// an arbitrary generated row domain. This primitive is shared by frontends
/// that deserialize Blueprint TOML into their own compatibility models.
pub fn project_scaled_cardinality_index(
    source_row_count: u64,
    observed_distinct_count: u64,
    estimated_distinct_count: u64,
    top_value_fraction: f64,
    frequency_quantiles: [u64; 3],
    generated_row_count: u64,
    row_idx: u64,
    col_idx: u64,
) -> u64 {
    if generated_row_count == 0 {
        return 0;
    }
    let source_distinct = estimated_distinct_count
        .max(observed_distinct_count)
        .min(source_row_count.max(1));
    if source_distinct == 0 {
        return row_idx;
    }
    let generated_distinct = if source_row_count == 0 {
        source_distinct.min(generated_row_count)
    } else {
        ((source_distinct as u128 * generated_row_count as u128 + source_row_count as u128 / 2)
            / source_row_count as u128)
            .clamp(1, generated_row_count as u128) as u64
    };
    if generated_distinct >= generated_row_count {
        return row_idx;
    }

    project_distribution_index(
        row_idx,
        generated_row_count,
        generated_distinct,
        top_value_fraction,
        frequency_quantiles,
        col_idx ^ 0x434f_4c55_4d4e_0001,
    )
}

/// Deterministically project a child row onto a referenced parent row.
///
/// The relationship summary contains aggregates only; no source keys are
/// retained. A composite foreign key must call this once per child row and use
/// the returned parent index for every member so tuple boundaries stay intact.
pub fn relationship_parent_row_index(
    statistics: Option<&BlueprintRelationship>,
    child_row_idx: u64,
    child_row_count: u64,
    parent_row_count: u64,
    edge_ordinal: u64,
    fallback_null_fraction: f64,
) -> Option<u64> {
    if parent_row_count == 0 || child_row_count == 0 {
        return None;
    }
    let null_fraction = statistics
        .filter(|statistics| statistics.sample_rows > 0)
        .map(|statistics| {
            1.0 - statistics.non_null_rows.min(statistics.sample_rows) as f64
                / statistics.sample_rows as f64
        })
        .unwrap_or(fallback_null_fraction)
        .clamp(0.0, 1.0);
    let position = rotated_position(
        child_row_idx,
        child_row_count,
        edge_ordinal ^ 0x464f_5245_4947_4e01,
    );
    let null_rows = ((child_row_count as f64) * null_fraction).round() as u64;
    if position < null_rows.min(child_row_count) {
        return None;
    }
    let non_null_rows = child_row_count.saturating_sub(null_rows).max(1);
    let non_null_position = position.saturating_sub(null_rows);
    let covered_parent_rows = statistics
        .and_then(|statistics| {
            (statistics.parent_coverage_fraction > 0.0).then(|| {
                ((parent_row_count as f64) * statistics.parent_coverage_fraction)
                    .round()
                    .max(1.0) as u64
            })
        })
        .unwrap_or(parent_row_count)
        .clamp(1, parent_row_count);
    let hot_fraction = statistics
        .filter(|statistics| statistics.non_null_rows > 0)
        .map(|statistics| statistics.fanout_max as f64 / statistics.non_null_rows.max(1) as f64)
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let frequency_quantiles = statistics
        .map(|statistics| {
            [
                statistics.fanout_p50,
                statistics.fanout_p95,
                statistics.fanout_p99,
            ]
        })
        .unwrap_or([1, 1, 1]);
    Some(project_distribution_index(
        non_null_position,
        non_null_rows,
        covered_parent_rows,
        hot_fraction,
        frequency_quantiles,
        edge_ordinal ^ 0x5041_5245_4e54_0001,
    ))
}

/// Deterministically materialize a bounded statistical distribution without
/// retaining source values. Every domain member is represented when the full
/// row range is consumed, then remaining rows follow the supplied skew hints.
pub fn project_distribution_index(
    row_idx: u64,
    row_count: u64,
    distinct_count: u64,
    top_value_fraction: f64,
    frequency_quantiles: [u64; 3],
    salt: u64,
) -> u64 {
    let rows = row_count.max(1);
    let domain = distinct_count.clamp(1, rows);
    if domain == 1 {
        return 0;
    }

    // Rotation is a bijection over the finite row range. Unlike hash modulo,
    // this guarantees that every requested domain value is represented when
    // a complete generated table is consumed.
    let position = rotated_position(row_idx, rows, salt);
    let tail_domain = domain - 1;
    let requested_hot_rows = ((rows as f64) * top_value_fraction.clamp(0.0, 1.0)).round() as u64;
    let hot_rows = requested_hot_rows
        .max(1)
        .min(rows.saturating_sub(tail_domain).max(1));
    if position < hot_rows {
        return 0;
    }

    let tail_position = position - hot_rows;
    if tail_position < tail_domain {
        // Reserve one occurrence for every tail value before allocating
        // repeats according to the sampled frequency profile.
        return 1 + tail_position;
    }

    let repeated_position = tail_position - tail_domain;
    let repeated_rows = rows.saturating_sub(hot_rows).saturating_sub(tail_domain);
    if repeated_rows == 0 {
        return 1 + repeated_position % tail_domain;
    }
    1 + weighted_tail_index(repeated_position, tail_domain, frequency_quantiles, salt)
}

fn rotated_position(row_idx: u64, row_count: u64, salt: u64) -> u64 {
    let rows = row_count.max(1);
    row_idx.wrapping_add(mix64(salt) % rows) % rows
}

fn weighted_tail_index(row_idx: u64, domain: u64, frequency_quantiles: [u64; 3], salt: u64) -> u64 {
    if domain <= 1 {
        return 0;
    }
    let p50 = frequency_quantiles[0].max(1);
    let p95 = frequency_quantiles[1].max(p50);
    let p99 = frequency_quantiles[2].max(p95);
    if p50 == p99 {
        return mix64(row_idx ^ salt) % domain;
    }

    let group_50 = ((domain as u128 * 50 + 99) / 100).max(1) as u64;
    let group_95 = ((domain as u128 * 45 + 99) / 100) as u64;
    let group_99 = domain.saturating_sub(group_50).saturating_sub(group_95);
    let weights = [(group_50, p50), (group_95, p95), (group_99, p99)];
    let total_weight = weights.iter().fold(0_u128, |total, (count, weight)| {
        total.saturating_add(*count as u128 * *weight as u128)
    });
    if total_weight == 0 {
        return mix64(row_idx ^ salt) % domain;
    }
    let selector = (mix64(
        row_idx
            .wrapping_mul(0x9e37_79b9_7f4a_7c15)
            .wrapping_add(salt),
    ) as u128)
        % total_weight;
    let mut offset = 0_u64;
    let mut cursor = 0_u128;
    for (count, weight) in weights {
        if count == 0 {
            continue;
        }
        let span = count as u128 * weight as u128;
        if selector < cursor.saturating_add(span) {
            let local = mix64(row_idx ^ salt ^ offset) % count;
            return (offset + local).min(domain - 1);
        }
        cursor = cursor.saturating_add(span);
        offset = offset.saturating_add(count);
    }
    mix64(row_idx ^ salt) % domain
}

pub fn default_entropy_for_table(table: &BlueprintTable) -> f64 {
    table
        .compression
        .as_ref()
        .filter(|compression| compression.sample_encoding == crate::SAMPLE_ENCODING_TAG)
        .map(|compression| compression.ratio_zstd_3)
        .filter(|ratio| ratio.is_finite() && *ratio >= 1.0)
        .map(default_entropy_for_ratio)
        .unwrap_or_else(|| {
            let text_like = table
                .cols
                .values()
                .filter(|col| is_text_type(&normalized_type(&col.column_type)))
                .count();
            let binary_like = table
                .cols
                .values()
                .filter(|col| is_binary_type(&normalized_type(&col.column_type)))
                .count();
            let lob_like = table
                .cols
                .values()
                .filter(|col| is_lob_like(&normalized_type(&col.column_type)))
                .count();
            if binary_like > 0 && text_like == 0 {
                0.75
            } else if lob_like > 0 || text_like >= 4 {
                0.35
            } else if text_like > 0 {
                0.50
            } else {
                0.80
            }
        })
}

pub fn default_entropy_for_ratio(ratio: f64) -> f64 {
    if ratio >= 32.0 {
        0.02
    } else if ratio >= 16.0 {
        0.06
    } else if ratio >= 8.0 {
        0.12
    } else if ratio >= 4.0 {
        0.22
    } else if ratio >= 2.0 {
        0.45
    } else {
        0.75
    }
}

pub fn normalized_type(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

pub fn is_boolean_type(ty: &str) -> bool {
    matches!(ty, "bool" | "boolean" | "bit")
}

fn is_year_column(ty: &str, column: &BlueprintColumn) -> bool {
    ty == "year" || normalized_type(&column.native_type).starts_with("year")
}

fn is_bit_column(ty: &str, column: &BlueprintColumn) -> bool {
    (ty == "bit" || normalized_type(&column.native_type).starts_with("bit")) && column.bit_width > 1
}

pub fn is_null_type(ty: &str) -> bool {
    matches!(ty, "null" | "null-only")
}

pub fn is_integer_type(ty: &str) -> bool {
    matches!(
        ty,
        "tinyint"
            | "smallint"
            | "int2"
            | "int"
            | "integer"
            | "int4"
            | "serial"
            | "bigint"
            | "int8"
            | "bigserial"
            | "identity"
            | "number"
            | "long"
    )
}

pub fn is_numeric_type(ty: &str) -> bool {
    is_integer_type(ty)
        || matches!(
            ty,
            "numeric"
                | "decimal"
                | "money"
                | "float"
                | "float4"
                | "real"
                | "double"
                | "float8"
                | "double precision"
        )
}

pub fn is_temporal_type(ty: &str) -> bool {
    matches!(
        ty,
        "date"
            | "time"
            | "datetime"
            | "datetime2"
            | "timestamp"
            | "timestamptz"
            | "timestamp with time zone"
    )
}

pub fn is_text_type(ty: &str) -> bool {
    ty.contains("char")
        || ty.contains("text")
        || ty.contains("json")
        || ty.contains("xml")
        || ty == "string"
        || ty == "uuid"
}

pub fn is_binary_type(ty: &str) -> bool {
    ty.contains("binary")
        || ty.contains("blob")
        || ty == "bytea"
        || ty == "image"
        || ty == "bytes"
        || ty == "varbinary"
}

pub fn is_lob_like(ty: &str) -> bool {
    ty.contains("text")
        || ty.contains("max")
        || ty.contains("clob")
        || ty.contains("json")
        || ty.contains("xml")
        || ty.contains("blob")
}

fn generated_text_value(
    table: &BlueprintTable,
    column: &BlueprintColumn,
    row_idx: u64,
    col_idx: u64,
    max_value_bytes: u64,
    entropy: f64,
) -> String {
    let len = generated_value_len(table, column, row_idx, col_idx, max_value_bytes);
    let style = column.style.to_ascii_lowercase();
    let ty = normalized_type(column.column_type.as_str());
    if style.contains("json") || ty.contains("json") {
        return generated_json(row_idx, col_idx, len, entropy);
    }
    if style.contains("xml") || ty.contains("xml") {
        return generated_xml(row_idx, col_idx, len, entropy);
    }
    if is_binary_type(&ty) || style.contains("binary") || style.contains("base64") {
        return generated_base64(row_idx, col_idx, len, entropy);
    }
    if style.contains("hex") {
        return generated_alphabet(row_idx, col_idx, len, entropy, b"0123456789abcdef");
    }
    if style.contains("numeric") {
        return generated_alphabet(row_idx, col_idx, len, entropy, b"0123456789,.- ");
    }
    if ty == "string"
        || column.charset.to_ascii_lowercase().contains("utf")
        || column.native_type.to_ascii_lowercase().contains("nvarchar")
    {
        return generated_utf8(row_idx, col_idx, len, entropy);
    }
    generated_text(row_idx, col_idx, len, entropy)
}

fn generated_utf8(row_idx: u64, col_idx: u64, len: usize, entropy: f64) -> String {
    const MULTIBYTE: &[&[u8]] = &[
        "é".as_bytes(),
        "λ".as_bytes(),
        "界".as_bytes(),
        "語".as_bytes(),
    ];
    const ASCII: &[u8] = b"customer data content value status title body language ";
    let mut out = Vec::with_capacity(len);
    let mut state = synthetic_seed(0, row_idx, col_idx);
    while out.len() < len {
        state = mix64(state.wrapping_add(out.len() as u64));
        let remaining = len - out.len();
        let noise = ((state >> 32) % 10_000) as f64 / 10_000.0;
        let use_noise = noise < entropy.clamp(0.0, 1.0);
        let use_multibyte =
            remaining >= 2 && use_noise && ((state >> 16) % 10_000) as f64 / 10_000.0 < 0.35;
        if use_multibyte {
            let mut appended = false;
            for offset in 0..MULTIBYTE.len() {
                let value = MULTIBYTE[(state as usize + offset) % MULTIBYTE.len()];
                if value.len() <= remaining {
                    out.extend_from_slice(value);
                    appended = true;
                    break;
                }
            }
            if appended {
                continue;
            }
        }
        out.push(if use_noise {
            ASCII[state as usize % ASCII.len()]
        } else {
            ASCII[out.len() % ASCII.len()]
        });
    }
    String::from_utf8(out).expect("generated UTF-8 alphabet is valid")
}

fn generated_binary(seed: u64, len: usize, entropy: f64) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    let mut state = seed;
    let entropy = entropy.clamp(0.0, 1.0);
    for idx in 0..len {
        state = mix64(state.wrapping_add(idx as u64));
        let noise = ((state >> 32) % 10_000) as f64 / 10_000.0;
        out.push(if noise < entropy {
            state as u8
        } else {
            b"DBWARP"[idx % 6]
        });
    }
    out
}

fn generated_numeric(seed: u64, column: &BlueprintColumn) -> String {
    let precision = if column.numeric_precision == 0 {
        18
    } else {
        column.numeric_precision.min(18)
    } as u32;
    let scale = if column.numeric_precision == 0 && column.numeric_scale == 0 {
        6
    } else {
        column.numeric_scale.min(precision as u64)
    } as u32;
    let modulus = 10_u64.pow(precision);
    let value = seed % modulus;
    if scale == 0 {
        value.to_string()
    } else {
        let divisor = 10_u64.pow(scale);
        format!(
            "{}.{:0width$}",
            value / divisor,
            value % divisor,
            width = scale as usize
        )
    }
}

fn generated_integer(seed: u64, ty: &str, column: &BlueprintColumn) -> String {
    let bit_width = integer_bit_width(ty, column);
    if column.numeric_unsigned {
        if bit_width == 64 {
            seed.to_string()
        } else {
            let modulus = 1_u64 << bit_width;
            (seed % modulus).to_string()
        }
    } else if bit_width == 64 {
        (seed as i64).to_string()
    } else {
        let modulus = 1_u64 << bit_width;
        let midpoint = 1_i128 << (bit_width - 1);
        (i128::from(seed % modulus) - midpoint).to_string()
    }
}

fn integer_bit_width(ty: &str, column: &BlueprintColumn) -> u32 {
    if (1..=64).contains(&column.bit_width) {
        return column.bit_width as u32;
    }
    match ty {
        "tinyint" => 8,
        "smallint" | "int2" => 16,
        "int" | "integer" | "int4" | "serial" => 32,
        _ => 64,
    }
}

fn generated_year(seed: u64) -> String {
    (1901 + seed % 255).to_string()
}

fn generated_bit(seed: u64, bit_width: u64) -> String {
    let bit_width = bit_width.clamp(1, 64) as u32;
    if bit_width == 64 {
        seed.to_string()
    } else {
        (seed % (1_u64 << bit_width)).to_string()
    }
}

/// Prefix a generated UTF-8 value with a fixed-width ASCII uniqueness token
/// while respecting independent character and byte capacities.
pub fn prefix_unique_utf8_value(
    value: &str,
    row_idx: u64,
    token_width: usize,
    max_chars: usize,
    max_bytes: usize,
) -> Option<String> {
    if token_width == 0 || max_chars == 0 || max_bytes == 0 {
        return None;
    }
    let token = base36_token_string(row_idx, token_width)?;
    if token.len() > max_bytes || token.chars().count() > max_chars {
        return None;
    }

    let mut output = String::with_capacity(max_bytes.min(value.len().saturating_add(token.len())));
    output.push_str(&token);
    let mut chars = token.chars().count();
    if chars < max_chars && output.len() < max_bytes && !value.is_empty() {
        output.push('_');
        chars += 1;
    }
    for character in value.chars() {
        if chars >= max_chars || output.len().saturating_add(character.len_utf8()) > max_bytes {
            break;
        }
        output.push(character);
        chars += 1;
    }
    Some(output)
}

fn base36_token_string(mut value: u64, width: usize) -> Option<String> {
    const DIGITS: &[u8; 36] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut token = vec![b'0'; width];
    for slot in token.iter_mut().rev() {
        *slot = DIGITS[(value % 36) as usize];
        value /= 36;
    }
    if value != 0 {
        return None;
    }
    String::from_utf8(token).ok()
}

fn generated_uuid(seed: u64) -> String {
    let high = mix64(seed).to_be_bytes();
    let low = mix64(seed ^ 0xa5a5_5a5a_d3c4_b2e1).to_be_bytes();
    let mut bytes = [0_u8; 16];
    bytes[..8].copy_from_slice(&high);
    bytes[8..].copy_from_slice(&low);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
}

fn generated_text(row_idx: u64, col_idx: u64, len: usize, entropy: f64) -> String {
    const LOW: &[u8] = b"content node field value menu user status published path alias taxonomy body title site paragraph block view revision language default ";
    const HIGH: &[u8] =
        b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789     .,:;/-_";
    let alphabet = if entropy < 0.35 { LOW } else { HIGH };
    generated_alphabet(row_idx, col_idx, len, entropy, alphabet)
}

fn generated_json(row_idx: u64, col_idx: u64, len: usize, entropy: f64) -> String {
    let prefix =
        format!("{{\"id\":{row_idx},\"column\":{col_idx},\"status\":\"active\",\"payload\":\"");
    fill_wrapped(prefix, "\"}".to_string(), row_idx, col_idx, len, entropy)
}

fn generated_xml(row_idx: u64, col_idx: u64, len: usize, entropy: f64) -> String {
    let prefix = format!("<row id=\"{row_idx}\" column=\"{col_idx}\"><payload>");
    fill_wrapped(
        prefix,
        "</payload></row>".to_string(),
        row_idx,
        col_idx,
        len,
        entropy,
    )
}

fn generated_base64(row_idx: u64, col_idx: u64, len: usize, entropy: f64) -> String {
    generated_alphabet(
        row_idx,
        col_idx,
        len,
        entropy,
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/",
    )
}

fn generated_alphabet(
    row_idx: u64,
    col_idx: u64,
    len: usize,
    entropy: f64,
    alphabet: &[u8],
) -> String {
    let mut out = Vec::with_capacity(len);
    let mut x = synthetic_seed(0, row_idx, col_idx);
    let e = entropy.clamp(0.0, 1.0);
    for idx in 0..len {
        x = mix64(x.wrapping_add(idx as u64));
        let noise = ((x >> 32) % 10_000) as f64 / 10_000.0;
        let byte = if noise < e {
            alphabet[(x as usize) % alphabet.len()]
        } else {
            alphabet[idx % alphabet.len()]
        };
        out.push(byte);
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn fill_wrapped(
    prefix: String,
    suffix: String,
    row_idx: u64,
    col_idx: u64,
    len: usize,
    entropy: f64,
) -> String {
    if len <= prefix.len() + suffix.len() {
        let mut out = prefix;
        out.push_str(&suffix);
        out.truncate(len);
        return out;
    }
    let body_len = len - prefix.len() - suffix.len();
    let mut out = prefix;
    out.push_str(&generated_text(row_idx, col_idx, body_len, entropy));
    out.push_str(&suffix);
    out
}

fn generated_bool(seed: u64, entropy: f64) -> bool {
    if entropy <= 0.1 {
        seed.checked_rem(20) == Some(0)
    } else {
        seed & 1 == 0
    }
}

fn generated_date(seed: u64) -> String {
    let day = (seed % 28) + 1;
    let month = ((seed / 29) % 12) + 1;
    let year = 2020 + ((seed / 997) % 7);
    format!("{year:04}-{month:02}-{day:02}")
}

fn generated_time(seed: u64) -> String {
    let second = seed % 60;
    let minute = (seed / 61) % 60;
    let hour = (seed / 3_661) % 24;
    format!("{hour:02}:{minute:02}:{second:02}")
}

fn should_emit_null(column: &BlueprintColumn, seed: u64, null_percent: u8) -> bool {
    if !column.nullable {
        return false;
    }
    let fraction = column
        .null_fraction
        .unwrap_or(f64::from(null_percent.min(100)) / 100.0)
        .clamp(0.0, 1.0);
    fraction > 0.0 && (seed % 1_000_000) as f64 / 1_000_000.0 < fraction
}

fn sane_len_hint(col: &BlueprintColumn) -> Option<u64> {
    (1..=1_048_576)
        .contains(&col.len_avg)
        .then_some(col.len_avg)
}

fn sane_len_hint_p95(col: &BlueprintColumn) -> Option<u64> {
    (1..=1_048_576)
        .contains(&col.len_p95)
        .then_some(col.len_p95)
}

fn estimate_row_bytes_from_columns(table: &BlueprintTable) -> u64 {
    table
        .cols
        .values()
        .map(|col| {
            sane_len_hint(col).unwrap_or_else(|| {
                let ty = normalized_type(&col.column_type);
                if is_boolean_type(&ty) {
                    1
                } else if is_integer_type(&ty) || is_numeric_type(&ty) || is_temporal_type(&ty) {
                    8
                } else if is_binary_type(&ty) || is_lob_like(&ty) {
                    512
                } else {
                    32
                }
            })
        })
        .sum::<u64>()
        .max(1)
}

fn synthetic_seed(table_idx: u64, row_idx: u64, col_idx: u64) -> u64 {
    mix64(
        table_idx
            .wrapping_mul(0x9e37_79b9_7f4a_7c15)
            .wrapping_add(row_idx.wrapping_mul(0xbf58_476d_1ce4_e5b9))
            .wrapping_add(col_idx.wrapping_mul(0x94d0_49bb_1331_11eb)),
    )
}

pub fn mix64(mut x: u64) -> u64 {
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BlueprintCardinality, BlueprintColumn, BlueprintCompression, BlueprintRelationship,
        BlueprintTable,
    };
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn generated_json_respects_value_cap() {
        let mut table = BlueprintTable {
            rows: 100,
            table_bytes: 100_000,
            ..Default::default()
        };
        let column = BlueprintColumn {
            ordinal: 1,
            column_type: "json".to_string(),
            nullable: false,
            len_avg: 512,
            len_p95: 1024,
            style: "json".to_string(),
            compression: Some(BlueprintCompression {
                ratio_zstd_3: 6.0,
                ..Default::default()
            }),
            ..Default::default()
        };
        table.cols.insert("col-1".to_string(), column.clone());
        let value = blueprint_row_value(
            &table,
            &column,
            0,
            42,
            0,
            SyntheticOptions {
                max_value_bytes: 128,
                null_percent: 3,
            },
        )
        .unwrap();
        assert!(value.len() <= 128);
    }

    #[test]
    fn generated_text_never_exceeds_the_declared_column_width() {
        let mut table = BlueprintTable {
            rows: 100,
            table_bytes: 100_000,
            ..Default::default()
        };
        let column = BlueprintColumn {
            ordinal: 1,
            column_type: "text".to_string(),
            native_type: "character varying(2)".to_string(),
            nullable: false,
            declared_max_chars: 2,
            len_avg: 0,
            len_p95: 0,
            ..Default::default()
        };
        table.cols.insert("col-1".to_string(), column.clone());
        let options = SyntheticOptions {
            max_value_bytes: 64 * 1024,
            null_percent: 0,
        };

        for row_idx in 0..100 {
            let value = blueprint_row_value(&table, &column, 0, row_idx, 0, options)
                .expect("NOT NULL text must produce a value");
            assert!(value.len() <= 2, "row {row_idx} exceeded VARCHAR(2)");
        }
    }

    #[test]
    fn structured_null_uuid_binary_decimal_and_utf8_values_preserve_type() {
        let table = BlueprintTable {
            rows: 100,
            table_bytes: 25_600,
            ..Default::default()
        };
        let options = SyntheticOptions {
            max_value_bytes: 1024,
            null_percent: 0,
        };

        let null_only = BlueprintColumn {
            column_type: "null".into(),
            nullable: true,
            null_fraction: Some(1.0),
            ..Default::default()
        };
        assert!(blueprint_row_value(&table, &null_only, 0, 0, 0, options).is_none());

        let uuid = BlueprintColumn {
            column_type: "uuid".into(),
            len_avg: 36,
            len_p95: 36,
            ..Default::default()
        };
        let uuid_value =
            String::from_utf8(blueprint_row_value(&table, &uuid, 0, 0, 1, options).unwrap())
                .unwrap();
        assert_eq!(uuid_value.len(), 36);
        assert_eq!(&uuid_value[14..15], "4");
        assert!(matches!(&uuid_value[19..20], "8" | "9" | "a" | "b"));

        let binary = BlueprintColumn {
            column_type: "bytes".into(),
            len_avg: 64,
            len_p95: 64,
            ..Default::default()
        };
        let binary_value = blueprint_row_value(&table, &binary, 0, 0, 2, options).unwrap();
        assert_eq!(binary_value.len(), 64);
        assert_eq!(
            binary_value,
            generated_binary(
                synthetic_seed(0, 0, 2),
                64,
                entropy_for_column(&table, &binary)
            )
        );

        let decimal = BlueprintColumn {
            column_type: "decimal".into(),
            numeric_precision: 18,
            numeric_scale: 5,
            ..Default::default()
        };
        let decimal_value =
            String::from_utf8(blueprint_row_value(&table, &decimal, 0, 0, 3, options).unwrap())
                .unwrap();
        assert_eq!(decimal_value.rsplit_once('.').unwrap().1.len(), 5);

        let string = BlueprintColumn {
            column_type: "string".into(),
            native_type: "parquet:string".into(),
            len_avg: 257,
            len_p95: 257,
            ..Default::default()
        };
        let string_value = blueprint_row_value(&table, &string, 0, 0, 4, options).unwrap();
        let string_text = std::str::from_utf8(&string_value).unwrap();
        assert_eq!(string_value.len(), 257);
        assert!(!string_text.is_ascii());
    }

    #[test]
    fn structured_null_fraction_and_transport_provenance_are_respected() {
        let table = BlueprintTable {
            rows: 10,
            table_bytes: 100,
            ..Default::default()
        };
        let never_null = BlueprintColumn {
            column_type: "string".into(),
            nullable: true,
            null_fraction: Some(0.0),
            len_avg: 8,
            ..Default::default()
        };
        let always_null = BlueprintColumn {
            null_fraction: Some(1.0),
            ..never_null.clone()
        };
        let options = SyntheticOptions {
            max_value_bytes: 64,
            null_percent: 100,
        };
        for row in 0..10 {
            assert!(blueprint_row_value(&table, &never_null, 0, row, 0, options).is_some());
            assert!(blueprint_row_value(&table, &always_null, 0, row, 1, options).is_none());
        }

        let storage_only = BlueprintColumn {
            compression: Some(BlueprintCompression {
                ratio_zstd_3: 32.0,
                ratio_storage: 32.0,
                sample_encoding: "parquet-file".into(),
                ..Default::default()
            }),
            ..never_null.clone()
        };
        let transport = BlueprintColumn {
            compression: Some(BlueprintCompression {
                sample_encoding: crate::SAMPLE_ENCODING_TAG.into(),
                ..storage_only.compression.clone().unwrap()
            }),
            ..never_null
        };
        assert_eq!(
            entropy_from_column(&storage_only),
            default_entropy_for_ratio(3.0)
        );
        assert_eq!(
            entropy_from_column(&transport),
            default_entropy_for_ratio(32.0)
        );
    }

    #[test]
    fn table_transport_compression_is_used_when_column_measurement_is_absent() {
        let table = BlueprintTable {
            compression: Some(BlueprintCompression {
                measured: true,
                ratio_zstd_3: 9.0,
                sample_encoding: crate::SAMPLE_ENCODING_TAG.into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let column = BlueprintColumn::default();
        assert_eq!(
            entropy_for_column(&table, &column),
            default_entropy_for_ratio(9.0)
        );
    }

    #[test]
    fn table_calibration_preserves_per_column_entropy_differences() {
        let table = BlueprintTable {
            compression: Some(BlueprintCompression {
                ratio_zstd_3: 3.0,
                sample_encoding: crate::SAMPLE_ENCODING_TAG.into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let compressible = BlueprintColumn {
            compression: Some(BlueprintCompression {
                ratio_zstd_3: 9.0,
                sample_encoding: crate::SAMPLE_ENCODING_TAG.into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let incompressible = BlueprintColumn {
            compression: Some(BlueprintCompression {
                ratio_zstd_3: 1.2,
                sample_encoding: crate::SAMPLE_ENCODING_TAG.into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let baseline = default_entropy_for_table(&table);
        let low = entropy_for_column_with_table_calibration(&table, &compressible, baseline);
        let high = entropy_for_column_with_table_calibration(&table, &incompressible, baseline);

        assert!((low - entropy_for_column(&table, &compressible)).abs() < f64::EPSILON);
        assert!((high - entropy_for_column(&table, &incompressible)).abs() < f64::EPSILON);
        assert!(low < high);
    }

    #[test]
    fn statistical_projection_guarantees_domain_and_hot_value_mass() {
        let table = BlueprintTable {
            rows: 1_000,
            ..Default::default()
        };
        let column = BlueprintColumn {
            cardinality: Some(BlueprintCardinality {
                measured: true,
                sample_rows: 1_000,
                non_null_rows: 1_000,
                observed_distinct_count: 10,
                estimated_distinct_count: 10,
                top_value_fraction: 0.20,
                frequency_p50: 40,
                frequency_p95: 100,
                frequency_p99: 150,
                frequency_max: 200,
                ..Default::default()
            }),
            ..Default::default()
        };
        let projected = (0..table.rows)
            .map(|row| statistical_value_row_index(&table, &column, row, 7))
            .collect::<Vec<_>>();
        assert_eq!(projected.iter().copied().collect::<BTreeSet<_>>().len(), 10);
        assert_eq!(projected.iter().filter(|value| **value == 0).count(), 200);
        assert!(projected.iter().all(|value| *value < 10));
    }

    #[test]
    fn statistical_projection_scales_distinct_domain_with_fixture_rows() {
        let table = BlueprintTable {
            rows: 100,
            ..Default::default()
        };
        let column = BlueprintColumn {
            cardinality: Some(BlueprintCardinality {
                measured: true,
                sample_rows: 100,
                non_null_rows: 100,
                observed_distinct_count: 10,
                estimated_distinct_count: 10,
                frequency_p50: 1,
                frequency_p95: 1,
                frequency_p99: 1,
                frequency_max: 1,
                ..Default::default()
            }),
            ..Default::default()
        };

        let downscaled = (0..50)
            .map(|row| statistical_value_row_index_for_generated_rows(&table, &column, 50, row, 0))
            .collect::<BTreeSet<_>>();
        let upscaled = (0..200)
            .map(|row| statistical_value_row_index_for_generated_rows(&table, &column, 200, row, 0))
            .collect::<BTreeSet<_>>();

        assert_eq!(downscaled.len(), 5);
        assert_eq!(upscaled.len(), 20);
    }

    #[test]
    fn relationship_projection_preserves_nulls_coverage_and_fanout() {
        let statistics = BlueprintRelationship {
            measured: true,
            sample_rows: 1_000,
            non_null_rows: 800,
            distinct_parent_values: 25,
            parent_coverage_fraction: 0.25,
            fanout_p50: 12,
            fanout_p95: 24,
            fanout_p99: 80,
            fanout_max: 200,
            ..Default::default()
        };
        let projected = (0..1_000)
            .map(|row| relationship_parent_row_index(Some(&statistics), row, 1_000, 100, 3, 0.0))
            .collect::<Vec<_>>();
        assert_eq!(
            projected.iter().filter(|value| value.is_none()).count(),
            200
        );
        let parents = projected.into_iter().flatten().collect::<Vec<_>>();
        assert!(parents.iter().all(|parent| *parent < 25));
        assert_eq!(parents.iter().copied().collect::<BTreeSet<_>>().len(), 25);
        assert_eq!(parents.iter().filter(|parent| **parent == 0).count(), 200);
    }

    #[test]
    fn generated_rows_reproduce_null_length_cardinality_and_hot_value_fidelity() {
        let generated_rows = 10_000u64;
        let length_profile = BlueprintColumn {
            ordinal: 1,
            column_type: "text".into(),
            nullable: true,
            null_fraction: Some(0.20),
            len_avg: 12,
            len_p95: 30,
            ..Default::default()
        };
        let categorical = BlueprintColumn {
            ordinal: 2,
            column_type: "text".into(),
            nullable: false,
            len_avg: 16,
            len_p95: 16,
            cardinality: Some(BlueprintCardinality {
                measured: true,
                sample_rows: generated_rows,
                non_null_rows: generated_rows,
                observed_distinct_count: 100,
                estimated_distinct_count: 100,
                top_value_fraction: 0.10,
                frequency_p50: 20,
                frequency_p95: 80,
                frequency_p99: 100,
                frequency_max: 1_000,
                ..Default::default()
            }),
            ..Default::default()
        };
        let table = BlueprintTable {
            rows: generated_rows,
            table_bytes: generated_rows * 40,
            cols: BTreeMap::from([
                ("col-1".to_string(), length_profile.clone()),
                ("col-2".to_string(), categorical.clone()),
            ]),
            ..Default::default()
        };
        let options = SyntheticOptions {
            max_value_bytes: 1_024,
            null_percent: 0,
        };
        let mut nulls = 0u64;
        let mut lengths = Vec::new();
        let mut frequencies = BTreeMap::<Vec<u8>, u64>::new();
        for row in 0..generated_rows {
            match blueprint_row_value_for_generated_rows_with_entropy(
                &table,
                &length_profile,
                generated_rows,
                0,
                row,
                0,
                options,
                Some(0.5),
            ) {
                Some(value) => lengths.push(value.len() as u64),
                None => nulls += 1,
            }
            let category = blueprint_row_value_for_generated_rows_with_entropy(
                &table,
                &categorical,
                generated_rows,
                0,
                row,
                1,
                options,
                Some(0.5),
            )
            .unwrap();
            *frequencies.entry(category).or_default() += 1;
        }

        let null_fraction = nulls as f64 / generated_rows as f64;
        assert!((null_fraction - 0.20).abs() <= 0.02, "{null_fraction}");

        lengths.sort_unstable();
        let average = lengths.iter().sum::<u64>() as f64 / lengths.len() as f64;
        let p95 = lengths[((lengths.len() as f64 * 0.95).ceil() as usize) - 1];
        assert!((average - 12.0).abs() / 12.0 <= 0.05, "{average}");
        assert_eq!(p95, 30);

        assert_eq!(frequencies.len(), 100);
        let hottest = frequencies.values().copied().max().unwrap();
        assert_eq!(hottest, 1_000);
    }

    #[test]
    fn distribution_projection_properties_hold_across_boundary_domains() {
        for rows in [1u64, 2, 49, 50, 127, 128, 999, 1_000] {
            for requested_domain in [1, 2, 7, rows, rows.saturating_add(1)] {
                for hot_fraction in [0.0, 0.01, 0.5, 1.0] {
                    let domain = requested_domain.clamp(1, rows);
                    let values = (0..rows)
                        .map(|row| {
                            project_distribution_index(
                                row,
                                rows,
                                requested_domain,
                                hot_fraction,
                                [1, 2, 4],
                                17,
                            )
                        })
                        .collect::<BTreeSet<_>>();
                    assert!(values.iter().all(|value| *value < domain));
                    assert_eq!(values.len() as u64, domain);
                }
            }
        }
    }

    #[test]
    fn entropy_calibration_searches_continuous_values_and_returns_best_observation() {
        let calibration =
            calibrate_entropy(4.0, 0.73, 16, 0.001, |entropy| Ok(8.0 - entropy * 6.0)).unwrap();
        assert!(calibration.matched);
        assert!((calibration.entropy - (2.0 / 3.0)).abs() < 0.01);
        assert!((calibration.observed_ratio - 4.0).abs() < 0.01);
        assert!(calibration.observations >= 3);
    }

    #[test]
    fn explicit_entropy_override_changes_generated_payload_without_changing_nulls() {
        let table = BlueprintTable {
            rows: 10,
            table_bytes: 10_000,
            ..Default::default()
        };
        let column = BlueprintColumn {
            column_type: "text".into(),
            len_avg: 512,
            null_fraction: Some(0.0),
            ..Default::default()
        };
        let low = blueprint_row_value_with_entropy(
            &table,
            &column,
            0,
            3,
            0,
            SyntheticOptions::default(),
            Some(0.0),
        )
        .unwrap();
        let high = blueprint_row_value_with_entropy(
            &table,
            &column,
            0,
            3,
            0,
            SyntheticOptions::default(),
            Some(1.0),
        )
        .unwrap();
        assert_ne!(low, high);
        assert_eq!(low.len(), high.len());
    }

    #[test]
    fn ordered_columns_sorts_by_ordinal_then_name() {
        let mut cols = BTreeMap::new();
        cols.insert(
            "b".to_string(),
            BlueprintColumn {
                ordinal: 2,
                ..Default::default()
            },
        );
        cols.insert(
            "a".to_string(),
            BlueprintColumn {
                ordinal: 1,
                ..Default::default()
            },
        );
        let table = BlueprintTable {
            cols,
            ..Default::default()
        };
        let ordered = ordered_columns(&table);
        assert_eq!(ordered[0].0, "a");
        assert_eq!(ordered[1].0, "b");
    }

    #[test]
    fn generated_integer_values_respect_signed_and_unsigned_widths() {
        let table = BlueprintTable::default();
        let options = SyntheticOptions {
            max_value_bytes: 64,
            null_percent: 0,
        };
        let signed = BlueprintColumn {
            column_type: "tinyint".into(),
            bit_width: 8,
            ..Default::default()
        };
        let unsigned = BlueprintColumn {
            numeric_unsigned: true,
            ..signed.clone()
        };
        for row in 0..10_000 {
            let signed_value = String::from_utf8(
                blueprint_row_value(&table, &signed, 0, row, 0, options).unwrap(),
            )
            .unwrap()
            .parse::<i16>()
            .unwrap();
            assert!((-128..=127).contains(&signed_value));

            let unsigned_value = String::from_utf8(
                blueprint_row_value(&table, &unsigned, 0, row, 1, options).unwrap(),
            )
            .unwrap()
            .parse::<u16>()
            .unwrap();
            assert!(unsigned_value <= 255);
        }
    }

    #[test]
    fn generated_year_and_multibit_values_stay_in_engine_domains() {
        let table = BlueprintTable::default();
        let options = SyntheticOptions {
            max_value_bytes: 64,
            null_percent: 0,
        };
        let year = BlueprintColumn {
            column_type: "year".into(),
            native_type: "year".into(),
            ..Default::default()
        };
        let bit = BlueprintColumn {
            column_type: "bit".into(),
            native_type: "bit(9)".into(),
            bit_width: 9,
            numeric_unsigned: true,
            ..Default::default()
        };
        for row in 0..10_000 {
            let year_value =
                String::from_utf8(blueprint_row_value(&table, &year, 0, row, 0, options).unwrap())
                    .unwrap()
                    .parse::<u16>()
                    .unwrap();
            assert!((1901..=2155).contains(&year_value));

            let bit_value =
                String::from_utf8(blueprint_row_value(&table, &bit, 0, row, 1, options).unwrap())
                    .unwrap()
                    .parse::<u16>()
                    .unwrap();
            assert!(bit_value <= 511);
        }
    }

    #[test]
    fn unique_utf8_prefix_respects_character_and_byte_limits() {
        let value = prefix_unique_utf8_value("界éabc", 35, 2, 4, 6).unwrap();
        assert_eq!(value, "0Z_界");
        assert_eq!(value.chars().count(), 4);
        assert_eq!(value.len(), 6);
        assert!(std::str::from_utf8(value.as_bytes()).is_ok());

        let byte_limited = prefix_unique_utf8_value("界éabc", 35, 2, 10, 5).unwrap();
        assert_eq!(byte_limited, "0Z_");
        assert!(std::str::from_utf8(byte_limited.as_bytes()).is_ok());
        assert!(prefix_unique_utf8_value("x", 36, 1, 10, 10).is_none());
    }

    #[test]
    fn generated_rowframe_uses_canonical_tags_and_varint_lengths() {
        let table = BlueprintTable {
            rows: 10,
            table_bytes: 10_000,
            ..Default::default()
        };
        let null_column = BlueprintColumn {
            column_type: "null".into(),
            nullable: true,
            null_fraction: Some(1.0),
            ..Default::default()
        };
        let text_column = BlueprintColumn {
            column_type: "text".into(),
            len_avg: 160,
            len_p95: 160,
            ..Default::default()
        };
        let mut row = Vec::new();
        append_synthetic_rowframe_row_with_entropy(
            &mut row,
            &table,
            &[&null_column, &text_column],
            0,
            2,
            SyntheticOptions::default(),
            Some(0.5),
        );
        assert_eq!(row[0], 0x00);
        assert_eq!(row[1], 0x01);
        let mut offset = 2;
        let mut shift = 0;
        let mut len = 0_u32;
        loop {
            let byte = row[offset];
            offset += 1;
            len |= u32::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                break;
            }
            shift += 7;
        }
        assert_eq!(row.len() - offset, len as usize);
        assert!(len > 127, "test must exercise a multi-byte varint");
    }
}
