//! Canonical serialized Blueprint data model.
//!
//! Serde defaults preserve the documented compatibility window while
//! validation in `io` enforces cross-field invariants. New emitters use only
//! current Blueprint names; earlier serialized names remain input-only aliases.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SCHEMA_VERSION: u32 = 7;
pub const MIN_SCHEMA_VERSION: u32 = 1;
/// Schema-v4 documents carry v4 contract identifiers. Normalising them yields
/// a v5 document, not a v6 document with invented topology evidence.
pub const LEGACY_IDENTIFIER_SCHEMA_VERSION: u32 = 5;
pub const BUNDLE_SCHEMA_VERSION: u32 = 3;
pub const BUNDLE_KIND: &str = "dbwarp-blueprint-bundle";
pub const PREVIOUS_BUNDLE_SCHEMA_VERSION: u32 = 2;
pub const SAMPLE_ENCODING_TAG: &str = "blueprint-compression-probe-v2";
/// Table-level ratios measured over a neutral columnar compression probe. The
/// representation is distinct from per-column SAMPLE_ENCODING_TAG ratios.
/// V1 represents a pledged one-shot zstd operation over concatenated frames.
pub const TRANSFER_SAMPLE_ENCODING_TAG: &str = "blueprint-columnar-transfer-probe-v1";
/// Current table-level measurement: the same neutral frames as v1, compressed
/// through one persistent zstd context with a flush after each frame. This
/// measures streaming history and flush effects. The tag distinguishes these
/// ratios from v1 one-shot ratios.
pub const TRANSFER_SAMPLE_STREAMING_ENCODING_TAG: &str = "blueprint-columnar-transfer-probe-v2";
/// Neutral v2 row-group encoding measured through a persistent zstd context
/// with a flush at each 256 KiB probe compression chunk.
pub const TRANSFER_SAMPLE_STREAMING_CHUNKED_ENCODING_TAG: &str =
    "blueprint-columnar-transfer-probe-v3";
pub const TRANSFER_PROBE_STREAMING_COMPRESSION_CHUNK_BYTES: usize = 256 * 1024;
/// Input-only compatibility tag. New output never emits it.
pub const PREVIOUS_SAMPLE_ENCODING_TAG: &str = "dbwarp-blueprint-rowframe-v1";
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
    /// Completeness of the relational structure visible through the selected
    /// source scope. Required for schema-v7 Blueprints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub structure_scope: Option<StructureScope>,
    /// Coarse capacity and hosting evidence observed through the database
    /// endpoint. This never describes the collector workstation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_environment: Option<SourceEnvironment>,
    /// Aggregate provenance and quality of the table-level statistics facts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statistics_evidence: Option<StatisticsEvidence>,
    /// Optional, explicitly requested bounded database-activity observation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activity_snapshot: Option<ActivitySnapshot>,
    #[serde(default)]
    pub tables: BTreeMap<String, BlueprintTable>,
    #[serde(default)]
    pub fk_edges: BTreeMap<String, Vec<FkEdge>>,
    /// Privacy-safe inventory of non-table objects and external prerequisites.
    /// The nested contract is independently versioned so it can evolve without
    /// changing the table vocabulary.
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

pub const ARTIFACT_CONTRACT: &str = "dbwarp-blueprint-artifacts/v2";
pub const PREVIOUS_ARTIFACT_CONTRACT: &str = "dbwarp-blueprint-artifacts/v1";
pub const LANGUAGE_CENSUS_CONTRACT: &str = "dbwarp-language-feature-census/v1";
pub const ARTIFACT_COMPLEXITY_CONTRACT: &str = "dbwarp-blueprint-artifact-complexity/v1";
pub const ARTIFACT_COMPLEXITY_ASSESSOR_VERSION: u32 = 1;
pub const ARTIFACT_COMPLEXITY_POPULATION_POLICY: &str =
    "exclude-known-engine-generated-and-secondary";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactInventory {
    #[serde(default)]
    pub contract: String,
    #[serde(default)]
    pub detail: String,
    /// Schema-v7 catalog scope. Empty is retained only for schemas 4-6.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub scope: String,
    #[serde(default)]
    pub visibility: String,
    #[serde(default)]
    pub inventory_complete: bool,
    #[serde(default)]
    pub dependencies_complete: bool,
    /// True only when the provider has qualified every applicable requirement
    /// source, the selected assessment population is complete, and every
    /// emitted artifact is complete or not applicable. Absence is conservative
    /// and therefore means false; an empty requirement list is not itself
    /// evidence that an object has no requirements.
    #[serde(default)]
    pub requirements_complete: bool,
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
    pub catalogs_not_applicable: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub families_not_inventoried: Vec<String>,
    /// Schema-v7 aggregate assessment over the anonymous artifact census. It
    /// is absent below graph detail and required for graph/analyzed captures.
    /// The fixed, one-dimensional records deliberately provide no place for
    /// per-object scores or cross-tabulated fingerprints.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub complexity: Option<ArtifactComplexity>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub artifacts: BTreeMap<String, BlueprintArtifact>,
}

/// Versioned estate-level assessment of the non-table artifact inventory.
///
/// This structure reserves the schema-v7 contract. Assessor v1 emits bands,
/// not `overall_score`; the optional score is reserved so a future assessor
/// can add justified precision without changing the outer Blueprint schema.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactComplexity {
    #[serde(default)]
    pub contract: String,
    #[serde(default)]
    pub assessor_version: u32,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub population_policy: String,
    /// Whether every object eligible under `population_policy` is known.
    /// This is intentionally distinct from the broader inventory completeness
    /// claim; omission is conservative and therefore means false.
    #[serde(default)]
    pub assessment_population_complete: bool,
    #[serde(default)]
    pub eligible_object_count: u64,
    #[serde(default)]
    pub fully_assessed_object_count: u64,
    #[serde(default)]
    pub partially_assessed_object_count: u64,
    #[serde(default)]
    pub unassessed_object_count: u64,
    #[serde(default)]
    pub excluded_object_count: u64,
    /// Single analyzer implementation selected for this capture. Graph-only
    /// assessment records `not-applicable` because it reads no definitions.
    #[serde(default)]
    pub analyzer_version: String,
    /// Exact sorted set from eligible per-object census records. V7 producers
    /// emit `executable-body`; graph-only assessment has no spans.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub analysis_spans: Vec<String>,
    /// Exact sorted set from eligible per-object census records. This is kept
    /// separate from grammar profiles so differently mixed estates remain
    /// distinguishable when readers assess comparability.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dialects: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub grammar_profiles: Vec<String>,
    #[serde(default)]
    pub overall_band: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overall_score: Option<u8>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<String>,
    #[serde(default)]
    pub dimensions: ArtifactComplexityDimensions,
}

/// Fixed dimension set for assessor v1. Adding an arbitrary map here would
/// permit accidental cross-tabulation and weaken the closed v7 contract.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactComplexityDimensions {
    #[serde(default)]
    pub volume: ArtifactComplexitySizeDimension,
    #[serde(default)]
    pub control_flow: ArtifactComplexityCountDimension,
    #[serde(default)]
    pub feature_breadth: ArtifactComplexityCountDimension,
    #[serde(default)]
    pub entanglement: ArtifactComplexityCountDimension,
    #[serde(default)]
    pub environment_coupling: ArtifactComplexityCountDimension,
    #[serde(default)]
    pub opacity: ArtifactComplexityCountDimension,
    #[serde(default)]
    pub dialect_coupling: ArtifactComplexityCountDimension,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactComplexitySizeDimension {
    /// Assessor verdict, using the shared overall-band vocabulary.
    #[serde(default)]
    pub band: String,
    /// Coverage of this dimension, not of the estate as a whole.
    #[serde(default)]
    pub coverage: String,
    #[serde(default)]
    pub histogram: ArtifactComplexitySizeHistogram,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactComplexityCountDimension {
    /// Assessor verdict, using the shared overall-band vocabulary.
    #[serde(default)]
    pub band: String,
    /// Coverage of this dimension, not of the estate as a whole.
    #[serde(default)]
    pub coverage: String,
    #[serde(default)]
    pub histogram: ArtifactComplexityCountHistogram,
}

/// Exact definition-size-band counts over the eligible artifact population.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactComplexitySizeHistogram {
    #[serde(default, rename = "0")]
    pub zero: u64,
    #[serde(default, rename = "1-255")]
    pub one_to_two_fifty_five: u64,
    #[serde(default, rename = "256-1k")]
    pub two_fifty_six_to_one_k: u64,
    #[serde(default, rename = "1k-4k")]
    pub one_k_to_four_k: u64,
    #[serde(default, rename = "4k-16k")]
    pub four_k_to_sixteen_k: u64,
    #[serde(default, rename = "16k-64k")]
    pub sixteen_k_to_sixty_four_k: u64,
    #[serde(default, rename = "64k+")]
    pub sixty_four_k_plus: u64,
    #[serde(default)]
    pub not_applicable: u64,
    #[serde(default)]
    pub unknown: u64,
}

/// Exact count-band counts over the eligible artifact population.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactComplexityCountHistogram {
    #[serde(default, rename = "0")]
    pub zero: u64,
    #[serde(default, rename = "1")]
    pub one: u64,
    #[serde(default, rename = "2-4")]
    pub two_to_four: u64,
    #[serde(default, rename = "5-8")]
    pub five_to_eight: u64,
    #[serde(default, rename = "9-16")]
    pub nine_to_sixteen: u64,
    #[serde(default, rename = "17-32")]
    pub seventeen_to_thirty_two: u64,
    #[serde(default, rename = "33+")]
    pub thirty_three_plus: u64,
    #[serde(default)]
    pub not_applicable: u64,
    #[serde(default)]
    pub unknown: u64,
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
    /// Schema-v7 typed replacement for the unqualified v1 dependency list.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relationships: Vec<ArtifactRelationship>,
    /// Closed set of engine feature requirements.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requirements: Vec<ArtifactRequirement>,
    /// Per-object coverage of every requirement-producing catalog and
    /// analyzer applicable to this artifact. Schema-v7 graph and analyzed
    /// records always emit one of `complete`, `partial`, `unavailable`, or
    /// `not_applicable`; older schemas omit it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub requirement_status: String,
    #[serde(default)]
    pub unresolved_dependency_count: u64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub unresolved_relationships: BTreeMap<String, u64>,
    #[serde(default)]
    pub definition_visibility: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub security_mode: String,
    /// Optional closed validity state observed from the source catalog.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub validity: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_by_engine: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temporary: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editioned: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secondary_object: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external: Option<BlueprintExternalPrerequisite>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis: Option<LanguageFeatureCensus>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactRelationship {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub evidence: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactRequirement {
    #[serde(default)]
    pub token: String,
    #[serde(default)]
    pub evidence: String,
    #[serde(default)]
    pub count_band: String,
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
    /// Closed description of the source span supplied to the analyzer.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub analysis_span: String,
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
    /// logical data-size estimate; its exact provenance is recorded by
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
    /// Schema-v7 object identity, independent of storage and partitioning.
    /// The `kind` field remains input-only for schemas 1-6.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub object_kind: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub storage_organization: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub partitioning: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub segment_state: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub parent_table: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub child_tables: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub table_features: Vec<String>,
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
    /// Per-table reasons why an otherwise required relationship or
    /// measurement is absent. These are deliberately object-scoped: a
    /// capture-wide limitation must never waive an invariant for every table.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub table_limitations: Vec<String>,
    /// Omitted means the table contributes to all aggregate totals. The only
    /// canonical explicit value is `false`, required for external tables,
    /// materialized views whose derived contents must not inflate source-data
    /// totals, session-scoped temporary tables, and structures whose numeric
    /// evidence is deliberately withheld.
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
    /// Schema-v7 row-count, optimizer-statistics, and size provenance. The
    /// `stats_freshness` remains input-only for schemas 1-6.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statistics: Option<TableStatisticsEvidence>,
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
    /// Exact Oracle DEFAULT ON NULL semantics. `None` means not observed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_on_null: Option<bool>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub type_kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain_has_check: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hidden: Option<bool>,
    /// Source-catalog invisibility, which is distinct from an engine-created
    /// hidden column. `None` means the catalog fact was not read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invisible: Option<bool>,
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
    /// Declared string-length unit: `characters`, `bytes`, `not-applicable`,
    /// or `unknown`. Required only when the source exposes the distinction.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub length_semantics: String,
    /// Required closed semantic family in schema v7. `decimal-float` describes
    /// exact decimal values with floating scale (including Oracle FLOAT), and
    /// is distinct from IEEE `binary-float`. `not-applicable` is used for
    /// known non-numeric types. Empty is retained only while reading schemas
    /// 1-6.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub numeric_model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub numeric_precision: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub numeric_scale: Option<i64>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub numeric_precision_radix: String,
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
    /// Optional database LOB storage evidence. This is separate from
    /// compression-probe ratios and never contains paths or segment names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lob_storage: Option<LobStorageEvidence>,
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
    /// Schema-v7 physical index semantics.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub partitioning: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub visibility: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub state: String,
    /// Estimated distinct tuple counts for key prefixes 1..N.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prefix_distinct_counts: Vec<u64>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub cardinality_sample_method: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LobStorageEvidence {
    #[serde(default)]
    pub storage_class: String,
    #[serde(default)]
    pub compression: String,
    #[serde(default)]
    pub deduplication: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub in_row: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encrypted: Option<bool>,
    #[serde(default)]
    pub visibility: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BlueprintSampleLayout {
    #[default]
    Unknown,
    PrimaryKeyRangeWindows,
}

impl BlueprintSampleLayout {
    fn is_unknown(&self) -> bool {
        *self == Self::Unknown
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlueprintCardinality {
    #[serde(default)]
    pub measured: bool,
    /// True only when one bounded statement observed the complete visible
    /// source-row population and retained this column's values without a
    /// value-level truncation. Readers must not infer this from prose in
    /// `sample_method`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub complete_source_read: bool,
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
    /// Machine-readable ordering/layout contract for the bounded sample.
    /// Human provenance remains in `sample_method`; readers must use this enum,
    /// not the prose.
    #[serde(default, skip_serializing_if = "BlueprintSampleLayout::is_unknown")]
    pub sample_layout: BlueprintSampleLayout,
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
    /// Source-container storage compression ratio, distinct from a
    /// compression-probe ratio.
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

pub const TOPOLOGY_CONTRACT: &str = "dbwarp-blueprint-topology/v2";
pub const PREVIOUS_TOPOLOGY_CONTRACT: &str = "dbwarp-blueprint-topology/v1";
pub const DATASET_SCOPE_CONTRACT: &str = "dbwarp-blueprint-dataset-scope/v1";
pub const STRUCTURE_SCOPE_CONTRACT: &str = "dbwarp-blueprint-structure-scope/v1";
pub const SOURCE_ENVIRONMENT_CONTRACT: &str = "dbwarp-blueprint-source-environment/v1";
pub const STATISTICS_EVIDENCE_CONTRACT: &str = "dbwarp-blueprint-statistics-evidence/v1";
pub const ACTIVITY_SNAPSHOT_CONTRACT: &str = "dbwarp-blueprint-activity-snapshot/v1";

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
    pub member_count_scope: String,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub catalogs_not_applicable: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceEnvironment {
    #[serde(default)]
    pub contract: String,
    #[serde(default)]
    pub evidence_origin: String,
    #[serde(default)]
    pub hosting_model: String,
    #[serde(default)]
    pub infrastructure_location: String,
    #[serde(default)]
    pub capacity_scope: String,
    #[serde(default)]
    pub capacity_visibility: String,
    #[serde(default)]
    pub cpu_capacity_band: String,
    #[serde(default)]
    pub cpu_capacity_basis: String,
    #[serde(default)]
    pub memory_capacity_band: String,
    #[serde(default)]
    pub memory_capacity_basis: String,
    #[serde(default)]
    pub collector_machine_excluded: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub member_capacity_uniform: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub catalogs_read: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub catalogs_unreadable: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub catalogs_not_applicable: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatisticsEvidence {
    #[serde(default)]
    pub contract: String,
    #[serde(default)]
    pub visibility: String,
    #[serde(default)]
    pub table_count: u64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub counts_by_statistics_state: BTreeMap<String, u64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub counts_by_row_count_quality: BTreeMap<String, u64>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub counts_by_size_quality: BTreeMap<String, u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub catalogs_read: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub catalogs_unreadable: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub catalogs_not_applicable: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TableStatisticsEvidence {
    #[serde(default)]
    pub row_count_method: String,
    #[serde(default)]
    pub row_count_quality: String,
    #[serde(default)]
    pub statistics_state: String,
    #[serde(default)]
    pub refresh_age_band: String,
    #[serde(default)]
    pub modification_ratio_band: String,
    #[serde(default)]
    pub sample_fraction_band: String,
    #[serde(default)]
    pub statistics_scope: String,
    #[serde(default)]
    pub size_method: String,
    #[serde(default)]
    pub size_quality: String,
    #[serde(default)]
    pub size_scope: String,
    #[serde(default)]
    pub size_accounting: String,
    #[serde(default)]
    pub size_visibility: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivitySnapshot {
    #[serde(default)]
    pub contract: String,
    #[serde(default)]
    pub evidence_origin: String,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub observation_window_band: String,
    #[serde(default)]
    pub active_connections_band: String,
    #[serde(default)]
    pub transaction_rate_band: String,
    #[serde(default)]
    pub write_rate_band: String,
    #[serde(default)]
    pub log_generation_rate_band: String,
    #[serde(default)]
    pub cpu_pressure_band: String,
    #[serde(default)]
    pub cache_pressure_band: String,
    #[serde(default)]
    pub reset_semantics: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub catalogs_read: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub catalogs_unreadable: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub catalogs_not_applicable: Vec<String>,
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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StructureScope {
    #[serde(default)]
    pub contract: String,
    #[serde(default)]
    pub visibility: String,
    #[serde(default)]
    pub table_inventory_completeness: String,
    #[serde(default)]
    pub column_inventory_completeness: String,
    #[serde(default)]
    pub index_inventory_completeness: String,
    #[serde(default)]
    pub relationship_inventory_completeness: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub catalogs_read: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub catalogs_unreadable: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub catalogs_not_applicable: Vec<String>,
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
            member_count_scope: "unknown".to_string(),
            identifiers_redacted: true,
            ..Self::default()
        }
    }
}

impl StructureScope {
    pub fn unknown_database() -> Self {
        Self {
            contract: STRUCTURE_SCOPE_CONTRACT.to_string(),
            visibility: "unknown".to_string(),
            table_inventory_completeness: "unknown".to_string(),
            column_inventory_completeness: "unknown".to_string(),
            index_inventory_completeness: "unknown".to_string(),
            relationship_inventory_completeness: "unknown".to_string(),
            limitations: vec!["metadata-visibility-unknown".to_string()],
            ..Self::default()
        }
    }

    pub fn structured_dataset(catalog: &str) -> Self {
        Self {
            contract: STRUCTURE_SCOPE_CONTRACT.to_string(),
            visibility: "full".to_string(),
            table_inventory_completeness: "complete".to_string(),
            column_inventory_completeness: "complete".to_string(),
            index_inventory_completeness: "complete".to_string(),
            relationship_inventory_completeness: "complete".to_string(),
            catalogs_read: vec![catalog.to_string()],
            ..Self::default()
        }
    }
}

impl SourceEnvironment {
    pub fn unknown_database() -> Self {
        Self {
            contract: SOURCE_ENVIRONMENT_CONTRACT.to_string(),
            evidence_origin: "none".to_string(),
            hosting_model: "unknown".to_string(),
            infrastructure_location: "unknown".to_string(),
            capacity_scope: "unknown".to_string(),
            capacity_visibility: "not-requested".to_string(),
            cpu_capacity_band: "unknown".to_string(),
            cpu_capacity_basis: "unknown".to_string(),
            memory_capacity_band: "unknown".to_string(),
            memory_capacity_basis: "unknown".to_string(),
            collector_machine_excluded: true,
            ..Self::default()
        }
    }
}

impl ArtifactInventory {
    /// Explicitly records that database object catalogs were not requested.
    /// This is distinct from a successful census that observed zero objects.
    pub fn not_requested_database() -> Self {
        Self {
            contract: ARTIFACT_CONTRACT.to_string(),
            detail: "none".to_string(),
            scope: "unknown".to_string(),
            visibility: "unknown".to_string(),
            families_not_inventoried: vec!["non_table_objects".to_string()],
            ..Self::default()
        }
    }

    /// Structured files have no database-resident non-table object catalogs.
    pub fn not_applicable_structured() -> Self {
        Self {
            contract: ARTIFACT_CONTRACT.to_string(),
            detail: "none".to_string(),
            scope: "structured-source".to_string(),
            visibility: "full".to_string(),
            inventory_complete: true,
            dependencies_complete: true,
            catalogs_not_applicable: vec!["database_object_catalogs".to_string()],
            ..Self::default()
        }
    }
}

impl TableStatisticsEvidence {
    pub fn unknown_database(row_count_method: &str, size_method: &str) -> Self {
        Self {
            row_count_method: row_count_method.to_string(),
            row_count_quality: "unknown".to_string(),
            statistics_state: "unknown".to_string(),
            refresh_age_band: "unknown".to_string(),
            modification_ratio_band: "unknown".to_string(),
            sample_fraction_band: "unknown".to_string(),
            statistics_scope: "unknown".to_string(),
            size_method: size_method.to_string(),
            size_quality: "unknown".to_string(),
            size_scope: "unknown".to_string(),
            size_accounting: "unknown".to_string(),
            size_visibility: "unknown".to_string(),
        }
    }

    pub fn structured_dataset(row_count_method: &str, size_method: &str) -> Self {
        Self {
            row_count_method: row_count_method.to_string(),
            row_count_quality: "exact-counter".to_string(),
            statistics_state: "not-applicable".to_string(),
            refresh_age_band: "not-applicable".to_string(),
            modification_ratio_band: "not-applicable".to_string(),
            sample_fraction_band: "not-applicable".to_string(),
            statistics_scope: "structured-dataset".to_string(),
            size_method: size_method.to_string(),
            size_quality: "exact-counter".to_string(),
            size_scope: "logical-dataset".to_string(),
            size_accounting: "logical-estimate".to_string(),
            size_visibility: "full".to_string(),
        }
    }
}

impl StatisticsEvidence {
    pub fn unknown_database(table_count: u64) -> Self {
        let counts = if table_count > 0 {
            BTreeMap::from([("unknown".to_string(), table_count)])
        } else {
            BTreeMap::new()
        };
        Self {
            contract: STATISTICS_EVIDENCE_CONTRACT.to_string(),
            visibility: "unknown".to_string(),
            table_count,
            counts_by_statistics_state: counts.clone(),
            counts_by_row_count_quality: counts.clone(),
            counts_by_size_quality: counts,
            limitations: vec!["statistics-provenance-unclassified".to_string()],
            ..Self::default()
        }
    }

    pub fn structured_dataset(table_count: u64, catalog: &str) -> Self {
        let state = if table_count > 0 {
            BTreeMap::from([("not-applicable".to_string(), table_count)])
        } else {
            BTreeMap::new()
        };
        let quality = if table_count > 0 {
            BTreeMap::from([("exact-counter".to_string(), table_count)])
        } else {
            BTreeMap::new()
        };
        Self {
            contract: STATISTICS_EVIDENCE_CONTRACT.to_string(),
            visibility: "full".to_string(),
            table_count,
            counts_by_statistics_state: state,
            counts_by_row_count_quality: quality.clone(),
            counts_by_size_quality: quality,
            catalogs_read: vec![catalog.to_string()],
            ..Self::default()
        }
    }
}

impl BlueprintFile {
    /// Populate conservative schema-v7 evidence for an engine that does not
    /// supply it. This records unknown evidence explicitly; it does not infer
    /// source-machine or optimiser facts from the collector host.
    pub fn initialize_v7_database_contract(&mut self) {
        let row_method = self
            .dataset_scope
            .as_ref()
            .map(|scope| scope.row_count_method.as_str())
            .unwrap_or("unknown");
        let size_method = self
            .dataset_scope
            .as_ref()
            .map(|scope| scope.size_method.as_str())
            .unwrap_or("unknown");
        for table in self.tables.values_mut() {
            initialize_v7_table_contract(table, row_method, size_method, false);
        }
        if let Some(topology) = self.database_topology.as_mut() {
            topology.contract = TOPOLOGY_CONTRACT.to_string();
            if topology.member_count_scope.is_empty() {
                topology.member_count_scope = if topology.visibility == "full" {
                    "deployment"
                } else if topology.member_count == 1 {
                    "connected-member"
                } else if topology.member_count > 1 {
                    "visible-subset"
                } else {
                    "unknown"
                }
                .to_string();
            }
        }
        let selection_limited = self.dataset_scope.as_ref().is_some_and(|scope| {
            scope
                .limitations
                .iter()
                .any(|limitation| limitation == "selection-limited")
        });
        let mut structure_scope = StructureScope::complete_database(&self.engine);
        if selection_limited {
            structure_scope
                .limitations
                .push("selection-limited".to_string());
        }
        self.structure_scope = Some(structure_scope);
        if self.source_environment.is_none() {
            self.source_environment = Some(SourceEnvironment::unknown_database());
        }
        self.statistics_evidence = Some(statistics_summary(&self.tables, false));
        self.activity_snapshot = None;
        if self.artifact_inventory.is_none() {
            self.artifact_inventory = Some(ArtifactInventory::not_requested_database());
        }
        if let Some(inventory) = self.artifact_inventory.as_mut() {
            inventory.scope = if selection_limited {
                "selected-schemas"
            } else {
                "all-visible-schemas"
            }
            .to_string();
            // The complexity block inherits the inventory's scope by
            // construction; the serializer rejects any divergence.
            if let Some(complexity) = inventory.complexity.as_mut() {
                complexity.scope = inventory.scope.clone();
            }
        }
    }

    pub fn initialize_v7_structured_contract(&mut self, catalog: &str) {
        let row_method = self
            .dataset_scope
            .as_ref()
            .map(|scope| scope.row_count_method.as_str())
            .unwrap_or("unknown");
        let size_method = self
            .dataset_scope
            .as_ref()
            .map(|scope| scope.size_method.as_str())
            .unwrap_or("unknown");
        for table in self.tables.values_mut() {
            initialize_v7_table_contract(table, row_method, size_method, true);
        }
        self.structure_scope = Some(StructureScope::structured_dataset(catalog));
        self.source_environment = None;
        self.statistics_evidence = Some(statistics_summary(&self.tables, true));
        self.activity_snapshot = None;
        self.artifact_inventory = Some(ArtifactInventory::not_applicable_structured());
    }
}

impl StructureScope {
    pub fn complete_database(engine: &str) -> Self {
        let catalogs = match engine {
            "postgresql" => vec![
                "postgresql-columns",
                "postgresql-foreign-keys",
                "postgresql-indexes",
                "postgresql-tables",
            ],
            "mysql" => vec![
                "mysql-information-schema-columns",
                "mysql-information-schema-foreign-keys",
                "mysql-information-schema-indexes",
                "mysql-information-schema-tables",
            ],
            "sqlserver" => vec![
                "sqlserver-columns",
                "sqlserver-foreign-keys",
                "sqlserver-indexes",
                "sqlserver-tables",
            ],
            "oracle" => vec![
                "oracle-constraints",
                "oracle-indexes",
                "oracle-tab-columns",
                "oracle-tables",
            ],
            _ => Vec::new(),
        };
        Self {
            contract: STRUCTURE_SCOPE_CONTRACT.to_string(),
            visibility: if engine == "postgresql" {
                "full"
            } else {
                "privilege-filtered"
            }
            .to_string(),
            table_inventory_completeness: "complete".to_string(),
            column_inventory_completeness: "complete".to_string(),
            index_inventory_completeness: "complete".to_string(),
            relationship_inventory_completeness: "complete".to_string(),
            catalogs_read: catalogs.into_iter().map(str::to_string).collect(),
            limitations: if engine != "postgresql" {
                vec!["metadata-visibility-privilege-filtered".to_string()]
            } else {
                Vec::new()
            },
            ..Self::default()
        }
    }
}

fn initialize_v7_table_contract(
    table: &mut BlueprintTable,
    row_method: &str,
    size_method: &str,
    structured: bool,
) {
    if table.numeric_contract_needs_initialization() {
        for column in table.cols.values_mut() {
            if column.numeric_model.is_empty() {
                column.numeric_model = default_numeric_model(&column.column_type).to_string();
                column.numeric_precision = None;
                column.numeric_scale = None;
                column.numeric_precision_radix.clear();
            }
        }
    }
    if table.object_kind.is_empty() {
        let legacy_kind = std::mem::take(&mut table.kind);
        table.object_kind = match legacy_kind.as_str() {
            "materialized-view" => "materialized-view",
            "external" => "external-table",
            _ => "ordinary-table",
        }
        .to_string();
        table.storage_organization = if legacy_kind == "external" {
            "external"
        } else {
            "unknown"
        }
        .to_string();
        table.partitioning = if legacy_kind == "partitioned" {
            let strategy = std::mem::take(&mut table.partition_strategy);
            if strategy.is_empty() {
                "unknown".to_string()
            } else {
                strategy
            }
        } else {
            "none".to_string()
        };
        table.segment_state = if structured { "unavailable" } else { "unknown" }.to_string();
        if matches!(
            legacy_kind.as_str(),
            "temporal-current"
                | "temporal-history"
                | "memory-optimized"
                | "graph-node"
                | "graph-edge"
        ) {
            table.table_features.push(legacy_kind);
        }
    }
    let legacy_freshness = std::mem::take(&mut table.stats_freshness);
    table.statistics = Some(if structured {
        TableStatisticsEvidence::structured_dataset(row_method, size_method)
    } else {
        let mut evidence = TableStatisticsEvidence::unknown_database(row_method, size_method);
        evidence.statistics_state = match legacy_freshness.as_str() {
            "fresh" => "current",
            "stale" => "known-stale",
            "never_analyzed" => "never-analyzed",
            _ => "unknown",
        }
        .to_string();
        evidence
    });
}

fn default_numeric_model(column_type: &str) -> &'static str {
    let normalized = column_type.trim().to_ascii_lowercase();
    if matches!(
        normalized.as_str(),
        "smallint"
            | "integer"
            | "bigint"
            | "numeric"
            | "decimal"
            | "real"
            | "float"
            | "double"
            | "double precision"
            | "number"
            | "binary_float"
            | "binary_double"
            | "user-defined"
            | "unknown"
    ) || normalized.starts_with("numeric(")
        || normalized.starts_with("decimal(")
        || normalized.starts_with("number(")
        || normalized.starts_with("float(")
    {
        "unknown"
    } else {
        "not-applicable"
    }
}

impl BlueprintTable {
    fn numeric_contract_needs_initialization(&self) -> bool {
        self.cols
            .values()
            .any(|column| column.numeric_model.is_empty())
    }
}

fn statistics_summary(
    tables: &BTreeMap<String, BlueprintTable>,
    structured: bool,
) -> StatisticsEvidence {
    let mut states = BTreeMap::new();
    let mut row_qualities = BTreeMap::new();
    let mut size_qualities = BTreeMap::new();
    for evidence in tables
        .values()
        .filter_map(|table| table.statistics.as_ref())
    {
        *states.entry(evidence.statistics_state.clone()).or_insert(0) += 1;
        *row_qualities
            .entry(evidence.row_count_quality.clone())
            .or_insert(0) += 1;
        *size_qualities
            .entry(evidence.size_quality.clone())
            .or_insert(0) += 1;
    }
    StatisticsEvidence {
        contract: STATISTICS_EVIDENCE_CONTRACT.to_string(),
        visibility: if structured { "full" } else { "unknown" }.to_string(),
        table_count: tables.len() as u64,
        counts_by_statistics_state: states,
        counts_by_row_count_quality: row_qualities,
        counts_by_size_quality: size_qualities,
        limitations: if structured {
            Vec::new()
        } else {
            vec!["statistics-provenance-unclassified".to_string()]
        },
        ..StatisticsEvidence::default()
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
