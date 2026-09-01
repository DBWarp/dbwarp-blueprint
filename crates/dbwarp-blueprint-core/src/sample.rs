use crate::{
    BlueprintCardinality, BlueprintColumn, BlueprintCompression, SamplingDeadline,
    SAMPLE_ENCODING_TAG,
};
use anyhow::{Context, Result};
use std::mem::size_of;
use std::time::Duration;

pub const DEFAULT_MAX_SAMPLE_BYTES: usize = 256 * 1024 * 1024;
const RESERVOIR_CAPACITY: usize = 8_192;

/// Ratio-variance measurement granularity. Chunking the sample at this size
/// and ending each chunk on a row boundary makes the per-chunk ratios
/// describe the transfer the estimator predicts, rather than one whole-buffer
/// average that hides variance.
pub const WIRE_CHUNK_BYTES: usize = 64 * 1024;

/// Byte buffers grow in fixed steps rather than per row. Exact per-row
/// reservations force a reallocation on nearly every push, which is a full
/// copy on allocators that cannot remap large blocks (Windows). The step
/// keeps reallocation count logarithmic-in-practice while the resident
/// budget still accounts actual capacities.
const RESERVE_STEP_BYTES: usize = 256 * 1024;

/// Target capacity for a stepped byte-buffer reservation: unchanged when the
/// spare capacity already covers `additional`, otherwise the length plus the
/// larger of `additional` and the step.
fn stepped_capacity(len: usize, capacity: usize, additional: usize) -> usize {
    let needed = len.saturating_add(additional);
    if capacity >= needed {
        capacity
    } else {
        len.saturating_add(additional.max(RESERVE_STEP_BYTES))
    }
}

pub const FIRST_N_BIAS_REASON: &str = "deterministic-first-n-rows";

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeTag {
    Null = 0x00,
    TextUtf8 = 0x01,
    TextUtf16Le = 0x02,
    TextOther = 0x03,
    NumberText = 0x04,
    BoolText = 0x05,
    TimestampText = 0x06,
    DateText = 0x07,
    TimeText = 0x08,
    UuidText = 0x09,
    JsonText = 0x0F,
    BinaryRaw = 0x10,
    UnknownText = 0xFE,
}

#[derive(Debug, Clone)]
pub struct OwnedCell {
    pub tag: TypeTag,
    pub bytes: Option<Vec<u8>>,
}

impl OwnedCell {
    pub fn null() -> Self {
        Self {
            tag: TypeTag::Null,
            bytes: None,
        }
    }

    pub fn new(tag: TypeTag, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            tag,
            bytes: Some(bytes.into()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DecodedCompressionOptions {
    pub sample_rows: u64,
    pub table_sample_method: String,
    pub column_sample_method: String,
    pub max_sample_bytes: usize,
    pub max_wall: Duration,
    pub sampled_with_bias: bool,
    pub bias_reason: String,
}

impl DecodedCompressionOptions {
    pub fn disabled() -> Self {
        Self {
            sample_rows: 0,
            table_sample_method: String::new(),
            column_sample_method: String::new(),
            max_sample_bytes: DEFAULT_MAX_SAMPLE_BYTES,
            max_wall: Duration::MAX,
            sampled_with_bias: false,
            bias_reason: String::new(),
        }
    }

    pub fn enabled(
        sample_rows: u64,
        table_method: impl Into<String>,
        column_method: impl Into<String>,
    ) -> Self {
        Self {
            sample_rows,
            table_sample_method: table_method.into(),
            column_sample_method: column_method.into(),
            max_sample_bytes: DEFAULT_MAX_SAMPLE_BYTES,
            max_wall: Duration::from_secs(300),
            sampled_with_bias: true,
            bias_reason: FIRST_N_BIAS_REASON.to_string(),
        }
    }

    pub fn with_limits(mut self, max_sample_bytes: usize, max_wall: Duration) -> Self {
        self.max_sample_bytes = max_sample_bytes.max(1);
        self.max_wall = max_wall.max(Duration::from_secs(1));
        self
    }

    pub fn is_enabled(&self) -> bool {
        self.sample_rows > 0
    }

    /// Start the absolute wall-clock budget for one logical operation.
    ///
    /// Batch callers must invoke this once and pass the returned deadline to
    /// every deadline-aware file reader. Calling it once per file would
    /// intentionally start a new operation budget for each file.
    pub fn deadline(&self) -> SamplingDeadline {
        SamplingDeadline::after(self.max_wall)
    }

    pub fn with_bias(mut self, sampled_with_bias: bool, reason: impl Into<String>) -> Self {
        self.sampled_with_bias = sampled_with_bias;
        self.bias_reason = if sampled_with_bias {
            reason.into()
        } else {
            String::new()
        };
        self
    }

    pub fn effective_bias(&self, sampled_rows: u64, total_rows: u64) -> (bool, &str) {
        if self.sampled_with_bias
            && self.bias_reason == FIRST_N_BIAS_REASON
            && total_rows > 0
            && sampled_rows >= total_rows
        {
            (false, "")
        } else {
            (self.sampled_with_bias, self.bias_reason.as_str())
        }
    }
}

#[derive(Debug)]
pub struct CompressionSampleAccumulator {
    table_buf: Vec<u8>,
    column_bufs: Vec<Vec<u8>>,
    table_per_chunk_ratios: Vec<f64>,
    column_per_chunk_ratios: Vec<Vec<f64>>,
    column_value_stats: Vec<ColumnValueStats>,
    table_chunk_start: usize,
    table_chunks_seen: u64,
    column_chunk_starts: Vec<usize>,
    column_chunks_seen: Vec<u64>,
    sample_rows: u64,
    max_resident_bytes: usize,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DecodedColumnStats {
    pub sample_rows: u64,
    pub non_null_values: u64,
    pub null_fraction: f64,
    pub len_avg: u64,
    pub len_p95: u64,
    pub len_p95_sample_rows: u64,
}

#[derive(Debug, Clone, Default)]
struct ColumnValueStats {
    rows: u64,
    non_null_values: u64,
    nulls: u64,
    total_bytes: u64,
    lengths: Vec<u64>,
    fingerprints: Vec<u64>,
}

impl ColumnValueStats {
    const MIN_UNBIASED_ESTIMATE_ROWS: u64 = 128;

    fn push(&mut self, cell: &OwnedCell) {
        self.rows = self.rows.saturating_add(1);
        let Some(bytes) = cell.bytes.as_ref() else {
            self.nulls = self.nulls.saturating_add(1);
            return;
        };
        self.non_null_values = self.non_null_values.saturating_add(1);
        self.total_bytes = self.total_bytes.saturating_add(bytes.len() as u64);
        reservoir_push(&mut self.lengths, self.non_null_values, bytes.len() as u64);
        reservoir_push(
            &mut self.fingerprints,
            self.non_null_values,
            fingerprint_cell(cell.tag, bytes),
        );
    }

    fn finish(&self) -> DecodedColumnStats {
        let len_avg = if self.non_null_values == 0 {
            0
        } else {
            self.total_bytes.saturating_add(self.non_null_values / 2) / self.non_null_values
        };
        let mut lengths = self.lengths.clone();
        lengths.sort_unstable();
        let len_p95 = if lengths.is_empty() {
            0
        } else {
            let rank = ((lengths.len() as f64 * 0.95).ceil() as usize)
                .saturating_sub(1)
                .min(lengths.len() - 1);
            lengths[rank]
        };
        DecodedColumnStats {
            sample_rows: self.rows,
            non_null_values: self.non_null_values,
            null_fraction: if self.rows == 0 {
                0.0
            } else {
                self.nulls as f64 / self.rows as f64
            },
            len_avg,
            len_p95,
            len_p95_sample_rows: lengths.len() as u64,
        }
    }

    fn cardinality(
        &self,
        source_rows: u64,
        sample_method: &str,
        sampled_with_bias: bool,
        bias_reason: &str,
    ) -> Option<BlueprintCardinality> {
        if self.rows == 0 || self.fingerprints.is_empty() {
            return None;
        }
        let mut fingerprints = self.fingerprints.clone();
        fingerprints.sort_unstable();
        let mut frequencies = Vec::new();
        let mut current = fingerprints[0];
        let mut count = 0_u64;
        for fingerprint in fingerprints {
            if fingerprint != current {
                frequencies.push(count);
                current = fingerprint;
                count = 0;
            }
            count = count.saturating_add(1);
        }
        frequencies.push(count);
        frequencies.sort_unstable();

        let observed = frequencies.len() as u64;
        let singleton_count = frequencies.iter().filter(|count| **count == 1).count() as u64;
        let doubleton_count = frequencies.iter().filter(|count| **count == 2).count() as u64;
        let retained_non_null = frequencies.iter().sum::<u64>();
        let estimated_source_non_null = if self.rows == 0 {
            0
        } else {
            ((source_rows as u128)
                .saturating_mul(self.non_null_values as u128)
                .saturating_add((self.rows / 2) as u128)
                / self.rows as u128)
                .min(u64::MAX as u128) as u64
        };
        let (estimated, estimate_method) = if source_rows > 0 && source_rows <= self.rows {
            (observed, "complete bounded sample")
        } else if sampled_with_bias {
            (observed, "cardinality observed lower bound (biased sample)")
        } else if retained_non_null < Self::MIN_UNBIASED_ESTIMATE_ROWS {
            (observed, "cardinality observed lower bound (small sample)")
        } else {
            let unseen = if doubleton_count > 0 {
                let numerator = (singleton_count as u128).saturating_mul(singleton_count as u128);
                (numerator / (2_u128.saturating_mul(doubleton_count as u128))).min(u64::MAX as u128)
                    as u64
            } else {
                singleton_count.saturating_mul(singleton_count.saturating_sub(1)) / 2
            };
            (
                observed
                    .saturating_add(unseen)
                    .clamp(observed, estimated_source_non_null.max(observed)),
                "Chao1 lower-bound cardinality estimate",
            )
        };
        let top_frequency = frequencies.last().copied().unwrap_or(0);
        let sample_rows = quantize_stat_count(self.rows);
        let non_null_rows = quantize_stat_count(self.non_null_values).min(sample_rows);
        let observed_distinct_count = quantize_stat_count(observed).min(non_null_rows);
        Some(BlueprintCardinality {
            measured: true,
            sample_rows,
            non_null_rows,
            observed_distinct_count,
            estimated_distinct_count: quantize_stat_count(estimated)
                .max(observed_distinct_count)
                .min(source_rows.max(observed_distinct_count)),
            top_value_fraction: quantize_fraction(
                top_frequency as f64 / retained_non_null.max(1) as f64,
            ),
            frequency_p50: quantile(&frequencies, 0.50),
            frequency_p95: quantile(&frequencies, 0.95),
            frequency_p99: quantile(&frequencies, 0.99),
            frequency_max: top_frequency,
            sample_method: format!("{sample_method}; {estimate_method}"),
            sampled_with_bias,
            bias_reason: if sampled_with_bias {
                bias_reason.to_string()
            } else {
                String::new()
            },
        })
    }
}

impl CompressionSampleAccumulator {
    pub fn new(column_count: usize) -> Self {
        Self::with_max_resident_bytes(column_count, DEFAULT_MAX_SAMPLE_BYTES)
            .expect("default decoded-sample memory budget must fit column metadata")
    }

    pub fn with_max_resident_bytes(column_count: usize, max_resident_bytes: usize) -> Result<Self> {
        let max_resident_bytes = max_resident_bytes.max(1);
        let fixed_per_column = size_of::<Vec<u8>>()
            .checked_add(size_of::<Vec<f64>>())
            .and_then(|value| value.checked_add(size_of::<ColumnValueStats>()))
            .context("decoded sample column metadata size overflow")?;
        let minimum_bytes = column_count
            .checked_mul(fixed_per_column)
            .context("decoded sample column-count memory size overflow")?;
        if minimum_bytes > max_resident_bytes {
            anyhow::bail!(
                "decoded sample needs at least {minimum_bytes} bytes for {column_count} columns, above the {max_resident_bytes} byte resident-memory budget"
            );
        }

        let accumulator = Self {
            table_buf: Vec::new(),
            column_bufs: (0..column_count).map(|_| Vec::new()).collect(),
            table_per_chunk_ratios: Vec::new(),
            column_per_chunk_ratios: (0..column_count).map(|_| Vec::new()).collect(),
            column_value_stats: (0..column_count)
                .map(|_| ColumnValueStats::default())
                .collect(),
            table_chunk_start: 0,
            table_chunks_seen: 0,
            column_chunk_starts: vec![0; column_count],
            column_chunks_seen: vec![0; column_count],
            sample_rows: 0,
            max_resident_bytes,
        };
        let resident = accumulator.resident_bytes();
        if resident > max_resident_bytes {
            anyhow::bail!(
                "decoded sample allocated {resident} bytes of column metadata, above the {max_resident_bytes} byte resident-memory budget"
            );
        }
        Ok(accumulator)
    }

    pub fn push_row(&mut self, cells: &[OwnedCell]) -> Result<()> {
        if !self.push_row_bounded(cells)? {
            anyhow::bail!(
                "decoded sample reached the {} byte resident-memory budget",
                self.max_resident_bytes
            );
        }
        Ok(())
    }

    pub fn push_row_bounded(&mut self, cells: &[OwnedCell]) -> Result<bool> {
        if cells.len() != self.column_bufs.len() {
            anyhow::bail!(
                "decoded sample row has {} cells but the accumulator expects {}",
                cells.len(),
                self.column_bufs.len()
            );
        }
        let input_bytes = cells_resident_bytes(cells);
        let projected = self.projected_resident_bytes(cells)?;
        let projected_table_len = self
            .table_buf
            .len()
            .checked_add(encoded_row_len_checked(cells)?)
            .context("decoded table sample size overflow")?;
        let compression_headroom = zstd::zstd_safe::compress_bound(projected_table_len);
        if projected
            .checked_add(input_bytes)
            .and_then(|bytes| bytes.checked_add(compression_headroom))
            .is_none_or(|bytes| bytes > self.max_resident_bytes)
        {
            return Ok(false);
        }

        self.reserve_row(cells)?;
        let retained = self.resident_bytes();
        if retained
            .checked_add(input_bytes)
            .and_then(|bytes| bytes.checked_add(compression_headroom))
            .is_none_or(|bytes| bytes > self.max_resident_bytes)
        {
            anyhow::bail!(
                "decoded sample allocator retained {retained} bytes beyond its preflighted budget"
            );
        }
        encode_owned_row(&mut self.table_buf, cells)?;
        if self.table_buf.len().saturating_sub(self.table_chunk_start) >= WIRE_CHUNK_BYTES {
            self.table_chunks_seen = self.table_chunks_seen.saturating_add(1);
            push_chunk_ratio_bounded(
                &mut self.table_per_chunk_ratios,
                &self.table_buf[self.table_chunk_start..],
                self.table_chunks_seen,
            );
            self.table_chunk_start = self.table_buf.len();
        }

        for (idx, cell) in cells.iter().enumerate().take(self.column_bufs.len()) {
            let column_buf = &mut self.column_bufs[idx];
            encode_owned_row(column_buf, std::slice::from_ref(cell))?;
            if column_buf
                .len()
                .saturating_sub(self.column_chunk_starts[idx])
                >= WIRE_CHUNK_BYTES
            {
                self.column_chunks_seen[idx] = self.column_chunks_seen[idx].saturating_add(1);
                push_chunk_ratio_bounded(
                    &mut self.column_per_chunk_ratios[idx],
                    &column_buf[self.column_chunk_starts[idx]..],
                    self.column_chunks_seen[idx],
                );
                self.column_chunk_starts[idx] = column_buf.len();
            }
            self.column_value_stats[idx].push(cell);
        }
        self.sample_rows = self.sample_rows.saturating_add(1);
        Ok(true)
    }

    /// Stored per-chunk ratios plus the not-yet-flushed tail chunk, so the
    /// variance always covers every sampled byte.
    fn ratios_with_tail(stored: &[f64], buf: &[u8], chunk_start: usize) -> Vec<f64> {
        let mut ratios = stored.to_vec();
        if let Some(tail) = buf.get(chunk_start..) {
            if !tail.is_empty() {
                if let Ok(compressed) = zstd::bulk::compress(tail, 3) {
                    if !compressed.is_empty() {
                        ratios.push(tail.len() as f64 / compressed.len() as f64);
                    }
                }
            }
        }
        ratios
    }

    pub fn resident_bytes(&self) -> usize {
        self.table_buf
            .capacity()
            .saturating_add(
                self.column_bufs
                    .capacity()
                    .saturating_mul(size_of::<Vec<u8>>()),
            )
            .saturating_add(self.column_bufs.iter().map(Vec::capacity).sum::<usize>())
            .saturating_add(
                self.table_per_chunk_ratios
                    .capacity()
                    .saturating_mul(size_of::<f64>()),
            )
            .saturating_add(
                self.column_per_chunk_ratios
                    .capacity()
                    .saturating_mul(size_of::<Vec<f64>>()),
            )
            .saturating_add(
                self.column_per_chunk_ratios
                    .iter()
                    .map(|values| values.capacity().saturating_mul(size_of::<f64>()))
                    .sum::<usize>(),
            )
            .saturating_add(
                self.column_value_stats
                    .capacity()
                    .saturating_mul(size_of::<ColumnValueStats>()),
            )
            .saturating_add(
                self.column_value_stats
                    .iter()
                    .map(|stats| stats.lengths.capacity().saturating_mul(size_of::<u64>()))
                    .sum::<usize>(),
            )
            .saturating_add(
                self.column_value_stats
                    .iter()
                    .map(|stats| {
                        stats
                            .fingerprints
                            .capacity()
                            .saturating_mul(size_of::<u64>())
                    })
                    .sum::<usize>(),
            )
    }

    pub fn max_input_row_bytes(&self) -> usize {
        self.max_resident_bytes
            .saturating_sub(self.resident_bytes())
            / 4
    }

    fn projected_resident_bytes(&self, cells: &[OwnedCell]) -> Result<usize> {
        let row_len = encoded_row_len_checked(cells)?;
        let mut projected = self.resident_bytes();
        projected = projected.saturating_add(
            stepped_capacity(self.table_buf.len(), self.table_buf.capacity(), row_len)
                .saturating_sub(self.table_buf.capacity()),
        );
        projected = projected.saturating_add(reservoir_projection(
            &self.table_per_chunk_ratios,
            size_of::<f64>(),
        ));
        for (idx, cell) in cells.iter().enumerate() {
            let cell_len = encoded_row_len_checked(std::slice::from_ref(cell))?;
            projected = projected.saturating_add(
                stepped_capacity(
                    self.column_bufs[idx].len(),
                    self.column_bufs[idx].capacity(),
                    cell_len,
                )
                .saturating_sub(self.column_bufs[idx].capacity()),
            );
            projected = projected.saturating_add(reservoir_projection(
                &self.column_per_chunk_ratios[idx],
                size_of::<f64>(),
            ));
            let stats = &self.column_value_stats[idx];
            if cell.bytes.is_some() {
                projected = projected
                    .saturating_add(reservoir_projection(&stats.lengths, size_of::<u64>()));
                projected = projected
                    .saturating_add(reservoir_projection(&stats.fingerprints, size_of::<u64>()));
            }
        }
        Ok(projected)
    }

    fn reserve_row(&mut self, cells: &[OwnedCell]) -> Result<()> {
        let row_len = encoded_row_len_checked(cells)?;
        reserve_stepped(&mut self.table_buf, row_len)
            .context("reserving decoded table sample bytes")?;
        reserve_reservoir(&mut self.table_per_chunk_ratios)
            .context("reserving decoded table ratio sample")?;
        for (idx, cell) in cells.iter().enumerate() {
            let cell_len = encoded_row_len_checked(std::slice::from_ref(cell))?;
            reserve_stepped(&mut self.column_bufs[idx], cell_len)
                .context("reserving decoded column sample bytes")?;
            reserve_reservoir(&mut self.column_per_chunk_ratios[idx])
                .context("reserving decoded column ratio sample")?;
            if cell.bytes.is_some() {
                reserve_reservoir(&mut self.column_value_stats[idx].lengths)
                    .context("reserving decoded column width sample")?;
                reserve_reservoir(&mut self.column_value_stats[idx].fingerprints)
                    .context("reserving decoded column cardinality sample")?;
            }
        }
        Ok(())
    }

    pub fn table_compression(
        &self,
        sample_method: impl Into<String>,
    ) -> Result<Option<BlueprintCompression>> {
        let ratios = Self::ratios_with_tail(
            &self.table_per_chunk_ratios,
            &self.table_buf,
            self.table_chunk_start,
        );
        compression_blueprint_from_buffer(
            self.table_buf.as_slice(),
            self.sample_rows,
            sample_method.into(),
            ratios.as_slice(),
            CompressionBuildOptions {
                deadline: &SamplingDeadline::unlimited(),
                sampled_with_bias: false,
                bias_reason: "",
                max_temporary_bytes: self
                    .max_resident_bytes
                    .saturating_sub(self.resident_bytes()),
            },
        )
    }

    pub fn table_compression_with_deadline(
        &self,
        sample_method: impl Into<String>,
        deadline: &SamplingDeadline,
        sampled_with_bias: bool,
        bias_reason: &str,
    ) -> Result<Option<BlueprintCompression>> {
        let ratios = Self::ratios_with_tail(
            &self.table_per_chunk_ratios,
            &self.table_buf,
            self.table_chunk_start,
        );
        compression_blueprint_from_buffer(
            self.table_buf.as_slice(),
            self.sample_rows,
            sample_method.into(),
            ratios.as_slice(),
            CompressionBuildOptions {
                deadline,
                sampled_with_bias,
                bias_reason,
                max_temporary_bytes: self
                    .max_resident_bytes
                    .saturating_sub(self.resident_bytes()),
            },
        )
    }

    pub fn column_compressions(
        &self,
        sample_method: impl Into<String>,
    ) -> Result<Vec<Option<BlueprintCompression>>> {
        let sample_method = sample_method.into();
        self.column_bufs
            .iter()
            .zip(self.column_per_chunk_ratios.iter())
            .zip(self.column_chunk_starts.iter())
            .map(|((buf, stored), chunk_start)| {
                let ratios = Self::ratios_with_tail(stored, buf, *chunk_start);
                compression_blueprint_from_buffer(
                    buf.as_slice(),
                    self.sample_rows,
                    sample_method.clone(),
                    ratios.as_slice(),
                    CompressionBuildOptions {
                        deadline: &SamplingDeadline::unlimited(),
                        sampled_with_bias: false,
                        bias_reason: "",
                        max_temporary_bytes: self
                            .max_resident_bytes
                            .saturating_sub(self.resident_bytes()),
                    },
                )
            })
            .collect()
    }

    pub fn column_compressions_with_deadline(
        &self,
        sample_method: impl Into<String>,
        deadline: &SamplingDeadline,
        sampled_with_bias: bool,
        bias_reason: &str,
    ) -> Result<Vec<Option<BlueprintCompression>>> {
        let sample_method = sample_method.into();
        self.column_bufs
            .iter()
            .zip(self.column_per_chunk_ratios.iter())
            .zip(self.column_chunk_starts.iter())
            .map(|((buf, stored), chunk_start)| {
                let ratios = Self::ratios_with_tail(stored, buf, *chunk_start);
                compression_blueprint_from_buffer(
                    buf.as_slice(),
                    self.sample_rows,
                    sample_method.clone(),
                    ratios.as_slice(),
                    CompressionBuildOptions {
                        deadline,
                        sampled_with_bias,
                        bias_reason,
                        max_temporary_bytes: self
                            .max_resident_bytes
                            .saturating_sub(self.resident_bytes()),
                    },
                )
            })
            .collect()
    }

    pub fn column_statistics(&self) -> Vec<DecodedColumnStats> {
        self.column_value_stats
            .iter()
            .map(ColumnValueStats::finish)
            .collect()
    }

    pub fn column_cardinalities(
        &self,
        source_rows: u64,
        sample_method: &str,
        sampled_with_bias: bool,
        bias_reason: &str,
    ) -> Vec<Option<BlueprintCardinality>> {
        self.column_value_stats
            .iter()
            .map(|stats| {
                stats.cardinality(source_rows, sample_method, sampled_with_bias, bias_reason)
            })
            .collect()
    }

    pub fn logical_sample_bytes(&self) -> u64 {
        self.table_buf.len() as u64
    }

    pub fn sample_rows(&self) -> u64 {
        self.sample_rows
    }
}

pub fn type_tag_for_column(column: &BlueprintColumn) -> TypeTag {
    let ty = crate::normalized_type(&column.column_type);
    if crate::is_boolean_type(&ty) {
        TypeTag::BoolText
    } else if crate::is_numeric_type(&ty) {
        TypeTag::NumberText
    } else if ty == "date" {
        TypeTag::DateText
    } else if ty == "time" {
        TypeTag::TimeText
    } else if crate::is_temporal_type(&ty) {
        TypeTag::TimestampText
    } else if ty == "uuid" {
        TypeTag::UuidText
    } else if ty.contains("json") {
        TypeTag::JsonText
    } else if crate::is_binary_type(&ty) {
        TypeTag::BinaryRaw
    } else if crate::is_text_type(&ty) {
        TypeTag::TextUtf8
    } else {
        TypeTag::UnknownText
    }
}

fn encode_owned_row(out: &mut Vec<u8>, cells: &[OwnedCell]) -> Result<()> {
    for cell in cells {
        out.push(cell.tag as u8);
        if matches!(cell.tag, TypeTag::Null) {
            continue;
        }
        let payload = cell.bytes.as_deref().unwrap_or(&[]);
        if payload.len() > u32::MAX as usize {
            anyhow::bail!(
                "single column value exceeds u32 length cap ({} bytes)",
                payload.len()
            );
        }
        write_varint(out, payload.len() as u32);
        out.extend_from_slice(payload);
    }
    Ok(())
}

fn write_varint(out: &mut Vec<u8>, mut value: u32) {
    while value >= 0x80 {
        out.push((value as u8) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

fn push_chunk_ratio_bounded(out: &mut Vec<f64>, chunk_slice: &[u8], seen: u64) {
    if chunk_slice.is_empty() {
        return;
    }
    if let Ok(compressed) = zstd::bulk::compress(chunk_slice, 3) {
        if !compressed.is_empty() {
            reservoir_push(
                out,
                seen,
                chunk_slice.len() as f64 / compressed.len() as f64,
            );
        }
    }
}

fn encoded_row_len_checked(cells: &[OwnedCell]) -> Result<usize> {
    cells.iter().try_fold(0usize, |total, cell| {
        let Some(bytes) = cell.bytes.as_ref() else {
            return total.checked_add(1).context("decoded row size overflow");
        };
        total
            .checked_add(1)
            .and_then(|value| value.checked_add(varint_len(bytes.len() as u64)))
            .and_then(|value| value.checked_add(bytes.len()))
            .context("decoded row size overflow")
    })
}

fn reserve_stepped(
    buf: &mut Vec<u8>,
    additional: usize,
) -> std::result::Result<(), std::collections::TryReserveError> {
    let target = stepped_capacity(buf.len(), buf.capacity(), additional);
    if target > buf.capacity() {
        buf.try_reserve_exact(target - buf.len())?;
    }
    Ok(())
}

/// Bounded reservoirs reserve their full fixed capacity on first use, so
/// they never reallocate afterwards.
fn reserve_reservoir<T>(
    values: &mut Vec<T>,
) -> std::result::Result<(), std::collections::TryReserveError> {
    if values.capacity() == 0 {
        values.try_reserve_exact(RESERVOIR_CAPACITY)?;
    }
    Ok(())
}

fn reservoir_projection<T>(values: &Vec<T>, element_size: usize) -> usize {
    if values.capacity() == 0 {
        RESERVOIR_CAPACITY.saturating_mul(element_size)
    } else {
        0
    }
}

fn cells_resident_bytes(cells: &[OwnedCell]) -> usize {
    cells
        .len()
        .saturating_mul(size_of::<OwnedCell>())
        .saturating_add(
            cells
                .iter()
                .filter_map(|cell| cell.bytes.as_ref())
                .map(Vec::capacity)
                .sum::<usize>(),
        )
}

fn varint_len(mut value: u64) -> usize {
    let mut len = 1;
    while value >= 0x80 {
        value >>= 7;
        len += 1;
    }
    len
}

fn reservoir_push<T: Copy>(values: &mut Vec<T>, seen: u64, value: T) {
    if values.len() < RESERVOIR_CAPACITY {
        values.push(value);
        return;
    }
    let slot = reservoir_mix(seen) % seen.max(1);
    if slot < RESERVOIR_CAPACITY as u64 {
        values[slot as usize] = value;
    }
}

fn reservoir_mix(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn fingerprint_cell(tag: TypeTag, bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64 ^ tag as u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    reservoir_mix(hash ^ bytes.len() as u64)
}

fn quantile(sorted: &[u64], percentile: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = ((sorted.len() as f64 * percentile).ceil() as usize)
        .saturating_sub(1)
        .min(sorted.len() - 1);
    quantize_stat_count(sorted[rank])
}

pub fn quantize_stat_count(value: u64) -> u64 {
    if value <= 32 {
        return value;
    }
    let magnitude = 1_u64 << (63 - value.leading_zeros());
    let bucket = (magnitude / 16).max(1);
    crate::round_to_bucket(value, bucket)
}

pub fn quantize_fraction(value: f64) -> f64 {
    if !value.is_finite() {
        return 0.0;
    }
    (value.clamp(0.0, 1.0) * 200.0).round() / 200.0
}

struct CompressionBuildOptions<'a> {
    deadline: &'a SamplingDeadline,
    sampled_with_bias: bool,
    bias_reason: &'a str,
    max_temporary_bytes: usize,
}

fn compression_blueprint_from_buffer(
    buf: &[u8],
    sample_rows: u64,
    sample_method: String,
    per_chunk_ratios: &[f64],
    options: CompressionBuildOptions<'_>,
) -> Result<Option<BlueprintCompression>> {
    if buf.is_empty() || sample_rows == 0 {
        return Ok(None);
    }
    let compression_bound = zstd::zstd_safe::compress_bound(buf.len());
    if compression_bound > options.max_temporary_bytes {
        anyhow::bail!(
            "decoded sample needs {compression_bound} temporary compression bytes but only {} bytes remain in its resident-memory budget",
            options.max_temporary_bytes
        );
    }
    options
        .deadline
        .check("starting zstd level 3 compression")?;
    let comp_3 = zstd::bulk::compress(buf, 3).context("zstd lvl 3 decoded file sample")?;
    options
        .deadline
        .check("finishing zstd level 3 compression")?;
    let comp_3_len = comp_3.len();
    drop(comp_3);
    if comp_3_len == 0 {
        return Ok(None);
    }
    let r3 = buf.len() as f64 / comp_3_len as f64;
    let stddev = if per_chunk_ratios.len() > 1 {
        let mean = per_chunk_ratios.iter().sum::<f64>() / per_chunk_ratios.len() as f64;
        let var = per_chunk_ratios
            .iter()
            .map(|r| (r - mean).powi(2))
            .sum::<f64>()
            / per_chunk_ratios.len() as f64;
        var.sqrt()
    } else {
        0.0
    };
    Ok(Some(BlueprintCompression {
        measured: true,
        sample_rows,
        sample_bytes: round_sample_bytes(buf.len() as u64),
        sample_method,
        sampled_with_bias: options.sampled_with_bias,
        bias_reason: if options.sampled_with_bias {
            options.bias_reason.to_string()
        } else {
            String::new()
        },
        ratio_zstd_3: round_ratio(r3),
        ratio_stddev: round_ratio(stddev),
        sample_encoding: SAMPLE_ENCODING_TAG.to_string(),
        ..Default::default()
    }))
}

pub fn round_sample_bytes(n: u64) -> u64 {
    let bucket = if n < 1_048_576 {
        64 * 1024
    } else if n < 1_073_741_824 {
        1_048_576
    } else {
        100 * 1_048_576
    };
    crate::round_to_bucket(n, bucket)
}

pub fn round_ratio(r: f64) -> f64 {
    if !r.is_finite() {
        return 0.0;
    }
    (r * 20.0).round() / 20.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rowframe_encoder_matches_expected_bytes() {
        let mut acc = CompressionSampleAccumulator::new(2);
        acc.push_row(&[
            OwnedCell::new(TypeTag::TextUtf8, b"hello".to_vec()),
            OwnedCell::null(),
        ])
        .unwrap();
        assert_eq!(
            acc.table_buf,
            vec![0x01, 0x05, b'h', b'e', b'l', b'l', b'o', 0x00]
        );
    }

    #[test]
    fn compression_summary_uses_rowframe_tag() {
        let mut acc = CompressionSampleAccumulator::new(1);
        for _ in 0..20 {
            acc.push_row(&[OwnedCell::new(
                TypeTag::TextUtf8,
                b"repeat repeat repeat".to_vec(),
            )])
            .unwrap();
        }
        let blueprint = acc.table_compression("test sample").unwrap().unwrap();
        assert!(blueprint.measured);
        assert_eq!(blueprint.sample_encoding, SAMPLE_ENCODING_TAG);
        assert!(blueprint.ratio_zstd_3 > 1.0);
    }

    #[test]
    fn resident_budget_accounts_for_empty_non_null_length_prefix() {
        let cells = [OwnedCell::new(TypeTag::TextUtf8, Vec::new())];
        assert_eq!(encoded_row_len_checked(&cells).unwrap(), 2);

        let baseline = CompressionSampleAccumulator::new(1).resident_bytes();
        let mut acc =
            CompressionSampleAccumulator::with_max_resident_bytes(1, baseline + 1).unwrap();
        assert!(!acc.push_row_bounded(&cells).unwrap());
        assert_eq!(acc.sample_rows(), 0);
    }

    #[test]
    fn wide_schema_is_rejected_before_per_column_buffers_are_allocated() {
        let error = CompressionSampleAccumulator::with_max_resident_bytes(100_000, 1024)
            .expect_err("column metadata alone exceeds the cap");
        assert!(error.to_string().contains("above the 1024 byte"));
    }

    #[test]
    fn accumulators_do_not_eagerly_reserve_payload_or_reservoir_buffers() {
        let acc = CompressionSampleAccumulator::with_max_resident_bytes(8, 64 * 1024).unwrap();
        assert_eq!(acc.table_buf.capacity(), 0);
        assert_eq!(acc.table_per_chunk_ratios.capacity(), 0);
        assert!(acc.column_bufs.iter().all(|buffer| buffer.capacity() == 0));
        assert!(acc
            .column_per_chunk_ratios
            .iter()
            .all(|buffer| buffer.capacity() == 0));
        assert!(acc
            .column_value_stats
            .iter()
            .all(|statistics| statistics.lengths.capacity() == 0));
        assert!(acc.resident_bytes() <= 64 * 1024);
    }

    #[test]
    fn oversized_cell_is_refused_without_breaking_the_resident_cap() {
        let baseline = CompressionSampleAccumulator::new(1).resident_bytes();
        let cap = baseline + 256;
        let mut acc = CompressionSampleAccumulator::with_max_resident_bytes(1, cap).unwrap();
        let cell = OwnedCell::new(TypeTag::BinaryRaw, vec![0xA5; 16 * 1024]);
        assert!(!acc.push_row_bounded(&[cell]).unwrap());
        assert_eq!(acc.sample_rows(), 0);
        assert!(acc.resident_bytes() <= cap);
    }

    #[test]
    fn deterministic_first_n_compression_is_marked_biased() {
        let mut acc = CompressionSampleAccumulator::new(1);
        acc.push_row(&[OwnedCell::new(TypeTag::TextUtf8, b"alpha".to_vec())])
            .unwrap();
        let options = DecodedCompressionOptions::enabled(1, "table-first-n", "column-first-n");
        let compression = acc
            .table_compression_with_deadline(
                options.table_sample_method,
                &SamplingDeadline::unlimited(),
                options.sampled_with_bias,
                &options.bias_reason,
            )
            .unwrap()
            .unwrap();
        assert!(compression.sampled_with_bias);
        assert_eq!(compression.bias_reason, FIRST_N_BIAS_REASON);
    }

    #[test]
    fn final_compression_obeys_the_shared_deadline() {
        let mut acc = CompressionSampleAccumulator::new(1);
        acc.push_row(&[OwnedCell::new(TypeTag::TextUtf8, vec![b'x'; 4096])])
            .unwrap();
        let error = acc
            .table_compression_with_deadline(
                "deadline-test",
                &SamplingDeadline::after(Duration::ZERO),
                false,
                "",
            )
            .expect_err("expired deadline must stop final zstd work");
        assert!(error.to_string().contains("deadline expired"));
    }

    #[test]
    fn cardinality_summary_retains_only_aggregates_and_distinguishes_skew() {
        let mut acc = CompressionSampleAccumulator::new(1);
        for row in 0..1_000_u64 {
            let value = if row < 400 {
                b"hot".to_vec()
            } else {
                format!("tail-{}", row % 10).into_bytes()
            };
            acc.push_row(&[OwnedCell::new(TypeTag::TextUtf8, value)])
                .unwrap();
        }
        let cardinality = acc.column_cardinalities(10_000, "bounded-test", false, "")[0]
            .clone()
            .unwrap();
        assert!(cardinality.measured);
        assert_eq!(cardinality.sample_rows, 992);
        assert_eq!(cardinality.observed_distinct_count, 11);
        assert!(cardinality.top_value_fraction >= 0.39);
        assert!(cardinality.frequency_max >= 390);
        let encoded = toml::to_string(&cardinality).unwrap();
        assert!(!encoded.contains("hot"));
        assert!(!encoded.contains("tail"));
    }

    #[test]
    fn biased_unique_like_cardinality_is_an_observed_lower_bound() {
        let mut acc = CompressionSampleAccumulator::new(1);
        for row in 0..1_000_u64 {
            acc.push_row(&[OwnedCell::new(
                TypeTag::NumberText,
                row.to_string().into_bytes(),
            )])
            .unwrap();
        }
        let cardinality = acc.column_cardinalities(100_000, "bounded-test", true, "first-n")[0]
            .clone()
            .unwrap();
        assert_eq!(cardinality.estimated_distinct_count, 992);
        assert!(cardinality.sampled_with_bias);
        assert_eq!(cardinality.bias_reason, "first-n");
        assert!(cardinality.sample_method.contains("observed lower bound"));
    }

    #[test]
    fn unbiased_cardinality_uses_chao1_collision_evidence() {
        let mut acc = CompressionSampleAccumulator::new(1);
        for row in 0..1_000_u64 {
            acc.push_row(&[OwnedCell::new(
                TypeTag::NumberText,
                (row % 500).to_string().into_bytes(),
            )])
            .unwrap();
        }
        let cardinality = acc.column_cardinalities(100_000, "random", false, "")[0]
            .clone()
            .unwrap();
        assert_eq!(cardinality.estimated_distinct_count, 496);
        assert!(cardinality.sample_method.contains("Chao1"));
    }
}
