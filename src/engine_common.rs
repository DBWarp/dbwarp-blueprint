//! Small, policy-neutral helpers shared by the database engines.
//!
//! Engine catalog SQL, privilege semantics, sampling order, and driver
//! behavior deliberately remain in their engine modules. This module is only
//! for byte-identical helpers whose behavior must not vary by engine.

use std::time::Instant;

use crate::audit::AuditLog;

/// Hard per-table ceiling for live row-sample payload. Queries project
/// variable-width cells through server-side truncation before the driver sees
/// them, and the local encoder independently refuses to exceed this bound.
pub const MAX_LIVE_TABLE_SAMPLE_BYTES: usize = 16 * 1024 * 1024;

const SAMPLE_CELL_OVERHEAD_BYTES: usize = 32;

/// Resolve a row count and per-cell payload budget that fit inside the hard
/// table ceiling even for unusually wide tables. `bytes_per_unit` is 4 for
/// character functions on UTF-8-capable engines and 1 for byte functions.
pub fn live_sample_budget(
    requested_rows: u64,
    column_count: usize,
    bytes_per_unit: usize,
) -> (u64, usize) {
    let columns = column_count.max(1);
    let bytes_per_unit = bytes_per_unit.max(1);
    let minimum_cell_bytes = SAMPLE_CELL_OVERHEAD_BYTES.saturating_add(bytes_per_unit);
    let max_rows = (MAX_LIVE_TABLE_SAMPLE_BYTES / columns / minimum_cell_bytes).max(1);
    let requested_rows = usize::try_from(requested_rows.max(1)).unwrap_or(usize::MAX);
    let rows = requested_rows.min(max_rows);
    let cells = rows.saturating_mul(columns).max(1);
    let payload_bytes = (MAX_LIVE_TABLE_SAMPLE_BYTES / cells)
        .saturating_sub(SAMPLE_CELL_OVERHEAD_BYTES)
        .max(1);
    (
        u64::try_from(rows).unwrap_or(u64::MAX),
        (payload_bytes / bytes_per_unit).max(1),
    )
}
use crate::format::{BlueprintTable, Totals};

/// Default Tier-2 sample cap shared by every live database engine.
pub(crate) const DEFAULT_SAMPLE_ROWS: u64 = 1000;

/// Decode percent-escaped URI components without interpreting the rest of the
/// URI. Malformed escapes are preserved verbatim so the engine parser can
/// produce its normal connection diagnostic.
pub(crate) fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) = (
                hexadecimal_digit(bytes[index + 1]),
                hexadecimal_digit(bytes[index + 2]),
            ) {
                decoded.push((high << 4) | low);
                index += 3;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

fn hexadecimal_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Convert elapsed wall time to the bounded millisecond value recorded in an
/// audit query entry.
pub(crate) fn elapsed_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u64::MAX as u128) as u64
}

/// Return rounded p50 and p95 milliseconds for the fixed, non-empty RTT probe.
/// With five samples the nearest-rank p95 is the maximum observation.
pub(crate) fn rtt_percentiles_ms(samples_us: &mut [u64]) -> (u64, u64) {
    assert!(!samples_us.is_empty(), "RTT samples must not be empty");
    samples_us.sort_unstable();
    let p50_us = samples_us[samples_us.len() / 2];
    let p95_us = samples_us[samples_us.len() - 1];
    ((p50_us + 500) / 1000, (p95_us + 500) / 1000)
}

/// Record the identical non-fatal compression degradation used by every
/// engine without exposing the driver error or source identity.
pub(crate) fn warn_compression_unavailable(target: &str, audit: &mut AuditLog) {
    let redacted = crate::i18n::format(
        "engine.driver_detail_redacted",
        &[("target", target.to_string())],
    );
    let detail = crate::i18n::format(
        "engine.compression_failed",
        &[("code", "DBP1407W".to_string()), ("error", redacted)],
    );
    eprintln!("dbwarp-blueprint: {detail}");
    audit.record_warning("DBP1407W", detail);
}

/// Add one emitted table to the aggregate totals without ever publishing a
/// wrapped or partially updated value. Source identifiers are deliberately
/// absent from errors because this helper can run before output redaction.
pub(crate) fn accumulate_table_totals(
    totals: &mut Totals,
    table: &BlueprintTable,
) -> anyhow::Result<()> {
    let row_count = totals
        .row_count
        .checked_add(table.rows)
        .ok_or_else(|| anyhow::anyhow!("Blueprint row total exceeds the u64 format limit"))?;
    let table_bytes = totals
        .table_bytes
        .checked_add(table.table_bytes)
        .ok_or_else(|| {
            anyhow::anyhow!("Blueprint table-byte total exceeds the u64 format limit")
        })?;
    let index_bytes = totals
        .index_bytes
        .checked_add(table.index_bytes)
        .ok_or_else(|| {
            anyhow::anyhow!("Blueprint index-byte total exceeds the u64 format limit")
        })?;

    totals.row_count = row_count;
    totals.table_bytes = table_bytes;
    totals.index_bytes = index_bytes;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_decoding_is_deterministic_and_preserves_invalid_escapes() {
        assert_eq!(percent_decode("user%40example.com"), "user@example.com");
        assert_eq!(percent_decode("%E2%82%AC"), "€");
        assert_eq!(percent_decode("bad%2Gescape"), "bad%2Gescape");
        assert_eq!(percent_decode("trailing%"), "trailing%");
    }

    #[test]
    fn rtt_percentiles_use_middle_and_maximum_of_five_samples() {
        let mut samples = [5_600, 400, 1_400, 3_600, 2_400];
        assert_eq!(rtt_percentiles_ms(&mut samples), (2, 6));
    }

    #[test]
    fn live_sample_budget_caps_payload_for_wide_tables() {
        let (rows, chars_per_cell) = live_sample_budget(1_000, 1_600, 4);
        assert!(rows <= 1_000);
        let cells = rows as usize * 1_600;
        let projected = cells
            .saturating_mul(SAMPLE_CELL_OVERHEAD_BYTES)
            .saturating_add(cells.saturating_mul(chars_per_cell).saturating_mul(4));
        assert!(projected <= MAX_LIVE_TABLE_SAMPLE_BYTES);
    }

    #[test]
    fn live_sample_budget_never_returns_zero() {
        assert_eq!(live_sample_budget(0, 0, 4).0, 1);
        assert!(live_sample_budget(u64::MAX, usize::MAX, 4).1 >= 1);
    }

    #[test]
    fn table_totals_are_checked_and_updated_atomically() {
        let mut totals = Totals {
            row_count: 10,
            table_bytes: 20,
            index_bytes: u64::MAX,
            ..Default::default()
        };
        let before = totals.clone();
        let table = BlueprintTable {
            rows: 1,
            table_bytes: 2,
            index_bytes: 1,
            ..Default::default()
        };

        let error = accumulate_table_totals(&mut totals, &table).unwrap_err();
        assert!(error.to_string().contains("index-byte total"));
        assert_eq!(totals.row_count, before.row_count);
        assert_eq!(totals.table_bytes, before.table_bytes);
        assert_eq!(totals.index_bytes, before.index_bytes);
    }

    #[test]
    fn table_totals_accept_representable_values() {
        let mut totals = Totals::default();
        let table = BlueprintTable {
            rows: 3,
            table_bytes: 5,
            index_bytes: 7,
            ..Default::default()
        };

        accumulate_table_totals(&mut totals, &table).unwrap();
        assert_eq!(totals.row_count, 3);
        assert_eq!(totals.table_bytes, 5);
        assert_eq!(totals.index_bytes, 7);
    }
}
