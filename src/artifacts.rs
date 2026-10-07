//! Bounded, name-free non-table artifact inventory and language-feature census.
//!
//! Engine readers construct `RawArtifact` values containing source identities
//! and, only for `analyzed`, transient definitions. This module converts them
//! to closed-vocabulary anonymous records. Raw identities and SQL text never
//! enter the serialized Blueprint.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result};
use clap::ValueEnum;
use zeroize::{Zeroize, Zeroizing};

use crate::audit::AuditLog;
use crate::format::{
    ArtifactInventory, ArtifactRequirement, BlueprintArtifact, BlueprintExternalPrerequisite,
    LanguageFeatureCensus, ARTIFACT_CONTRACT, LANGUAGE_CENSUS_CONTRACT,
};

#[derive(ValueEnum, Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum ArtifactDetail {
    None,
    #[default]
    Summary,
    Graph,
    Analyzed,
}

impl ArtifactDetail {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Summary => "summary",
            Self::Graph => "graph",
            Self::Analyzed => "analyzed",
        }
    }

    pub fn emits_graph(self) -> bool {
        matches!(self, Self::Graph | Self::Analyzed)
    }

    pub fn reads_definitions(self) -> bool {
        matches!(self, Self::Analyzed)
    }
}

#[derive(Debug, Clone)]
pub struct RawArtifact {
    /// Stable source identity used only for deterministic sorting and edges.
    pub identity: String,
    pub kind: &'static str,
    pub subkind: &'static str,
    pub tier: &'static str,
    pub schema_identity: Option<String>,
    /// Optional parent artifact identity. This is distinct from a table parent
    /// so Oracle package/type members can retain their catalog hierarchy.
    pub parent_artifact_identity: Option<String>,
    pub parent_table_identity: Option<String>,
    pub dependencies: Vec<String>,
    /// Engine-classified unresolved references, keyed by the closed
    /// unresolved-relationship reason vocabulary. Engines classify at the
    /// catalog, where the evidence lives (SQL Server knows a reference is
    /// cross-database or ambiguous from the dependency row itself); the
    /// central resolver only adds the two reasons it can actually establish.
    pub unresolved_reasons: BTreeMap<String, u64>,
    pub definition_visibility: &'static str,
    pub security_mode: &'static str,
    pub validity: &'static str,
    pub enabled: Option<bool>,
    pub generated_by_engine: Option<bool>,
    pub temporary: Option<bool>,
    pub editioned: Option<bool>,
    pub secondary_object: Option<bool>,
    /// Closed, generation-relevant facts established by a catalog or syntax
    /// reader. Source names and free-form values must never enter this list.
    pub requirements: Vec<RawArtifactRequirement>,
    /// Coverage of the requirement producers applicable to this object. The
    /// conservative default is unavailable. The builder promotes it to partial
    /// when it retains a known fact; an empty vector never proves the object
    /// has no requirements.
    pub requirement_status: RequirementStatus,
    pub external: Option<RawExternalPrerequisite>,
    pub analysis: Option<RawLanguageAnalysis>,
}

impl RawArtifact {
    pub fn new(identity: impl Into<String>, kind: &'static str, subkind: &'static str) -> Self {
        Self {
            identity: identity.into(),
            kind,
            subkind,
            tier: tier_for_kind(kind),
            schema_identity: None,
            parent_artifact_identity: None,
            parent_table_identity: None,
            dependencies: Vec::new(),
            unresolved_reasons: BTreeMap::new(),
            definition_visibility: "not_applicable",
            security_mode: "",
            validity: "",
            enabled: None,
            generated_by_engine: None,
            temporary: None,
            editioned: None,
            secondary_object: None,
            requirements: Vec::new(),
            requirement_status: RequirementStatus::Unavailable,
            external: None,
            analysis: None,
        }
    }
}

// Capture engines construct the remaining states from their requirement
// sources. Keeping the closed type complete avoids stringly-typed additions.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RequirementStatus {
    Complete,
    Partial,
    #[default]
    Unavailable,
    NotApplicable,
}

/// Close an engine's catalogue-backed requirement census after every required
/// field has been collected for the selected engine and version.
///
/// Engine readers call this only after every artifact family has been attempted.
/// A failed family remains visible through `catalogs_unreadable`, which keeps
/// the aggregate incomplete; objects returned by a successful family have a
/// complete per-object census, including a proven empty requirement list.
pub fn qualify_catalog_requirement_coverage(
    artifacts: &mut [RawArtifact],
    completeness: &mut CaptureCompleteness,
) {
    for artifact in artifacts {
        if artifact.requirement_status == RequirementStatus::Unavailable {
            artifact.requirement_status = RequirementStatus::Complete;
        }
    }
    completeness.requirements_complete = true;
}

impl RequirementStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::Unavailable => "unavailable",
            Self::NotApplicable => "not_applicable",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RawArtifactRequirement {
    pub token: &'static str,
    pub evidence: &'static str,
    pub count_band: &'static str,
}

impl RawArtifactRequirement {
    pub const fn catalog(token: &'static str) -> Self {
        Self {
            token,
            evidence: "catalog-confirmed",
            count_band: "1",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RawExternalPrerequisite {
    pub class: &'static str,
    pub deployment_scope: &'static str,
    pub binary_material: &'static str,
    pub secret_material: &'static str,
    pub endpoint_material: &'static str,
    pub compatibility: &'static str,
}

impl RawExternalPrerequisite {
    pub fn package(class: &'static str, compatibility: &'static str) -> Self {
        Self {
            class,
            deployment_scope: "host_or_server",
            binary_material: "required_not_captured",
            secret_material: "not_captured",
            endpoint_material: "not_captured",
            compatibility,
        }
    }

    pub fn infrastructure(class: &'static str, scope: &'static str) -> Self {
        Self {
            class,
            deployment_scope: scope,
            binary_material: "not_captured",
            secret_material: "may_be_required_not_captured",
            endpoint_material: "may_be_required_not_captured",
            compatibility: "target_environment_specific",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RawDefinitionSpan {
    /// The provider already supplied only the executable/declarative body.
    #[default]
    ExecutableBody,
    /// SQL Server exposed a complete CREATE/ALTER module definition; the
    /// shared analyzer must isolate the body before measuring it.
    SqlServerModule,
    /// PostgreSQL exposed a complete CREATE RULE wrapper.
    PostgreSqlRule,
    /// A provider requested definition analysis but cannot establish the
    /// canonical body span. This includes provider-detected wrapped source.
    Unknown,
    /// The artifact has no independently analyzable source body.
    NotApplicable,
}

#[derive(Debug, Clone, Default)]
pub struct RawLanguageAnalysis {
    pub definition: Option<Zeroizing<String>>,
    pub definition_span: RawDefinitionSpan,
    pub dialect: String,
    pub grammar_profile: String,
    pub sql_mode_flags: Vec<String>,
    pub compatibility_level: String,
    pub ansi_nulls: String,
    pub quoted_identifier: String,
}

#[derive(Debug, Clone, Default)]
pub struct CaptureCompleteness {
    pub visibility: String,
    pub inventory_complete: bool,
    /// Engine-proven completeness of the assessment population for the
    /// SELECTED scope, independent of broad inventory completeness. None
    /// falls back to the broad flag. MySQL is why this exists: its broad
    /// completeness bar is a global ALL PRIVILEGES grant that the shipped
    /// least-privilege model deliberately never holds, while per-schema
    /// grants plus global SHOW_ROUTINE genuinely prove every in-scope
    /// object is listed.
    pub assessment_population_complete: Option<bool>,
    pub dependencies_complete: bool,
    /// Capture-wide completeness check for artifact-requirement sources.
    /// The emitted aggregate also requires a complete selected assessment
    /// population and every per-object status to be complete or not applicable.
    /// Keep this false until the engine reader covers every field for the
    /// captured version; empty vectors alone prove nothing.
    pub requirements_complete: bool,
    pub catalogs_read: Vec<String>,
    pub catalogs_unreadable: Vec<String>,
    pub families_not_inventoried: Vec<String>,
}

pub fn table_identity(engine: &str, schema: &str, table: &str) -> String {
    format!("{engine}|table|{schema}|{table}")
}

pub fn grammar_profile(engine: &str, version: &str) -> String {
    let start = version
        .char_indices()
        .find_map(|(index, ch)| ch.is_ascii_digit().then_some(index));
    let numeric = start
        .map(|start| {
            version[start..]
                .chars()
                .take_while(|ch| ch.is_ascii_digit() || *ch == '.')
                .collect::<String>()
                .trim_matches('.')
                .split('.')
                .take(4)
                .collect::<Vec<_>>()
                .join(".")
        })
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    format!("{engine}-{numeric}")
}

pub fn build_inventory(
    detail: ArtifactDetail,
    raw: Vec<RawArtifact>,
    schema_ids: &BTreeMap<String, String>,
    table_ids: &BTreeMap<String, String>,
    selection: &crate::schema_scope::SchemaSelection,
    completeness: CaptureCompleteness,
    audit: &mut AuditLog,
) -> Result<ArtifactInventory> {
    build_inventory_with_assessor(
        InventoryBuildInputs {
            detail,
            raw,
            schema_ids,
            table_ids,
            selection,
            completeness,
            audit,
        },
        dbwarp_blueprint_core::assess_artifact_complexity,
    )
}

/// Assign anonymous schema identifiers over the complete emitted namespace.
///
/// Table and artifact collectors run independently, so a selected schema can
/// legitimately contain only non-table objects. Building this map from tables
/// alone silently erased that artifact's schema association. Keep the union in
/// one helper so every engine uses the same keyed ordering rule.
pub fn schema_ids_for_capture<'a>(
    table_schemas: impl IntoIterator<Item = &'a str>,
    raw_artifacts: &[RawArtifact],
    detail: ArtifactDetail,
) -> BTreeMap<String, String> {
    let mut schemas = table_schemas
        .into_iter()
        .map(ToString::to_string)
        .collect::<std::collections::BTreeSet<_>>();
    if detail.emits_graph() {
        schemas.extend(
            raw_artifacts
                .iter()
                .filter_map(|artifact| artifact.schema_identity.clone()),
        );
    }
    let mut schemas = schemas.into_iter().collect::<Vec<_>>();
    schemas.sort_by(|left, right| {
        crate::format::schema_hash(left)
            .cmp(&crate::format::schema_hash(right))
            .then_with(|| left.cmp(right))
    });
    schemas
        .into_iter()
        .enumerate()
        .map(|(index, schema)| (schema, crate::format::schema_id(index + 1)))
        .collect()
}

struct InventoryBuildInputs<'a> {
    detail: ArtifactDetail,
    raw: Vec<RawArtifact>,
    schema_ids: &'a BTreeMap<String, String>,
    table_ids: &'a BTreeMap<String, String>,
    selection: &'a crate::schema_scope::SchemaSelection,
    completeness: CaptureCompleteness,
    audit: &'a mut AuditLog,
}

fn build_inventory_with_assessor<F>(
    inputs: InventoryBuildInputs<'_>,
    assessor: F,
) -> Result<ArtifactInventory>
where
    F: FnOnce(&ArtifactInventory, bool) -> Result<dbwarp_blueprint_core::ArtifactComplexity>,
{
    let InventoryBuildInputs {
        detail,
        mut raw,
        schema_ids,
        table_ids,
        selection,
        mut completeness,
        audit,
    } = inputs;
    completeness.catalogs_read.sort();
    completeness.catalogs_read.dedup();
    completeness.catalogs_unreadable.sort();
    completeness.catalogs_unreadable.dedup();
    completeness.families_not_inventoried.sort();
    completeness.families_not_inventoried.dedup();

    raw.sort_by(|a, b| {
        a.kind
            .cmp(b.kind)
            .then_with(|| stable_hash(&a.identity).cmp(&stable_hash(&b.identity)))
            .then_with(|| a.identity.cmp(&b.identity))
    });

    let mut ordinals: BTreeMap<&str, u64> = BTreeMap::new();
    let mut artifact_ids = BTreeMap::new();
    for item in &raw {
        let ordinal = ordinals.entry(item.kind).or_default();
        *ordinal = ordinal
            .checked_add(1)
            .context("artifact kind ordinal overflowed u64")?;
        artifact_ids.insert(item.identity.clone(), format!("{}-{ordinal:03}", item.kind));
    }

    let mut counts_by_kind = BTreeMap::new();
    let mut counts_by_external_class = BTreeMap::new();
    let mut external_prerequisite_count = 0_u64;
    let mut dependency_edge_count = 0_u64;
    let mut artifacts = BTreeMap::new();
    let mut all_analysis_complete = detail == ArtifactDetail::Analyzed;
    let mut saw_analysis = false;

    for item in raw {
        let kind_count = counts_by_kind.entry(item.kind.to_string()).or_insert(0_u64);
        *kind_count = kind_count
            .checked_add(1)
            .context("artifact kind count overflowed u64")?;
        if let Some(external) = &item.external {
            external_prerequisite_count = external_prerequisite_count
                .checked_add(1)
                .context("artifact external prerequisite count overflowed u64")?;
            let external_count = counts_by_external_class
                .entry(external.class.to_string())
                .or_insert(0_u64);
            *external_count = external_count
                .checked_add(1)
                .context("artifact external class count overflowed u64")?;
        }

        if !detail.emits_graph() {
            continue;
        }

        let mut relationships = Vec::new();
        let mut unresolved_relationships = item.unresolved_reasons.clone();
        for dependency in &item.dependencies {
            if let Some(id) = artifact_ids.get(dependency) {
                relationships.push(dbwarp_blueprint_core::ArtifactRelationship {
                    kind: "references-object".to_string(),
                    target: id.clone(),
                    evidence: "dependency-confirmed".to_string(),
                });
            } else if let Some(id) = table_ids.get(dependency) {
                relationships.push(dbwarp_blueprint_core::ArtifactRelationship {
                    kind: "references-table".to_string(),
                    target: id.clone(),
                    evidence: "dependency-confirmed".to_string(),
                });
            } else {
                // Identities carry their schema as the third segment on
                // every engine, so a target in an unselected schema is
                // distinguishable here from one the account cannot see. The
                // check is against the operator's selection, never against
                // schema_ids: that map covers schemas represented by emitted
                // records, not the schema of an unresolved target. Using it
                // would misread a selected but privilege-hidden target as
                // outside the selection.
                // MySQL's global loadable functions use the pseudo-schema
                // "loadable" in their identities; they are always in
                // artifact_ids when they exist at all, so they cannot reach
                // this arm. An inactive selection includes everything, which
                // makes all-visible captures classify nothing as outside.
                let reason = match dependency.split('|').nth(2) {
                    Some(schema) if !schema.is_empty() && !selection.includes(schema) => {
                        "outside-selected-schema"
                    }
                    _ => "target-not-visible",
                };
                let unresolved_count = unresolved_relationships
                    .entry(reason.to_string())
                    .or_insert(0_u64);
                *unresolved_count = unresolved_count
                    .checked_add(1)
                    .context("artifact unresolved relationship count overflowed u64")?;
            }
        }
        relationships.sort_by(|left, right| {
            (&left.kind, &left.target, &left.evidence).cmp(&(
                &right.kind,
                &right.target,
                &right.evidence,
            ))
        });
        relationships.dedup();
        dependency_edge_count = dependency_edge_count
            .checked_add(
                u64::try_from(relationships.len())
                    .context("artifact relationship count exceeds u64")?,
            )
            .context("artifact dependency edge count overflowed u64")?;

        let analysis = if detail == ArtifactDetail::Analyzed {
            item.analysis.as_ref().map(analyze_language)
        } else {
            None
        };
        if let Some(analysis) = &analysis {
            saw_analysis = true;
            if analysis.status != "complete" {
                all_analysis_complete = false;
            }
        }

        let id = artifact_ids
            .get(&item.identity)
            .expect("artifact id assigned above")
            .clone();
        let requirement_status = if item.requirement_status == RequirementStatus::Unavailable
            && (!item.requirements.is_empty() || item.external.is_some())
        {
            // A captured fact proves that at least one engine reader ran, but it
            // does not make the list exhaustive. Unless the reader assigns a
            // complete/not-applicable state, preserve that useful
            // distinction as partial coverage.
            RequirementStatus::Partial
        } else {
            item.requirement_status
        };
        let mut requirements = item.requirements;
        requirements.sort();
        requirements.dedup();
        let schema = match item.schema_identity.as_ref() {
            Some(schema) => schema_ids
                .get(schema)
                .cloned()
                .context("schema-scoped artifact is missing its anonymous schema mapping")?,
            None => String::new(),
        };
        artifacts.insert(
            id,
            BlueprintArtifact {
                kind: item.kind.to_string(),
                subkind: item.subkind.to_string(),
                tier: item.tier.to_string(),
                schema,
                parent: item
                    .parent_artifact_identity
                    .as_ref()
                    .and_then(|artifact| artifact_ids.get(artifact))
                    .or_else(|| {
                        item.parent_table_identity
                            .as_ref()
                            .and_then(|table| table_ids.get(table))
                    })
                    .cloned()
                    .unwrap_or_default(),
                dependencies: Vec::new(),
                relationships,
                requirements: requirements
                    .into_iter()
                    .map(|requirement| ArtifactRequirement {
                        token: requirement.token.to_string(),
                        evidence: requirement.evidence.to_string(),
                        count_band: requirement.count_band.to_string(),
                    })
                    .collect(),
                requirement_status: requirement_status.as_str().to_string(),
                unresolved_dependency_count: 0,
                unresolved_relationships,
                definition_visibility: item.definition_visibility.to_string(),
                security_mode: item.security_mode.to_string(),
                validity: item.validity.to_string(),
                enabled: item.enabled,
                generated_by_engine: item.generated_by_engine,
                temporary: item.temporary,
                editioned: item.editioned,
                secondary_object: item.secondary_object,
                external: item.external.map(|external| BlueprintExternalPrerequisite {
                    class: external.class.to_string(),
                    deployment_scope: external.deployment_scope.to_string(),
                    binary_material: external.binary_material.to_string(),
                    secret_material: external.secret_material.to_string(),
                    endpoint_material: external.endpoint_material.to_string(),
                    compatibility: external.compatibility.to_string(),
                }),
                analysis,
            },
        );
    }

    let object_count = counts_by_kind
        .values()
        .try_fold(0_u64, |sum, count| sum.checked_add(*count))
        .context("artifact object count overflowed u64")?;
    let completeness_inventory_complete = completeness.inventory_complete
        && completeness.catalogs_unreadable.is_empty()
        && completeness.families_not_inventoried.is_empty();
    let completeness_dependencies_complete =
        completeness.dependencies_complete && completeness.catalogs_unreadable.is_empty();
    // Independent of broad inventory completeness: the
    // families this collector never inventories were never candidates for
    // the assessment population, so declaring them must not force every
    // estate's population incomplete. What matters is that the object
    // listing for the inventoried families is complete and every catalog
    // that feeds it was readable.
    let population_complete = completeness
        .assessment_population_complete
        .unwrap_or(completeness.inventory_complete)
        && completeness.catalogs_unreadable.is_empty();
    let mut inventory = ArtifactInventory {
        contract: ARTIFACT_CONTRACT.to_string(),
        detail: detail.as_str().to_string(),
        scope: String::new(),
        visibility: if completeness.visibility.is_empty()
            || (completeness.visibility == "full" && !completeness.catalogs_unreadable.is_empty())
        {
            // A capture that could not read one of its own catalogs cannot
            // stand behind a full-visibility claim, whatever the engine
            // asserted before the failures happened.
            "unknown".to_string()
        } else {
            completeness.visibility
        },
        inventory_complete: completeness_inventory_complete,
        dependencies_complete: completeness_dependencies_complete,
        requirements_complete: completeness.requirements_complete
            && detail.emits_graph()
            && population_complete
            && artifacts.values().all(|artifact| {
                matches!(
                    artifact.requirement_status.as_str(),
                    "complete" | "not_applicable"
                )
            }),
        // An empty or wholly inapplicable inventory must not make a vacuous
        // language-analysis completeness claim.
        analysis_complete: all_analysis_complete && saw_analysis,
        object_count,
        dependency_edge_count,
        external_prerequisite_count,
        counts_by_kind,
        counts_by_external_class,
        catalogs_read: completeness.catalogs_read,
        catalogs_unreadable: completeness.catalogs_unreadable,
        catalogs_not_applicable: Vec::new(),
        families_not_inventoried: completeness.families_not_inventoried,
        complexity: None,
        artifacts,
    };
    if detail.emits_graph() {
        inventory.complexity = Some(match assessor(&inventory, population_complete) {
            Ok(complexity) => complexity,
            Err(_) => {
                audit.record_warning(
                        "DBP1422W",
                        "DBP1422W artifact complexity assessment failed; the inventory was retained and every affected aggregate dimension was marked unknown",
                    );
                dbwarp_blueprint_core::failed_artifact_complexity(&inventory, population_complete)?
            }
        });
    }
    Ok(inventory)
}

// ---------------------------------------------------------------------------
// Test-only differential complexity reference
//
// Production assessment and validation live in dbwarp-blueprint-core. This
// deliberately separate transliteration remains available only to unit tests
// as a differential check; it is not an independent semantic oracle. The
// hand-authored fixture corpus in tests/fixtures/artifact_complexity provides
// that independence.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod reference_assessor {
    use super::*;

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum DimensionObservation {
        Assessed(usize),
        NotApplicable,
        Unknown,
    }

    const SIZE_BAND_ORDER: [&str; 7] =
        ["0", "1-255", "256-1k", "1k-4k", "4k-16k", "16k-64k", "64k+"];
    const COUNT_BAND_ORDER: [&str; 7] = ["0", "1", "2-4", "5-8", "9-16", "17-32", "33+"];

    /// Histogram bucket index to the shared complexity-band vocabulary. The two
    /// lowest buckets are trivial, the top one very-high; migration effort
    /// follows the estate's ceiling, so dimension verdicts use the highest
    /// occupied bucket.
    fn complexity_band_for_bucket(index: usize) -> &'static str {
        match index {
            0 | 1 => "trivial",
            2 => "low",
            3 => "moderate",
            4 | 5 => "high",
            _ => "very-high",
        }
    }

    fn band_rank(band: &str) -> u8 {
        match band {
            "trivial" => 1,
            "low" => 2,
            "moderate" => 3,
            "high" => 4,
            "very-high" => 5,
            _ => 0,
        }
    }

    struct DimensionTally {
        assessed: [u64; 7],
        not_applicable: u64,
        unknown: u64,
    }

    impl DimensionTally {
        fn new() -> Self {
            Self {
                assessed: [0; 7],
                not_applicable: 0,
                unknown: 0,
            }
        }

        fn record(&mut self, observation: DimensionObservation) {
            match observation {
                DimensionObservation::Assessed(index) => self.assessed[index.min(6)] += 1,
                DimensionObservation::NotApplicable => self.not_applicable += 1,
                DimensionObservation::Unknown => self.unknown += 1,
            }
        }

        fn coverage_and_band(&self, eligible: u64) -> (String, String) {
            if eligible == 0 || (self.not_applicable == eligible && self.unknown == 0) {
                return ("not-applicable".to_string(), "not-applicable".to_string());
            }
            if self.unknown == eligible {
                return ("unknown".to_string(), "unknown".to_string());
            }
            let coverage = if self.unknown == 0 {
                "complete"
            } else {
                "partial"
            };
            let lower_bound = self
                .assessed
                .iter()
                .enumerate()
                .filter(|(_, count)| **count > 0)
                .map(|(index, _)| index)
                .next_back()
                .map(complexity_band_for_bucket)
                .unwrap_or("trivial");
            let band = if self.unknown == 0 || lower_bound == "very-high" {
                lower_bound
            } else {
                "unknown"
            };
            (coverage.to_string(), band.to_string())
        }

        fn size_dimension(
            &self,
            eligible: u64,
        ) -> dbwarp_blueprint_core::ArtifactComplexitySizeDimension {
            let (coverage, band) = self.coverage_and_band(eligible);
            dbwarp_blueprint_core::ArtifactComplexitySizeDimension {
                band,
                coverage,
                histogram: dbwarp_blueprint_core::ArtifactComplexitySizeHistogram {
                    zero: self.assessed[0],
                    one_to_two_fifty_five: self.assessed[1],
                    two_fifty_six_to_one_k: self.assessed[2],
                    one_k_to_four_k: self.assessed[3],
                    four_k_to_sixteen_k: self.assessed[4],
                    sixteen_k_to_sixty_four_k: self.assessed[5],
                    sixty_four_k_plus: self.assessed[6],
                    not_applicable: self.not_applicable,
                    unknown: self.unknown,
                },
            }
        }

        fn count_dimension(
            &self,
            eligible: u64,
        ) -> dbwarp_blueprint_core::ArtifactComplexityCountDimension {
            let (coverage, band) = self.coverage_and_band(eligible);
            dbwarp_blueprint_core::ArtifactComplexityCountDimension {
                band,
                coverage,
                histogram: dbwarp_blueprint_core::ArtifactComplexityCountHistogram {
                    zero: self.assessed[0],
                    one: self.assessed[1],
                    two_to_four: self.assessed[2],
                    five_to_eight: self.assessed[3],
                    nine_to_sixteen: self.assessed[4],
                    seventeen_to_thirty_two: self.assessed[5],
                    thirty_three_plus: self.assessed[6],
                    not_applicable: self.not_applicable,
                    unknown: self.unknown,
                },
            }
        }
    }

    fn size_band_index(band: &str) -> usize {
        SIZE_BAND_ORDER.iter().position(|b| *b == band).unwrap_or(0)
    }

    fn count_band_index(band: &str) -> usize {
        COUNT_BAND_ORDER
            .iter()
            .position(|b| *b == band)
            .unwrap_or(0)
    }

    pub fn assess_complexity(
        detail: ArtifactDetail,
        artifacts: &BTreeMap<String, dbwarp_blueprint_core::BlueprintArtifact>,
        population_complete: bool,
        dependencies_complete: bool,
        requirements_complete: bool,
    ) -> dbwarp_blueprint_core::ArtifactComplexity {
        let analyzed = detail == ArtifactDetail::Analyzed;
        let mut volume = DimensionTally::new();
        let mut control_flow = DimensionTally::new();
        let mut feature_breadth = DimensionTally::new();
        let mut entanglement = DimensionTally::new();
        let mut environment_coupling = DimensionTally::new();
        let mut opacity = DimensionTally::new();
        let mut dialect_coupling = DimensionTally::new();

        let mut eligible = 0_u64;
        let mut excluded = 0_u64;
        let mut fully = 0_u64;
        let mut partially = 0_u64;
        let mut unassessed = 0_u64;
        let mut dialects = BTreeSet::new();
        let mut grammar_profiles = BTreeSet::new();
        let mut analysis_spans = BTreeSet::new();
        let mut definitions_withheld = false;
        let mut unsupported_dialect = false;
        let mut wrapped_source = false;
        let mut outside_selected_scope = false;
        // Mirrors the serializer's LEXICAL_SUPPORTED_DIALECTS: the validator
        // recomputes the unsupported-dialect limitation from this exact set.
        let lexical_supported = ["sql", "plpgsql", "mysql-sql-psm", "plsql", "tsql"];

        for artifact in artifacts.values() {
            if artifact.generated_by_engine == Some(true) || artifact.secondary_object == Some(true)
            {
                excluded += 1;
                continue;
            }
            eligible += 1;

            definitions_withheld |= analyzed && artifact.definition_visibility == "withheld";
            wrapped_source |= analyzed
                && artifact
                    .requirements
                    .iter()
                    .any(|requirement| requirement.token == "oracle.source.wrapped");
            outside_selected_scope |= artifact
                .unresolved_relationships
                .contains_key("outside-selected-schema");

            // Graph-visible dimensions: catalog facts, determined at either
            // detail. Unresolved references are still edges the object has.
            //
            // FORMAT.md defines missing graph edges as unknown rather than low
            // entanglement. Dependency completeness is recorded per capture,
            // not per object, so every eligible object takes the capture-wide
            // flag. Engines declare incomplete dependency evidence when they
            // cannot support this dimension.
            let mut unknown_dimensions = 0_u32;
            let mut assessed_dimensions = 0_u32;
            if dependencies_complete {
                let unresolved_total: u64 = artifact.unresolved_relationships.values().sum();
                let edge_count = artifact.relationships.len() as u64 + unresolved_total;
                entanglement.record(DimensionObservation::Assessed(count_band_index(
                    &count_band(edge_count),
                )));
                assessed_dimensions += 1;
            } else {
                entanglement.record(DimensionObservation::Unknown);
                unknown_dimensions += 1;
            }
            match artifact.requirement_status.as_str() {
                "complete" => {
                    let external_requirements = artifact
                        .requirements
                        .iter()
                        .filter(|requirement| requirement.token.starts_with("external."))
                        .count() as u64;
                    let external_count =
                        external_requirements + u64::from(artifact.external.is_some());
                    environment_coupling.record(DimensionObservation::Assessed(count_band_index(
                        &count_band(external_count),
                    )));
                    let dialect_specific =
                        artifact.requirements.len() as u64 - external_requirements;
                    dialect_coupling.record(DimensionObservation::Assessed(count_band_index(
                        &count_band(dialect_specific),
                    )));
                    assessed_dimensions += 2;
                }
                "not_applicable" => {
                    environment_coupling.record(DimensionObservation::NotApplicable);
                    dialect_coupling.record(DimensionObservation::NotApplicable);
                    assessed_dimensions += 2;
                }
                _ => {
                    environment_coupling.record(DimensionObservation::Unknown);
                    dialect_coupling.record(DimensionObservation::Unknown);
                    unknown_dimensions += 2;
                }
            }

            // Definition-derived dimensions. The collector's own visibility
            // verdict decides not-applicable; nothing is inferred from kind.
            let definition_free = artifact.definition_visibility == "not_applicable";
            let census = artifact.analysis.as_ref();
            if let Some(analysis) = census {
                dialects.insert(analysis.dialect.clone());
                grammar_profiles.insert(analysis.grammar_profile.clone());
                analysis_spans.insert(if analysis.analysis_span.is_empty() {
                    "unknown".to_string()
                } else {
                    analysis.analysis_span.clone()
                });
                wrapped_source |= analyzed && analysis.features.contains_key("source.wrapped");
                if analysis.status == "unavailable"
                    && artifact.definition_visibility == "available"
                    && !lexical_supported.contains(&analysis.dialect.as_str())
                {
                    unsupported_dialect = true;
                }
            }
            let definition_observation = |assessed: DimensionObservation| {
                if definition_free {
                    DimensionObservation::NotApplicable
                } else {
                    assessed
                }
            };
            let census_partial = census.filter(|analysis| analysis.status == "partial");
            match census_partial {
                Some(analysis) if analyzed => {
                    volume.record(definition_observation(DimensionObservation::Assessed(
                        size_band_index(&analysis.definition_size_band),
                    )));
                    control_flow.record(definition_observation(DimensionObservation::Assessed(
                        count_band_index(&analysis.cyclomatic_complexity_band),
                    )));
                    // Feature breadth is the number of distinct feature families
                    // the census recorded - band arithmetic over census output,
                    // never a second lexing pass.
                    feature_breadth.record(definition_observation(DimensionObservation::Assessed(
                        count_band_index(&count_band(analysis.features.len() as u64)),
                    )));
                    opacity.record(definition_observation(DimensionObservation::Assessed(
                        count_band_index(&analysis.opaque_region_count_band),
                    )));
                    assessed_dimensions += 4;
                }
                _ => {
                    let observation = if definition_free {
                        assessed_dimensions += 4;
                        DimensionObservation::NotApplicable
                    } else {
                        unknown_dimensions += 4;
                        DimensionObservation::Unknown
                    };
                    volume.record(observation);
                    control_flow.record(observation);
                    feature_breadth.record(observation);
                    opacity.record(observation);
                }
            }

            if unknown_dimensions == 0 {
                fully += 1;
            } else if assessed_dimensions == 0 {
                unassessed += 1;
            } else {
                partially += 1;
            }
        }

        let dimensions = dbwarp_blueprint_core::ArtifactComplexityDimensions {
            volume: volume.size_dimension(eligible),
            control_flow: control_flow.count_dimension(eligible),
            feature_breadth: feature_breadth.count_dimension(eligible),
            entanglement: entanglement.count_dimension(eligible),
            environment_coupling: environment_coupling.count_dimension(eligible),
            opacity: opacity.count_dimension(eligible),
            dialect_coupling: dialect_coupling.count_dimension(eligible),
        };

        let mut limitations: Vec<String> = Vec::new();
        if !analyzed {
            limitations.push("definition-analysis-not-requested".to_string());
        }
        if definitions_withheld {
            limitations.push("definitions-withheld".to_string());
        }
        if unsupported_dialect {
            limitations.push("unsupported-dialect".to_string());
        }
        if wrapped_source {
            limitations.push("wrapped-source".to_string());
        }
        if !dependencies_complete {
            limitations.push("graph-incomplete".to_string());
        }
        if !requirements_complete {
            limitations.push("requirements-incomplete".to_string());
        }
        if outside_selected_scope {
            limitations.push("outside-selected-scope".to_string());
        }
        limitations.sort();

        let assessment_population_complete = population_complete;
        let candidate = [
            dimensions.volume.band.as_str(),
            dimensions.control_flow.band.as_str(),
            dimensions.feature_breadth.band.as_str(),
            dimensions.entanglement.band.as_str(),
            dimensions.environment_coupling.band.as_str(),
            dimensions.opacity.band.as_str(),
            dimensions.dialect_coupling.band.as_str(),
        ]
        .into_iter()
        .max_by_key(|band| band_rank(band))
        .filter(|band| band_rank(band) > 0)
        .unwrap_or("trivial")
        .to_string();

        let overall_band = if eligible == 0 {
            if assessment_population_complete {
                "not-applicable".to_string()
            } else {
                "unknown".to_string()
            }
        } else if !analyzed {
            "unknown".to_string()
        } else if assessment_population_complete && fully == eligible {
            candidate
        } else if candidate == "very-high" {
            // An incomplete or partially assessed estate already proves at
            // least very-high; more evidence can only keep it there.
            candidate
        } else {
            "unknown".to_string()
        };

        dbwarp_blueprint_core::ArtifactComplexity {
            contract: dbwarp_blueprint_core::ARTIFACT_COMPLEXITY_CONTRACT.to_string(),
            assessor_version: dbwarp_blueprint_core::ARTIFACT_COMPLEXITY_ASSESSOR_VERSION,
            // Stamped alongside the inventory scope by the v7 contract
            // initializer; empty until then, exactly like the inventory's own.
            scope: String::new(),
            population_policy: dbwarp_blueprint_core::ARTIFACT_COMPLEXITY_POPULATION_POLICY
                .to_string(),
            eligible_object_count: eligible,
            fully_assessed_object_count: fully,
            partially_assessed_object_count: partially,
            unassessed_object_count: unassessed,
            excluded_object_count: excluded,
            analyzer_version: if analyzed {
                "lexical-v2"
            } else {
                "not-applicable"
            }
            .to_string(),
            dialects: dialects.into_iter().collect(),
            grammar_profiles: grammar_profiles.into_iter().collect(),
            analysis_spans: analysis_spans.into_iter().collect(),
            assessment_population_complete,
            overall_band,
            overall_score: None,
            limitations,
            dimensions,
        }
    }
}

fn stable_hash(value: &str) -> [u8; 8] {
    crate::format::artifact_hash(value)
}

fn tier_for_kind(kind: &str) -> &'static str {
    match kind {
        "view" | "materialized_view" | "sequence" | "type" | "default" | "rule" | "policy"
        | "synonym" => "declarative",
        "function" | "procedure" | "aggregate" | "trigger" | "event_trigger" | "scheduled_job" => {
            "programmatic"
        }
        "extension" | "foreign_server" | "publication" | "subscription" | "assembly"
        | "external_table" | "full_text" => "external",
        "partition_scheme" | "physical_placement" => "physical",
        "certificate" | "encryption_key" => "security",
        _ => "other",
    }
}

fn analyze_language(raw: &RawLanguageAnalysis) -> LanguageFeatureCensus {
    let dialect = normalize_dialect(&raw.dialect);
    if raw.definition_span == RawDefinitionSpan::NotApplicable {
        return empty_language_census(raw, dialect, "not_applicable", "not-applicable");
    }
    if raw.definition_span == RawDefinitionSpan::Unknown {
        return empty_language_census(raw, dialect, "unavailable", "unknown");
    }
    let Some(definition) = raw.definition.as_deref() else {
        return empty_language_census(raw, dialect, "unavailable", "executable-body");
    };

    let definition = match raw.definition_span {
        RawDefinitionSpan::ExecutableBody => definition,
        RawDefinitionSpan::SqlServerModule => {
            let Some(body) = extract_wrapped_body(definition, "AS") else {
                return empty_language_census(raw, dialect, "unavailable", "unknown");
            };
            body
        }
        RawDefinitionSpan::PostgreSqlRule => {
            let Some(body) = extract_wrapped_body(definition, "DO") else {
                return empty_language_census(raw, dialect, "unavailable", "unknown");
            };
            body
        }
        RawDefinitionSpan::Unknown => unreachable!("handled above"),
        RawDefinitionSpan::NotApplicable => unreachable!("handled above"),
    };

    if !lexical_v2_supports_dialect(&dialect) {
        return empty_language_census(raw, dialect, "unavailable", "executable-body");
    }

    let scrubbed = scrub_sql(definition, &dialect);
    let tokens = sql_tokens(&scrubbed);
    // Oracle exposes wrapped source as readable text. Refuse to measure it in
    // the shared analyzer even when a provider accidentally passes the bytes
    // through; otherwise obfuscation tokens would look like real PL/SQL.
    if dialect == "plsql" && is_wrapped_plsql(&tokens) {
        return empty_language_census(raw, dialect, "unavailable", "unknown");
    }
    let mut features = BTreeMap::new();
    let feature_specs: &[(&str, &[&str])] = &[
        ("control.if", &["IF"]),
        ("control.case", &["CASE"]),
        ("control.loop", &["LOOP"]),
        ("control.while", &["WHILE"]),
        ("control.repeat", &["REPEAT"]),
        ("control.exception", &["EXCEPTION", "CATCH", "HANDLER"]),
        ("control.goto", &["GOTO"]),
        (
            "control.raise",
            &["RAISE", "THROW", "RAISERROR", "SIGNAL", "RESIGNAL"],
        ),
        ("interface.cursor", &["CURSOR"]),
        ("query.join", &["JOIN"]),
        ("query.subquery", &["SELECT"]),
        ("query.recursive", &["RECURSIVE"]),
        ("query.aggregate", &["COUNT", "SUM", "AVG", "MIN", "MAX"]),
        ("query.window", &["OVER"]),
        ("query.group_by", &["GROUP_BY"]),
        ("query.set_operation", &["UNION", "INTERSECT", "EXCEPT"]),
        ("query.order_by", &["ORDER_BY"]),
        ("query.limit", &["LIMIT", "TOP", "FETCH"]),
        ("data.select", &["SELECT"]),
        ("data.merge", &["MERGE"]),
        ("state.ddl", &["CREATE", "ALTER", "DROP", "TRUNCATE"]),
        ("state.temporary", &["TEMP", "TEMPORARY", "#TEMP"]),
        ("type.interval", &["INTERVAL"]),
        ("type.boolean", &["BOOLEAN", "BOOL", "BIT"]),
        ("type.json", &["JSON", "JSONB", "JSON_VALUE", "JSON_QUERY"]),
        ("type.xml", &["XML"]),
        ("type.spatial", &["GEOMETRY", "GEOGRAPHY", "ST_"]),
        ("type.vector", &["VECTOR"]),
        ("security.definer", &["DEFINER"]),
        ("security.invoker", &["INVOKER"]),
        ("security.impersonation", &["EXECUTE_AS"]),
    ];
    for (feature, needles) in feature_specs {
        let count = needles
            .iter()
            .map(|needle| tokens.iter().filter(|token| **token == *needle).count())
            .sum::<usize>();
        if count > 0 {
            features.insert((*feature).to_string(), count_band(count as u64));
        }
    }

    insert_sequence_feature(
        &mut features,
        "binding.percent_type",
        sequence_count(&tokens, &["%", "TYPE"]),
    );
    insert_sequence_feature(
        &mut features,
        "binding.percent_rowtype",
        sequence_count(&tokens, &["%", "ROWTYPE"]),
    );
    insert_sequence_feature(
        &mut features,
        "interface.ref_cursor",
        token_count(&tokens, &["REFCURSOR", "SYS_REFCURSOR"])
            + sequence_count(&tokens, &["REF", "CURSOR"]),
    );
    insert_sequence_feature(
        &mut features,
        "type.timezone",
        token_count(&tokens, &["TIMESTAMPTZ", "DATETIMEOFFSET"])
            + sequence_count(&tokens, &["AT", "TIME", "ZONE"])
            + sequence_count(&tokens, &["TIMESTAMP", "WITH", "TIME", "ZONE"])
            + sequence_count(&tokens, &["TIMESTAMP", "WITH", "LOCAL", "TIME", "ZONE"]),
    );
    let transaction_count = tokens
        .iter()
        .filter(|token| matches!(**token, "COMMIT" | "ROLLBACK" | "SAVEPOINT"))
        .count() as u64
        + sequence_count(&tokens, &["START", "TRANSACTION"])
        + sequence_count(&tokens, &["BEGIN", "TRANSACTION"])
        + sequence_count(&tokens, &["BEGIN", "TRAN"]);
    insert_sequence_feature(&mut features, "transaction.control", transaction_count);
    let max_lob_count = ["VARBINARY", "VARCHAR", "NVARCHAR"]
        .iter()
        .map(|kind| sequence_count(&tokens, &[kind, "(", "MAX", ")"]))
        .sum::<u64>();
    insert_sequence_feature(
        &mut features,
        "type.lob",
        token_count(&tokens, &["BLOB", "CLOB", "NCLOB", "BYTEA", "IMAGE"]) + max_lob_count,
    );

    // A lone top-level SELECT is not a subquery. Record subquery only when
    // another SELECT exists, while retaining data.select for either case.
    let select_count = tokens.iter().filter(|token| **token == "SELECT").count();
    if select_count < 2 {
        features.remove("query.subquery");
    } else {
        features.insert(
            "query.subquery".to_string(),
            count_band((select_count - 1) as u64),
        );
    }

    let cte_count = cte_count(&tokens);
    if cte_count > 0 {
        features.insert("query.cte".to_string(), count_band(cte_count));
    }

    for (feature, keyword) in [
        ("data.insert", "INSERT"),
        ("data.update", "UPDATE"),
        ("data.delete", "DELETE"),
    ] {
        let count = data_operation_count(&tokens, keyword);
        if count > 0 {
            features.insert(feature.to_string(), count_band(count));
        }
    }

    let dynamic_count = dynamic_sql_count(&tokens);
    if dynamic_count > 0 {
        features.insert("dynamic.sql".to_string(), count_band(dynamic_count));
    }

    let maximum_nesting = maximum_nesting(&dialect, &tokens);
    let branches = branch_count(&tokens);
    let statement_count = tokens.iter().filter(|token| **token == ";").count().max(1);
    let opaque_count = dynamic_count;

    LanguageFeatureCensus {
        contract: LANGUAGE_CENSUS_CONTRACT.to_string(),
        // This analyzer is deliberately lexical. It never claims that a
        // dialect grammar or semantic binder accepted the source definition.
        status: "partial".to_string(),
        dialect,
        grammar_profile: raw.grammar_profile.clone(),
        analyzer_version: "lexical-v2".to_string(),
        analysis_span: "executable-body".to_string(),
        definition_size_band: byte_size_band(definition.len() as u64),
        statement_count_band: count_band(statement_count as u64),
        token_count_band: count_band(tokens.len() as u64),
        maximum_nesting_band: count_band(maximum_nesting),
        cyclomatic_complexity_band: count_band(1 + branches),
        opaque_region_count_band: count_band(opaque_count as u64),
        minimum_source_version: String::new(),
        minimum_version_complete: false,
        sql_mode_flags: normalize_sql_modes(&raw.sql_mode_flags),
        compatibility_level: raw.compatibility_level.clone(),
        ansi_nulls: normalize_switch(&raw.ansi_nulls),
        quoted_identifier: normalize_switch(&raw.quoted_identifier),
        features,
    }
}

fn empty_language_census(
    raw: &RawLanguageAnalysis,
    dialect: String,
    status: &str,
    analysis_span: &str,
) -> LanguageFeatureCensus {
    LanguageFeatureCensus {
        contract: LANGUAGE_CENSUS_CONTRACT.to_string(),
        status: status.to_string(),
        dialect,
        grammar_profile: raw.grammar_profile.clone(),
        analyzer_version: "lexical-v2".to_string(),
        analysis_span: analysis_span.to_string(),
        sql_mode_flags: normalize_sql_modes(&raw.sql_mode_flags),
        compatibility_level: raw.compatibility_level.clone(),
        ansi_nulls: normalize_switch(&raw.ansi_nulls),
        quoted_identifier: normalize_switch(&raw.quoted_identifier),
        ..LanguageFeatureCensus::default()
    }
}

fn insert_sequence_feature(features: &mut BTreeMap<String, String>, feature: &str, count: u64) {
    if count > 0 {
        features.insert(feature.to_string(), count_band(count));
    }
}

fn token_count(tokens: &[&str], needles: &[&str]) -> u64 {
    tokens
        .iter()
        .filter(|token| needles.contains(token))
        .count() as u64
}

fn sequence_count(tokens: &[&str], needle: &[&str]) -> u64 {
    if needle.is_empty() {
        return 0;
    }
    tokens
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count() as u64
}

#[derive(Clone, Copy)]
struct SqlWord<'a> {
    value: &'a str,
    end: usize,
}

/// Return words outside literals, quoted identifiers and comments while
/// retaining their byte offsets in the transient definition. The offsets let
/// wrapper extraction slice the original text without ever serializing source
/// words or relying on a regex that mistakes `EXECUTE AS` for the body marker.
fn sql_words(input: &str) -> Vec<SqlWord<'_>> {
    #[derive(Clone, Copy)]
    enum State {
        Normal,
        Single,
        Double,
        Backtick,
        Bracket,
        LineComment,
        BlockComment(u32),
    }

    fn flush<'a>(
        input: &'a str,
        start: &mut Option<usize>,
        end: usize,
        out: &mut Vec<SqlWord<'a>>,
    ) {
        if let Some(start) = start.take() {
            out.push(SqlWord {
                value: &input[start..end],
                end,
            });
        }
    }

    let mut out = Vec::new();
    let mut state = State::Normal;
    let mut word_start = None;
    let mut chars = input.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        let next = chars.peek().map(|(_, ch)| *ch);
        match state {
            State::Normal if ch == '-' && next == Some('-') => {
                flush(input, &mut word_start, index, &mut out);
                state = State::LineComment;
                chars.next();
            }
            State::Normal if ch == '/' && next == Some('*') => {
                flush(input, &mut word_start, index, &mut out);
                state = State::BlockComment(1);
                chars.next();
            }
            State::Normal if ch == '\'' => {
                flush(input, &mut word_start, index, &mut out);
                state = State::Single;
            }
            State::Normal if ch == '"' => {
                flush(input, &mut word_start, index, &mut out);
                state = State::Double;
            }
            State::Normal if ch == '`' => {
                flush(input, &mut word_start, index, &mut out);
                state = State::Backtick;
            }
            State::Normal if ch == '[' => {
                flush(input, &mut word_start, index, &mut out);
                state = State::Bracket;
            }
            State::Normal if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '#' | '$') => {
                word_start.get_or_insert(index);
            }
            State::Normal => flush(input, &mut word_start, index, &mut out),
            State::LineComment if ch == '\n' => state = State::Normal,
            State::BlockComment(depth) if ch == '/' && next == Some('*') => {
                state = State::BlockComment(depth.saturating_add(1));
                chars.next();
            }
            State::BlockComment(depth) if ch == '*' && next == Some('/') => {
                chars.next();
                state = if depth == 1 {
                    State::Normal
                } else {
                    State::BlockComment(depth - 1)
                };
            }
            State::Single if ch == '\\' => {
                chars.next();
            }
            State::Single if ch == '\'' => {
                if next == Some('\'') {
                    chars.next();
                } else {
                    state = State::Normal;
                }
            }
            State::Double if ch == '"' => {
                if next == Some('"') {
                    chars.next();
                } else {
                    state = State::Normal;
                }
            }
            State::Backtick if ch == '`' => {
                if next == Some('`') {
                    chars.next();
                } else {
                    state = State::Normal;
                }
            }
            State::Bracket if ch == ']' => {
                if next == Some(']') {
                    chars.next();
                } else {
                    state = State::Normal;
                }
            }
            _ => {}
        }
    }
    flush(input, &mut word_start, input.len(), &mut out);
    out
}

fn extract_wrapped_body<'a>(definition: &'a str, delimiter: &str) -> Option<&'a str> {
    let words = sql_words(definition);
    words.iter().enumerate().find_map(|(index, word)| {
        if !word.value.eq_ignore_ascii_case(delimiter) {
            return None;
        }
        if delimiter.eq_ignore_ascii_case("AS")
            && index
                .checked_sub(1)
                .and_then(|previous| words.get(previous))
                .is_some_and(|previous| previous.value.eq_ignore_ascii_case("EXECUTE"))
        {
            return None;
        }
        definition.get(word.end..).map(str::trim_start)
    })
}

fn is_wrapped_plsql(tokens: &[&str]) -> bool {
    let header = tokens.iter().take(12).copied().collect::<Vec<_>>();
    header
        .iter()
        .any(|token| matches!(*token, "PROCEDURE" | "FUNCTION" | "PACKAGE" | "TYPE"))
        && header.contains(&"WRAPPED")
}

fn normalize_dialect(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "sql" | "plpgsql" | "plpython" | "plperl" | "mysql-sql-psm" | "plsql" | "java" | "tsql"
        | "clr" | "c" | "internal" => value.trim().to_ascii_lowercase(),
        _ => "unknown".to_string(),
    }
}

fn lexical_v2_supports_dialect(dialect: &str) -> bool {
    matches!(
        dialect,
        "sql" | "plpgsql" | "mysql-sql-psm" | "plsql" | "tsql"
    )
}

fn normalize_switch(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "on" | "true" | "1" => "on".to_string(),
        "off" | "false" | "0" => "off".to_string(),
        "" => String::new(),
        _ => "unknown".to_string(),
    }
}

fn normalize_sql_modes(values: &[String]) -> Vec<String> {
    const ALLOWED: &[&str] = &[
        "ALLOW_INVALID_DATES",
        "ANSI",
        "ANSI_QUOTES",
        "ERROR_FOR_DIVISION_BY_ZERO",
        "HIGH_NOT_PRECEDENCE",
        "IGNORE_SPACE",
        "NO_AUTO_VALUE_ON_ZERO",
        "NO_BACKSLASH_ESCAPES",
        "NO_DIR_IN_CREATE",
        "NO_ENGINE_SUBSTITUTION",
        "NO_UNSIGNED_SUBTRACTION",
        "NO_ZERO_DATE",
        "NO_ZERO_IN_DATE",
        "ONLY_FULL_GROUP_BY",
        "PIPES_AS_CONCAT",
        "REAL_AS_FLOAT",
        "STRICT_ALL_TABLES",
        "STRICT_TRANS_TABLES",
        "TIME_TRUNCATE_FRACTIONAL",
    ];
    let allowed: BTreeSet<&str> = ALLOWED.iter().copied().collect();
    let mut out: Vec<String> = values
        .iter()
        .flat_map(|value| value.split(','))
        .map(|value| value.trim().to_ascii_uppercase())
        .filter(|value| allowed.contains(value.as_str()))
        .collect();
    out.sort();
    out.dedup();
    out
}

fn scrub_sql(input: &str, dialect: &str) -> Zeroizing<String> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum State {
        Normal,
        Single,
        Double,
        Backtick,
        Bracket,
        LineComment,
        BlockComment(u32),
    }
    let mysql_hash_comments = dialect.eq_ignore_ascii_case("mysql-sql-psm");
    let mut chars = input.chars().peekable();
    let mut state = State::Normal;
    let mut out = Zeroizing::new(String::with_capacity(input.len()));
    while let Some(c) = chars.next() {
        let next = chars.peek().copied();
        match state {
            State::Normal if c == '-' && next == Some('-') => {
                state = State::LineComment;
                out.push(' ');
                chars.next();
                continue;
            }
            State::Normal if c == '/' && next == Some('*') => {
                state = State::BlockComment(1);
                out.push(' ');
                chars.next();
                continue;
            }
            State::Normal if mysql_hash_comments && c == '#' => {
                state = State::LineComment;
                out.push(' ');
            }
            State::Normal if c == '\'' => {
                state = State::Single;
                out.push(' ');
            }
            State::Normal if c == '"' => {
                state = State::Double;
                out.push(' ');
            }
            State::Normal if c == '`' => {
                state = State::Backtick;
                out.push(' ');
            }
            State::Normal if c == '[' => {
                state = State::Bracket;
                out.push(' ');
            }
            State::LineComment if c == '\n' => {
                state = State::Normal;
                out.push('\n');
            }
            State::BlockComment(depth) if c == '/' && next == Some('*') => {
                state = State::BlockComment(depth.saturating_add(1));
                chars.next();
            }
            State::BlockComment(depth) if c == '*' && next == Some('/') => {
                chars.next();
                if depth == 1 {
                    state = State::Normal;
                    out.push(' ');
                } else {
                    state = State::BlockComment(depth - 1);
                }
            }
            State::Single if c == '\\' && mysql_hash_comments => {
                chars.next();
            }
            State::Single if c == '\'' => {
                if next == Some('\'') {
                    chars.next();
                } else {
                    state = State::Normal;
                    out.push(' ');
                }
            }
            State::Double if c == '"' => {
                if next == Some('"') {
                    chars.next();
                } else {
                    state = State::Normal;
                    out.push(' ');
                }
            }
            State::Backtick if c == '`' => {
                if next == Some('`') {
                    chars.next();
                } else {
                    state = State::Normal;
                    out.push(' ');
                }
            }
            State::Bracket if c == ']' => {
                if next == Some(']') {
                    chars.next();
                } else {
                    state = State::Normal;
                    out.push(' ');
                }
            }
            State::Normal => out.push(c),
            _ => {}
        }
    }
    out
}

fn sql_tokens(input: &str) -> Vec<&'static str> {
    let mut tokens = Vec::new();
    let mut current = Zeroizing::new(String::new());
    let flush = |current: &mut Zeroizing<String>, tokens: &mut Vec<&'static str>| {
        if !current.is_empty() {
            tokens.push(canonical_sql_token(current));
            current.zeroize();
        }
    };
    for c in input.chars() {
        if c.is_ascii_alphanumeric() || c == '_' || c == '#' {
            current.push(c.to_ascii_uppercase());
        } else {
            flush(&mut current, &mut tokens);
            if matches!(c, '(' | ')' | ';' | '%') {
                tokens.push(match c {
                    '(' => "(",
                    ')' => ")",
                    ';' => ";",
                    '%' => "%",
                    _ => unreachable!(),
                });
            }
        }
    }
    flush(&mut current, &mut tokens);

    let mut joined = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while i < tokens.len() {
        if i + 1 < tokens.len()
            && matches!(
                (tokens[i], tokens[i + 1]),
                ("GROUP", "BY") | ("ORDER", "BY") | ("EXECUTE", "AS")
            )
        {
            joined.push(match (tokens[i], tokens[i + 1]) {
                ("GROUP", "BY") => "GROUP_BY",
                ("ORDER", "BY") => "ORDER_BY",
                ("EXECUTE", "AS") => "EXECUTE_AS",
                _ => unreachable!(),
            });
            i += 2;
        } else {
            joined.push(tokens[i]);
            i += 1;
        }
    }
    joined
}

fn canonical_sql_token(token: &str) -> &'static str {
    const KEYWORDS: &[&str] = &[
        "#TEMP",
        "AFTER",
        "ALTER",
        "AS",
        "AT",
        "AVG",
        "BEFORE",
        "BEGIN",
        "BIT",
        "BLOB",
        "BODY",
        "BOOL",
        "BOOLEAN",
        "BY",
        "BYTEA",
        "CASE",
        "CATCH",
        "CLOB",
        "COMMIT",
        "COUNT",
        "CREATE",
        "CURSOR",
        "DATETIMEOFFSET",
        "DEFINER",
        "DELETE",
        "DROP",
        "ELSEIF",
        "ELSIF",
        "ENCRYPTION",
        "END",
        "EXCEPT",
        "EXCEPTION",
        "EXEC",
        "EXECUTE",
        "FETCH",
        "FUNCTION",
        "GEOGRAPHY",
        "GEOMETRY",
        "GOTO",
        "GROUP",
        "HANDLER",
        "IF",
        "INSERT",
        "INTERSECT",
        "INSTEAD",
        "INVOKER",
        "IMAGE",
        "INTERVAL",
        "JOIN",
        "JSON",
        "JSONB",
        "JSON_QUERY",
        "JSON_VALUE",
        "LIMIT",
        "LOCAL",
        "LOOP",
        "MAX",
        "MERGE",
        "MIN",
        "NATIVE_COMPILATION",
        "NCLOB",
        "NVARCHAR",
        "ORDER",
        "OR",
        "OVER",
        "PREPARE",
        "PROCEDURE",
        "PACKAGE",
        "RAISE",
        "RAISERROR",
        "REF",
        "REFCURSOR",
        "RESIGNAL",
        "RETURN",
        "ROLLBACK",
        "ROWTYPE",
        "SAVEPOINT",
        "RECURSIVE",
        "REPEAT",
        "SCHEMABINDING",
        "SELECT",
        "SP_EXECUTESQL",
        "START",
        "SUM",
        "SYS_REFCURSOR",
        "TEMP",
        "TEMPORARY",
        "TOP",
        "TRAN",
        "TRANSACTION",
        "TRIGGER",
        "TRUNCATE",
        "UNION",
        "UPDATE",
        "VARBINARY",
        "VARCHAR",
        "VECTOR",
        "WHEN",
        "WHILE",
        "WITH",
        "WRAPPED",
        "THROW",
        "SIGNAL",
        "TIME",
        "TIMESTAMP",
        "TIMESTAMPTZ",
        "TYPE",
        "ZONE",
        "XML",
    ];
    if let Some(keyword) = KEYWORDS.iter().copied().find(|keyword| *keyword == token) {
        keyword
    } else if token.starts_with("ST_") {
        "ST_"
    } else if token.bytes().all(|byte| byte.is_ascii_digit()) {
        "NUMBER"
    } else {
        "IDENT"
    }
}

fn maximum_nesting(dialect: &str, tokens: &[&str]) -> u64 {
    // Block nesting is spelled differently per dialect. Openers stay
    // per-dialect so
    // T-SQL's blockless IF (no END pairs with it) is never counted, and a
    // token directly after END is that END's qualifier (END IF, END LOOP,
    // END CASE, END WHILE, END REPEAT), never a new opener.
    let dialect_openers: &[&str] = match dialect {
        "plpgsql" | "plsql" => &["IF"],
        "mysql-sql-psm" => &["IF", "WHILE", "REPEAT"],
        _ => &[],
    };
    let mut depth = 0_u64;
    let mut maximum = 0_u64;
    let mut previous_end = false;
    for token in tokens {
        match *token {
            "(" => {
                depth += 1;
                maximum = maximum.max(depth);
            }
            ")" => depth = depth.saturating_sub(1),
            "BEGIN" | "CASE" | "LOOP" if !previous_end => {
                depth += 1;
                maximum = maximum.max(depth);
            }
            "END" => depth = depth.saturating_sub(1),
            other if !previous_end && dialect_openers.contains(&other) => {
                depth += 1;
                maximum = maximum.max(depth);
            }
            _ => {}
        }
        previous_end = *token == "END";
    }
    maximum
}

fn branch_count(tokens: &[&str]) -> u64 {
    let mut count = 0_u64;
    for (index, token) in tokens.iter().enumerate() {
        let previous = index.checked_sub(1).and_then(|i| tokens.get(i)).copied();
        match *token {
            "IF" if previous != Some("END") => count += 1,
            "ELSIF" | "ELSEIF" | "WHEN" | "WHILE" | "CATCH" | "HANDLER" => count += 1,
            _ => {}
        }
    }
    count
}

fn cte_count(tokens: &[&str]) -> u64 {
    tokens
        .iter()
        .enumerate()
        .filter(|(index, token)| {
            if **token != "WITH" {
                return false;
            }
            let mut cursor = index + 1;
            if tokens.get(cursor).copied() == Some("RECURSIVE") {
                cursor += 1;
            }
            if matches!(
                tokens.get(cursor).copied(),
                Some("EXECUTE_AS" | "SCHEMABINDING" | "ENCRYPTION" | "NATIVE_COMPILATION")
            ) {
                return false;
            }
            tokens[cursor..tokens.len().min(cursor + 10)]
                .windows(2)
                .any(|window| window[0] == "AS" && window[1] == "(")
        })
        .count() as u64
}

fn data_operation_count(tokens: &[&str], keyword: &str) -> u64 {
    tokens
        .iter()
        .enumerate()
        .filter(|(index, token)| **token == keyword && !is_trigger_event_keyword(tokens, *index))
        .count() as u64
}

fn is_trigger_event_keyword(tokens: &[&str], index: usize) -> bool {
    let previous = index
        .checked_sub(1)
        .and_then(|position| tokens.get(position))
        .copied();
    if matches!(previous, Some("BEFORE" | "AFTER" | "INSTEAD")) {
        return true;
    }
    if previous != Some("OR") {
        return false;
    }
    tokens[..index]
        .iter()
        .rev()
        .take_while(|token| !matches!(**token, ";" | "BEGIN"))
        .any(|token| *token == "TRIGGER")
}

fn dynamic_sql_count(tokens: &[&str]) -> u64 {
    tokens
        .iter()
        .enumerate()
        .filter(|(index, token)| match **token {
            "PREPARE" | "SP_EXECUTESQL" => true,
            "EXECUTE" => !matches!(
                tokens.get(index + 1).copied(),
                Some("FUNCTION" | "PROCEDURE")
            ),
            "EXEC" => tokens.get(index + 1).copied() == Some("("),
            _ => false,
        })
        .count() as u64
}

/// Record a partial optional-catalog result without retaining a driver error
/// that may expose source identifiers or SQL text in the operational audit.
pub fn record_catalog_unreadable(
    audit: &mut AuditLog,
    completeness: &mut CaptureCompleteness,
    catalog: &'static str,
    family: &'static str,
) {
    completeness.visibility = "privilege_filtered".to_string();
    completeness.inventory_complete = false;
    completeness.catalogs_unreadable.push(catalog.to_string());
    audit.record_warning(
        "DBP1410W",
        format!(
            "DBP1410W artifact family {family} is incomplete because catalog {catalog} could not be read"
        ),
    );
}

fn count_band(value: u64) -> String {
    match value {
        0 => "0",
        1 => "1",
        2..=4 => "2-4",
        5..=8 => "5-8",
        9..=16 => "9-16",
        17..=32 => "17-32",
        _ => "33+",
    }
    .to_string()
}

fn byte_size_band(value: u64) -> String {
    match value {
        0 => "0",
        1..=255 => "1-255",
        256..=1023 => "256-1k",
        1024..=4095 => "1k-4k",
        4096..=16383 => "4k-16k",
        16384..=65535 => "16k-64k",
        _ => "64k+",
    }
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::reference_assessor::assess_complexity;
    use super::*;

    fn test_audit() -> AuditLog {
        AuditLog::new("artifact-test", 0)
    }

    fn assessed_artifact(
        kind: &'static str,
        statements: &str,
    ) -> dbwarp_blueprint_core::BlueprintArtifact {
        let mut item = RawArtifact::new(format!("t|{kind}|s|{statements}"), kind, "ordinary");
        item.definition_visibility = "available";
        item.requirement_status = RequirementStatus::Complete;
        item.analysis = Some(RawLanguageAnalysis {
            dialect: "plpgsql".to_string(),
            definition: Some(statements.to_string().into()),
            grammar_profile: "postgresql-17.0".to_string(),
            ..RawLanguageAnalysis::default()
        });
        let inventory = build_inventory(
            ArtifactDetail::Analyzed,
            vec![item],
            &BTreeMap::new(),
            &BTreeMap::new(),
            &crate::schema_scope::SchemaSelection::default(),
            CaptureCompleteness {
                visibility: "full".to_string(),
                inventory_complete: true,
                ..CaptureCompleteness::default()
            },
            &mut test_audit(),
        )
        .expect("build assessed inventory");
        inventory.artifacts.into_values().next().expect("artifact")
    }

    #[test]
    fn artifact_only_schemas_receive_anonymous_ids_and_missing_maps_fail_closed() {
        let mut item = RawArtifact::new(
            "postgresql|sequence|artifact_only|s1",
            "sequence",
            "ordinary",
        );
        item.schema_identity = Some("artifact_only".to_string());
        item.definition_visibility = "not_applicable";
        item.requirement_status = RequirementStatus::NotApplicable;

        let schema_ids = schema_ids_for_capture(
            std::iter::empty(),
            std::slice::from_ref(&item),
            ArtifactDetail::Graph,
        );
        assert_eq!(schema_ids.len(), 1);
        assert!(schema_ids_for_capture(
            std::iter::empty(),
            std::slice::from_ref(&item),
            ArtifactDetail::Summary,
        )
        .is_empty());
        let inventory = build_inventory(
            ArtifactDetail::Graph,
            vec![item.clone()],
            &schema_ids,
            &BTreeMap::new(),
            &crate::schema_scope::SchemaSelection::default(),
            CaptureCompleteness {
                visibility: "full".to_string(),
                inventory_complete: true,
                dependencies_complete: true,
                requirements_complete: true,
                ..CaptureCompleteness::default()
            },
            &mut test_audit(),
        )
        .expect("artifact-only schema must retain an anonymous association");
        assert!(inventory
            .artifacts
            .values()
            .all(|artifact| artifact.schema.starts_with("schema-")));

        let error = build_inventory(
            ArtifactDetail::Graph,
            vec![item],
            &BTreeMap::new(),
            &BTreeMap::new(),
            &crate::schema_scope::SchemaSelection::default(),
            CaptureCompleteness::default(),
            &mut test_audit(),
        )
        .expect_err("a schema-scoped artifact must never lose its schema silently");
        assert!(error
            .to_string()
            .contains("missing its anonymous schema mapping"));
    }

    #[test]
    fn assessor_empty_complete_population_is_not_applicable() {
        let complexity =
            assess_complexity(ArtifactDetail::Analyzed, &BTreeMap::new(), true, true, true);
        assert_eq!(complexity.overall_band, "not-applicable");
        assert!(complexity.assessment_population_complete);
        assert_eq!(complexity.eligible_object_count, 0);
        // An empty incomplete population is unknown instead: absence that
        // cannot be proven is not a determination.
        let incomplete = assess_complexity(
            ArtifactDetail::Analyzed,
            &BTreeMap::new(),
            false,
            true,
            true,
        );
        assert_eq!(incomplete.overall_band, "unknown");
    }

    #[test]
    fn incomplete_dependency_evidence_makes_entanglement_unknown() {
        let artifact = assessed_artifact("function", "BEGIN x := 1; END");
        let artifacts = BTreeMap::from([("function-001".to_string(), artifact)]);
        let complexity = assess_complexity(ArtifactDetail::Analyzed, &artifacts, true, false, true);
        // Missing graph edges are not ordinary low entanglement.
        assert_eq!(complexity.dimensions.entanglement.coverage, "unknown");
        assert_eq!(complexity.dimensions.entanglement.histogram.unknown, 1);
        assert_eq!(complexity.partially_assessed_object_count, 1);
        assert_eq!(complexity.overall_band, "unknown");
        assert!(complexity
            .limitations
            .contains(&"graph-incomplete".to_string()));
    }

    #[test]
    fn reference_assessor_counts_not_applicable_dimensions_as_assessed() {
        let artifact = dbwarp_blueprint_core::BlueprintArtifact {
            kind: "sequence".to_string(),
            definition_visibility: "not_applicable".to_string(),
            requirement_status: "not_applicable".to_string(),
            ..Default::default()
        };
        let artifacts = BTreeMap::from([("sequence-001".to_string(), artifact)]);

        let complexity = assess_complexity(ArtifactDetail::Analyzed, &artifacts, true, false, true);

        assert_eq!(complexity.fully_assessed_object_count, 0);
        assert_eq!(complexity.partially_assessed_object_count, 1);
        assert_eq!(complexity.unassessed_object_count, 0);
    }

    #[test]
    fn assessor_fully_assessed_complete_estate_earns_a_definitive_band() {
        let artifact = assessed_artifact("function", "BEGIN x := 1; END");
        let artifacts = BTreeMap::from([("function-001".to_string(), artifact)]);
        let complexity = assess_complexity(ArtifactDetail::Analyzed, &artifacts, true, true, true);
        assert_eq!(complexity.fully_assessed_object_count, 1);
        assert!(matches!(
            complexity.overall_band.as_str(),
            "trivial" | "low" | "moderate" | "high"
        ));
        assert!(complexity.overall_score.is_none());
    }

    #[test]
    fn assessor_unknowns_force_unknown_unless_already_maximal() {
        // One assessed trivial object plus one whose census is unavailable:
        // the bounded rule's bounds diverge, so the answer is unknown.
        let assessed = assessed_artifact("function", "BEGIN x := 1; END");
        let opaque = dbwarp_blueprint_core::BlueprintArtifact {
            kind: "procedure".to_string(),
            subkind: "ordinary".to_string(),
            definition_visibility: "encrypted".to_string(),
            ..Default::default()
        };
        let artifacts = BTreeMap::from([
            ("function-001".to_string(), assessed),
            ("procedure-001".to_string(), opaque),
        ]);
        let complexity = assess_complexity(ArtifactDetail::Analyzed, &artifacts, true, true, true);
        assert_eq!(complexity.partially_assessed_object_count, 1);
        assert_eq!(complexity.overall_band, "unknown");
        assert!(complexity.dimensions.opacity.histogram.unknown >= 1);
    }

    #[test]
    fn assessor_graph_detail_is_unknown_with_graph_dimensions_kept() {
        let item = dbwarp_blueprint_core::BlueprintArtifact {
            kind: "view".to_string(),
            definition_visibility: "not_read".to_string(),
            relationships: vec![dbwarp_blueprint_core::ArtifactRelationship {
                kind: "references-table".to_string(),
                target: "table-001".to_string(),
                evidence: "dependency-confirmed".to_string(),
            }],
            ..Default::default()
        };
        let artifacts = BTreeMap::from([("view-001".to_string(), item)]);
        let complexity = assess_complexity(ArtifactDetail::Graph, &artifacts, true, true, true);
        assert_eq!(complexity.overall_band, "unknown");
        assert_eq!(complexity.analyzer_version, "not-applicable");
        assert!(complexity
            .limitations
            .contains(&"definition-analysis-not-requested".to_string()));
        assert_eq!(complexity.dimensions.entanglement.coverage, "complete");
        assert_eq!(complexity.dimensions.volume.coverage, "unknown");
    }

    #[test]
    fn assessed_inventory_survives_the_validating_serializer() {
        let mut item = RawArtifact::new("postgresql|view|app|v1|10", "view", "ordinary");
        item.schema_identity = Some("app".to_string());
        item.definition_visibility = "available";
        item.analysis = Some(RawLanguageAnalysis {
            dialect: "sql".to_string(),
            definition: Some("SELECT 1".to_string().into()),
            grammar_profile: "postgresql-17.0".to_string(),
            ..RawLanguageAnalysis::default()
        });
        let inventory = build_inventory(
            ArtifactDetail::Analyzed,
            vec![item],
            &BTreeMap::from([("app".to_string(), "schema-A".to_string())]),
            &BTreeMap::new(),
            &crate::schema_scope::SchemaSelection::new(["app".to_string()]),
            CaptureCompleteness {
                visibility: "full".to_string(),
                inventory_complete: true,
                ..CaptureCompleteness::default()
            },
            &mut test_audit(),
        )
        .expect("build assessed inventory");
        assert!(inventory.complexity.is_some());
        let mut blueprint = dbwarp_blueprint_core::BlueprintFile {
            schema_version: dbwarp_blueprint_core::SCHEMA_VERSION,
            engine: "postgresql".into(),
            engine_version: "17".into(),
            source_kind: "production".into(),
            totals: dbwarp_blueprint_core::Totals {
                table_count: 1,
                row_count: 7,
                table_bytes: 70,
                index_bytes: 14,
            },
            database_topology: Some(dbwarp_blueprint_core::DatabaseTopology::unknown()),
            dataset_scope: Some(dbwarp_blueprint_core::DatasetScope::unknown_database(
                "postgres-planner-estimate",
                "postgres-local-relation-size",
            )),
            tables: BTreeMap::from([(
                "table-001".to_string(),
                dbwarp_blueprint_core::BlueprintTable {
                    schema: "schema-A".to_string(),
                    rows: 7,
                    table_bytes: 70,
                    index_bytes: 14,
                    ..Default::default()
                },
            )]),
            ..Default::default()
        };
        blueprint.artifact_inventory = Some(inventory);
        blueprint.initialize_v7_database_contract();
        // The serializer runs the full contract validator, complexity
        // arithmetic included; surviving it is the acceptance.
        let toml = dbwarp_blueprint_core::blueprint_to_toml(&blueprint)
            .expect("assessed inventory must satisfy the contract validator");
        assert!(toml.contains("dbwarp-blueprint-artifact-complexity/v1"));
    }

    #[test]
    fn nesting_counts_block_syntax_per_dialect() {
        fn census(dialect: &str, body: &str) -> LanguageFeatureCensus {
            analyze_language(&RawLanguageAnalysis {
                dialect: dialect.to_string(),
                definition: Some(body.to_string().into()),
                ..RawLanguageAnalysis::default()
            })
        }
        // Three IF levels inside the routine body: depth 4 with the BEGIN.
        let plpgsql = census(
            "plpgsql",
            "BEGIN IF a THEN IF b THEN IF c THEN x := 1; END IF; END IF; \
             END IF; END",
        );
        assert_eq!(plpgsql.maximum_nesting_band, "2-4");
        // END IF must not eat the function-body depth: after the inner
        // blocks close, one more statement still sits inside BEGIN.
        let mysql = census(
            "mysql-sql-psm",
            "BEGIN WHILE a DO SET x = 1; END WHILE; REPEAT SET y = 2; \
             UNTIL b END REPEAT; END",
        );
        assert_eq!(mysql.maximum_nesting_band, "2-4");
        // T-SQL IF opens no block, so only BEGIN/END pairs count; the same
        // token stream must not inflate.
        let tsql = census(
            "tsql",
            "BEGIN IF @a = 1 BEGIN SET @x = 1; END IF @b = 2 BEGIN SET \
             @y = 2; END END",
        );
        assert_eq!(tsql.maximum_nesting_band, "2-4");
        // Deep block-IF nesting must reach the deep bands on every dialect
        // that spells blocks that way - the defect measured these as flat.
        let mut body = String::from("x := 1;");
        for _ in 0..24 {
            body = format!("IF a THEN {body} END IF;");
        }
        let deep = census("plpgsql", &format!("BEGIN {body} END"));
        assert_eq!(deep.maximum_nesting_band, "17-32");
    }

    #[test]
    fn unresolved_targets_are_classified_by_schema_membership() {
        let (schemas, tables) = test_maps();
        let mut item = RawArtifact::new("postgresql|view|private_app|v1|11", "view", "ordinary");
        item.schema_identity = Some("private_app".to_string());
        // Outside the selection: the schema segment is not a selected schema.
        item.dependencies
            .push(table_identity("postgresql", "other_app", "orders"));
        // Inside the selection but not in the inventory: not visible.
        item.dependencies
            .push(table_identity("postgresql", "private_app", "missing"));
        // Selected, but the hidden target has no emitted table or artifact,
        // so its schema is absent from schema_ids. Classifying against that
        // map instead of the selection branded an in-scope hidden target as
        // outside the operator's own scope; this dependency pins the
        // distinction.
        item.dependencies
            .push(table_identity("postgresql", "views_only", "missing"));
        // Classified by the engine at the catalog row; must merge, not vanish.
        item.unresolved_reasons
            .insert("cross-database".to_string(), 2);
        let inventory = build_inventory(
            ArtifactDetail::Graph,
            vec![item],
            &schemas,
            &tables,
            &crate::schema_scope::SchemaSelection::new([
                "private_app".to_string(),
                "views_only".to_string(),
            ]),
            CaptureCompleteness::default(),
            &mut test_audit(),
        )
        .expect("build graph inventory");
        let artifact = inventory.artifacts.values().next().expect("artifact");
        assert_eq!(
            artifact.unresolved_relationships,
            BTreeMap::from([
                ("cross-database".to_string(), 2),
                ("outside-selected-schema".to_string(), 1),
                ("target-not-visible".to_string(), 2),
            ])
        );
        // A narrow scope and an invisible estate are different findings; the
        // old single-token collapse reported both as target-not-visible.
        assert_eq!(artifact.unresolved_dependency_count, 0);
    }

    fn test_maps() -> (BTreeMap<String, String>, BTreeMap<String, String>) {
        let schemas = BTreeMap::from([("private_app".to_string(), "schema-A".to_string())]);
        let tables = BTreeMap::from([(
            table_identity("postgresql", "private_app", "customers"),
            "table-001".to_string(),
        )]);
        (schemas, tables)
    }

    #[test]
    fn graph_is_anonymous_deterministic_and_keeps_external_prerequisites() {
        let (schemas, tables) = test_maps();
        let mut function = RawArtifact::new(
            "postgresql|function|private_app|secret_fn(integer)",
            "function",
            "stored_function",
        );
        function.schema_identity = Some("private_app".to_string());
        function
            .dependencies
            .push(table_identity("postgresql", "private_app", "customers"));
        function.definition_visibility = "available";
        function.requirements.extend([
            RawArtifactRequirement {
                token: "postgresql.function.security-definer",
                evidence: "catalog-confirmed",
                count_band: "1",
            },
            // Providers may combine overlapping bounded catalog reads. The
            // serializer emits one canonical fact rather than two copies.
            RawArtifactRequirement {
                token: "postgresql.function.security-definer",
                evidence: "catalog-confirmed",
                count_band: "1",
            },
            RawArtifactRequirement {
                token: "external.procedural-language-runtime",
                evidence: "catalog-confirmed",
                count_band: "1",
            },
        ]);
        function.analysis = Some(RawLanguageAnalysis {
            definition: Some(Zeroizing::new(
                "BEGIN IF secret_value > 0 THEN SELECT 'secret'; END IF; END".into(),
            )),
            dialect: "plpgsql".into(),
            grammar_profile: "postgresql-18".into(),
            ..RawLanguageAnalysis::default()
        });

        let mut extension = RawArtifact::new(
            "postgresql|extension|private_extension_name",
            "extension",
            "server_extension",
        );
        extension.external = Some(RawExternalPrerequisite::package(
            "postgresql_extension",
            "target_compatible_package",
        ));

        let inventory = build_inventory(
            ArtifactDetail::Analyzed,
            vec![extension, function],
            &schemas,
            &tables,
            &crate::schema_scope::SchemaSelection::default(),
            CaptureCompleteness {
                visibility: "full".into(),
                inventory_complete: true,
                catalogs_read: vec!["pg_proc".into(), "pg_extension".into()],
                ..CaptureCompleteness::default()
            },
            &mut test_audit(),
        )
        .expect("build analyzed inventory");
        let encoded = toml::to_string(&inventory).unwrap();
        assert!(!inventory.requirements_complete);
        assert!(inventory
            .artifacts
            .values()
            .all(|artifact| artifact.requirement_status == "partial"));
        assert_eq!(inventory.external_prerequisite_count, 1);
        assert_eq!(inventory.counts_by_kind["function"], 1);
        let function = inventory
            .artifacts
            .values()
            .find(|artifact| artifact.kind == "function")
            .unwrap();
        assert_eq!(function.relationships.len(), 1);
        assert_eq!(function.relationships[0].kind, "references-table");
        assert_eq!(function.relationships[0].target, "table-001");
        assert_eq!(function.requirements.len(), 2);
        assert_eq!(
            function
                .requirements
                .iter()
                .map(|requirement| requirement.token.as_str())
                .collect::<Vec<_>>(),
            [
                "external.procedural-language-runtime",
                "postgresql.function.security-definer",
            ]
        );
        assert_eq!(function.requirements[0].evidence, "catalog-confirmed");
        assert_eq!(function.requirements[0].count_band, "1");
        assert!(encoded.contains("schema-A"));
        assert!(encoded.contains("table-001"));
        assert!(!encoded.contains("private_app"));
        assert!(!encoded.contains("customers"));
        assert!(!encoded.contains("secret_fn"));
        assert!(!encoded.contains("secret_value"));
        assert!(!encoded.contains("private_extension_name"));
        assert!(!inventory.analysis_complete);
    }

    #[test]
    fn summary_has_counts_but_no_fingerprintable_graph() {
        let raw = vec![RawArtifact::new("mysql|view|app|v1", "view", "ordinary")];
        let inventory = build_inventory(
            ArtifactDetail::Summary,
            raw,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &crate::schema_scope::SchemaSelection::default(),
            CaptureCompleteness::default(),
            &mut test_audit(),
        )
        .expect("build summary inventory");
        assert_eq!(inventory.object_count, 1);
        assert_eq!(inventory.counts_by_kind["view"], 1);
        assert!(inventory.artifacts.is_empty());
    }

    #[test]
    fn per_object_requirement_status_preserves_partial_coverage() {
        let mut complete = RawArtifact::new("pg|function|one", "function", "ordinary");
        complete.requirement_status = RequirementStatus::Complete;
        let mut not_applicable = RawArtifact::new("pg|sequence|two", "sequence", "ordinary");
        not_applicable.requirement_status = RequirementStatus::NotApplicable;
        let qualified = build_inventory(
            ArtifactDetail::Graph,
            vec![complete.clone(), not_applicable],
            &BTreeMap::new(),
            &BTreeMap::new(),
            &crate::schema_scope::SchemaSelection::default(),
            CaptureCompleteness {
                visibility: "full".into(),
                inventory_complete: true,
                dependencies_complete: true,
                requirements_complete: true,
                ..CaptureCompleteness::default()
            },
            &mut test_audit(),
        )
        .expect("build qualified requirement inventory");
        assert!(qualified.requirements_complete);
        let qualified_complexity = qualified.complexity.as_ref().unwrap();
        assert_eq!(
            qualified_complexity
                .dimensions
                .environment_coupling
                .coverage,
            "complete"
        );
        assert_eq!(
            qualified_complexity
                .dimensions
                .environment_coupling
                .histogram
                .not_applicable,
            1
        );

        let mut partial = RawArtifact::new("pg|function|three", "function", "ordinary");
        partial.requirement_status = RequirementStatus::Partial;
        let mixed = build_inventory(
            ArtifactDetail::Graph,
            vec![complete, partial],
            &BTreeMap::new(),
            &BTreeMap::new(),
            &crate::schema_scope::SchemaSelection::default(),
            CaptureCompleteness {
                visibility: "full".into(),
                inventory_complete: true,
                dependencies_complete: true,
                requirements_complete: true,
                ..CaptureCompleteness::default()
            },
            &mut test_audit(),
        )
        .expect("build partially qualified requirement inventory");
        assert!(!mixed.requirements_complete);
        let mixed_complexity = mixed.complexity.as_ref().unwrap();
        assert_eq!(
            mixed_complexity.dimensions.environment_coupling.coverage,
            "partial"
        );
        assert_eq!(
            mixed_complexity
                .dimensions
                .environment_coupling
                .histogram
                .unknown,
            1
        );
        assert!(mixed_complexity
            .limitations
            .contains(&"requirements-incomplete".to_string()));
    }

    #[test]
    fn graph_preserves_artifact_parentage_and_population_flags() {
        let spec_identity = "oracle|package|app|payments|spec";
        let spec = RawArtifact::new(spec_identity, "package", "specification");
        let mut member = RawArtifact::new(
            "oracle|procedure|app|payments|settle",
            "procedure",
            "package_member",
        );
        member.parent_artifact_identity = Some(spec_identity.to_string());
        member.generated_by_engine = Some(false);
        member.temporary = Some(false);
        member.editioned = Some(true);
        member.secondary_object = Some(false);
        member.validity = "valid";
        member.enabled = Some(true);

        let inventory = build_inventory(
            ArtifactDetail::Graph,
            vec![member, spec],
            &BTreeMap::new(),
            &BTreeMap::new(),
            &crate::schema_scope::SchemaSelection::default(),
            CaptureCompleteness::default(),
            &mut test_audit(),
        )
        .expect("build parented inventory");
        let spec_id = inventory
            .artifacts
            .iter()
            .find_map(|(id, artifact)| (artifact.kind == "package").then_some(id))
            .unwrap();
        let member = inventory
            .artifacts
            .values()
            .find(|artifact| artifact.subkind == "package_member")
            .unwrap();

        assert_eq!(&member.parent, spec_id);
        assert_eq!(member.generated_by_engine, Some(false));
        assert_eq!(member.temporary, Some(false));
        assert_eq!(member.editioned, Some(true));
        assert_eq!(member.secondary_object, Some(false));
        assert_eq!(member.validity, "valid");
        assert_eq!(member.enabled, Some(true));
    }

    #[test]
    fn strings_and_comments_do_not_inflate_language_features() {
        let raw = RawLanguageAnalysis {
            definition: Some(Zeroizing::new(
                "SELECT 'JOIN DELETE WHILE'; -- INSERT LOOP\nFROM t WHERE id = 1".into(),
            )),
            dialect: "sql".into(),
            grammar_profile: "postgresql-18".into(),
            ..RawLanguageAnalysis::default()
        };
        let analysis = analyze_language(&raw);
        assert!(!analysis.features.contains_key("query.join"));
        assert!(!analysis.features.contains_key("data.delete"));
        assert!(!analysis.features.contains_key("data.insert"));
        assert!(!analysis.features.contains_key("control.while"));
        assert_eq!(analysis.features["data.select"], "1");
    }

    #[test]
    fn unsupported_dialects_are_named_but_not_lexically_misclassified() {
        for dialect in ["plpython", "plperl", "java", "clr", "c", "internal"] {
            let raw = RawLanguageAnalysis {
                definition: Some(Zeroizing::new(
                    "SELECT LOOP DELETE implementation_specific_text".into(),
                )),
                dialect: dialect.into(),
                grammar_profile: "postgresql-18".into(),
                ..RawLanguageAnalysis::default()
            };
            let analysis = analyze_language(&raw);
            assert_eq!(analysis.status, "unavailable");
            assert_eq!(analysis.dialect, dialect);
            assert!(analysis.features.is_empty());
            assert!(analysis.definition_size_band.is_empty());
        }
    }

    #[test]
    fn trigger_events_and_execute_function_are_not_body_operations() {
        let raw = RawLanguageAnalysis {
            definition: Some(Zeroizing::new(
                "CREATE TRIGGER t BEFORE INSERT OR UPDATE OR DELETE ON x FOR EACH ROW EXECUTE FUNCTION f();".into(),
            )),
            dialect: "sql".into(),
            grammar_profile: "postgresql-18".into(),
            ..RawLanguageAnalysis::default()
        };
        let analysis = analyze_language(&raw);
        assert!(!analysis.features.contains_key("data.insert"));
        assert!(!analysis.features.contains_key("data.update"));
        assert!(!analysis.features.contains_key("data.delete"));
        assert!(!analysis.features.contains_key("dynamic.sql"));
        assert_eq!(analysis.opaque_region_count_band, "0");
    }

    #[test]
    fn sqlserver_module_options_are_not_ctes() {
        let raw = RawLanguageAnalysis {
            definition: Some(Zeroizing::new(
                "CREATE PROCEDURE p WITH EXECUTE AS CALLER AS BEGIN SELECT 1; END;".into(),
            )),
            definition_span: RawDefinitionSpan::SqlServerModule,
            dialect: "tsql".into(),
            grammar_profile: "sqlserver-16".into(),
            ..RawLanguageAnalysis::default()
        };
        let analysis = analyze_language(&raw);
        assert!(!analysis.features.contains_key("query.cte"));
        assert!(!analysis.features.contains_key("security.impersonation"));
        assert!(!analysis.features.contains_key("state.ddl"));
        assert_eq!(analysis.analysis_span, "executable-body");
    }

    #[test]
    fn real_cte_dynamic_sql_and_trigger_body_mutation_are_detected() {
        let raw = RawLanguageAnalysis {
            definition: Some(Zeroizing::new(
                "CREATE TRIGGER t AFTER INSERT ON x AS BEGIN INSERT INTO audit SELECT * FROM inserted; WITH cte AS (SELECT 1 AS n) SELECT n FROM cte; EXECUTE command_text; END;".into(),
            )),
            definition_span: RawDefinitionSpan::SqlServerModule,
            dialect: "tsql".into(),
            grammar_profile: "sqlserver-16".into(),
            ..RawLanguageAnalysis::default()
        };
        let analysis = analyze_language(&raw);
        assert_eq!(analysis.features["data.insert"], "1");
        assert_eq!(analysis.features["query.cte"], "1");
        assert_eq!(analysis.features["dynamic.sql"], "1");
        assert_eq!(analysis.opaque_region_count_band, "1");
    }

    #[test]
    fn lexical_v2_detects_the_locked_cross_engine_semantics() {
        let raw = RawLanguageAnalysis {
            definition: Some(Zeroizing::new(
                r#"
                DECLARE
                    amount orders.amount%TYPE;
                    row_copy orders%ROWTYPE;
                    result_cursor SYS_REFCURSOR;
                    delay INTERVAL DAY TO SECOND;
                    payload BLOB;
                    observed_at TIMESTAMP WITH TIME ZONE;
                    enabled BOOLEAN;
                BEGIN
                    GOTO retry_work;
                    RAISE application_error;
                    COMMIT;
                END;
                "#
                .into(),
            )),
            dialect: "plsql".into(),
            grammar_profile: "oracle-19".into(),
            ..RawLanguageAnalysis::default()
        };
        let analysis = analyze_language(&raw);

        for token in [
            "binding.percent_type",
            "binding.percent_rowtype",
            "interface.ref_cursor",
            "control.goto",
            "control.raise",
            "transaction.control",
            "type.interval",
            "type.timezone",
            "type.boolean",
            "type.lob",
        ] {
            assert_eq!(analysis.features[token], "1", "missing {token}");
        }
        assert_eq!(analysis.analyzer_version, "lexical-v2");
        assert_eq!(analysis.analysis_span, "executable-body");
    }

    #[test]
    fn shared_analyzer_refuses_wrapped_plsql_even_if_a_provider_passes_it() {
        let raw = RawLanguageAnalysis {
            definition: Some(Zeroizing::new(
                "CREATE OR REPLACE PACKAGE BODY customer_package WRAPPED a000000 1f abcd".into(),
            )),
            dialect: "plsql".into(),
            grammar_profile: "oracle-19".into(),
            ..RawLanguageAnalysis::default()
        };
        let analysis = analyze_language(&raw);

        assert_eq!(analysis.status, "unavailable");
        assert_eq!(analysis.analysis_span, "unknown");
        assert!(analysis.features.is_empty());
        assert!(analysis.definition_size_band.is_empty());

        let provider_detected = analyze_language(&RawLanguageAnalysis {
            definition: None,
            definition_span: RawDefinitionSpan::Unknown,
            dialect: "plsql".into(),
            grammar_profile: "oracle-19".into(),
            ..RawLanguageAnalysis::default()
        });
        assert_eq!(provider_detected.status, "unavailable");
        assert_eq!(provider_detected.analysis_span, "unknown");
    }

    #[test]
    fn wrapper_extraction_ignores_comments_literals_and_execute_as() {
        let raw = RawLanguageAnalysis {
            definition: Some(Zeroizing::new(
                "CREATE PROCEDURE p WITH EXECUTE AS OWNER /* AS DROP */ AS BEGIN SELECT 'AS DROP'; RETURN; END;".into(),
            )),
            definition_span: RawDefinitionSpan::SqlServerModule,
            dialect: "tsql".into(),
            grammar_profile: "sqlserver-16".into(),
            ..RawLanguageAnalysis::default()
        };
        let analysis = analyze_language(&raw);

        assert_eq!(analysis.status, "partial");
        assert_eq!(analysis.features["data.select"], "1");
        assert!(!analysis.features.contains_key("state.ddl"));
        assert!(!analysis.features.contains_key("security.impersonation"));
    }

    #[test]
    fn lexical_tokens_never_retain_source_identifiers_or_literals() {
        let scrubbed = scrub_sql(
            "SELECT private_customer_ledger, ST_SECRET(private_geometry) FROM [private table] WHERE tenant_private = 93847 AND note = 'classified';",
            "tsql",
        );
        let tokens = sql_tokens(&scrubbed);

        assert!(tokens.contains(&"SELECT"));
        assert!(tokens.contains(&"ST_"));
        assert!(tokens.contains(&"NUMBER"));
        assert!(tokens.contains(&"IDENT"));
        assert!(!tokens.iter().any(|token| token.contains("PRIVATE")));
        assert!(!tokens.iter().any(|token| token.contains("CLASSIFIED")));
    }

    #[test]
    fn scrubber_handles_nested_and_engine_specific_comments_and_escaped_identifiers() {
        let postgres = scrub_sql(
            "SELECT \"JOIN\"\"hidden\"; /* DELETE /* UPDATE */ INSERT */ SELECT 1;",
            "plpgsql",
        );
        let postgres_tokens = sql_tokens(&postgres);
        assert_eq!(
            postgres_tokens
                .iter()
                .filter(|token| **token == "SELECT")
                .count(),
            2
        );
        assert!(!postgres_tokens.contains(&"JOIN"));
        assert!(!postgres_tokens.contains(&"DELETE"));
        assert!(!postgres_tokens.contains(&"UPDATE"));
        assert!(!postgres_tokens.contains(&"INSERT"));

        let mysql = scrub_sql(
            "SELECT 1; # DELETE private_name\nSELECT `UPDATE``x`;",
            "mysql-sql-psm",
        );
        let mysql_tokens = sql_tokens(&mysql);
        assert_eq!(
            mysql_tokens
                .iter()
                .filter(|token| **token == "SELECT")
                .count(),
            2
        );
        assert!(!mysql_tokens.contains(&"DELETE"));
        assert!(!mysql_tokens.contains(&"UPDATE"));
    }

    #[test]
    fn analyzed_inventory_without_an_analysis_is_not_vacuously_complete() {
        let inventory = build_inventory(
            ArtifactDetail::Analyzed,
            vec![RawArtifact::new(
                "postgresql|extension|private_extension",
                "extension",
                "server_extension",
            )],
            &BTreeMap::new(),
            &BTreeMap::new(),
            &crate::schema_scope::SchemaSelection::default(),
            CaptureCompleteness::default(),
            &mut test_audit(),
        )
        .expect("build incomplete analyzed inventory");
        assert!(!inventory.analysis_complete);
    }

    #[test]
    fn complexity_calculation_failure_is_visible_and_retains_the_inventory() {
        let item = RawArtifact::new("postgresql|function|app|f", "function", "ordinary");
        let mut audit = test_audit();
        let inventory = build_inventory_with_assessor(
            InventoryBuildInputs {
                detail: ArtifactDetail::Graph,
                raw: vec![item],
                schema_ids: &BTreeMap::new(),
                table_ids: &BTreeMap::new(),
                selection: &crate::schema_scope::SchemaSelection::default(),
                completeness: CaptureCompleteness {
                    assessment_population_complete: Some(true),
                    ..CaptureCompleteness::default()
                },
                audit: &mut audit,
            },
            |_inventory, _population_complete| {
                anyhow::bail!("injected assessor failure after valid inventory construction")
            },
        )
        .expect("retain inventory after complexity failure");

        assert_eq!(inventory.object_count, 1);
        let complexity = inventory
            .complexity
            .as_ref()
            .expect("fail-closed complexity");
        assert_eq!(complexity.unassessed_object_count, 1);
        assert!(complexity
            .limitations
            .contains(&"computation-failed".to_string()));
        assert_eq!(complexity.dimensions.entanglement.histogram.unknown, 1);
        assert_eq!(
            audit.warnings,
            ["DBP1422W artifact complexity assessment failed; the inventory was retained and every affected aggregate dimension was marked unknown"]
        );

        let mut blueprint = dbwarp_blueprint_core::BlueprintFile {
            schema_version: dbwarp_blueprint_core::SCHEMA_VERSION,
            engine: "postgresql".into(),
            engine_version: "17".into(),
            source_kind: "production".into(),
            database_topology: Some(dbwarp_blueprint_core::DatabaseTopology::unknown()),
            dataset_scope: Some(dbwarp_blueprint_core::DatasetScope::unknown_database(
                "postgres-planner-estimate",
                "postgres-local-relation-size",
            )),
            artifact_inventory: Some(inventory),
            ..Default::default()
        };
        blueprint.initialize_v7_database_contract();
        dbwarp_blueprint_core::blueprint_to_toml(&blueprint)
            .expect("fail-closed complexity must survive the validating serializer");
    }

    #[test]
    fn catalog_warning_contains_only_closed_catalog_labels() {
        let mut audit = AuditLog::new("tier-1", 0);
        let mut completeness = CaptureCompleteness::default();
        record_catalog_unreadable(&mut audit, &mut completeness, "pg_proc", "routines");

        assert_eq!(completeness.visibility, "privilege_filtered");
        assert!(!completeness.inventory_complete);
        assert_eq!(completeness.catalogs_unreadable, ["pg_proc"]);
        assert_eq!(
            audit.warnings,
            ["DBP1410W artifact family routines is incomplete because catalog pg_proc could not be read"]
        );
    }

    #[test]
    fn qualified_catalog_requirements_prove_empty_objects_but_fail_closed_on_catalog_loss() {
        let mut with_fact = RawArtifact::new("pg|function|one", "function", "ordinary");
        with_fact.requirements.push(RawArtifactRequirement::catalog(
            "postgresql.function.security-definer",
        ));
        let empty = RawArtifact::new("pg|sequence|two", "sequence", "integer_sequence");
        let mut raw = vec![with_fact, empty];
        let mut completeness = CaptureCompleteness {
            visibility: "full".into(),
            inventory_complete: true,
            ..CaptureCompleteness::default()
        };

        qualify_catalog_requirement_coverage(&mut raw, &mut completeness);
        assert!(completeness.requirements_complete);
        assert!(raw
            .iter()
            .all(|artifact| artifact.requirement_status == RequirementStatus::Complete));

        let qualified = build_inventory(
            ArtifactDetail::Graph,
            raw.clone(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            &crate::schema_scope::SchemaSelection::default(),
            completeness.clone(),
            &mut test_audit(),
        )
        .expect("build qualified catalog-requirement inventory");
        assert!(qualified.requirements_complete);
        assert!(qualified.artifacts.values().any(|artifact| {
            artifact.requirement_status == "complete"
                && artifact.requirements.is_empty()
                && artifact.external.is_none()
        }));

        completeness.catalogs_unreadable.push("pg_proc".into());
        let degraded = build_inventory(
            ArtifactDetail::Graph,
            raw,
            &BTreeMap::new(),
            &BTreeMap::new(),
            &crate::schema_scope::SchemaSelection::default(),
            completeness,
            &mut test_audit(),
        )
        .expect("catalog loss degrades rather than aborts");
        assert!(!degraded.requirements_complete);
    }
}
