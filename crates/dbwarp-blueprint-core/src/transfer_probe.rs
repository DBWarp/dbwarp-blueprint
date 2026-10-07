//! Neutral columnar compression probe used by Blueprint capture.
//!
//! The probe frames rows column by column: bounded row groups, one type
//! descriptor per column, value lengths, then payload bytes. Readers must use
//! the encoding tag to distinguish its table ratios from row-major per-column
//! measurements.

use anyhow::{bail, Context, Result};

#[cfg(feature = "sampling")]
use zstd::stream::raw::{Encoder as ZstdRawEncoder, InBuffer, Operation, OutBuffer};

/// Rows per probe frame.
pub const TRANSFER_PROBE_FRAME_ROWS: usize = 1_000;
/// Probe frame byte ceilings: a smaller first frame and larger continuation
/// frames.
pub const TRANSFER_PROBE_STREAMING_FIRST_FRAME_BYTES: usize = 4 * 1024 * 1024;
pub const TRANSFER_PROBE_STREAMING_FRAME_BYTES: usize = 16 * 1024 * 1024;
pub const TRANSFER_PROBE_MAX_SAMPLE_BYTES: usize = 16 * 1024 * 1024;
/// Maximum bytes used by the canonical sampling-cell envelope: one type/null
/// tag plus the five-byte worst-case u32 payload-length varint.
pub const TRANSFER_PROBE_CELL_OVERHEAD_BYTES: u64 = 6;

const FRAME_MAGIC: [u8; 4] = *b"BTP1";
const NULL_LENGTH: u32 = u32::MAX;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferProbeColumnShape {
    Fixed { payload_reserve: u64 },
    Variable { declared_max_bytes: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferProbeSamplePlan {
    pub sample_rows: u64,
    /// Per-column byte budgets before a text projection converts bytes to its
    /// engine-specific character limit. Fixed-width columns contain zero.
    pub variable_byte_limits: Vec<u64>,
}

/// Compression sizes observed while feeding probe frames through one zstd
/// streaming context and flushing after every frame. The compressor preserves
/// its history across those flushes, so this deliberately differs
/// from compressing every frame independently or pledging one concatenated
/// input size.
#[cfg(feature = "sampling")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferProbeStreamingMeasurement {
    pub total_input_bytes: usize,
    pub total_compressed_bytes: usize,
    pub frame_compressed_bytes: Vec<usize>,
}

#[cfg(feature = "sampling")]
pub fn measure_zstd_streaming_probe_frames(
    frames: &[Vec<u8>],
    level: i32,
) -> Result<TransferProbeStreamingMeasurement> {
    measure_zstd_streaming_probe_frames_with_chunk_bytes(frames, level, usize::MAX)
}

/// Measure a neutral probe while flushing the persistent zstd context at the
/// caller-selected probe chunk boundary. `frames` retain the
/// neutral row-group geometry used for per-frame variance; compressed byte
/// counts aggregate all probe chunks belonging to each neutral frame.
#[cfg(feature = "sampling")]
pub fn measure_zstd_streaming_probe_frames_with_chunk_bytes(
    frames: &[Vec<u8>],
    level: i32,
    compression_chunk_bytes: usize,
) -> Result<TransferProbeStreamingMeasurement> {
    if frames.is_empty() {
        bail!("streaming transfer probe requires at least one frame");
    }
    if frames.iter().any(Vec::is_empty) {
        bail!("streaming transfer probe frames must not be empty");
    }
    if compression_chunk_bytes == 0 {
        bail!("streaming transfer probe compression chunk size must be non-zero");
    }

    let mut encoder =
        ZstdRawEncoder::new(level).context("initializing streaming zstd transfer-probe encoder")?;
    let mut total_input_bytes = 0usize;
    let mut total_compressed_bytes = 0usize;
    let mut frame_compressed_bytes = Vec::with_capacity(frames.len());

    for frame in frames {
        let mut frame_compressed_len = 0usize;
        for chunk in frame.chunks(compression_chunk_bytes) {
            let bound = zstd::zstd_safe::compress_bound(chunk.len());
            let mut output = vec![0_u8; bound.saturating_add(256).max(256)];
            let mut output_position = 0usize;
            let mut input = InBuffer::around(chunk);
            while input.pos() < chunk.len() {
                if output_position == output.len() {
                    output.resize(output.len().saturating_mul(2).max(256), 0);
                }
                let mut out = OutBuffer::around_pos(output.as_mut_slice(), output_position);
                encoder
                    .run(&mut input, &mut out)
                    .context("compressing streaming zstd transfer-probe chunk")?;
                output_position = out.pos();
            }
            loop {
                if output_position == output.len() {
                    output.resize(output.len().saturating_mul(2).max(256), 0);
                }
                let mut out = OutBuffer::around_pos(output.as_mut_slice(), output_position);
                let remaining = encoder
                    .flush(&mut out)
                    .context("flushing streaming zstd transfer-probe chunk")?;
                output_position = out.pos();
                if remaining == 0 {
                    break;
                }
            }
            if output_position == 0 {
                bail!("streaming zstd transfer probe produced an empty chunk");
            }
            frame_compressed_len = frame_compressed_len
                .checked_add(output_position)
                .context("streaming transfer-probe frame compressed size overflow")?;
        }
        total_input_bytes = total_input_bytes
            .checked_add(frame.len())
            .context("streaming transfer-probe input size overflow")?;
        total_compressed_bytes = total_compressed_bytes
            .checked_add(frame_compressed_len)
            .context("streaming transfer-probe compressed size overflow")?;
        frame_compressed_bytes.push(frame_compressed_len);
    }

    Ok(TransferProbeStreamingMeasurement {
        total_input_bytes,
        total_compressed_bytes,
        frame_compressed_bytes,
    })
}

/// Allocate a bounded sample across heterogeneous columns. Known narrow
/// columns receive only their declared capacity; the remainder is
/// redistributed to wide/unbounded values. This avoids truncating one LOB to
/// an equal per-cell share merely because the same table has many fixed or
/// narrow columns.
pub fn plan_transfer_probe_sample(
    requested_rows: u64,
    columns: &[TransferProbeColumnShape],
) -> Result<TransferProbeSamplePlan> {
    if columns.is_empty() {
        bail!("transfer probe sample plan requires at least one column");
    }
    let column_count = u64::try_from(columns.len()).unwrap_or(u64::MAX);
    let fixed_payload = columns.iter().fold(0_u64, |total, column| match column {
        TransferProbeColumnShape::Fixed { payload_reserve } => {
            total.saturating_add((*payload_reserve).max(1))
        }
        TransferProbeColumnShape::Variable { .. } => total,
    });
    let variable_count = columns
        .iter()
        .filter(|column| matches!(column, TransferProbeColumnShape::Variable { .. }))
        .count() as u64;
    let minimum_row_bytes = column_count
        .saturating_mul(TRANSFER_PROBE_CELL_OVERHEAD_BYTES)
        .saturating_add(fixed_payload)
        .saturating_add(variable_count)
        .max(1);
    let max_sample_bytes = TRANSFER_PROBE_MAX_SAMPLE_BYTES as u64;
    let maximum_rows = (max_sample_bytes / minimum_row_bytes).max(1);
    let sample_rows = requested_rows.max(1).min(maximum_rows);
    let row_budget = max_sample_bytes / sample_rows;
    let variable_budget = row_budget
        .saturating_sub(
            column_count
                .saturating_mul(TRANSFER_PROBE_CELL_OVERHEAD_BYTES)
                .saturating_add(fixed_payload),
        )
        .max(variable_count);

    let mut pending = columns
        .iter()
        .enumerate()
        .filter_map(|(index, column)| match column {
            TransferProbeColumnShape::Variable { declared_max_bytes } => Some((
                index,
                if *declared_max_bytes == 0 {
                    u64::MAX
                } else {
                    *declared_max_bytes
                },
            )),
            TransferProbeColumnShape::Fixed { .. } => None,
        })
        .collect::<Vec<_>>();
    let mut variable_byte_limits = vec![0_u64; columns.len()];
    let mut remaining = variable_budget;
    while !pending.is_empty() {
        let pending_count = pending.len() as u64;
        let share = (remaining / pending_count).max(1);
        let bounded = pending
            .iter()
            .enumerate()
            .filter_map(|(position, (_, desired))| (*desired <= share).then_some(position))
            .collect::<Vec<_>>();
        if bounded.is_empty() {
            let remainder = remaining % pending_count;
            for (position, (index, _)) in pending.into_iter().enumerate() {
                variable_byte_limits[index] =
                    share.saturating_add(u64::from((position as u64) < remainder));
            }
            break;
        }
        for position in bounded.into_iter().rev() {
            let (index, desired) = pending.swap_remove(position);
            variable_byte_limits[index] = desired.max(1);
            remaining = remaining.saturating_sub(desired.max(1));
        }
    }

    Ok(TransferProbeSamplePlan {
        sample_rows,
        variable_byte_limits,
    })
}

#[derive(Debug, Clone, Copy)]
struct EncodedCell {
    tag: u8,
    payload_start: usize,
    payload_end: usize,
}

impl EncodedCell {
    fn payload_len(self) -> usize {
        self.payload_end.saturating_sub(self.payload_start)
    }
}

/// Convert canonical `blueprint-compression-probe-v2` per-column buffers into
/// bounded, columnar table frames. The returned frames are deterministic and
/// contain no source values beyond the transient input sample; callers must
/// compress and discard them in memory.
pub fn encode_columnar_transfer_probe_frames(
    column_buffers: &[Vec<u8>],
    sample_rows: u64,
) -> Result<Vec<Vec<u8>>> {
    if column_buffers.is_empty() {
        bail!("columnar transfer probe requires at least one column");
    }
    if column_buffers.len() > u16::MAX as usize {
        bail!("columnar transfer probe exceeds the u16 column-count limit");
    }
    let row_count = usize::try_from(sample_rows)
        .context("columnar transfer probe row count exceeds platform limits")?;
    if row_count == 0 {
        bail!("columnar transfer probe requires at least one row");
    }

    let decoded = column_buffers
        .iter()
        .enumerate()
        .map(|(ordinal, bytes)| {
            decode_column(bytes, row_count)
                .with_context(|| format!("decoding compression probe column {}", ordinal + 1))
        })
        .collect::<Result<Vec<_>>>()?;

    let mut frames = Vec::with_capacity(row_count.div_ceil(TRANSFER_PROBE_FRAME_ROWS));
    for row_start in (0..row_count).step_by(TRANSFER_PROBE_FRAME_ROWS) {
        let row_end = row_start
            .saturating_add(TRANSFER_PROBE_FRAME_ROWS)
            .min(row_count);
        frames.push(encode_columnar_transfer_probe_frame(
            column_buffers,
            decoded.as_slice(),
            row_start,
            row_end,
        )?);
    }
    Ok(frames)
}

/// Encode the current v2 measurement frames using the probe's first-frame and
/// continuation-frame byte ceilings. This models generic batching effects;
/// each frame remains independently bounded.
pub fn encode_columnar_transfer_probe_streaming_frames(
    column_buffers: &[Vec<u8>],
    sample_rows: u64,
) -> Result<Vec<Vec<u8>>> {
    if column_buffers.is_empty() || column_buffers.len() > u16::MAX as usize {
        bail!("streaming columnar transfer probe has an invalid column count");
    }
    let row_count = usize::try_from(sample_rows)
        .context("streaming columnar transfer probe row count exceeds platform limits")?;
    if row_count == 0 {
        bail!("streaming columnar transfer probe requires at least one row");
    }
    let decoded = column_buffers
        .iter()
        .enumerate()
        .map(|(ordinal, bytes)| {
            decode_column(bytes, row_count)
                .with_context(|| format!("decoding compression probe column {}", ordinal + 1))
        })
        .collect::<Result<Vec<_>>>()?;
    let fixed_header = 10usize.saturating_add(column_buffers.len().saturating_mul(4));
    let per_row_lengths = column_buffers.len().saturating_mul(4);
    let mut frames = Vec::new();
    let mut row_start = 0usize;
    while row_start < row_count {
        let frame_byte_limit = if frames.is_empty() {
            TRANSFER_PROBE_STREAMING_FIRST_FRAME_BYTES
        } else {
            TRANSFER_PROBE_STREAMING_FRAME_BYTES
        };
        let mut row_end = row_start;
        let mut estimated = fixed_header;
        while row_end < row_count {
            let payload = decoded
                .iter()
                .map(|column| column[row_end].payload_len())
                .sum::<usize>();
            let next = estimated
                .saturating_add(per_row_lengths)
                .saturating_add(payload);
            if row_end > row_start && next > frame_byte_limit {
                break;
            }
            estimated = next;
            row_end += 1;
        }
        frames.push(encode_columnar_transfer_probe_frame(
            column_buffers,
            decoded.as_slice(),
            row_start,
            row_end,
        )?);
        row_start = row_end;
    }
    Ok(frames)
}

fn encode_columnar_transfer_probe_frame(
    column_buffers: &[Vec<u8>],
    decoded: &[Vec<EncodedCell>],
    row_start: usize,
    row_end: usize,
) -> Result<Vec<u8>> {
    let frame_rows = row_end.saturating_sub(row_start);
    let mut estimated = 10usize;
    for column in decoded {
        estimated = estimated
            .saturating_add(4)
            .saturating_add(frame_rows.saturating_mul(4));
        for cell in &column[row_start..row_end] {
            estimated = estimated.saturating_add(cell.payload_len());
        }
    }
    let mut frame = Vec::with_capacity(estimated);
    frame.extend_from_slice(&FRAME_MAGIC);
    frame.extend_from_slice(&(frame_rows as u32).to_be_bytes());
    frame.extend_from_slice(&(column_buffers.len() as u16).to_be_bytes());
    for (ordinal, column) in decoded.iter().enumerate() {
        let cells = &column[row_start..row_end];
        let type_tag = cells
            .iter()
            .find(|cell| cell.tag != 0)
            .map_or(0, |cell| cell.tag);
        frame.extend_from_slice(&(ordinal as u16).to_be_bytes());
        frame.push(type_tag);
        frame.push(0);
        for cell in cells {
            let length = if cell.tag == 0 {
                NULL_LENGTH
            } else {
                u32::try_from(cell.payload_len())
                    .context("columnar transfer probe cell exceeds u32 length limit")?
            };
            frame.extend_from_slice(&length.to_be_bytes());
        }
        for cell in cells {
            frame.extend_from_slice(&column_buffers[ordinal][cell.payload_start..cell.payload_end]);
        }
    }
    debug_assert_eq!(frame.len(), estimated);
    Ok(frame)
}

/// Concatenate a frame sequence for one-shot zstd measurement. Keeping the
/// frame boundaries in the uncompressed representation preserves repeated
/// descriptors and length vectors while allowing zstd to learn across the
/// bounded sample.
pub fn concatenate_transfer_probe_frames(frames: &[Vec<u8>]) -> Result<Vec<u8>> {
    if frames.is_empty() {
        bail!("columnar transfer probe produced no frames");
    }
    let total = frames
        .iter()
        .try_fold(0usize, |sum, frame| sum.checked_add(frame.len()))
        .context("columnar transfer probe size overflow")?;
    let mut output = Vec::with_capacity(total);
    for frame in frames {
        output.extend_from_slice(frame);
    }
    Ok(output)
}

fn decode_column(bytes: &[u8], row_count: usize) -> Result<Vec<EncodedCell>> {
    let mut cells = Vec::with_capacity(row_count);
    let mut cursor = 0usize;
    let mut non_null_tag = None;
    while cursor < bytes.len() && cells.len() < row_count {
        let tag = bytes[cursor];
        cursor += 1;
        if tag == 0 {
            cells.push(EncodedCell {
                tag,
                payload_start: cursor,
                payload_end: cursor,
            });
            continue;
        }
        if let Some(expected) = non_null_tag {
            if tag != expected {
                bail!("column changes type tag from 0x{expected:02x} to 0x{tag:02x}");
            }
        } else {
            non_null_tag = Some(tag);
        }
        let length = read_varint(bytes, &mut cursor)? as usize;
        let end = cursor
            .checked_add(length)
            .context("compression probe cell length overflow")?;
        if end > bytes.len() {
            bail!("compression probe cell payload is truncated");
        }
        cells.push(EncodedCell {
            tag,
            payload_start: cursor,
            payload_end: end,
        });
        cursor = end;
    }
    if cells.len() != row_count {
        bail!(
            "column contains {} cells but the sample declares {row_count} rows",
            cells.len()
        );
    }
    if cursor != bytes.len() {
        bail!("column contains trailing bytes after {row_count} cells");
    }
    Ok(cells)
}

fn read_varint(bytes: &[u8], cursor: &mut usize) -> Result<u32> {
    let mut value = 0u32;
    for shift in (0..=28).step_by(7) {
        let byte = *bytes
            .get(*cursor)
            .context("compression probe length varint is truncated")?;
        *cursor += 1;
        if shift == 28 && byte > 0x0f {
            bail!("compression probe length varint exceeds u32");
        }
        value |= u32::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    bail!("compression probe length varint exceeds five bytes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::append_probe_cell;

    #[test]
    fn encodes_columnar_lengths_then_payloads() {
        let mut first = Vec::new();
        let mut second = Vec::new();
        append_probe_cell(&mut first, 0x04, Some(b"7"));
        append_probe_cell(&mut first, 0x04, Some(b"42"));
        append_probe_cell(&mut second, 0x01, Some(b"a"));
        append_probe_cell(&mut second, 0x01, None);

        let frames = encode_columnar_transfer_probe_frames(&[first, second], 2).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(&frames[0][..10], b"BTP1\0\0\0\x02\0\x02");
        assert_eq!(&frames[0][10..14], b"\0\0\x04\0");
        assert_eq!(&frames[0][14..22], b"\0\0\0\x01\0\0\0\x02");
        assert_eq!(&frames[0][22..25], b"742");
        assert_eq!(&frames[0][25..29], b"\0\x01\x01\0");
        assert_eq!(&frames[0][29..37], b"\0\0\0\x01\xff\xff\xff\xff");
        assert_eq!(&frames[0][37..], b"a");
    }

    #[test]
    fn splits_at_the_probe_row_limit() {
        let mut column = Vec::new();
        for row in 0..1_001u32 {
            append_probe_cell(&mut column, 0x04, Some(row.to_string().as_bytes()));
        }
        let frames = encode_columnar_transfer_probe_frames(&[column], 1_001).unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(
            u32::from_be_bytes(frames[0][4..8].try_into().unwrap()),
            1_000
        );
        assert_eq!(u32::from_be_bytes(frames[1][4..8].try_into().unwrap()), 1);
    }

    #[test]
    fn streaming_frames_split_by_bytes_and_preserve_every_row() {
        let payload = vec![b'x'; 1_024];
        let mut column = Vec::new();
        for _ in 0..12_000 {
            append_probe_cell(&mut column, 0x01, Some(payload.as_slice()));
        }
        let frames = encode_columnar_transfer_probe_streaming_frames(&[column], 12_000).unwrap();
        assert_eq!(frames.len(), 2);
        assert!(frames[0].len() <= TRANSFER_PROBE_STREAMING_FIRST_FRAME_BYTES);
        assert!(frames[1].len() > TRANSFER_PROBE_STREAMING_FIRST_FRAME_BYTES);
        assert!(frames[1].len() <= TRANSFER_PROBE_STREAMING_FRAME_BYTES);
        let rows = frames
            .iter()
            .map(|frame| u32::from_be_bytes(frame[4..8].try_into().unwrap()) as u64)
            .sum::<u64>();
        assert_eq!(rows, 12_000);
    }

    #[test]
    fn rejects_inconsistent_rows_tags_and_trailing_bytes() {
        let mut short = Vec::new();
        append_probe_cell(&mut short, 0x01, Some(b"one"));
        assert!(encode_columnar_transfer_probe_frames(&[short], 2).is_err());

        let mut mixed = Vec::new();
        append_probe_cell(&mut mixed, 0x01, Some(b"one"));
        append_probe_cell(&mut mixed, 0x04, Some(b"2"));
        assert!(encode_columnar_transfer_probe_frames(&[mixed], 2).is_err());

        let mut trailing = Vec::new();
        append_probe_cell(&mut trailing, 0x01, Some(b"one"));
        trailing.push(0);
        assert!(encode_columnar_transfer_probe_frames(&[trailing], 1).is_err());
    }

    #[test]
    fn concatenation_is_deterministic_and_exact() {
        let frames = vec![b"abc".to_vec(), b"defg".to_vec()];
        assert_eq!(
            concatenate_transfer_probe_frames(&frames).unwrap(),
            b"abcdefg"
        );
        assert!(concatenate_transfer_probe_frames(&[]).is_err());
    }

    #[cfg(feature = "sampling")]
    #[test]
    fn streaming_measurement_preserves_one_context_across_frame_flushes() {
        let frames = vec![
            b"repeated-blueprint-probe-value|".repeat(2_048),
            b"repeated-blueprint-probe-value|".repeat(2_048),
        ];
        let first = measure_zstd_streaming_probe_frames(&frames, 3).unwrap();
        let second = measure_zstd_streaming_probe_frames(&frames, 3).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.total_input_bytes,
            frames.iter().map(Vec::len).sum::<usize>()
        );
        assert_eq!(first.frame_compressed_bytes.len(), frames.len());
        assert_eq!(
            first.total_compressed_bytes,
            first.frame_compressed_bytes.iter().sum::<usize>()
        );
        assert!(first.total_compressed_bytes > 0);
        assert!(first.total_compressed_bytes < first.total_input_bytes);
        assert!(measure_zstd_streaming_probe_frames(&[], 3).is_err());
    }

    #[cfg(feature = "sampling")]
    #[test]
    fn streaming_measurement_honors_probe_chunk_boundary() {
        let frames = vec![b"same-value-with-useful-locality|".repeat(16_384)];
        let whole = measure_zstd_streaming_probe_frames(&frames, 3).unwrap();
        let chunked =
            measure_zstd_streaming_probe_frames_with_chunk_bytes(&frames, 3, 64 * 1024).unwrap();
        assert_eq!(chunked.total_input_bytes, whole.total_input_bytes);
        assert_eq!(chunked.frame_compressed_bytes.len(), frames.len());
        assert_eq!(
            chunked.total_compressed_bytes,
            chunked.frame_compressed_bytes.iter().sum::<usize>()
        );
        assert_ne!(chunked.total_compressed_bytes, whole.total_compressed_bytes);
        assert!(measure_zstd_streaming_probe_frames_with_chunk_bytes(&frames, 3, 0).is_err());
    }

    #[test]
    fn sample_plan_redistributes_narrow_column_capacity_to_a_lob() {
        let columns = [
            TransferProbeColumnShape::Fixed {
                payload_reserve: 21,
            },
            TransferProbeColumnShape::Variable {
                declared_max_bytes: 480,
            },
            TransferProbeColumnShape::Variable {
                declared_max_bytes: 20,
            },
            TransferProbeColumnShape::Variable {
                declared_max_bytes: u32::MAX.into(),
            },
            TransferProbeColumnShape::Variable {
                declared_max_bytes: 16,
            },
        ];
        let plan = plan_transfer_probe_sample(4_096, &columns).unwrap();
        assert_eq!(plan.sample_rows, 4_096);
        assert_eq!(plan.variable_byte_limits[1], 480);
        assert_eq!(plan.variable_byte_limits[2], 20);
        assert!(plan.variable_byte_limits[3] / 4 >= 291);
        assert_eq!(plan.variable_byte_limits[4], 16);
    }

    #[test]
    fn sample_plan_reduces_rows_for_pathologically_wide_fixed_schemas() {
        let columns = vec![
            TransferProbeColumnShape::Fixed {
                payload_reserve: 69,
            };
            1_600
        ];
        let plan = plan_transfer_probe_sample(4_096, &columns).unwrap();
        assert!(plan.sample_rows < 4_096);
        assert_eq!(plan.variable_byte_limits, vec![0; columns.len()]);
    }
}
