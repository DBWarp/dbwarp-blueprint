//! Versioned, checked artifact-complexity assessment.
//!
//! Complexity is derived entirely from the serialized artifact graph. Keeping
//! the assessor beside the model lets every reader and writer recompute the
//! same result instead of trusting a stale aggregate block.

use std::collections::BTreeSet;

use anyhow::{bail, Context, Result};

use crate::{
    ArtifactComplexity, ArtifactComplexityCountDimension, ArtifactComplexityCountHistogram,
    ArtifactComplexityDimensions, ArtifactComplexitySizeDimension, ArtifactComplexitySizeHistogram,
    ArtifactInventory, BlueprintArtifact, ARTIFACT_COMPLEXITY_ASSESSOR_VERSION,
    ARTIFACT_COMPLEXITY_CONTRACT, ARTIFACT_COMPLEXITY_POPULATION_POLICY,
};

const SIZE_BAND_ORDER: [&str; 7] = ["0", "1-255", "256-1k", "1k-4k", "4k-16k", "16k-64k", "64k+"];
const COUNT_BAND_ORDER: [&str; 7] = ["0", "1", "2-4", "5-8", "9-16", "17-32", "33+"];
const LEXICAL_SUPPORTED_DIALECTS: [&str; 5] = ["sql", "plpgsql", "mysql-sql-psm", "plsql", "tsql"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum DimensionObservation {
    Assessed(usize),
    NotApplicable,
    Unknown,
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

    fn record(&mut self, observation: DimensionObservation) -> Result<()> {
        let count = match observation {
            DimensionObservation::Assessed(index) => self
                .assessed
                .get_mut(index)
                .context("artifact complexity received an invalid histogram bucket")?,
            DimensionObservation::NotApplicable => &mut self.not_applicable,
            DimensionObservation::Unknown => &mut self.unknown,
        };
        *count = count
            .checked_add(1)
            .context("artifact complexity histogram count overflowed u64")?;
        Ok(())
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
            .rposition(|count| *count > 0)
            .map(complexity_band_for_bucket)
            .unwrap_or("trivial");
        // An unknown observation may occupy any closed bucket. The dimension
        // is definitive only when coverage is complete or when the known
        // lower bound has already reached the maximum possible band.
        let band = if self.unknown == 0 || lower_bound == "very-high" {
            lower_bound
        } else {
            "unknown"
        };
        (coverage.to_string(), band.to_string())
    }

    fn size_dimension(&self, eligible: u64) -> ArtifactComplexitySizeDimension {
        let (coverage, band) = self.coverage_and_band(eligible);
        ArtifactComplexitySizeDimension {
            band,
            coverage,
            histogram: ArtifactComplexitySizeHistogram {
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

    fn count_dimension(&self, eligible: u64) -> ArtifactComplexityCountDimension {
        let (coverage, band) = self.coverage_and_band(eligible);
        ArtifactComplexityCountDimension {
            band,
            coverage,
            histogram: ArtifactComplexityCountHistogram {
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

fn size_band_index(band: &str) -> Option<usize> {
    SIZE_BAND_ORDER
        .iter()
        .position(|candidate| *candidate == band)
}

fn count_band_index(band: &str) -> Option<usize> {
    COUNT_BAND_ORDER
        .iter()
        .position(|candidate| *candidate == band)
}

fn count_band(value: u64) -> &'static str {
    match value {
        0 => "0",
        1 => "1",
        2..=4 => "2-4",
        5..=8 => "5-8",
        9..=16 => "9-16",
        17..=32 => "17-32",
        _ => "33+",
    }
}

fn checked_len(value: usize, label: &str) -> Result<u64> {
    u64::try_from(value).with_context(|| format!("artifact complexity {label} exceeds u64"))
}

fn checked_increment(value: &mut u64, label: &str) -> Result<()> {
    *value = value
        .checked_add(1)
        .with_context(|| format!("artifact complexity {label} overflowed u64"))?;
    Ok(())
}

fn recorded_band(band: &str, order: fn(&str) -> Option<usize>) -> DimensionObservation {
    order(band)
        .map(DimensionObservation::Assessed)
        .unwrap_or(DimensionObservation::Unknown)
}

fn definition_observation(
    artifact: &BlueprintArtifact,
    band: &str,
    order: fn(&str) -> Option<usize>,
) -> DimensionObservation {
    if artifact.definition_visibility == "not_applicable"
        || artifact
            .analysis
            .as_ref()
            .is_some_and(|analysis| analysis.status == "not_applicable")
    {
        return DimensionObservation::NotApplicable;
    }
    let Some(analysis) = artifact.analysis.as_ref() else {
        return DimensionObservation::Unknown;
    };
    if !matches!(analysis.status.as_str(), "complete" | "partial") {
        return DimensionObservation::Unknown;
    }
    recorded_band(band, order)
}

/// Recompute assessor-v1 output from the serialized artifact inventory.
///
/// `assessment_population_complete` is collector evidence and therefore the
/// sole input that cannot be derived from the graph itself. Every derived
/// count, histogram, coverage value, limitation and verdict is recomputed.
pub fn assess_artifact_complexity(
    inventory: &ArtifactInventory,
    assessment_population_complete: bool,
) -> Result<ArtifactComplexity> {
    if !matches!(inventory.detail.as_str(), "graph" | "analyzed") {
        bail!("artifact complexity requires graph or analyzed detail");
    }
    let analyzed = inventory.detail == "analyzed";
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

    for artifact in inventory.artifacts.values() {
        if artifact.generated_by_engine == Some(true) || artifact.secondary_object == Some(true) {
            checked_increment(&mut excluded, "excluded object count")?;
            continue;
        }
        checked_increment(&mut eligible, "eligible object count")?;

        definitions_withheld |= analyzed && artifact.definition_visibility == "withheld";
        wrapped_source |= analyzed
            && artifact
                .requirements
                .iter()
                .any(|requirement| requirement.token == "oracle.source.wrapped");
        outside_selected_scope |= artifact
            .unresolved_relationships
            .contains_key("outside-selected-schema");

        let mut unknown_dimensions = 0_u8;
        let mut assessed_dimensions = 0_u8;
        let entanglement_observation = if inventory.dependencies_complete {
            let unresolved_total = artifact
                .unresolved_relationships
                .values()
                .try_fold(0_u64, |sum, count| sum.checked_add(*count))
                .context("artifact complexity unresolved edge count overflowed u64")?;
            let edge_count = checked_len(artifact.relationships.len(), "relationship count")?
                .checked_add(unresolved_total)
                .context("artifact complexity edge count overflowed u64")?;
            recorded_band(count_band(edge_count), count_band_index)
        } else {
            DimensionObservation::Unknown
        };
        unknown_dimensions += u8::from(entanglement_observation == DimensionObservation::Unknown);
        assessed_dimensions += u8::from(matches!(
            entanglement_observation,
            DimensionObservation::Assessed(_)
        ));
        entanglement.record(entanglement_observation)?;

        match artifact.requirement_status.as_str() {
            "complete" => {
                let external_requirements = artifact
                    .requirements
                    .iter()
                    .try_fold(0_u64, |count, requirement| {
                        if requirement.token.starts_with("external.") {
                            count.checked_add(1)
                        } else {
                            Some(count)
                        }
                    })
                    .context("artifact complexity external requirement count overflowed u64")?;
                let external_count = external_requirements
                    .checked_add(u64::from(artifact.external.is_some()))
                    .context("artifact complexity environment coupling overflowed u64")?;
                environment_coupling
                    .record(recorded_band(count_band(external_count), count_band_index))?;
                let requirement_count =
                    checked_len(artifact.requirements.len(), "requirement count")?;
                let dialect_specific = requirement_count
                    .checked_sub(external_requirements)
                    .context("artifact complexity requirement classification was inconsistent")?;
                dialect_coupling.record(recorded_band(
                    count_band(dialect_specific),
                    count_band_index,
                ))?;
                assessed_dimensions += 2;
            }
            "not_applicable" => {
                environment_coupling.record(DimensionObservation::NotApplicable)?;
                dialect_coupling.record(DimensionObservation::NotApplicable)?;
                // Not applicable is an assessed outcome: the collector looked
                // and proved that these dimensions do not apply.  Treating it
                // like no evidence would misclassify definition-free objects
                // as wholly unassessed when another dimension is unknown.
                assessed_dimensions += 2;
            }
            _ => {
                // Known requirements remain useful evidence, but partial,
                // unavailable, and empty pre-v7 coverage cannot prove
                // that the serialized list is exhaustive.
                environment_coupling.record(DimensionObservation::Unknown)?;
                dialect_coupling.record(DimensionObservation::Unknown)?;
                unknown_dimensions += 2;
            }
        }

        if let Some(analysis) = artifact.analysis.as_ref() {
            dialects.insert(analysis.dialect.clone());
            grammar_profiles.insert(analysis.grammar_profile.clone());
            analysis_spans.insert(if analysis.analysis_span.is_empty() {
                "unknown".to_string()
            } else {
                analysis.analysis_span.clone()
            });
            wrapped_source |= analyzed && analysis.features.contains_key("source.wrapped");
            unsupported_dialect |= analysis.status == "unavailable"
                && artifact.definition_visibility == "available"
                && !LEXICAL_SUPPORTED_DIALECTS.contains(&analysis.dialect.as_str());
        }

        let definition_dimensions = [
            (
                &mut volume,
                definition_observation(
                    artifact,
                    artifact
                        .analysis
                        .as_ref()
                        .map(|analysis| analysis.definition_size_band.as_str())
                        .unwrap_or(""),
                    size_band_index,
                ),
            ),
            (
                &mut control_flow,
                definition_observation(
                    artifact,
                    artifact
                        .analysis
                        .as_ref()
                        .map(|analysis| analysis.cyclomatic_complexity_band.as_str())
                        .unwrap_or(""),
                    count_band_index,
                ),
            ),
            (
                &mut opacity,
                definition_observation(
                    artifact,
                    artifact
                        .analysis
                        .as_ref()
                        .map(|analysis| analysis.opaque_region_count_band.as_str())
                        .unwrap_or(""),
                    count_band_index,
                ),
            ),
        ];
        for (tally, observation) in definition_dimensions {
            unknown_dimensions += u8::from(observation == DimensionObservation::Unknown);
            assessed_dimensions += u8::from(matches!(
                observation,
                DimensionObservation::Assessed(_) | DimensionObservation::NotApplicable
            ));
            tally.record(observation)?;
        }

        let feature_observation = if artifact.definition_visibility == "not_applicable"
            || artifact
                .analysis
                .as_ref()
                .is_some_and(|analysis| analysis.status == "not_applicable")
        {
            DimensionObservation::NotApplicable
        } else if artifact
            .analysis
            .as_ref()
            .is_some_and(|analysis| matches!(analysis.status.as_str(), "complete" | "partial"))
        {
            let count = checked_len(
                artifact
                    .analysis
                    .as_ref()
                    .map(|analysis| analysis.features.len())
                    .unwrap_or(0),
                "language feature count",
            )?;
            recorded_band(count_band(count), count_band_index)
        } else {
            DimensionObservation::Unknown
        };
        unknown_dimensions += u8::from(feature_observation == DimensionObservation::Unknown);
        assessed_dimensions += u8::from(matches!(
            feature_observation,
            DimensionObservation::Assessed(_) | DimensionObservation::NotApplicable
        ));
        feature_breadth.record(feature_observation)?;

        if unknown_dimensions == 0 {
            checked_increment(&mut fully, "fully assessed object count")?;
        } else if assessed_dimensions == 0 {
            checked_increment(&mut unassessed, "unassessed object count")?;
        } else {
            checked_increment(&mut partially, "partially assessed object count")?;
        }
    }

    let dimensions = ArtifactComplexityDimensions {
        volume: volume.size_dimension(eligible),
        control_flow: control_flow.count_dimension(eligible),
        feature_breadth: feature_breadth.count_dimension(eligible),
        entanglement: entanglement.count_dimension(eligible),
        environment_coupling: environment_coupling.count_dimension(eligible),
        opacity: opacity.count_dimension(eligible),
        dialect_coupling: dialect_coupling.count_dimension(eligible),
    };

    let mut limitations = Vec::new();
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
    if !inventory.dependencies_complete {
        limitations.push("graph-incomplete".to_string());
    }
    if !inventory.requirements_complete {
        limitations.push("requirements-incomplete".to_string());
    }
    if outside_selected_scope {
        limitations.push("outside-selected-scope".to_string());
    }
    limitations.sort();

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
    } else if candidate == "very-high" || (assessment_population_complete && fully == eligible) {
        candidate
    } else {
        "unknown".to_string()
    };

    Ok(ArtifactComplexity {
        contract: ARTIFACT_COMPLEXITY_CONTRACT.to_string(),
        assessor_version: ARTIFACT_COMPLEXITY_ASSESSOR_VERSION,
        scope: inventory.scope.clone(),
        population_policy: ARTIFACT_COMPLEXITY_POPULATION_POLICY.to_string(),
        assessment_population_complete,
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
        analysis_spans: analysis_spans.into_iter().collect(),
        dialects: dialects.into_iter().collect(),
        grammar_profiles: grammar_profiles.into_iter().collect(),
        overall_band,
        overall_score: None,
        limitations,
        dimensions,
    })
}

/// Fail-closed assessor output used when checked computation cannot complete.
/// No object disappears: the eligible population is retained and every
/// affected dimension becomes unknown, allowing the rest of the Blueprint to
/// remain usable without presenting invented complexity evidence.
pub fn failed_artifact_complexity(
    inventory: &ArtifactInventory,
    assessment_population_complete: bool,
) -> Result<ArtifactComplexity> {
    let analyzed = inventory.detail == "analyzed";
    let mut eligible = 0_u64;
    let mut excluded = 0_u64;
    let mut dialects = BTreeSet::new();
    let mut grammar_profiles = BTreeSet::new();
    let mut analysis_spans = BTreeSet::new();
    let mut limitations = BTreeSet::from(["computation-failed".to_string()]);

    if !analyzed {
        limitations.insert("definition-analysis-not-requested".to_string());
    }
    if !inventory.dependencies_complete {
        limitations.insert("graph-incomplete".to_string());
    }
    if !inventory.requirements_complete {
        limitations.insert("requirements-incomplete".to_string());
    }
    for artifact in inventory.artifacts.values() {
        if artifact.generated_by_engine == Some(true) || artifact.secondary_object == Some(true) {
            checked_increment(&mut excluded, "excluded object count")?;
            continue;
        }
        checked_increment(&mut eligible, "eligible object count")?;
        if analyzed && artifact.definition_visibility == "withheld" {
            limitations.insert("definitions-withheld".to_string());
        }
        if analyzed
            && artifact
                .requirements
                .iter()
                .any(|requirement| requirement.token == "oracle.source.wrapped")
        {
            limitations.insert("wrapped-source".to_string());
        }
        if artifact
            .unresolved_relationships
            .contains_key("outside-selected-schema")
        {
            limitations.insert("outside-selected-scope".to_string());
        }
        if let Some(analysis) = artifact.analysis.as_ref() {
            dialects.insert(analysis.dialect.clone());
            grammar_profiles.insert(analysis.grammar_profile.clone());
            analysis_spans.insert(if analysis.analysis_span.is_empty() {
                "unknown".to_string()
            } else {
                analysis.analysis_span.clone()
            });
            if analyzed && analysis.features.contains_key("source.wrapped") {
                limitations.insert("wrapped-source".to_string());
            }
            if analyzed
                && analysis.status == "unavailable"
                && artifact.definition_visibility == "available"
                && !LEXICAL_SUPPORTED_DIALECTS.contains(&analysis.dialect.as_str())
            {
                limitations.insert("unsupported-dialect".to_string());
            }
        }
    }

    let count_unknown = ArtifactComplexityCountDimension {
        band: if eligible == 0 {
            "not-applicable".to_string()
        } else {
            "unknown".to_string()
        },
        coverage: if eligible == 0 {
            "not-applicable".to_string()
        } else {
            "unknown".to_string()
        },
        histogram: ArtifactComplexityCountHistogram {
            unknown: eligible,
            ..Default::default()
        },
    };
    let size_unknown = ArtifactComplexitySizeDimension {
        band: count_unknown.band.clone(),
        coverage: count_unknown.coverage.clone(),
        histogram: ArtifactComplexitySizeHistogram {
            unknown: eligible,
            ..Default::default()
        },
    };

    Ok(ArtifactComplexity {
        contract: ARTIFACT_COMPLEXITY_CONTRACT.to_string(),
        assessor_version: ARTIFACT_COMPLEXITY_ASSESSOR_VERSION,
        scope: inventory.scope.clone(),
        population_policy: ARTIFACT_COMPLEXITY_POPULATION_POLICY.to_string(),
        assessment_population_complete,
        eligible_object_count: eligible,
        fully_assessed_object_count: 0,
        partially_assessed_object_count: 0,
        unassessed_object_count: eligible,
        excluded_object_count: excluded,
        analyzer_version: if analyzed {
            "lexical-v2"
        } else {
            "not-applicable"
        }
        .to_string(),
        analysis_spans: analysis_spans.into_iter().collect(),
        dialects: dialects.into_iter().collect(),
        grammar_profiles: grammar_profiles.into_iter().collect(),
        overall_band: if eligible == 0 && assessment_population_complete {
            "not-applicable".to_string()
        } else {
            "unknown".to_string()
        },
        overall_score: None,
        limitations: limitations.into_iter().collect(),
        dimensions: ArtifactComplexityDimensions {
            volume: size_unknown,
            control_flow: count_unknown.clone(),
            feature_breadth: count_unknown.clone(),
            entanglement: count_unknown.clone(),
            environment_coupling: count_unknown.clone(),
            opacity: count_unknown.clone(),
            dialect_coupling: count_unknown,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn partial_dimension_is_unknown_unless_known_lower_bound_is_maximal() {
        let mut bounded = DimensionTally::new();
        bounded
            .record(DimensionObservation::Assessed(2))
            .expect("record low observation");
        bounded
            .record(DimensionObservation::Unknown)
            .expect("record unknown observation");
        assert_eq!(
            bounded.coverage_and_band(2),
            ("partial".to_string(), "unknown".to_string())
        );

        let mut already_maximal = DimensionTally::new();
        already_maximal
            .record(DimensionObservation::Assessed(6))
            .expect("record maximal observation");
        already_maximal
            .record(DimensionObservation::Unknown)
            .expect("record unknown observation");
        assert_eq!(
            already_maximal.coverage_and_band(2),
            ("partial".to_string(), "very-high".to_string())
        );
    }

    #[test]
    fn unqualified_requirement_absence_is_unknown_not_zero() {
        let inventory = ArtifactInventory {
            detail: "graph".to_string(),
            scope: "all-visible-schemas".to_string(),
            dependencies_complete: true,
            requirements_complete: false,
            object_count: 1,
            counts_by_kind: BTreeMap::from([("sequence".to_string(), 1)]),
            artifacts: BTreeMap::from([(
                "sequence-001".to_string(),
                BlueprintArtifact {
                    kind: "sequence".to_string(),
                    requirement_status: "unavailable".to_string(),
                    definition_visibility: "not_applicable".to_string(),
                    ..Default::default()
                },
            )]),
            ..Default::default()
        };
        let result = assess_artifact_complexity(&inventory, true).expect("assess inventory");
        assert_eq!(result.dimensions.environment_coupling.coverage, "unknown");
        assert_eq!(result.dimensions.dialect_coupling.coverage, "unknown");
        assert!(result
            .limitations
            .contains(&"requirements-incomplete".to_string()));
        assert_eq!(result.partially_assessed_object_count, 1);
    }

    #[test]
    fn not_applicable_dimensions_are_assessed_for_object_coverage() {
        let inventory = ArtifactInventory {
            detail: "analyzed".to_string(),
            scope: "all-visible-schemas".to_string(),
            dependencies_complete: false,
            requirements_complete: true,
            object_count: 1,
            counts_by_kind: BTreeMap::from([("sequence".to_string(), 1)]),
            artifacts: BTreeMap::from([(
                "sequence-001".to_string(),
                BlueprintArtifact {
                    kind: "sequence".to_string(),
                    requirement_status: "not_applicable".to_string(),
                    definition_visibility: "not_applicable".to_string(),
                    ..Default::default()
                },
            )]),
            ..Default::default()
        };

        let result = assess_artifact_complexity(&inventory, true).expect("assess inventory");
        assert_eq!(result.fully_assessed_object_count, 0);
        assert_eq!(result.partially_assessed_object_count, 1);
        assert_eq!(result.unassessed_object_count, 0);
        assert_eq!(
            result.dimensions.environment_coupling.coverage,
            "not-applicable"
        );
        assert_eq!(result.dimensions.volume.coverage, "not-applicable");
        assert_eq!(result.dimensions.entanglement.coverage, "unknown");
    }
}
