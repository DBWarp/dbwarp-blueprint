//! Collector-side anonymization and rounding for the canonical Blueprint model.
//!
//! Two design rules drive this module:
//!   1. Anonymization is structural and keyed. Ordinals are stable within a
//!      run, and across runs only when the customer supplies the same key.
//!   2. Numeric statistics are rounded to documented precision to constrain
//!      low-bit information channels.
//!
//! Data-transfer objects and serialization come directly from
//! `dbwarp-blueprint-core`; see `FORMAT.md` for the schema contract.

#[cfg(test)]
use std::collections::BTreeMap;
use std::sync::OnceLock;

use anyhow::{bail, Context, Result};
use hmac::{Hmac, Mac};
use sha2::Sha256;

#[cfg(test)]
pub use dbwarp_blueprint_core::MIN_SCHEMA_VERSION;
pub use dbwarp_blueprint_core::{
    ArtifactInventory, BlueprintArtifact, BlueprintCardinality, BlueprintColumn,
    BlueprintCompression, BlueprintExternalPrerequisite, BlueprintFile, BlueprintIndex,
    BlueprintRelationship, BlueprintTable, DatabaseTopology, DatasetScope, FkEdge,
    LanguageFeatureCensus, NetworkProbe, Totals, ARTIFACT_CONTRACT, LANGUAGE_CENSUS_CONTRACT,
    SCHEMA_VERSION,
};

/// Round to nearest millisecond. Used by the RTT probe to emit
/// integer-ms statistics (kills low-bit hidden channel).
pub fn round_ms(elapsed: std::time::Duration) -> u64 {
    let milliseconds = elapsed.as_micros().saturating_add(500) / 1000;
    milliseconds.min(u128::from(u64::MAX)) as u64
}

// ---------------------------------------------------------------------------
// Anonymization
// ---------------------------------------------------------------------------

const ANONYMIZATION_KEY_BYTES: usize = 32;
static ANONYMIZATION_KEY: OnceLock<[u8; ANONYMIZATION_KEY_BYTES]> = OnceLock::new();

/// Generate a process-local key from the operating system CSPRNG.
pub fn generate_anonymization_key() -> Result<[u8; ANONYMIZATION_KEY_BYTES]> {
    let mut key = [0_u8; ANONYMIZATION_KEY_BYTES];
    getrandom::fill(&mut key).context("DBP1607E obtaining operating-system randomness")?;
    Ok(key)
}

/// Install the key before any source identity is mapped. Reinstalling the
/// same key is harmless; changing it inside one process would make a batch
/// internally inconsistent and is rejected.
pub fn install_anonymization_key(key: [u8; ANONYMIZATION_KEY_BYTES]) -> Result<()> {
    if let Some(existing) = ANONYMIZATION_KEY.get() {
        if existing == &key {
            return Ok(());
        }
        bail!("DBP1607E anonymization key was already initialized for this process");
    }
    ANONYMIZATION_KEY
        .set(key)
        .map_err(|_| anyhow::anyhow!("DBP1607E anonymization key initialization raced"))
}

pub fn anonymization_key_is_initialized() -> bool {
    ANONYMIZATION_KEY.get().is_some()
}

fn anonymization_key() -> &'static [u8; ANONYMIZATION_KEY_BYTES] {
    #[cfg(test)]
    {
        ANONYMIZATION_KEY.get_or_init(|| [0x42; ANONYMIZATION_KEY_BYTES])
    }
    #[cfg(not(test))]
    {
        ANONYMIZATION_KEY
            .get()
            .expect("anonymization key must be initialized before source collection")
    }
}

fn keyed_order_hash(domain: &[u8], components: &[&[u8]]) -> [u8; 8] {
    keyed_order_hash_with_key(anonymization_key(), domain, components)
}

fn keyed_order_hash_with_key(
    key: &[u8; ANONYMIZATION_KEY_BYTES],
    domain: &[u8],
    components: &[&[u8]],
) -> [u8; 8] {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC-SHA256 accepts a 32-byte key");
    mac.update(b"dbwarp-blueprint-anonymization-v1");
    mac.update(&(domain.len() as u64).to_be_bytes());
    mac.update(domain);
    for component in components {
        mac.update(&(component.len() as u64).to_be_bytes());
        mac.update(component);
    }
    let digest = mac.finalize().into_bytes();
    let mut output = [0_u8; 8];
    output.copy_from_slice(&digest[..8]);
    output
}

/// Keyed 8-byte ordering token for a schema/table identity.
pub fn table_hash(schema: &str, table: &str) -> [u8; 8] {
    keyed_order_hash(b"table", &[schema.as_bytes(), table.as_bytes()])
}

/// Keyed 8-byte ordering token for a schema identity.
pub fn schema_hash(schema: &str) -> [u8; 8] {
    keyed_order_hash(b"schema", &[schema.as_bytes()])
}

/// Keyed 8-byte ordering token for an index identity.
pub fn index_hash(index: &str) -> [u8; 8] {
    keyed_order_hash(b"index", &[index.as_bytes()])
}

/// Keyed 8-byte ordering token for a non-table artifact identity.
pub fn artifact_hash(identity: &str) -> [u8; 8] {
    keyed_order_hash(b"artifact", &[identity.as_bytes()])
}

/// "table-001"-style identifier. Caller passes a 1-indexed ordinal.
pub fn table_id(ordinal: usize) -> String {
    format!("table-{ordinal:03}")
}

/// "schema-A"-style identifier (A, B, ..., Z, AA, AB, ...).
pub fn schema_id(ordinal: usize) -> String {
    let mut buf = String::new();
    let mut n = ordinal; // 1-indexed
    if n == 0 {
        return "schema-?".to_string();
    }
    while n > 0 {
        let c = ((n - 1) % 26) as u8 + b'A';
        buf.insert(0, c as char);
        n = (n - 1) / 26;
    }
    format!("schema-{buf}")
}

/// "col-N"-style id; N is the column's natural attribute order (1-indexed).
pub fn col_id(ordinal: u32) -> String {
    format!("col-{ordinal}")
}

/// "idx-N"-style id; N is sorted-by-stable-hash ordinal (1-indexed).
pub fn idx_id(ordinal: u32) -> String {
    format!("idx-{ordinal}")
}

// ---------------------------------------------------------------------------
// Anti-steganography rounding
// ---------------------------------------------------------------------------

/// Round row count to the documented precision band:
///   <= 10_000        : nearest 100
///   <= 1_000_000     : nearest 1_000
///   otherwise         : nearest 10_000
pub fn round_rows(n: u64) -> u64 {
    let bucket: u64 = if n <= 10_000 {
        100
    } else if n <= 1_000_000 {
        1_000
    } else {
        10_000
    };
    round_to(n, bucket)
}

/// Round bytes to nearest 1KiB if < 1MiB, else nearest 1MiB if < 1GiB,
/// else nearest 100MiB. Sizes are bucket-quantized so low-bit channels are gone.
pub fn round_bytes(n: u64) -> u64 {
    let bucket: u64 = if n < 1_048_576 {
        1024
    } else if n < 1_073_741_824 {
        1_048_576
    } else {
        100 * 1_048_576
    };
    round_to(n, bucket)
}

/// Produce a `generated_at` value. The customer may pin this to a
/// fixed string for byte-identical reproducibility runs by passing
/// `--generated-at "2026-04-26T00:00:00Z"`; otherwise the current
/// UTC time (seconds resolution) is used.
///
/// Pinning is via the `--generated-at` CLI flag, never an environment
/// variable — the audit-relevant runtime surface stays narrow and
/// explicit, matching the README trust contract "no environment
/// variables read by default."
pub fn generated_at_now(pinned: Option<&str>) -> String {
    if let Some(s) = pinned {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Round Tier-2 in-memory sample buffer size — coarser than round_bytes
/// because the exact byte count is the most exfiltration-friendly field
/// in the file (one tunable u64 per table the customer chose to sample).
///
///   < 1 MiB  : nearest 64 KiB (~16 distinct values per MiB → 4 bits)
///   < 1 GiB  : nearest 1 MiB
///   otherwise : nearest 100 MiB
///
/// An exact buf.len() sample_bytes value carries ~30 bits of u64
/// entropy per table — sufficient to encode arbitrary data across a
/// multi-table report as a hidden channel. Coarsening removes the
/// channel.
pub fn round_sample_bytes(n: u64) -> u64 {
    let bucket: u64 = if n < 1_048_576 {
        64 * 1024
    } else if n < 1_073_741_824 {
        1_048_576
    } else {
        100 * 1_048_576
    };
    round_to(n, bucket)
}

/// Round average-length (variable-length columns) to nearest 10 bytes.
pub fn round_len_avg(n: u64) -> u64 {
    round_to(n, 10)
}

/// Round 95th-percentile length to nearest 100 bytes.
pub fn round_len_p95(n: u64) -> u64 {
    round_to(n, 100)
}

/// Round a sampled length while keeping the maximum quantization error near
/// 3.2%. Short values are preserved exactly because coarse fixed buckets can
/// materially change realistic key distributions (for example 9/12 bytes).
pub fn round_len_relative(n: u64) -> u64 {
    if n <= 32 {
        return n;
    }
    let magnitude = 1_u64 << (63 - n.leading_zeros());
    let bucket = (magnitude / 16).max(1);
    round_to(n, bucket).max(1)
}

/// Round compression ratio to nearest 0.05.
pub fn round_ratio(r: f64) -> f64 {
    if !r.is_finite() {
        return 0.0;
    }
    (r * 20.0).round() / 20.0
}

fn round_to(n: u64, bucket: u64) -> u64 {
    dbwarp_blueprint_core::round_to_bucket(n, bucket)
}

// ---------------------------------------------------------------------------
// Canonical TOML emission
// ---------------------------------------------------------------------------

/// Header comment placed at the top of every Blueprint file.
#[cfg(test)]
pub const FILE_HEADER: &str = dbwarp_blueprint_core::BLUEPRINT_TOML_HEADER;

/// Emit a BlueprintFile as canonical TOML — alphabetical keys (BTreeMap is
/// already sorted), fixed precision for floats, no inserted comments beyond
/// the verbatim header.
pub fn emit_toml(file: &BlueprintFile) -> anyhow::Result<String> {
    dbwarp_blueprint_core::blueprint_to_toml(file)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn round_ms_under_half_ms_rounds_to_zero() {
        assert_eq!(round_ms(Duration::from_micros(0)), 0);
        assert_eq!(round_ms(Duration::from_micros(499)), 0);
    }

    #[test]
    fn round_ms_at_half_ms_rounds_up() {
        assert_eq!(round_ms(Duration::from_micros(500)), 1);
        assert_eq!(round_ms(Duration::from_micros(1_499)), 1);
        assert_eq!(round_ms(Duration::from_micros(1_500)), 2);
    }

    #[test]
    fn round_ms_typical_wan_values() {
        // 38.4 ms → 38 ms (rounds down because < 38.5)
        assert_eq!(round_ms(Duration::from_micros(38_400)), 38);
        // 38.5 ms → 39 ms
        assert_eq!(round_ms(Duration::from_micros(38_500)), 39);
        // 170.0 ms → 170 ms
        assert_eq!(round_ms(Duration::from_millis(170)), 170);
    }

    #[test]
    fn rounding_large_values_never_wraps() {
        assert_eq!(round_rows(u64::MAX), (u64::MAX / 10_000) * 10_000);
        assert_eq!(
            round_bytes(u64::MAX),
            (u64::MAX / (100 * 1_048_576)) * (100 * 1_048_576)
        );
        assert_eq!(round_ms(Duration::MAX), u64::MAX);
    }

    #[test]
    fn network_probe_serde_roundtrip() {
        let probe = NetworkProbe {
            sample_count: 5,
            connect_total_ms: 142,
            query_rtt_ms_p50: 38,
            query_rtt_ms_p95: 44,
        };
        let s = toml::to_string(&probe).unwrap();
        // Field names must match what we document in FORMAT.md.
        assert!(s.contains("sample_count = 5"));
        assert!(s.contains("connect_total_ms = 142"));
        assert!(s.contains("query_rtt_ms_p50 = 38"));
        assert!(s.contains("query_rtt_ms_p95 = 44"));
        let back: NetworkProbe = toml::from_str(&s).unwrap();
        assert_eq!(back.sample_count, 5);
        assert_eq!(back.connect_total_ms, 142);
        assert_eq!(back.query_rtt_ms_p50, 38);
        assert_eq!(back.query_rtt_ms_p95, 44);
    }

    #[test]
    fn blueprint_file_without_network_block_parses_back() {
        let mut file = BlueprintFile {
            artifact_inventory: None,
            schema_version: SCHEMA_VERSION,
            generated_at: "2026-04-28T00:00:00Z".to_string(),
            engine: "postgresql".to_string(),
            engine_version: "18.3".to_string(),
            source_kind: "production".to_string(),
            length_metadata: "not-captured".to_string(),
            declared_length_fidelity: "not-captured".to_string(),
            index_length_fidelity: "not-captured".to_string(),
            observed_length_fidelity: "not-sampled".to_string(),
            totals: Totals::default(),
            network: None,
            database_topology: Some(DatabaseTopology::unknown()),
            dataset_scope: Some(DatasetScope::unknown_database(
                "postgres-planner-estimate",
                "postgres-local-relation-size",
            )),
            tables: BTreeMap::new(),
            fk_edges: BTreeMap::new(),
        };
        let s = toml::to_string(&file).unwrap();
        assert!(s.contains("length_metadata = \"not-captured\""));
        // network field is skipped when None.
        assert!(!s.contains("[network]"));
        assert!(!s.contains("network ="));
        // And it must round-trip without panicking on the absent field.
        let _back: BlueprintFile = toml::from_str(&s).unwrap();
        // Same once a probe is set.
        file.network = Some(NetworkProbe {
            sample_count: 5,
            connect_total_ms: 38,
            query_rtt_ms_p50: 1,
            query_rtt_ms_p95: 2,
        });
        let s = toml::to_string(&file).unwrap();
        assert!(s.contains("[network]"));
    }

    #[test]
    fn schema_id_letters() {
        assert_eq!(schema_id(1), "schema-A");
        assert_eq!(schema_id(26), "schema-Z");
        assert_eq!(schema_id(27), "schema-AA");
        assert_eq!(schema_id(52), "schema-AZ");
    }

    #[test]
    fn table_hash_is_stable() {
        let a = table_hash("public", "events");
        let b = table_hash("public", "events");
        assert_eq!(a, b);
        let c = table_hash("public", "users");
        assert_ne!(a, c);
    }

    #[test]
    fn anonymous_ordering_depends_on_secret_key_and_domain() {
        let components = [b"public".as_slice(), b"users".as_slice()];
        let first = keyed_order_hash_with_key(&[1_u8; 32], b"table", &components);
        let second = keyed_order_hash_with_key(&[2_u8; 32], b"table", &components);
        let schema = keyed_order_hash_with_key(&[1_u8; 32], b"schema", &components);
        assert_ne!(first, second);
        assert_ne!(first, schema);
    }

    #[test]
    fn round_rows_buckets() {
        assert_eq!(round_rows(0), 0);
        assert_eq!(round_rows(149), 100);
        assert_eq!(round_rows(150), 200);
        assert_eq!(round_rows(9_949), 9_900);
        assert_eq!(round_rows(10_000), 10_000);
        assert_eq!(round_rows(12_499), 12_000);
        assert_eq!(round_rows(12_500), 13_000);
        assert_eq!(round_rows(1_500_000), 1_500_000);
        assert_eq!(round_rows(1_504_999), 1_500_000);
        assert_eq!(round_rows(1_505_000), 1_510_000);
    }

    #[test]
    fn round_bytes_buckets() {
        assert_eq!(round_bytes(0), 0);
        assert_eq!(round_bytes(1_023), 1_024);
        // 1_500_000 ≈ 1.43 MiB → nearest 1 MiB bucket = 1 MiB.
        assert_eq!(round_bytes(1_500_000), 1_048_576);
        // 1_700_000 ≈ 1.62 MiB → nearest 1 MiB bucket = 2 MiB.
        assert_eq!(round_bytes(1_700_000), 2_097_152);
        // 2 GB ≈ 1907 MiB → nearest 100 MiB = 1900 MiB.
        assert_eq!(round_bytes(2_000_000_000), 1_900 * 1_048_576);
    }

    #[test]
    fn generated_at_pinning_via_cli_flag() {
        // Pinning is done via the `--generated-at` CLI flag passed
        // through as Some(&str). When None or an empty/whitespace
        // string, fall back to the current UTC time. The function
        // must NOT read any env var — the README trust contract says
        // "no env vars read by default."
        assert_eq!(
            generated_at_now(Some("2026-04-26T00:00:00Z")),
            "2026-04-26T00:00:00Z"
        );
        // Whitespace gets trimmed; if that leaves nothing, fall back to now().
        assert_ne!(generated_at_now(Some("   ")), "");
        // None → live timestamp; just confirm blueprint (10-19 chars ISO-8601-ish).
        let live = generated_at_now(None);
        assert!(live.contains('T') && live.ends_with('Z'), "got: {live}");
    }

    /// Lock the no-env-var contract: the function must not consult
    /// any environment variable. Set the previously-honored env var
    /// to a known sentinel and assert the function still returns a
    /// fresh ISO timestamp (i.e., it ignored the env var).
    #[test]
    fn generated_at_does_not_read_env_var() {
        let key = "DBWARP_BLUEPRINT_GENERATED_AT";
        let prev = std::env::var(key).ok();
        std::env::set_var(key, "2099-12-31T23:59:59Z");
        let got = generated_at_now(None);
        if let Some(v) = prev {
            std::env::set_var(key, v);
        } else {
            std::env::remove_var(key);
        }
        assert_ne!(
            got, "2099-12-31T23:59:59Z",
            "generated_at_now must not read env vars"
        );
    }

    #[test]
    fn round_sample_bytes_buckets() {
        // Coarse buckets so the per-sample low-bit channel is gone.
        assert_eq!(round_sample_bytes(0), 0);
        // Below 1 MiB: nearest 64 KiB (65_536 byte bucket).
        assert_eq!(round_sample_bytes(32_767), 0);
        assert_eq!(round_sample_bytes(32_768), 65_536);
        assert_eq!(round_sample_bytes(100_000), 131_072);
        assert_eq!(round_sample_bytes(900_000), 917_504);
        // 1 MiB exactly → 1 MiB bucket boundary, lands on 1 MiB.
        assert_eq!(round_sample_bytes(1_048_576), 1_048_576);
        // 1.43 MiB → nearest 1 MiB = 1 MiB.
        assert_eq!(round_sample_bytes(1_500_000), 1_048_576);
        // 1.62 MiB → nearest 1 MiB = 2 MiB.
        assert_eq!(round_sample_bytes(1_700_000), 2_097_152);
        // 2 GB ≈ 1907 MiB → nearest 100 MiB = 1900 MiB.
        assert_eq!(round_sample_bytes(2_000_000_000), 1_900 * 1_048_576);
    }

    #[test]
    fn round_len_avg_to_ten() {
        assert_eq!(round_len_avg(0), 0);
        assert_eq!(round_len_avg(4), 0);
        assert_eq!(round_len_avg(5), 10);
        assert_eq!(round_len_avg(127), 130);
        assert_eq!(round_len_avg(128), 130);
        assert_eq!(round_len_avg(129), 130);
        assert_eq!(round_len_avg(135), 140);
    }

    #[test]
    fn relative_length_rounding_preserves_short_values_and_bounds_error() {
        assert_eq!(round_len_relative(0), 0);
        assert_eq!(round_len_relative(9), 9);
        assert_eq!(round_len_relative(12), 12);
        assert_eq!(round_len_relative(33), 34);
        assert_eq!(round_len_relative(191), 192);
        assert_eq!(round_len_relative(3_000), 2_944);
        for value in 33_u64..=100_000 {
            let rounded = round_len_relative(value);
            let relative_error = rounded.abs_diff(value) as f64 / value as f64;
            assert!(
                relative_error <= 0.032,
                "value={value} rounded={rounded} error={relative_error}"
            );
        }
    }

    #[test]
    fn round_ratio_to_005() {
        assert_eq!(round_ratio(3.21), 3.20);
        assert_eq!(round_ratio(3.225), 3.25);
        assert_eq!(round_ratio(0.0), 0.0);
        assert_eq!(round_ratio(f64::NAN), 0.0);
        assert_eq!(round_ratio(f64::INFINITY), 0.0);
    }

    #[test]
    fn blueprint_file_round_trip_through_toml() {
        let mut tables = BTreeMap::new();
        let mut cols = BTreeMap::new();
        cols.insert(
            col_id(1),
            BlueprintColumn {
                ordinal: 1,
                column_type: "bigint".to_string(),
                nullable: false,
                len_avg: 0,
                len_p95: 0,
                style: String::new(),
                compression: None,
                ..BlueprintColumn::default()
            },
        );
        cols.insert(
            col_id(2),
            BlueprintColumn {
                ordinal: 2,
                column_type: "text".to_string(),
                nullable: false,
                native_type: "varchar".to_string(),
                declared_max_chars: 255,
                declared_max_bytes: 1_020,
                charset: "utf8mb4".to_string(),
                collation: "utf8mb4_0900_ai_ci".to_string(),
                len_avg: 130,
                len_p95: 500,
                style: "json".to_string(),
                compression: Some(BlueprintCompression {
                    measured: true,
                    sample_rows: 100,
                    sample_bytes: 65_536,
                    sample_method: "test column projection".to_string(),
                    sampled_with_bias: false,
                    bias_reason: String::new(),
                    ratio_zstd_3: 4.25,
                    ratio_zstd_19: 5.0,
                    ratio_stddev: 0.0,
                    sample_encoding: "dbwarp-blueprint-rowframe-v1".to_string(),
                    ..BlueprintCompression::default()
                }),
                ..BlueprintColumn::default()
            },
        );
        let mut idxs = BTreeMap::new();
        idxs.insert(
            idx_id(1),
            BlueprintIndex {
                index_type: "btree".to_string(),
                primary: true,
                unique: true,
                cols: vec![1],
                include_cols: Vec::new(),
                expression: false,
                filtered: false,
                descending: false,
                ..BlueprintIndex::default()
            },
        );
        idxs.insert(
            idx_id(2),
            BlueprintIndex {
                index_type: "btree".to_string(),
                cols: vec![2],
                prefix_lengths: vec![191],
                ..BlueprintIndex::default()
            },
        );
        tables.insert(
            table_id(1),
            BlueprintTable {
                rows: 12_500_000,
                table_bytes: 4_200_000_000,
                index_bytes: 1_100_000_000,
                schema: schema_id(1),
                has_clustered_index: false,
                stats_freshness: "fresh".to_string(),
                cols,
                idxs,
                compression: None,
                ..BlueprintTable::default()
            },
        );
        let file = BlueprintFile {
            artifact_inventory: None,
            schema_version: SCHEMA_VERSION,
            generated_at: "2026-04-26T00:00:00Z".to_string(),
            engine: "postgresql".to_string(),
            engine_version: "16.2".to_string(),
            source_kind: "production".to_string(),
            length_metadata: "not-captured".to_string(),
            declared_length_fidelity: "not-captured".to_string(),
            index_length_fidelity: "not-captured".to_string(),
            observed_length_fidelity: "not-sampled".to_string(),
            totals: Totals {
                table_count: 1,
                row_count: 12_500_000,
                table_bytes: 4_200_000_000,
                index_bytes: 1_100_000_000,
            },
            network: None,
            database_topology: Some(DatabaseTopology::unknown()),
            dataset_scope: Some(DatasetScope::unknown_database(
                "postgres-planner-estimate",
                "postgres-local-relation-size",
            )),
            tables,
            fk_edges: BTreeMap::new(),
        };
        let toml = emit_toml(&file).unwrap();
        // Header is verbatim.
        assert!(toml.starts_with(FILE_HEADER));
        // Round-trip: parse the body back and the structure should match.
        let body = toml.strip_prefix(FILE_HEADER).unwrap();
        let parsed: BlueprintFile = toml::from_str(body).unwrap();
        assert_eq!(parsed.schema_version, SCHEMA_VERSION);
        assert_eq!(parsed.tables.len(), 1);
        let t = parsed.tables.get(&table_id(1)).unwrap();
        assert!(toml.contains("primary = true"));
        assert!(t.idxs.get(&idx_id(1)).unwrap().primary);
        assert_eq!(t.cols.len(), 2);
        assert_eq!(t.cols.get(&col_id(2)).unwrap().style, "json");
        let text_col = t.cols.get(&col_id(2)).unwrap();
        assert_eq!(text_col.native_type, "varchar");
        assert_eq!(text_col.declared_max_chars, 255);
        assert_eq!(text_col.declared_max_bytes, 1_020);
        assert_eq!(text_col.charset, "utf8mb4");
        assert_eq!(t.idxs.get(&idx_id(2)).unwrap().prefix_lengths, vec![191]);
        let col_compression = t
            .cols
            .get(&col_id(2))
            .unwrap()
            .compression
            .as_ref()
            .unwrap();
        assert_eq!(col_compression.ratio_zstd_3, 4.25);
        assert_eq!(
            col_compression.sample_encoding,
            "dbwarp-blueprint-rowframe-v1"
        );
    }
}
