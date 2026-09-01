//! Canonical serialized Blueprint data model.
//!
//! Serde defaults preserve the documented compatibility window while
//! validation in `io` enforces cross-field invariants. New emitters use only
//! current Blueprint names; legacy serialized names remain input-only aliases.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SCHEMA_VERSION: u32 = 6;
pub const MIN_SCHEMA_VERSION: u32 = 1;
/// Schema v4 used the former contract identifiers. Normalizing those names
/// yields a v5 document, not a v6 document with invented topology evidence.
pub const LEGACY_IDENTIFIER_SCHEMA_VERSION: u32 = 5;
pub const BUNDLE_SCHEMA_VERSION: u32 = 3;
pub const BUNDLE_KIND: &str = "dbwarp-blueprint-bundle";
pub const PREVIOUS_BUNDLE_SCHEMA_VERSION: u32 = 2;
pub const SAMPLE_ENCODING_TAG: &str = "dbwarp-blueprint-rowframe-v1";
pub const LEGACY_BUNDLE_SCHEMA_VERSION: u32 = 1;
pub const LEGACY_BUNDLE_KIND: &str = "dbwarp-shape-bundle";
pub const LEGACY_SAMPLE_ENCODING_TAG: &str = "dbwarp-shape-rowframe-v1";
pub const LEGACY_ARTIFACT_CONTRACT: &str = "dbwarp-shape-artifacts/v1";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlueprintFile {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub engine: String,
    #[serde(default)]
    pub engine_version: String,
    #[serde(default)]
    pub source_kind: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub length_metadata: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub declared_length_fidelity: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub index_length_fidelity: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub observed_length_fidelity: String,
    #[serde(default)]
    pub totals: Totals,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<NetworkProbe>,
    /// Privacy-safe facts about the database deployment visible through the
    /// connected endpoint. Required for schema-v6 database Blueprints and
    /// absent for structured-file Blueprints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database_topology: Option<DatabaseTopology>,
    /// Declares which logical dataset the totals cover. Required for every
    /// schema-v6 Blueprint; absent on older schemas means unknown evidence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dataset_scope: Option<DatasetScope>,
    #[serde(default)]
    pub tables: BTreeMap<String, BlueprintTable>,
    #[serde(default)]
    pub fk_edges: BTreeMap<String, Vec<FkEdge>>,
    /// Privacy-safe inventory of non-table objects and external prerequisites.
    /// The nested contract is independently versioned so consumers can evolve
    /// artifact planning without changing the table-blueprint vocabulary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_inventory: Option<ArtifactInventory>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Totals {
    #[serde(default)]
    pub table_count: u64,
    #[serde(default)]
    pub row_count: u64,
    #[serde(default)]
    pub table_bytes: u64,
    #[serde(default)]
    pub index_bytes: u64,
}

pub const ARTIFACT_CONTRACT: &str = "dbwarp-blueprint-artifacts/v1";
pub const LANGUAGE_CENSUS_CONTRACT: &str = "dbwarp-language-feature-census/v1";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactInventory {
    #[serde(default)]
    pub contract: String,
    #[serde(default)]
    pub detail: String,
    #[serde(default)]
    pub visibility: String,
    #[serde(default)]
    pub inventory_complete: bool,
    #[serde(default)]
    pub dependencies_complete: bool,
    #[serde(default)]
    pub analysis_complete: bool,
    #[serde(default)]
    pub object_count: u64,
    #[serde(default)]
    pub dependency_edge_count: u64,
    #[serde(default)]
    pub external_prerequisite_count: u64,
    #[serde(default)]
    pub counts_by_kind: BTreeMap<String, u64>,
    #[serde(default)]
    pub counts_by_external_class: BTreeMap<String, u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub catalogs_read: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub catalogs_unreadable: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub families_not_inventoried: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub artifacts: BTreeMap<String, BlueprintArtifact>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlueprintArtifact {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub subkind: String,
    #[serde(default)]
    pub tier: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub schema: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub parent: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub unresolved_dependency_count: u64,
    #[serde(default)]
    pub definition_visibility: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub security_mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external: Option<BlueprintExternalPrerequisite>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis: Option<LanguageFeatureCensus>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlueprintExternalPrerequisite {
    #[serde(default)]
    pub class: String,
    #[serde(default)]
    pub deployment_scope: String,
    #[serde(default)]
    pub binary_material: String,
    #[serde(default)]
    pub secret_material: String,
    #[serde(default)]
    pub endpoint_material: String,
    #[serde(default)]
    pub compatibility: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageFeatureCensus {
    #[serde(default)]
    pub contract: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub dialect: String,
    #[serde(default)]
    pub grammar_profile: String,
    #[serde(default)]
    pub analyzer_version: String,
    #[serde(default)]
    pub definition_size_band: String,
    #[serde(default)]
    pub statement_count_band: String,
    #[serde(default)]
    pub token_count_band: String,
    #[serde(default)]
    pub maximum_nesting_band: String,
    #[serde(default)]
    pub cyclomatic_complexity_band: String,
    #[serde(default)]
    pub opaque_region_count_band: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub minimum_source_version: String,
    #[serde(default)]
    pub minimum_version_complete: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sql_mode_flags: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub compatibility_level: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub ansi_nulls: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub quoted_identifier: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub features: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlueprintTable {
    #[serde(default)]
    pub rows: u64,
    #[serde(default)]
    pub table_bytes: u64,
    /// Bytes occupied by the source file/container. `table_bytes` is the
    /// logical transfer-sizing estimate; its exact provenance is recorded by
    /// the structured-file reader and optional decoded sampling.
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub storage_bytes: u64,
    #[serde(default)]
    pub index_bytes: u64,
    #[serde(default)]
    pub schema: String,
    /// Non-ordinary table/storage semantics. An empty value means the table
    /// was captured as an ordinary table or the evidence was not collected.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub kind: String,
    /// PostgreSQL logged/unlogged evidence. `None` means not captured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unlogged: Option<bool>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub partition_strategy: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partition_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub partition_key_cols: Vec<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partition_rows_max: Option<u64>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub temporal_history: String,
    /// Omitted means the table contributes to all aggregate totals. The only
    /// canonical explicit value is `false`, currently required for external
    /// tables that are inventoried but not locally sized.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counted_in_totals: Option<bool>,
    /// Exact structural CHECK count when the catalog family was read. `None`
    /// is unknown; `Some(0)` is a verified absence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check_count: Option<u64>,
    #[serde(default)]
    pub has_clustered_index: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub stats_freshness: String,
    /// Number of independently schedulable source partitions/files.
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub source_partitions: u64,
    /// Parquet row-group count when the source exposes it.
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub row_group_count: u64,
    /// Sanitized source storage codec set, for example `snappy,zstd`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_codec: String,
    #[serde(default)]
    pub cols: BTreeMap<String, BlueprintColumn>,
    #[serde(default)]
    pub idxs: BTreeMap<String, BlueprintIndex>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compression: Option<BlueprintCompression>,
}

impl BlueprintTable {
    pub fn counts_toward_totals(&self) -> bool {
        self.counted_in_totals.unwrap_or(true)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlueprintColumn {
    #[serde(default)]
    pub ordinal: u32,
    #[serde(rename = "type", default)]
    pub column_type: String,
    #[serde(default)]
    pub nullable: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub value_source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_default: Option<bool>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub default_kind: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub type_kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain_has_check: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub masked: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encrypted: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sparse: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_check: Option<bool>,
    /// Observed null fraction in the inclusive range 0.0..=1.0. `None`
    /// means it was not measured; this is distinct from an observed zero.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub null_fraction: Option<f64>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub native_type: String,
    #[serde(default)]
    pub declared_max_chars: u64,
    #[serde(default)]
    pub declared_max_bytes: u64,
    #[serde(default)]
    pub numeric_precision: u64,
    #[serde(default)]
    pub numeric_scale: u64,
    #[serde(default, skip_serializing_if = "is_false")]
    pub numeric_unsigned: bool,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub bit_width: u64,
    #[serde(default)]
    pub datetime_precision: u64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub charset: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub collation: String,
    #[serde(default)]
    pub len_avg: u64,
    #[serde(default)]
    pub len_p95: u64,
    /// Number of decoded values used for `len_avg` and `len_p95`.
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub length_sample_rows: u64,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub length_p95_sample_rows: u64,
    /// Provenance for width statistics. Footer-encoded byte estimates are
    /// explicitly distinguishable from decoded-value observations.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub length_sample_method: String,
    /// Additional structured-file semantics, such as `repeated-leaf` or
    /// `multi-type-union`, that cannot be represented by a scalar SQL type.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_semantics: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub style: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compression: Option<BlueprintCompression>,
    /// Privacy-safe value-distribution summary. Sample values and temporary
    /// fingerprints are discarded by the producer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cardinality: Option<BlueprintCardinality>,
    /// Coarse signed decimal exponents for the smallest/largest sampled
    /// non-null absolute numeric value. `0` is also the canonical zero band.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub magnitude_min: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub magnitude_max: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_negative: Option<bool>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub time_span: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_recent_decade: Option<i32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlueprintIndex {
    #[serde(rename = "type", default)]
    pub index_type: String,
    #[serde(default)]
    pub unique: bool,
    #[serde(default)]
    pub primary: bool,
    #[serde(default)]
    pub cols: Vec<u32>,
    #[serde(default)]
    pub prefix_lengths: Vec<u64>,
    #[serde(default)]
    pub include_cols: Vec<u32>,
    #[serde(default)]
    pub expression: bool,
    #[serde(default)]
    pub filtered: bool,
    #[serde(default)]
    pub descending: bool,
    /// Estimated distinct tuple counts for key prefixes 1..N.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prefix_distinct_counts: Vec<u64>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub cardinality_sample_method: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlueprintCardinality {
    #[serde(default)]
    pub measured: bool,
    #[serde(default)]
    pub sample_rows: u64,
    #[serde(default)]
    pub non_null_rows: u64,
    #[serde(default)]
    pub observed_distinct_count: u64,
    #[serde(default)]
    pub estimated_distinct_count: u64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub top_value_fraction: f64,
    #[serde(default)]
    pub frequency_p50: u64,
    #[serde(default)]
    pub frequency_p95: u64,
    #[serde(default)]
    pub frequency_p99: u64,
    #[serde(default)]
    pub frequency_max: u64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sample_method: String,
    #[serde(default)]
    pub sampled_with_bias: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub bias_reason: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlueprintCompression {
    #[serde(default)]
    pub measured: bool,
    #[serde(default)]
    pub sample_rows: u64,
    #[serde(default)]
    pub sample_bytes: u64,
    #[serde(default)]
    pub sample_method: String,
    #[serde(default)]
    pub sampled_with_bias: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub bias_reason: String,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub ratio_zstd_3: f64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub ratio_zstd_19: f64,
    /// Zero is a measured result meaning that the sampled chunk ratios had no
    /// variance. Keep `default` for reading older Blueprints, but always emit
    /// this field so zero cannot be confused with an unmeasured value.
    #[serde(default)]
    pub ratio_stddev: f64,
    /// Source-container storage compression ratio. This is never a DBWarp
    /// transfer-ratio estimate and must not be used as one by consumers.
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub ratio_storage: f64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sample_encoding: String,
}

fn is_zero_u64(value: &u64) -> bool {
    *value == 0
}

fn is_zero_f64(value: &f64) -> bool {
    *value == 0.0
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FkEdge {
    #[serde(default)]
    pub to: String,
    #[serde(default)]
    pub cols: Vec<u32>,
    #[serde(default)]
    pub to_cols: Vec<u32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub on_update: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub on_delete: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub match_type: String,
    #[serde(default)]
    pub deferrable: bool,
    #[serde(default)]
    pub initially_deferred: bool,
    #[serde(default = "default_true")]
    pub validated: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statistics: Option<BlueprintRelationship>,
}

impl Default for FkEdge {
    fn default() -> Self {
        Self {
            to: String::new(),
            cols: Vec::new(),
            to_cols: Vec::new(),
            on_update: String::new(),
            on_delete: String::new(),
            match_type: String::new(),
            deferrable: false,
            initially_deferred: false,
            validated: true,
            statistics: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlueprintRelationship {
    #[serde(default)]
    pub measured: bool,
    #[serde(default)]
    pub sample_rows: u64,
    #[serde(default)]
    pub non_null_rows: u64,
    #[serde(default)]
    pub distinct_parent_values: u64,
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub parent_coverage_fraction: f64,
    #[serde(default)]
    pub fanout_p50: u64,
    #[serde(default)]
    pub fanout_p95: u64,
    #[serde(default)]
    pub fanout_p99: u64,
    #[serde(default)]
    pub fanout_max: u64,
    #[serde(default)]
    pub orphan_rows: u64,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub sample_method: String,
    #[serde(default)]
    pub sampled_with_bias: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub bias_reason: String,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkProbe {
    #[serde(default)]
    pub sample_count: u32,
    #[serde(default)]
    pub connect_total_ms: u64,
    #[serde(default)]
    pub query_rtt_ms_p50: u64,
    #[serde(default)]
    pub query_rtt_ms_p95: u64,
}

pub const TOPOLOGY_CONTRACT: &str = "dbwarp-blueprint-topology/v1";
pub const DATASET_SCOPE_CONTRACT: &str = "dbwarp-blueprint-dataset-scope/v1";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatabaseTopology {
    #[serde(default)]
    pub contract: String,
    #[serde(default)]
    pub deployment: String,
    #[serde(default)]
    pub local_role: String,
    #[serde(default)]
    pub visibility: String,
    /// Number of members visible through successful evidence sources. Zero
    /// means unknown; it never means that the deployment has no members.
    #[serde(default)]
    pub member_count: u64,
    #[serde(default)]
    pub identifiers_redacted: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub role_counts: BTreeMap<String, u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub catalogs_read: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub catalogs_unreadable: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatasetScope {
    #[serde(default)]
    pub contract: String,
    #[serde(default)]
    pub layout: String,
    #[serde(default)]
    pub table_inventory_completeness: String,
    #[serde(default)]
    pub row_count_completeness: String,
    #[serde(default)]
    pub size_completeness: String,
    #[serde(default)]
    pub row_count_method: String,
    #[serde(default)]
    pub size_method: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<String>,
}

impl DatabaseTopology {
    /// Conservative evidence used only until an engine-specific probe has
    /// classified the connected endpoint.
    pub fn unknown() -> Self {
        Self {
            contract: TOPOLOGY_CONTRACT.to_string(),
            deployment: "unknown".to_string(),
            local_role: "unknown".to_string(),
            visibility: "unknown".to_string(),
            identifiers_redacted: true,
            ..Self::default()
        }
    }
}

impl DatasetScope {
    /// Conservative database scope. Method tokens describe the local catalog
    /// query, while completeness remains unknown until topology is classified.
    pub fn unknown_database(row_count_method: &str, size_method: &str) -> Self {
        Self {
            contract: DATASET_SCOPE_CONTRACT.to_string(),
            layout: "unknown".to_string(),
            table_inventory_completeness: "unknown".to_string(),
            row_count_completeness: "unknown".to_string(),
            size_completeness: "unknown".to_string(),
            row_count_method: row_count_method.to_string(),
            size_method: size_method.to_string(),
            limitations: vec![
                "topology-unobserved".to_string(),
                "topology-visibility-unknown".to_string(),
            ],
        }
    }

    pub fn structured_dataset(row_count_method: &str, size_method: &str) -> Self {
        Self {
            contract: DATASET_SCOPE_CONTRACT.to_string(),
            layout: "structured-dataset".to_string(),
            table_inventory_completeness: "complete".to_string(),
            row_count_completeness: "complete".to_string(),
            size_completeness: "complete".to_string(),
            row_count_method: row_count_method.to_string(),
            size_method: size_method.to_string(),
            limitations: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlueprintBundle {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub bundle_totals: BundleTotals,
    #[serde(default, skip_serializing_if = "is_false")]
    pub partial: bool,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub failed_source_count: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failed_sources: Vec<String>,
    #[serde(default)]
    pub sources: BTreeMap<String, BundleSource>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dataset_groups: BTreeMap<String, BundleDatasetGroup>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleTotals {
    #[serde(default)]
    pub aggregation: String,
    #[serde(default)]
    pub source_count: u64,
    #[serde(default)]
    pub logical_dataset_count: u64,
    #[serde(default)]
    pub table_count: u64,
    #[serde(default)]
    pub row_count: u64,
    #[serde(default)]
    pub table_bytes: u64,
    #[serde(default)]
    pub index_bytes: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleDatasetGroup {
    #[serde(default)]
    pub relationship: String,
    #[serde(default)]
    pub members_complete: bool,
    #[serde(default)]
    pub members: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleSource {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub engine: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub engine_version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source_kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(alias = "shape_path")]
    pub blueprint_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit_path: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default)]
    pub dataset_relationship: String,
    #[serde(default)]
    pub dataset_group: String,
    #[serde(default)]
    pub dataset_scope_completeness: String,
    #[serde(default)]
    pub table_count: u64,
    #[serde(default)]
    pub row_count: u64,
    #[serde(default)]
    pub table_bytes: u64,
    #[serde(default)]
    pub index_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[serde(alias = "shape")]
    pub blueprint: Option<BlueprintFile>,
}

pub fn recompute_bundle_totals(bundle: &mut BlueprintBundle) -> Result<()> {
    for source in bundle.sources.values_mut() {
        if let Some(blueprint) = &source.blueprint {
            source.engine = blueprint.engine.clone();
            source.engine_version = blueprint.engine_version.clone();
            source.source_kind = blueprint.source_kind.clone();
            source.table_count = u64::try_from(
                blueprint
                    .tables
                    .values()
                    .filter(|table| table.counts_toward_totals())
                    .count(),
            )
            .context("embedded Blueprint table count exceeds the supported u64 range")?;
            source.row_count = if blueprint.totals.row_count > 0 {
                blueprint.totals.row_count
            } else {
                checked_blueprint_source_sum(blueprint, "row_count", |table| table.rows)?
            };
            source.table_bytes = if blueprint.totals.table_bytes > 0 {
                blueprint.totals.table_bytes
            } else {
                checked_blueprint_source_sum(blueprint, "table_bytes", |table| table.table_bytes)?
            };
            source.index_bytes = if blueprint.totals.index_bytes > 0 {
                blueprint.totals.index_bytes
            } else {
                checked_blueprint_source_sum(blueprint, "index_bytes", |table| table.index_bytes)?
            };
            source.dataset_scope_completeness =
                blueprint_dataset_scope_completeness(blueprint).to_string();
        }
    }
    bundle.bundle_totals = aggregate_bundle_totals(bundle)?;
    Ok(())
}

pub fn blueprint_dataset_scope_completeness(blueprint: &BlueprintFile) -> &'static str {
    let Some(scope) = blueprint.dataset_scope.as_ref() else {
        return "unknown";
    };
    let values = [
        scope.table_inventory_completeness.as_str(),
        scope.row_count_completeness.as_str(),
        scope.size_completeness.as_str(),
    ];
    if values.iter().all(|value| *value == "complete") {
        "complete"
    } else if values.contains(&"unknown") {
        "unknown"
    } else {
        "incomplete"
    }
}

fn aggregate_bundle_totals(bundle: &BlueprintBundle) -> Result<BundleTotals> {
    let mut totals = BundleTotals {
        aggregation: "complete".to_string(),
        source_count: u64::try_from(bundle.sources.len())
            .context("Blueprint bundle source count exceeds the supported u64 range")?,
        ..Default::default()
    };
    if !bundle.failed_sources.is_empty() {
        totals.limitations.push("failed-sources".to_string());
    }

    let mut suppress = false;
    for group in bundle.dataset_groups.values() {
        let successful: Vec<&BundleSource> = group
            .members
            .iter()
            .filter_map(|member| bundle.sources.get(member))
            .collect();
        match group.relationship.as_str() {
            "independent" => {
                if let Some(source) = successful.first() {
                    add_bundle_source_totals(&mut totals, source)?;
                    totals.logical_dataset_count = totals
                        .logical_dataset_count
                        .checked_add(1)
                        .context("Blueprint bundle logical_dataset_count overflows u64")?;
                    note_incomplete_source_scope(&mut totals, source);
                } else {
                    totals.limitations.push("failed-sources".to_string());
                }
            }
            "replica" => {
                if let Some(representative) = successful.first() {
                    add_bundle_source_totals(&mut totals, representative)?;
                    totals.logical_dataset_count = totals
                        .logical_dataset_count
                        .checked_add(1)
                        .context("Blueprint bundle logical_dataset_count overflows u64")?;
                    note_incomplete_source_scope(&mut totals, representative);
                    if successful.iter().skip(1).any(|candidate| {
                        candidate.table_count != representative.table_count
                            || candidate.row_count != representative.row_count
                            || candidate.table_bytes != representative.table_bytes
                            || candidate.index_bytes != representative.index_bytes
                    }) {
                        totals
                            .limitations
                            .push("replica-group-disagreement".to_string());
                    }
                } else {
                    totals.limitations.push("failed-sources".to_string());
                }
            }
            "shard" => {
                let all_members_succeeded = successful.len() == group.members.len();
                if group.members_complete && all_members_succeeded {
                    for source in successful {
                        add_bundle_source_totals(&mut totals, source)?;
                        note_incomplete_source_scope(&mut totals, source);
                    }
                    totals.logical_dataset_count = totals
                        .logical_dataset_count
                        .checked_add(1)
                        .context("Blueprint bundle logical_dataset_count overflows u64")?;
                } else {
                    totals
                        .limitations
                        .push("shard-group-incomplete".to_string());
                }
            }
            "unknown" => {
                suppress = true;
                totals
                    .limitations
                    .push("unknown-dataset-relationship".to_string());
            }
            other => anyhow::bail!("unsupported Blueprint bundle dataset relationship '{other}'"),
        }
    }

    totals.limitations.sort();
    totals.limitations.dedup();
    if suppress {
        totals.aggregation = "suppressed".to_string();
        totals.logical_dataset_count = 0;
        totals.table_count = 0;
        totals.row_count = 0;
        totals.table_bytes = 0;
        totals.index_bytes = 0;
    } else if !totals.limitations.is_empty() {
        totals.aggregation = "incomplete".to_string();
    }
    Ok(totals)
}

fn add_bundle_source_totals(totals: &mut BundleTotals, source: &BundleSource) -> Result<()> {
    totals.table_count = totals
        .table_count
        .checked_add(source.table_count)
        .context("Blueprint bundle table_count overflows u64")?;
    totals.row_count = totals
        .row_count
        .checked_add(source.row_count)
        .context("Blueprint bundle row_count overflows u64")?;
    totals.table_bytes = totals
        .table_bytes
        .checked_add(source.table_bytes)
        .context("Blueprint bundle table_bytes overflows u64")?;
    totals.index_bytes = totals
        .index_bytes
        .checked_add(source.index_bytes)
        .context("Blueprint bundle index_bytes overflows u64")?;
    Ok(())
}

fn note_incomplete_source_scope(totals: &mut BundleTotals, source: &BundleSource) {
    if source.dataset_scope_completeness != "complete" {
        totals
            .limitations
            .push("source-dataset-scope-incomplete".to_string());
    }
}

fn checked_blueprint_source_sum(
    blueprint: &BlueprintFile,
    field: &str,
    value: impl Fn(&BlueprintTable) -> u64,
) -> Result<u64> {
    blueprint
        .tables
        .values()
        .filter(|table| table.counts_toward_totals())
        .try_fold(0_u64, |total, table| {
            total
                .checked_add(value(table))
                .with_context(|| format!("embedded Blueprint {field} overflows u64"))
        })
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlueprintSelector {
    pub source: Option<String>,
    pub table: Option<String>,
    pub engine: Option<String>,
    pub tag: Option<String>,
}

impl BlueprintSelector {
    pub fn is_empty(&self) -> bool {
        self.source.is_none() && self.table.is_none() && self.engine.is_none() && self.tag.is_none()
    }
}
