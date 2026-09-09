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
    let mut columns = table
        .cols
        .iter()
        .filter(|(_, column)| column_is_transfer_value(column))
        .collect::<Vec<_>>();
    columns.sort_by_key(|(name, col)| (col.ordinal, name.as_str()));
    columns
}

/// Whether a source column contributes a value to a logical migration row.
/// Database-maintained generated/computed/system columns remain represented
/// in the Blueprint, but a statistical twin must not invent and transfer a
/// value that the real source adapter omits.
pub fn column_is_transfer_value(column: &BlueprintColumn) -> bool {
    !matches!(
        column.value_source.as_str(),
        "generated-stored"
            | "generated-virtual"
            | "computed-persisted"
            | "computed-virtual"
            | "system-time"
            | "rowversion"
    )
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
    let entropy = entropy_override
        .filter(|entropy| entropy.is_finite())
        .map(|entropy| entropy.clamp(0.0, 1.0))
        .unwrap_or_else(|| entropy_for_column(table, column));
    let value_row_idx = statistical_value_row_index_for_generated_rows_with_entropy(
        table,
        column,
        generated_row_count,
        row_idx,
        col_idx,
        entropy,
    );
    let generated_distinct_count =
        projected_statistical_distinct_count(table, column, generated_row_count);
    let seed = synthetic_seed(table_idx, value_row_idx, col_idx);
    let has_compression_profile = entropy_override.is_some()
        || column.compression.as_ref().is_some_and(|compression| {
            compression.sample_encoding == crate::SAMPLE_ENCODING_TAG
                && compression.ratio_zstd_3.is_finite()
                && compression.ratio_zstd_3 >= 1.0
        });
    let distribution_seed = if let Some(distinct_count) = generated_distinct_count {
        // A measured cardinality domain is semantic evidence. Entropy may
        // affect byte content and ordering, but it must never expand a
        // bounded source domain into unrelated 64-bit values.
        if is_integer_type(&ty) {
            // Row-order permutation alone leaves decimal digits in adjacent
            // domain order and makes high-cardinality numeric data
            // artificially compressible. Relabel the finite domain with the
            // same bounded bijection: frequencies and cardinality remain
            // exact while entropy can also reproduce numeric digit disorder.
            entropy_permuted_cardinality_index(
                value_row_idx % distinct_count.max(1),
                distinct_count.max(1),
                synthetic_seed(table_idx, 0, col_idx) ^ 0x4e55_4d45_5249_4301,
                entropy,
            )
        } else {
            value_row_idx
        }
    } else if has_compression_profile {
        entropy_shaped_index(value_row_idx, table_idx, col_idx, entropy)
    } else {
        seed
    };
    if ty == "vector" {
        let dimension = float32_vector_dimension(column).unwrap_or(1);
        return Some(generated_float32_vector(seed, dimension, entropy));
    }
    if is_binary_type(&ty) {
        let len = generated_value_len(
            table,
            column,
            value_row_idx,
            col_idx,
            options.max_value_bytes,
        );
        return Some(if crate::is_precompressed_style(&column.style) {
            generated_precompressed_container(seed, len)
        } else {
            generated_binary(seed, len, entropy)
        });
    }
    let value = if is_year_column(&ty, column) {
        generated_year(distribution_seed)
    } else if is_bit_column(&ty, column) {
        generated_bit(seed, column.bit_width)
    } else if is_boolean_type(&ty) {
        if generated_bool(seed, entropy) {
            "1".to_string()
        } else {
            "0".to_string()
        }
    } else if is_integer_type(&ty) {
        generated_integer(distribution_seed, &ty, column)
    } else if is_numeric_type(&ty) {
        generated_numeric(
            distribution_seed,
            column,
            generated_distinct_count,
            synthetic_seed(table_idx, 0, col_idx) ^ 0x4e55_4d45_5249_4302,
            entropy,
        )
    } else if matches!(
        ty.as_str(),
        "float" | "float4" | "real" | "double" | "float8" | "double precision"
    ) {
        format!("{:.6}", (distribution_seed % 10_000_000) as f64 / 97.0)
    } else if ty == "date" {
        generated_date(distribution_seed)
    } else if ty == "time" {
        generated_time(distribution_seed, column.datetime_precision, entropy)
    } else if is_temporal_type(&ty) {
        generated_timestamp(
            distribution_seed,
            matches!(ty.as_str(), "timestamptz" | "timestamp with time zone"),
            column.datetime_precision,
            entropy,
        )
    } else if ty == "uuid" {
        generated_uuid(distribution_seed)
    } else {
        generated_text_value(
            table,
            column,
            value_row_idx,
            col_idx,
            options.max_value_bytes,
            entropy,
            generated_distinct_count,
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

/// Append one generated row using the canonical typed compression-probe
/// representation used by dbwarp-blueprint sampling. This lets every generator
/// calibrate synthetic entropy against the captured ratio without retaining
/// or reconstructing source values.
pub fn append_synthetic_probe_row_with_entropy(
    out: &mut Vec<u8>,
    table: &BlueprintTable,
    columns: &[&BlueprintColumn],
    table_idx: u64,
    row_idx: u64,
    options: SyntheticOptions,
    entropy_override: Option<f64>,
) {
    append_synthetic_probe_row_for_generated_rows_with_entropy(
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

/// Append one canonical probe row using the actual generated table size
/// for source-cardinality projection.
pub fn append_synthetic_probe_row_for_generated_rows_with_entropy(
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
            None => append_probe_cell(out, 0x00, None),
            Some(value) => append_probe_cell(
                out,
                synthetic_probe_type_tag(column.column_type.as_str()),
                Some(value.as_slice()),
            ),
        }
    }
}

/// Append a canonical probe row while applying a table-level calibration
/// as an offset to each column's own measured entropy. This preserves
/// per-column differences while still allowing the aggregate table ratio to be
/// matched.
pub fn append_synthetic_probe_row_for_generated_rows_with_table_entropy_calibration(
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
            None => append_probe_cell(out, 0x00, None),
            Some(value) => append_probe_cell(
                out,
                synthetic_probe_type_tag(column.column_type.as_str()),
                Some(value.as_slice()),
            ),
        }
    }
}

/// Return the canonical transient compression-probe tag for a normalized SQL
/// type. Frontends with their own Blueprint model use this rather than
/// duplicating the public measurement representation.
pub fn synthetic_probe_type_tag(column_type: &str) -> u8 {
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
    } else if ty == "vector" {
        0x11
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
pub fn append_probe_cell(out: &mut Vec<u8>, type_tag: u8, value: Option<&[u8]>) {
    let Some(value) = value else {
        out.push(0x00);
        return;
    };
    out.push(type_tag);
    let len = value.len().min(u32::MAX as usize);
    write_probe_varint(out, len as u32);
    out.extend_from_slice(&value[..len]);
}

fn write_probe_varint(out: &mut Vec<u8>, mut value: u32) {
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
    let mut sampled_entropies = Vec::with_capacity(iterations as usize * 3 + 3);
    for entropy in [0.0, 1.0, initial_entropy.clamp(0.0, 1.0)] {
        record_distinct_entropy_observation(
            entropy,
            target_ratio,
            &mut observe,
            &mut best,
            &mut observations,
            &mut sampled_entropies,
        )?;
    }

    // Generator ordering is intentionally discrete at finite-domain locality
    // boundaries, while byte-content entropy is continuous. Ratios therefore
    // need not be globally monotonic. Cover the complete domain first rather
    // than choosing one binary-search direction from the two endpoints.
    for step in 1..iterations {
        if best.relative_error <= tolerance {
            break;
        }
        record_distinct_entropy_observation(
            f64::from(step) / f64::from(iterations),
            target_ratio,
            &mut observe,
            &mut best,
            &mut observations,
            &mut sampled_entropies,
        )?;
    }

    // Refine both neighbours around the best sampled point. Recomputing the
    // bracket after each pair follows a non-monotonic local basin without
    // losing the bounded work guarantee.
    for _ in 0..iterations {
        if best.relative_error <= tolerance {
            break;
        }
        sampled_entropies.sort_by(f64::total_cmp);
        let best_index = sampled_entropies
            .iter()
            .position(|entropy| (*entropy - best.entropy).abs() <= f64::EPSILON)
            .unwrap_or(0);
        let left = if best_index == 0 {
            0.0
        } else {
            sampled_entropies[best_index - 1]
        };
        let right = sampled_entropies
            .get(best_index + 1)
            .copied()
            .unwrap_or(1.0);
        for entropy in [(left + best.entropy) / 2.0, (best.entropy + right) / 2.0] {
            record_distinct_entropy_observation(
                entropy,
                target_ratio,
                &mut observe,
                &mut best,
                &mut observations,
                &mut sampled_entropies,
            )?;
        }
    }
    best.observations = observations;
    best.matched = best.relative_error <= tolerance;
    Ok(best)
}

fn record_distinct_entropy_observation<F>(
    entropy: f64,
    target_ratio: f64,
    observe: &mut F,
    best: &mut EntropyCalibration,
    observations: &mut u32,
    sampled_entropies: &mut Vec<f64>,
) -> Result<()>
where
    F: FnMut(f64) -> Result<f64>,
{
    let entropy = entropy.clamp(0.0, 1.0);
    if sampled_entropies
        .iter()
        .any(|sampled| (*sampled - entropy).abs() <= f64::EPSILON)
    {
        return Ok(());
    }
    record_entropy_observation(entropy, target_ratio, observe, best, observations)?;
    sampled_entropies.push(entropy);
    Ok(())
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
            is_text_type(&ty) || is_binary_type(&ty) || ty == "vector"
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
    let p95 = sane_len_hint_p95(column).unwrap_or(len);
    if p95 > len {
        // A little over five percent of generated values must land in the p95
        // band. Using fewer than five percent makes the independently measured
        // nearest-rank p95 collapse back to the average-length band.
        if row_idx.checked_rem(19) == Some(0) {
            len = p95;
        } else if row_idx.wrapping_add(col_idx).checked_rem(11) == Some(0) {
            len = (len / 2).max(1);
        }
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
    shift_column_entropy_for_table_calibration(
        table,
        column,
        entropy_for_column(table, column),
        calibrated_table_entropy,
    )
}

/// Scale a column's already-calibrated entropy through the table baseline
/// while preserving column ordering. Both halves span their complete bounded
/// range: table entropy zero can make every unpinned column fully local, and
/// table entropy one can make every unpinned column fully disordered. An
/// additive shift clipped high-entropy columns before they reached either
/// endpoint and made valid aggregate compression ratios unreachable. A measured incompressible
/// binary/container profile is pinned: aggregate calibration must never make
/// JPEG/PNG/archive-like bytes artificially compressible.
pub fn shift_column_entropy_for_table_calibration(
    table: &BlueprintTable,
    column: &BlueprintColumn,
    column_entropy: f64,
    calibrated_table_entropy: f64,
) -> f64 {
    let column_entropy = column_entropy.clamp(0.0, 1.0);
    let ty = normalized_type(column.column_type.as_str());
    let pinned_incompressible = (is_binary_type(&ty)
        || crate::is_precompressed_style(&column.style))
        && column.compression.as_ref().is_some_and(|compression| {
            compression.sample_encoding == crate::SAMPLE_ENCODING_TAG
                && compression.ratio_zstd_3.is_finite()
                && compression.ratio_zstd_3 <= 1.1
        });
    if pinned_incompressible {
        return column_entropy;
    }
    let table_baseline = default_entropy_for_table(table);
    let table_entropy = calibrated_table_entropy.clamp(0.0, 1.0);
    if table_entropy <= table_baseline {
        if table_baseline <= f64::EPSILON {
            table_entropy
        } else {
            (column_entropy * table_entropy / table_baseline).clamp(0.0, 1.0)
        }
    } else if table_baseline >= 1.0 - f64::EPSILON {
        table_entropy
    } else {
        (column_entropy
            + (1.0 - column_entropy) * (table_entropy - table_baseline) / (1.0 - table_baseline))
            .clamp(0.0, 1.0)
    }
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
    statistical_value_row_index_for_generated_rows_with_entropy(
        table,
        column,
        generated_row_count,
        row_idx,
        col_idx,
        entropy_for_column(table, column),
    )
}

fn statistical_value_row_index_for_generated_rows_with_entropy(
    table: &BlueprintTable,
    column: &BlueprintColumn,
    generated_row_count: u64,
    row_idx: u64,
    col_idx: u64,
    entropy: f64,
) -> u64 {
    let Some(cardinality) = column
        .cardinality
        .as_ref()
        .filter(|cardinality| cardinality.measured)
    else {
        return row_idx;
    };
    project_scaled_cardinality_index_with_locality(
        table.rows,
        cardinality.observed_distinct_count,
        effective_source_distinct_count(table.rows, cardinality),
        cardinality.top_value_fraction,
        [
            cardinality.frequency_p50,
            cardinality.frequency_p95,
            cardinality.frequency_p99,
        ],
        generated_row_count,
        row_idx,
        col_idx,
        entropy,
        Some((cardinality.sample_rows, cardinality.observed_distinct_count)),
    )
}

/// Project the captured per-column cardinality into a generated row domain.
pub fn projected_statistical_distinct_count(
    table: &BlueprintTable,
    column: &BlueprintColumn,
    generated_row_count: u64,
) -> Option<u64> {
    let cardinality = column
        .cardinality
        .as_ref()
        .filter(|cardinality| cardinality.measured)?;
    let source_distinct = effective_source_distinct_count(table.rows, cardinality)
        .max(cardinality.observed_distinct_count)
        .min(table.rows.max(1));
    Some(scaled_distinct_count(
        table.rows,
        source_distinct,
        generated_row_count,
    ))
}

fn effective_source_distinct_count(
    source_row_count: u64,
    cardinality: &crate::BlueprintCardinality,
) -> u64 {
    let biased_near_unique_lower_bound = cardinality.sampled_with_bias
        && cardinality.non_null_rows > 0
        && cardinality.observed_distinct_count.saturating_mul(20)
            >= cardinality.non_null_rows.saturating_mul(19)
        && cardinality.estimated_distinct_count <= cardinality.observed_distinct_count;
    if !biased_near_unique_lower_bound || cardinality.sample_rows == 0 {
        return cardinality.estimated_distinct_count;
    }

    // Older Blueprints retained only the observed lower bound for biased
    // samples. Do not let one duplicate (or privacy quantization around the
    // sample size) collapse a near-unique many-million-row domain to a few
    // thousand values. New captures carry a continuous collision estimate in
    // `estimated_distinct_count`; this fallback is for the old lower-bound
    // contract only.
    let estimated_source_non_null = ((source_row_count as u128)
        .saturating_mul(cardinality.non_null_rows as u128)
        .saturating_add(cardinality.sample_rows as u128 / 2)
        / cardinality.sample_rows as u128)
        .min(u64::MAX as u128) as u64;
    cardinality
        .estimated_distinct_count
        .max(estimated_source_non_null)
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
    project_scaled_cardinality_index_with_locality(
        source_row_count,
        observed_distinct_count,
        estimated_distinct_count,
        top_value_fraction,
        frequency_quantiles,
        generated_row_count,
        row_idx,
        col_idx,
        1.0,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn project_scaled_cardinality_index_with_locality(
    source_row_count: u64,
    observed_distinct_count: u64,
    estimated_distinct_count: u64,
    top_value_fraction: f64,
    frequency_quantiles: [u64; 3],
    generated_row_count: u64,
    row_idx: u64,
    col_idx: u64,
    entropy: f64,
    locality_reference: Option<(u64, u64)>,
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
    let generated_distinct =
        scaled_distinct_count(source_row_count, source_distinct, generated_row_count);
    project_distribution_index_with_locality(
        row_idx,
        generated_row_count,
        generated_distinct,
        top_value_fraction,
        frequency_quantiles,
        col_idx ^ 0x434f_4c55_4d4e_0001,
        entropy,
        locality_reference,
    )
}

fn scaled_distinct_count(
    source_row_count: u64,
    source_distinct_count: u64,
    generated_row_count: u64,
) -> u64 {
    if generated_row_count == 0 {
        return 0;
    }
    if source_row_count == 0 {
        source_distinct_count.min(generated_row_count).max(1)
    } else {
        ((source_distinct_count as u128 * generated_row_count as u128
            + source_row_count as u128 / 2)
            / source_row_count as u128)
            .clamp(1, generated_row_count as u128) as u64
    }
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
    project_distribution_index_with_locality(
        row_idx,
        row_count,
        distinct_count,
        top_value_fraction,
        frequency_quantiles,
        salt,
        1.0,
        None,
    )
}

fn project_distribution_index_with_locality(
    row_idx: u64,
    row_count: u64,
    distinct_count: u64,
    top_value_fraction: f64,
    frequency_quantiles: [u64; 3],
    salt: u64,
    entropy: f64,
    locality_reference: Option<(u64, u64)>,
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
    let p50 = frequency_quantiles[0].max(1);
    let p95 = frequency_quantiles[1].max(p50);
    let p99 = frequency_quantiles[2].max(p95);
    let uniform_top_fraction = 1.0 / domain as f64;
    // Blueprint frequency fractions are deliberately privacy-quantized. A
    // mathematically uniform four-value domain can therefore be emitted as
    // 0.245 or 0.255 rather than exactly 0.25. Treat one full 0.005 bucket on
    // either side, plus floating-point representation slack, as uniform.
    const UNIFORM_TOP_FRACTION_TOLERANCE: f64 = 0.010_000_001;
    let uniform_frequency_profile = p99 <= p50.saturating_mul(2)
        && (top_value_fraction <= 0.0
            || (top_value_fraction - uniform_top_fraction).abs() <= UNIFORM_TOP_FRACTION_TOLERANCE);
    if uniform_frequency_profile {
        // Preserve the exact domain while varying only run locality. At zero
        // entropy each value occupies one broad run; at full entropy values
        // repeat in a compact cycle. Intermediate run lengths let the closed
        // loop compression calibrator reproduce source ordering without
        // retaining source values.
        let (group, group_count) =
            uniform_group_position(rows, domain, entropy, locality_reference, position);
        return uniform_group_index(group, group_count, domain, salt, entropy);
    }
    // Preserve the exact captured frequency allocation while making its row
    // order calibratable. This is a permutation of the finite row domain, so
    // changing entropy cannot create or remove occurrences of any bucket.
    let position = entropy_permuted_index(position, rows, salt ^ 0x4f52_4445_525f_0001, entropy);
    // Frequency evidence was observed inside a bounded sample. Repeating its
    // locality horizon prevents a broad run from growing from thousands to
    // millions of rows merely because the generated fixture is larger. A
    // domain wider than the sample remains the lower bound so every projected
    // value can still be represented.
    let locality_rows = locality_reference
        .map(|(reference_rows, _)| reference_rows.max(domain).min(rows))
        .unwrap_or(rows)
        .max(1);
    let locality_cycle = position / locality_rows;
    let locality_start = locality_cycle.saturating_mul(locality_rows);
    let local_rows = locality_rows
        .min(rows.saturating_sub(locality_start))
        .max(1);
    let local_position = position.saturating_sub(locality_start).min(local_rows - 1);
    let local_salt = salt ^ mix64(locality_cycle ^ 0x4c4f_4341_4c49_5459);
    let tail_domain = domain - 1;
    let requested_hot_rows =
        ((local_rows as f64) * top_value_fraction.clamp(0.0, 1.0)).round() as u64;
    let hot_rows = requested_hot_rows
        .max(1)
        .min(local_rows.saturating_sub(tail_domain).max(1));
    if local_position < hot_rows {
        return 0;
    }

    let tail_position = local_position - hot_rows;
    if tail_position < tail_domain {
        // Reserve one occurrence for every tail value before allocating
        // repeats according to the sampled frequency profile.
        return 1 + tail_position;
    }

    let repeated_position = tail_position - tail_domain;
    let repeated_rows = local_rows
        .saturating_sub(hot_rows)
        .saturating_sub(tail_domain);
    if repeated_rows == 0 {
        return 1 + repeated_position % tail_domain;
    }
    1 + weighted_tail_index(
        repeated_position,
        repeated_rows,
        tail_domain,
        frequency_quantiles,
        local_salt,
    )
}

fn uniform_run_length_target(
    rows: u64,
    domain: u64,
    entropy: f64,
    locality_reference: Option<(u64, u64)>,
) -> f64 {
    let maximum_run_length = locality_reference
        .filter(|(reference_rows, reference_domain)| *reference_rows > 0 && *reference_domain > 0)
        .map(|(reference_rows, reference_domain)| (reference_rows / reference_domain).max(1))
        .unwrap_or_else(|| (rows / domain).max(1));
    let locality = 1.0 - entropy.clamp(0.0, 1.0);
    1.0 + (maximum_run_length - 1) as f64 * locality * locality
}

fn uniform_group_position(
    rows: u64,
    domain: u64,
    entropy: f64,
    locality_reference: Option<(u64, u64)>,
    position: u64,
) -> (u64, u64) {
    const FRACTION_SCALE: u64 = 1_000_000;

    let target = uniform_run_length_target(rows, domain, entropy, locality_reference);
    let base = target.floor().max(1.0) as u64;
    let extra = ((target - base as f64) * FRACTION_SCALE as f64).round() as u64;
    let extra = extra.min(FRACTION_SCALE);
    let start = |group: u64| -> u64 {
        let value = u128::from(group)
            .saturating_mul(u128::from(base))
            .saturating_add(
                u128::from(group).saturating_mul(u128::from(extra)) / u128::from(FRACTION_SCALE),
            );
        value.min(u128::from(u64::MAX)) as u64
    };
    let denominator = u128::from(base)
        .saturating_mul(u128::from(FRACTION_SCALE))
        .saturating_add(u128::from(extra))
        .max(1);
    let estimate = u128::from(position).saturating_mul(u128::from(FRACTION_SCALE)) / denominator;
    let mut group = estimate.min(u128::from(u64::MAX)) as u64;
    while start(group.saturating_add(1)) <= position {
        group = group.saturating_add(1);
    }
    while group > 0 && start(group) > position {
        group -= 1;
    }

    let last_position = rows.saturating_sub(1);
    let count_estimate =
        u128::from(last_position).saturating_mul(u128::from(FRACTION_SCALE)) / denominator;
    let mut last_group = count_estimate.min(u128::from(u64::MAX)) as u64;
    while start(last_group.saturating_add(1)) <= last_position {
        last_group = last_group.saturating_add(1);
    }
    while last_group > 0 && start(last_group) > last_position {
        last_group -= 1;
    }
    (group, last_group.saturating_add(1).max(1))
}

fn entropy_permuted_index(index: u64, count: u64, salt: u64, entropy: f64) -> u64 {
    if count <= 1 {
        return 0;
    }
    let entropy = entropy.clamp(0.0, 1.0);
    if entropy <= f64::EPSILON {
        return index.min(count - 1);
    }

    // Grow the shuffled locality window from one item to the complete finite
    // domain. Interpolate between adjacent powers of two instead of rounding
    // entropy to a whole bit: the old staircase left common categorical
    // compression ratios unreachable at large row counts. Every arbitrary
    // window is still independently permuted by a bounded bijection, so exact
    // domain membership and bucket frequencies survive.
    let block_size = entropy_permutation_window(count, entropy);
    let block_start = (index / block_size) * block_size;
    let block_len = block_size.min(count - block_start);
    block_start + permute_bounded(index - block_start, block_len, salt ^ mix64(block_start))
}

fn entropy_permuted_index_power_of_two(index: u64, count: u64, salt: u64, entropy: f64) -> u64 {
    if count <= 1 {
        return 0;
    }
    let entropy = entropy.clamp(0.0, 1.0);
    if entropy <= f64::EPSILON {
        return index.min(count - 1);
    }
    let domain_bits = 64 - count.saturating_sub(1).leading_zeros();
    let shuffled_bits =
        ((f64::from(domain_bits) * entropy).round() as u32).clamp(1, domain_bits.max(1));
    let (block_start, block_len) = if shuffled_bits >= 64 {
        (0, count)
    } else {
        let block_size = 1_u64 << shuffled_bits;
        let block_start = (index / block_size) * block_size;
        (block_start, block_size.min(count - block_start))
    };
    block_start + permute_bounded(index - block_start, block_len, salt ^ mix64(block_start))
}

fn entropy_permutation_window(count: u64, entropy: f64) -> u64 {
    if count <= 1 || entropy <= f64::EPSILON {
        return 1;
    }
    if entropy >= 1.0 - f64::EPSILON {
        return count;
    }
    let domain_bits = 64 - count.saturating_sub(1).leading_zeros();
    let scaled_bits = f64::from(domain_bits) * entropy.clamp(0.0, 1.0);
    let lower_bits = scaled_bits.floor() as u32;
    let lower = 1_u64.checked_shl(lower_bits).unwrap_or(u64::MAX).min(count);
    if lower >= count {
        return count;
    }
    let upper = lower.saturating_mul(2).min(count);
    let fraction = scaled_bits - f64::from(lower_bits);
    let offset = ((upper - lower) as f64 * fraction).round() as u64;
    lower.saturating_add(offset).clamp(1, count)
}

/// Deterministically permute a finite cardinality domain as entropy rises.
///
/// This is the shared ordering primitive for consumers that must satisfy a
/// compound-key constraint while retaining Blueprint distribution fidelity.
/// It is a bijection over `0..count`, so callers can change order without
/// introducing duplicate or missing domain members.
pub fn entropy_permuted_cardinality_index(index: u64, count: u64, salt: u64, entropy: f64) -> u64 {
    entropy_permuted_index(index, count, salt, entropy)
}

fn uniform_group_index(group: u64, group_count: u64, domain: u64, salt: u64, entropy: f64) -> u64 {
    if domain <= 1 {
        return 0;
    }
    // Permute the finite group positions before projecting them onto the
    // domain. Because this is a bijection over every group position, all
    // captured frequencies (including a partial final cycle) remain exact.
    // A unique domain is still shuffled at non-zero entropy; keying the
    // shuffle window only from the number of repeated cycles accidentally
    // made unique domains permanently ordered.
    // Fractional run-length mixing already supplies the continuous locality
    // control for uniform distributions. Keep the stable power-of-two group
    // permutation here so a second continuous control does not destroy that
    // calibrated response curve.
    entropy_permuted_index_power_of_two(
        group.min(group_count.saturating_sub(1)),
        group_count.max(1),
        salt ^ 0x554e_4946_4f52_4d01,
        entropy,
    ) % domain
}

fn permute_bounded(value: u64, modulus: u64, salt: u64) -> u64 {
    if modulus <= 1 {
        return 0;
    }
    let bits = 64 - modulus.saturating_sub(1).leading_zeros();
    let mask = if bits >= 64 {
        u64::MAX
    } else {
        (1_u64 << bits) - 1
    };
    let mut candidate = value;
    loop {
        for round in 0..3_u64 {
            let round_salt = mix64(salt ^ round.wrapping_mul(0x9e37_79b9_7f4a_7c15));
            let shift = ((bits + round as u32 * 3) / 2).clamp(1, bits.clamp(1, 63));
            let multiplier = mix64(round_salt ^ 0x4d55_4c54_4950_4c59) | 1;
            let offset = mix64(round_salt ^ 0x4f46_4653_4554_0001) & mask;
            candidate = candidate.wrapping_add(offset) & mask;
            candidate ^= candidate >> shift;
            candidate = candidate.wrapping_mul(multiplier) & mask;
            candidate ^= candidate >> shift;
            candidate &= mask;
        }
        if candidate < modulus {
            return candidate;
        }
    }
}

fn rotated_position(row_idx: u64, row_count: u64, salt: u64) -> u64 {
    let rows = row_count.max(1);
    row_idx.wrapping_add(mix64(salt) % rows) % rows
}

fn weighted_tail_index(
    row_idx: u64,
    row_count: u64,
    domain: u64,
    frequency_quantiles: [u64; 3],
    salt: u64,
) -> u64 {
    if domain <= 1 || row_count == 0 {
        return 0;
    }
    let p50 = frequency_quantiles[0].max(1);
    let p95 = frequency_quantiles[1].max(p50);
    let p99 = frequency_quantiles[2].max(p95);

    let group_50 = ((domain as u128 * 50 + 99) / 100).max(1) as u64;
    let group_95 = ((domain as u128 * 45 + 99) / 100) as u64;
    let group_99 = domain.saturating_sub(group_50).saturating_sub(group_95);
    let weights = [(group_50, p50), (group_95, p95), (group_99, p99)];
    let total_weight = weights.iter().fold(0_u128, |total, (count, weight)| {
        total.saturating_add(*count as u128 * *weight as u128)
    });
    if total_weight == 0 {
        return row_idx % domain;
    }
    // Materialize the same weighted multiset in broad, deterministic runs.
    // The caller applies an entropy-controlled bijection to row positions, so
    // low entropy retains source-like locality while high entropy scatters the
    // identical frequency allocation. Hashing here made every skewed domain
    // maximally disordered even when entropy was zero.
    let selector = (u128::from(row_idx).saturating_mul(total_weight) / u128::from(row_count))
        .min(total_weight.saturating_sub(1));
    let mut offset = 0_u64;
    let mut cursor = 0_u128;
    for (count, weight) in weights {
        if count == 0 {
            continue;
        }
        let span = count as u128 * weight as u128;
        if selector < cursor.saturating_add(span) {
            let local = ((selector - cursor) / u128::from(weight)) as u64;
            return permute_bounded(
                (offset + local).min(domain - 1),
                domain,
                salt ^ 0x5745_4947_4854_0001,
            );
        }
        cursor = cursor.saturating_add(span);
        offset = offset.saturating_add(count);
    }
    permute_bounded(domain - 1, domain, salt ^ 0x5745_4947_4854_0001)
}

pub fn default_entropy_for_table(table: &BlueprintTable) -> f64 {
    table
        .compression
        .as_ref()
        .filter(|compression| {
            matches!(
                compression.sample_encoding.as_str(),
                crate::SAMPLE_ENCODING_TAG
                    | crate::TRANSFER_SAMPLE_ENCODING_TAG
                    | crate::TRANSFER_SAMPLE_STREAMING_CHUNKED_ENCODING_TAG
                    | crate::TRANSFER_SAMPLE_STREAMING_ENCODING_TAG
            )
        })
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
    if ratio <= 1.1 {
        1.0
    } else if ratio >= 32.0 {
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

fn normalized_type_base(ty: &str) -> &str {
    ty.split(['(', '[']).next().unwrap_or(ty).trim_end()
}

pub fn is_boolean_type(ty: &str) -> bool {
    matches!(normalized_type_base(ty), "bool" | "boolean" | "bit")
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
        normalized_type_base(ty),
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
            normalized_type_base(ty),
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
        normalized_type_base(ty),
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
    generated_distinct_count: Option<u64>,
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
    let value = if ty == "string"
        || column.charset.to_ascii_lowercase().contains("utf")
        || column.native_type.to_ascii_lowercase().contains("nvarchar")
    {
        generated_utf8(row_idx, col_idx, len, entropy)
    } else {
        generated_text(row_idx, col_idx, len, entropy)
    };
    generated_distinct_count
        .filter(|distinct| *distinct > 1)
        .and_then(|distinct| inject_statistical_domain_token(value.as_str(), row_idx, distinct))
        .unwrap_or(value)
}

fn inject_statistical_domain_token(
    value: &str,
    value_index: u64,
    distinct_count: u64,
) -> Option<String> {
    let width = decimal_width(distinct_count.saturating_sub(1)) as usize;
    let token = format!("{value_index:0width$}");
    if token.len() > value.len() {
        return None;
    }
    let mut output = String::with_capacity(value.len());
    let prefix_bytes = value.len() - token.len();
    for character in value.chars() {
        if output.len().saturating_add(character.len_utf8()) > prefix_bytes {
            break;
        }
        output.push(character);
    }
    output.extend(std::iter::repeat_n('x', prefix_bytes - output.len()));
    output.push_str(token.as_str());
    Some(output)
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

/// Recover a dense float32 vector dimension from its exact PostgreSQL binary
/// payload width. The transferable Blueprint exposes the neutral vector type
/// and byte width, never the source extension or type name.
pub fn float32_vector_dimension(column: &BlueprintColumn) -> Option<u16> {
    const HEADER_BYTES: u64 = 4;
    const ELEMENT_BYTES: u64 = 4;
    const MAX_DIMENSIONS: u64 = 16_000;
    let body_bytes = column.declared_max_bytes.checked_sub(HEADER_BYTES)?;
    if body_bytes == 0 || body_bytes % ELEMENT_BYTES != 0 {
        return None;
    }
    let dimension = body_bytes / ELEMENT_BYTES;
    if dimension > MAX_DIMENSIONS {
        return None;
    }
    u16::try_from(dimension).ok()
}

fn generated_float32_vector(seed: u64, dimension: u16, entropy: f64) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + usize::from(dimension) * 4);
    out.extend_from_slice(&dimension.to_be_bytes());
    out.extend_from_slice(&0_u16.to_be_bytes());
    let entropy = entropy.clamp(0.0, 1.0);
    let mut state = seed;
    for component in 0..dimension {
        state = mix64(state.wrapping_add(u64::from(component)));
        let noise = ((state >> 32) % 10_000) as f64 / 10_000.0;
        let value = if noise < entropy {
            // A bounded finite float distribution has the exponent/sign bias
            // of embedding vectors while retaining high mantissa entropy.
            ((state as u32) as f64 / u32::MAX as f64 * 2.0 - 1.0) as f32
        } else {
            (i32::from(component % 16) - 8) as f32 / 64.0
        };
        out.extend_from_slice(&value.to_bits().to_be_bytes());
    }
    out
}

/// Emit a deterministic, valid ZIP container with one stored high-entropy
/// member. The coarse Blueprint profile never reveals the source format, so a
/// neutral container preserves the relevant compression behavior without
/// pretending that the customer value was a particular image or archive.
fn generated_precompressed_container(seed: u64, len: usize) -> Vec<u8> {
    const FILE_NAME: &[u8] = b"payload.bin";
    const LOCAL_HEADER_BYTES: usize = 30;
    const CENTRAL_HEADER_BYTES: usize = 46;
    const END_RECORD_BYTES: usize = 22;
    const OVERHEAD: usize = LOCAL_HEADER_BYTES
        + FILE_NAME.len()
        + CENTRAL_HEADER_BYTES
        + FILE_NAME.len()
        + END_RECORD_BYTES;

    if len < OVERHEAD || len.saturating_sub(OVERHEAD) > u32::MAX as usize {
        return generated_binary(seed, len, 1.0);
    }
    let payload = generated_binary(seed, len - OVERHEAD, 1.0);
    let payload_len = payload.len() as u32;
    let checksum = crc32(&payload);
    let mut out = Vec::with_capacity(len);

    // Local file header.
    push_u32_le(&mut out, 0x0403_4b50);
    push_u16_le(&mut out, 20);
    push_u16_le(&mut out, 0);
    push_u16_le(&mut out, 0); // stored, already incompressible payload
    push_u16_le(&mut out, 0);
    push_u16_le(&mut out, 0x0021); // 1980-01-01, deterministic
    push_u32_le(&mut out, checksum);
    push_u32_le(&mut out, payload_len);
    push_u32_le(&mut out, payload_len);
    push_u16_le(&mut out, FILE_NAME.len() as u16);
    push_u16_le(&mut out, 0);
    out.extend_from_slice(FILE_NAME);
    out.extend_from_slice(&payload);

    let central_offset = out.len() as u32;
    push_u32_le(&mut out, 0x0201_4b50);
    push_u16_le(&mut out, 20);
    push_u16_le(&mut out, 20);
    push_u16_le(&mut out, 0);
    push_u16_le(&mut out, 0);
    push_u16_le(&mut out, 0);
    push_u16_le(&mut out, 0x0021);
    push_u32_le(&mut out, checksum);
    push_u32_le(&mut out, payload_len);
    push_u32_le(&mut out, payload_len);
    push_u16_le(&mut out, FILE_NAME.len() as u16);
    push_u16_le(&mut out, 0);
    push_u16_le(&mut out, 0);
    push_u16_le(&mut out, 0);
    push_u16_le(&mut out, 0);
    push_u32_le(&mut out, 0);
    push_u32_le(&mut out, 0);
    out.extend_from_slice(FILE_NAME);
    let central_size = out.len() as u32 - central_offset;

    // End of central directory.
    push_u32_le(&mut out, 0x0605_4b50);
    push_u16_le(&mut out, 0);
    push_u16_le(&mut out, 0);
    push_u16_le(&mut out, 1);
    push_u16_le(&mut out, 1);
    push_u32_le(&mut out, central_size);
    push_u32_le(&mut out, central_offset);
    push_u16_le(&mut out, 0);
    debug_assert_eq!(out.len(), len);
    out
}

fn push_u16_le(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u32_le(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

const CRC32_TABLE: [u32; 256] = build_crc32_table();

const fn build_crc32_table() -> [u32; 256] {
    let mut table = [0_u32; 256];
    let mut index = 0;
    while index < table.len() {
        let mut value = index as u32;
        let mut bit = 0;
        while bit < 8 {
            value = (value >> 1) ^ (0xedb8_8320 & (0_u32.wrapping_sub(value & 1)));
            bit += 1;
        }
        table[index] = value;
        index += 1;
    }
    table
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc = (crc >> 8) ^ CRC32_TABLE[((crc ^ u32::from(*byte)) & 0xff) as usize];
    }
    !crc
}

fn generated_numeric(
    seed: u64,
    column: &BlueprintColumn,
    distinct_count: Option<u64>,
    salt: u64,
    entropy: f64,
) -> String {
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
    let width_offset = if column.len_avg > 0 {
        let punctuation_width = u64::from(scale > 0);
        let requested_integer_digits = column
            .len_avg
            .saturating_sub(u64::from(scale))
            .saturating_sub(punctuation_width)
            .clamp(1, u64::from(precision.saturating_sub(scale).max(1)));
        10_u64
            .checked_pow(requested_integer_digits.saturating_sub(1) as u32)
            .and_then(|integer_offset| integer_offset.checked_mul(10_u64.pow(scale)))
            .filter(|offset| *offset < modulus)
            .unwrap_or(0)
    } else {
        0
    };
    let value_domain = modulus.saturating_sub(width_offset).max(1);
    let captured_display_domain = if column.len_p95 > 0 {
        let punctuation_width = u64::from(scale > 0);
        let integer_digits = column
            .len_p95
            .saturating_sub(u64::from(scale))
            .saturating_sub(punctuation_width)
            .clamp(1, u64::from(precision.saturating_sub(scale).max(1)));
        10_u64
            .checked_pow(integer_digits.saturating_add(u64::from(scale)) as u32)
            .unwrap_or(modulus)
            .min(modulus)
            .saturating_sub(width_offset)
            .max(1)
    } else {
        value_domain
    };
    let bounded_seed = if distinct_count.is_some_and(|count| {
        count > 1 && count <= captured_display_domain && captured_display_domain > 1
    }) {
        // Keep the captured number of values and frequency allocation exact,
        // but allow their digits to occupy the full captured display-width
        // domain. This is a bijection, so spreading a compact synthetic
        // ordinal range cannot introduce a collision.
        entropy_permuted_index(
            seed % captured_display_domain,
            captured_display_domain,
            salt,
            entropy,
        )
    } else {
        seed % value_domain
    };
    let value = width_offset + bounded_seed % value_domain;
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
    } else if column
        .cardinality
        .as_ref()
        .is_some_and(|cardinality| cardinality.measured)
    {
        // Use the compact non-negative half of the signed domain first. This
        // preserves measured small-domain value lengths (IDs and categories)
        // while still covering the negative half if a domain genuinely spans
        // the complete signed type.
        let positive_domain = 1_u128 << (bit_width - 1);
        let observed_domain = column
            .cardinality
            .as_ref()
            .map(|cardinality| {
                cardinality
                    .estimated_distinct_count
                    .max(cardinality.observed_distinct_count)
            })
            .unwrap_or(1)
            .max(1);
        let minimum_digits = decimal_width(observed_domain.saturating_sub(1));
        let desired_digits = column.len_avg.clamp(1, 19) as u32;
        let width_offset = if desired_digits > minimum_digits {
            10_u128
                .checked_pow(desired_digits - 1)
                .filter(|offset| offset.saturating_add(u128::from(seed)) < positive_domain)
                .unwrap_or(0)
        } else {
            0
        };
        let positive_value = width_offset.saturating_add(u128::from(seed));
        if positive_value < positive_domain {
            positive_value.to_string()
        } else {
            let negative_ordinal = u128::from(seed) - positive_domain;
            (-(negative_ordinal as i128) - 1).to_string()
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

fn decimal_width(mut value: u64) -> u32 {
    let mut width = 1u32;
    while value >= 10 {
        value /= 10;
        width += 1;
    }
    width
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
    // Use one mixed-radix calendar index. The previous unrelated divisors
    // (`28`, `29`, and `997`) aliased distinct seeds onto the same date and
    // silently collapsed measured date cardinality. Keeping 28 days per
    // month avoids invalid dates while preserving a large one-to-one domain.
    const DAYS_PER_YEAR: u64 = 12 * 28;
    const GENERATED_YEARS: u64 = 9_999 - 2_020 + 1;
    let ordinal = seed % (DAYS_PER_YEAR * GENERATED_YEARS);
    let day = (ordinal % 28) + 1;
    let month = ((ordinal / 28) % 12) + 1;
    let year = 2_020 + ordinal / DAYS_PER_YEAR;
    format!("{year:04}-{month:02}-{day:02}")
}

fn generated_time(seed: u64, precision: u64, entropy: f64) -> String {
    let second = seed % 60;
    let minute = (seed / 61) % 60;
    let hour = (seed / 3_661) % 24;
    format!(
        "{hour:02}:{minute:02}:{second:02}{}",
        generated_fractional_second(seed, precision, entropy)
    )
}

fn generated_timestamp(seed: u64, with_timezone: bool, precision: u64, entropy: f64) -> String {
    let second = seed % 60;
    let minute = (seed / 60) % 60;
    let hour = (seed / 3_600) % 24;
    let day_ordinal = seed / 86_400;
    let year = 2024 + day_ordinal / (12 * 28);
    let month = (day_ordinal / 28) % 12 + 1;
    let day = day_ordinal % 28 + 1;
    let suffix = if with_timezone { "+00" } else { "" };
    let fraction = generated_fractional_second(seed, precision, entropy);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}{fraction}{suffix}")
}

fn generated_fractional_second(seed: u64, precision: u64, entropy: f64) -> String {
    let precision = precision.min(9) as usize;
    if precision == 0 {
        return String::new();
    }
    let variable_digits = ((precision as f64) * entropy.clamp(0.0, 1.0)).round() as u32;
    let modulus = 10_u64.pow(variable_digits);
    let value = if variable_digits == 0 {
        0
    } else {
        seed % modulus
    };
    format!(".{value:0precision$}")
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

/// Deterministically vary value locality without changing the number of
/// distinct projected domain indexes. The odd affine transform is a bijection
/// inside each power-of-two bucket. Low entropy preserves broad ordered runs;
/// increasing entropy widens the permuted suffix until the complete 64-bit
/// domain is involved.
fn entropy_shaped_index(row_idx: u64, table_idx: u64, col_idx: u64, entropy: f64) -> u64 {
    let shuffled_bits = (entropy.clamp(0.0, 1.0) * 64.0).round() as u32;
    if shuffled_bits == 0 {
        return row_idx;
    }
    let mask = if shuffled_bits == 64 {
        u64::MAX
    } else {
        (1_u64 << shuffled_bits) - 1
    };
    let salt = mix64(
        table_idx
            .wrapping_mul(0x9e37_79b9_7f4a_7c15)
            .wrapping_add(col_idx.wrapping_mul(0xbf58_476d_1ce4_e5b9))
            .wrapping_add(0x454e_5452_4f50_5901),
    );
    let multiplier = mix64(salt ^ 0x4c4f_4341_4c49_5459) | 1;
    let offset = mix64(salt ^ 0x5045_524d_5554_4501);
    let low = row_idx.wrapping_mul(multiplier).wrapping_add(offset) & mask;
    (row_idx & !mask) | low
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
    fn equal_average_and_p95_lengths_do_not_invent_short_values() {
        let mut table = BlueprintTable {
            rows: 1_000,
            table_bytes: 11_000,
            ..Default::default()
        };
        let column = BlueprintColumn {
            column_type: "text".into(),
            nullable: false,
            len_avg: 11,
            len_p95: 11,
            ..Default::default()
        };
        table.cols.insert("col-1".into(), column.clone());

        for row_idx in 0..table.rows {
            assert_eq!(
                generated_value_len(
                    &table,
                    &column,
                    row_idx,
                    0,
                    SyntheticOptions::default().max_value_bytes,
                ),
                11
            );
        }
    }

    #[test]
    fn numeric_entropy_changes_locality_without_losing_cardinality() {
        let table = BlueprintTable {
            rows: 4_096,
            ..Default::default()
        };
        let column = BlueprintColumn {
            column_type: "bigint".to_string(),
            nullable: false,
            ..Default::default()
        };
        let options = SyntheticOptions::default();
        let generate = |entropy| {
            (0..table.rows)
                .map(|row_idx| {
                    blueprint_row_value_for_generated_rows_with_entropy(
                        &table,
                        &column,
                        table.rows,
                        7,
                        row_idx,
                        3,
                        options,
                        Some(entropy),
                    )
                    .expect("NOT NULL bigint must produce a value")
                })
                .collect::<Vec<_>>()
        };

        let ordered = generate(0.0);
        let shuffled = generate(1.0);
        assert_ne!(ordered, shuffled);
        assert_eq!(
            ordered.iter().collect::<BTreeSet<_>>().len(),
            table.rows as usize
        );
        assert_eq!(
            shuffled.iter().collect::<BTreeSet<_>>().len(),
            table.rows as usize
        );
        assert_eq!(shuffled, generate(1.0));
    }

    #[test]
    fn biased_all_distinct_sample_remains_a_lower_bound_when_generating() {
        let table = BlueprintTable {
            rows: 10_000,
            ..Default::default()
        };
        let column = BlueprintColumn {
            cardinality: Some(BlueprintCardinality {
                measured: true,
                sample_rows: 4_096,
                non_null_rows: 4_096,
                observed_distinct_count: 4_096,
                estimated_distinct_count: 4_096,
                frequency_p50: 1,
                frequency_p95: 1,
                frequency_p99: 1,
                frequency_max: 1,
                sampled_with_bias: true,
                bias_reason: "server_side_cell_cap".into(),
                ..Default::default()
            }),
            ..Default::default()
        };

        let projected = (0..table.rows)
            .map(|row| {
                statistical_value_row_index_for_generated_rows(&table, &column, table.rows, row, 0)
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(projected.len(), table.rows as usize);
    }

    #[test]
    fn legacy_biased_near_unique_lower_bound_does_not_collapse_at_scale() {
        let table = BlueprintTable {
            rows: 100_000,
            ..Default::default()
        };
        let column = BlueprintColumn {
            cardinality: Some(BlueprintCardinality {
                measured: true,
                sample_rows: 4_096,
                non_null_rows: 4_096,
                observed_distinct_count: 4_000,
                estimated_distinct_count: 4_000,
                frequency_p50: 1,
                frequency_p95: 1,
                frequency_p99: 1,
                frequency_max: 2,
                sampled_with_bias: true,
                bias_reason: "natural order".into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            projected_statistical_distinct_count(&table, &column, table.rows),
            Some(100_000)
        );
    }

    #[test]
    fn locality_projection_is_stable_between_sample_and_full_scale() {
        let reference = Some((4_096, 4));
        let sample_run = uniform_run_length_target(4_096, 4, 0.9, reference);
        let full_run = uniform_run_length_target(100_000, 4, 0.9, reference);
        assert_eq!(sample_run, full_run);
        assert!((full_run - 11.23).abs() < 0.001);
    }

    #[test]
    fn fractional_uniform_locality_mixes_adjacent_run_lengths() {
        let entropy = 0.5;
        let positions = (0..1_500_u64)
            .map(|position| uniform_group_position(1_500, 500, entropy, None, position).0)
            .collect::<Vec<_>>();
        let mut run_lengths = Vec::new();
        let mut run = 0_u64;
        let mut prior = None;
        for group in positions {
            if prior == Some(group) {
                run += 1;
            } else {
                if run > 0 {
                    run_lengths.push(run);
                }
                prior = Some(group);
                run = 1;
            }
        }
        run_lengths.push(run);
        assert!(run_lengths.contains(&1));
        assert!(run_lengths.contains(&2));
        assert!(run_lengths.iter().all(|run| matches!(run, 1 | 2)));
    }

    #[test]
    fn unique_numeric_domain_entropy_changes_order_without_losing_values() {
        let table = BlueprintTable {
            rows: 4_096,
            ..Default::default()
        };
        let column = BlueprintColumn {
            column_type: "bigint".into(),
            cardinality: Some(BlueprintCardinality {
                measured: true,
                sample_rows: table.rows,
                non_null_rows: table.rows,
                observed_distinct_count: table.rows,
                estimated_distinct_count: table.rows,
                frequency_p50: 1,
                frequency_p95: 1,
                frequency_p99: 1,
                frequency_max: 1,
                ..Default::default()
            }),
            ..Default::default()
        };
        let values = |entropy| {
            (0..table.rows)
                .map(|row| {
                    statistical_value_row_index_for_generated_rows_with_entropy(
                        &table, &column, table.rows, row, 0, entropy,
                    )
                })
                .collect::<Vec<_>>()
        };
        let ordered = values(0.0);
        let shuffled = values(1.0);
        assert_ne!(ordered, shuffled);
        assert_eq!(
            ordered.iter().copied().collect::<BTreeSet<_>>(),
            shuffled.iter().copied().collect::<BTreeSet<_>>()
        );
    }

    #[test]
    fn public_cardinality_permutation_is_a_bijection() {
        let ordered = (0..997_u64).collect::<Vec<_>>();
        let permuted = (0..997_u64)
            .map(|index| entropy_permuted_cardinality_index(index, 997, 41, 1.0))
            .collect::<Vec<_>>();
        assert_ne!(ordered, permuted);
        assert_eq!(
            ordered.iter().copied().collect::<BTreeSet<_>>(),
            permuted.iter().copied().collect::<BTreeSet<_>>()
        );
    }

    #[test]
    fn entropy_permutation_window_is_continuous_between_bit_boundaries() {
        let count = 59_150_000;
        let windows = (0..=100)
            .map(|step| entropy_permutation_window(count, f64::from(step) / 100.0))
            .collect::<Vec<_>>();
        assert_eq!(windows[0], 1);
        assert_eq!(windows[100], count);
        assert!(windows.windows(2).all(|pair| pair[0] <= pair[1]));
        assert!(windows.iter().copied().collect::<BTreeSet<_>>().len() > 64);

        for entropy in [0.01, 0.17, 0.43, 0.71, 0.99] {
            let permuted = (0..997_u64)
                .map(|index| entropy_permuted_cardinality_index(index, 997, 41, entropy))
                .collect::<BTreeSet<_>>();
            assert_eq!(permuted, (0..997_u64).collect::<BTreeSet<_>>());
        }
    }

    #[test]
    fn numeric_generation_uses_captured_display_width() {
        let column = BlueprintColumn {
            numeric_precision: 15,
            numeric_scale: 2,
            len_avg: 8,
            ..Default::default()
        };
        assert_eq!(generated_numeric(0, &column, None, 0, 0.0), "10000.00");
        assert_eq!(generated_numeric(99, &column, None, 0, 0.0), "10000.99");
    }

    #[test]
    fn measured_integer_cardinality_is_not_expanded_by_entropy() {
        let rows = 4_096;
        let table = BlueprintTable {
            rows,
            ..Default::default()
        };
        let column = BlueprintColumn {
            column_type: "integer".into(),
            cardinality: Some(BlueprintCardinality {
                measured: true,
                sample_rows: rows,
                non_null_rows: rows,
                observed_distinct_count: 42,
                estimated_distinct_count: 42,
                top_value_fraction: 0.025,
                frequency_p50: 100,
                frequency_p95: 100,
                frequency_p99: 100,
                frequency_max: 100,
                ..Default::default()
            }),
            ..Default::default()
        };
        let options = SyntheticOptions::default();
        for entropy in [0.0, 0.5, 1.0] {
            let generated = (0..rows)
                .map(|row_idx| {
                    blueprint_row_value_for_generated_rows_with_entropy(
                        &table,
                        &column,
                        rows,
                        0,
                        row_idx,
                        0,
                        options,
                        Some(entropy),
                    )
                    .expect("NOT NULL integer")
                })
                .collect::<BTreeSet<_>>();
            assert_eq!(generated.len(), 42);
            assert!(generated.iter().all(|value| value.len() <= 2));
        }

        let fixed_width_constant = BlueprintColumn {
            column_type: "bigint".into(),
            len_avg: 6,
            len_p95: 6,
            cardinality: Some(BlueprintCardinality {
                measured: true,
                sample_rows: rows,
                non_null_rows: rows,
                observed_distinct_count: 1,
                estimated_distinct_count: 1,
                top_value_fraction: 1.0,
                frequency_p50: rows,
                frequency_p95: rows,
                frequency_p99: rows,
                frequency_max: rows,
                ..Default::default()
            }),
            ..Default::default()
        };
        let value = blueprint_row_value_for_generated_rows_with_entropy(
            &table,
            &fixed_width_constant,
            rows,
            0,
            0,
            0,
            options,
            Some(0.0),
        )
        .expect("NOT NULL bigint");
        assert_eq!(value.len(), 6);
    }

    #[test]
    fn measured_uniform_domain_entropy_controls_runs_without_changing_the_domain() {
        let rows = 4_096;
        let table = BlueprintTable {
            rows,
            ..Default::default()
        };
        let column = BlueprintColumn {
            cardinality: Some(BlueprintCardinality {
                measured: true,
                sample_rows: rows,
                non_null_rows: rows,
                observed_distinct_count: 4,
                estimated_distinct_count: 4,
                top_value_fraction: 0.25,
                frequency_p50: 1_024,
                frequency_p95: 1_024,
                frequency_p99: 1_024,
                frequency_max: 1_024,
                ..Default::default()
            }),
            ..Default::default()
        };
        let generate = |entropy| {
            (0..rows)
                .map(|row_idx| {
                    statistical_value_row_index_for_generated_rows_with_entropy(
                        &table, &column, rows, row_idx, 0, entropy,
                    )
                })
                .collect::<Vec<_>>()
        };
        let broad_runs = generate(0.0);
        let compact_cycle = generate(1.0);
        assert_eq!(broad_runs.iter().copied().collect::<BTreeSet<_>>().len(), 4);
        assert_eq!(
            compact_cycle.iter().copied().collect::<BTreeSet<_>>().len(),
            4
        );
        let broad_transitions = broad_runs
            .windows(2)
            .filter(|pair| pair[0] != pair[1])
            .count();
        let cycle_transitions = compact_cycle
            .windows(2)
            .filter(|pair| pair[0] != pair[1])
            .count();
        assert!(broad_transitions <= 4, "{broad_transitions}");
        assert!(cycle_transitions > 3_000, "{cycle_transitions}");
    }

    #[test]
    fn privacy_quantized_uniform_fraction_retains_locality_control() {
        let rows = 32_768;
        let values = (0..rows)
            .map(|row_idx| {
                project_distribution_index_with_locality(
                    row_idx,
                    rows,
                    4,
                    0.255,
                    [8_192, 8_192, 8_192],
                    29,
                    0.0,
                    Some((rows, 4)),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(values.iter().copied().collect::<BTreeSet<_>>().len(), 4);
        let transitions = values.windows(2).filter(|pair| pair[0] != pair[1]).count();
        assert!(transitions <= 4, "{transitions}");
    }

    #[test]
    fn approximate_uniform_frequency_profile_keeps_counts_while_shuffling_order() {
        let rows = 4_096;
        let generate = |entropy| {
            (0..rows)
                .map(|row_idx| {
                    project_distribution_index_with_locality(
                        row_idx,
                        rows,
                        1_024,
                        0.0,
                        [4, 7, 7],
                        17,
                        entropy,
                        Some((rows, 1_024)),
                    )
                })
                .collect::<Vec<_>>()
        };
        let ordered = generate(0.0);
        let shuffled = generate(1.0);
        let histogram = |values: &[u64]| {
            let mut counts = BTreeMap::new();
            for value in values {
                *counts.entry(*value).or_insert(0_u64) += 1;
            }
            counts
        };
        assert_ne!(ordered, shuffled);
        assert_eq!(
            histogram(ordered.as_slice()),
            histogram(shuffled.as_slice())
        );
        assert_eq!(histogram(shuffled.as_slice()).len(), 1_024);
    }

    #[test]
    fn skewed_frequency_profile_entropy_changes_order_not_frequency() {
        let rows = 4_096;
        let generate = |entropy| {
            (0..rows)
                .map(|row_idx| {
                    project_distribution_index_with_locality(
                        row_idx,
                        rows,
                        64,
                        0.20,
                        [2, 6, 12],
                        23,
                        entropy,
                        Some((rows, 64)),
                    )
                })
                .collect::<Vec<_>>()
        };
        let ordered = generate(0.0);
        let shuffled = generate(1.0);
        let histogram = |values: &[u64]| {
            let mut counts = BTreeMap::new();
            for value in values {
                *counts.entry(*value).or_insert(0_u64) += 1;
            }
            counts
        };
        assert_ne!(ordered, shuffled);
        assert_eq!(
            histogram(ordered.as_slice()),
            histogram(shuffled.as_slice())
        );
        assert_eq!(histogram(shuffled.as_slice()).len(), 64);
    }

    #[test]
    fn skewed_locality_horizon_does_not_expand_with_fixture_scale() {
        let generate = |rows| {
            (0..rows)
                .map(|row_idx| {
                    project_distribution_index_with_locality(
                        row_idx,
                        rows,
                        8,
                        0.205,
                        [448, 1_664, 1_664],
                        31,
                        0.0,
                        Some((4_096, 8)),
                    )
                })
                .collect::<Vec<_>>()
        };
        let sample = generate(4_096);
        let scaled = generate(40_960);
        let transition_density = |values: &[u64]| {
            values.windows(2).filter(|pair| pair[0] != pair[1]).count() as f64 / values.len() as f64
        };
        assert_eq!(sample.iter().copied().collect::<BTreeSet<_>>().len(), 8);
        assert_eq!(scaled.iter().copied().collect::<BTreeSet<_>>().len(), 8);
        assert!((transition_density(&sample) - transition_density(&scaled)).abs() < 0.005);
    }

    #[test]
    fn measured_text_cardinality_survives_zero_entropy() {
        let rows = 4_096;
        let table = BlueprintTable {
            rows,
            table_bytes: rows * 6,
            ..Default::default()
        };
        let column = BlueprintColumn {
            column_type: "text".into(),
            declared_max_chars: 16,
            len_avg: 6,
            len_p95: 6,
            cardinality: Some(BlueprintCardinality {
                measured: true,
                sample_rows: rows,
                non_null_rows: rows,
                observed_distinct_count: 4,
                estimated_distinct_count: 4,
                top_value_fraction: 0.25,
                frequency_p50: 1_024,
                frequency_p95: 1_024,
                frequency_p99: 1_024,
                frequency_max: 1_024,
                ..Default::default()
            }),
            ..Default::default()
        };
        let generated = (0..rows)
            .map(|row_idx| {
                blueprint_row_value_for_generated_rows_with_entropy(
                    &table,
                    &column,
                    rows,
                    0,
                    row_idx,
                    0,
                    SyntheticOptions::default(),
                    Some(0.0),
                )
                .expect("NOT NULL text")
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(generated.len(), 4);
    }

    #[test]
    fn timestamptz_values_carry_an_explicit_utc_offset() {
        let table = BlueprintTable {
            rows: 2,
            ..Default::default()
        };
        let column = BlueprintColumn {
            column_type: "timestamptz".into(),
            cardinality: Some(BlueprintCardinality {
                measured: true,
                sample_rows: 2,
                non_null_rows: 2,
                observed_distinct_count: 2,
                estimated_distinct_count: 2,
                frequency_p50: 1,
                frequency_p95: 1,
                frequency_p99: 1,
                frequency_max: 1,
                ..Default::default()
            }),
            ..Default::default()
        };
        let value = blueprint_row_value_for_generated_rows_with_entropy(
            &table,
            &column,
            table.rows,
            0,
            0,
            0,
            SyntheticOptions::default(),
            Some(0.0),
        )
        .expect("NOT NULL timestamptz");
        assert!(value.ends_with(b"+00"));
        assert_eq!(value.len(), 22);

        let next = blueprint_row_value_for_generated_rows_with_entropy(
            &table,
            &column,
            table.rows,
            0,
            1,
            0,
            SyntheticOptions::default(),
            Some(0.0),
        )
        .expect("NOT NULL timestamptz");
        assert_eq!(value, b"2024-01-01 00:00:00+00");
        assert_eq!(next, b"2024-01-01 00:00:01+00");
    }

    #[test]
    fn temporal_values_preserve_declared_fractional_precision() {
        assert_eq!(
            generated_timestamp(1, false, 6, 0.0),
            "2024-01-01 00:00:01.000000"
        );
        assert_eq!(
            generated_timestamp(1, false, 6, 1.0),
            "2024-01-01 00:00:01.000001"
        );
        assert_eq!(generated_time(1, 3, 0.0), "00:00:01.000");
    }

    #[test]
    fn dense_float32_vectors_preserve_binary_width_and_dimension() {
        let table = BlueprintTable::default();
        let column = BlueprintColumn {
            column_type: "vector".into(),
            declared_max_bytes: 3_076,
            nullable: false,
            ..Default::default()
        };
        assert_eq!(float32_vector_dimension(&column), Some(768));
        assert_eq!(synthetic_probe_type_tag("vector"), 0x11);
        let value = blueprint_row_value_with_entropy(
            &table,
            &column,
            0,
            7,
            0,
            SyntheticOptions::default(),
            Some(1.0),
        )
        .expect("NOT NULL vector");
        assert_eq!(value.len(), 3_076);
        assert_eq!(&value[..4], &[0x03, 0x00, 0x00, 0x00]);
        assert!(value[4..].chunks_exact(4).all(|bytes| {
            f32::from_bits(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])).is_finite()
        }));

        let invalid = BlueprintColumn {
            column_type: "vector".into(),
            declared_max_bytes: 3_075,
            ..Default::default()
        };
        assert_eq!(float32_vector_dimension(&invalid), None);
    }

    #[test]
    fn generated_dates_preserve_the_bounded_calendar_domain() {
        let values = (0..2_352_u64).map(generated_date).collect::<BTreeSet<_>>();
        assert_eq!(values.len(), 2_352);
        assert!(values.contains("2020-01-01"));
        assert!(values.contains("2026-12-28"));
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
    fn precompressed_binary_profile_emits_deterministic_valid_neutral_zip() {
        let table = BlueprintTable {
            rows: 100,
            table_bytes: 409_600,
            ..Default::default()
        };
        let column = BlueprintColumn {
            column_type: "binary".into(),
            len_avg: 4_096,
            len_p95: 4_096,
            style: crate::PRECOMPRESSED_STYLE.into(),
            ..Default::default()
        };
        let options = SyntheticOptions {
            max_value_bytes: 8 * 1024,
            null_percent: 0,
        };
        let first = blueprint_row_value(&table, &column, 7, 11, 2, options).unwrap();
        let repeated = blueprint_row_value(&table, &column, 7, 11, 2, options).unwrap();
        let next = blueprint_row_value(&table, &column, 7, 12, 2, options).unwrap();

        assert_eq!(first, repeated);
        assert_ne!(first, next);
        assert_eq!(first.len(), 4_096);
        assert!(crate::has_precompressed_container_signature(&first));
        assert_eq!(
            u32::from_le_bytes(first[0..4].try_into().unwrap()),
            0x0403_4b50
        );
        assert_eq!(u16::from_le_bytes(first[8..10].try_into().unwrap()), 0);
        let expected_crc = u32::from_le_bytes(first[14..18].try_into().unwrap());
        let compressed_len = u32::from_le_bytes(first[18..22].try_into().unwrap()) as usize;
        let uncompressed_len = u32::from_le_bytes(first[22..26].try_into().unwrap()) as usize;
        let name_len = u16::from_le_bytes(first[26..28].try_into().unwrap()) as usize;
        let extra_len = u16::from_le_bytes(first[28..30].try_into().unwrap()) as usize;
        let payload_start = 30 + name_len + extra_len;
        let payload_end = payload_start + compressed_len;
        assert_eq!(compressed_len, uncompressed_len);
        assert_eq!(&first[30..30 + name_len], b"payload.bin");
        assert_eq!(crc32(&first[payload_start..payload_end]), expected_crc);
        assert_eq!(
            u32::from_le_bytes(first[payload_end..payload_end + 4].try_into().unwrap()),
            0x0201_4b50
        );
        assert_eq!(
            u32::from_le_bytes(
                first[first.len() - 22..first.len() - 18]
                    .try_into()
                    .unwrap()
            ),
            0x0605_4b50
        );
    }

    #[test]
    fn precompressed_style_does_not_change_short_or_nonbinary_generation() {
        let table = BlueprintTable {
            rows: 10,
            table_bytes: 1_000,
            ..Default::default()
        };
        let options = SyntheticOptions {
            max_value_bytes: 1_024,
            null_percent: 0,
        };
        let short_binary = BlueprintColumn {
            column_type: "binary".into(),
            len_avg: 64,
            style: crate::PRECOMPRESSED_STYLE.into(),
            ..Default::default()
        };
        let short = blueprint_row_value(&table, &short_binary, 1, 2, 3, options).unwrap();
        assert_eq!(short.len(), 64);
        assert!(!crate::has_precompressed_container_signature(&short));

        let text = BlueprintColumn {
            column_type: "text".into(),
            len_avg: 256,
            style: crate::PRECOMPRESSED_STYLE.into(),
            ..Default::default()
        };
        let value = blueprint_row_value(&table, &text, 1, 2, 3, options).unwrap();
        assert_eq!(value.len(), 256);
        assert!(!crate::has_precompressed_container_signature(&value));
    }

    #[cfg(feature = "sampling")]
    #[test]
    fn precompressed_profile_generates_an_incompressible_binary_workload() {
        let table = BlueprintTable {
            rows: 64,
            table_bytes: 64 * 16 * 1024,
            ..Default::default()
        };
        let column = BlueprintColumn {
            column_type: "binary".into(),
            len_avg: 16 * 1024,
            len_p95: 16 * 1024,
            style: crate::PRECOMPRESSED_STYLE.into(),
            ..Default::default()
        };
        let options = SyntheticOptions {
            max_value_bytes: 32 * 1024,
            null_percent: 0,
        };
        let mut generated = Vec::with_capacity(table.table_bytes as usize);
        for row_idx in 0..table.rows {
            generated.extend(
                blueprint_row_value(&table, &column, 3, row_idx, 1, options)
                    .expect("binary generation cannot return null"),
            );
        }

        let compressed = zstd::bulk::compress(&generated, 3).expect("zstd compression");
        let ratio = generated.len() as f64 / compressed.len() as f64;
        assert!(
            ratio < 1.10,
            "profiled binary workload unexpectedly compresses at {ratio:.3}:1"
        );
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
            column_type: "binary".into(),
            compression: Some(BlueprintCompression {
                ratio_zstd_3: 1.05,
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
        assert!(
            entropy_for_column_with_table_calibration(&table, &compressible, 1.0)
                > entropy_for_column(&table, &compressible)
        );
        assert_eq!(
            entropy_for_column_with_table_calibration(&table, &compressible, 0.0),
            0.0
        );
        assert_eq!(
            entropy_for_column_with_table_calibration(&table, &compressible, 1.0),
            1.0
        );
        assert_eq!(
            entropy_for_column_with_table_calibration(&table, &incompressible, 0.0),
            entropy_for_column(&table, &incompressible)
        );

        let unmeasured = BlueprintColumn::default();
        assert_eq!(
            entropy_for_column_with_table_calibration(&table, &unmeasured, 0.73),
            0.73
        );
    }

    #[cfg(feature = "sampling")]
    #[test]
    fn measured_one_to_one_binary_profile_generates_incompressible_bytes() {
        let table = BlueprintTable {
            rows: 128,
            table_bytes: 128 * 16 * 1024,
            ..Default::default()
        };
        let column = BlueprintColumn {
            column_type: "binary".into(),
            len_avg: 16 * 1024,
            len_p95: 16 * 1024,
            compression: Some(BlueprintCompression {
                measured: true,
                ratio_zstd_3: 1.0,
                sample_encoding: crate::SAMPLE_ENCODING_TAG.into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut generated = Vec::with_capacity(table.table_bytes as usize);
        for row_idx in 0..table.rows {
            generated.extend(
                blueprint_row_value(&table, &column, 0, row_idx, 0, SyntheticOptions::default())
                    .expect("binary generation cannot return null"),
            );
        }
        let compressed = zstd::bulk::compress(&generated, 3).expect("zstd compression");
        let ratio = generated.len() as f64 / compressed.len() as f64;
        assert!(ratio < 1.05, "generated binary compressed at {ratio:.3}:1");
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
    fn entropy_calibration_finds_a_non_monotonic_interior_basin() {
        let calibration = calibrate_entropy(2.0, 0.9, 12, 0.001, |entropy| {
            Ok(2.0 + (entropy - 0.37).abs() * 20.0)
        })
        .unwrap();
        assert!(calibration.matched, "{calibration:?}");
        assert!((calibration.entropy - 0.37).abs() < 0.01, "{calibration:?}");
        assert!(calibration.observations >= 12);
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
    fn generated_probe_stream_uses_canonical_tags_and_varint_lengths() {
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
        append_synthetic_probe_row_with_entropy(
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

    #[test]
    fn ordered_columns_excludes_database_maintained_values() {
        let mut table = BlueprintTable::default();
        table.cols.insert(
            "ordinary".into(),
            BlueprintColumn {
                ordinal: 1,
                ..Default::default()
            },
        );
        table.cols.insert(
            "generated".into(),
            BlueprintColumn {
                ordinal: 2,
                value_source: "generated-stored".into(),
                ..Default::default()
            },
        );
        table.cols.insert(
            "identity".into(),
            BlueprintColumn {
                ordinal: 3,
                value_source: "auto-increment".into(),
                ..Default::default()
            },
        );
        let ordinals = ordered_columns(&table)
            .into_iter()
            .map(|(_, column)| column.ordinal)
            .collect::<Vec<_>>();
        assert_eq!(ordinals, vec![1, 3]);
    }

    #[test]
    fn parameterized_logical_types_keep_their_generation_family() {
        assert!(is_numeric_type("decimal(19,4)"));
        assert!(is_numeric_type("numeric(38,9)"));
        assert!(is_integer_type("bigint(20)"));
        assert!(is_boolean_type("bit(1)"));
        assert!(is_temporal_type("timestamp(6)"));
        assert_eq!(synthetic_probe_type_tag("decimal(19,4)"), 0x04);

        let column = BlueprintColumn {
            column_type: "decimal(19,4)".into(),
            numeric_precision: 19,
            numeric_scale: 4,
            ..Default::default()
        };
        let value = blueprint_row_value(
            &BlueprintTable::default(),
            &column,
            0,
            7,
            0,
            SyntheticOptions::default(),
        )
        .expect("generated decimal");
        let value = String::from_utf8(value).expect("decimal text");
        assert_eq!(
            value.split_once('.').map(|(_, fraction)| fraction.len()),
            Some(4)
        );
    }
}
