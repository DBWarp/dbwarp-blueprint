//! Row-frame encoder for Tier-2 compression sampling.
//!
//! ## Design goals
//!
//! 1. **Faithful byte distribution.** The byte stream we hand to zstd must
//!    have the same character distribution and redundancy patterns as the
//!    bytes a customer's production traffic actually moves. That means:
//!    text in its native charset (UTF-8 for PG, UTF-16LE for MSSQL nvarchar,
//!    column-charset bytes for MySQL); numbers and timestamps as their
//!    DB textual decimal form; binary as raw bytes.
//! 2. **Type-tagged but compact.** A small per-column tag (1 byte) plus a
//!    u32 length prefix carries enough metadata for the dbwarp estimator
//!    to do per-type variance analysis later, without bloating the buffer
//!    so much that the framing overhead distorts the measured ratio.
//! 3. **Engine-agnostic encoder, engine-specific value mapping.** This
//!    module owns the framing and the type-tag enum. Each engine module
//!    decides how to map its driver-native values onto a `Cell`.
//! 4. **Deterministic.** Two runs on the same row data produce byte-
//!    identical output; the only non-deterministic parts of a Tier-2
//!    sample are the rows TABLESAMPLE picked. Encoding order within a
//!    row is preserved as the column ordinal order from the engine.
//!
//! ## Encoding (`dbwarp-blueprint-rowframe-v1`)
//!
//! Minimal framing — every byte that is not row content adds noise to
//! the compression ratio without contributing to the estimate. The
//! design favors a byte distribution close to `COPY ... TO STDOUT`'s
//! tab-separated form (the closest readily-comparable PG production
//! wire format) so that ratio numbers we report track real compression
//! of customer traffic within ~20%.
//!
//! ```text
//! Buffer = (Column)*       — flat stream; rows are not delimited
//!
//! Column:
//!   u8 type_tag                                  — see TypeTag below
//!   if type_tag != 0x00:
//!     varint length        (LEB128, 1-5 bytes)   — payload byte count
//!     length bytes payload
//!   else:                                         — NULL
//!     (just the tag byte; no length, no payload)
//! ```
//!
//! Per-column overhead for typical short values (decimal integers,
//! cat-NN tags, ISO timestamps): 2 bytes (1 type tag + 1-byte varint).
//! Per-column overhead for medium text bodies (up to ~16 KB): 3 bytes
//! (1 tag + 2-byte varint). This is comparable to COPY TEXT's 1 tab
//! per column, plus or minus a byte; the resulting compressed ratios
//! track COPY TEXT within ~20% on representative tables.
//!
//! No row marker, no column-count byte, no row terminator: the buffer
//! is opaque to the dbwarp estimator (which only consumes the ratio
//! number, not the bytes), so framing meant for hex-dump debugging
//! adds noise without buying anything in production.
//!
//! ### Type tags
//!
//! | Tag  | Name | Used for |
//! |------|------|----------|
//! | 0x00 | Null            | SQL NULL (no payload follows) |
//! | 0x01 | TextUtf8        | UTF-8 text (PG text/varchar, MySQL utf8mb*, MSSQL varchar where collation maps to UTF-8) |
//! | 0x02 | TextUtf16Le     | UTF-16LE bytes (MSSQL nvarchar/nchar/ntext — preserves the byte-doubling that drives the higher zstd ratio for nvarchar in production) |
//! | 0x03 | TextOther       | Bytes in some other charset (MySQL latin1, MSSQL non-Unicode collations) — opaque to the encoder |
//! | 0x04 | NumberText      | Decimal-textual representation (int, bigint, numeric, real, double) |
//! | 0x05 | BoolText        | Boolean as text ("t" / "f" / "true" / "false") |
//! | 0x06 | TimestampText   | ISO-8601 timestamp text |
//! | 0x07 | DateText        | ISO-8601 date text |
//! | 0x08 | TimeText        | HH:MM:SS[.fff] text |
//! | 0x09 | UuidText        | Canonical 36-char UUID text |
//! | 0x0F | JsonText        | JSON UTF-8 text (separate from TextUtf8 so estimator can analyze JSON columns differently — they tend to compress better than free-form text) |
//! | 0x10 | BinaryRaw       | bytea / varbinary / image / blob — raw bytes |
//! | 0xFE | UnknownText     | Fallback: DB-provided textual representation; used for any type the engine module didn't classify |
//!
//! 0x0A (the row terminator) is intentionally NOT used as a type tag so
//! a hex dump of the buffer is easy to read.
//!
//! ## Estimator contract
//!
//! The Blueprint file's `[compression]` block carries
//! `sample_encoding = "dbwarp-blueprint-rowframe-v1"`. The dbwarp estimator
//! must validate this string before consuming the ratio — a mismatch
//! means the producer used a different encoding and the ratio is not
//! comparable. Future versions of this module bump the suffix
//! (`-v2`, etc.) on any incompatible change.

pub const SAMPLE_ENCODING_TAG: &str = "dbwarp-blueprint-rowframe-v1";

/// Append an unsigned LEB128 varint to `out`. 1 byte for values < 128,
/// 2 bytes for values < 16384, 3 bytes for values < 2^21, and so on.
fn write_varint(out: &mut Vec<u8>, mut value: u32) {
    while value >= 0x80 {
        out.push((value as u8) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

/// Per-column type classification. The numeric value of each variant is
/// what gets emitted as the type-tag byte in the encoded stream — these
/// values are part of the wire format and must NOT be renumbered. Add
/// new variants only at the end (or in unused gaps) and bump the
/// `SAMPLE_ENCODING_TAG` suffix on any incompatible change.
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

/// One column-cell within a row. `bytes` is `None` for SQL NULL
/// regardless of `tag`; otherwise it carries the payload bytes in the
/// representation the tag promises (UTF-8, UTF-16LE, raw binary, etc.).
#[derive(Debug, Clone)]
pub struct Cell<'a> {
    pub tag: TypeTag,
    pub bytes: Option<&'a [u8]>,
}

/// Bounded, value-free cardinality sampler. Only 64-bit temporary
/// fingerprints are retained and they are discarded after the aggregate Blueprint
/// is produced; no customer value or fingerprint reaches the TOML file.
#[derive(Debug, Clone, Default)]
pub struct CardinalityAccumulator {
    rows: u64,
    non_null_rows: u64,
    fingerprints: Vec<u64>,
}

impl CardinalityAccumulator {
    const MAX_FINGERPRINTS: usize = 8_192;
    const MIN_UNBIASED_ESTIMATE_ROWS: u64 = 128;

    pub fn push(&mut self, cell: &Cell<'_>) {
        self.rows = self.rows.saturating_add(1);
        let Some(bytes) = cell.bytes else {
            return;
        };
        self.non_null_rows = self.non_null_rows.saturating_add(1);
        let fingerprint = fingerprint_cell(cell.tag, bytes);
        if self.fingerprints.len() < Self::MAX_FINGERPRINTS {
            self.fingerprints.push(fingerprint);
            return;
        }
        let slot = mix64(self.non_null_rows) % self.non_null_rows.max(1);
        if slot < Self::MAX_FINGERPRINTS as u64 {
            self.fingerprints[slot as usize] = fingerprint;
        }
    }

    pub fn null_fraction(&self) -> Option<f64> {
        if self.rows == 0 {
            return None;
        }
        Some(quantize_fraction(
            self.rows.saturating_sub(self.non_null_rows) as f64 / self.rows as f64,
        ))
    }

    pub fn finish(
        &self,
        source_rows: u64,
        sample_method: &str,
        sampled_with_bias: bool,
        bias_reason: &str,
    ) -> Option<crate::format::BlueprintCardinality> {
        if self.rows == 0 || self.fingerprints.is_empty() {
            return None;
        }
        let mut values = self.fingerprints.clone();
        values.sort_unstable();
        let mut frequencies = Vec::new();
        let mut current = values[0];
        let mut count = 0_u64;
        for value in values {
            if value != current {
                frequencies.push(count);
                current = value;
                count = 0;
            }
            count = count.saturating_add(1);
        }
        frequencies.push(count);
        frequencies.sort_unstable();

        let observed = frequencies.len() as u64;
        let singletons = frequencies.iter().filter(|count| **count == 1).count() as u64;
        let doubletons = frequencies.iter().filter(|count| **count == 2).count() as u64;
        let retained_non_null = frequencies.iter().sum::<u64>();
        let source_non_null = ((source_rows as u128)
            .saturating_mul(self.non_null_rows as u128)
            .saturating_add((self.rows / 2) as u128)
            / self.rows as u128)
            .min(u64::MAX as u128) as u64;
        // Extrapolating every singleton across the complete table makes a
        // tiny sample of a repeating domain look unique. In particular, 32
        // distinct dates from a 365-day cycle previously became roughly the
        // complete table row count. Biased first-N samples do not support a
        // population estimate at all, and very small random samples do not
        // contain enough collision evidence. Preserve their observed count as
        // an explicit lower bound. For larger unbiased samples use Chao1,
        // which estimates unseen species from singleton/doubleton evidence
        // instead of scaling singletons linearly to the source row count.
        let (estimated, estimate_method) = if source_rows > 0 && source_rows <= self.rows {
            (observed, "complete bounded sample")
        } else if sampled_with_bias {
            (observed, "cardinality observed lower bound (biased sample)")
        } else if retained_non_null < Self::MIN_UNBIASED_ESTIMATE_ROWS {
            (observed, "cardinality observed lower bound (small sample)")
        } else {
            let unseen = if doubletons > 0 {
                let numerator = (singletons as u128).saturating_mul(singletons as u128);
                (numerator / (2_u128.saturating_mul(doubletons as u128))).min(u64::MAX as u128)
                    as u64
            } else {
                singletons.saturating_mul(singletons.saturating_sub(1)) / 2
            };
            (
                observed
                    .saturating_add(unseen)
                    .clamp(observed, source_non_null.max(observed)),
                "Chao1 lower-bound cardinality estimate",
            )
        };
        let top = frequencies.last().copied().unwrap_or(0);
        let sample_rows = quantize_count(self.rows);
        let non_null_rows = quantize_count(self.non_null_rows).min(sample_rows);
        let observed_distinct_count = quantize_count(observed).min(non_null_rows);
        let emitted_source_domain =
            crate::format::round_rows(source_rows).max(observed_distinct_count);
        let estimated_distinct_count = quantize_count(estimated)
            .max(observed_distinct_count)
            .min(emitted_source_domain);
        Some(crate::format::BlueprintCardinality {
            measured: true,
            sample_rows,
            non_null_rows,
            observed_distinct_count,
            estimated_distinct_count,
            top_value_fraction: quantize_fraction(top as f64 / retained_non_null.max(1) as f64),
            frequency_p50: quantile(&frequencies, 0.50),
            frequency_p95: quantile(&frequencies, 0.95),
            frequency_p99: quantile(&frequencies, 0.99),
            frequency_max: quantize_count(top),
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

impl<'a> Cell<'a> {
    pub fn null() -> Self {
        Self {
            tag: TypeTag::Null,
            bytes: None,
        }
    }
    pub fn new(tag: TypeTag, bytes: &'a [u8]) -> Self {
        Self {
            tag,
            bytes: Some(bytes),
        }
    }
}

/// Append one row's columns to `out`. v1 has no per-row delimiter —
/// rows are simply concatenated in the column stream. The caller is
/// expected to call this once per row in iteration order; the resulting
/// buffer is opaque to anyone but zstd.
pub fn encode_row(out: &mut Vec<u8>, cells: &[Cell<'_>]) -> anyhow::Result<()> {
    for cell in cells {
        out.push(cell.tag as u8);
        if matches!(cell.tag, TypeTag::Null) {
            // NULL: just the tag byte, no length, no payload. The
            // serializer ignores cell.bytes for NULL even if Some.
            continue;
        }
        // Defensive: a non-NULL tag with bytes=None is a bug at the
        // call site. Treat as zero-length rather than panicking.
        let payload: &[u8] = cell.bytes.unwrap_or(&[]);
        let len = payload.len();
        if len > u32::MAX as usize {
            anyhow::bail!(
                "single column value exceeds u32 length cap ({} bytes); v1 limit is {}",
                len,
                u32::MAX
            );
        }
        write_varint(out, len as u32);
        out.extend_from_slice(payload);
    }
    Ok(())
}

/// Convenience: encode an entire batch of rows into a fresh buffer.
#[cfg(test)]
pub fn encode_rows<'a, I>(rows: I) -> anyhow::Result<Vec<u8>>
where
    I: IntoIterator<Item = Vec<Cell<'a>>>,
{
    let mut out: Vec<u8> = Vec::with_capacity(64 * 1024);
    for row in rows {
        encode_row(&mut out, &row)?;
    }
    Ok(out)
}

fn fingerprint_cell(tag: TypeTag, bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64 ^ tag as u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    mix64(hash ^ bytes.len() as u64)
}

fn mix64(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
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

fn quantile(sorted: &[u64], percentile: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = ((sorted.len() as f64 * percentile).ceil() as usize)
        .saturating_sub(1)
        .min(sorted.len() - 1);
    quantize_count(sorted[rank])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_roundtrip_short() {
        // Verify the varint encoding for boundary cases.
        let mut out = Vec::new();
        write_varint(&mut out, 0);
        assert_eq!(out, vec![0x00]);
        out.clear();
        write_varint(&mut out, 127);
        assert_eq!(out, vec![0x7F]);
        out.clear();
        write_varint(&mut out, 128);
        assert_eq!(out, vec![0x80, 0x01]);
        out.clear();
        write_varint(&mut out, 16383);
        assert_eq!(out, vec![0xFF, 0x7F]);
        out.clear();
        write_varint(&mut out, 16384);
        assert_eq!(out, vec![0x80, 0x80, 0x01]);
    }

    #[test]
    fn empty_row_is_empty() {
        let mut out = Vec::new();
        encode_row(&mut out, &[]).unwrap();
        assert!(out.is_empty(), "no cells = no bytes");
    }

    #[test]
    fn null_column_is_one_tag_byte() {
        let mut out = Vec::new();
        encode_row(&mut out, &[Cell::null()]).unwrap();
        assert_eq!(out, vec![0x00]);
    }

    #[test]
    fn single_text_utf8_column() {
        let mut out = Vec::new();
        encode_row(&mut out, &[Cell::new(TypeTag::TextUtf8, b"hello")]).unwrap();
        // tag (0x01) + varint(5) (0x05) + "hello"
        assert_eq!(out, vec![0x01, 0x05, b'h', b'e', b'l', b'l', b'o']);
    }

    #[test]
    fn utf16le_preserves_byte_doubling() {
        let utf16: Vec<u8> = "abc".encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        assert_eq!(utf16, vec![b'a', 0, b'b', 0, b'c', 0]);
        let mut out = Vec::new();
        encode_row(&mut out, &[Cell::new(TypeTag::TextUtf16Le, &utf16)]).unwrap();
        // tag (0x02) + varint(6) (0x06) + payload bytes
        assert_eq!(out, vec![0x02, 0x06, b'a', 0, b'b', 0, b'c', 0]);
    }

    #[test]
    fn nul_bytes_in_text_payload_are_preserved() {
        let mut out = Vec::new();
        let payload: &[u8] = b"a\x00b\x00c";
        encode_row(&mut out, &[Cell::new(TypeTag::TextUtf8, payload)]).unwrap();
        // tag + varint(5) + payload (NUL bytes survive because we
        // length-prefix; nothing terminates on 0x00)
        assert_eq!(out, vec![0x01, 0x05, b'a', 0, b'b', 0, b'c']);
    }

    #[test]
    fn mixed_row_with_null_and_binary() {
        let mut out = Vec::new();
        encode_row(
            &mut out,
            &[
                Cell::new(TypeTag::NumberText, b"42"),
                Cell::null(),
                Cell::new(TypeTag::BinaryRaw, &[0xDE, 0xAD, 0xBE, 0xEF]),
            ],
        )
        .unwrap();
        // col 0: tag(0x04) varint(2) "42"
        // col 1: tag(0x00)
        // col 2: tag(0x10) varint(4) DE AD BE EF
        assert_eq!(
            out,
            vec![0x04, 0x02, b'4', b'2', 0x00, 0x10, 0x04, 0xDE, 0xAD, 0xBE, 0xEF,]
        );
    }

    #[test]
    fn long_payload_uses_2byte_varint() {
        // 200 bytes — varint should emit (200 & 0x7F) | 0x80, then (200 >> 7).
        // 200 = 0xC8 = 0b11001000. low 7 bits = 0b1001000 = 0x48; with MSB
        // set, first byte = 0xC8. Second byte = 200 >> 7 = 1.
        let payload: Vec<u8> = (0..200u32).map(|i| i as u8).collect();
        let mut out = Vec::new();
        encode_row(&mut out, &[Cell::new(TypeTag::BinaryRaw, &payload)]).unwrap();
        assert_eq!(&out[..3], &[0x10, 0xC8, 0x01]);
        assert_eq!(&out[3..], payload.as_slice());
    }

    #[test]
    fn deterministic_encoding() {
        let row = vec![
            Cell::new(TypeTag::TextUtf8, b"deterministic"),
            Cell::new(TypeTag::NumberText, b"3.14"),
            Cell::null(),
        ];
        let mut a = Vec::new();
        let mut b = Vec::new();
        encode_row(&mut a, &row).unwrap();
        encode_row(&mut b, &row).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn cardinality_frequency_rounding_preserves_percentile_order() {
        let mut accumulator = CardinalityAccumulator::default();
        let cell = Cell::new(TypeTag::TextUtf8, b"same-value");
        for _ in 0..99 {
            accumulator.push(&cell);
        }

        let cardinality = accumulator
            .finish(99, "unit-test", false, "")
            .expect("non-empty cardinality");
        assert!(cardinality.frequency_p50 <= cardinality.frequency_p95);
        assert!(cardinality.frequency_p95 <= cardinality.frequency_p99);
        assert!(cardinality.frequency_p99 <= cardinality.frequency_max);
        assert!(cardinality.frequency_max <= cardinality.non_null_rows);
    }

    #[test]
    fn null_fraction_is_present_for_all_null_and_non_null_samples() {
        let mut accumulator = CardinalityAccumulator::default();
        accumulator.push(&Cell::null());
        accumulator.push(&Cell::null());
        accumulator.push(&Cell::new(TypeTag::TextUtf8, b"present"));
        accumulator.push(&Cell::new(TypeTag::TextUtf8, b"also-present"));

        assert_eq!(accumulator.null_fraction(), Some(0.5));

        let mut all_null = CardinalityAccumulator::default();
        all_null.push(&Cell::null());
        assert_eq!(all_null.null_fraction(), Some(1.0));
        assert!(all_null.finish(1, "unit-test", false, "").is_none());
    }

    #[test]
    fn small_and_biased_samples_do_not_extrapolate_singletons_to_table_rows() {
        let mut small = CardinalityAccumulator::default();
        for value in 0..32_u64 {
            let bytes = value.to_string();
            small.push(&Cell::new(TypeTag::NumberText, bytes.as_bytes()));
        }
        let cardinality = small
            .finish(1_000_000, "random", false, "")
            .expect("small sample cardinality");
        assert_eq!(cardinality.estimated_distinct_count, 32);
        assert!(cardinality.sample_method.contains("small sample"));

        let stale_catalog = small
            .finish(0, "random", false, "")
            .expect("sample with an unknown catalog row count");
        assert!(!stale_catalog
            .sample_method
            .contains("complete bounded sample"));
        assert_eq!(stale_catalog.estimated_distinct_count, 32);

        let mut biased = CardinalityAccumulator::default();
        for value in 0..1_000_u64 {
            let bytes = value.to_string();
            biased.push(&Cell::new(TypeTag::NumberText, bytes.as_bytes()));
        }
        let cardinality = biased
            .finish(1_000_000, "first-n", true, "natural order")
            .expect("biased sample cardinality");
        assert_eq!(cardinality.estimated_distinct_count, 992);
        assert!(cardinality.sample_method.contains("biased sample"));
    }

    #[test]
    fn chao1_uses_collision_evidence_and_stays_inside_the_emitted_row_domain() {
        let mut accumulator = CardinalityAccumulator::default();
        for value in 0..200_u64 {
            let bytes = (value % 100).to_string();
            accumulator.push(&Cell::new(TypeTag::NumberText, bytes.as_bytes()));
        }
        let cardinality = accumulator
            .finish(149, "random", false, "")
            .expect("collision sample cardinality");
        assert_eq!(cardinality.estimated_distinct_count, 100);
        assert!(cardinality
            .sample_method
            .contains("complete bounded sample"));

        let mut sparse_collisions = CardinalityAccumulator::default();
        for value in 0..200_u64 {
            let bytes = (value % 150).to_string();
            sparse_collisions.push(&Cell::new(TypeTag::NumberText, bytes.as_bytes()));
        }
        let cardinality = sparse_collisions
            .finish(151, "random", false, "")
            .expect("bounded cardinality");
        assert!(cardinality.estimated_distinct_count <= crate::format::round_rows(151));
    }

    /// Diagnostic: show row-frame vs COPY-style tab-separated ratios
    /// on text-heavy Blueprint data (1 small id, 1 long repetitive body,
    /// 1 short category). The bodies are highly compressible (shared
    /// "lorem ipsum dolor sit amet " prefix + repeating "blah ").
    /// COPY-style tab format compresses dramatically because long
    /// text bodies share content across rows; row-frame should also
    /// compress well — the framing overhead is minor and zstd should
    /// see through it.
    #[test]
    fn rowframe_vs_copy_text_on_repetitive_text() {
        let prefix = "lorem ipsum dolor sit amet ";
        // Build the same 1000 rows in both encodings.
        let mut copy_text: Vec<u8> = Vec::new();
        let mut frame_rows: Vec<Vec<Cell<'static>>> = Vec::new();
        for g in 1..=1000u64 {
            let body = format!("{}{}{}", prefix, "blah ".repeat(((g % 30) + 5) as usize), g);
            let category = format!("cat-{}", g % 12);
            let id = g.to_string();
            // COPY TO STDOUT (text format) layout: id\tbody\tcategory\n
            copy_text.extend_from_slice(id.as_bytes());
            copy_text.push(b'\t');
            copy_text.extend_from_slice(body.as_bytes());
            copy_text.push(b'\t');
            copy_text.extend_from_slice(category.as_bytes());
            copy_text.push(b'\n');
            // Row-frame layout (build owned Strings, leak for static).
            let id_static: &'static [u8] = Box::leak(id.into_boxed_str()).as_bytes();
            let body_static: &'static [u8] = Box::leak(body.into_boxed_str()).as_bytes();
            let cat_static: &'static [u8] = Box::leak(category.into_boxed_str()).as_bytes();
            frame_rows.push(vec![
                Cell::new(TypeTag::NumberText, id_static),
                Cell::new(TypeTag::TextUtf8, body_static),
                Cell::new(TypeTag::TextUtf8, cat_static),
            ]);
        }
        let frame_buf = encode_rows(frame_rows).unwrap();

        let copy_comp = zstd::encode_all(copy_text.as_slice(), 3).unwrap();
        let frame_comp = zstd::encode_all(frame_buf.as_slice(), 3).unwrap();
        let copy_ratio = copy_text.len() as f64 / copy_comp.len() as f64;
        let frame_ratio = frame_buf.len() as f64 / frame_comp.len() as f64;

        eprintln!(
            "COPY: {} -> {} bytes, ratio {:.2}",
            copy_text.len(),
            copy_comp.len(),
            copy_ratio
        );
        eprintln!(
            "FRAME: {} -> {} bytes, ratio {:.2}",
            frame_buf.len(),
            frame_comp.len(),
            frame_ratio
        );

        // Both should compress similarly well (within 2× of each other)
        // because they contain the same logical data with similar
        // overhead-to-content ratio. If they diverge by more, there's
        // something in the row-frame layout interfering with zstd's
        // ability to find redundancy.
        let ratio_of_ratios = (copy_ratio / frame_ratio).max(frame_ratio / copy_ratio);
        assert!(
            ratio_of_ratios < 2.0,
            "row-frame and copy-text ratios diverge by {}× (copy={:.2}, frame={:.2}); \
             framing is interfering with zstd's compression",
            ratio_of_ratios,
            copy_ratio,
            frame_ratio
        );
    }

    /// Headline regression test for the row-encoder campaign: prove
    /// the NEW encoding produces a MEANINGFULLY LOWER ratio than the
    /// OLD tab-separated encoding on the same logical row data, where
    /// non-text columns were rendered as empty fields.
    ///
    /// Concretely: 1000 rows of `(short_text, 9 numeric columns)`.
    ///   - OLD: emits text + 9 empty fields per row → row buffer is
    ///     dominated by `\t\t\t\t\t\t\t\t\t\n` blocks → zstd ratio
    ///     dramatically inflated.
    ///   - NEW: each numeric column carries its actual textual decimal
    ///     value → buffer has no separator-only sequences → ratio
    ///     reflects realistic compression of the actual data.
    ///
    /// Old must beat new by at least 2× — a softer bound than "absolute
    /// ratio < N" because the synthetic data's compressibility depends
    /// on prefix sharing between the integer values.
    #[test]
    fn new_encoder_beats_old_tab_separated_encoding() {
        // Shared row data: same integers, same text, regardless of
        // encoding. Use Box::leak for static lifetimes within the test.
        let texts: Vec<&'static [u8]> = (0..1000)
            .map(|i| {
                let s = format!("user-{i:06}");
                Box::leak(s.into_boxed_str()).as_bytes()
            })
            .collect();
        let nums: Vec<Vec<String>> = (0..1000)
            .map(|i| {
                (0..9)
                    .map(|j| {
                        let v = (i as u64).wrapping_mul(0x9E3779B97F4A7C15) ^ (j as u64);
                        v.to_string()
                    })
                    .collect()
            })
            .collect();

        // OLD encoding: text + 9 empty fields per row, tab-separated, LF-terminated.
        let mut old_buf: Vec<u8> = Vec::with_capacity(64 * 1024);
        for i in 0..1000 {
            old_buf.extend_from_slice(texts[i]);
            for _ in 0..9 {
                old_buf.push(b'\t');
                // Old encoder: non-text columns rendered as empty. This
                // is the bug — we deliberately reproduce it here.
            }
            old_buf.push(b'\n');
        }

        // NEW encoding: row-frame, each numeric column carries its value.
        let mut new_rows: Vec<Vec<Cell<'_>>> = Vec::with_capacity(1000);
        for i in 0..1000 {
            let mut row: Vec<Cell<'_>> = Vec::with_capacity(10);
            row.push(Cell::new(TypeTag::TextUtf8, texts[i]));
            for j in 0..9 {
                row.push(Cell::new(TypeTag::NumberText, nums[i][j].as_bytes()));
            }
            new_rows.push(row);
        }
        let new_buf = encode_rows(new_rows).unwrap();

        let old_compressed = zstd::encode_all(old_buf.as_slice(), 3).unwrap();
        let new_compressed = zstd::encode_all(new_buf.as_slice(), 3).unwrap();
        let old_ratio = old_buf.len() as f64 / old_compressed.len() as f64;
        let new_ratio = new_buf.len() as f64 / new_compressed.len() as f64;

        // Diagnostic for failure cases.
        eprintln!(
            "OLD: {} -> {} bytes, ratio {:.2}",
            old_buf.len(),
            old_compressed.len(),
            old_ratio
        );
        eprintln!(
            "NEW: {} -> {} bytes, ratio {:.2}",
            new_buf.len(),
            new_compressed.len(),
            new_ratio
        );

        // Old ratio should be much higher than new — that's the bug.
        // Require at least 2× separation; in practice it's typically
        // 3–4× on this synthetic.
        assert!(
            old_ratio > new_ratio * 2.0,
            "old ratio ({old_ratio:.2}) should be >2× new ratio ({new_ratio:.2}); \
             if this fails, either the encoder regressed or the synthetic \
             data has lost the property that exposed the original bug"
        );
    }
}
