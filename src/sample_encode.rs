//! Transient type-aware encoder for Tier-2 compression measurement.
//!
//! ## Design goals
//!
//! 1. **Representative byte distribution.** The byte stream handed to zstd
//!    retains the sampled values' character distribution and redundancy:
//!    text in its sampled charset representation, numbers and timestamps as
//!    database text, and binary as raw bytes.
//! 2. **Type-tagged but compact.** A small per-column tag (1 byte) plus a
//!    variable-length prefix keeps unlike value families distinguishable without
//!    materially distorting the measured ratio.
//! 3. **Engine-agnostic encoder, engine-specific value mapping.** This
//!    module owns the transient representation and type-tag enum. Each engine
//!    decides how to map its driver-native values onto a `Cell`.
//! 4. **Deterministic.** Two runs on the same row data produce byte-
//!    identical output; the only non-deterministic parts of a Tier-2
//!    sample are the rows TABLESAMPLE picked. Encoding order within a
//!    row is preserved as the column ordinal order from the engine.
//!
//! ## Probe representation
//!
//! Minimal bookkeeping keeps the measurement close to the sampled value
//! distribution. It is deliberately a public measurement representation, not
//! a database-protocol capture or a reusable data stream.
//!
//! ```text
//! Buffer = (Column)*      : flat stream; rows are not delimited
//!
//! Column:
//!   u8 type_tag                                 : see TypeTag below
//!   if type_tag != 0x00:
//!     varint length        (LEB128, 1-5 bytes)  : payload byte count
//!     length bytes payload
//!   else:                                        : NULL
//!     (just the tag byte; no length, no payload)
//! ```
//!
//! Per-column overhead for typical short values (decimal integers,
//! cat-NN tags, ISO timestamps): 2 bytes (1 type tag + 1-byte varint).
//! Per-column overhead for medium text bodies (up to ~16 KB): 3 bytes
//! (1 tag + 2-byte varint).
//!
//! No row marker, no column-count byte, no row terminator: only the resulting
//! ratio is emitted, never the bytes, so additional framing adds noise without
//! improving the measurement.
//!
//! ### Type tags
//!
//! | Tag  | Name | Used for |
//! |------|------|----------|
//! | 0x00 | Null            | SQL NULL (no payload follows) |
//! | 0x01 | TextUtf8        | UTF-8 text (PG text/varchar, MySQL utf8mb*, MSSQL varchar where collation maps to UTF-8) |
//! | 0x02 | TextUtf16Le     | UTF-16LE bytes (MSSQL nvarchar/nchar/ntext, preserving their byte width) |
//! | 0x03 | TextOther       | Bytes in some other charset (MySQL latin1, MSSQL non-Unicode collations): opaque to the encoder |
//! | 0x04 | NumberText      | Decimal-textual representation (int, bigint, numeric, real, double) |
//! | 0x05 | BoolText        | Boolean as text ("t" / "f" / "true" / "false") |
//! | 0x06 | TimestampText   | ISO-8601 timestamp text |
//! | 0x07 | DateText        | ISO-8601 date text |
//! | 0x08 | TimeText        | HH:MM:SS[.fff] text |
//! | 0x09 | UuidText        | Canonical 36-char UUID text |
//! | 0x0F | JsonText        | JSON UTF-8 text (separate from TextUtf8 because JSON tends to compress differently from free-form text) |
//! | 0x10 | BinaryRaw       | bytea / varbinary / image / blob: raw bytes |
//! | 0x11 | VectorBinary    | Dense float32 vector in PostgreSQL pgvector send layout |
//! | 0xFE | UnknownText     | Fallback: DB-provided textual representation; used for any type the engine module didn't classify |
//!
//! 0x0A is not used as a type tag so a hex dump of the buffer is easy to read.
//!
//! ## Encoding tag
//!
//! Blueprint per-column `[compression]` blocks carry
//! `sample_encoding = "blueprint-compression-probe-v2"`. Live-database table
//! blocks use the separate neutral columnar transfer-probe contract. Readers
//! must validate each string before using its ratio; measurements from unlike
//! representations are not interchangeable.

pub const SAMPLE_ENCODING_TAG: &str = dbwarp_blueprint_core::SAMPLE_ENCODING_TAG;

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
/// what gets emitted as the type-tag byte in the transient probe stream. These
/// values must not be renumbered without changing `SAMPLE_ENCODING_TAG`.
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
    VectorBinary = 0x11,
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

    /// Return the null fraction in the same public population domain as the
    /// emitted cardinality counts.
    ///
    /// When a cardinality block exists, independently rounding the fraction
    /// and the non-NULL count can make the two fields contradict each other.
    /// Deriving the fraction from the already-emitted counts discloses no new
    /// fact and preserves sparse presence. Samples without cardinality retain
    /// the existing privacy-rounded fraction.
    pub fn emitted_null_fraction(
        &self,
        cardinality: Option<&crate::format::BlueprintCardinality>,
    ) -> Option<f64> {
        if let Some(cardinality) = cardinality.filter(|value| value.sample_rows > 0) {
            return Some(
                cardinality
                    .sample_rows
                    .saturating_sub(cardinality.non_null_rows) as f64
                    / cardinality.sample_rows as f64,
            );
        }
        self.null_fraction()
    }

    #[cfg(test)]
    pub fn finish(
        &self,
        source_rows: u64,
        sample_method: &str,
        sampled_with_bias: bool,
        bias_reason: &str,
    ) -> Option<crate::format::BlueprintCardinality> {
        self.finish_with_source_rows(
            Some(source_rows),
            false,
            false,
            sample_method,
            sampled_with_bias,
            bias_reason,
        )
    }

    /// Finalize cardinality only when the source population is known.
    ///
    /// Observed frequencies from a bounded sample remain useful for null and
    /// compression evidence, but they cannot define a source cardinality
    /// domain without a table-row population. Omitting the cardinality block
    /// is safer than serializing the observed sample domain as though it were
    /// the complete table, which would understate the column's real cardinality.
    pub fn finish_with_source_rows(
        &self,
        source_rows: Option<u64>,
        complete_row_read: bool,
        complete_value_read: bool,
        sample_method: &str,
        sampled_with_bias: bool,
        bias_reason: &str,
    ) -> Option<crate::format::BlueprintCardinality> {
        // A bounded query returning fewer rows than its LIMIT has enumerated
        // the table as it existed for that statement. That observation is
        // stronger than a cached catalogue estimate: use the rows actually
        // retained instead of dropping exact evidence when the estimate is
        // absent or slightly stale.
        let source_rows = if complete_row_read {
            self.rows
        } else {
            source_rows?
        };
        // A stale estimate can be present but still impossible: a sampler
        // cannot observe more rows than the table contains. Treat that as
        // unknown population evidence rather than capping the synthetic
        // domain at the sample size (including the common stale-zero case).
        if self.rows == 0 || source_rows < self.rows || self.fingerprints.is_empty() {
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
        // distinct dates from a 365-day cycle would extrapolate to roughly the
        // complete table row count. Biased first-N samples do not support a
        // population estimate at all, and very small random samples do not
        // contain enough collision evidence. Preserve their observed count as
        // an explicit lower bound. For larger unbiased samples use Chao1,
        // which estimates unseen species from singleton/doubleton evidence
        // instead of scaling singletons linearly to the source row count.
        let collision_pairs = frequencies.iter().fold(0_u64, |total, frequency| {
            total.saturating_add(frequency.saturating_mul(frequency.saturating_sub(1)) / 2)
        });
        let biased_near_unique_estimate =
            dbwarp_blueprint_core::estimate_biased_near_unique_cardinality(
                retained_non_null,
                observed,
                collision_pairs,
                source_non_null,
            );
        let (estimated, estimate_method) = if complete_row_read && complete_value_read {
            (observed, "complete bounded sample")
        } else if sampled_with_bias && biased_near_unique_estimate.is_some() {
            (
                biased_near_unique_estimate.unwrap_or(observed),
                "conservative birthday-collision estimate from biased near-unique sample",
            )
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
        // A complete row read already makes the exact table population the
        // emitted `rows` value. Keep the cardinality population in that same
        // domain: quantizing 49 sampled rows up to 50 would otherwise let the
        // sample and estimated distinct counts exceed the table itself.
        let complete_source_read = complete_row_read && complete_value_read;
        // Row completeness and value completeness are separate facts.  A
        // server-side cell cap can truncate a value without hiding whether a
        // row was returned.  Once the bounded statement proved it read the
        // whole table, keep the sample population in the table's exact domain
        // even when the value census must remain a lower bound.
        // The exact retained population is already serialized on the table's
        // compression block. Reuse it unless a rounded-down catalogue estimate
        // would make cardinality coverage exceed 100%; in that case the public
        // table domain is the safe upper bound.
        let emitted_source_population = if complete_row_read {
            source_rows
        } else {
            crate::format::round_estimated_rows(source_rows)
        };
        let sample_rows = self.rows.min(emitted_source_population);
        // Round the retained numerator directly. Deriving it from the rounded
        // null fraction would erase sparse presence (for example 2 non-NULL
        // values in 1,000 rows would become zero) and could turn a nullable column into an
        // apparent NOT NULL column. Preserve the exact zero and all-non-NULL
        // endpoints; privacy-quantize every mixed population without allowing
        // an upward bucket to escape through an exact public bound.
        let non_null_rows = quantize_non_null_rows(self.rows, sample_rows, self.non_null_rows);
        let observed_distinct_count = quantize_count_at_most(observed, non_null_rows);
        let estimated_distinct_count = if non_null_rows == 0 {
            0
        } else if estimated == observed {
            // The lower-bound branches carry no population estimate: their
            // public estimate is the observed census itself.  Quantizing the
            // two fields against different bounds can otherwise turn the
            // same retained count into different values when a rounded-down
            // catalogue population is below the returned row set.
            observed_distinct_count
        } else {
            quantize_count_at_most(estimated, emitted_source_population)
                .max(observed_distinct_count)
        };
        let frequency_p50 = quantize_count_at_most(quantile(&frequencies, 0.50), non_null_rows);
        let frequency_p95 = quantize_count_at_most(quantile(&frequencies, 0.95), non_null_rows);
        let frequency_p99 = quantize_count_at_most(quantile(&frequencies, 0.99), non_null_rows);
        let frequency_max = quantize_count_at_most(top, non_null_rows);
        Some(crate::format::BlueprintCardinality {
            measured: true,
            complete_source_read,
            sample_rows,
            non_null_rows,
            observed_distinct_count,
            estimated_distinct_count,
            top_value_fraction: quantize_fraction(top as f64 / retained_non_null.max(1) as f64),
            frequency_p50,
            frequency_p95,
            frequency_p99,
            frequency_max,
            sample_method: format!("{sample_method}; {estimate_method}"),
            sample_layout: Default::default(),
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

/// Append one row's columns to `out`. v1 has no per-row delimiter;
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

fn quantize_count_at_most(value: u64, upper_bound: u64) -> u64 {
    let bounded = value.min(upper_bound);
    let rounded = quantize_count(bounded);
    if rounded <= upper_bound {
        return rounded;
    }
    // Rounding to nearest can cross an already-public upper bound. Use the
    // lower edge of the same privacy bucket rather than exposing the exact
    // bound as an off-grid count.
    let magnitude = 1_u64 << (63 - bounded.leading_zeros());
    let bucket = (magnitude / 16).max(1);
    (bounded / bucket) * bucket
}

fn quantize_fraction(value: f64) -> f64 {
    (value.clamp(0.0, 1.0) * 200.0).round() / 200.0
}

fn quantize_non_null_rows(
    retained_sample_rows: u64,
    emitted_sample_rows: u64,
    retained_non_null_rows: u64,
) -> u64 {
    if retained_sample_rows == 0 || emitted_sample_rows == 0 || retained_non_null_rows == 0 {
        return 0;
    }
    if retained_non_null_rows >= retained_sample_rows {
        return emitted_sample_rows;
    }
    let scaled = ((retained_non_null_rows as u128)
        .saturating_mul(emitted_sample_rows as u128)
        .saturating_add((retained_sample_rows / 2) as u128)
        / retained_sample_rows as u128)
        .min(u64::MAX as u128) as u64;
    let mixed_upper_bound = emitted_sample_rows.saturating_sub(1);
    if mixed_upper_bound == 0 {
        return 1;
    }
    quantize_count_at_most(scaled.clamp(1, mixed_upper_bound), mixed_upper_bound)
}

fn quantile(sorted: &[u64], percentile: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = ((sorted.len() as f64 * percentile).ceil() as usize)
        .saturating_sub(1)
        .min(sorted.len() - 1);
    sorted[rank]
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
        // 200 bytes: varint should emit (200 & 0x7F) | 0x80, then (200 >> 7).
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
    fn small_unbiased_and_low_diversity_biased_samples_remain_lower_bounds() {
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

        assert!(small.finish(0, "random", false, "").is_none());

        assert!(small
            .finish_with_source_rows(None, false, false, "random", false, "")
            .is_none());
        assert!(small
            .finish_with_source_rows(Some(31), false, false, "random", false, "")
            .is_none());

        let complete = small
            .finish_with_source_rows(Some(31), true, true, "complete LIMIT", false, "")
            .expect("complete read overrides a stale lower catalogue estimate");
        assert_eq!(complete.sample_rows, 32);
        assert_eq!(complete.observed_distinct_count, 32);
        assert_eq!(complete.estimated_distinct_count, 32);
        assert!(complete.sample_method.contains("complete bounded sample"));

        let complete_without_catalog = small
            .finish_with_source_rows(None, true, true, "complete LIMIT", false, "")
            .expect("complete read does not require a catalogue row estimate");
        assert_eq!(complete_without_catalog.estimated_distinct_count, 32);

        let prefix_capped_complete_rows = small
            .finish_with_source_rows(
                None,
                true,
                false,
                "complete LIMIT with capped values",
                true,
                "server-side cell cap",
            )
            .expect("complete rows still establish the source population");
        assert!(!prefix_capped_complete_rows
            .sample_method
            .contains("complete bounded sample"));
        assert!(prefix_capped_complete_rows
            .sample_method
            .contains("biased sample"));

        let estimate_equal_to_sample = small
            .finish_with_source_rows(Some(32), false, false, "LIMIT", true, "natural order")
            .expect("equal cached estimate remains a bounded lower bound");
        assert!(!estimate_equal_to_sample
            .sample_method
            .contains("complete bounded sample"));
        assert!(estimate_equal_to_sample
            .sample_method
            .contains("biased sample"));

        let mut biased = CardinalityAccumulator::default();
        for value in 0..1_000_u64 {
            let bytes = (value % 100).to_string();
            biased.push(&Cell::new(TypeTag::NumberText, bytes.as_bytes()));
        }
        let cardinality = biased
            .finish(1_000_000, "first-n", true, "natural order")
            .expect("biased sample cardinality");
        assert_eq!(cardinality.estimated_distinct_count, 100);
        assert!(cardinality.sample_method.contains("biased sample"));

        let mut medium_diversity = CardinalityAccumulator::default();
        for value in 0..1_000_u64 {
            let bytes = (value % 750).to_string();
            medium_diversity.push(&Cell::new(TypeTag::NumberText, bytes.as_bytes()));
        }
        let cardinality = medium_diversity
            .finish(1_000_000, "first-n", true, "natural order")
            .expect("medium-diversity biased cardinality");
        assert_eq!(
            cardinality.estimated_distinct_count,
            cardinality.observed_distinct_count
        );
        assert!(cardinality.observed_distinct_count >= 700);
        assert!(cardinality.sample_method.contains("biased sample"));
    }

    #[test]
    fn biased_nullable_lower_bound_cannot_become_an_extrapolated_estimate() {
        let mut accumulator = CardinalityAccumulator::default();
        for value in 0..128_u64 {
            if value < 25 {
                accumulator.push(&Cell::null());
            } else {
                let bytes = value.to_string();
                accumulator.push(&Cell::new(TypeTag::NumberText, bytes.as_bytes()));
            }
        }

        let cardinality = accumulator
            .finish_with_source_rows(
                Some(128),
                false,
                false,
                "biased bounded sample",
                true,
                "server-side cell cap",
            )
            .expect("biased nullable cardinality");

        assert!(cardinality.sample_method.contains("observed lower bound"));
        assert_eq!(cardinality.sample_rows, 100);
        assert_eq!(cardinality.non_null_rows, 80);
        assert_eq!(cardinality.observed_distinct_count, 80);
        assert_eq!(
            cardinality.estimated_distinct_count,
            cardinality.observed_distinct_count
        );
    }

    #[test]
    fn biased_near_unique_samples_use_continuous_collision_estimation() {
        let capture = |duplicate_last: bool| {
            let mut accumulator = CardinalityAccumulator::default();
            for value in 0..1_000_u64 {
                let sampled = if duplicate_last && value == 999 {
                    0
                } else {
                    value
                };
                let bytes = sampled.to_string();
                accumulator.push(&Cell::new(TypeTag::NumberText, bytes.as_bytes()));
            }
            accumulator
                .finish(1_000_000, "first-n", true, "natural order")
                .expect("biased cardinality")
        };

        let unique = capture(false);
        let one_collision = capture(true);
        assert!(unique.estimated_distinct_count >= 900_000);
        assert!(one_collision.estimated_distinct_count >= 900_000);
        assert!(unique.sample_method.contains("birthday-collision"));
        assert!(one_collision.sample_method.contains("birthday-collision"));
    }

    #[test]
    fn chao1_uses_collision_evidence_and_stays_inside_the_emitted_row_domain() {
        let mut accumulator = CardinalityAccumulator::default();
        for value in 0..200_u64 {
            let bytes = (value % 100).to_string();
            accumulator.push(&Cell::new(TypeTag::NumberText, bytes.as_bytes()));
        }
        let cardinality = accumulator
            .finish_with_source_rows(Some(149), true, true, "random", false, "")
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
            .finish_with_source_rows(Some(151), true, true, "random", false, "")
            .expect("bounded cardinality");
        assert!(cardinality.estimated_distinct_count <= crate::format::round_rows(200));

        let chao1 = sparse_collisions
            .finish_with_source_rows(Some(500), false, false, "random", false, "")
            .expect("partial sample cardinality");
        assert!(chao1.sample_method.contains("Chao1"));
        assert!(chao1.estimated_distinct_count <= crate::format::round_rows(500));
    }

    #[test]
    fn complete_small_table_cardinality_never_exceeds_exact_table_rows() {
        let mut accumulator = CardinalityAccumulator::default();
        for value in 0..49_u64 {
            let bytes = value.to_string();
            accumulator.push(&Cell::new(TypeTag::NumberText, bytes.as_bytes()));
        }

        let cardinality = accumulator
            .finish_with_source_rows(None, true, true, "complete LIMIT", false, "")
            .expect("complete small-table cardinality");

        assert_eq!(cardinality.sample_rows, 49);
        assert_eq!(cardinality.non_null_rows, 49);
        assert_eq!(cardinality.observed_distinct_count, 48);
        assert_eq!(cardinality.estimated_distinct_count, 48);
        assert!(cardinality.complete_source_read);
    }

    #[test]
    fn complete_small_skewed_table_cannot_round_frequency_above_non_null_rows() {
        let mut accumulator = CardinalityAccumulator::default();
        for _ in 0..49_u64 {
            accumulator.push(&Cell::new(TypeTag::TextUtf8, b"active"));
        }

        let cardinality = accumulator
            .finish_with_source_rows(None, true, true, "complete LIMIT", false, "")
            .expect("complete skewed-table cardinality");

        assert_eq!(cardinality.sample_rows, 49);
        assert_eq!(cardinality.non_null_rows, 49);
        assert_eq!(cardinality.frequency_max, 48);
        assert!(cardinality.frequency_p99 <= cardinality.frequency_max);
    }

    #[test]
    fn complete_read_mixed_non_null_count_stays_on_the_privacy_grid() {
        let mut accumulator = CardinalityAccumulator::default();
        for _ in 0..47_u64 {
            accumulator.push(&Cell::new(TypeTag::TextUtf8, b"active"));
        }
        accumulator.push(&Cell::null());
        accumulator.push(&Cell::null());

        let cardinality = accumulator
            .finish_with_source_rows(None, true, true, "complete LIMIT", false, "")
            .expect("complete nullable cardinality");

        assert_eq!(cardinality.sample_rows, 49);
        // Forty-seven is not on the count grid at this magnitude; 48 is the
        // independently specified nearest bucket and remains below the
        // 49-row mixed-population bound.
        assert_eq!(cardinality.non_null_rows, 48);
        assert!(cardinality.frequency_max <= cardinality.non_null_rows);
    }

    #[test]
    fn sparse_presence_never_quantizes_to_absence() {
        let mut accumulator = CardinalityAccumulator::default();
        for row in 0..1_000_u64 {
            if row < 2 {
                let value = row.to_string();
                accumulator.push(&Cell::new(TypeTag::NumberText, value.as_bytes()));
            } else {
                accumulator.push(&Cell::null());
            }
        }

        let cardinality = accumulator
            .finish_with_source_rows(None, true, true, "complete LIMIT", false, "")
            .expect("sparse cardinality");
        assert_eq!(cardinality.sample_rows, 1_000);
        assert_eq!(cardinality.non_null_rows, 2);
        assert_eq!(cardinality.observed_distinct_count, 2);
        assert_eq!(cardinality.estimated_distinct_count, 2);
        assert_eq!(cardinality.frequency_max, 1);
        assert_eq!(cardinality.top_value_fraction, 0.5);
        assert_eq!(
            accumulator.emitted_null_fraction(Some(&cardinality)),
            Some(0.998)
        );
    }

    #[test]
    fn emitted_null_fraction_tracks_rounded_non_null_population() {
        let mut accumulator = CardinalityAccumulator::default();
        for row in 0..100_u64 {
            if row == 0 {
                accumulator.push(&Cell::null());
            } else {
                let value = row.to_string();
                accumulator.push(&Cell::new(TypeTag::NumberText, value.as_bytes()));
            }
        }

        let cardinality = accumulator
            .finish_with_source_rows(None, true, true, "complete LIMIT", false, "")
            .expect("dense cardinality");
        assert_eq!(cardinality.non_null_rows, 96);
        assert_eq!(
            accumulator.emitted_null_fraction(Some(&cardinality)),
            Some(0.04)
        );
    }

    #[test]
    fn mixed_non_null_counts_keep_presence_and_nullable_endpoints() {
        assert_eq!(quantize_non_null_rows(2_000, 2_000, 5), 5);
        assert_eq!(quantize_non_null_rows(10_000, 10_000, 26), 26);
        assert_eq!(quantize_non_null_rows(1_000, 1_000, 999), 992);
        assert_eq!(quantize_non_null_rows(1_000, 1_000, 1_000), 1_000);
        assert_eq!(quantize_non_null_rows(1_000, 1_000, 0), 0);
    }

    #[test]
    fn complete_rows_with_truncated_values_do_not_emit_exact_cardinality_provenance() {
        let mut accumulator = CardinalityAccumulator::default();
        for row in 0..49_u64 {
            let value = row.to_string();
            accumulator.push(&Cell::new(TypeTag::NumberText, value.as_bytes()));
        }

        let cardinality = accumulator
            .finish_with_source_rows(None, true, false, "complete LIMIT", false, "")
            .expect("bounded cardinality");

        assert!(!cardinality.complete_source_read);
        assert_eq!(cardinality.sample_rows, 49);
        assert_eq!(cardinality.non_null_rows, 49);
        assert!(cardinality.observed_distinct_count <= 49);
        assert!(cardinality.estimated_distinct_count <= 49);
        assert!(cardinality.sample_method.contains("lower bound"));
    }

    #[test]
    fn incomplete_read_reuses_the_exact_already_disclosed_retained_population() {
        let mut accumulator = CardinalityAccumulator::default();
        for row in 0..1_000_u64 {
            let value = row.to_string();
            accumulator.push(&Cell::new(TypeTag::NumberText, value.as_bytes()));
        }

        let cardinality = accumulator
            .finish_with_source_rows(
                Some(20_000),
                false,
                true,
                "bounded partial sample",
                false,
                "",
            )
            .expect("partial cardinality");

        assert!(!cardinality.complete_source_read);
        assert_eq!(cardinality.sample_rows, 1_000);
        assert_eq!(cardinality.sample_rows, accumulator.rows);
    }

    #[test]
    fn incomplete_not_null_sample_keeps_its_exact_retained_numerator() {
        let mut accumulator = CardinalityAccumulator::default();
        for row in 0..1_000_u64 {
            let value = row.to_string();
            accumulator.push(&Cell::new(TypeTag::NumberText, value.as_bytes()));
        }

        let cardinality = accumulator
            .finish_with_source_rows(
                Some(20_000),
                false,
                true,
                "bounded partial sample",
                false,
                "",
            )
            .expect("partial cardinality");

        assert_eq!(cardinality.sample_rows, 1_000);
        assert_eq!(cardinality.non_null_rows, 1_000);
    }

    #[test]
    fn stale_low_population_scales_the_public_non_null_numerator() {
        let mut accumulator = CardinalityAccumulator::default();
        for row in 0..1_010_u64 {
            if row % 2 == 0 {
                accumulator.push(&Cell::new(TypeTag::NumberText, row.to_string().as_bytes()));
            } else {
                accumulator.push(&Cell::null());
            }
        }

        let cardinality = accumulator
            .finish_with_source_rows(
                Some(1_030),
                false,
                true,
                "bounded partial sample",
                false,
                "",
            )
            .expect("clamped partial cardinality");

        assert_eq!(cardinality.sample_rows, 1_000);
        // The scaled retained numerator is 500; its privacy-grid value is 496.
        assert_eq!(cardinality.non_null_rows, 496);
    }

    /// Diagnostic: compare the probe with a delimiter-separated control.
    /// on text-heavy Blueprint data (1 small id, 1 long repetitive body,
    /// 1 short category). The bodies are highly compressible (shared
    /// "lorem ipsum dolor sit amet " prefix + repeating "blah ").
    /// COPY-style tab format compresses dramatically because long
    /// text bodies share content across rows; the probe should also
    /// compress well: the framing overhead is minor and zstd should
    /// see through it.
    #[test]
    fn probe_vs_delimited_text_on_repetitive_text() {
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
        // something in the probe layout interfering with zstd's
        // ability to find redundancy.
        let ratio_of_ratios = (copy_ratio / frame_ratio).max(frame_ratio / copy_ratio);
        assert!(
            ratio_of_ratios < 2.0,
            "probe and delimited-text ratios diverge by {}× (control={:.2}, probe={:.2}); \
             framing is interfering with zstd's compression",
            ratio_of_ratios,
            copy_ratio,
            frame_ratio
        );
    }

    /// Rendering non-text columns as empty fields inflates the compression
    /// ratio. Check that the probe, which carries every value, measures at
    /// least 2x lower than that control encoding.
    ///
    /// Concretely: 1000 rows of `(short_text, 9 numeric columns)`.
    ///   - Control: emits text + 9 empty fields per row → row buffer is
    ///     dominated by `\t\t\t\t\t\t\t\t\t\n` blocks → zstd ratio
    ///     dramatically inflated.
    ///   - Probe: each numeric column carries its actual textual decimal
    ///     value → buffer has no separator-only sequences → ratio
    ///     reflects realistic compression of the actual data.
    ///
    /// The control must exceed the probe by at least 2×: a softer bound than "absolute
    /// ratio < N" because the synthetic data's compressibility depends
    /// on prefix sharing between the integer values.
    #[test]
    fn probe_measures_lower_than_empty_field_control() {
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

        // Control encoding: text + 9 empty fields per row, tab-separated, LF-terminated.
        let mut old_buf: Vec<u8> = Vec::with_capacity(64 * 1024);
        for i in 0..1000 {
            old_buf.extend_from_slice(texts[i]);
            for _ in 0..9 {
                old_buf.push(b'\t');
                // Non-text columns are deliberately empty in the control.
            }
            old_buf.push(b'\n');
        }

        // Probe encoding: each numeric column carries its value.
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

        // Require at least 2× separation.
        assert!(
            old_ratio > new_ratio * 2.0,
            "control ratio ({old_ratio:.2}) should be >2× probe ratio ({new_ratio:.2})"
        );
    }
}
