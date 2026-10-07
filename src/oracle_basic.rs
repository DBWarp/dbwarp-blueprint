//! Map a bounded Oracle Basic catalog capture into the schema-v7 Blueprint.
//!
//! This module receives only normalized provider rows.  Native object names
//! are used transiently to join catalogs and assign keyed anonymous ordinals;
//! none are copied into the serialized contract.  Missing optional catalogs
//! lower the relevant completeness field instead of inventing zeros.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{bail, Context, Result};
use dbwarp_blueprint_core::{
    ArtifactInventory, BlueprintColumn, BlueprintFile, BlueprintIndex, BlueprintTable,
    DatabaseTopology, DatasetScope, FkEdge, LobStorageEvidence, SourceEnvironment,
    TableStatisticsEvidence, Totals, ARTIFACT_CONTRACT, SCHEMA_VERSION, TOPOLOGY_CONTRACT,
};

use crate::artifacts::ArtifactDetail;
use crate::engine_common::accumulate_table_totals;
use crate::format;
use crate::oracle_catalog::{queries_for_tier, CatalogTier, Degradation, CATALOG_QUERIES};
#[cfg(test)]
use crate::oracle_provider::OracleClientVersionAttestation;
use crate::oracle_provider::OracleVersion;
use crate::oracle_scope::OracleOwnerScope;
use crate::oracle_session::{
    OracleCaptureAbort, OracleCatalogCapture, OracleQueryOutcome, OracleQueryStatus, OracleRow,
    OracleValue,
};

#[derive(Debug, Clone)]
pub struct OracleBasicOptions {
    pub source_kind: String,
    pub generated_at_pin: Option<String>,
    pub artifact_detail: ArtifactDetail,
}

#[derive(Debug, Clone, Default)]
struct OracleObjectFacts {
    object_types: BTreeSet<String>,
    temporary: ObservedOptionalBool,
    generated: ObservedOptionalBool,
    secondary: ObservedOptionalBool,
}

/// Order-independent reduction for a nullable Oracle catalogue flag that may
/// occur on more than one object row. A positive observation is definitive;
/// otherwise any unknown observation prevents a false completeness claim.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum ObservedOptionalBool {
    #[default]
    Unseen,
    FalseOnly,
    Unknown,
    True,
}

impl ObservedOptionalBool {
    fn observe(&mut self, observed: Option<bool>) {
        *self = match (*self, observed) {
            (Self::True, _) | (_, Some(true)) => Self::True,
            (Self::Unseen, Some(false)) => Self::FalseOnly,
            (Self::FalseOnly, Some(false)) => Self::FalseOnly,
            (Self::Unknown, Some(false)) => Self::Unknown,
            (Self::Unseen | Self::FalseOnly | Self::Unknown, None) => Self::Unknown,
        };
    }

    fn value(self) -> Option<bool> {
        match self {
            Self::FalseOnly => Some(false),
            Self::True => Some(true),
            Self::Unseen | Self::Unknown => None,
        }
    }
}

#[derive(Debug, Clone)]
struct RawTable {
    owner: String,
    name: String,
    rows: Option<u64>,
    avg_row_len: Option<u64>,
    last_analyzed: Option<String>,
    partitioned: bool,
    temporary: bool,
    iot_type: Option<String>,
    iot_name: Option<String>,
    secondary: bool,
    external: bool,
    nested: Option<bool>,
    object_table: bool,
    clustered: bool,
    dropped: bool,
    segment_created: Option<bool>,
    classification_complete: bool,
}

#[derive(Debug, Clone)]
struct RawIndex {
    owner: String,
    name: String,
    table_owner: String,
    table_name: String,
    index_type: String,
    unique: bool,
    partitioned: bool,
    status: String,
    generated: bool,
    secondary: bool,
    visibility: String,
    join_index: bool,
    segment_created: Option<bool>,
}

#[derive(Debug, Clone)]
struct RawConstraint {
    owner: String,
    name: String,
    constraint_type: String,
    table_name: String,
    referenced_owner: Option<String>,
    referenced_constraint: Option<String>,
    delete_rule: String,
    status: String,
    deferrable: bool,
    initially_deferred: bool,
    validated: bool,
    index_owner: Option<String>,
    index_name: Option<String>,
    generated_name: bool,
    is_not_null_constraint: Option<bool>,
}

#[derive(Debug, Clone, Default)]
struct PartitionFacts {
    strategy: String,
    count: Option<u64>,
    key_ordinals: Vec<u32>,
    rows_max: Option<u64>,
    top_partition_rows: Vec<Option<u64>>,
    subpartition_rows: Vec<Option<u64>>,
}

#[derive(Debug, Clone, Default)]
struct LobFacts {
    storage_class: String,
    in_row: Option<bool>,
    compression: String,
    deduplication: String,
    encrypted: Option<bool>,
    segment_name: String,
    index_name: String,
}

#[derive(Debug, Clone, Default)]
struct ColumnCatalogFacts {
    hidden: Option<bool>,
    invisible: Option<bool>,
    generated_virtual: Option<bool>,
    internal_ordinal: Option<u64>,
}

#[derive(Debug, Clone, Default)]
struct OracleStatisticsFacts {
    last_analyzed: Option<String>,
    stale: Option<bool>,
    sample_size: Option<u64>,
    global: Option<bool>,
    user_supplied: Option<bool>,
    locked: bool,
    scope: String,
}

#[derive(Debug, Default)]
struct CollectedOracleStatisticsFacts {
    facts: BTreeMap<(String, String), OracleStatisticsFacts>,
    invalid_tables: BTreeSet<(String, String)>,
    unscoped_malformed: bool,
}

type NativeObjectKey = (String, String);
#[derive(Debug, Clone)]
struct RawIndexColumn {
    position: u64,
    ordinal: Option<u32>,
    descending: bool,
}
type IndexColumnsByIndex = BTreeMap<NativeObjectKey, Vec<RawIndexColumn>>;
type ExpressionIndexColumns = BTreeSet<(String, String, u64)>;
#[derive(Debug, Default)]
struct ExpressionIndexFacts {
    columns: ExpressionIndexColumns,
    indexes: BTreeSet<NativeObjectKey>,
    invalid_indexes: BTreeSet<NativeObjectKey>,
    unscoped_malformed: bool,
}
type SegmentBytesByObject = BTreeMap<NativeObjectKey, u64>;
#[derive(Debug, Default)]
struct CollectedSegmentBytes {
    table_bytes: SegmentBytesByObject,
    index_bytes: SegmentBytesByObject,
    incomplete_tables: BTreeSet<NativeObjectKey>,
    lob_attribution_incomplete: bool,
    unmapped_nested_segments: bool,
}
#[derive(Debug, Default)]
struct SegmentAttributionLookup {
    index_to_table: BTreeMap<NativeObjectKey, NativeObjectKey>,
    iot_top_indexes: BTreeSet<NativeObjectKey>,
    iot_overflow_to_parent: BTreeMap<NativeObjectKey, NativeObjectKey>,
}
type ColumnOrdinalsWithGaps = (
    BTreeMap<(String, String, String), u32>,
    BTreeSet<NativeObjectKey>,
);

fn is_generated_hidden_support_index(
    index: &RawIndex,
    index_columns: &IndexColumnsByIndex,
) -> bool {
    index.generated
        && index_columns
            .get(&(index.owner.clone(), index.name.clone()))
            .is_some_and(|columns| {
                !columns.is_empty() && columns.iter().all(|column| column.ordinal.is_none())
            })
}

fn oracle_table_is_customer_data(
    table: &RawTable,
    object: &OracleObjectFacts,
    oracle_maintained_owners: &BTreeSet<String>,
    support_tables: &BTreeSet<NativeObjectKey>,
) -> bool {
    !oracle_maintained_owners.contains(&table.owner)
        && !support_tables.contains(&(table.owner.clone(), table.name.clone()))
        && !table.dropped
        && table.nested != Some(true)
        && !table.iot_type.as_deref().is_some_and(|value| {
            matches!(
                value.to_ascii_uppercase().as_str(),
                "IOT_OVERFLOW" | "IOT_MAPPING"
            )
        })
        && (object.object_types.contains("MATERIALIZED VIEW")
            || (!table.secondary
                && object.generated.value() != Some(true)
                && object.secondary.value() != Some(true)))
}

/// Convert a successful Basic capture. The named Oracle floor is evaluated per
/// owner: at least one owner must have complete object classification, table
/// inventory, and column inventory. Optional families and unreadable owners
/// degrade without discarding a usable owner.
pub fn map_oracle_basic_capture(
    capture: &OracleCatalogCapture,
    owner_scope: &OracleOwnerScope,
    server_version: OracleVersion,
    options: &OracleBasicOptions,
) -> Result<BlueprintFile> {
    if options.artifact_detail > ArtifactDetail::Summary {
        bail!("Oracle Basic supports artifact detail none or summary only");
    }
    if matches!(
        capture.abort,
        Some(OracleCaptureAbort::AdapterContractViolated | OracleCaptureAbort::NoLiveSession)
    ) {
        bail!(
            "Oracle Basic catalog capture cannot be trusted: {:?}",
            capture.abort
        );
    }
    validate_capture_shape_and_scope(capture, owner_scope, server_version)?;
    if server_version.components()[0] >= 12
        && rows(capture, "oracle-containers")
            .iter()
            .any(|row| optional_u64(row, 0).ok().flatten() == Some(1))
    {
        bail!("Oracle capture is connected to CDB$ROOT; connect directly to the intended PDB");
    }
    let floor = select_oracle_floor(capture, owner_scope, server_version)?;
    let floor_truncated = floor.truncated;
    let capture = &floor.capture;
    let owner_scope = &floor.owner_scope;
    for query in CATALOG_QUERIES
        .iter()
        .filter(|query| query.tier == CatalogTier::Basic)
        .filter(|query| matches!(query.degradation, Degradation::Fatal))
        .filter(|query| query.applies_to(server_version.components()[0]))
    {
        require_owner_query_complete(capture, query.query_id, owner_scope.owners().len())?;
    }

    let object_facts = collect_object_facts(capture)?;
    let (oracle_maintained_owners, oracle_maintained_owners_complete) =
        collect_oracle_maintained_owners(capture, owner_scope.owners().len());
    // Optional classification catalogs are parsed independently per owner.
    // One malformed or denied owner must not erase exact support identities or
    // object-table rows already proved for another owner.
    let mut mview_log_tables = BTreeSet::new();
    let mut mview_update_candidates = BTreeSet::new();
    let mut queue_storage_tables = BTreeSet::new();
    let mut external_tables = BTreeSet::new();
    let mut object_tables = Vec::new();
    let mut support_gap_owners = BTreeSet::new();
    let mut external_gap_owners = BTreeSet::new();
    let mut object_table_gap_owners = BTreeSet::new();
    for (owner_index, owner) in owner_scope.owners().iter().enumerate() {
        let owner_ordinal =
            u32::try_from(owner_index + 1).context("Oracle owner ordinal exceeds u32")?;

        if !owner_query_complete(capture, "oracle-mview-logs", owner_ordinal) {
            support_gap_owners.insert(owner.clone());
        } else {
            match collect_support_table_keys(capture, "oracle-mview-logs", false, owner_ordinal) {
                Ok(tables) => mview_log_tables.extend(tables),
                Err(_) => {
                    support_gap_owners.insert(owner.clone());
                }
            }
            match collect_mview_update_candidates(capture, owner_ordinal) {
                Ok(candidates) => mview_update_candidates.extend(candidates),
                Err(_) => {
                    support_gap_owners.insert(owner.clone());
                }
            }
        }

        if !owner_query_complete(capture, "oracle-queue-tables", owner_ordinal) {
            support_gap_owners.insert(owner.clone());
        } else {
            match collect_support_table_keys(capture, "oracle-queue-tables", true, owner_ordinal) {
                Ok(tables) => queue_storage_tables.extend(tables),
                Err(_) => {
                    support_gap_owners.insert(owner.clone());
                }
            }
        }

        if !owner_query_complete(capture, "oracle-external-tables", owner_ordinal) {
            external_gap_owners.insert(owner.clone());
        } else {
            match collect_support_table_keys(
                capture,
                "oracle-external-tables",
                false,
                owner_ordinal,
            ) {
                Ok(tables) => external_tables.extend(tables),
                Err(_) => {
                    external_gap_owners.insert(owner.clone());
                }
            }
        }

        if !owner_query_complete(capture, "oracle-object-tables", owner_ordinal) {
            object_table_gap_owners.insert(owner.clone());
        } else {
            match collect_object_tables(capture, owner_ordinal) {
                Ok(mut tables) => object_tables.append(&mut tables),
                Err(_) => {
                    object_table_gap_owners.insert(owner.clone());
                }
            }
        }
    }
    let mut support_tables = mview_log_tables
        .union(&queue_storage_tables)
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut all_raw_tables = collect_tables(capture)?;
    let regular_table_keys = all_raw_tables
        .iter()
        .map(|table| (table.owner.clone(), table.name.clone()))
        .collect::<BTreeSet<_>>();
    // DBA_OBJECTS is part of the fatal floor and sees object/XMLType tables
    // even when DBA_OBJECT_TABLES is denied.  A denied object-table query is
    // therefore a real inventory gap only when the authoritative object list
    // contains a TABLE identity that the regular DBA_TABLES result did not.
    // This disproof keeps a table-less or ordinary-table-only owner from
    // withdrawing every other owner's floor completeness.
    object_table_gap_owners.retain(|owner| {
        object_facts.iter().any(|(key, facts)| {
            &key.0 == owner
                && facts.object_types.contains("TABLE")
                && !regular_table_keys.contains(key)
        })
    });
    all_raw_tables.extend(object_tables);
    let mut raw_tables_by_key = BTreeMap::new();
    for table in all_raw_tables {
        let key = (table.owner.clone(), table.name.clone());
        raw_tables_by_key
            .entry(key)
            .and_modify(|existing: &mut RawTable| {
                existing.object_table |= table.object_table;
            })
            .or_insert(table);
    }
    let mut all_raw_tables = raw_tables_by_key.into_values().collect::<Vec<_>>();
    // Object tables are different from the support and external refinements:
    // their dedicated catalog is the only inventory authority, so every owner
    // must remain complete for that family. Support/external completeness is
    // evaluated below against the emitted customer-table population, after
    // exact support rows have been filtered out.
    let object_tables_complete = object_table_gap_owners.is_empty();
    let mut classification_gap_tables = BTreeSet::new();
    for table in &mut all_raw_tables {
        let key = (table.owner.clone(), table.name.clone());
        if support_gap_owners.contains(&table.owner) {
            // Without the exact support catalogs, any table in this owner can
            // be Oracle-owned queue or materialized-view-log storage. Retain
            // it as best-effort evidence, but make the ambiguity per-object.
            classification_gap_tables.insert(key.clone());
        }
        if !table.classification_complete {
            classification_gap_tables.insert(key.clone());
        }
        if object_facts.get(&key).is_none_or(|facts| {
            facts.temporary.value().is_none()
                || facts.generated.value().is_none()
                || facts.secondary.value().is_none()
        }) {
            classification_gap_tables.insert(key.clone());
        }
        let catalog_external = external_tables.contains(&key);
        if !external_gap_owners.contains(&table.owner) {
            if server_version.is_at_least(12, 2) && table.external != catalog_external {
                // Concurrent DDL can move the two catalog snapshots apart.
                // Preserve the table and use the conservative classification
                // while withdrawing the completeness claim.
                classification_gap_tables.insert(key.clone());
                table.external |= catalog_external;
            } else {
                table.external = catalog_external;
            }
        } else if !server_version.is_at_least(12, 2) {
            // The stable pre-12.2 DBA_TABLES projection carries only a
            // placeholder, so classification is unknown without the
            // dedicated family.
            classification_gap_tables.insert(key.clone());
        }
    }
    // Oracle 19c can create a temporary RUPD$_<master> table for a primary-key
    // materialized-view log; later releases ordinarily do not. The
    // dictionary exposes no direct support-table identity for it, so require
    // both the catalog-derived name and Oracle's temporary-table flag. An
    // ordinary customer table with the same reserved-looking prefix remains
    // customer data.
    for candidate in mview_update_candidates {
        if all_raw_tables
            .iter()
            .any(|table| (table.owner.clone(), table.name.clone()) == candidate && table.temporary)
        {
            support_tables.insert(candidate);
        }
    }
    for table in &all_raw_tables {
        if !object_facts.contains_key(&(table.owner.clone(), table.name.clone())) {
            classification_gap_tables.insert((table.owner.clone(), table.name.clone()));
        }
    }
    let mut raw_tables = all_raw_tables
        .iter()
        .filter(|table| {
            let object = object_facts
                .get(&(table.owner.clone(), table.name.clone()))
                .cloned()
                .unwrap_or_default();
            // Oracle-created storage tables are not independent customer
            // datasets. Their logical queue/MV objects are artifact-inventory
            // items, not tables.
            oracle_table_is_customer_data(
                table,
                &object,
                &oracle_maintained_owners,
                &support_tables,
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    raw_tables.sort_by(|left, right| {
        format::table_hash(&left.owner, &left.name)
            .cmp(&format::table_hash(&right.owner, &right.name))
            .then_with(|| (&left.owner, &left.name).cmp(&(&right.owner, &right.name)))
    });

    let mut table_ids = BTreeMap::new();
    for (index, table) in raw_tables.iter().enumerate() {
        let previous = table_ids.insert(
            (table.owner.clone(), table.name.clone()),
            format::table_id(index + 1),
        );
        if previous.is_some() {
            bail!("Oracle table catalog returned a duplicate native identity");
        }
    }
    // Aggregate classification completeness speaks only for emitted logical
    // tables. Oracle-owned support storage filtered above must not degrade the
    // customer table inventory merely because its classification row was
    // incomplete.
    classification_gap_tables.retain(|key| table_ids.contains_key(key));
    let emitted_table_owners = table_ids
        .keys()
        .map(|(owner, _)| owner.clone())
        .collect::<BTreeSet<_>>();
    let support_tables_complete = support_gap_owners.is_disjoint(&emitted_table_owners);
    // From 12.2 onward DBA_TABLES.EXTERNAL is itself a floor authority.  The
    // dedicated external-table catalogue remains a cross-check/refinement,
    // but denying it cannot withdraw row or size completeness when every
    // emitted table already carries the classification flag.
    let external_classification_complete =
        server_version.is_at_least(12, 2) || external_gap_owners.is_disjoint(&emitted_table_owners);
    let schema_ids = schema_ids(raw_tables.iter().map(|table| table.owner.as_str()));
    let columns = collect_columns(capture)?;
    let mut column_catalog_facts = BTreeMap::new();
    for (owner_index, _) in owner_scope.owners().iter().enumerate() {
        let owner_ordinal =
            u32::try_from(owner_index + 1).context("Oracle owner ordinal exceeds u32")?;
        // A denied or malformed hidden-column refinement is not a capture
        // failure. Visible COLUMN_ID values remain usable; an invisible column
        // without its fallback ordinal is narrowed to its own table below.
        if !owner_query_complete(capture, "oracle-tab-cols-hidden", owner_ordinal) {
            continue;
        } else if let Ok(owner_facts) = collect_column_catalog_facts(capture, owner_ordinal) {
            column_catalog_facts.extend(owner_facts);
        }
    }
    let (column_ordinals, mut column_gap_tables) =
        build_column_ordinals(&columns, &column_catalog_facts)?;
    // Column completeness has the same emitted-table population as the
    // Blueprint inventory, not the raw catalog population.
    column_gap_tables.retain(|key| table_ids.contains_key(key));
    let mut identity_columns = BTreeMap::new();
    let mut identity_gap_owners = BTreeSet::new();
    if server_version.components()[0] >= 12 {
        for (owner_index, owner) in owner_scope.owners().iter().enumerate() {
            let owner_ordinal =
                u32::try_from(owner_index + 1).context("Oracle owner ordinal exceeds u32")?;
            if !owner_query_complete(capture, "oracle-identity-columns", owner_ordinal) {
                identity_gap_owners.insert(owner.clone());
                continue;
            }
            match collect_identity_columns(capture, owner_ordinal) {
                Ok(owner_columns) => identity_columns.extend(owner_columns),
                Err(_) => {
                    identity_gap_owners.insert(owner.clone());
                }
            }
        }
    }
    // Identity/default metadata is an optional refinement of a column record,
    // not part of the required ordinal/type/nullability inventory. Missing
    // evidence is represented by the optional fields below; it must not
    // withdraw a complete floor column inventory.
    // Optional structure families are parsed per owner. A malformed row in
    // one schema withdraws that schema's refinement evidence; it must not
    // erase the independently readable inventory of another schema.
    let mut indexes = Vec::new();
    let mut index_columns = BTreeMap::new();
    let mut expression_index_facts = ExpressionIndexFacts::default();
    let mut invalid_index_owners = BTreeSet::new();
    for (owner_index, owner) in owner_scope.owners().iter().enumerate() {
        let owner_ordinal =
            u32::try_from(owner_index + 1).context("Oracle owner ordinal exceeds u32")?;
        match collect_indexes(capture, owner_ordinal) {
            Ok(mut owner_indexes) => indexes.append(&mut owner_indexes),
            Err(_) => {
                invalid_index_owners.insert(owner.clone());
            }
        }
        match collect_index_columns(capture, &column_ordinals, owner_ordinal) {
            Ok(owner_columns) => index_columns.extend(owner_columns),
            Err(_) => {
                invalid_index_owners.insert(owner.clone());
            }
        }
        let owner_expressions = collect_expression_index_facts(capture, owner_ordinal);
        expression_index_facts
            .columns
            .extend(owner_expressions.columns);
        expression_index_facts
            .indexes
            .extend(owner_expressions.indexes);
        expression_index_facts
            .invalid_indexes
            .extend(owner_expressions.invalid_indexes);
        if owner_expressions.unscoped_malformed {
            invalid_index_owners.insert(owner.clone());
        }
    }
    let invalid_expression_tables = indexes
        .iter()
        .filter(|index| {
            table_ids.contains_key(&(index.table_owner.clone(), index.table_name.clone()))
        })
        .filter(|index| {
            !index.secondary && !is_generated_hidden_support_index(index, &index_columns)
        })
        .filter(|index| {
            expression_index_facts
                .invalid_indexes
                .contains(&(index.owner.clone(), index.name.clone()))
        })
        .map(|index| (index.table_owner.clone(), index.table_name.clone()))
        .collect::<BTreeSet<_>>();
    // An ordinary index key without an emitted column ordinal makes the index
    // inventory incomplete. It does not, by itself, withdraw the required
    // column inventory: Oracle represents descending and function-based keys
    // with hidden SYS_NC columns that DBA_TAB_COLUMNS intentionally omits.
    // Keep the same expression predicate here and at emission so a denied
    // DBA_IND_EXPRESSIONS query degrades only the optional index family.
    let unresolved_index_column_tables = indexes
        .iter()
        // Physical indexes belonging to excluded nested, queue, materialized
        // view-log, or other engine-support tables are not part of the
        // emitted logical inventory. Their hidden support columns must not
        // downgrade the independently complete customer-table family.
        .filter(|index| {
            table_ids.contains_key(&(index.table_owner.clone(), index.table_name.clone()))
        })
        .filter(|index| {
            !index.secondary && !is_generated_hidden_support_index(index, &index_columns)
        })
        .filter(|index| {
            index_columns
                .get(&(index.owner.clone(), index.name.clone()))
                .is_some_and(|columns| {
                    columns.iter().any(|column| {
                        column.ordinal.is_none()
                            && !oracle_index_column_is_expression(
                                index,
                                column,
                                &expression_index_facts,
                            )
                    })
                })
        })
        .map(|index| (index.table_owner.clone(), index.table_name.clone()))
        .collect::<BTreeSet<_>>();
    let mut constraints = Vec::new();
    let mut invalid_constraint_owners = BTreeSet::new();
    for (owner_index, owner) in owner_scope.owners().iter().enumerate() {
        let owner_ordinal =
            u32::try_from(owner_index + 1).context("Oracle owner ordinal exceeds u32")?;
        match collect_constraints(capture, owner_ordinal) {
            Ok(mut owner_constraints) => constraints.append(&mut owner_constraints),
            Err(_) => {
                invalid_constraint_owners.insert(owner.clone());
            }
        }
    }
    let constraints_by_key = constraints
        .iter()
        .map(|constraint| {
            (
                (constraint.owner.clone(), constraint.name.clone()),
                constraint,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut constraint_columns = BTreeMap::new();
    let mut invalid_constraint_keys = BTreeSet::new();
    let mut invalid_constraint_column_owners = BTreeSet::new();
    for (owner_index, owner) in owner_scope.owners().iter().enumerate() {
        let owner_ordinal =
            u32::try_from(owner_index + 1).context("Oracle owner ordinal exceeds u32")?;
        match collect_constraint_columns(
            capture,
            &column_ordinals,
            &constraints_by_key,
            owner_ordinal,
        ) {
            Ok((owner_columns, _limitations, owner_invalid)) => {
                constraint_columns.extend(owner_columns);
                invalid_constraint_keys.extend(owner_invalid);
            }
            Err(_) => {
                invalid_constraint_column_owners.insert(owner.clone());
            }
        }
    }
    let mut partition_facts = BTreeMap::new();
    let mut invalid_partition_owners = BTreeSet::new();
    for (owner_index, owner) in owner_scope.owners().iter().enumerate() {
        let owner_ordinal =
            u32::try_from(owner_index + 1).context("Oracle owner ordinal exceeds u32")?;
        match collect_partition_facts(capture, &column_ordinals, owner_ordinal) {
            Ok(owner_partitions) => partition_facts.extend(owner_partitions),
            Err(_) => {
                invalid_partition_owners.insert(owner.clone());
            }
        }
    }
    let mut lob_facts = BTreeMap::new();
    let mut invalid_lob_owners = BTreeSet::new();
    let mut nested_table_parents = BTreeMap::new();
    let mut invalid_nested_owners = BTreeSet::new();
    for (owner_index, owner) in owner_scope.owners().iter().enumerate() {
        let owner_ordinal =
            u32::try_from(owner_index + 1).context("Oracle owner ordinal exceeds u32")?;
        match collect_lob_facts(capture, owner_ordinal) {
            Ok(owner_lobs) => lob_facts.extend(owner_lobs),
            Err(_) => {
                invalid_lob_owners.insert(owner.clone());
            }
        }
        match collect_nested_table_parents(capture, owner_ordinal) {
            Ok(owner_parents) => nested_table_parents.extend(owner_parents),
            Err(_) => {
                invalid_nested_owners.insert(owner.clone());
            }
        }
        if !owner_query_complete(capture, "oracle-nested-tables", owner_ordinal) {
            invalid_nested_owners.insert(owner.clone());
        }
    }
    let mut table_segment_bytes = BTreeMap::new();
    let mut index_segment_bytes = BTreeMap::new();
    let mut invalid_segment_owners = BTreeSet::new();
    let mut incomplete_segment_tables = BTreeSet::new();
    let mut owners_with_unattributed_lob_segments = BTreeSet::new();
    let mut owners_with_unmapped_nested_segments = BTreeSet::new();
    let segment_lookup = build_segment_attribution_lookup(&all_raw_tables, &indexes);
    for (owner_index, owner) in owner_scope.owners().iter().enumerate() {
        let owner_ordinal =
            u32::try_from(owner_index + 1).context("Oracle owner ordinal exceeds u32")?;
        match collect_segment_bytes(
            capture,
            &lob_facts,
            &nested_table_parents,
            &segment_lookup,
            &all_raw_tables,
            &indexes,
            owner_ordinal,
            owner,
        ) {
            Ok(owner_segments) => {
                incomplete_segment_tables.extend(merge_segment_bytes(
                    &mut table_segment_bytes,
                    owner_segments.table_bytes,
                ));
                incomplete_segment_tables.extend(merge_segment_bytes(
                    &mut index_segment_bytes,
                    owner_segments.index_bytes,
                ));
                incomplete_segment_tables.extend(owner_segments.incomplete_tables);
                if owner_segments.lob_attribution_incomplete {
                    owners_with_unattributed_lob_segments.insert(owner.clone());
                }
                if owner_segments.unmapped_nested_segments {
                    owners_with_unmapped_nested_segments.insert(owner.clone());
                }
            }
            Err(_) => {
                invalid_segment_owners.insert(owner.clone());
            }
        }
    }
    // When the mapping view is denied, only tables carrying a named
    // collection-like type can own the unattributed nested storage. Preserve
    // exact direct segment evidence for unrelated tables instead of applying
    // one capture-wide absence flag to every object.
    let mut owners_with_possible_nested_storage = all_raw_tables
        .iter()
        // `Some(false)` is the only catalog fact that proves this owner has no
        // nested storage. Preserve uncertainty when the floor row carried a
        // malformed flag rather than treating it as a negative observation.
        .filter(|table| table.nested != Some(false))
        .map(|table| table.owner.clone())
        .collect::<BTreeSet<_>>();
    // A physical nested segment is stronger positive evidence than a stale or
    // contradictory table flag. Keep its owner in scope even when the floor
    // catalog failed to identify the storage row itself.
    owners_with_possible_nested_storage
        .extend(owners_with_unmapped_nested_segments.iter().cloned());
    let possible_nested_parent_tables = columns
        .iter()
        .filter_map(|(key, table_columns)| {
            (owners_with_possible_nested_storage.contains(&key.0)
                && table_columns
                    .iter()
                    .any(oracle_column_may_own_nested_storage))
            .then_some(key.clone())
        })
        .collect::<BTreeSet<_>>();
    let nested_size_unavailable_tables = possible_nested_parent_tables
        .into_iter()
        .filter(|(owner, _)| {
            invalid_nested_owners.contains(owner)
                || owners_with_unmapped_nested_segments.contains(owner)
        })
        .collect::<BTreeSet<_>>();
    let mut indexes_by_table = BTreeMap::<NativeObjectKey, Vec<&RawIndex>>::new();
    let mut cross_owner_index_owners = BTreeMap::<NativeObjectKey, BTreeSet<String>>::new();
    for index in &indexes {
        let table_key = (index.table_owner.clone(), index.table_name.clone());
        indexes_by_table
            .entry(table_key.clone())
            .or_default()
            .push(index);
        if index.owner != index.table_owner {
            cross_owner_index_owners
                .entry(table_key)
                .or_default()
                .insert(index.owner.clone());
        }
    }
    let primary_index_keys = constraints
        .iter()
        .filter(|constraint| constraint.constraint_type == "P")
        .filter_map(|constraint| {
            constraint.index_name.as_ref().map(|index_name| {
                (
                    constraint
                        .index_owner
                        .clone()
                        .unwrap_or_else(|| constraint.owner.clone()),
                    index_name.clone(),
                )
            })
        })
        .collect::<BTreeSet<_>>();
    // Oracle stores both NOT NULL declarations and user CHECK expressions as
    // type C constraints. The identical `column IS NOT NULL` expression can
    // come from a NOT NULL declaration or an explicit CHECK, whether Oracle
    // generated the name or the customer supplied one. Name provenance and
    // the column's current nullability do not disambiguate the two (a primary
    // key or disabled constraint can change nullability independently), so
    // withhold the table's exact CHECK count rather than guess.
    let check_classification = constraints
        .iter()
        .filter(|constraint| constraint.constraint_type == "C")
        .map(|constraint| {
            let key = (constraint.owner.clone(), constraint.name.clone());
            let classification = match constraint.is_not_null_constraint {
                Some(false) => Some(false),
                Some(true) | None => None,
            };
            (key, classification)
        })
        .collect::<BTreeMap<_, _>>();
    let unknown_check_tables = constraints
        .iter()
        .filter(|constraint| constraint.constraint_type == "C")
        .filter(|constraint| {
            check_classification
                .get(&(constraint.owner.clone(), constraint.name.clone()))
                .is_none_or(Option::is_none)
        })
        .map(|constraint| (constraint.owner.clone(), constraint.table_name.clone()))
        .collect::<BTreeSet<_>>();
    let check_counts = constraints
        .iter()
        .filter(|constraint| {
            constraint.constraint_type == "C"
                && check_classification
                    .get(&(constraint.owner.clone(), constraint.name.clone()))
                    .copied()
                    .flatten()
                    == Some(false)
        })
        .try_fold(
            BTreeMap::<NativeObjectKey, u64>::new(),
            |mut counts, constraint| {
                let count = counts
                    .entry((constraint.owner.clone(), constraint.table_name.clone()))
                    .or_default();
                *count = count
                    .checked_add(1)
                    .context("Oracle CHECK count exceeds u64")?;
                Ok::<_, anyhow::Error>(counts)
            },
        )?;
    let owner_ordinals = owner_scope
        .owners()
        .iter()
        .enumerate()
        .map(|(index, owner)| {
            Ok::<_, anyhow::Error>((
                owner.clone(),
                u32::try_from(index + 1).context("Oracle owner ordinal exceeds u32")?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;

    let collected_statistics = collect_statistics_facts(capture);
    let statistics_facts = collected_statistics.facts;
    let invalid_statistics_tables = collected_statistics.invalid_tables;
    let statistics_unscoped_malformed = collected_statistics.unscoped_malformed;
    let expected_owner_queries = owner_scope.owners().len();
    let tables_catalog_status =
        query_catalog_evidence_status(capture, "oracle-tables", expected_owner_queries);
    let columns_catalog_status =
        query_catalog_evidence_status(capture, "oracle-tab-columns", expected_owner_queries);
    let indexes_catalog_status =
        query_catalog_evidence_status(capture, "oracle-indexes", expected_owner_queries);
    let constraints_catalog_status =
        query_catalog_evidence_status(capture, "oracle-constraints", expected_owner_queries);
    let constraint_columns_catalog_status =
        query_catalog_evidence_status(capture, "oracle-constraint-columns", expected_owner_queries);
    let identity_columns_catalog_status = (server_version.components()[0] >= 12).then(|| {
        query_catalog_evidence_status(capture, "oracle-identity-columns", expected_owner_queries)
    });
    let index_family_catalog_complete = [
        "oracle-indexes",
        "oracle-index-columns",
        "oracle-index-expressions",
    ]
    .into_iter()
    .all(|query_id| query_complete(capture, query_id, expected_owner_queries));
    let relationship_family_catalog_complete = ["oracle-constraints", "oracle-constraint-columns"]
        .into_iter()
        .all(|query_id| query_complete(capture, query_id, expected_owner_queries));
    let cross_scope_fk_tables = constraints
        .iter()
        .filter(|constraint| constraint.constraint_type.eq_ignore_ascii_case("R"))
        .filter(|constraint| {
            constraint
                .referenced_owner
                .as_ref()
                .is_some_and(|owner| !owner_scope.owners().contains(owner))
        })
        .map(|constraint| (constraint.owner.clone(), constraint.table_name.clone()))
        .collect::<BTreeSet<_>>();
    let unresolved_target_fk_tables = constraints
        .iter()
        .filter(|constraint| constraint.constraint_type.eq_ignore_ascii_case("R"))
        .filter(|constraint| {
            if constraint.referenced_owner.is_none() {
                return true;
            }
            let Some(parent_name) = constraint.referenced_constraint.as_ref() else {
                return true;
            };
            let parent_owner = constraint
                .referenced_owner
                .as_ref()
                .unwrap_or(&constraint.owner);
            if !owner_scope.owners().contains(parent_owner) {
                return !owner_scope.is_selection_limited();
            }
            constraints_by_key
                .get(&(parent_owner.clone(), parent_name.clone()))
                .is_none_or(|parent| {
                    !table_ids.contains_key(&(parent.owner.clone(), parent.table_name.clone()))
                })
        })
        .map(|constraint| (constraint.owner.clone(), constraint.table_name.clone()))
        .collect::<BTreeSet<_>>();
    let relationship_key_gap_tables = constraints
        .iter()
        .filter(|constraint| constraint.constraint_type.eq_ignore_ascii_case("R"))
        .filter(|constraint| {
            table_ids.contains_key(&(constraint.owner.clone(), constraint.table_name.clone()))
        })
        .filter(|constraint| {
            let child_key = (constraint.owner.clone(), constraint.name.clone());
            let Some(child_columns) = constraint_columns
                .get(&child_key)
                .filter(|columns| !columns.is_empty())
            else {
                return true;
            };
            if invalid_constraint_keys.contains(&child_key) {
                return true;
            }
            let (Some(parent_owner), Some(parent_name)) = (
                constraint.referenced_owner.as_ref(),
                constraint.referenced_constraint.as_ref(),
            ) else {
                return true;
            };
            if !owner_scope.owners().contains(parent_owner) {
                // A deliberately out-of-scope parent is represented by the
                // scope/target limitation, not mislabelled as a malformed
                // key. This is true for both selected and all-visible scope;
                // the latter already records target visibility as unknown.
                return false;
            }
            let parent_key = (parent_owner.clone(), parent_name.clone());
            let Some(parent) = constraints_by_key.get(&parent_key) else {
                return true;
            };
            if !table_ids.contains_key(&(parent.owner.clone(), parent.table_name.clone())) {
                // A visible-but-filtered target is classified by the target
                // visibility set. It is not a column-vector defect.
                return false;
            }
            let Some(parent_columns) = constraint_columns
                .get(&parent_key)
                .filter(|columns| !columns.is_empty())
            else {
                return true;
            };
            invalid_constraint_keys.contains(&parent_key)
                || child_columns.len() != parent_columns.len()
        })
        .map(|constraint| (constraint.owner.clone(), constraint.table_name.clone()))
        .collect::<BTreeSet<_>>();
    let mut tables = BTreeMap::new();
    let mut totals = Totals::default();
    let mut any_unknown_rows = false;
    let mut any_unknown_size = false;
    let mut any_measured_size = false;
    let mut any_derived_size = false;
    let mut any_available_size = false;
    let mut any_stale_statistics = false;
    let mut any_counted_stale_statistics = false;
    let mut emitted_column_ordinals = BTreeMap::<NativeObjectKey, BTreeSet<u32>>::new();
    for table in &raw_tables {
        let key = (table.owner.clone(), table.name.clone());
        let owner_ordinal = *owner_ordinals
            .get(&table.owner)
            .context("Oracle table owner is outside the resolved floor scope")?;
        let owner_index_catalog_complete = !invalid_index_owners.contains(&table.owner)
            && owner_query_complete(capture, "oracle-indexes", owner_ordinal)
            && owner_query_complete(capture, "oracle-index-columns", owner_ordinal)
            && owner_query_complete(capture, "oracle-index-expressions", owner_ordinal);
        let owner_constraint_catalog_complete = !invalid_constraint_owners.contains(&table.owner)
            && owner_query_complete(capture, "oracle-constraints", owner_ordinal);
        let owner_constraint_columns_complete = !invalid_constraint_column_owners
            .contains(&table.owner)
            && owner_query_complete(capture, "oracle-constraint-columns", owner_ordinal);
        let owner_partition_catalog_complete = !invalid_partition_owners.contains(&table.owner)
            && owner_query_complete(capture, "oracle-part-tables", owner_ordinal)
            && owner_query_complete(capture, "oracle-tab-partitions", owner_ordinal)
            && owner_query_complete(capture, "oracle-tab-subpartitions", owner_ordinal)
            && owner_query_complete(capture, "oracle-part-key-columns", owner_ordinal);
        let owner_identity_catalog_complete = !identity_gap_owners.contains(&table.owner);
        let table_id = table_ids
            .get(&key)
            .context("Oracle table id was not assigned")?
            .clone();
        let schema_id = schema_ids
            .get(&table.owner)
            .context("Oracle schema id was not assigned")?
            .clone();
        let object = object_facts.get(&key).cloned().unwrap_or_default();
        let object_kind = oracle_table_kind(table, &object);
        let external = object_kind == "external-table";
        let derived = object_kind == "materialized-view";
        let part = partition_facts.get(&key).cloned().unwrap_or_default();
        let temporary = object_kind == "temporary-table";
        let excluded_from_totals = external || derived || temporary;
        let rows = match table.rows {
            Some(rows) => rows,
            None => {
                // External and derived objects are not migration-volume
                // inputs. Their unavailable row evidence must not downgrade
                // the completeness of totals that deliberately exclude them.
                if !excluded_from_totals {
                    any_unknown_rows = true;
                }
                0
            }
        };
        let cross_owner_index_gap =
            cross_owner_index_owners
                .get(&key)
                .is_some_and(|index_owners| {
                    index_owners.iter().any(|index_owner| {
                        owner_ordinals.get(index_owner).is_none_or(|owner_ordinal| {
                            invalid_segment_owners.contains(index_owner)
                                || !owner_query_complete(capture, "oracle-segments", *owner_ordinal)
                        })
                    })
                });
        // A refinement gap must never erase segment bytes that Oracle did
        // return.  Completeness describes attribution of every dependent
        // segment; availability describes whether the segment census itself
        // was usable.  Only the latter permits the DBA_TABLES logical
        // fallback.
        let measured_table_bytes = table_segment_bytes.get(&key).copied();
        let measured_index_bytes = index_segment_bytes.get(&key).copied();
        let has_positive_measured_size_contribution =
            measured_table_bytes.unwrap_or(0) > 0 || measured_index_bytes.unwrap_or(0) > 0;
        let storage_requires_attribution_fallback = table.clustered
            || (table
                .iot_type
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case("IOT"))
                && measured_table_bytes.is_none())
            || (incomplete_segment_tables.contains(&key)
                && !has_positive_measured_size_contribution)
            || (table.rows.is_some_and(|rows| rows > 0)
                && table.segment_created != Some(false)
                && measured_table_bytes.is_none());
        let uses_measured_size = !invalid_segment_owners.contains(&table.owner)
            && owner_query_complete(capture, "oracle-segments", owner_ordinal)
            && !storage_requires_attribution_fallback;
        let owner_lobs_complete = !invalid_lob_owners.contains(&table.owner)
            && owner_query_complete(capture, "oracle-lobs", owner_ordinal);
        let owner_indexes_mappable = !invalid_index_owners.contains(&table.owner)
            && owner_query_complete(capture, "oracle-indexes", owner_ordinal);
        // Oracle domain indexes (including Text and Spatial indexes) can own
        // secondary tables and indexes whose storage is deliberately absent
        // from the customer-table inventory. Preserve the bytes we can
        // attribute, but do not claim that the index scope is complete.
        let table_has_domain_index = indexes_by_table.get(&key).is_some_and(|table_indexes| {
            table_indexes
                .iter()
                .any(|index| index.index_type.to_ascii_uppercase().contains("DOMAIN"))
        });
        let table_size_full = uses_measured_size
            && owner_lobs_complete
            && owner_indexes_mappable
            && !table_has_domain_index
            && !cross_owner_index_gap
            && !incomplete_segment_tables.contains(&key)
            && !owners_with_unattributed_lob_segments.contains(&table.owner)
            && !table.clustered
            && !nested_size_unavailable_tables.contains(&key);
        let logical_size_estimate = match (table.rows, table.avg_row_len) {
            (Some(rows), Some(avg_row_len)) => Some(
                rows.checked_mul(avg_row_len)
                    .context("Oracle logical table size estimate exceeds u64")?,
            ),
            _ => None,
        };
        let uses_derived_size =
            !uses_measured_size && !external && !temporary && logical_size_estimate.is_some();
        let table_size_available = uses_measured_size || uses_derived_size;
        if !excluded_from_totals {
            any_measured_size |= uses_measured_size;
            any_derived_size |= uses_derived_size;
            any_available_size |= table_size_available;
            any_unknown_size |= !table_size_full;
        }
        let table_bytes = if uses_measured_size {
            measured_table_bytes.unwrap_or(0)
        } else {
            logical_size_estimate.unwrap_or(0)
        };
        let index_bytes = if uses_measured_size {
            measured_index_bytes.unwrap_or(0)
        } else {
            0
        };

        let table_columns = columns.get(&key).cloned().unwrap_or_default();
        if table_columns.is_empty() {
            column_gap_tables.insert(key.clone());
        }
        let mut cols = BTreeMap::new();
        for row in &table_columns {
            let Ok(column_name) = required_identifier(row, 2, "Oracle column name") else {
                column_gap_tables.insert(key.clone());
                continue;
            };
            let native_key = (table.owner.clone(), table.name.clone(), column_name.clone());
            let Some(ordinal) = column_ordinals.get(&native_key).copied() else {
                column_gap_tables.insert(key.clone());
                continue;
            };
            let Ok(data_type) = required_text(row, 4, "Oracle column type") else {
                column_gap_tables.insert(key.clone());
                continue;
            };
            let (Ok(data_length), Ok(precision), Ok(scale), Some(nullable), Ok(char_length)) = (
                optional_u64(row, 5),
                optional_u64(row, 6),
                optional_i64(row, 7),
                yes_no(row, 8),
                optional_u64(row, 9),
            ) else {
                column_gap_tables.insert(key.clone());
                continue;
            };
            let char_used = optional_text(row, 10).unwrap_or_default();
            let default_length = optional_u64(row, 11).unwrap_or(None);
            let semantics = oracle_column_semantics(
                &data_type,
                data_length,
                char_length,
                &char_used,
                precision,
                scale,
            );
            let identity = owner_identity_catalog_complete
                .then(|| identity_columns.get(&native_key))
                .flatten();
            let catalog_facts = column_catalog_facts
                .get(&native_key)
                .cloned()
                .unwrap_or_default();
            let value_source = identity.cloned().unwrap_or_else(|| {
                if catalog_facts.generated_virtual == Some(true) {
                    "generated-virtual".to_string()
                } else {
                    String::new()
                }
            });
            let generated_value =
                identity.is_some() || catalog_facts.generated_virtual == Some(true);
            let has_default = if (!owner_identity_catalog_complete
                || catalog_facts.generated_virtual.is_none())
                && catalog_facts.generated_virtual != Some(true)
            {
                None
            } else {
                default_length.map(|length| length > 0 && !generated_value)
            };
            let lob_storage = lob_facts.get(&native_key).map(|facts| {
                let complete = facts.compression != "unknown"
                    && facts.deduplication != "unknown"
                    && facts.in_row.is_some()
                    && facts.encrypted.is_some();
                LobStorageEvidence {
                    storage_class: facts.storage_class.clone(),
                    compression: facts.compression.clone(),
                    deduplication: facts.deduplication.clone(),
                    in_row: facts.in_row,
                    encrypted: facts.encrypted,
                    visibility: if complete { "full" } else { "partial" }.to_string(),
                }
            });
            let previous = cols.insert(
                format::col_id(ordinal),
                BlueprintColumn {
                    ordinal,
                    column_type: semantics.column_type,
                    nullable,
                    value_source,
                    has_default,
                    // The SQL*Plus-compatible Basic projection proves
                    // presence from DEFAULT_LENGTH without transporting the
                    // LONG DATA_DEFAULT text. Its semantic class therefore
                    // remains unknown rather than being guessed.
                    default_kind: String::new(),
                    default_on_null: yes_no(row, 12).filter(|_| has_default == Some(true)),
                    hidden: catalog_facts.hidden,
                    invisible: catalog_facts.invisible,
                    native_type: String::new(),
                    declared_max_chars: semantics.declared_max_chars,
                    declared_max_bytes: semantics.declared_max_bytes,
                    length_semantics: semantics.length_semantics,
                    numeric_model: semantics.numeric_model,
                    numeric_precision: semantics.numeric_precision,
                    numeric_scale: semantics.numeric_scale,
                    numeric_precision_radix: semantics.numeric_precision_radix,
                    bit_width: semantics.bit_width,
                    datetime_precision: semantics.datetime_precision,
                    lob_storage,
                    ..BlueprintColumn::default()
                },
            );
            if previous.is_some() {
                bail!("Oracle table has duplicate column ordinals");
            }
        }
        let table_emitted_ordinals = cols
            .values()
            .map(|column| column.ordinal)
            .collect::<BTreeSet<_>>();
        emitted_column_ordinals.insert(key.clone(), table_emitted_ordinals.clone());

        let table_index_keys_complete =
            indexes_by_table
                .get(&key)
                .into_iter()
                .flatten()
                .all(|index| {
                    index.secondary
                        || index.index_type.eq_ignore_ascii_case("LOB")
                        || index.index_type.eq_ignore_ascii_case("IOT - TOP")
                        || index_columns
                            .get(&(index.owner.clone(), index.name.clone()))
                            .is_some_and(|columns| !columns.is_empty())
                });
        let table_index_inventory_complete = owner_index_catalog_complete
            && table_index_keys_complete
            && !invalid_expression_tables.contains(&key)
            && !unresolved_index_column_tables.contains(&key);
        let mut table_indexes = if table_index_inventory_complete {
            indexes_by_table
                .get(&key)
                .into_iter()
                .flatten()
                .filter(|index| {
                    !index.secondary && !is_generated_hidden_support_index(index, &index_columns)
                })
                .filter(|index| {
                    let index_key = (index.owner.clone(), index.name.clone());
                    index_columns.get(&index_key).is_some_and(|columns| {
                        !columns.is_empty()
                            && columns.iter().all(|column| match column.ordinal {
                                Some(ordinal) => table_emitted_ordinals.contains(&ordinal),
                                None => oracle_index_column_is_expression(
                                    index,
                                    column,
                                    &expression_index_facts,
                                ),
                            })
                    })
                })
                .copied()
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        table_indexes.sort_by(|left, right| {
            format::index_hash(&left.name)
                .cmp(&format::index_hash(&right.name))
                .then_with(|| left.name.cmp(&right.name))
        });
        let mut idxs = BTreeMap::new();
        for (index_position, index) in table_indexes.iter().enumerate() {
            let index_key = (index.owner.clone(), index.name.clone());
            let mut ordinals = index_columns.get(&index_key).cloned().unwrap_or_default();
            ordinals.sort_by_key(|column| column.position);
            let bitmap_join = index.join_index;
            let expression =
                oracle_index_has_expression(index, &expression_index_facts, &index_columns);
            let cols = if bitmap_join {
                Vec::new()
            } else {
                ordinals
                    .into_iter()
                    .filter_map(|column| column.ordinal)
                    .collect::<Vec<_>>()
            };
            let primary = primary_index_keys.contains(&(index.owner.clone(), index.name.clone()));
            idxs.insert(
                format::idx_id(u32::try_from(index_position + 1).context("too many indexes")?),
                BlueprintIndex {
                    index_type: normalize_index_type(&index.index_type).to_string(),
                    unique: index.unique,
                    primary,
                    cols,
                    expression,
                    partitioning: if index.partitioned { "unknown" } else { "none" }.to_string(),
                    visibility: match index.visibility.to_ascii_uppercase().as_str() {
                        "INVISIBLE" => "invisible",
                        "VISIBLE" => "visible",
                        _ => "unknown",
                    }
                    .to_string(),
                    state: normalize_index_state(&index.status).to_string(),
                    ..BlueprintIndex::default()
                },
            );
        }

        let check_count = check_counts.get(&key).copied().unwrap_or(0);
        let table_statistics_valid = table_statistics_catalog_complete(
            owner_query_complete(capture, "oracle-tab-statistics", owner_ordinal),
            statistics_unscoped_malformed,
            &invalid_statistics_tables,
            &key,
        );
        let table_statistics = table_statistics_valid
            .then(|| statistics_facts.get(&key))
            .flatten();
        let last_analyzed = table_statistics
            .and_then(|facts| facts.last_analyzed.as_ref())
            .or(table.last_analyzed.as_ref());
        let statistics_state = if external || temporary {
            "not-applicable"
        } else if table_statistics.is_some_and(|facts| facts.locked) {
            "locked"
        } else if table_statistics.and_then(|facts| facts.user_supplied) == Some(true) {
            "user-supplied"
        } else if last_analyzed.is_none() {
            "never-analyzed"
        } else if table_statistics.and_then(|facts| facts.stale) == Some(true) {
            "known-stale"
        } else if table_statistics.and_then(|facts| facts.stale) == Some(false) {
            "current"
        } else if table_statistics_valid {
            "possibly-stale"
        } else {
            "unknown"
        };
        let table_statistics_stale = matches!(statistics_state, "known-stale" | "possibly-stale");
        any_stale_statistics |= table_statistics_stale;
        if !excluded_from_totals {
            any_counted_stale_statistics |= table_statistics_stale;
        }
        let statistics = TableStatisticsEvidence {
            row_count_method: if external || temporary {
                "unknown"
            } else if table.rows.is_some() {
                "oracle-table-statistics"
            } else {
                "unknown"
            }
            .to_string(),
            row_count_quality: if external || temporary {
                "unavailable"
            } else if table.rows.is_some() {
                "engine-estimate"
            } else {
                "unavailable"
            }
            .to_string(),
            statistics_state: statistics_state.to_string(),
            refresh_age_band: if external || temporary || last_analyzed.is_none() {
                "not-applicable"
            } else {
                "unknown"
            }
            .to_string(),
            modification_ratio_band: if external || temporary || last_analyzed.is_none() {
                "not-applicable"
            } else {
                "unknown"
            }
            .to_string(),
            sample_fraction_band: if external || temporary {
                "not-applicable"
            } else {
                sample_fraction_band(
                    table.rows,
                    table_statistics.and_then(|facts| facts.sample_size),
                )
            }
            .to_string(),
            statistics_scope: if external {
                "unknown"
            } else if temporary {
                table_statistics
                    .map(|facts| facts.scope.as_str())
                    .filter(|scope| matches!(*scope, "session" | "global"))
                    .unwrap_or("unknown")
            } else if table_statistics.is_none() {
                "unknown"
            } else if table.partitioned
                && table_statistics.and_then(|facts| facts.global) != Some(true)
            {
                "partition"
            } else {
                "global"
            }
            .to_string(),
            size_method: if external || temporary {
                "unknown"
            } else if uses_measured_size {
                "oracle-segment-bytes"
            } else if uses_derived_size {
                "oracle-table-logical-estimate"
            } else {
                "unknown"
            }
            .to_string(),
            size_quality: if external || temporary {
                "unavailable"
            } else if uses_measured_size {
                "exact-counter"
            } else if uses_derived_size {
                "engine-estimate"
            } else {
                "unavailable"
            }
            .to_string(),
            size_scope: if external || temporary {
                "unknown"
            } else if table_size_full {
                "table-lob-and-index"
            } else if uses_derived_size {
                "table-only"
            } else if uses_measured_size {
                // Partial segment evidence can include an arbitrary subset
                // of table, LOB and index storage. Calling it table-only can
                // contradict a positive index_bytes value and tells a
                // reader more than the evidence established.
                "unknown"
            } else {
                "unknown"
            }
            .to_string(),
            size_accounting: if external || temporary {
                "unknown"
            } else if uses_measured_size {
                "allocated-segment"
            } else if uses_derived_size {
                "logical-estimate"
            } else {
                "unknown"
            }
            .to_string(),
            size_visibility: if external || temporary {
                "unavailable"
            } else if table_size_full {
                "full"
            } else if table_size_available {
                "partial"
            } else {
                "unavailable"
            }
            .to_string(),
        };
        let table_partition_key_complete = !table.partitioned
            || partition_facts.get(&key).is_some_and(|facts| {
                matches!(facts.strategy.as_str(), "system" | "reference")
                    || !facts.key_ordinals.is_empty()
            });
        let partition_details_available = table.partitioned
            && owner_partition_catalog_complete
            && table_partition_key_complete
            && !part.strategy.is_empty()
            && part.strategy != "unknown"
            && part.count.is_some_and(|count| count > 0)
            && part
                .key_ordinals
                .iter()
                .all(|ordinal| table_emitted_ordinals.contains(ordinal));
        let partition_count = if partition_details_available {
            part.count
        } else {
            None
        };
        let mut table_limitations = Vec::new();
        if cross_scope_fk_tables.contains(&key) {
            table_limitations.push(
                if owner_scope.is_selection_limited() {
                    "relationship-target-outside-selected-scope"
                } else {
                    "relationship-target-visibility-unknown"
                }
                .to_string(),
            );
        }
        if unresolved_target_fk_tables.contains(&key)
            && !table_limitations
                .iter()
                .any(|item| item == "relationship-target-visibility-unknown")
        {
            table_limitations.push("relationship-target-visibility-unknown".to_string());
        }
        if classification_gap_tables.contains(&key) {
            table_limitations.push("table-classification-unavailable".to_string());
        }
        if column_gap_tables.contains(&key) {
            table_limitations.push("column-inventory-unavailable".to_string());
            table_limitations.push("dependent-structure-suppressed".to_string());
        }
        if !table_index_inventory_complete {
            table_limitations.push("index-inventory-unavailable".to_string());
        }
        if !owner_constraint_catalog_complete
            || !owner_constraint_columns_complete
            || relationship_key_gap_tables.contains(&key)
        {
            table_limitations.push("relationship-inventory-unavailable".to_string());
        }
        table_limitations.sort();
        table_limitations.dedup();
        let serialized_rows = if external || temporary {
            0
        } else {
            format::round_estimated_rows(rows)
        };
        let serialized_table_bytes = if external || temporary {
            0
        } else {
            format::round_bytes(table_bytes)
        };
        let serialized_index_bytes = if external || temporary {
            0
        } else {
            format::round_bytes(index_bytes)
        };
        let blueprint_table = BlueprintTable {
            rows: serialized_rows,
            table_bytes: serialized_table_bytes,
            index_bytes: serialized_index_bytes,
            schema: schema_id,
            object_kind: object_kind.to_string(),
            storage_organization: oracle_storage_organization(table, external).to_string(),
            partitioning: if table.partitioned {
                if !partition_details_available {
                    "unknown".to_string()
                } else {
                    part.strategy.clone()
                }
            } else {
                "none".to_string()
            },
            segment_state: if external || temporary || !uses_measured_size {
                "unavailable"
            } else if table_bytes == 0 && index_bytes == 0 {
                "deferred"
            } else if table_bytes > 0
                && serialized_table_bytes == 0
                && index_bytes > 0
                && serialized_index_bytes == 0
            {
                // Preserve both positive sub-bucket contributions. A single
                // `mixed` token can only prove the index allocation and would
                // lose the independently observed table allocation.
                "mixed-table-and-index"
            } else if index_bytes > 0 && serialized_index_bytes == 0 {
                // Preserve the fact that an attributed index allocation was
                // present even when privacy rounding suppresses its byte
                // bucket. Without this state, `index_bytes = 0` is
                // indistinguishable from a table that had no index storage.
                "mixed"
            } else {
                "created"
            }
            .to_string(),
            partition_count,
            partition_key_cols: if !partition_details_available {
                Vec::new()
            } else {
                part.key_ordinals.clone()
            },
            partition_rows_max: if !partition_details_available {
                None
            } else {
                part.rows_max
                    .and_then(|rows| format::round_partition_rows_max(rows, serialized_rows, false))
            },
            counted_in_totals: excluded_from_totals.then_some(false),
            check_count: (owner_constraint_catalog_complete
                && !unknown_check_tables.contains(&key))
            .then_some(check_count),
            table_limitations,
            statistics: Some(statistics),
            cols,
            idxs,
            ..BlueprintTable::default()
        };
        accumulate_table_totals(&mut totals, &blueprint_table)?;
        tables.insert(table_id, blueprint_table);
    }

    let (fk_edges, emitted_column_relationship_gaps) = build_fk_edges(
        &constraints,
        &constraint_columns,
        &table_ids,
        &invalid_constraint_keys,
        &emitted_column_ordinals,
    );
    for child_table in emitted_column_relationship_gaps {
        if let Some(table_id) = table_ids.get(&child_table) {
            let table = tables
                .get_mut(table_id)
                .context("Oracle foreign-key child table disappeared during mapping")?;
            table
                .table_limitations
                .push("relationship-inventory-unavailable".to_string());
            table.table_limitations.sort();
            table.table_limitations.dedup();
        }
    }
    // A missing parent column suppresses the child's relationship even when
    // every child column was readable. Carry that dependency loss on the
    // child as well as the table whose own column inventory has the gap.
    for constraint in constraints
        .iter()
        .filter(|constraint| constraint.constraint_type.eq_ignore_ascii_case("R"))
    {
        let child_table = (constraint.owner.clone(), constraint.table_name.clone());
        let parent_table = constraint
            .referenced_owner
            .as_ref()
            .zip(constraint.referenced_constraint.as_ref())
            .and_then(|(owner, name)| constraints_by_key.get(&(owner.clone(), name.clone())))
            .map(|parent| (parent.owner.clone(), parent.table_name.clone()));
        if !column_gap_tables.contains(&child_table)
            && parent_table
                .as_ref()
                .is_none_or(|parent| !column_gap_tables.contains(parent))
        {
            continue;
        }
        if let Some(table_id) = table_ids.get(&child_table) {
            let table = tables
                .get_mut(table_id)
                .context("Oracle foreign-key child table disappeared during mapping")?;
            table
                .table_limitations
                .push("relationship-inventory-unavailable".to_string());
            table.table_limitations.sort();
            table.table_limitations.dedup();
        }
    }
    // Catalogue provenance is derived directly from observed query outcomes;
    // table limitations independently identify which emitted objects lost
    // representable structure. Aggregate completeness requires both. This is
    // intentionally not an `all()` over tables alone: that predicate is
    // vacuously true for an empty inventory and cannot observe a denied query
    // for a selected owner that contributed no tables.
    let logical_indexes_complete = index_family_catalog_complete
        && tables.values().all(|table| {
            !table.table_limitations.iter().any(|item| {
                matches!(
                    item.as_str(),
                    "index-inventory-unavailable" | "dependent-structure-suppressed"
                )
            })
        });
    let relationships_complete = relationship_family_catalog_complete
        && tables.values().all(|table| {
            !table.table_limitations.iter().any(|item| {
                matches!(
                    item.as_str(),
                    "relationship-inventory-unavailable"
                        | "relationship-target-visibility-unknown"
                        | "dependent-structure-suppressed"
                )
            })
        });
    let empty_table_inventory = tables.is_empty();
    let population_inventory_complete = object_tables_complete
        && support_tables_complete
        && oracle_maintained_owners_complete
        && external_classification_complete
        && classification_gap_tables.is_empty()
        && !empty_table_inventory
        && !floor_truncated;
    let mut limitations = vec!["row-counts-statistical".to_string()];
    if owner_scope.is_selection_limited() {
        limitations.push("selection-limited".to_string());
    }
    if any_unknown_rows {
        limitations.push("row-count-evidence-incomplete".to_string());
    }
    if !population_inventory_complete {
        if !oracle_maintained_owners_complete || empty_table_inventory {
            limitations.push("table-inventory-visibility-unknown".to_string());
        }
        limitations.push("row-count-evidence-incomplete".to_string());
        limitations.push("size-evidence-incomplete".to_string());
    }
    if any_unknown_size {
        limitations.push("size-evidence-incomplete".to_string());
    }
    if floor_truncated {
        limitations.push("catalog-capture-truncated".to_string());
        limitations.push("table-inventory-visibility-unknown".to_string());
    }
    if any_counted_stale_statistics {
        limitations.push("statistics-stale".to_string());
    }
    limitations.sort();
    limitations.dedup();
    let excluded_only_population =
        totals.table_count == 0 && !tables.is_empty() && population_inventory_complete;

    let mut blueprint = BlueprintFile {
        schema_version: SCHEMA_VERSION,
        generated_at: format::generated_at_now(options.generated_at_pin.as_deref()),
        engine: "oracle".to_string(),
        engine_version: server_version
            .components()
            .iter()
            .map(u16::to_string)
            .collect::<Vec<_>>()
            .join("."),
        source_kind: options.source_kind.clone(),
        length_metadata: "hybrid-v2".to_string(),
        declared_length_fidelity: "exact".to_string(),
        index_length_fidelity: "not-captured".to_string(),
        observed_length_fidelity: "not-captured".to_string(),
        totals,
        database_topology: Some(build_topology(capture, server_version.components()[0])),
        dataset_scope: Some(DatasetScope {
            contract: dbwarp_blueprint_core::DATASET_SCOPE_CONTRACT.to_string(),
            layout: "full-copy".to_string(),
            table_inventory_completeness: if population_inventory_complete {
                "complete"
            } else {
                "incomplete"
            }
            .to_string(),
            row_count_completeness: if any_unknown_rows || !population_inventory_complete {
                "incomplete"
            } else {
                "complete"
            }
            .to_string(),
            size_completeness: if !any_unknown_size
                && population_inventory_complete
                && !floor_truncated
            {
                "complete"
            } else {
                "incomplete"
            }
            .to_string(),
            row_count_method: if excluded_only_population {
                "not-applicable"
            } else {
                "oracle-table-statistics"
            }
            .to_string(),
            // A non-empty inventory whose tables are all deliberately
            // excluded from copy totals has no aggregate sizing population.
            // It is complete by policy, but no segment measurement method
            // applies to the deliberately empty total.
            size_method: if excluded_only_population {
                "not-applicable"
            } else if !any_available_size {
                "unknown"
            } else if any_measured_size && any_derived_size {
                "mixed"
            } else if any_derived_size {
                "oracle-table-logical-estimate"
            } else {
                "oracle-segment-bytes"
            }
            .to_string(),
            limitations,
        }),
        source_environment: Some(build_source_environment(capture)),
        tables,
        fk_edges,
        artifact_inventory: Some(build_artifact_summary(
            capture,
            owner_scope,
            floor_truncated,
            options,
        )?),
        ..BlueprintFile::default()
    };
    let oracle_table_statistics = blueprint
        .tables
        .iter()
        .map(|(table_id, table)| {
            (
                table_id.clone(),
                table
                    .statistics
                    .clone()
                    .context("Oracle table statistics evidence was not built"),
            )
        })
        .map(|(table_id, evidence)| Ok((table_id, evidence?)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    blueprint.initialize_v7_database_contract();
    // The shared initializer deliberately installs conservative unknown
    // evidence for the other engine paths. Oracle Basic has already classified
    // each table from its live catalogs, so restore that source evidence
    // before deriving the checked aggregate.
    for (table_id, evidence) in oracle_table_statistics {
        blueprint
            .tables
            .get_mut(&table_id)
            .context("Oracle table disappeared during v7 initialization")?
            .statistics = Some(evidence);
    }
    let any_table_statistics_partial = blueprint
        .tables
        .values()
        .filter(|table| table.counted_in_totals != Some(false))
        .any(|table| {
            table.statistics.as_ref().is_none_or(|evidence| {
                evidence.size_visibility != "full"
                    || matches!(
                        evidence.row_count_quality.as_str(),
                        "unavailable" | "unknown"
                    )
                    || evidence.statistics_state == "unknown"
            })
        });
    let statistics_population_empty = blueprint
        .tables
        .values()
        .all(|table| table.counted_in_totals == Some(false));
    let any_emitted_invalid_statistics = invalid_statistics_tables
        .iter()
        .any(|key| table_ids.contains_key(key));

    let structure = blueprint
        .structure_scope
        .as_mut()
        .context("Oracle structure scope was not initialized")?;
    // Replace the optimistic initializer values from the query outcomes on
    // every path. Inventory completeness is evaluated separately below and
    // must never control whether positive/negative catalogue evidence is
    // retained.
    for (catalog, status) in [
        ("oracle-tables", tables_catalog_status),
        ("oracle-tab-columns", columns_catalog_status),
        ("oracle-indexes", indexes_catalog_status),
        ("oracle-constraints", constraints_catalog_status),
        (
            "oracle-constraint-columns",
            constraint_columns_catalog_status,
        ),
    ] {
        reconcile_structure_catalog(structure, catalog, status);
    }
    if let Some(status) = identity_columns_catalog_status {
        reconcile_structure_catalog(structure, "oracle-identity-columns", status);
    }
    if empty_table_inventory {
        structure.visibility = "unknown".to_string();
        structure.table_inventory_completeness = "incomplete".to_string();
        structure.column_inventory_completeness = "incomplete".to_string();
        structure.index_inventory_completeness = "incomplete".to_string();
        structure.relationship_inventory_completeness = "incomplete".to_string();
        structure
            .limitations
            .push("table-kinds-not-inventoried".to_string());
    }
    if !object_tables_complete {
        structure.table_inventory_completeness = "incomplete".to_string();
        structure.column_inventory_completeness = "incomplete".to_string();
        structure.index_inventory_completeness = "incomplete".to_string();
        structure.relationship_inventory_completeness = "incomplete".to_string();
        structure
            .limitations
            .push("table-kinds-not-inventoried".to_string());
        structure
            .limitations
            .push("dependent-structure-suppressed".to_string());
    }
    if floor_truncated {
        structure.visibility = "unknown".to_string();
        structure.table_inventory_completeness = "incomplete".to_string();
        structure.column_inventory_completeness = "incomplete".to_string();
        structure.index_inventory_completeness = "incomplete".to_string();
        structure.relationship_inventory_completeness = "incomplete".to_string();
        structure
            .limitations
            .push("catalog-capture-truncated".to_string());
    }
    if !external_classification_complete || !classification_gap_tables.is_empty() {
        structure.table_inventory_completeness = "incomplete".to_string();
        structure.column_inventory_completeness = "incomplete".to_string();
        structure.index_inventory_completeness = "incomplete".to_string();
        structure.relationship_inventory_completeness = "incomplete".to_string();
        structure
            .limitations
            .push("table-kinds-not-inventoried".to_string());
        if !external_classification_complete {
            structure
                .limitations
                .push("metadata-visibility-unknown".to_string());
        }
    }
    if !column_gap_tables.is_empty() {
        structure.column_inventory_completeness = "incomplete".to_string();
        structure.index_inventory_completeness = "incomplete".to_string();
        structure.relationship_inventory_completeness = "incomplete".to_string();
        structure
            .limitations
            .push("column-inventory-unavailable".to_string());
        structure
            .limitations
            .push("dependent-structure-suppressed".to_string());
    }
    if !identity_gap_owners.is_disjoint(&emitted_table_owners) {
        // Identity generation is an optional column refinement. Keep the
        // required ordinal/type/nullability inventory complete, but surface a
        // denied or malformed identity catalogue instead of making the output
        // indistinguishable from a proven non-identity column set.
        structure
            .limitations
            .push("metadata-visibility-unknown".to_string());
    }
    if !oracle_maintained_owners_complete {
        // Without the Oracle-maintained flag, system-owned support objects
        // cannot be separated safely from customer objects. Preserve visible
        // rows as best-effort evidence, but withdraw every completeness claim
        // that depends on that classification.
        structure.table_inventory_completeness = "incomplete".to_string();
        structure.column_inventory_completeness = "incomplete".to_string();
        structure.index_inventory_completeness = "incomplete".to_string();
        structure.relationship_inventory_completeness = "incomplete".to_string();
        structure.visibility = "unknown".to_string();
        structure
            .limitations
            .push("metadata-visibility-unknown".to_string());
        structure
            .limitations
            .push("dependent-structure-suppressed".to_string());
    }
    if !logical_indexes_complete {
        structure.index_inventory_completeness = "incomplete".to_string();
        structure
            .limitations
            .push("index-inventory-unavailable".to_string());
    }
    if !relationships_complete {
        structure.relationship_inventory_completeness = "incomplete".to_string();
        structure
            .limitations
            .push("relationship-inventory-unavailable".to_string());
    }
    for values in [
        &mut structure.catalogs_read,
        &mut structure.catalogs_unreadable,
        &mut structure.catalogs_not_applicable,
        &mut structure.limitations,
    ] {
        values.sort();
        values.dedup();
    }

    let mut statistics_catalogs_read = Vec::new();
    let mut statistics_catalogs_unreadable = Vec::new();
    let statistics_catalogs_not_applicable = Vec::<&str>::new();
    let statistics_catalog_statuses = [
        (
            "oracle-segments",
            query_catalog_evidence_status(capture, "oracle-segments", expected_owner_queries),
        ),
        (
            "oracle-tab-statistics",
            query_catalog_evidence_status(capture, "oracle-tab-statistics", expected_owner_queries),
        ),
    ];
    for (catalog, status) in statistics_catalog_statuses {
        match status {
            OracleCatalogEvidenceStatus::Read | OracleCatalogEvidenceStatus::PartiallyRead => {
                statistics_catalogs_read.push(catalog)
            }
            OracleCatalogEvidenceStatus::Unreadable => statistics_catalogs_unreadable.push(catalog),
            OracleCatalogEvidenceStatus::Unobserved => {}
        }
    }
    let mut stats_limitations = vec![
        "modification-evidence-unavailable",
        "optimizer-statistics-not-row-counter",
        "refresh-age-unavailable",
    ];
    let statistics_partial = any_unknown_rows
        || any_unknown_size
        || statistics_unscoped_malformed
        || any_emitted_invalid_statistics
        || !population_inventory_complete
        || any_table_statistics_partial
        || statistics_catalog_statuses
            .iter()
            .any(|(_, status)| *status != OracleCatalogEvidenceStatus::Read)
        || !statistics_catalogs_unreadable.is_empty();
    if statistics_partial {
        stats_limitations.push("statistics-partial");
    }
    if any_stale_statistics {
        stats_limitations.push("statistics-stale");
    }
    if statistics_population_empty {
        stats_limitations.push("statistics-visibility-unknown");
    }
    if floor_truncated {
        stats_limitations.push("catalog-capture-truncated");
    }
    crate::statistics::rebuild_statistics_evidence(
        &mut blueprint,
        if statistics_population_empty {
            "unknown"
        } else if statistics_partial {
            "partial"
        } else {
            "full"
        },
        &statistics_catalogs_read,
        &statistics_catalogs_unreadable,
        &statistics_catalogs_not_applicable,
        &stats_limitations,
    )?;
    dbwarp_blueprint_core::validate_blueprint_contract(&blueprint)
        .context("mapped Oracle Basic Blueprint failed its schema-v7 contract")?;
    Ok(blueprint)
}

/// Reject adapter or offline-pack drift before native catalog values are
/// joined. In particular, a result attributed to owner ordinal N must contain
/// exactly that resolved owner in every row. This makes `--schema`/owner scope
/// a server-side and receiver-verified boundary rather than trusting the
/// command client to have applied its bind correctly.
fn validate_capture_shape_and_scope(
    capture: &OracleCatalogCapture,
    owner_scope: &OracleOwnerScope,
    server_version: OracleVersion,
) -> Result<()> {
    let server_major = server_version.components()[0];
    let queries = CATALOG_QUERIES
        .iter()
        .map(|query| (query.query_id, query))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();

    for outcome in &capture.outcomes {
        let query = queries
            .get(outcome.query_id)
            .with_context(|| "Oracle capture contains an unknown query id")?;
        if outcome.view != query.view {
            bail!("Oracle capture query/view provenance does not match the registry");
        }
        if query.tier != CatalogTier::Basic {
            bail!("Oracle Basic capture contains an above-Basic catalog family");
        }
        if !seen.insert((outcome.query_id, outcome.owner_ordinal)) {
            bail!("Oracle capture contains a duplicate query/owner outcome");
        }
        match (query.applies_to(server_major), &outcome.status) {
            (false, OracleQueryStatus::SkippedVersion) => {}
            (false, _) => bail!("Oracle capture executed a version-inapplicable query"),
            (true, OracleQueryStatus::SkippedVersion) => {
                bail!("Oracle capture skipped an applicable query as version-inapplicable")
            }
            (true, _) => {}
        }

        let projected = query.output_columns_for_version(server_version);
        let expected_owner = match (query.owner_column, outcome.owner_ordinal) {
            (Some(_), Some(ordinal)) => {
                let zero_based = ordinal
                    .checked_sub(1)
                    .context("Oracle capture owner ordinal must be one-based")?;
                let index = usize::try_from(zero_based)
                    .context("Oracle owner ordinal exceeds the local index range")?;
                Some(
                    owner_scope
                        .owners()
                        .get(index)
                        .context("Oracle capture owner ordinal is outside the resolved scope")?,
                )
            }
            (Some(_), None) | (None, Some(_)) => {
                bail!("Oracle capture owner provenance does not match the Oracle query set")
            }
            (None, None) => None,
        };
        let owner_column_index = query
            .owner_column
            .map(|owner_column| {
                projected
                    .iter()
                    .position(|column| column.eq_ignore_ascii_case(owner_column))
                    .context("owner-scoped Oracle query does not project its owner column")
            })
            .transpose()?;

        let executed_rows = match outcome.status {
            OracleQueryStatus::Executed { rows } => Some(rows),
            _ => None,
        };
        if executed_rows.is_none() && !outcome.rows.is_empty() {
            bail!("Oracle capture attached rows to a non-executed outcome");
        }
        if let Some(declared_rows) = executed_rows {
            if declared_rows != outcome.rows.len() as u64 {
                bail!("Oracle capture row count does not match its outcome evidence");
            }
        }
        for row in &outcome.rows {
            if row.values.len() != projected.len() {
                bail!(
                    "Oracle capture row width does not match the Oracle query set for {}",
                    outcome.query_id
                );
            }
            if let (Some(expected), Some(owner_index)) = (expected_owner, owner_column_index) {
                if required_identifier(row, owner_index, "Oracle owner scope value")?
                    != expected.as_str()
                {
                    bail!("Oracle capture row falls outside its resolved owner scope");
                }
            }
        }
    }
    let owner_count = u32::try_from(owner_scope.owners().len())
        .context("Oracle owner scope exceeds the contract ordinal range")?;
    let expected = queries_for_tier(CatalogTier::Basic)
        .flat_map(|query| {
            if query.owner_column.is_some() {
                (1..=owner_count)
                    .map(|ordinal| (query.query_id, Some(ordinal)))
                    .collect::<Vec<_>>()
            } else {
                vec![(query.query_id, None)]
            }
        })
        .collect::<BTreeSet<_>>();
    if seen != expected {
        bail!("Oracle Basic capture outcome manifest is incomplete or unexpected");
    }
    Ok(())
}

fn outcomes<'a>(capture: &'a OracleCatalogCapture, query_id: &str) -> Vec<&'a OracleQueryOutcome> {
    capture
        .outcomes
        .iter()
        .filter(|outcome| outcome.query_id == query_id)
        .collect()
}

fn rows<'a>(capture: &'a OracleCatalogCapture, query_id: &str) -> Vec<&'a OracleRow> {
    outcomes(capture, query_id)
        .into_iter()
        .filter(|outcome| matches!(outcome.status, OracleQueryStatus::Executed { .. }))
        .flat_map(|outcome| outcome.rows.iter())
        .collect()
}

fn owner_rows<'a>(
    capture: &'a OracleCatalogCapture,
    query_id: &str,
    owner_ordinal: u32,
) -> Vec<&'a OracleRow> {
    outcomes(capture, query_id)
        .into_iter()
        .filter(|outcome| outcome.owner_ordinal == Some(owner_ordinal))
        .filter(|outcome| matches!(outcome.status, OracleQueryStatus::Executed { .. }))
        .flat_map(|outcome| outcome.rows.iter())
        .collect()
}

fn query_complete(capture: &OracleCatalogCapture, query_id: &str, expected: usize) -> bool {
    let outcomes = outcomes(capture, query_id);
    outcomes.len() == expected
        && outcomes
            .iter()
            .all(|outcome| matches!(outcome.status, OracleQueryStatus::Executed { .. }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OracleCatalogEvidenceStatus {
    Read,
    PartiallyRead,
    Unreadable,
    Unobserved,
}

fn reconcile_structure_catalog(
    structure: &mut dbwarp_blueprint_core::StructureScope,
    catalog: &str,
    status: OracleCatalogEvidenceStatus,
) {
    structure
        .catalogs_read
        .retain(|candidate| candidate != catalog);
    structure
        .catalogs_unreadable
        .retain(|candidate| candidate != catalog);
    structure
        .catalogs_not_applicable
        .retain(|candidate| candidate != catalog);
    match status {
        OracleCatalogEvidenceStatus::Read | OracleCatalogEvidenceStatus::PartiallyRead => {
            structure.catalogs_read.push(catalog.to_string())
        }
        OracleCatalogEvidenceStatus::Unreadable => {
            structure.catalogs_unreadable.push(catalog.to_string())
        }
        // A truncated capture may leave a family wholly unattempted. The
        // catalog-capture-truncated limitation records that fact; assigning
        // readability would invent evidence. A partial read is retained above
        // because its rows remain positive catalog provenance even though the
        // family completeness claim is withdrawn separately.
        OracleCatalogEvidenceStatus::Unobserved => {}
    }
}

fn query_catalog_evidence_status(
    capture: &OracleCatalogCapture,
    query_id: &str,
    expected: usize,
) -> OracleCatalogEvidenceStatus {
    let outcomes = outcomes(capture, query_id);
    let executed = outcomes
        .iter()
        .filter(|outcome| matches!(outcome.status, OracleQueryStatus::Executed { .. }))
        .count();
    if outcomes.len() == expected && executed == expected {
        return OracleCatalogEvidenceStatus::Read;
    }
    // Retain positive evidence whenever at least one intended owner query
    // completed. Family completeness is a separate decision, so a read for
    // one owner plus a denial or truncation for another is partially read,
    // not wholly unreadable.
    if executed > 0 {
        OracleCatalogEvidenceStatus::PartiallyRead
    } else if outcomes.iter().any(|outcome| {
        matches!(
            outcome.status,
            OracleQueryStatus::Failed { .. } | OracleQueryStatus::ResultDiscarded { .. }
        )
    }) {
        OracleCatalogEvidenceStatus::Unreadable
    } else {
        // NotReached (and the Basic-inapplicable skip statuses rejected by the
        // capture-shape validation) are not evidence that the catalogue was read or
        // unreadable.
        OracleCatalogEvidenceStatus::Unobserved
    }
}

struct OracleFloorSelection {
    capture: OracleCatalogCapture,
    owner_scope: OracleOwnerScope,
    truncated: bool,
}

fn owner_query_complete(
    capture: &OracleCatalogCapture,
    query_id: &str,
    owner_ordinal: u32,
) -> bool {
    outcomes(capture, query_id).into_iter().any(|outcome| {
        outcome.owner_ordinal == Some(owner_ordinal)
            && matches!(outcome.status, OracleQueryStatus::Executed { .. })
    })
}

fn floor_family_label(query_id: &str) -> &'static str {
    match query_id {
        "oracle-objects" => "table classification",
        "oracle-tables" => "table and row inventory",
        "oracle-tab-columns" => "column inventory",
        _ => "required catalogue inventory",
    }
}

/// Select the owners that satisfy the customer-useful Oracle floor. This is
/// the only catalogue-availability decision that can refuse a mapped capture.
fn select_oracle_floor(
    capture: &OracleCatalogCapture,
    owner_scope: &OracleOwnerScope,
    server_version: OracleVersion,
) -> Result<OracleFloorSelection> {
    let floor_families = CATALOG_QUERIES
        .iter()
        .filter(|query| query.tier == CatalogTier::Basic)
        .filter(|query| matches!(query.degradation, Degradation::Fatal))
        .filter(|query| query.applies_to(server_version.components()[0]))
        .collect::<Vec<_>>();
    if floor_families.is_empty()
        || floor_families
            .iter()
            .any(|query| query.owner_column.is_none())
    {
        bail!("The Oracle query set does not define an owner-scoped Blueprint floor");
    }

    let mut selected = Vec::<(u32, String)>::new();
    let mut missing = Vec::new();
    for (index, owner) in owner_scope.owners().iter().enumerate() {
        let ordinal = u32::try_from(index + 1).context("Oracle owner ordinal exceeds u32")?;
        let absent = floor_families
            .iter()
            .filter(|query| !owner_query_complete(capture, query.query_id, ordinal))
            .map(|query| floor_family_label(query.query_id))
            .collect::<Vec<_>>();
        if absent.is_empty() {
            selected.push((ordinal, owner.clone()));
        } else {
            missing.push(format!("owner-{ordinal}: {}", absent.join(", ")));
        }
    }
    if selected.is_empty() {
        bail!("Oracle Blueprint floor unavailable; {}", missing.join("; "));
    }

    let ordinal_map = selected
        .iter()
        .enumerate()
        .map(|(new_index, (old, _))| Ok::<_, anyhow::Error>((*old, u32::try_from(new_index + 1)?)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    let mut narrowed = capture.clone();
    narrowed.outcomes = capture
        .outcomes
        .iter()
        .filter_map(|outcome| match outcome.owner_ordinal {
            None => Some(outcome.clone()),
            Some(old) => ordinal_map.get(&old).map(|new| {
                let mut outcome = outcome.clone();
                outcome.owner_ordinal = Some(*new);
                outcome
            }),
        })
        .collect();
    let owners = selected.into_iter().map(|(_, owner)| owner).collect();
    let truncated = ordinal_map.len() != owner_scope.owners().len()
        || capture.abort.is_some()
        || owner_scope.omitted_unrepresentable_owners();
    Ok(OracleFloorSelection {
        capture: narrowed,
        owner_scope: owner_scope.capture_subset(owners),
        truncated,
    })
}

fn require_owner_query_complete(
    capture: &OracleCatalogCapture,
    query_id: &str,
    owners: usize,
) -> Result<()> {
    if !query_complete(capture, query_id, owners) {
        bail!("required Oracle catalog family '{query_id}' is incomplete");
    }
    Ok(())
}

fn optional_text(row: &OracleRow, index: usize) -> Option<String> {
    match row.values.get(index)? {
        OracleValue::Null => None,
        OracleValue::Number(value)
        | OracleValue::Text(value)
        | OracleValue::Timestamp(value)
        | OracleValue::LobText(value) => Some(value.trim().to_string()),
    }
    .filter(|value| !value.is_empty())
}

fn required_text(row: &OracleRow, index: usize, field: &str) -> Result<String> {
    optional_text(row, index).with_context(|| format!("{field} is null or missing"))
}

/// Oracle quoted identifiers may legitimately begin or end with spaces. Keep
/// their exact bytes for joins and anonymous ordering; token/numeric fields use
/// the trimming helpers above because catalog presentation padding is not
/// identity.
fn required_identifier(row: &OracleRow, index: usize, field: &str) -> Result<String> {
    match row.values.get(index) {
        Some(OracleValue::Number(value))
        | Some(OracleValue::Text(value))
        | Some(OracleValue::Timestamp(value))
        | Some(OracleValue::LobText(value))
            if !value.is_empty() =>
        {
            Ok(value.clone())
        }
        _ => bail!("{field} is null or missing"),
    }
}

fn optional_identifier(row: &OracleRow, index: usize) -> Option<String> {
    match row.values.get(index)? {
        OracleValue::Null => None,
        OracleValue::Number(value)
        | OracleValue::Text(value)
        | OracleValue::Timestamp(value)
        | OracleValue::LobText(value)
            if !value.is_empty() =>
        {
            Some(value.clone())
        }
        _ => None,
    }
}

fn optional_u64(row: &OracleRow, index: usize) -> Result<Option<u64>> {
    optional_text(row, index)
        .map(|value| parse_oracle_u64(&value))
        .transpose()
}

fn optional_i64(row: &OracleRow, index: usize) -> Result<Option<i64>> {
    optional_text(row, index)
        .map(|value| parse_oracle_i64(&value))
        .transpose()
}

fn parse_oracle_u64(value: &str) -> Result<u64> {
    let integral = oracle_integral_lexeme(value);
    integral
        .parse::<u64>()
        .with_context(|| format!("Oracle numeric value is not an unsigned integer: {value}"))
}

fn parse_oracle_i64(value: &str) -> Result<i64> {
    let integral = oracle_integral_lexeme(value);
    integral
        .parse::<i64>()
        .with_context(|| format!("Oracle numeric value is not an integer: {value}"))
}

fn oracle_integral_lexeme(value: &str) -> &str {
    // The adapter pins NLS_NUMERIC_CHARACTERS to `.,`, so catalogue numbers
    // reach the mapper with a dot decimal separator. A trailing comma-zero
    // group is ambiguous (`42,000` could be read as `42`), so reject it.
    // Every caller reads a semantically integral dictionary field. Accept an
    // Oracle spelling such as `42.000` only when its fractional part is all
    // zero; a genuine fraction is not rounded or truncated and therefore
    // fails the caller's integer parse explicitly.
    value
        .rmatch_indices('.')
        .next()
        .and_then(|(index, _)| {
            let fraction = &value[index + 1..];
            (!fraction.is_empty() && fraction.bytes().all(|byte| byte == b'0'))
                .then_some(&value[..index])
        })
        .unwrap_or(value)
}

fn required_u32(row: &OracleRow, index: usize, field: &str) -> Result<u32> {
    let value = optional_u64(row, index)?.with_context(|| format!("{field} is null or missing"))?;
    u32::try_from(value).with_context(|| format!("{field} exceeds u32"))
}

fn yes_no(row: &OracleRow, index: usize) -> Option<bool> {
    optional_text(row, index).and_then(|value| match value.to_ascii_uppercase().as_str() {
        "Y" | "YES" | "TRUE" | "ENABLED" => Some(true),
        "N" | "NO" | "FALSE" | "DISABLED" => Some(false),
        _ => None,
    })
}

fn required_yes_no(row: &OracleRow, index: usize, field: &str) -> Result<bool> {
    yes_no(row, index).with_context(|| format!("{field} is null, missing, or not yes/no"))
}

fn required_token_bool(
    row: &OracleRow,
    index: usize,
    field: &str,
    true_token: &str,
    false_token: &str,
) -> Result<bool> {
    let value = required_text(row, index, field)?;
    if value.eq_ignore_ascii_case(true_token) {
        Ok(true)
    } else if value.eq_ignore_ascii_case(false_token) {
        Ok(false)
    } else {
        bail!("{field} is outside its closed Oracle vocabulary")
    }
}

fn collect_object_facts(
    capture: &OracleCatalogCapture,
) -> Result<BTreeMap<(String, String), OracleObjectFacts>> {
    let mut objects = BTreeMap::new();
    for row in rows(capture, "oracle-objects") {
        let owner = required_identifier(row, 0, "Oracle object owner")?;
        let name = required_identifier(row, 1, "Oracle object name")?;
        let object_type = required_text(row, 2, "Oracle object type")?.to_ascii_uppercase();
        if !matches!(object_type.as_str(), "TABLE" | "MATERIALIZED VIEW") {
            continue;
        }
        let entry = objects
            .entry((owner, name))
            .or_insert_with(OracleObjectFacts::default);
        entry.object_types.insert(object_type);
        entry.temporary.observe(yes_no(row, 4));
        entry.generated.observe(yes_no(row, 5));
        entry.secondary.observe(yes_no(row, 6));
    }
    Ok(objects)
}

fn collect_oracle_maintained_owners(
    capture: &OracleCatalogCapture,
    expected_outcomes: usize,
) -> (BTreeSet<String>, bool) {
    let mut owners = BTreeSet::new();
    let mut classification_complete = true;
    for row in rows(capture, "oracle-users") {
        let maintained = yes_no(row, 1);
        if maintained.is_none() {
            // Pre-12c ALL_USERS lacks ORACLE_MAINTAINED. A successful query
            // without that classification is not evidence that every visible
            // owner is customer-managed.
            classification_complete = false;
        }
        if maintained == Some(true) {
            let Ok(owner) = required_identifier(row, 0, "Oracle maintained-owner identity") else {
                return (BTreeSet::new(), false);
            };
            owners.insert(owner);
        }
    }
    (
        owners,
        classification_complete && query_complete(capture, "oracle-users", expected_outcomes),
    )
}

/// Read Oracle-owned support storage by exact catalog identity. Prefixes such
/// as `MLOG$_` and `AQ$_` are not ownership evidence: customers can legally
/// use those names, while Oracle does not consistently mark its own support
/// tables generated or secondary.
fn collect_support_table_keys(
    capture: &OracleCatalogCapture,
    query_id: &str,
    include_aq_auxiliaries: bool,
    owner_ordinal: u32,
) -> Result<BTreeSet<NativeObjectKey>> {
    let mut tables = BTreeSet::new();
    for row in owner_rows(capture, query_id, owner_ordinal) {
        let owner = required_identifier(row, 0, "Oracle support-table owner")?;
        let table = required_identifier(row, 1, "Oracle support-table name")?;
        let key = (owner.clone(), table.clone());
        if !tables.insert(key) {
            bail!("Oracle support-table catalog returned a duplicate identity");
        }
        if include_aq_auxiliaries {
            // DBA_QUEUE_TABLES is the authority that this is an AQ-owned
            // table. Oracle documents the related dequeue/history IOT and
            // subscriber/log storage names as deterministic derivatives of
            // that catalog value. Derive only those exact identities; never
            // exclude an arbitrary customer table merely because it starts
            // with AQ$.
            for suffix in ["_C", "_D", "_G", "_H", "_I", "_L", "_P", "_R", "_S", "_T"] {
                tables.insert((owner.clone(), format!("AQ$_{table}{suffix}")));
            }
        }
    }
    Ok(tables)
}

fn collect_mview_update_candidates(
    capture: &OracleCatalogCapture,
    owner_ordinal: u32,
) -> Result<BTreeSet<NativeObjectKey>> {
    let mut tables = BTreeSet::new();
    for row in owner_rows(capture, "oracle-mview-logs", owner_ordinal) {
        let owner = required_identifier(row, 0, "Oracle support-table owner")?;
        // Validate the exact log identity as well so this helper cannot make a
        // malformed four-column catalog row look complete.
        required_identifier(row, 1, "Oracle support-table name")?;
        let master = required_identifier(row, 2, "Oracle materialized-view log master")?;
        let primary_key = required_yes_no(row, 3, "Oracle primary-key log flag")?;
        if primary_key {
            tables.insert((owner, format!("RUPD$_{master}")));
        }
    }
    Ok(tables)
}

fn collect_tables(capture: &OracleCatalogCapture) -> Result<Vec<RawTable>> {
    collect_table_family(capture, "oracle-tables", false)
}

fn collect_object_tables(
    capture: &OracleCatalogCapture,
    owner_ordinal: u32,
) -> Result<Vec<RawTable>> {
    collect_table_family_for_owner(capture, "oracle-object-tables", true, owner_ordinal)
}

fn collect_table_family(
    capture: &OracleCatalogCapture,
    query_id: &str,
    object_table: bool,
) -> Result<Vec<RawTable>> {
    collect_table_rows(rows(capture, query_id), object_table)
}

fn collect_table_family_for_owner(
    capture: &OracleCatalogCapture,
    query_id: &str,
    object_table: bool,
    owner_ordinal: u32,
) -> Result<Vec<RawTable>> {
    collect_table_rows(owner_rows(capture, query_id, owner_ordinal), object_table)
}

fn collect_table_rows(rows: Vec<&OracleRow>, object_table: bool) -> Result<Vec<RawTable>> {
    rows.into_iter()
        .map(|row| {
            let partitioned = yes_no(row, 7);
            let temporary = yes_no(row, 8);
            let nested = yes_no(row, 10);
            let secondary = yes_no(row, 13);
            let external = yes_no(row, 14);
            let dropped = yes_no(row, 16);
            let segment_created = yes_no(row, 17);
            Ok(RawTable {
                owner: required_identifier(row, 0, "Oracle table owner")?,
                name: required_identifier(row, 1, "Oracle table name")?,
                rows: optional_u64(row, 3).unwrap_or(None),
                avg_row_len: optional_u64(row, 5).unwrap_or(None),
                last_analyzed: optional_text(row, 6),
                partitioned: partitioned.unwrap_or(false),
                temporary: temporary.unwrap_or(false),
                iot_type: optional_text(row, 9),
                nested,
                iot_name: optional_identifier(row, 12),
                secondary: secondary.unwrap_or(false),
                external: external.unwrap_or(false),
                object_table,
                clustered: optional_text(row, 15).is_some(),
                dropped: dropped.unwrap_or(false),
                segment_created,
                classification_complete: [
                    partitioned,
                    temporary,
                    nested,
                    secondary,
                    external,
                    dropped,
                ]
                .iter()
                .all(Option::is_some),
            })
        })
        .collect()
}

fn collect_columns(
    capture: &OracleCatalogCapture,
) -> Result<BTreeMap<(String, String), Vec<OracleRow>>> {
    let mut columns = BTreeMap::new();
    for row in rows(capture, "oracle-tab-columns") {
        let key = (
            required_identifier(row, 0, "Oracle column owner")?,
            required_identifier(row, 1, "Oracle column table")?,
        );
        columns
            .entry(key)
            .or_insert_with(Vec::new)
            .push(row.clone());
    }
    for table_columns in columns.values_mut() {
        table_columns.sort_by_key(|row| optional_u64(row, 3).ok().flatten().unwrap_or(u64::MAX));
    }
    Ok(columns)
}

fn collect_column_catalog_facts(
    capture: &OracleCatalogCapture,
    owner_ordinal: u32,
) -> Result<BTreeMap<(String, String, String), ColumnCatalogFacts>> {
    let mut hidden = BTreeMap::new();
    for row in owner_rows(capture, "oracle-tab-cols-hidden", owner_ordinal) {
        let catalog_hidden = yes_no(row, 3);
        let user_generated = yes_no(row, 6);
        let previous = hidden.insert(
            (
                required_identifier(row, 0, "Oracle hidden-column owner")?,
                required_identifier(row, 1, "Oracle hidden-column table")?,
                required_identifier(row, 2, "Oracle hidden-column name")?,
            ),
            ColumnCatalogFacts {
                hidden: match (catalog_hidden, user_generated) {
                    (Some(true), Some(false)) => Some(true),
                    (Some(false), _) => Some(false),
                    _ => None,
                },
                invisible: match (catalog_hidden, user_generated) {
                    (Some(true), Some(true)) => Some(true),
                    (Some(false), _) => Some(false),
                    _ => None,
                },
                generated_virtual: yes_no(row, 4),
                internal_ordinal: optional_u64(row, 5).unwrap_or(None),
            },
        );
        if previous.is_some() {
            bail!("Oracle hidden-column catalog returned a duplicate column");
        }
    }
    Ok(hidden)
}

fn build_column_ordinals(
    columns: &BTreeMap<(String, String), Vec<OracleRow>>,
    facts: &BTreeMap<(String, String, String), ColumnCatalogFacts>,
) -> Result<ColumnOrdinalsWithGaps> {
    let mut result = BTreeMap::new();
    let mut gaps = BTreeSet::new();
    for ((owner, table), rows) in columns {
        let mut ordered = Vec::with_capacity(rows.len());
        for row in rows {
            let name = required_identifier(row, 2, "Oracle column name")?;
            let key = (owner.clone(), table.clone(), name.clone());
            let visible = optional_u64(row, 3).unwrap_or(None);
            let internal = facts.get(&key).and_then(|fact| fact.internal_ordinal);
            let Some(stable_position) = visible.or(internal) else {
                gaps.insert((owner.clone(), table.clone()));
                continue;
            };
            // Current visible COLUMN_ID is authoritative for user-visible
            // ordering. INTERNAL_COLUMN_ID is only the fallback for an
            // invisible column whose COLUMN_ID is null; using it for every
            // column misorders a column made visible again.
            ordered.push((visible.is_none(), stable_position, internal, name));
        }
        ordered.sort();
        if ordered
            .windows(2)
            .any(|pair| pair[0].0 == pair[1].0 && pair[0].1 == pair[1].1)
        {
            gaps.insert((owner.clone(), table.clone()));
        }
        for (index, (_, _, _, name)) in ordered.into_iter().enumerate() {
            let ordinal = u32::try_from(index + 1).context("too many Oracle columns")?;
            result.insert((owner.clone(), table.clone(), name), ordinal);
        }
    }
    Ok((result, gaps))
}

fn collect_statistics_facts(capture: &OracleCatalogCapture) -> CollectedOracleStatisticsFacts {
    let mut result = BTreeMap::new();
    let mut duplicate_tables = BTreeSet::new();
    let mut unscoped_malformed = false;
    for row in rows(capture, "oracle-tab-statistics") {
        // Partition rows support the partition maximum elsewhere. The table
        // record's freshness and sample evidence comes from the global row;
        // mixing one leaf's state into the whole table would be misleading.
        if optional_text(row, 2).is_some() {
            continue;
        }
        let key = match (
            required_identifier(row, 0, "Oracle statistics owner"),
            required_identifier(row, 1, "Oracle statistics table"),
        ) {
            (Ok(owner), Ok(table)) => (owner, table),
            _ => {
                // Without a native key the bad row cannot be attributed to a
                // single table. Preserve all independently parsed rows, but
                // withdraw the family-wide completeness claim.
                unscoped_malformed = true;
                continue;
            }
        };
        if duplicate_tables.contains(&key) {
            continue;
        }
        let parsed = (|| {
            Ok::<_, anyhow::Error>(OracleStatisticsFacts {
                last_analyzed: optional_text(row, 6),
                stale: yes_no(row, 7),
                sample_size: optional_u64(row, 9)?,
                global: yes_no(row, 10),
                user_supplied: yes_no(row, 11),
                locked: optional_text(row, 12).is_some(),
                scope: optional_text(row, 13)
                    .map(|scope| match scope.to_ascii_uppercase().as_str() {
                        "SESSION" => "session",
                        "SHARED" => "global",
                        _ => "unknown",
                    })
                    .unwrap_or("unknown")
                    .to_string(),
            })
        })();
        let facts = match parsed {
            Ok(facts) => facts,
            Err(_) => {
                // The owner/table key is sound, so degrade only this table.
                // A malformed sample-size cell must not erase current
                // statistics already parsed for unrelated objects.
                result.remove(&key);
                duplicate_tables.insert(key);
                continue;
            }
        };
        let previous = result.insert(key.clone(), facts);
        if previous.is_some() {
            result.remove(&key);
            duplicate_tables.insert(key);
        }
    }
    CollectedOracleStatisticsFacts {
        facts: result,
        invalid_tables: duplicate_tables,
        unscoped_malformed,
    }
}

fn table_statistics_catalog_complete(
    query_complete: bool,
    unscoped_malformed: bool,
    invalid_tables: &BTreeSet<(String, String)>,
    table: &(String, String),
) -> bool {
    query_complete && !unscoped_malformed && !invalid_tables.contains(table)
}

fn sample_fraction_band(rows: Option<u64>, sample_size: Option<u64>) -> &'static str {
    let Some((rows, sample_size)) = rows.zip(sample_size) else {
        return "unknown";
    };
    if rows == 0 {
        return "unknown";
    }
    // A u64 multiplied by 100 is always representable in u128, and rows is
    // proven nonzero above. Keep the calculation exact rather than using a
    // saturating fallback that could make corrupted arithmetic look valid.
    let percentage = (u128::from(sample_size) * 100) / u128::from(rows);
    match percentage {
        100.. => "full",
        75..=99 => "75-99pct",
        50..=74 => "50-74pct",
        25..=49 => "25-49pct",
        _ => "under-25pct",
    }
}

fn collect_identity_columns(
    capture: &OracleCatalogCapture,
    owner_ordinal: u32,
) -> Result<BTreeMap<(String, String, String), String>> {
    let mut identities = BTreeMap::new();
    for row in owner_rows(capture, "oracle-identity-columns", owner_ordinal) {
        let generation = required_text(row, 3, "Oracle identity generation")?;
        let value_source = if generation.to_ascii_uppercase().contains("ALWAYS") {
            "identity-always"
        } else {
            "identity-default"
        };
        let previous = identities.insert(
            (
                required_identifier(row, 0, "Oracle identity owner")?,
                required_identifier(row, 1, "Oracle identity table")?,
                required_identifier(row, 2, "Oracle identity column")?,
            ),
            value_source.to_string(),
        );
        if previous.is_some() {
            bail!("Oracle identity catalog returned a duplicate column");
        }
    }
    Ok(identities)
}

fn collect_indexes(capture: &OracleCatalogCapture, owner_ordinal: u32) -> Result<Vec<RawIndex>> {
    owner_rows(capture, "oracle-indexes", owner_ordinal)
        .into_iter()
        .map(|row| {
            Ok(RawIndex {
                owner: required_identifier(row, 0, "Oracle index owner")?,
                name: required_identifier(row, 1, "Oracle index name")?,
                table_owner: required_identifier(row, 2, "Oracle index table owner")?,
                table_name: required_identifier(row, 3, "Oracle index table")?,
                index_type: required_text(row, 4, "Oracle index type")?,
                unique: required_token_bool(
                    row,
                    5,
                    "Oracle index uniqueness",
                    "UNIQUE",
                    "NONUNIQUE",
                )?,
                partitioned: required_yes_no(row, 11, "Oracle index partitioned flag")?,
                status: optional_text(row, 12).unwrap_or_else(|| "unknown".to_string()),
                generated: required_yes_no(row, 13, "Oracle generated-index flag")?,
                secondary: required_yes_no(row, 14, "Oracle secondary-index flag")?,
                visibility: optional_text(row, 15).unwrap_or_else(|| "unknown".to_string()),
                join_index: required_yes_no(row, 16, "Oracle bitmap-join flag")?,
                // Non-partitioned indexes report YES/NO. Oracle reports N/A
                // for a partitioned index because allocation is decided per
                // leaf; preserve that as unknown instead of inventing a
                // capture-wide boolean.
                segment_created: yes_no(row, 17),
            })
        })
        // Oracle exposes each LOB's physical support index in DBA_INDEXES.
        // It is storage evidence for the owning column, not a logical index a
        // migration should recreate independently, and DBA_IND_COLUMNS has no
        // user key for it.
        .filter_map(|index| match index {
            Ok(index) if index.index_type.eq_ignore_ascii_case("LOB") => None,
            other => Some(other),
        })
        .collect()
}

fn collect_index_columns(
    capture: &OracleCatalogCapture,
    ordinal_by_name: &BTreeMap<(String, String, String), u32>,
    owner_ordinal: u32,
) -> Result<IndexColumnsByIndex> {
    let mut indexes = BTreeMap::new();
    for row in owner_rows(capture, "oracle-index-columns", owner_ordinal) {
        let owner = required_identifier(row, 0, "Oracle index-column owner")?;
        let index = required_identifier(row, 1, "Oracle index-column index")?;
        let table = required_identifier(row, 2, "Oracle index-column table")?;
        let column = required_identifier(row, 3, "Oracle index-column name")?;
        let position =
            optional_u64(row, 4)?.context("Oracle index-column position is unavailable")?;
        let table_owner = required_identifier(row, 6, "Oracle index-column table owner")?;
        let ordinal = ordinal_by_name.get(&(table_owner, table, column)).copied();
        let descending =
            optional_text(row, 5).is_some_and(|value| value.eq_ignore_ascii_case("DESC"));
        indexes
            .entry((owner, index))
            .or_insert_with(Vec::new)
            .push(RawIndexColumn {
                position,
                ordinal,
                descending,
            });
    }
    for values in indexes.values() {
        let mut positions = values
            .iter()
            .map(|column| column.position)
            .collect::<Vec<_>>();
        positions.sort_unstable();
        if positions.windows(2).any(|pair| pair[0] == pair[1]) {
            bail!("Oracle index catalog returned duplicate key positions");
        }
    }
    Ok(indexes)
}

fn collect_expression_index_facts(
    capture: &OracleCatalogCapture,
    owner_ordinal: u32,
) -> ExpressionIndexFacts {
    let mut facts = ExpressionIndexFacts::default();
    for row in owner_rows(capture, "oracle-index-expressions", owner_ordinal) {
        let (Ok(owner), Ok(index)) = (
            required_identifier(row, 0, "Oracle expression-index owner"),
            required_identifier(row, 1, "Oracle expression-index name"),
        ) else {
            facts.unscoped_malformed = true;
            continue;
        };
        let key = (owner.clone(), index.clone());
        facts.indexes.insert(key.clone());
        match optional_u64(row, 2) {
            Ok(Some(position)) => {
                facts.columns.insert((owner, index, position));
            }
            Ok(None) | Err(_) => {
                facts.invalid_indexes.insert(key);
            }
        }
    }
    facts
}

fn collect_constraints(
    capture: &OracleCatalogCapture,
    owner_ordinal: u32,
) -> Result<Vec<RawConstraint>> {
    owner_rows(capture, "oracle-constraints", owner_ordinal)
        .into_iter()
        .map(|row| {
            let constraint_type = required_text(row, 2, "Oracle constraint type")?;
            let delete_rule = if constraint_type.eq_ignore_ascii_case("R") {
                required_text(row, 6, "Oracle foreign-key delete rule")?
            } else {
                optional_text(row, 6).unwrap_or_else(|| "NO ACTION".to_string())
            };
            let status = required_text(row, 7, "Oracle constraint status")?;
            if !matches!(status.to_ascii_uppercase().as_str(), "ENABLED" | "DISABLED") {
                bail!("Oracle constraint status is outside its closed vocabulary");
            }
            Ok(RawConstraint {
                owner: required_identifier(row, 0, "Oracle constraint owner")?,
                name: required_identifier(row, 1, "Oracle constraint name")?,
                constraint_type,
                table_name: required_identifier(row, 3, "Oracle constraint table")?,
                referenced_owner: optional_identifier(row, 4),
                referenced_constraint: optional_identifier(row, 5),
                delete_rule,
                status,
                deferrable: required_token_bool(
                    row,
                    8,
                    "Oracle constraint deferrability",
                    "DEFERRABLE",
                    "NOT DEFERRABLE",
                )?,
                validated: required_token_bool(
                    row,
                    9,
                    "Oracle constraint validation",
                    "VALIDATED",
                    "NOT VALIDATED",
                )?,
                initially_deferred: required_token_bool(
                    row,
                    10,
                    "Oracle constraint deferred state",
                    "DEFERRED",
                    "IMMEDIATE",
                )?,
                index_owner: optional_identifier(row, 11),
                index_name: optional_identifier(row, 12),
                generated_name: required_token_bool(
                    row,
                    13,
                    "Oracle constraint name provenance",
                    "GENERATED NAME",
                    "USER NAME",
                )?,
                is_not_null_constraint: yes_no(row, 14),
            })
        })
        .collect()
}

fn collect_constraint_columns(
    capture: &OracleCatalogCapture,
    ordinal_by_name: &BTreeMap<(String, String, String), u32>,
    constraints_by_key: &BTreeMap<NativeObjectKey, &RawConstraint>,
    owner_ordinal: u32,
) -> Result<(
    BTreeMap<(String, String), Vec<u32>>,
    BTreeSet<NativeObjectKey>,
    BTreeSet<NativeObjectKey>,
)> {
    let mut result = BTreeMap::<_, Vec<(u64, u32)>>::new();
    let mut unresolved_tables = BTreeSet::new();
    let mut invalid_constraints = BTreeSet::new();
    for row in owner_rows(capture, "oracle-constraint-columns", owner_ordinal) {
        let owner = required_identifier(row, 0, "Oracle constraint-column owner")?;
        let constraint = required_identifier(row, 1, "Oracle constraint-column constraint")?;
        let constraint_facts = constraints_by_key
            .get(&(owner.clone(), constraint.clone()))
            .context("Oracle constraint-column row has no matching constraint")?;
        // Oracle exposes column rows for CHECK constraints as well, including
        // the system-generated CHECK used for NOT NULL. Their POSITION is
        // legitimately null because CHECK conditions have no ordered key.
        // Only primary, unique and foreign keys contribute ordered columns to
        // the relationship model, so do not turn valid CHECK metadata into a
        // capture-wide relationship/index degradation.
        if !matches!(constraint_facts.constraint_type.as_str(), "P" | "U" | "R") {
            continue;
        }
        let table = required_identifier(row, 2, "Oracle constraint-column table")?;
        let column = required_identifier(row, 3, "Oracle constraint-column name")?;
        let position =
            optional_u64(row, 4)?.context("Oracle constraint-column position is unavailable")?;
        let ordinal = ordinal_by_name
            .get(&(owner.clone(), table.clone(), column))
            .copied();
        // Object tables receive an engine-generated OID uniqueness constraint
        // over SYS_NC_OID$, a hidden system column deliberately absent from
        // the user-column inventory. It is recreated by Oracle with the
        // object table and must not make the customer's relationship census
        // incomplete. A missing column on a foreign key or a user-named key
        // remains a real integrity failure.
        let Some(ordinal) = ordinal else {
            if constraint_facts.generated_name
                && matches!(constraint_facts.constraint_type.as_str(), "P" | "U")
            {
                // Oracle object tables can have generated keys over hidden
                // columns. They are not user-visible structure, but a foreign
                // key that references such a key cannot be represented
                // safely. Invalidate the whole key instead of retaining a
                // truncated prefix that could point at the wrong column.
                invalid_constraints.insert((owner, constraint));
                continue;
            }
            // Object-attribute keys and other unrepresentable column paths do
            // not invalidate unrelated relationships. Preserve every other
            // constraint and mark only this table unresolved through the
            // completeness calculation above.
            unresolved_tables.insert((constraint_facts.owner.clone(), table));
            invalid_constraints.insert((owner, constraint));
            continue;
        };
        result
            .entry((owner, constraint))
            .or_default()
            .push((position, ordinal));
    }
    result.retain(|key, _| !invalid_constraints.contains(key));
    let columns = result
        .into_iter()
        .map(|(key, mut values)| {
            values.sort_by_key(|(position, _)| *position);
            if values.windows(2).any(|pair| pair[0].0 == pair[1].0) {
                return Err(anyhow::anyhow!(
                    "Oracle constraint catalog returned duplicate key positions"
                ));
            }
            Ok((
                key,
                values.into_iter().map(|(_, ordinal)| ordinal).collect(),
            ))
        })
        .collect::<Result<_>>()?;
    Ok((columns, unresolved_tables, invalid_constraints))
}

fn collect_partition_facts(
    capture: &OracleCatalogCapture,
    ordinal_by_name: &BTreeMap<(String, String, String), u32>,
    owner_ordinal: u32,
) -> Result<BTreeMap<(String, String), PartitionFacts>> {
    let mut result = BTreeMap::new();
    for row in owner_rows(capture, "oracle-part-tables", owner_ordinal) {
        let owner = required_identifier(row, 0, "Oracle partition owner")?;
        let table = required_identifier(row, 1, "Oracle partition table")?;
        let primary = required_text(row, 2, "Oracle partition strategy")?;
        let sub = optional_text(row, 3).unwrap_or_else(|| "NONE".to_string());
        let interval = optional_text(row, 5).is_some();
        let strategy = if !sub.eq_ignore_ascii_case("NONE") {
            "composite"
        } else if interval {
            "interval"
        } else {
            match primary.to_ascii_uppercase().as_str() {
                "RANGE" => "range",
                "LIST" => "list",
                "HASH" => "hash",
                "SYSTEM" => "system",
                "REFERENCE" => "reference",
                _ => "unknown",
            }
        };
        let previous = result.insert(
            (owner, table),
            PartitionFacts {
                strategy: strategy.to_string(),
                ..PartitionFacts::default()
            },
        );
        if previous.is_some() {
            bail!("Oracle partition catalog returned a duplicate table");
        }
    }
    let mut keys = BTreeMap::<_, Vec<(u64, u32)>>::new();
    for row in owner_rows(capture, "oracle-part-key-columns", owner_ordinal) {
        if optional_text(row, 2).is_some_and(|value| !value.eq_ignore_ascii_case("TABLE")) {
            continue;
        }
        let owner = required_identifier(row, 0, "Oracle partition-key owner")?;
        let table = required_identifier(row, 1, "Oracle partition-key table")?;
        let column = required_identifier(row, 3, "Oracle partition-key column")?;
        let position =
            optional_u64(row, 4)?.context("Oracle partition-key position is unavailable")?;
        let ordinal = ordinal_by_name
            .get(&(owner.clone(), table.clone(), column))
            .copied()
            .context("Oracle partition key is absent from the column inventory")?;
        keys.entry((owner, table))
            .or_default()
            .push((position, ordinal));
    }
    for (key, mut values) in keys {
        values.sort_by_key(|(position, _)| *position);
        if values.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            bail!("Oracle partition catalog returned duplicate key positions");
        }
        if let Some(facts) = result.get_mut(&key) {
            facts.key_ordinals = values.into_iter().map(|(_, ordinal)| ordinal).collect();
        }
    }
    for row in owner_rows(capture, "oracle-tab-partitions", owner_ordinal) {
        let key = (
            required_identifier(row, 0, "Oracle table-partition owner")?,
            required_identifier(row, 1, "Oracle table-partition table")?,
        );
        if let Some(facts) = result.get_mut(&key) {
            facts.top_partition_rows.push(optional_u64(row, 4)?);
        }
    }
    for row in owner_rows(capture, "oracle-tab-subpartitions", owner_ordinal) {
        let key = (
            required_identifier(row, 0, "Oracle table-subpartition owner")?,
            required_identifier(row, 1, "Oracle table-subpartition table")?,
        );
        if let Some(facts) = result.get_mut(&key) {
            facts.subpartition_rows.push(optional_u64(row, 5)?);
        }
    }
    for facts in result.values_mut() {
        let physical_rows = if facts.strategy == "composite" {
            &facts.subpartition_rows
        } else {
            &facts.top_partition_rows
        };
        facts.count =
            Some(u64::try_from(physical_rows.len()).context("too many Oracle partitions")?);
        if !physical_rows.is_empty() && physical_rows.iter().all(Option::is_some) {
            facts.rows_max = physical_rows.iter().flatten().copied().max();
        }
    }
    Ok(result)
}

fn collect_lob_facts(
    capture: &OracleCatalogCapture,
    owner_ordinal: u32,
) -> Result<BTreeMap<(String, String, String), LobFacts>> {
    let mut result = BTreeMap::new();
    for row in owner_rows(capture, "oracle-lobs", owner_ordinal) {
        let previous = result.insert(
            (
                required_identifier(row, 0, "Oracle LOB owner")?,
                required_identifier(row, 1, "Oracle LOB table")?,
                required_identifier(row, 2, "Oracle LOB column")?,
            ),
            LobFacts {
                storage_class: if required_yes_no(row, 7, "Oracle SecureFile flag")? {
                    "securefile"
                } else {
                    "basicfile"
                }
                .to_string(),
                in_row: yes_no(row, 6),
                compression: match optional_text(row, 8)
                    .map(|value| value.to_ascii_uppercase())
                    .as_deref()
                {
                    Some("LOW") => "low",
                    Some("MEDIUM") => "medium",
                    Some("HIGH") => "high",
                    Some("NONE" | "NO") => "none",
                    _ => "unknown",
                }
                .to_string(),
                deduplication: match optional_text(row, 9)
                    .map(|value| value.to_ascii_uppercase())
                    .as_deref()
                {
                    Some("YES") => "enabled",
                    Some("NO" | "NONE") => "disabled",
                    _ => "unknown",
                }
                .to_string(),
                encrypted: yes_no(row, 10),
                segment_name: required_identifier(row, 3, "Oracle LOB segment")?,
                index_name: required_identifier(row, 4, "Oracle LOB index")?,
            },
        );
        if previous.is_some() {
            bail!("Oracle LOB catalog returned a duplicate column");
        }
    }
    Ok(result)
}

fn collect_nested_table_parents(
    capture: &OracleCatalogCapture,
    owner_ordinal: u32,
) -> Result<BTreeMap<(String, String), (String, String)>> {
    let mut parents = BTreeMap::new();
    for row in owner_rows(capture, "oracle-nested-tables", owner_ordinal) {
        let owner = required_identifier(row, 0, "Oracle nested-table owner")?;
        let storage = required_identifier(row, 1, "Oracle nested storage table")?;
        let parent = required_identifier(row, 2, "Oracle nested parent table")?;
        let _column = required_identifier(row, 3, "Oracle nested parent column")?;
        if parents
            .insert((owner.clone(), storage), (owner, parent))
            .is_some()
        {
            bail!("Oracle nested-table catalog returned duplicate storage");
        }
    }
    Ok(parents)
}

fn build_segment_attribution_lookup(
    tables: &[RawTable],
    indexes: &[RawIndex],
) -> SegmentAttributionLookup {
    let index_to_table = indexes
        .iter()
        .map(|index| {
            (
                (index.owner.clone(), index.name.clone()),
                (index.table_owner.clone(), index.table_name.clone()),
            )
        })
        .collect();
    let iot_top_indexes = indexes
        .iter()
        .filter(|index| index.index_type.eq_ignore_ascii_case("IOT - TOP"))
        .map(|index| (index.owner.clone(), index.name.clone()))
        .collect();
    let iot_overflow_to_parent = tables
        .iter()
        .filter_map(|table| {
            table
                .iot_type
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case("IOT_OVERFLOW"))
                .then(|| {
                    table.iot_name.as_ref().map(|parent| {
                        (
                            (table.owner.clone(), table.name.clone()),
                            (table.owner.clone(), parent.clone()),
                        )
                    })
                })
                .flatten()
        })
        .collect();
    SegmentAttributionLookup {
        index_to_table,
        iot_top_indexes,
        iot_overflow_to_parent,
    }
}

fn collect_segment_bytes(
    capture: &OracleCatalogCapture,
    lob_facts: &BTreeMap<(String, String, String), LobFacts>,
    nested_table_parents: &BTreeMap<(String, String), (String, String)>,
    lookup: &SegmentAttributionLookup,
    tables: &[RawTable],
    indexes: &[RawIndex],
    owner_ordinal: u32,
    expected_owner: &str,
) -> Result<CollectedSegmentBytes> {
    let mut lob_segment_to_table = BTreeMap::new();
    let mut lob_index_to_table = BTreeMap::new();
    for ((owner, table, _column), facts) in lob_facts
        .iter()
        .filter(|((owner, _, _), _)| owner == expected_owner)
    {
        let logical_table =
            resolve_nested_parent((owner.clone(), table.clone()), nested_table_parents)?;
        lob_segment_to_table.insert(
            (owner.clone(), facts.segment_name.clone()),
            logical_table.clone(),
        );
        lob_index_to_table.insert((owner.clone(), facts.index_name.clone()), logical_table);
    }
    let mut collected = CollectedSegmentBytes::default();
    let mut observed_index_segments = BTreeSet::new();
    let mut observed_direct_table_segments = BTreeSet::new();
    for row in owner_rows(capture, "oracle-segments", owner_ordinal) {
        let owner = required_identifier(row, 0, "Oracle segment owner")?;
        let segment_name = required_identifier(row, 1, "Oracle segment name")?;
        let segment_key = (owner.clone(), segment_name.clone());
        let segment_type = required_text(row, 2, "Oracle segment type")?.to_ascii_uppercase();
        // Parse the byte cell after resolving its logical table. A malformed
        // contribution withdraws completeness for that table (or the owner
        // when no mapping row survived) without discarding valid bytes already
        // attributed from other segment rows.
        let bytes = optional_u64(row, 4).ok().flatten();
        if segment_type.starts_with("LOBINDEX")
            || (segment_type.starts_with("INDEX")
                && lob_index_to_table.contains_key(&(owner.clone(), segment_name.clone())))
        {
            if let Some(table) = lob_index_to_table.get(&(owner.clone(), segment_name)) {
                if let Some(bytes) = bytes {
                    checked_add_bytes(&mut collected.index_bytes, table.clone(), bytes)?;
                } else {
                    collected.incomplete_tables.insert(table.clone());
                }
            } else {
                collected.lob_attribution_incomplete = true;
            }
        } else if segment_type.starts_with("LOBSEGMENT")
            || segment_type == "LOB PARTITION"
            || segment_type == "LOB SUBPARTITION"
        {
            if let Some(table) = lob_segment_to_table.get(&(owner.clone(), segment_name)) {
                if let Some(bytes) = bytes {
                    checked_add_bytes(&mut collected.table_bytes, table.clone(), bytes)?;
                } else {
                    collected.incomplete_tables.insert(table.clone());
                }
            } else {
                collected.lob_attribution_incomplete = true;
            }
        } else if segment_type == "NESTED TABLE" {
            if let Some(table) = nested_table_parents.get(&segment_key) {
                let table = resolve_nested_parent(table.clone(), nested_table_parents)?;
                if let Some(bytes) = bytes {
                    checked_add_bytes(&mut collected.table_bytes, table, bytes)?;
                } else {
                    collected.incomplete_tables.insert(table);
                }
            } else {
                collected.unmapped_nested_segments = true;
            }
        } else if segment_type.starts_with("INDEX") {
            let Some(table) = lookup.index_to_table.get(&segment_key) else {
                // The logical index inventory is the snapshot boundary for
                // index storage. A segment with no index identity was either
                // created after that read or belongs to an out-of-scope
                // table, so it is not attributed to any table in this
                // Blueprint. Catalog denials and malformed index rows are
                // tracked independently and still degrade affected tables.
                continue;
            };
            observed_index_segments.insert(segment_key.clone());
            let table = resolve_nested_parent(table.clone(), nested_table_parents)?;
            if let Some(bytes) = bytes {
                if lookup.iot_top_indexes.contains(&segment_key) {
                    observed_direct_table_segments.insert(table.clone());
                    checked_add_bytes(&mut collected.table_bytes, table, bytes)?;
                } else {
                    checked_add_bytes(&mut collected.index_bytes, table, bytes)?;
                }
            } else {
                collected.incomplete_tables.insert(table);
            }
        } else if segment_type.starts_with("TABLE") || segment_type.starts_with("IOT") {
            let table = lookup
                .iot_overflow_to_parent
                .get(&segment_key)
                .cloned()
                .or_else(|| nested_table_parents.get(&segment_key).cloned())
                .unwrap_or(segment_key);
            let table = resolve_nested_parent(table, nested_table_parents)?;
            observed_direct_table_segments.insert(table.clone());
            if let Some(bytes) = bytes {
                checked_add_bytes(&mut collected.table_bytes, table, bytes)?;
            } else {
                collected.incomplete_tables.insert(table);
            }
        }
    }
    // DBA_TABLES.SEGMENT_CREATED is the authority for non-partitioned table
    // allocation. An absent segment row is a proven zero only when Oracle
    // says creation is deferred. YES (or an unreadable flag) without an
    // attributed contribution is a catalogue gap. Partitioned parents report
    // N/A because allocation belongs to their physical leaves.
    for table in tables
        .iter()
        .filter(|table| table.owner == expected_owner && !table.partitioned)
    {
        let table_key = (table.owner.clone(), table.name.clone());
        if table.segment_created != Some(false)
            && !observed_direct_table_segments.contains(&table_key)
            && !table.clustered
        {
            collected.incomplete_tables.insert(table_key);
        }
    }
    // A non-partitioned index for which Oracle says SEGMENT_CREATED=YES must
    // have a matching segment contribution. Its absence is a catalogue race,
    // not a measured zero. Conversely, deferred segment creation is an
    // explicit proof that the missing row represents no allocation. A
    // partitioned index reports N/A at the parent and cannot be checked by
    // this rule; its physical leaves are still summed from DBA_SEGMENTS.
    // LOB/domain/secondary indexes use their own storage mappings and do not
    // require a same-name INDEX segment.
    for index in indexes.iter().filter(|index| {
        index.owner == expected_owner
            && !index.partitioned
            && index.segment_created != Some(false)
            && !index.secondary
            && !matches!(
                index.index_type.to_ascii_uppercase().as_str(),
                "LOB" | "DOMAIN"
            )
    }) {
        let index_key = (index.owner.clone(), index.name.clone());
        if !observed_index_segments.contains(&index_key) {
            collected.incomplete_tables.insert(resolve_nested_parent(
                (index.table_owner.clone(), index.table_name.clone()),
                nested_table_parents,
            )?);
        }
    }
    Ok(collected)
}

fn merge_segment_bytes(
    target: &mut SegmentBytesByObject,
    source: SegmentBytesByObject,
) -> BTreeSet<NativeObjectKey> {
    let mut incomplete = BTreeSet::new();
    for (table, bytes) in source {
        if checked_add_bytes(target, table.clone(), bytes).is_err() {
            incomplete.insert(table);
        }
    }
    incomplete
}

fn resolve_nested_parent(
    mut table: NativeObjectKey,
    parents: &BTreeMap<NativeObjectKey, NativeObjectKey>,
) -> Result<NativeObjectKey> {
    let mut seen = BTreeSet::new();
    while let Some(parent) = parents.get(&table) {
        if !seen.insert(table.clone()) {
            bail!("Oracle nested-table parent catalog contains a cycle");
        }
        table = parent.clone();
    }
    Ok(table)
}

fn checked_add_bytes(
    values: &mut BTreeMap<(String, String), u64>,
    key: (String, String),
    bytes: u64,
) -> Result<()> {
    let value = values.entry(key).or_default();
    *value = value
        .checked_add(bytes)
        .context("Oracle segment bytes exceed u64")?;
    Ok(())
}

fn build_fk_edges(
    constraints: &[RawConstraint],
    columns: &BTreeMap<(String, String), Vec<u32>>,
    table_ids: &BTreeMap<(String, String), String>,
    invalid_constraints: &BTreeSet<NativeObjectKey>,
    emitted_column_ordinals: &BTreeMap<NativeObjectKey, BTreeSet<u32>>,
) -> (BTreeMap<String, Vec<FkEdge>>, BTreeSet<NativeObjectKey>) {
    let constraints_by_key = constraints
        .iter()
        .map(|constraint| {
            (
                (constraint.owner.clone(), constraint.name.clone()),
                constraint,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut result = BTreeMap::<String, Vec<FkEdge>>::new();
    let mut emitted_column_gaps = BTreeSet::new();
    for constraint in constraints
        .iter()
        .filter(|constraint| constraint.constraint_type == "R")
    {
        let child_key = (constraint.owner.clone(), constraint.name.clone());
        if invalid_constraints.contains(&child_key) {
            continue;
        }
        let Some(parent_key) = constraint
            .referenced_owner
            .as_ref()
            .zip(constraint.referenced_constraint.as_ref())
            .map(|(owner, name)| (owner.clone(), name.clone()))
        else {
            continue;
        };
        if invalid_constraints.contains(&parent_key) {
            continue;
        }
        let Some(parent_constraint) = constraints_by_key.get(&parent_key) else {
            continue;
        };
        let Some(from) = table_ids.get(&(constraint.owner.clone(), constraint.table_name.clone()))
        else {
            continue;
        };
        let Some(to) = table_ids.get(&(
            parent_constraint.owner.clone(),
            parent_constraint.table_name.clone(),
        )) else {
            continue;
        };
        let child_columns = columns
            .get(&(constraint.owner.clone(), constraint.name.clone()))
            .cloned()
            .unwrap_or_default();
        let parent_columns = columns.get(&parent_key).cloned().unwrap_or_default();
        if child_columns.is_empty() || child_columns.len() != parent_columns.len() {
            continue;
        }
        let child_table_key = (constraint.owner.clone(), constraint.table_name.clone());
        let parent_table_key = (
            parent_constraint.owner.clone(),
            parent_constraint.table_name.clone(),
        );
        if emitted_column_ordinals
            .get(&child_table_key)
            .is_none_or(|ordinals| {
                child_columns
                    .iter()
                    .any(|column| !ordinals.contains(column))
            })
            || emitted_column_ordinals
                .get(&parent_table_key)
                .is_none_or(|ordinals| {
                    parent_columns
                        .iter()
                        .any(|column| !ordinals.contains(column))
                })
        {
            emitted_column_gaps.insert(child_table_key);
            continue;
        }
        result.entry(from.clone()).or_default().push(FkEdge {
            to: to.clone(),
            cols: child_columns,
            to_cols: parent_columns,
            on_update: "no-action".to_string(),
            on_delete: match constraint.delete_rule.to_ascii_uppercase().as_str() {
                "CASCADE" => "cascade",
                "SET NULL" => "set-null",
                _ => "no-action",
            }
            .to_string(),
            match_type: "simple".to_string(),
            deferrable: constraint.deferrable,
            initially_deferred: constraint.initially_deferred,
            validated: constraint.validated && constraint.status.eq_ignore_ascii_case("ENABLED"),
            statistics: None,
        });
    }
    for edges in result.values_mut() {
        edges.sort_by(|left, right| (&left.to, &left.cols).cmp(&(&right.to, &right.cols)));
    }
    (result, emitted_column_gaps)
}

fn schema_ids<'a>(schemas: impl IntoIterator<Item = &'a str>) -> BTreeMap<String, String> {
    let mut schemas = schemas
        .into_iter()
        .map(str::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    schemas.sort_by(|left, right| {
        format::schema_hash(left)
            .cmp(&format::schema_hash(right))
            .then_with(|| left.cmp(right))
    });
    schemas
        .into_iter()
        .enumerate()
        .map(|(index, schema)| (schema, format::schema_id(index + 1)))
        .collect()
}

fn oracle_table_kind(table: &RawTable, object: &OracleObjectFacts) -> &'static str {
    if object.object_types.contains("MATERIALIZED VIEW") {
        "materialized-view"
    } else if table.external {
        "external-table"
    } else if table.nested == Some(true) {
        "nested-table"
    } else if table.object_table {
        "object-table"
    } else if table.temporary || object.temporary.value() == Some(true) {
        "temporary-table"
    } else {
        "ordinary-table"
    }
}

fn oracle_storage_organization(table: &RawTable, external: bool) -> &'static str {
    if external {
        "external"
    } else if table.clustered {
        "clustered"
    } else {
        match table
            .iot_type
            .as_deref()
            .map(str::to_ascii_uppercase)
            .as_deref()
        {
            None => "heap",
            Some("IOT") => "index-organized",
            Some(_) => "unknown",
        }
    }
}

/// A denied DBA_NESTED_TABLES query cannot identify the parent of a visible
/// nested-storage table. Narrow the uncertainty to columns whose native type
/// can actually be a named collection. Oracle's built-in structured scalar
/// families use the same Blueprint `user-defined` class but do not imply
/// nested-table storage and must not contaminate every such table in an owner.
fn oracle_column_may_own_nested_storage(row: &OracleRow) -> bool {
    optional_text(row, 4).is_some_and(|data_type| {
        let upper = data_type.trim().to_ascii_uppercase();
        oracle_column_semantics(&upper, None, None, "", None, None).column_type == "user-defined"
            && !matches!(
                upper.as_str(),
                "XMLTYPE" | "JSON" | "VECTOR" | "SDO_GEOMETRY"
            )
    })
}

fn oracle_index_column_is_expression(
    index: &RawIndex,
    column: &RawIndexColumn,
    expression_facts: &ExpressionIndexFacts,
) -> bool {
    index.join_index
        || column.descending
        || expression_facts.columns.contains(&(
            index.owner.clone(),
            index.name.clone(),
            column.position,
        ))
}

fn oracle_index_has_expression(
    index: &RawIndex,
    expression_facts: &ExpressionIndexFacts,
    index_columns: &IndexColumnsByIndex,
) -> bool {
    index.join_index
        || index
            .index_type
            .to_ascii_uppercase()
            .contains("FUNCTION-BASED")
        || expression_facts
            .indexes
            .contains(&(index.owner.clone(), index.name.clone()))
        || index_columns
            .get(&(index.owner.clone(), index.name.clone()))
            .is_some_and(|columns| columns.iter().any(|column| column.descending))
}

fn normalize_index_type(index_type: &str) -> &'static str {
    let upper = index_type.to_ascii_uppercase();
    if upper.contains("BITMAP") {
        "bitmap"
    } else if upper.contains("DOMAIN") {
        "domain"
    } else if matches!(
        upper.as_str(),
        "NORMAL" | "NORMAL/REV" | "FUNCTION-BASED NORMAL" | "CLUSTER" | "IOT - TOP"
    ) {
        "btree"
    } else {
        "unknown"
    }
}

fn normalize_index_state(status: &str) -> &'static str {
    match status.to_ascii_uppercase().as_str() {
        "VALID" | "USABLE" | "N/A" => "usable",
        "UNUSABLE" => "unusable",
        _ => "unknown",
    }
}

#[derive(Debug, Clone)]
struct OracleColumnSemantics {
    column_type: String,
    declared_max_chars: u64,
    declared_max_bytes: u64,
    length_semantics: String,
    numeric_model: String,
    numeric_precision: Option<u64>,
    numeric_scale: Option<i64>,
    numeric_precision_radix: String,
    bit_width: u64,
    datetime_precision: u64,
}

fn oracle_column_semantics(
    data_type: &str,
    data_length: Option<u64>,
    char_length: Option<u64>,
    char_used: &str,
    precision: Option<u64>,
    scale: Option<i64>,
) -> OracleColumnSemantics {
    let upper = data_type.trim().to_ascii_uppercase();
    let mut result = OracleColumnSemantics {
        column_type: "user-defined".to_string(),
        declared_max_chars: 0,
        declared_max_bytes: 0,
        length_semantics: "not-applicable".to_string(),
        numeric_model: "not-applicable".to_string(),
        numeric_precision: None,
        numeric_scale: None,
        numeric_precision_radix: String::new(),
        bit_width: 0,
        datetime_precision: 0,
    };
    match upper.as_str() {
        "NUMBER" | "DECIMAL" | "NUMERIC" | "INTEGER" => {
            result.column_type = "numeric".to_string();
            result.numeric_model = if scale == Some(0) || upper == "INTEGER" {
                "integer"
            } else if precision.is_none() {
                "unconstrained-decimal"
            } else {
                "fixed-decimal"
            }
            .to_string();
            result.numeric_precision = precision;
            result.numeric_scale = precision.and(scale);
            result.numeric_precision_radix = "decimal".to_string();
        }
        "FLOAT" => {
            result.column_type = "numeric".to_string();
            result.numeric_model = "decimal-float".to_string();
            result.numeric_precision = precision;
            result.numeric_scale = None;
            result.numeric_precision_radix = "binary".to_string();
        }
        "BINARY_FLOAT" => {
            result.column_type = "real".to_string();
            result.numeric_model = "binary-float".to_string();
            result.numeric_precision_radix = "binary".to_string();
            result.bit_width = 32;
        }
        "BINARY_DOUBLE" => {
            result.column_type = "double".to_string();
            result.numeric_model = "binary-float".to_string();
            result.numeric_precision_radix = "binary".to_string();
            result.bit_width = 64;
        }
        "BOOLEAN" => {
            result.column_type = "boolean".to_string();
            result.bit_width = 1;
        }
        "VARCHAR2" | "NVARCHAR2" | "CHAR" | "NCHAR" | "VARCHAR" => {
            result.column_type = "string".to_string();
            result.declared_max_chars = char_length.unwrap_or(0);
            result.declared_max_bytes = data_length.unwrap_or(0);
            result.length_semantics = match char_used.to_ascii_uppercase().as_str() {
                "C" => "characters",
                "B" => "bytes",
                _ => "unknown",
            }
            .to_string();
        }
        "CLOB" | "NCLOB" | "LONG" => result.column_type = "text".to_string(),
        "RAW" => {
            result.column_type = "binary".to_string();
            result.declared_max_bytes = data_length.unwrap_or(0);
        }
        "BLOB" | "LONG RAW" | "BFILE" => result.column_type = "binary".to_string(),
        "DATE" => {
            result.column_type = "datetime".to_string();
            result.datetime_precision = 0;
        }
        value if value.starts_with("TIMESTAMP") => {
            result.column_type = "datetime".to_string();
            result.datetime_precision = scale
                .and_then(|value| u64::try_from(value).ok())
                .unwrap_or(6);
        }
        value if value.starts_with("INTERVAL") => result.column_type = "interval".to_string(),
        "ROWID" | "UROWID" => result.column_type = "string".to_string(),
        "XMLTYPE" | "JSON" | "VECTOR" | "SDO_GEOMETRY" => {
            result.column_type = "user-defined".to_string();
            result.numeric_model = "unknown".to_string();
        }
        _ => {
            result.numeric_model = "unknown".to_string();
        }
    }
    result
}

fn build_source_environment(capture: &OracleCatalogCapture) -> SourceEnvironment {
    let status = query_catalog_evidence_status(capture, "oracle-capacity-parameters", 1);
    let complete = status == OracleCatalogEvidenceStatus::Read;
    let mut cpu_count = None;
    let mut memory_target = None;
    let mut sga_target = None;
    let mut sga_max_size = None;
    if complete {
        for row in rows(capture, "oracle-capacity-parameters") {
            let Some(name) = optional_text(row, 0) else {
                continue;
            };
            let value = optional_text(row, 1).and_then(|value| value.parse::<u64>().ok());
            match name.to_ascii_lowercase().as_str() {
                "cpu_count" => cpu_count = value,
                "memory_target" => memory_target = value.filter(|value| *value > 0),
                "sga_target" => sga_target = value.filter(|value| *value > 0),
                "sga_max_size" => sga_max_size = value.filter(|value| *value > 0),
                _ => {}
            }
        }
    }
    let (memory_bytes, memory_basis) = if let Some(value) = memory_target {
        (Some(value), "database-resource-limit")
    } else {
        (sga_target.or(sga_max_size), "database-buffer-cache")
    };
    let (catalogs_read, catalogs_unreadable) = match status {
        OracleCatalogEvidenceStatus::Read | OracleCatalogEvidenceStatus::PartiallyRead => {
            (vec!["oracle-capacity-parameters".to_string()], Vec::new())
        }
        OracleCatalogEvidenceStatus::Unreadable => {
            (Vec::new(), vec!["oracle-capacity-parameters".to_string()])
        }
        // A capture that stopped before this optional query was attempted has
        // no positive or negative readability evidence. Leave both sets empty
        // rather than claiming a denial that was never observed.
        OracleCatalogEvidenceStatus::Unobserved => (Vec::new(), Vec::new()),
    };
    let mut environment = crate::environment::database_source_environment(
        cpu_count,
        "logical-cpu-limit",
        memory_bytes,
        memory_basis,
        "connected-instance",
        catalogs_read,
        catalogs_unreadable,
    );
    if let Some(limitation) = capture
        .client_version_attestation
        .and_then(|attestation| attestation.limitation_token())
    {
        environment.limitations.push(limitation.to_string());
    }
    environment.limitations.sort();
    environment.limitations.dedup();
    environment
}

fn build_topology(capture: &OracleCatalogCapture, server_major: u16) -> DatabaseTopology {
    let database = rows(capture, "oracle-database");
    let instance = rows(capture, "oracle-instance");
    let database_role = database
        .first()
        .and_then(|row| optional_text(row, 1))
        .unwrap_or_else(|| "UNKNOWN".to_string());
    let local_role = match database_role.to_ascii_uppercase().as_str() {
        "PRIMARY" => "primary",
        "PHYSICAL STANDBY" => "physical-standby",
        "LOGICAL STANDBY" => "logical-standby",
        "SNAPSHOT STANDBY" => "snapshot-standby",
        _ => "unknown",
    };
    let mut role_counts = BTreeMap::new();
    if local_role != "unknown" {
        role_counts.insert(local_role.to_string(), 1);
    }
    let mut features = Vec::new();
    let cdb = database.first().and_then(|row| yes_no(row, 3));
    match cdb {
        Some(true) => features.push("oracle-cdb".to_string()),
        Some(false) => features.push("oracle-non-cdb".to_string()),
        None if server_major < 12 => features.push("oracle-non-cdb".to_string()),
        None => {}
    }
    if local_role != "primary" && local_role != "unknown" {
        features.push("oracle-data-guard".to_string());
    }
    let rac = instance.first().and_then(|row| yes_no(row, 2));
    if rac == Some(true) {
        features.push("oracle-rac".to_string());
    }
    let connected_to_pdb = rows(capture, "oracle-containers")
        .iter()
        .any(|row| optional_u64(row, 0).ok().flatten().is_some_and(|id| id > 1));
    if connected_to_pdb {
        features.push("oracle-pdb".to_string());
    }
    features.sort();
    let database_complete = query_complete(capture, "oracle-database", 1);
    let instance_complete = query_complete(capture, "oracle-instance", 1);
    let containers_complete = server_major < 12 || query_complete(capture, "oracle-containers", 1);
    let mut catalogs_read = Vec::new();
    let mut catalogs_unreadable = Vec::new();
    let mut catalogs_not_applicable = Vec::new();
    for (complete, catalog) in [
        (database_complete, "oracle-cdb"),
        (database_complete, "oracle-data-guard"),
        (instance_complete, "oracle-instance"),
        (instance_complete && rac.is_some(), "oracle-rac"),
    ] {
        (if complete {
            &mut catalogs_read
        } else {
            &mut catalogs_unreadable
        })
        .push(catalog.to_string());
    }
    if server_major < 12 {
        catalogs_not_applicable.push("oracle-pdb".to_string());
    } else if containers_complete {
        catalogs_read.push("oracle-pdb".to_string());
    } else {
        catalogs_unreadable.push("oracle-pdb".to_string());
    }
    catalogs_read.sort();
    catalogs_unreadable.sort();
    catalogs_not_applicable.sort();
    // Pre-12c has no container catalogue. Treating that not-applicable query as
    // positive evidence would make an otherwise unreadable topology look
    // partially observed. Only a catalogue that can exist for this server
    // generation may contribute evidence.
    let any_evidence =
        database_complete || instance_complete || (server_major >= 12 && containers_complete);
    DatabaseTopology {
        contract: TOPOLOGY_CONTRACT.to_string(),
        deployment: "unknown".to_string(),
        local_role: local_role.to_string(),
        visibility: if any_evidence { "partial" } else { "unknown" }.to_string(),
        member_count: u64::from(instance_complete),
        member_count_scope: if instance_complete {
            "connected-member"
        } else {
            "unknown"
        }
        .to_string(),
        identifiers_redacted: true,
        role_counts,
        features,
        catalogs_read,
        catalogs_unreadable,
        catalogs_not_applicable,
    }
}

fn build_artifact_summary(
    capture: &OracleCatalogCapture,
    scope: &OracleOwnerScope,
    scope_truncated: bool,
    options: &OracleBasicOptions,
) -> Result<ArtifactInventory> {
    if options.artifact_detail == ArtifactDetail::None {
        return Ok(ArtifactInventory::not_requested_database());
    }
    let query_is_complete = query_complete(capture, "oracle-objects", scope.owners().len());
    let mut classification_complete = query_is_complete;
    let mut counts = BTreeMap::<String, u64>::new();
    if query_is_complete {
        for row in rows(capture, "oracle-objects") {
            let generated = yes_no(row, 5);
            let secondary = yes_no(row, 6);
            if generated.is_none() || secondary.is_none() {
                classification_complete = false;
                continue;
            }
            if generated == Some(true) || secondary == Some(true) {
                continue;
            }
            let Some(object_type) = optional_text(row, 2) else {
                classification_complete = false;
                continue;
            };
            let Some(kind) = oracle_artifact_kind(&object_type) else {
                continue;
            };
            let count = counts.entry(kind.to_string()).or_default();
            *count = count
                .checked_add(1)
                .context("Oracle artifact count exceeds u64")?;
        }
    }
    let object_count = counts
        .values()
        .try_fold(0_u64, |sum, count| sum.checked_add(*count))
        .context("Oracle artifact total exceeds u64")?;
    let (catalogs_read, catalogs_unreadable) = if query_is_complete {
        (vec!["oracle.objects".to_string()], Vec::new())
    } else {
        (Vec::new(), vec!["oracle.objects".to_string()])
    };
    // The floor can retain readable owners while dropping another selected or
    // visible owner. The narrowed object query is still positive catalog
    // evidence, but its counts cannot describe the original requested scope.
    let complete = query_is_complete && classification_complete && !scope_truncated;
    Ok(ArtifactInventory {
        contract: ARTIFACT_CONTRACT.to_string(),
        detail: "summary".to_string(),
        scope: if scope.is_selection_limited() {
            "selected-schemas"
        } else {
            "all-visible-schemas"
        }
        .to_string(),
        visibility: if complete { "full" } else { "unknown" }.to_string(),
        inventory_complete: complete,
        dependencies_complete: false,
        requirements_complete: false,
        analysis_complete: false,
        object_count,
        counts_by_kind: counts,
        catalogs_read,
        catalogs_unreadable,
        families_not_inventoried: if !classification_complete && query_is_complete {
            vec!["object_classification".to_string()]
        } else {
            Vec::new()
        },
        ..ArtifactInventory::default()
    })
}

fn oracle_artifact_kind(object_type: &str) -> Option<&'static str> {
    Some(match object_type.trim().to_ascii_uppercase().as_str() {
        "TABLE" | "INDEX" | "INDEX PARTITION" | "TABLE PARTITION" | "LOB" | "LOB PARTITION" => {
            return None
        }
        "VIEW" => "view",
        "MATERIALIZED VIEW" => "materialized_view",
        "SEQUENCE" => "sequence",
        "SYNONYM" => "synonym",
        "TYPE" => "type",
        "TYPE BODY" => "type",
        "FUNCTION" => "function",
        "PROCEDURE" => "procedure",
        "PACKAGE" => "package",
        "PACKAGE BODY" => "package",
        "TRIGGER" => "trigger",
        "DATABASE LINK" => "database_link",
        "JAVA CLASS" | "JAVA SOURCE" | "JAVA RESOURCE" => "java",
        "LIBRARY" => "library",
        "DIRECTORY" => "directory",
        "OPERATOR" => "operator",
        "INDEXTYPE" => "indextype",
        "QUEUE" => "queue",
        "EDITION" => "edition",
        "JOB" => "scheduled_job",
        "PROGRAM" => "scheduler_program",
        "SCHEDULE" => "scheduler_schedule",
        _ => "other",
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::oracle_catalog::{queries_for_tier, CatalogTier, CATALOG_QUERIES};
    use crate::oracle_scope::{resolve_oracle_owner_scope, OracleOwnerSelection};
    use crate::oracle_session::{OracleQueryFailure, OracleQueryStatus, OracleRow};
    use serde::Deserialize;

    fn text(value: &str) -> OracleValue {
        OracleValue::Text(value.to_string())
    }

    fn number(value: u64) -> OracleValue {
        OracleValue::Number(value.to_string())
    }

    fn first_forbidden_substring<'a>(text: &str, forbidden: &'a [String]) -> Option<&'a str> {
        #[derive(Default)]
        struct Node {
            next: BTreeMap<u8, usize>,
            terminal: Option<usize>,
        }

        let mut trie = vec![Node::default()];
        for (pattern_index, pattern) in forbidden.iter().enumerate() {
            let mut node = 0;
            for byte in pattern.bytes() {
                let next = if let Some(next) = trie[node].next.get(&byte) {
                    *next
                } else {
                    let next = trie.len();
                    trie.push(Node::default());
                    trie[node].next.insert(byte, next);
                    next
                };
                node = next;
            }
            trie[node].terminal = Some(pattern_index);
        }
        for start in 0..text.len() {
            let mut node = 0;
            for byte in &text.as_bytes()[start..] {
                let Some(next) = trie[node].next.get(byte).copied() else {
                    break;
                };
                node = next;
                if let Some(index) = trie[node].terminal {
                    return Some(&forbidden[index]);
                }
            }
        }
        None
    }

    fn outcome(
        query_id: &'static str,
        view: &'static str,
        rows: Vec<OracleRow>,
    ) -> OracleQueryOutcome {
        OracleQueryOutcome {
            query_id,
            view,
            owner_ordinal: Some(1),
            status: OracleQueryStatus::Executed {
                rows: rows.len() as u64,
            },
            rows,
        }
    }

    fn unscoped(mut outcome: OracleQueryOutcome) -> OracleQueryOutcome {
        outcome.owner_ordinal = None;
        outcome
    }

    pub(crate) fn fixture() -> OracleCatalogCapture {
        let table = OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS"),
                text("USERS"),
                number(120),
                number(4),
                number(20),
                text("2026-09-01"),
                text("NO"),
                text("N"),
                OracleValue::Null,
                text("NO"),
                text("DISABLED"),
                OracleValue::Null,
                text("NO"),
                text("NO"),
                OracleValue::Null,
                text("NO"),
                text("YES"),
            ],
        };
        let columns = vec![
            OracleRow {
                values: vec![
                    text("APP"),
                    text("ORDERS"),
                    text("ID"),
                    number(1),
                    text("NUMBER"),
                    number(22),
                    number(38),
                    number(0),
                    text("N"),
                    number(0),
                    OracleValue::Null,
                    OracleValue::Null,
                    text("NO"),
                ],
            },
            OracleRow {
                values: vec![
                    text("APP"),
                    text("ORDERS"),
                    text("NOTE"),
                    number(2),
                    text("VARCHAR2"),
                    number(200),
                    OracleValue::Null,
                    OracleValue::Null,
                    text("Y"),
                    number(100),
                    text("C"),
                    number(3),
                    text("NO"),
                ],
            },
        ];
        let mut outcomes = vec![
            outcome(
                "oracle-users",
                "ALL_USERS",
                vec![OracleRow {
                    values: vec![text("APP"), text("NO")],
                }],
            ),
            outcome("oracle-tables", "DBA_TABLES", vec![table]),
            outcome("oracle-object-tables", "DBA_OBJECT_TABLES", Vec::new()),
            outcome("oracle-external-tables", "DBA_EXTERNAL_TABLES", Vec::new()),
            outcome("oracle-nested-tables", "DBA_NESTED_TABLES", Vec::new()),
            outcome("oracle-mview-logs", "DBA_MVIEW_LOGS", Vec::new()),
            outcome("oracle-queue-tables", "DBA_QUEUE_TABLES", Vec::new()),
            outcome("oracle-tab-columns", "DBA_TAB_COLUMNS", columns),
            outcome(
                "oracle-objects",
                "DBA_OBJECTS",
                vec![OracleRow {
                    values: vec![
                        text("APP"),
                        text("ORDERS"),
                        text("TABLE"),
                        text("VALID"),
                        text("N"),
                        text("N"),
                        text("N"),
                        text("Y"),
                    ],
                }],
            ),
            outcome(
                "oracle-tab-cols-hidden",
                "DBA_TAB_COLS",
                vec![
                    OracleRow {
                        values: vec![
                            text("APP"),
                            text("ORDERS"),
                            text("ID"),
                            text("NO"),
                            text("NO"),
                            number(1),
                            text("YES"),
                        ],
                    },
                    OracleRow {
                        values: vec![
                            text("APP"),
                            text("ORDERS"),
                            text("NOTE"),
                            text("NO"),
                            text("NO"),
                            number(2),
                            text("YES"),
                        ],
                    },
                ],
            ),
            outcome(
                "oracle-identity-columns",
                "DBA_TAB_IDENTITY_COLS",
                Vec::new(),
            ),
            outcome("oracle-indexes", "DBA_INDEXES", Vec::new()),
            outcome("oracle-index-columns", "DBA_IND_COLUMNS", Vec::new()),
            outcome(
                "oracle-index-expressions",
                "DBA_IND_EXPRESSIONS",
                Vec::new(),
            ),
            outcome("oracle-constraints", "DBA_CONSTRAINTS", Vec::new()),
            outcome("oracle-constraint-columns", "DBA_CONS_COLUMNS", Vec::new()),
            outcome("oracle-part-tables", "DBA_PART_TABLES", Vec::new()),
            outcome("oracle-tab-partitions", "DBA_TAB_PARTITIONS", Vec::new()),
            outcome(
                "oracle-tab-subpartitions",
                "DBA_TAB_SUBPARTITIONS",
                Vec::new(),
            ),
            outcome(
                "oracle-part-key-columns",
                "DBA_PART_KEY_COLUMNS",
                Vec::new(),
            ),
            outcome("oracle-lobs", "DBA_LOBS", Vec::new()),
            outcome("oracle-tab-statistics", "DBA_TAB_STATISTICS", Vec::new()),
            outcome(
                "oracle-segments",
                "DBA_SEGMENTS",
                vec![OracleRow {
                    values: vec![
                        text("APP"),
                        text("ORDERS"),
                        text("TABLE"),
                        text("USERS"),
                        number(8192),
                        number(1),
                    ],
                }],
            ),
            unscoped(outcome(
                "oracle-nls",
                "NLS_DATABASE_PARAMETERS",
                vec![OracleRow {
                    values: vec![text("NLS_CHARACTERSET"), text("AL32UTF8")],
                }],
            )),
            unscoped(outcome(
                "oracle-version",
                "PRODUCT_COMPONENT_VERSION",
                vec![OracleRow {
                    values: vec![text("Oracle Database"), text("21.3"), text("21.3.0.0.0")],
                }],
            )),
            unscoped(outcome(
                "oracle-database",
                "V_$DATABASE",
                vec![OracleRow {
                    values: vec![
                        text("Linux"),
                        text("PRIMARY"),
                        text("READ WRITE"),
                        text("YES"),
                    ],
                }],
            )),
            unscoped(outcome(
                "oracle-instance",
                "V_$INSTANCE",
                vec![OracleRow {
                    values: vec![text("21.3"), text("OPEN"), text("NO")],
                }],
            )),
            unscoped(outcome(
                "oracle-containers",
                "V_$CONTAINERS",
                vec![OracleRow {
                    values: vec![number(3), text("READ WRITE"), text("NO")],
                }],
            )),
            unscoped(outcome(
                "oracle-capacity-parameters",
                "V_$PARAMETER",
                vec![
                    OracleRow {
                        values: vec![text("cpu_count"), number(4), text("FALSE")],
                    },
                    OracleRow {
                        values: vec![text("memory_target"), number(0), text("TRUE")],
                    },
                    OracleRow {
                        values: vec![text("sga_target"), number(1_073_741_824), text("FALSE")],
                    },
                    OracleRow {
                        values: vec![text("sga_max_size"), number(1_073_741_824), text("FALSE")],
                    },
                ],
            )),
        ];
        outcomes.sort_by_key(|outcome| outcome.query_id);
        OracleCatalogCapture {
            outcomes,
            catalogs_read: Vec::new(),
            catalogs_unreadable: Vec::new(),
            abort: None,
            session_discarded: false,
            unconfirmed_server_work: false,
            rows_consumed: 0,
            bytes_consumed: 0,
            lob_bytes_consumed: 0,
            elapsed_ms: 0,
            client_version_attestation: None,
        }
    }

    fn append_second_owner(
        capture: &mut OracleCatalogCapture,
        owner: &str,
        server_major: u16,
    ) -> OracleOwnerScope {
        let query_by_id = CATALOG_QUERIES
            .iter()
            .map(|query| (query.query_id, query))
            .collect::<BTreeMap<_, _>>();
        let mut second_owner = capture
            .outcomes
            .iter()
            .filter(|outcome| outcome.owner_ordinal == Some(1))
            .cloned()
            .collect::<Vec<_>>();
        for outcome in &mut second_owner {
            outcome.owner_ordinal = Some(2);
            let query = query_by_id[outcome.query_id];
            let columns = query.output_columns(server_major);
            let owner_index = columns
                .iter()
                .position(|column| {
                    query
                        .owner_column
                        .is_some_and(|owner_column| column.eq_ignore_ascii_case(owner_column))
                })
                .expect("owner-scoped fixture query must project its owner");
            for row in &mut outcome.rows {
                row.values[owner_index] = text(owner);
            }
        }
        capture.outcomes.extend(second_owner);
        resolve_oracle_owner_scope(
            OracleOwnerSelection::Selected(vec!["APP".into(), owner.into()]),
            "APP",
            vec!["APP".into(), owner.into()],
        )
        .unwrap()
    }

    fn append_test_index(
        capture: &mut OracleCatalogCapture,
        table_owner_ordinal: u32,
        segment_owner_ordinal: Option<u32>,
        index_owner: &str,
        index_name: &str,
        table_owner: &str,
        table_name: &str,
        segment_bytes: u64,
    ) {
        let indexes = capture
            .outcomes
            .iter_mut()
            .find(|outcome| {
                outcome.query_id == "oracle-indexes"
                    && outcome.owner_ordinal == Some(table_owner_ordinal)
            })
            .expect("index fixture outcome");
        indexes.rows.push(OracleRow {
            values: vec![
                text(index_owner),
                text(index_name),
                text(table_owner),
                text(table_name),
                text("NORMAL"),
                text("NONUNIQUE"),
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                text("NO"),
                text("VALID"),
                text("N"),
                text("NO"),
                text("VISIBLE"),
                text("NO"),
                text("YES"),
            ],
        });
        indexes.status = OracleQueryStatus::Executed {
            rows: indexes.rows.len() as u64,
        };

        let index_columns = capture
            .outcomes
            .iter_mut()
            .find(|outcome| {
                outcome.query_id == "oracle-index-columns"
                    && outcome.owner_ordinal == Some(table_owner_ordinal)
            })
            .expect("index-column fixture outcome");
        index_columns.rows.push(OracleRow {
            values: vec![
                text(index_owner),
                text(index_name),
                text(table_name),
                text("ID"),
                number(1),
                text("ASC"),
                text(table_owner),
            ],
        });
        index_columns.status = OracleQueryStatus::Executed {
            rows: index_columns.rows.len() as u64,
        };

        if let Some(segment_owner_ordinal) = segment_owner_ordinal {
            let segments = capture
                .outcomes
                .iter_mut()
                .find(|outcome| {
                    outcome.query_id == "oracle-segments"
                        && outcome.owner_ordinal == Some(segment_owner_ordinal)
                })
                .expect("segment fixture outcome");
            segments.rows.push(OracleRow {
                values: vec![
                    text(index_owner),
                    text(index_name),
                    text("INDEX"),
                    text("USERS"),
                    number(segment_bytes),
                    number(1),
                ],
            });
            segments.status = OracleQueryStatus::Executed {
                rows: segments.rows.len() as u64,
            };
        }
    }

    #[test]
    fn basic_mapper_emits_contract_valid_name_free_oracle_v7() {
        let blueprint = map_oracle_basic_capture(
            &fixture(),
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21, 3]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: Some("2026-09-13T00:00:00Z".to_string()),
                artifact_detail: ArtifactDetail::Summary,
            },
        )
        .unwrap();
        assert_eq!(blueprint.engine, "oracle");
        assert_eq!(blueprint.engine_version, "21.3");
        assert_eq!(blueprint.tables.len(), 1);
        let table = blueprint.tables.values().next().unwrap();
        assert_eq!(table.rows, 100);
        assert_eq!(table.cols.len(), 2);
        assert!(table.cols.values().any(|column| {
            column.column_type == "numeric" && column.numeric_model == "integer"
        }));
        let serialized = toml::to_string(&blueprint).unwrap();
        assert!(!serialized.contains("APP"));
        assert!(!serialized.contains("ORDERS"));
        assert!(!serialized.contains("NOTE"));
    }

    #[test]
    fn basic_mapper_records_unverified_sqlplus_client_provenance_without_failing_capture() {
        let cases = [
            (
                OracleClientVersionAttestation::Mismatched,
                "oracle-client-version-mismatch",
            ),
            (
                OracleClientVersionAttestation::Attested(
                    OracleVersion::from_components(&[11, 2]).unwrap(),
                ),
                "oracle-client-version-below-tested-floor",
            ),
        ];
        for (attestation, limitation) in cases {
            let mut capture = fixture();
            capture.client_version_attestation = Some(attestation);
            let blueprint = map_oracle_basic_capture(
                &capture,
                &OracleOwnerScope::one("APP"),
                OracleVersion::from_components(&[21, 3]).unwrap(),
                &OracleBasicOptions {
                    source_kind: "production".to_string(),
                    generated_at_pin: Some("2026-09-13T00:00:00Z".to_string()),
                    artifact_detail: ArtifactDetail::Summary,
                },
            )
            .expect("client-version provenance cannot block catalog mapping");
            assert_eq!(
                blueprint.source_environment.unwrap().limitations,
                [limitation]
            );
        }
    }

    #[test]
    fn invisible_column_without_column_id_uses_internal_generation_order() {
        let mut capture = fixture();
        let columns = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tab-columns")
            .unwrap();
        columns.rows[1].values[3] = OracleValue::Null;
        let hidden = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tab-cols-hidden")
            .unwrap();
        hidden.rows[1].values[3] = text("YES");
        hidden.rows[1].values[6] = text("YES");

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21, 3]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::Summary,
            },
        )
        .unwrap();
        let table = blueprint.tables.values().next().unwrap();
        assert_eq!(table.cols.len(), 2);
        assert_eq!(table.cols["col-2"].invisible, Some(true));
    }

    #[test]
    fn cdb_root_capture_is_refused_even_when_catalog_rows_are_well_formed() {
        let mut capture = fixture();
        let containers = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-containers")
            .unwrap();
        containers.rows[0].values[0] = number(1);
        let error = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21, 3]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::Summary,
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("CDB$ROOT"));
    }

    #[test]
    fn engine_generated_objects_do_not_enter_artifact_counts() {
        let mut capture = fixture();
        let objects = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-objects")
            .unwrap();
        objects.rows.push(OracleRow {
            values: vec![
                text("APP"),
                text("SYS_GENERATED_HELPER"),
                text("PROCEDURE"),
                text("VALID"),
                text("N"),
                text("Y"),
                text("N"),
                text("Y"),
            ],
        });
        objects.status = OracleQueryStatus::Executed {
            rows: objects.rows.len() as u64,
        };
        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21, 3]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::Summary,
            },
        )
        .unwrap();
        assert_eq!(blueprint.artifact_inventory.unwrap().object_count, 0);
    }

    #[test]
    fn syntactic_not_null_constraints_never_invent_an_exact_check_count() {
        let mut capture = fixture();
        let constraints = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-constraints")
            .unwrap();
        let constraint_row = |name: &str, generated: &str, is_not_null: &str| OracleRow {
            values: vec![
                text("APP"),
                text(name),
                text("C"),
                text("ORDERS"),
                OracleValue::Null,
                OracleValue::Null,
                text("NO ACTION"),
                text("ENABLED"),
                text("NOT DEFERRABLE"),
                text("VALIDATED"),
                text("IMMEDIATE"),
                OracleValue::Null,
                OracleValue::Null,
                text(generated),
                text(is_not_null),
            ],
        };
        constraints.rows = vec![
            constraint_row("SYS_C001", "GENERATED NAME", "Y"),
            constraint_row("ORDERS_ID_NN", "USER NAME", "Y"),
            constraint_row("CUSTOMER_RULE", "USER NAME", "N"),
        ];
        constraints.status = OracleQueryStatus::Executed { rows: 3 };
        let constraint_columns = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-constraint-columns")
            .unwrap();
        constraint_columns.rows = ["SYS_C001", "ORDERS_ID_NN"]
            .into_iter()
            .map(|name| OracleRow {
                values: vec![
                    text("APP"),
                    text(name),
                    text("ORDERS"),
                    text("ID"),
                    number(1),
                ],
            })
            .collect();
        constraint_columns.status = OracleQueryStatus::Executed { rows: 2 };
        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21, 3]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::Summary,
            },
        )
        .unwrap();
        assert_eq!(blueprint.tables.values().next().unwrap().check_count, None);
    }

    #[test]
    fn absent_core_owner_result_is_never_published_as_empty() {
        let mut capture = fixture();
        capture
            .outcomes
            .retain(|outcome| outcome.query_id != "oracle-tab-columns");
        assert!(map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .is_err());
    }

    #[test]
    fn declared_outcome_row_count_must_match_the_captured_rows() {
        let mut capture = fixture();
        capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tables")
            .unwrap()
            .status = OracleQueryStatus::Executed { rows: 2 };

        let error = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect_err("declared and captured row counts are one contract");
        assert!(error
            .to_string()
            .contains("row count does not match its outcome evidence"));
    }

    #[test]
    fn named_floor_refusal_identifies_the_missing_owner_component() {
        let mut capture = fixture();
        let tables = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tables")
            .unwrap();
        tables.status = OracleQueryStatus::Failed {
            class: OracleQueryFailure::PermissionDenied,
        };
        tables.rows.clear();

        let error = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect_err("an owner without its table inventory cannot satisfy the floor");
        let message = error.to_string();
        assert!(message.contains("Oracle Blueprint floor unavailable"));
        assert!(message.contains("owner-1: table and row inventory"));
        assert!(!message.contains("APP"));
    }

    #[test]
    fn readable_tableless_schema_is_retained_without_claiming_a_complete_empty_database() {
        let mut capture = fixture();
        for query_id in ["oracle-objects", "oracle-tables", "oracle-tab-columns"] {
            let outcome = capture
                .outcomes
                .iter_mut()
                .find(|outcome| outcome.query_id == query_id)
                .unwrap();
            outcome.rows.clear();
            outcome.status = OracleQueryStatus::Executed { rows: 0 };
        }

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("a resolved readable schema may legitimately contain no tables");
        assert!(blueprint.tables.is_empty());
        assert_eq!(blueprint.totals.table_count, 0);
        let dataset = blueprint.dataset_scope.unwrap();
        assert_eq!(dataset.table_inventory_completeness, "incomplete");
        assert_eq!(dataset.row_count_completeness, "incomplete");
        assert_eq!(dataset.size_completeness, "incomplete");
        assert_eq!(dataset.size_method, "unknown");
        assert!(dataset
            .limitations
            .contains(&"table-inventory-visibility-unknown".to_string()));
        let structure = blueprint.structure_scope.as_ref().unwrap();
        assert_eq!(structure.visibility, "unknown");
        assert_eq!(structure.table_inventory_completeness, "incomplete");
    }

    #[test]
    fn minimum_catalog_families_alone_emit_a_degraded_floor_blueprint() {
        let mut capture = fixture();
        let floor_views = BTreeSet::from([
            "DBA_OBJECTS",
            "DBA_TABLES",
            "DBA_TAB_COLUMNS",
            "DBA_SEGMENTS",
            "V_$INSTANCE",
        ]);
        for outcome in &mut capture.outcomes {
            if !floor_views.contains(outcome.view) {
                outcome.rows.clear();
                outcome.status = OracleQueryStatus::Failed {
                    class: OracleQueryFailure::PermissionDenied,
                };
            }
        }

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("the five-view minimum tier must emit the Blueprint floor");

        assert_eq!(blueprint.tables.len(), 1);
        let table = blueprint.tables.values().next().unwrap();
        assert_eq!(table.cols.len(), 2);
        assert!(table.table_bytes > 0);
        assert_eq!(
            table.statistics.as_ref().unwrap().size_method,
            "oracle-segment-bytes"
        );
        assert_eq!(
            blueprint
                .dataset_scope
                .as_ref()
                .unwrap()
                .table_inventory_completeness,
            "incomplete"
        );
        assert_eq!(
            blueprint
                .structure_scope
                .as_ref()
                .unwrap()
                .column_inventory_completeness,
            "incomplete"
        );
        dbwarp_blueprint_core::validate_blueprint_contract(&blueprint).unwrap();
    }

    #[test]
    fn every_environment_abort_after_floor_emits_a_verified_truncated_prefix() {
        let environmental = [
            OracleCaptureAbort::CoreInventoryUnavailable,
            OracleCaptureAbort::OwnerScopeUnavailable,
            OracleCaptureAbort::RowLimitExceeded,
            OracleCaptureAbort::ByteLimitExceeded,
            OracleCaptureAbort::TransientValueLimitExceeded,
            OracleCaptureAbort::LobLimitExceeded,
            OracleCaptureAbort::DeadlineExceeded,
            OracleCaptureAbort::SessionLost,
            OracleCaptureAbort::UnconfirmedCancellation,
        ];
        for reason in environmental {
            let mut capture = fixture();
            capture.abort = Some(reason);
            let indexes = capture
                .outcomes
                .iter_mut()
                .find(|outcome| outcome.query_id == "oracle-indexes")
                .unwrap();
            indexes.status = OracleQueryStatus::NotReached;
            indexes.rows.clear();
            let statistics = capture
                .outcomes
                .iter_mut()
                .find(|outcome| outcome.query_id == "oracle-tab-statistics")
                .unwrap();
            statistics.status = OracleQueryStatus::NotReached;
            statistics.rows.clear();

            let blueprint = map_oracle_basic_capture(
                &capture,
                &OracleOwnerScope::one("APP"),
                OracleVersion::from_components(&[21]).unwrap(),
                &OracleBasicOptions {
                    source_kind: "production".to_string(),
                    generated_at_pin: None,
                    artifact_detail: ArtifactDetail::None,
                },
            )
            .unwrap_or_else(|error| panic!("{reason:?} discarded a valid floor: {error:#}"));
            assert_eq!(blueprint.tables.len(), 1);
            assert!(blueprint
                .dataset_scope
                .as_ref()
                .unwrap()
                .limitations
                .contains(&"catalog-capture-truncated".to_string()));
            let structure = blueprint.structure_scope.as_ref().unwrap();
            assert!(!structure
                .catalogs_read
                .contains(&"oracle-indexes".to_string()));
            assert!(!structure
                .catalogs_unreadable
                .contains(&"oracle-indexes".to_string()));
            assert!(!structure
                .catalogs_not_applicable
                .contains(&"oracle-indexes".to_string()));
            let statistics = blueprint.statistics_evidence.as_ref().unwrap();
            assert!(!statistics
                .catalogs_read
                .contains(&"oracle-tab-statistics".to_string()));
            assert!(!statistics
                .catalogs_unreadable
                .contains(&"oracle-tab-statistics".to_string()));
            assert!(!statistics
                .catalogs_not_applicable
                .contains(&"oracle-tab-statistics".to_string()));
        }
    }

    #[test]
    fn catalog_evidence_preserves_partial_reads_without_inventing_failures() {
        let mut capture = fixture();
        assert_eq!(
            query_catalog_evidence_status(&capture, "oracle-tab-statistics", 1),
            OracleCatalogEvidenceStatus::Read
        );

        let outcome = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tab-statistics")
            .unwrap();
        outcome.status = OracleQueryStatus::Failed {
            class: OracleQueryFailure::PermissionDenied,
        };
        assert_eq!(
            query_catalog_evidence_status(&capture, "oracle-tab-statistics", 1),
            OracleCatalogEvidenceStatus::Unreadable
        );

        let first = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tab-statistics")
            .unwrap();
        first.status = OracleQueryStatus::Executed {
            rows: first.rows.len() as u64,
        };

        let mut second = capture
            .outcomes
            .iter()
            .find(|outcome| outcome.query_id == "oracle-tab-statistics")
            .unwrap()
            .clone();
        second.owner_ordinal = Some(2);
        second.status = OracleQueryStatus::NotReached;
        second.rows.clear();
        capture.outcomes.push(second);
        assert_eq!(
            query_catalog_evidence_status(&capture, "oracle-tab-statistics", 2),
            OracleCatalogEvidenceStatus::PartiallyRead
        );

        capture
            .outcomes
            .iter_mut()
            .find(|outcome| {
                outcome.query_id == "oracle-tab-statistics" && outcome.owner_ordinal == Some(2)
            })
            .unwrap()
            .status = OracleQueryStatus::Failed {
            class: OracleQueryFailure::PermissionDenied,
        };
        assert_eq!(
            query_catalog_evidence_status(&capture, "oracle-tab-statistics", 2),
            OracleCatalogEvidenceStatus::PartiallyRead
        );
        capture
            .outcomes
            .iter_mut()
            .find(|outcome| {
                outcome.query_id == "oracle-tab-statistics" && outcome.owner_ordinal == Some(2)
            })
            .unwrap()
            .status = OracleQueryStatus::NotReached;

        let first = capture
            .outcomes
            .iter_mut()
            .find(|outcome| {
                outcome.query_id == "oracle-tab-statistics" && outcome.owner_ordinal == Some(1)
            })
            .unwrap();
        first.status = OracleQueryStatus::NotReached;
        first.rows.clear();
        assert_eq!(
            query_catalog_evidence_status(&capture, "oracle-tab-statistics", 2),
            OracleCatalogEvidenceStatus::Unobserved
        );
    }

    #[test]
    fn only_adapter_contract_and_no_live_session_aborts_remain_fatal() {
        for reason in [
            OracleCaptureAbort::AdapterContractViolated,
            OracleCaptureAbort::NoLiveSession,
        ] {
            let mut capture = fixture();
            capture.abort = Some(reason);
            assert!(map_oracle_basic_capture(
                &capture,
                &OracleOwnerScope::one("APP"),
                OracleVersion::from_components(&[21]).unwrap(),
                &OracleBasicOptions {
                    source_kind: "production".to_string(),
                    generated_at_pin: None,
                    artifact_detail: ArtifactDetail::None,
                },
            )
            .is_err());
        }
    }

    #[test]
    fn mapper_rejects_rows_outside_the_resolved_owner_scope() {
        let mut capture = fixture();
        let table = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tables")
            .unwrap();
        table.rows[0].values[0] = text("OTHER");
        let error = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("falls outside its resolved owner scope"));
    }

    #[test]
    fn multi_owner_mapper_keeps_both_scopes_without_emitting_their_names() {
        let mut capture = fixture();
        let query_by_id = CATALOG_QUERIES
            .iter()
            .map(|query| (query.query_id, query))
            .collect::<BTreeMap<_, _>>();
        let mut second_owner = capture
            .outcomes
            .iter()
            .filter(|outcome| outcome.owner_ordinal.is_some())
            .cloned()
            .collect::<Vec<_>>();
        for outcome in &mut second_owner {
            outcome.owner_ordinal = Some(2);
            let query = query_by_id[outcome.query_id];
            let columns = query.output_columns(21);
            let owner_index = columns
                .iter()
                .position(|column| {
                    query
                        .owner_column
                        .is_some_and(|owner| column.eq_ignore_ascii_case(owner))
                })
                .unwrap();
            for row in &mut outcome.rows {
                row.values[owner_index] = text("OTHER");
            }
        }
        capture.outcomes.extend(second_owner);
        let scope = resolve_oracle_owner_scope(
            OracleOwnerSelection::Selected(vec!["APP".into(), "OTHER".into()]),
            "APP",
            vec!["APP".into(), "OTHER".into()],
        )
        .unwrap();
        let blueprint = map_oracle_basic_capture(
            &capture,
            &scope,
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::Summary,
            },
        )
        .unwrap();
        assert_eq!(blueprint.tables.len(), 2);
        assert_eq!(
            blueprint
                .structure_scope
                .as_ref()
                .unwrap()
                .table_inventory_completeness,
            "complete"
        );
        let structure = blueprint.structure_scope.as_ref().unwrap();
        assert_eq!(structure.column_inventory_completeness, "complete");
        assert_eq!(structure.index_inventory_completeness, "complete");
        assert_eq!(structure.relationship_inventory_completeness, "complete");
        let dataset = blueprint.dataset_scope.as_ref().unwrap();
        assert_eq!(dataset.row_count_completeness, "complete");
        assert_eq!(dataset.size_completeness, "complete");
        assert_eq!(dataset.size_method, "oracle-segment-bytes");
        assert_eq!(
            blueprint
                .tables
                .values()
                .map(|table| table.schema.as_str())
                .collect::<BTreeSet<_>>()
                .len(),
            2
        );
        let serialized = toml::to_string(&blueprint).unwrap();
        assert!(!serialized.contains("APP"));
        assert!(!serialized.contains("OTHER"));
    }

    #[test]
    fn malformed_hidden_and_identity_rows_degrade_only_their_native_owner() {
        let mut capture = fixture();
        let scope = append_second_owner(&mut capture, "OTHER", 21);

        let app_identity = capture
            .outcomes
            .iter_mut()
            .find(|outcome| {
                outcome.query_id == "oracle-identity-columns" && outcome.owner_ordinal == Some(1)
            })
            .unwrap();
        app_identity.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS"),
                text("ID"),
                text("ALWAYS"),
                OracleValue::Null,
            ],
        }];
        app_identity.status = OracleQueryStatus::Executed { rows: 1 };

        let other_identity = capture
            .outcomes
            .iter_mut()
            .find(|outcome| {
                outcome.query_id == "oracle-identity-columns" && outcome.owner_ordinal == Some(2)
            })
            .unwrap();
        let duplicate_identity = OracleRow {
            values: vec![
                text("OTHER"),
                text("ORDERS"),
                text("ID"),
                text("ALWAYS"),
                OracleValue::Null,
            ],
        };
        other_identity.rows = vec![duplicate_identity.clone(), duplicate_identity];
        other_identity.status = OracleQueryStatus::Executed { rows: 2 };

        let other_hidden = capture
            .outcomes
            .iter_mut()
            .find(|outcome| {
                outcome.query_id == "oracle-tab-cols-hidden" && outcome.owner_ordinal == Some(2)
            })
            .unwrap();
        other_hidden.rows.push(other_hidden.rows[0].clone());
        other_hidden.status = OracleQueryStatus::Executed {
            rows: other_hidden.rows.len() as u64,
        };
        let other_columns = capture
            .outcomes
            .iter_mut()
            .find(|outcome| {
                outcome.query_id == "oracle-tab-columns" && outcome.owner_ordinal == Some(2)
            })
            .unwrap();
        other_columns.rows[1].values[3] = OracleValue::Null;
        capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tables" && outcome.owner_ordinal == Some(2))
            .unwrap()
            .rows[0]
            .values[3] = number(900);

        let blueprint = map_oracle_basic_capture(
            &capture,
            &scope,
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("malformed optional column refinements must not abort the floor capture");

        let healthy = blueprint
            .tables
            .values()
            .find(|table| table.rows == 100)
            .expect("APP table");
        let affected = blueprint
            .tables
            .values()
            .find(|table| table.rows == 900)
            .expect("OTHER table");
        assert_eq!(healthy.cols["col-1"].value_source, "identity-always");
        assert!(!healthy
            .table_limitations
            .contains(&"column-inventory-unavailable".to_string()));
        assert_eq!(affected.cols["col-1"].value_source, "");
        assert_eq!(affected.cols.len(), 1);
        assert!(affected
            .table_limitations
            .contains(&"column-inventory-unavailable".to_string()));
        assert!(affected
            .table_limitations
            .contains(&"dependent-structure-suppressed".to_string()));
        let structure = blueprint.structure_scope.as_ref().unwrap();
        assert_eq!(structure.column_inventory_completeness, "incomplete");
    }

    #[test]
    fn tableless_owner_column_refinement_denials_do_not_degrade_emitted_columns() {
        let mut capture = fixture();
        let scope = append_second_owner(&mut capture, "OTHER", 21);
        for outcome in capture
            .outcomes
            .iter_mut()
            .filter(|outcome| outcome.owner_ordinal == Some(2))
        {
            outcome.rows.clear();
            outcome.status = if matches!(
                outcome.query_id,
                "oracle-tab-cols-hidden" | "oracle-identity-columns"
            ) {
                OracleQueryStatus::Failed {
                    class: OracleQueryFailure::PermissionDenied,
                }
            } else {
                OracleQueryStatus::Executed { rows: 0 }
            };
            if outcome.query_id == "oracle-objects" {
                outcome.rows.push(OracleRow {
                    values: vec![
                        text("OTHER"),
                        text("COUNTER_SEQ"),
                        text("SEQUENCE"),
                        text("VALID"),
                        text("N"),
                        text("N"),
                        text("N"),
                        text("Y"),
                    ],
                });
                outcome.status = OracleQueryStatus::Executed { rows: 1 };
            }
        }

        let blueprint = map_oracle_basic_capture(
            &capture,
            &scope,
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::Summary,
            },
        )
        .expect("a tableless owner's optional denial cannot affect another owner's columns");

        let structure = blueprint.structure_scope.as_ref().unwrap();
        assert_eq!(structure.column_inventory_completeness, "complete");
        assert_eq!(structure.index_inventory_completeness, "complete");
        assert_eq!(structure.relationship_inventory_completeness, "complete");
        let table = blueprint.tables.values().next().unwrap();
        assert_eq!(table.cols.len(), 2);
        assert!(table.table_limitations.is_empty());
    }

    #[test]
    fn optional_classification_denials_for_filtered_support_storage_do_not_degrade_customers() {
        let mut capture = fixture();
        let scope = append_second_owner(&mut capture, "OTHER", 21);

        let other_tables = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tables" && outcome.owner_ordinal == Some(2))
            .unwrap();
        other_tables.rows[0].values[1] = text("WORK_QUEUE");
        let other_objects = capture
            .outcomes
            .iter_mut()
            .find(|outcome| {
                outcome.query_id == "oracle-objects" && outcome.owner_ordinal == Some(2)
            })
            .unwrap();
        other_objects.rows[0].values[1] = text("WORK_QUEUE");
        let other_queues = capture
            .outcomes
            .iter_mut()
            .find(|outcome| {
                outcome.query_id == "oracle-queue-tables" && outcome.owner_ordinal == Some(2)
            })
            .unwrap();
        other_queues.rows = vec![OracleRow {
            values: vec![text("OTHER"), text("WORK_QUEUE")],
        }];
        other_queues.status = OracleQueryStatus::Executed { rows: 1 };
        for query_id in ["oracle-mview-logs", "oracle-external-tables"] {
            let outcome = capture
                .outcomes
                .iter_mut()
                .find(|outcome| outcome.query_id == query_id && outcome.owner_ordinal == Some(2))
                .unwrap();
            outcome.rows.clear();
            outcome.status = OracleQueryStatus::Failed {
                class: OracleQueryFailure::PermissionDenied,
            };
        }

        let blueprint = map_oracle_basic_capture(
            &capture,
            &scope,
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("denied refinements for filtered support storage cannot degrade customer tables");

        assert_eq!(blueprint.tables.len(), 1);
        let dataset = blueprint.dataset_scope.as_ref().unwrap();
        assert_eq!(dataset.table_inventory_completeness, "complete");
        assert_eq!(dataset.row_count_completeness, "complete");
        assert_eq!(dataset.size_completeness, "complete");
        let structure = blueprint.structure_scope.as_ref().unwrap();
        assert_eq!(structure.table_inventory_completeness, "complete");
        assert_eq!(structure.column_inventory_completeness, "complete");
    }

    #[test]
    fn malformed_support_catalog_preserves_another_owners_exact_support_filter() {
        let mut capture = fixture();
        let scope = append_second_owner(&mut capture, "OTHER", 21);

        let app_tables = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tables" && outcome.owner_ordinal == Some(1))
            .unwrap();
        let mut support_table = app_tables.rows[0].clone();
        support_table.values[1] = text("WORK_QUEUE");
        support_table.values[3] = number(300);
        app_tables.rows.push(support_table);
        app_tables.status = OracleQueryStatus::Executed {
            rows: app_tables.rows.len() as u64,
        };
        let app_objects = capture
            .outcomes
            .iter_mut()
            .find(|outcome| {
                outcome.query_id == "oracle-objects" && outcome.owner_ordinal == Some(1)
            })
            .unwrap();
        let mut support_object = app_objects.rows[0].clone();
        support_object.values[1] = text("WORK_QUEUE");
        app_objects.rows.push(support_object);
        app_objects.status = OracleQueryStatus::Executed {
            rows: app_objects.rows.len() as u64,
        };
        let app_queues = capture
            .outcomes
            .iter_mut()
            .find(|outcome| {
                outcome.query_id == "oracle-queue-tables" && outcome.owner_ordinal == Some(1)
            })
            .unwrap();
        app_queues.rows = vec![OracleRow {
            values: vec![text("APP"), text("WORK_QUEUE")],
        }];
        app_queues.status = OracleQueryStatus::Executed { rows: 1 };

        let other_queues = capture
            .outcomes
            .iter_mut()
            .find(|outcome| {
                outcome.query_id == "oracle-queue-tables" && outcome.owner_ordinal == Some(2)
            })
            .unwrap();
        let duplicate = OracleRow {
            values: vec![text("OTHER"), text("BROKEN_QUEUE")],
        };
        other_queues.rows = vec![duplicate.clone(), duplicate];
        other_queues.status = OracleQueryStatus::Executed { rows: 2 };
        capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tables" && outcome.owner_ordinal == Some(2))
            .unwrap()
            .rows[0]
            .values[3] = number(900);

        let blueprint = map_oracle_basic_capture(
            &capture,
            &scope,
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("one malformed support catalog must not erase another owner's exact identities");

        assert_eq!(blueprint.tables.len(), 2);
        assert!(!blueprint.tables.values().any(|table| table.rows == 300));
        let healthy = blueprint
            .tables
            .values()
            .find(|table| table.rows == 100)
            .expect("APP table");
        let affected = blueprint
            .tables
            .values()
            .find(|table| table.rows == 900)
            .expect("OTHER table");
        assert!(!healthy
            .table_limitations
            .contains(&"table-classification-unavailable".to_string()));
        assert!(affected
            .table_limitations
            .contains(&"table-classification-unavailable".to_string()));
        assert_eq!(
            blueprint
                .structure_scope
                .as_ref()
                .unwrap()
                .table_inventory_completeness,
            "incomplete"
        );
    }

    #[test]
    fn malformed_external_catalog_preserves_another_owners_pre_12_2_classification() {
        let mut capture = fixture();
        capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-version")
            .unwrap()
            .rows[0]
            .values
            .truncate(2);
        let scope = append_second_owner(&mut capture, "OTHER", 12);

        let app_external = capture
            .outcomes
            .iter_mut()
            .find(|outcome| {
                outcome.query_id == "oracle-external-tables" && outcome.owner_ordinal == Some(1)
            })
            .unwrap();
        app_external.rows = vec![OracleRow {
            values: vec![text("APP"), text("ORDERS")],
        }];
        app_external.status = OracleQueryStatus::Executed { rows: 1 };
        let other_external = capture
            .outcomes
            .iter_mut()
            .find(|outcome| {
                outcome.query_id == "oracle-external-tables" && outcome.owner_ordinal == Some(2)
            })
            .unwrap();
        let duplicate = OracleRow {
            values: vec![text("OTHER"), text("ORDERS")],
        };
        other_external.rows = vec![duplicate.clone(), duplicate];
        other_external.status = OracleQueryStatus::Executed { rows: 2 };
        capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tables" && outcome.owner_ordinal == Some(2))
            .unwrap()
            .rows[0]
            .values[3] = number(900);

        let blueprint = map_oracle_basic_capture(
            &capture,
            &scope,
            OracleVersion::from_components(&[12, 1, 0, 2]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("one malformed external catalog must not erase another owner's classification");

        let external = blueprint
            .tables
            .values()
            .find(|table| table.object_kind == "external-table")
            .expect("APP external table");
        assert_eq!(external.counted_in_totals, Some(false));
        assert!(!external
            .table_limitations
            .contains(&"table-classification-unavailable".to_string()));
        let affected = blueprint
            .tables
            .values()
            .find(|table| table.rows == 900)
            .expect("OTHER table");
        assert!(affected
            .table_limitations
            .contains(&"table-classification-unavailable".to_string()));
    }

    #[test]
    fn malformed_object_table_catalog_is_disproved_for_an_ordinary_table_only_owner() {
        let mut capture = fixture();
        let scope = append_second_owner(&mut capture, "OTHER", 21);

        let base_table = capture
            .outcomes
            .iter()
            .find(|outcome| outcome.query_id == "oracle-tables" && outcome.owner_ordinal == Some(1))
            .unwrap()
            .rows[0]
            .clone();
        let mut object_table = base_table;
        object_table.values[1] = text("OBJECT_PAYLOAD");
        object_table.values[3] = number(700);
        let app_object_tables = capture
            .outcomes
            .iter_mut()
            .find(|outcome| {
                outcome.query_id == "oracle-object-tables" && outcome.owner_ordinal == Some(1)
            })
            .unwrap();
        app_object_tables.rows = vec![object_table.clone()];
        app_object_tables.status = OracleQueryStatus::Executed { rows: 1 };

        let other_object_tables = capture
            .outcomes
            .iter_mut()
            .find(|outcome| {
                outcome.query_id == "oracle-object-tables" && outcome.owner_ordinal == Some(2)
            })
            .unwrap();
        object_table.values[0] = text("OTHER");
        object_table.values[1] = OracleValue::Null;
        other_object_tables.rows = vec![object_table];
        other_object_tables.status = OracleQueryStatus::Executed { rows: 1 };

        let app_objects = capture
            .outcomes
            .iter_mut()
            .find(|outcome| {
                outcome.query_id == "oracle-objects" && outcome.owner_ordinal == Some(1)
            })
            .unwrap();
        let mut object = app_objects.rows[0].clone();
        object.values[1] = text("OBJECT_PAYLOAD");
        app_objects.rows.push(object);
        app_objects.status = OracleQueryStatus::Executed {
            rows: app_objects.rows.len() as u64,
        };
        let app_columns = capture
            .outcomes
            .iter_mut()
            .find(|outcome| {
                outcome.query_id == "oracle-tab-columns" && outcome.owner_ordinal == Some(1)
            })
            .unwrap();
        let mut column = app_columns.rows[0].clone();
        column.values[1] = text("OBJECT_PAYLOAD");
        app_columns.rows.push(column);
        app_columns.status = OracleQueryStatus::Executed {
            rows: app_columns.rows.len() as u64,
        };
        let app_hidden = capture
            .outcomes
            .iter_mut()
            .find(|outcome| {
                outcome.query_id == "oracle-tab-cols-hidden" && outcome.owner_ordinal == Some(1)
            })
            .unwrap();
        let mut hidden = app_hidden.rows[0].clone();
        hidden.values[1] = text("OBJECT_PAYLOAD");
        app_hidden.rows.push(hidden);
        app_hidden.status = OracleQueryStatus::Executed {
            rows: app_hidden.rows.len() as u64,
        };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &scope,
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("one malformed object-table catalog must not erase another owner's rows");

        assert!(blueprint.tables.values().any(|table| table.rows == 700));
        assert_eq!(
            blueprint
                .structure_scope
                .as_ref()
                .unwrap()
                .table_inventory_completeness,
            "complete"
        );
    }

    #[test]
    fn truncated_floor_owner_cannot_leave_artifact_summary_complete() {
        let mut capture = fixture();
        let scope = append_second_owner(&mut capture, "OTHER", 21);
        let other_objects = capture
            .outcomes
            .iter_mut()
            .find(|outcome| {
                outcome.query_id == "oracle-objects" && outcome.owner_ordinal == Some(2)
            })
            .unwrap();
        other_objects.rows[0].values[2] = text("SEQUENCE");
        let other_tables = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tables" && outcome.owner_ordinal == Some(2))
            .unwrap();
        other_tables.rows.clear();
        other_tables.status = OracleQueryStatus::Failed {
            class: OracleQueryFailure::PermissionDenied,
        };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &scope,
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::Summary,
            },
        )
        .expect("a readable owner must survive another owner's missing floor");

        let inventory = blueprint.artifact_inventory.as_ref().unwrap();
        assert_eq!(inventory.scope, "selected-schemas");
        assert_eq!(inventory.visibility, "unknown");
        assert!(!inventory.inventory_complete);
        assert_eq!(inventory.object_count, 0);
        assert!(blueprint
            .dataset_scope
            .as_ref()
            .unwrap()
            .limitations
            .contains(&"catalog-capture-truncated".to_string()));
    }

    #[test]
    fn malformed_optional_families_degrade_only_their_native_owner() {
        let mut capture = fixture();
        let query_by_id = CATALOG_QUERIES
            .iter()
            .map(|query| (query.query_id, query))
            .collect::<BTreeMap<_, _>>();
        let mut second_owner = capture
            .outcomes
            .iter()
            .filter(|outcome| outcome.owner_ordinal.is_some())
            .cloned()
            .collect::<Vec<_>>();
        for outcome in &mut second_owner {
            outcome.owner_ordinal = Some(2);
            let query = query_by_id[outcome.query_id];
            let columns = query.output_columns(21);
            let owner_index = columns
                .iter()
                .position(|column| {
                    query
                        .owner_column
                        .is_some_and(|owner| column.eq_ignore_ascii_case(owner))
                })
                .unwrap();
            for row in &mut outcome.rows {
                row.values[owner_index] = text("OTHER");
            }
            if outcome.query_id == "oracle-tables" {
                outcome.rows[0].values[3] = number(900);
                outcome.rows[0].values[7] = text("YES");
            }
        }
        let malformed_indexes = second_owner
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-indexes")
            .unwrap();
        malformed_indexes.rows = vec![OracleRow {
            values: vec![
                text("OTHER"),
                text("ORDERS_IX"),
                text("OTHER"),
                text("ORDERS"),
                text("NORMAL"),
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                text("NO"),
                text("VALID"),
                text("N"),
                text("NO"),
                text("VISIBLE"),
                text("NO"),
                text("YES"),
            ],
        }];
        malformed_indexes.status = OracleQueryStatus::Executed { rows: 1 };
        let malformed_constraints = second_owner
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-constraints")
            .unwrap();
        malformed_constraints.rows = vec![OracleRow {
            values: vec![
                text("OTHER"),
                text("ORDERS_CK"),
                text("C"),
                text("ORDERS"),
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                text("UNRECOGNISED"),
                text("NOT DEFERRABLE"),
                text("VALIDATED"),
                text("IMMEDIATE"),
                OracleValue::Null,
                OracleValue::Null,
                text("USER NAME"),
                text("N"),
            ],
        }];
        malformed_constraints.status = OracleQueryStatus::Executed { rows: 1 };
        let malformed_partitions = second_owner
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-part-tables")
            .unwrap();
        malformed_partitions.rows = vec![OracleRow {
            values: vec![
                text("OTHER"),
                text("ORDERS"),
                OracleValue::Null,
                text("NONE"),
                number(1),
                OracleValue::Null,
            ],
        }];
        malformed_partitions.status = OracleQueryStatus::Executed { rows: 1 };
        let malformed_segments = second_owner
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-segments")
            .unwrap();
        malformed_segments.rows[0].values[4] = OracleValue::Null;
        capture.outcomes.extend(second_owner);

        let scope = resolve_oracle_owner_scope(
            OracleOwnerSelection::Selected(vec!["APP".into(), "OTHER".into()]),
            "APP",
            vec!["APP".into(), "OTHER".into()],
        )
        .unwrap();
        let blueprint = map_oracle_basic_capture(
            &capture,
            &scope,
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("malformed optional rows must not erase another owner's readable refinements");

        let healthy = blueprint
            .tables
            .values()
            .find(|table| table.rows == 100)
            .expect("APP fixture table");
        assert_eq!(healthy.partitioning, "none");
        assert_eq!(healthy.check_count, Some(0));
        assert_eq!(
            healthy.statistics.as_ref().unwrap().size_method,
            "oracle-segment-bytes"
        );
        assert!(!healthy
            .table_limitations
            .contains(&"index-inventory-unavailable".to_string()));
        assert!(!healthy
            .table_limitations
            .contains(&"relationship-inventory-unavailable".to_string()));

        let affected = blueprint
            .tables
            .values()
            .find(|table| table.rows == 900)
            .expect("OTHER fixture table");
        assert_eq!(affected.partitioning, "unknown");
        assert_eq!(affected.check_count, None);
        assert_eq!(
            affected.statistics.as_ref().unwrap().size_method,
            "oracle-table-logical-estimate"
        );
        assert!(affected
            .table_limitations
            .contains(&"index-inventory-unavailable".to_string()));
        assert!(affected
            .table_limitations
            .contains(&"relationship-inventory-unavailable".to_string()));
        let structure = blueprint.structure_scope.as_ref().unwrap();
        assert_eq!(structure.index_inventory_completeness, "incomplete");
        assert_eq!(structure.relationship_inventory_completeness, "incomplete");
    }

    #[test]
    fn readable_tableless_owner_does_not_degrade_a_multi_owner_capture() {
        let mut capture = fixture();
        let mut second_owner = capture
            .outcomes
            .iter()
            .filter(|outcome| outcome.owner_ordinal.is_some())
            .cloned()
            .collect::<Vec<_>>();
        for outcome in &mut second_owner {
            outcome.owner_ordinal = Some(2);
            outcome.rows.clear();
            if outcome.query_id == "oracle-objects" {
                outcome.rows.push(OracleRow {
                    values: vec![
                        text("OTHER"),
                        text("COUNTER_SEQ"),
                        text("SEQUENCE"),
                        text("VALID"),
                        text("N"),
                        text("N"),
                        text("N"),
                        text("Y"),
                    ],
                });
            }
            outcome.status = OracleQueryStatus::Executed {
                rows: outcome.rows.len() as u64,
            };
        }
        capture.outcomes.extend(second_owner);
        let scope = resolve_oracle_owner_scope(
            OracleOwnerSelection::Selected(vec!["APP".into(), "OTHER".into()]),
            "APP",
            vec!["APP".into(), "OTHER".into()],
        )
        .unwrap();

        let blueprint = map_oracle_basic_capture(
            &capture,
            &scope,
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::Summary,
            },
        )
        .unwrap();
        assert_eq!(blueprint.tables.len(), 1);
        let dataset = blueprint.dataset_scope.as_ref().unwrap();
        assert_eq!(dataset.table_inventory_completeness, "complete");
        assert_eq!(dataset.row_count_completeness, "complete");
        assert_eq!(dataset.size_completeness, "complete");
        assert!(!dataset
            .limitations
            .contains(&"catalog-capture-truncated".to_string()));
    }

    #[test]
    fn tableless_owner_denial_preserves_positive_catalog_evidence_but_withdraws_family_completeness(
    ) {
        for (query_id, catalog, index_family) in [
            ("oracle-indexes", "oracle-indexes", true),
            ("oracle-index-columns", "oracle-indexes", true),
            ("oracle-index-expressions", "oracle-indexes", true),
            ("oracle-constraints", "oracle-constraints", false),
            ("oracle-constraint-columns", "oracle-constraints", false),
        ] {
            let mut capture = fixture();
            let mut second_owner = capture
                .outcomes
                .iter()
                .filter(|outcome| outcome.owner_ordinal.is_some())
                .cloned()
                .collect::<Vec<_>>();
            for outcome in &mut second_owner {
                outcome.owner_ordinal = Some(2);
                outcome.rows.clear();
                if outcome.query_id == "oracle-objects" {
                    outcome.rows.push(OracleRow {
                        values: vec![
                            text("OTHER"),
                            text("COUNTER_SEQ"),
                            text("SEQUENCE"),
                            text("VALID"),
                            text("N"),
                            text("N"),
                            text("N"),
                            text("Y"),
                        ],
                    });
                }
                outcome.status = if outcome.query_id == query_id {
                    OracleQueryStatus::Failed {
                        class: OracleQueryFailure::PermissionDenied,
                    }
                } else {
                    OracleQueryStatus::Executed {
                        rows: outcome.rows.len() as u64,
                    }
                };
            }
            capture.outcomes.extend(second_owner);
            let scope = resolve_oracle_owner_scope(
                OracleOwnerSelection::Selected(vec!["APP".into(), "OTHER".into()]),
                "APP",
                vec!["APP".into(), "OTHER".into()],
            )
            .unwrap();

            let blueprint = map_oracle_basic_capture(
                &capture,
                &scope,
                OracleVersion::from_components(&[21]).unwrap(),
                &OracleBasicOptions {
                    source_kind: "production".to_string(),
                    generated_at_pin: None,
                    artifact_detail: ArtifactDetail::None,
                },
            )
            .expect("a tableless owner's refinement denial cannot erase a healthy floor");

            assert_eq!(blueprint.tables.len(), 1);
            let structure = blueprint.structure_scope.as_ref().unwrap();
            assert!(structure.catalogs_read.contains(&catalog.to_string()));
            assert!(!structure.catalogs_unreadable.contains(&catalog.to_string()));
            if index_family {
                assert_eq!(structure.index_inventory_completeness, "incomplete");
                assert!(structure
                    .limitations
                    .contains(&"index-inventory-unavailable".to_string()));
            } else {
                assert_eq!(structure.relationship_inventory_completeness, "incomplete");
                assert!(structure
                    .limitations
                    .contains(&"relationship-inventory-unavailable".to_string()));
            }
        }
    }

    #[test]
    fn empty_inventory_keeps_denied_structure_catalogs_unreadable() {
        let mut capture = fixture();
        for outcome in &mut capture.outcomes {
            if matches!(
                outcome.query_id,
                "oracle-objects" | "oracle-tables" | "oracle-tab-columns"
            ) {
                outcome.rows.clear();
                outcome.status = OracleQueryStatus::Executed { rows: 0 };
            } else if matches!(outcome.query_id, "oracle-indexes" | "oracle-constraints") {
                outcome.rows.clear();
                outcome.status = OracleQueryStatus::Failed {
                    class: OracleQueryFailure::PermissionDenied,
                };
            }
        }

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("an empty readable floor retains negative refinement provenance");

        let structure = blueprint.structure_scope.as_ref().unwrap();
        for catalog in ["oracle-indexes", "oracle-constraints"] {
            assert!(!structure.catalogs_read.contains(&catalog.to_string()));
            assert!(structure.catalogs_unreadable.contains(&catalog.to_string()));
        }
        assert_eq!(structure.index_inventory_completeness, "incomplete");
        assert_eq!(structure.relationship_inventory_completeness, "incomplete");
    }

    #[test]
    fn unreadable_segments_for_tableless_owner_degrade_statistics_without_aborting() {
        let mut capture = fixture();
        let mut second_owner = capture
            .outcomes
            .iter()
            .filter(|outcome| outcome.owner_ordinal.is_some())
            .cloned()
            .collect::<Vec<_>>();
        for outcome in &mut second_owner {
            outcome.owner_ordinal = Some(2);
            outcome.rows.clear();
            if outcome.query_id == "oracle-objects" {
                outcome.rows.push(OracleRow {
                    values: vec![
                        text("OTHER"),
                        text("COUNTER_SEQ"),
                        text("SEQUENCE"),
                        text("VALID"),
                        text("N"),
                        text("N"),
                        text("N"),
                        text("Y"),
                    ],
                });
            }
            outcome.status = if outcome.query_id == "oracle-segments" {
                OracleQueryStatus::Failed {
                    class: OracleQueryFailure::PermissionDenied,
                }
            } else {
                OracleQueryStatus::Executed {
                    rows: outcome.rows.len() as u64,
                }
            };
        }
        capture.outcomes.extend(second_owner);
        let scope = resolve_oracle_owner_scope(
            OracleOwnerSelection::Selected(vec!["APP".into(), "OTHER".into()]),
            "APP",
            vec!["APP".into(), "OTHER".into()],
        )
        .unwrap();

        let blueprint = map_oracle_basic_capture(
            &capture,
            &scope,
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::Summary,
            },
        )
        .expect("an unreadable refinement catalog for a tableless owner cannot erase the floor");

        assert_eq!(blueprint.tables.len(), 1);
        assert_eq!(
            blueprint.dataset_scope.as_ref().unwrap().size_completeness,
            "complete"
        );
        let aggregate = blueprint.statistics_evidence.as_ref().unwrap();
        assert_eq!(aggregate.visibility, "partial");
        assert!(aggregate
            .catalogs_read
            .contains(&"oracle-segments".to_string()));
        assert!(!aggregate
            .catalogs_unreadable
            .contains(&"oracle-segments".to_string()));
        assert!(aggregate
            .limitations
            .contains(&"statistics-partial".to_string()));
    }

    #[test]
    fn partial_owner_reads_retain_positive_catalog_evidence_after_truncation() {
        let mut capture = fixture();
        let mut second_owner = capture
            .outcomes
            .iter()
            .filter(|outcome| outcome.owner_ordinal.is_some())
            .cloned()
            .collect::<Vec<_>>();
        for outcome in &mut second_owner {
            outcome.owner_ordinal = Some(2);
            outcome.rows.clear();
            if outcome.query_id == "oracle-objects" {
                outcome.rows.push(OracleRow {
                    values: vec![
                        text("OTHER"),
                        text("COUNTER_SEQ"),
                        text("SEQUENCE"),
                        text("VALID"),
                        text("N"),
                        text("N"),
                        text("N"),
                        text("Y"),
                    ],
                });
            }
            outcome.status = if matches!(outcome.query_id, "oracle-indexes" | "oracle-segments") {
                OracleQueryStatus::NotReached
            } else {
                OracleQueryStatus::Executed {
                    rows: outcome.rows.len() as u64,
                }
            };
        }
        capture.outcomes.extend(second_owner);
        capture.abort = Some(OracleCaptureAbort::DeadlineExceeded);
        let scope = resolve_oracle_owner_scope(
            OracleOwnerSelection::Selected(vec!["APP".into(), "OTHER".into()]),
            "APP",
            vec!["APP".into(), "OTHER".into()],
        )
        .unwrap();

        let blueprint = map_oracle_basic_capture(
            &capture,
            &scope,
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::Summary,
            },
        )
        .expect("a truncated second owner cannot erase the first owner's positive reads");

        let structure = blueprint.structure_scope.as_ref().unwrap();
        assert!(structure
            .catalogs_read
            .contains(&"oracle-indexes".to_string()));
        assert!(!structure
            .catalogs_unreadable
            .contains(&"oracle-indexes".to_string()));
        assert!(structure
            .limitations
            .contains(&"catalog-capture-truncated".to_string()));
        let aggregate = blueprint.statistics_evidence.as_ref().unwrap();
        assert!(aggregate
            .catalogs_read
            .contains(&"oracle-segments".to_string()));
        assert!(!aggregate
            .catalogs_unreadable
            .contains(&"oracle-segments".to_string()));
        assert!(aggregate
            .limitations
            .contains(&"catalog-capture-truncated".to_string()));
        assert_eq!(aggregate.visibility, "partial");
    }

    #[test]
    fn one_owner_segment_failure_does_not_erase_another_owners_measured_bytes() {
        let mut capture = fixture();
        let query_by_id = CATALOG_QUERIES
            .iter()
            .map(|query| (query.query_id, query))
            .collect::<BTreeMap<_, _>>();
        let mut second_owner = capture
            .outcomes
            .iter()
            .filter(|outcome| outcome.owner_ordinal.is_some())
            .cloned()
            .collect::<Vec<_>>();
        for outcome in &mut second_owner {
            outcome.owner_ordinal = Some(2);
            let query = query_by_id[outcome.query_id];
            let columns = query.output_columns(21);
            let owner_index = columns
                .iter()
                .position(|column| {
                    query
                        .owner_column
                        .is_some_and(|owner| column.eq_ignore_ascii_case(owner))
                })
                .unwrap();
            for row in &mut outcome.rows {
                row.values[owner_index] = text("OTHER");
            }
            if outcome.query_id == "oracle-segments" {
                outcome.status = OracleQueryStatus::Failed {
                    class: OracleQueryFailure::PermissionDenied,
                };
                outcome.rows.clear();
            }
        }
        capture.outcomes.extend(second_owner);
        let scope = resolve_oracle_owner_scope(
            OracleOwnerSelection::Selected(vec!["APP".into(), "OTHER".into()]),
            "APP",
            vec!["APP".into(), "OTHER".into()],
        )
        .unwrap();

        let blueprint = map_oracle_basic_capture(
            &capture,
            &scope,
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        let methods = blueprint
            .tables
            .values()
            .map(|table| table.statistics.as_ref().unwrap().size_method.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            methods
                .iter()
                .filter(|method| **method == "oracle-segment-bytes")
                .count(),
            1
        );
        assert_eq!(
            methods
                .iter()
                .filter(|method| **method == "oracle-table-logical-estimate")
                .count(),
            1
        );
        assert_eq!(
            blueprint.dataset_scope.as_ref().unwrap().size_completeness,
            "incomplete"
        );
        assert_eq!(
            blueprint.dataset_scope.as_ref().unwrap().size_method,
            "mixed"
        );
    }

    #[test]
    fn cross_owner_index_segments_are_summed_into_the_logical_table() {
        let mut capture = fixture();
        let scope = append_second_owner(&mut capture, "OTHER", 21);

        // Keep the second owner tableless while retaining its independently
        // scoped catalog outcomes. Its only physical contribution is an index
        // segment owned by OTHER for APP.ORDERS.
        for outcome in capture
            .outcomes
            .iter_mut()
            .filter(|outcome| outcome.owner_ordinal == Some(2))
        {
            outcome.rows.clear();
            outcome.status = OracleQueryStatus::Executed { rows: 0 };
        }

        append_test_index(
            &mut capture,
            1,
            Some(1),
            "APP",
            "ORDERS_LOCAL_IX",
            "APP",
            "ORDERS",
            8192,
        );
        append_test_index(
            &mut capture,
            1,
            Some(2),
            "OTHER",
            "ORDERS_CROSS_IX",
            "APP",
            "ORDERS",
            8192,
        );

        let blueprint = map_oracle_basic_capture(
            &capture,
            &scope,
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("cross-owner index bytes must remain additive and contract-valid");

        let table = blueprint.tables.values().next().unwrap();
        assert_eq!(table.index_bytes, 16_384);
        assert_eq!(blueprint.totals.index_bytes, 16_384);
        assert_eq!(table.idxs.len(), 2);
        let statistics = table.statistics.as_ref().unwrap();
        assert_eq!(statistics.size_method, "oracle-segment-bytes");
        assert_eq!(statistics.size_quality, "exact-counter");
        assert_eq!(statistics.size_visibility, "full");
        assert_eq!(
            blueprint.dataset_scope.as_ref().unwrap().size_completeness,
            "complete"
        );
    }

    #[test]
    fn unresolved_cross_owner_index_storage_remains_partial() {
        let mut outside_scope = fixture();
        append_test_index(
            &mut outside_scope,
            1,
            None,
            "OTHER",
            "ORDERS_CROSS_IX",
            "APP",
            "ORDERS",
            8192,
        );
        let blueprint = map_oracle_basic_capture(
            &outside_scope,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("an outside-scope index owner must degrade rather than abort");
        assert_eq!(
            blueprint
                .tables
                .values()
                .next()
                .unwrap()
                .statistics
                .as_ref()
                .unwrap()
                .size_visibility,
            "partial"
        );
        assert_eq!(
            blueprint.dataset_scope.as_ref().unwrap().size_completeness,
            "incomplete"
        );

        let mut denied_owner = fixture();
        let scope = append_second_owner(&mut denied_owner, "OTHER", 21);
        for outcome in denied_owner
            .outcomes
            .iter_mut()
            .filter(|outcome| outcome.owner_ordinal == Some(2))
        {
            outcome.rows.clear();
            outcome.status = if outcome.query_id == "oracle-segments" {
                OracleQueryStatus::Failed {
                    class: OracleQueryFailure::PermissionDenied,
                }
            } else {
                OracleQueryStatus::Executed { rows: 0 }
            };
        }
        append_test_index(
            &mut denied_owner,
            1,
            None,
            "OTHER",
            "ORDERS_CROSS_IX",
            "APP",
            "ORDERS",
            8192,
        );
        let blueprint = map_oracle_basic_capture(
            &denied_owner,
            &scope,
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("a denied index-owner segment catalog must degrade rather than abort");
        assert_eq!(
            blueprint
                .tables
                .values()
                .next()
                .unwrap()
                .statistics
                .as_ref()
                .unwrap()
                .size_visibility,
            "partial"
        );

        let mut selected_owner_without_segment = fixture();
        let scope = append_second_owner(&mut selected_owner_without_segment, "OTHER", 21);
        for outcome in selected_owner_without_segment
            .outcomes
            .iter_mut()
            .filter(|outcome| outcome.owner_ordinal == Some(2))
        {
            outcome.rows.clear();
            outcome.status = OracleQueryStatus::Executed { rows: 0 };
        }
        append_test_index(
            &mut selected_owner_without_segment,
            1,
            None,
            "OTHER",
            "ORDERS_CROSS_IX",
            "APP",
            "ORDERS",
            8192,
        );
        let blueprint = map_oracle_basic_capture(
            &selected_owner_without_segment,
            &scope,
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("a catalogued index without a segment must degrade rather than abort");
        assert_eq!(
            blueprint
                .tables
                .values()
                .next()
                .unwrap()
                .statistics
                .as_ref()
                .unwrap()
                .size_visibility,
            "partial"
        );
        assert_eq!(
            blueprint.dataset_scope.as_ref().unwrap().size_completeness,
            "incomplete"
        );
    }

    #[test]
    fn unmapped_mapping_dependent_segments_withdraw_completeness_but_keep_known_bytes() {
        let mut capture = fixture();
        append_test_index(
            &mut capture,
            1,
            Some(1),
            "APP",
            "ORDERS_KNOWN_IX",
            "APP",
            "ORDERS",
            4096,
        );
        let segments = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-segments")
            .unwrap();
        for (name, segment_type) in [
            ("UNMAPPED_IX", "INDEX"),
            ("UNMAPPED_LOB", "LOBSEGMENT"),
            ("UNMAPPED_LOB_IX", "LOBINDEX"),
        ] {
            segments.rows.push(OracleRow {
                values: vec![
                    text("APP"),
                    text(name),
                    text(segment_type),
                    text("USERS"),
                    number(8192),
                    number(1),
                ],
            });
        }
        segments.status = OracleQueryStatus::Executed {
            rows: segments.rows.len() as u64,
        };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("unmapped refinement segments must degrade rather than abort");
        let table = blueprint.tables.values().next().unwrap();
        assert_eq!(table.table_bytes, 8192);
        assert_eq!(table.index_bytes, 4096);
        assert_eq!(blueprint.totals.table_bytes, 8192);
        assert_eq!(blueprint.totals.index_bytes, 4096);
        assert_eq!(
            table.statistics.as_ref().unwrap().size_visibility,
            "partial"
        );
        assert_eq!(
            blueprint.dataset_scope.as_ref().unwrap().size_completeness,
            "incomplete"
        );
    }

    #[test]
    fn unattributed_segment_from_a_tableless_owner_does_not_degrade_other_owners() {
        let mut capture = fixture();
        let scope = append_second_owner(&mut capture, "OTHER", 21);
        for outcome in capture
            .outcomes
            .iter_mut()
            .filter(|outcome| outcome.owner_ordinal == Some(2))
        {
            outcome.rows.clear();
            outcome.status = OracleQueryStatus::Executed { rows: 0 };
        }
        let segments = capture
            .outcomes
            .iter_mut()
            .find(|outcome| {
                outcome.query_id == "oracle-segments" && outcome.owner_ordinal == Some(2)
            })
            .unwrap();
        segments.rows.push(OracleRow {
            values: vec![
                text("OTHER"),
                text("RACING_CROSS_OWNER_IX"),
                text("INDEX"),
                text("USERS"),
                number(8192),
                number(1),
            ],
        });
        segments.status = OracleQueryStatus::Executed { rows: 1 };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &scope,
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("an unattributed table-less-owner segment must not abort the floor");
        assert_eq!(
            blueprint
                .tables
                .values()
                .next()
                .unwrap()
                .statistics
                .as_ref()
                .unwrap()
                .size_visibility,
            "full"
        );
        assert_eq!(
            blueprint.dataset_scope.as_ref().unwrap().size_completeness,
            "complete"
        );
    }

    #[test]
    fn unattributed_lob_segment_degrades_only_its_owner() {
        let mut capture = fixture();
        let scope = append_second_owner(&mut capture, "OTHER", 21);
        let segments = capture
            .outcomes
            .iter_mut()
            .find(|outcome| {
                outcome.query_id == "oracle-segments" && outcome.owner_ordinal == Some(1)
            })
            .unwrap();
        segments.rows.push(OracleRow {
            values: vec![
                text("APP"),
                text("RACING_LOB_SEGMENT"),
                text("LOBSEGMENT"),
                text("USERS"),
                number(8192),
                number(1),
            ],
        });
        segments.status = OracleQueryStatus::Executed {
            rows: segments.rows.len() as u64,
        };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &scope,
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("an unattributed LOB segment must remain owner-scoped");
        let mut visibility = blueprint
            .tables
            .values()
            .map(|table| table.statistics.as_ref().unwrap().size_visibility.as_str())
            .collect::<Vec<_>>();
        visibility.sort_unstable();
        assert_eq!(visibility, vec!["full", "partial"]);
        assert_eq!(
            blueprint.dataset_scope.as_ref().unwrap().size_completeness,
            "incomplete"
        );
    }

    #[test]
    fn malformed_segment_bytes_preserve_other_attributed_contributions() {
        let mut capture = fixture();
        append_test_index(
            &mut capture,
            1,
            Some(1),
            "APP",
            "ORDERS_KNOWN_IX",
            "APP",
            "ORDERS",
            4096,
        );
        let segments = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-segments")
            .unwrap();
        segments.rows.push(OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS"),
                text("TABLE"),
                text("USERS"),
                OracleValue::Null,
                number(1),
            ],
        });
        segments.status = OracleQueryStatus::Executed {
            rows: segments.rows.len() as u64,
        };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("one malformed segment must not erase valid same-owner bytes");
        let table = blueprint.tables.values().next().unwrap();
        assert_eq!(table.table_bytes, 8192);
        assert_eq!(table.index_bytes, 4096);
        assert_eq!(
            table.statistics.as_ref().unwrap().size_scope,
            "unknown",
            "partial measured evidence must not claim table-only scope while retaining index bytes"
        );
        assert_eq!(
            table.statistics.as_ref().unwrap().size_visibility,
            "partial"
        );
        assert_eq!(
            blueprint.dataset_scope.as_ref().unwrap().size_completeness,
            "incomplete"
        );
    }

    #[test]
    fn deferred_index_without_segment_is_a_proven_zero_allocation() {
        let mut capture = fixture();
        capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tables")
            .unwrap()
            .rows[0]
            .values[3] = number(0);
        append_test_index(
            &mut capture,
            1,
            None,
            "APP",
            "ORDERS_DEFERRED_IX",
            "APP",
            "ORDERS",
            0,
        );
        let indexes = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-indexes")
            .unwrap();
        indexes.rows.last_mut().unwrap().values[17] = text("NO");

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("deferred segment creation must not look like missing size evidence");
        let table = blueprint.tables.values().next().unwrap();
        assert_eq!(table.index_bytes, 0);
        let size = table.statistics.as_ref().unwrap();
        assert_eq!(size.size_scope, "table-lob-and-index");
        assert_eq!(size.size_visibility, "full");
        assert_eq!(
            blueprint.dataset_scope.as_ref().unwrap().size_completeness,
            "complete"
        );
    }

    #[test]
    fn partitioned_lob_data_and_index_segments_are_attributed_by_base_name() {
        let mut capture = fixture();
        let lobs = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-lobs")
            .unwrap();
        lobs.rows.push(OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS"),
                text("PAYLOAD"),
                text("SYS_LOB_ORDERS_PAYLOAD"),
                text("SYS_IL_ORDERS_PAYLOAD"),
                number(8192),
                text("YES"),
                text("YES"),
                text("NONE"),
                text("NO"),
                text("NO"),
            ],
        });
        lobs.status = OracleQueryStatus::Executed { rows: 1 };
        let segments = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-segments")
            .unwrap();
        segments.rows.extend([
            OracleRow {
                values: vec![
                    text("APP"),
                    text("SYS_LOB_ORDERS_PAYLOAD"),
                    text("LOB PARTITION"),
                    text("USERS"),
                    number(16384),
                    number(2),
                ],
            },
            OracleRow {
                values: vec![
                    text("APP"),
                    text("SYS_IL_ORDERS_PAYLOAD"),
                    text("INDEX PARTITION"),
                    text("USERS"),
                    number(4096),
                    number(1),
                ],
            },
        ]);
        segments.status = OracleQueryStatus::Executed {
            rows: segments.rows.len() as u64,
        };

        let lob_facts = collect_lob_facts(&capture, 1).unwrap();
        let tables = collect_tables(&capture).unwrap();
        let indexes = collect_indexes(&capture, 1).unwrap();
        let lookup = build_segment_attribution_lookup(&tables, &indexes);
        let collected = collect_segment_bytes(
            &capture,
            &lob_facts,
            &BTreeMap::new(),
            &lookup,
            &tables,
            &indexes,
            1,
            "APP",
        )
        .expect("partitioned LOB rows must map through the existing DBA_LOBS names");
        let key = ("APP".to_string(), "ORDERS".to_string());
        assert_eq!(collected.table_bytes.get(&key), Some(&(8192 + 16384)));
        assert_eq!(collected.index_bytes.get(&key), Some(&4096));
        assert!(!collected.lob_attribution_incomplete);
    }

    #[test]
    fn null_or_malformed_only_segment_without_statistics_is_never_measured_zero() {
        for unreadable_bytes in [OracleValue::Null, text("not-a-number")] {
            let mut capture = fixture();
            let tables = capture
                .outcomes
                .iter_mut()
                .find(|outcome| outcome.query_id == "oracle-tables")
                .unwrap();
            tables.rows[0].values[3] = OracleValue::Null;
            let segments = capture
                .outcomes
                .iter_mut()
                .find(|outcome| outcome.query_id == "oracle-segments")
                .unwrap();
            segments.rows[0].values[4] = unreadable_bytes;

            let blueprint = map_oracle_basic_capture(
                &capture,
                &OracleOwnerScope::one("APP"),
                OracleVersion::from_components(&[21]).unwrap(),
                &OracleBasicOptions {
                    source_kind: "production".to_string(),
                    generated_at_pin: None,
                    artifact_detail: ArtifactDetail::None,
                },
            )
            .expect("an unreadable sole segment counter must degrade rather than abort");
            let table = blueprint.tables.values().next().unwrap();
            let size = table.statistics.as_ref().unwrap();
            assert_eq!(table.table_bytes, 0);
            assert_eq!(table.index_bytes, 0);
            assert_eq!(table.segment_state, "unavailable");
            assert_eq!(size.size_method, "unknown");
            assert_eq!(size.size_quality, "unavailable");
            assert_eq!(size.size_accounting, "unknown");
            assert_eq!(size.size_visibility, "unavailable");
            assert_eq!(
                blueprint.dataset_scope.as_ref().unwrap().size_completeness,
                "incomplete"
            );
            assert_eq!(
                blueprint.dataset_scope.as_ref().unwrap().size_method,
                "unknown"
            );
        }
    }

    #[test]
    fn cross_owner_segment_merge_overflow_marks_the_table_incomplete() {
        let key = ("APP".to_string(), "ORDERS".to_string());
        let mut target = BTreeMap::from([(key.clone(), u64::MAX)]);
        let source = BTreeMap::from([(key.clone(), 1)]);
        let incomplete = merge_segment_bytes(&mut target, source);
        assert_eq!(incomplete, BTreeSet::from([key.clone()]));
        assert_eq!(target.get(&key), Some(&u64::MAX));
    }

    #[test]
    fn cross_owner_segment_merge_overflow_preserves_the_floor_blueprint() {
        let mut capture = fixture();
        let scope = append_second_owner(&mut capture, "OTHER", 21);
        for outcome in capture
            .outcomes
            .iter_mut()
            .filter(|outcome| outcome.owner_ordinal == Some(2))
        {
            outcome.rows.clear();
            outcome.status = OracleQueryStatus::Executed { rows: 0 };
        }
        append_test_index(
            &mut capture,
            1,
            Some(1),
            "APP",
            "ORDERS_LOCAL_IX",
            "APP",
            "ORDERS",
            u64::MAX,
        );
        append_test_index(
            &mut capture,
            1,
            Some(2),
            "OTHER",
            "ORDERS_CROSS_IX",
            "APP",
            "ORDERS",
            1,
        );

        let blueprint = map_oracle_basic_capture(
            &capture,
            &scope,
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("cross-owner overflow must preserve a degraded floor Blueprint");
        let table = blueprint.tables.values().next().unwrap();
        assert!(table.index_bytes > 0);
        assert_eq!(
            table.statistics.as_ref().unwrap().size_visibility,
            "partial"
        );
        assert_eq!(
            blueprint.dataset_scope.as_ref().unwrap().size_completeness,
            "incomplete"
        );
    }

    #[test]
    fn one_owner_refinement_denials_preserve_another_owners_structure_and_statistics() {
        let mut capture = fixture();
        capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tables")
            .unwrap()
            .rows[0]
            .values[7] = text("YES");

        let indexes = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-indexes")
            .unwrap();
        indexes.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_IX"),
                text("APP"),
                text("ORDERS"),
                text("NORMAL"),
                text("NONUNIQUE"),
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                text("NO"),
                text("VALID"),
                text("N"),
                text("NO"),
                text("VISIBLE"),
                text("NO"),
                text("YES"),
            ],
        }];
        indexes.status = OracleQueryStatus::Executed { rows: 1 };
        let index_columns = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-index-columns")
            .unwrap();
        index_columns.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_IX"),
                text("ORDERS"),
                text("ID"),
                number(1),
                text("ASC"),
                text("APP"),
            ],
        }];
        index_columns.status = OracleQueryStatus::Executed { rows: 1 };

        let constraints = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-constraints")
            .unwrap();
        constraints.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_AMOUNT_CK"),
                text("C"),
                text("ORDERS"),
                OracleValue::Null,
                OracleValue::Null,
                text("NO ACTION"),
                text("ENABLED"),
                text("NOT DEFERRABLE"),
                text("VALIDATED"),
                text("IMMEDIATE"),
                OracleValue::Null,
                OracleValue::Null,
                text("USER NAME"),
                text("N"),
            ],
        }];
        constraints.status = OracleQueryStatus::Executed { rows: 1 };

        let part_tables = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-part-tables")
            .unwrap();
        part_tables.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS"),
                text("RANGE"),
                text("NONE"),
                number(1),
                OracleValue::Null,
            ],
        }];
        part_tables.status = OracleQueryStatus::Executed { rows: 1 };
        let partitions = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tab-partitions")
            .unwrap();
        partitions.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS"),
                text("P1"),
                number(1),
                number(120),
                number(4),
            ],
        }];
        partitions.status = OracleQueryStatus::Executed { rows: 1 };
        let partition_keys = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-part-key-columns")
            .unwrap();
        partition_keys.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS"),
                text("TABLE"),
                text("ID"),
                number(1),
            ],
        }];
        partition_keys.status = OracleQueryStatus::Executed { rows: 1 };

        let statistics = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tab-statistics")
            .unwrap();
        statistics.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS"),
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                text("2026-09-21T00:00:00Z"),
                text("NO"),
                OracleValue::Null,
                number(120),
                text("YES"),
                text("NO"),
                OracleValue::Null,
                text("SHARED"),
            ],
        }];
        statistics.status = OracleQueryStatus::Executed { rows: 1 };

        let mut second_owner = capture
            .outcomes
            .iter()
            .filter(|outcome| outcome.owner_ordinal.is_some())
            .cloned()
            .collect::<Vec<_>>();
        let denied = BTreeSet::from([
            "oracle-indexes",
            "oracle-index-columns",
            "oracle-index-expressions",
            "oracle-constraints",
            "oracle-constraint-columns",
            "oracle-part-tables",
            "oracle-tab-partitions",
            "oracle-tab-subpartitions",
            "oracle-part-key-columns",
            "oracle-tab-statistics",
        ]);
        for outcome in &mut second_owner {
            outcome.owner_ordinal = Some(2);
            for row in &mut outcome.rows {
                for value in &mut row.values {
                    if matches!(value, OracleValue::Text(text) if text == "APP") {
                        *value = text("OTHER");
                    }
                }
            }
            if denied.contains(outcome.query_id) {
                outcome.rows.clear();
                outcome.status = OracleQueryStatus::Failed {
                    class: OracleQueryFailure::PermissionDenied,
                };
            }
        }
        capture.outcomes.extend(second_owner);
        let scope = resolve_oracle_owner_scope(
            OracleOwnerSelection::Selected(vec!["APP".into(), "OTHER".into()]),
            "APP",
            vec!["APP".into(), "OTHER".into()],
        )
        .unwrap();

        let blueprint = map_oracle_basic_capture(
            &capture,
            &scope,
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();

        let readable = blueprint
            .tables
            .values()
            .find(|table| !table.idxs.is_empty())
            .expect("the readable owner must retain its index");
        assert_eq!(readable.check_count, Some(1));
        assert_eq!(readable.partitioning, "range");
        assert_eq!(readable.partition_count, Some(1));
        assert_eq!(
            readable.statistics.as_ref().unwrap().sample_fraction_band,
            "full"
        );
        assert!(!readable
            .table_limitations
            .contains(&"dependent-structure-suppressed".to_string()));

        let denied_owner = blueprint
            .tables
            .values()
            .find(|table| table.idxs.is_empty())
            .expect("the denied owner must remain in the floor inventory");
        assert_eq!(denied_owner.check_count, None);
        assert_eq!(denied_owner.partitioning, "unknown");
        assert_eq!(
            denied_owner
                .statistics
                .as_ref()
                .unwrap()
                .sample_fraction_band,
            "unknown"
        );
        assert!(denied_owner
            .table_limitations
            .contains(&"index-inventory-unavailable".to_string()));
        assert!(denied_owner
            .table_limitations
            .contains(&"relationship-inventory-unavailable".to_string()));
        assert!(!denied_owner
            .table_limitations
            .contains(&"dependent-structure-suppressed".to_string()));

        let structure = blueprint.structure_scope.as_ref().unwrap();
        assert_eq!(structure.index_inventory_completeness, "incomplete");
        assert_eq!(structure.relationship_inventory_completeness, "incomplete");
    }

    #[test]
    fn unreadable_owner_does_not_discard_an_owner_that_satisfies_the_floor() {
        let mut capture = fixture();
        let query_by_id = CATALOG_QUERIES
            .iter()
            .map(|query| (query.query_id, query))
            .collect::<BTreeMap<_, _>>();
        let mut second_owner = capture
            .outcomes
            .iter()
            .filter(|outcome| outcome.owner_ordinal.is_some())
            .cloned()
            .collect::<Vec<_>>();
        for outcome in &mut second_owner {
            outcome.owner_ordinal = Some(2);
            let query = query_by_id[outcome.query_id];
            let columns = query.output_columns(21);
            let owner_index = columns
                .iter()
                .position(|column| {
                    query
                        .owner_column
                        .is_some_and(|owner| column.eq_ignore_ascii_case(owner))
                })
                .unwrap();
            for row in &mut outcome.rows {
                row.values[owner_index] = text("OTHER");
            }
            if outcome.query_id == "oracle-tables" {
                outcome.status = OracleQueryStatus::Failed {
                    class: OracleQueryFailure::PermissionDenied,
                };
                outcome.rows.clear();
            }
        }
        capture.outcomes.extend(second_owner);
        let scope = resolve_oracle_owner_scope(
            OracleOwnerSelection::Selected(vec!["APP".into(), "OTHER".into()]),
            "APP",
            vec!["APP".into(), "OTHER".into()],
        )
        .unwrap();

        let blueprint = map_oracle_basic_capture(
            &capture,
            &scope,
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        assert_eq!(blueprint.tables.len(), 1);
        assert!(blueprint
            .dataset_scope
            .as_ref()
            .unwrap()
            .limitations
            .contains(&"catalog-capture-truncated".to_string()));
        let structure = blueprint.structure_scope.as_ref().unwrap();
        assert_eq!(structure.table_inventory_completeness, "incomplete");
        assert!(structure
            .limitations
            .contains(&"catalog-capture-truncated".to_string()));
    }

    #[test]
    fn unrelated_unrepresentable_visible_owner_emits_requested_floor_with_truncation_evidence() {
        let scope = resolve_oracle_owner_scope(
            OracleOwnerSelection::Default,
            "APP",
            ["APP".to_string(), "BAD\nOWNER".to_string()],
        )
        .unwrap();
        let blueprint = map_oracle_basic_capture(
            &fixture(),
            &scope,
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("an unrelated unrepresentable owner must not discard APP");
        assert_eq!(blueprint.tables.len(), 1);
        let dataset = blueprint.dataset_scope.as_ref().unwrap();
        assert_eq!(dataset.table_inventory_completeness, "incomplete");
        assert!(dataset
            .limitations
            .contains(&"catalog-capture-truncated".to_string()));
    }

    #[test]
    fn optional_index_failure_degrades_without_losing_the_table_inventory() {
        let mut capture = fixture();
        let indexes = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-indexes")
            .unwrap();
        indexes.status = OracleQueryStatus::Failed {
            class: OracleQueryFailure::PermissionDenied,
        };
        indexes.rows.clear();

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        assert_eq!(blueprint.tables.len(), 1);
        let structure = blueprint.structure_scope.as_ref().unwrap();
        assert_eq!(structure.index_inventory_completeness, "incomplete");
        assert!(structure
            .limitations
            .contains(&"index-inventory-unavailable".to_string()));
    }

    #[test]
    fn malformed_optional_index_row_degrades_instead_of_inventing_non_unique() {
        let mut capture = fixture();
        let indexes = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-indexes")
            .unwrap();
        indexes.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_NOTE_IX"),
                text("APP"),
                text("ORDERS"),
                text("NORMAL"),
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                text("NO"),
                text("VALID"),
                text("N"),
                text("NO"),
                text("VISIBLE"),
                text("NO"),
                text("YES"),
            ],
        }];
        indexes.status = OracleQueryStatus::Executed { rows: 1 };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("a malformed optional family must not discard the core inventory");
        assert!(blueprint.tables.values().next().unwrap().idxs.is_empty());
        let structure = blueprint.structure_scope.as_ref().unwrap();
        assert_eq!(structure.index_inventory_completeness, "incomplete");
        assert!(structure
            .catalogs_read
            .contains(&"oracle-indexes".to_string()));
        assert!(!structure
            .catalogs_unreadable
            .contains(&"oracle-indexes".to_string()));
    }

    #[test]
    fn missing_identity_catalog_makes_generation_evidence_unknown() {
        let mut capture = fixture();
        let identities = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-identity-columns")
            .unwrap();
        identities.status = OracleQueryStatus::Failed {
            class: OracleQueryFailure::PermissionDenied,
        };
        identities.rows.clear();

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        assert_eq!(
            blueprint
                .structure_scope
                .as_ref()
                .unwrap()
                .column_inventory_completeness,
            "complete"
        );
        assert!(blueprint
            .tables
            .values()
            .flat_map(|table| table.cols.values())
            .all(|column| column.has_default.is_none()));
    }

    #[test]
    fn malformed_object_classification_degrades_without_losing_floor_tables() {
        let mut capture = fixture();
        let objects = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-objects")
            .unwrap();
        objects.rows[0].values[5] = OracleValue::Null;

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        let structure = blueprint.structure_scope.as_ref().unwrap();
        assert_eq!(structure.table_inventory_completeness, "incomplete");
        assert!(structure
            .limitations
            .contains(&"table-kinds-not-inventoried".to_string()));
        assert_eq!(blueprint.tables.len(), 1);
    }

    #[test]
    fn missing_object_classification_degrades_without_losing_floor_tables() {
        let mut capture = fixture();
        let objects = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-objects")
            .unwrap();
        objects.rows.clear();
        objects.status = OracleQueryStatus::Executed { rows: 0 };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        let structure = blueprint.structure_scope.as_ref().unwrap();
        assert_eq!(structure.table_inventory_completeness, "incomplete");
        assert!(structure
            .limitations
            .contains(&"table-kinds-not-inventoried".to_string()));
        assert_eq!(blueprint.tables.len(), 1);
    }

    #[test]
    fn disagreeing_external_catalogs_emit_conservative_classification_and_gap() {
        let mut capture = fixture();
        let external = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-external-tables")
            .unwrap();
        external.rows = vec![OracleRow {
            values: vec![text("APP"), text("ORDERS")],
        }];
        external.status = OracleQueryStatus::Executed { rows: 1 };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("a concurrent classification snapshot must degrade, not refuse");
        let table = blueprint.tables.values().next().unwrap();
        assert_eq!(table.object_kind, "external-table");
        assert_eq!(table.counted_in_totals, Some(false));
        assert!(table
            .table_limitations
            .contains(&"table-classification-unavailable".to_string()));
        let structure = blueprint.structure_scope.as_ref().unwrap();
        assert_eq!(structure.table_inventory_completeness, "incomplete");
        assert!(structure
            .limitations
            .contains(&"table-kinds-not-inventoried".to_string()));
    }

    #[test]
    fn table_without_matching_columns_emits_size_evidence_and_column_gap() {
        let mut capture = fixture();
        let columns = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tab-columns")
            .unwrap();
        columns.rows.clear();
        columns.status = OracleQueryStatus::Executed { rows: 0 };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("a concurrent column snapshot must preserve usable table evidence");
        let table = blueprint.tables.values().next().unwrap();
        assert!(table.cols.is_empty());
        assert!(table.table_bytes > 0);
        assert!(table
            .table_limitations
            .contains(&"column-inventory-unavailable".to_string()));
        assert!(table
            .table_limitations
            .contains(&"dependent-structure-suppressed".to_string()));
        let structure = blueprint.structure_scope.as_ref().unwrap();
        assert_eq!(structure.column_inventory_completeness, "incomplete");
        assert!(structure
            .limitations
            .contains(&"column-inventory-unavailable".to_string()));
        assert!(structure
            .limitations
            .contains(&"dependent-structure-suppressed".to_string()));
    }

    #[test]
    fn index_on_unresolved_hidden_key_does_not_invent_a_column_gap() {
        let mut capture = fixture();
        let columns = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tab-columns")
            .unwrap();
        columns
            .rows
            .retain(|row| optional_text(row, 2).as_deref() != Some("NOTE"));
        columns.status = OracleQueryStatus::Executed {
            rows: columns.rows.len() as u64,
        };
        let indexes = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-indexes")
            .unwrap();
        indexes.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_NOTE_IX"),
                text("APP"),
                text("ORDERS"),
                text("NORMAL"),
                text("NONUNIQUE"),
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                text("NO"),
                text("VALID"),
                text("N"),
                text("NO"),
                text("VISIBLE"),
                text("NO"),
                text("YES"),
            ],
        }];
        indexes.status = OracleQueryStatus::Executed { rows: 1 };
        let index_columns = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-index-columns")
            .unwrap();
        index_columns.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_NOTE_IX"),
                text("ORDERS"),
                text("NOTE"),
                number(1),
                text("ASC"),
                text("APP"),
            ],
        }];
        index_columns.status = OracleQueryStatus::Executed { rows: 1 };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("one unstable ordinal must not discard the remaining floor");
        let table = blueprint.tables.values().next().unwrap();
        assert_eq!(table.cols.len(), 1);
        assert!(table.idxs.is_empty());
        assert!(!table
            .table_limitations
            .contains(&"column-inventory-unavailable".to_string()));
        assert!(!table
            .table_limitations
            .contains(&"dependent-structure-suppressed".to_string()));
        assert!(table
            .table_limitations
            .contains(&"index-inventory-unavailable".to_string()));
        let structure = blueprint.structure_scope.as_ref().unwrap();
        assert_eq!(structure.column_inventory_completeness, "complete");
        assert_eq!(structure.index_inventory_completeness, "incomplete");
        assert!(structure
            .limitations
            .contains(&"index-inventory-unavailable".to_string()));
    }

    #[test]
    fn duplicate_snapshot_ordinals_are_rebuilt_but_never_called_complete() {
        let mut capture = fixture();
        capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tab-columns")
            .unwrap()
            .rows[1]
            .values[3] = number(1);

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("duplicate snapshot ordinals must degrade, not refuse");
        let table = blueprint.tables.values().next().unwrap();
        assert_eq!(table.cols.len(), 2);
        assert_eq!(table.cols["col-1"].ordinal, 1);
        assert_eq!(table.cols["col-2"].ordinal, 2);
        let structure = blueprint.structure_scope.as_ref().unwrap();
        assert_eq!(structure.column_inventory_completeness, "incomplete");
        assert!(structure
            .limitations
            .contains(&"column-inventory-unavailable".to_string()));
    }

    #[test]
    fn malformed_non_table_classification_degrades_artifact_summary() {
        let mut capture = fixture();
        let objects = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-objects")
            .unwrap();
        objects.rows.push(OracleRow {
            values: vec![
                text("APP"),
                text("REPORTING_VIEW"),
                text("VIEW"),
                text("VALID"),
                text("N"),
                OracleValue::Null,
                text("N"),
                text("Y"),
            ],
        });
        objects.status = OracleQueryStatus::Executed { rows: 2 };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::Summary,
            },
        )
        .expect("a malformed non-table object must not discard the table census");
        let inventory = blueprint.artifact_inventory.unwrap();
        assert!(!inventory.inventory_complete);
        assert_eq!(inventory.visibility, "unknown");
        assert!(inventory
            .families_not_inventoried
            .contains(&"object_classification".to_string()));
        assert_eq!(inventory.counts_by_kind.get("view"), None);
    }

    #[test]
    fn expression_catalog_failure_suppresses_indexes_instead_of_claiming_false() {
        let mut capture = fixture();
        let indexes = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-indexes")
            .unwrap();
        indexes.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_NOTE_IX"),
                text("APP"),
                text("ORDERS"),
                text("FUNCTION-BASED NORMAL"),
                text("NONUNIQUE"),
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                text("NO"),
                text("VALID"),
                text("N"),
                text("NO"),
                text("VISIBLE"),
                text("NO"),
                text("YES"),
            ],
        }];
        indexes.status = OracleQueryStatus::Executed { rows: 1 };
        let index_columns = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-index-columns")
            .unwrap();
        index_columns.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_NOTE_IX"),
                text("ORDERS"),
                text("NOTE"),
                number(1),
                text("ASC"),
                text("APP"),
            ],
        }];
        index_columns.status = OracleQueryStatus::Executed { rows: 1 };
        let expressions = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-index-expressions")
            .unwrap();
        expressions.status = OracleQueryStatus::Failed {
            class: OracleQueryFailure::PermissionDenied,
        };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        assert!(blueprint.tables.values().next().unwrap().idxs.is_empty());
        assert_eq!(
            blueprint
                .structure_scope
                .as_ref()
                .unwrap()
                .index_inventory_completeness,
            "incomplete"
        );
    }

    #[test]
    fn denied_expression_catalog_does_not_withdraw_columns_for_descending_index() {
        let mut capture = fixture();
        let indexes = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-indexes")
            .unwrap();
        indexes.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_NOTE_DESC_IX"),
                text("APP"),
                text("ORDERS"),
                text("FUNCTION-BASED NORMAL"),
                text("NONUNIQUE"),
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                text("NO"),
                text("VALID"),
                text("N"),
                text("NO"),
                text("VISIBLE"),
                text("NO"),
                text("YES"),
            ],
        }];
        indexes.status = OracleQueryStatus::Executed { rows: 1 };
        let index_columns = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-index-columns")
            .unwrap();
        index_columns.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_NOTE_DESC_IX"),
                text("ORDERS"),
                text("SYS_NC00003$"),
                number(1),
                text("DESC"),
                text("APP"),
            ],
        }];
        index_columns.status = OracleQueryStatus::Executed { rows: 1 };
        let expressions = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-index-expressions")
            .unwrap();
        expressions.rows.clear();
        expressions.status = OracleQueryStatus::Failed {
            class: OracleQueryFailure::PermissionDenied,
        };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("a denied refinement catalog must not withdraw floor columns");

        let table = blueprint.tables.values().next().unwrap();
        assert_eq!(table.cols.len(), 2);
        assert!(table.idxs.is_empty());
        assert!(!table
            .table_limitations
            .contains(&"column-inventory-unavailable".to_string()));
        assert!(!table
            .table_limitations
            .contains(&"dependent-structure-suppressed".to_string()));
        assert!(table
            .table_limitations
            .contains(&"index-inventory-unavailable".to_string()));
        let structure = blueprint.structure_scope.as_ref().unwrap();
        assert_eq!(structure.column_inventory_completeness, "complete");
        assert_eq!(structure.index_inventory_completeness, "incomplete");
        assert_eq!(structure.relationship_inventory_completeness, "complete");
        assert!(structure
            .catalogs_read
            .contains(&"oracle-indexes".to_string()));
        assert!(!structure
            .catalogs_unreadable
            .contains(&"oracle-indexes".to_string()));
        dbwarp_blueprint_core::validate_blueprint_contract(&blueprint).unwrap();
    }

    #[test]
    fn expression_key_does_not_hide_a_separate_unresolved_index_column() {
        let mut capture = fixture();
        let indexes = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-indexes")
            .unwrap();
        indexes.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_MIXED_IX"),
                text("APP"),
                text("ORDERS"),
                text("FUNCTION-BASED NORMAL"),
                text("NONUNIQUE"),
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                text("NO"),
                text("VALID"),
                text("N"),
                text("NO"),
                text("VISIBLE"),
                text("NO"),
                text("YES"),
            ],
        }];
        indexes.status = OracleQueryStatus::Executed { rows: 1 };
        let index_columns = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-index-columns")
            .unwrap();
        index_columns.rows = vec![
            OracleRow {
                values: vec![
                    text("APP"),
                    text("ORDERS_MIXED_IX"),
                    text("ORDERS"),
                    text("SYS_NC00003$"),
                    number(1),
                    text("DESC"),
                    text("APP"),
                ],
            },
            OracleRow {
                values: vec![
                    text("APP"),
                    text("ORDERS_MIXED_IX"),
                    text("ORDERS"),
                    text("DROPPED_DURING_CAPTURE"),
                    number(2),
                    text("ASC"),
                    text("APP"),
                ],
            },
        ];
        index_columns.status = OracleQueryStatus::Executed { rows: 2 };
        let expressions = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-index-expressions")
            .unwrap();
        expressions.rows = vec![OracleRow {
            values: vec![text("APP"), text("ORDERS_MIXED_IX"), number(1), text("APP")],
        }];
        expressions.status = OracleQueryStatus::Executed { rows: 1 };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("one expression key cannot excuse a different missing key column");

        let table = blueprint.tables.values().next().unwrap();
        assert_eq!(table.cols.len(), 2);
        assert!(table.idxs.is_empty());
        assert!(table
            .table_limitations
            .contains(&"index-inventory-unavailable".to_string()));
        assert!(!table
            .table_limitations
            .contains(&"column-inventory-unavailable".to_string()));
        let structure = blueprint.structure_scope.as_ref().unwrap();
        assert_eq!(structure.column_inventory_completeness, "complete");
        assert_eq!(structure.index_inventory_completeness, "incomplete");
    }

    #[test]
    fn malformed_expression_position_degrades_only_its_native_table() {
        let mut capture = fixture();
        let query_by_id = CATALOG_QUERIES
            .iter()
            .map(|query| (query.query_id, query))
            .collect::<BTreeMap<_, _>>();
        let mut second_owner = capture
            .outcomes
            .iter()
            .filter(|outcome| outcome.owner_ordinal.is_some())
            .cloned()
            .collect::<Vec<_>>();
        for outcome in &mut second_owner {
            outcome.owner_ordinal = Some(2);
            let query = query_by_id[outcome.query_id];
            let columns = query.output_columns(21);
            let owner_index = columns
                .iter()
                .position(|column| {
                    query
                        .owner_column
                        .is_some_and(|owner| column.eq_ignore_ascii_case(owner))
                })
                .unwrap();
            for row in &mut outcome.rows {
                row.values[owner_index] = text("OTHER");
            }
            if outcome.query_id == "oracle-tables" {
                outcome.rows[0].values[3] = number(900);
            }
        }
        capture.outcomes.extend(second_owner);

        let indexes = capture
            .outcomes
            .iter_mut()
            .find(|outcome| {
                outcome.query_id == "oracle-indexes" && outcome.owner_ordinal == Some(1)
            })
            .unwrap();
        indexes.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_EXPR_IX"),
                text("APP"),
                text("ORDERS"),
                text("FUNCTION-BASED NORMAL"),
                text("NONUNIQUE"),
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                text("NO"),
                text("VALID"),
                text("N"),
                text("NO"),
                text("VISIBLE"),
                text("NO"),
                text("YES"),
            ],
        }];
        indexes.status = OracleQueryStatus::Executed { rows: 1 };
        let index_columns = capture
            .outcomes
            .iter_mut()
            .find(|outcome| {
                outcome.query_id == "oracle-index-columns" && outcome.owner_ordinal == Some(1)
            })
            .unwrap();
        index_columns.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_EXPR_IX"),
                text("ORDERS"),
                text("ID"),
                number(1),
                text("ASC"),
                text("APP"),
            ],
        }];
        index_columns.status = OracleQueryStatus::Executed { rows: 1 };
        let expressions = capture
            .outcomes
            .iter_mut()
            .find(|outcome| {
                outcome.query_id == "oracle-index-expressions" && outcome.owner_ordinal == Some(1)
            })
            .unwrap();
        expressions.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_EXPR_IX"),
                OracleValue::Null,
                text("APP"),
            ],
        }];
        expressions.status = OracleQueryStatus::Executed { rows: 1 };

        let scope = resolve_oracle_owner_scope(
            OracleOwnerSelection::Selected(vec!["APP".into(), "OTHER".into()]),
            "APP",
            vec!["APP".into(), "OTHER".into()],
        )
        .unwrap();
        let blueprint = map_oracle_basic_capture(
            &capture,
            &scope,
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("one malformed expression key cannot erase another table's index evidence");

        let affected = blueprint
            .tables
            .values()
            .find(|table| table.rows == 100)
            .expect("APP fixture table");
        let unaffected = blueprint
            .tables
            .values()
            .find(|table| table.rows != 100)
            .expect("OTHER fixture table");
        assert!(affected.idxs.is_empty());
        assert!(affected
            .table_limitations
            .contains(&"index-inventory-unavailable".to_string()));
        assert!(!unaffected
            .table_limitations
            .contains(&"index-inventory-unavailable".to_string()));
        let structure = blueprint.structure_scope.as_ref().unwrap();
        assert_eq!(structure.index_inventory_completeness, "incomplete");
        assert!(structure
            .catalogs_read
            .contains(&"oracle-indexes".to_string()));
        assert!(!structure
            .catalogs_unreadable
            .contains(&"oracle-indexes".to_string()));
    }

    #[test]
    fn hidden_support_index_does_not_degrade_customer_index_inventory() {
        let mut capture = fixture();
        let tables = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tables")
            .unwrap();
        let mut support_table = tables.rows[0].clone();
        support_table.values[1] = text("ORDERS_TAGS_NT");
        support_table.values[10] = text("YES");
        support_table.values[12] = text("ORDERS");
        tables.rows.push(support_table);
        tables.status = OracleQueryStatus::Executed {
            rows: tables.rows.len() as u64,
        };
        let objects = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-objects")
            .unwrap();
        let mut support_object = objects.rows[0].clone();
        support_object.values[1] = text("ORDERS_TAGS_NT");
        // A malformed classification flag on filtered Oracle support storage
        // cannot become a customer-table aggregate gap.
        support_object.values[5] = OracleValue::Null;
        objects.rows.push(support_object);
        objects.status = OracleQueryStatus::Executed {
            rows: objects.rows.len() as u64,
        };
        let indexes = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-indexes")
            .unwrap();
        indexes.rows.push(OracleRow {
            values: vec![
                text("APP"),
                text("SYS_NESTED_SUPPORT_IX"),
                text("APP"),
                text("ORDERS_TAGS_NT"),
                text("NORMAL"),
                text("NONUNIQUE"),
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                text("NO"),
                text("VALID"),
                text("N"),
                text("NO"),
                text("VISIBLE"),
                text("NO"),
                text("YES"),
            ],
        });
        indexes.status = OracleQueryStatus::Executed { rows: 1 };
        let index_columns = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-index-columns")
            .unwrap();
        index_columns.rows.push(OracleRow {
            values: vec![
                text("APP"),
                text("SYS_NESTED_SUPPORT_IX"),
                text("ORDERS_TAGS_NT"),
                text("NESTED_TABLE_ID"),
                number(1),
                text("ASC"),
                text("APP"),
            ],
        });
        index_columns.status = OracleQueryStatus::Executed { rows: 1 };
        let expressions = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-index-expressions")
            .unwrap();
        expressions.rows.push(OracleRow {
            values: vec![
                text("APP"),
                text("SYS_NESTED_SUPPORT_IX"),
                OracleValue::Null,
                text("APP"),
            ],
        });
        expressions.status = OracleQueryStatus::Executed { rows: 1 };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("an excluded nested-table index cannot degrade customer inventory");

        assert_eq!(blueprint.tables.len(), 1);
        let structure = blueprint.structure_scope.as_ref().unwrap();
        assert_eq!(structure.table_inventory_completeness, "complete");
        assert_eq!(structure.column_inventory_completeness, "complete");
        assert_eq!(structure.index_inventory_completeness, "complete");
    }

    #[test]
    fn malformed_support_index_expressions_on_customer_table_do_not_degrade_inventory() {
        let mut capture = fixture();
        let indexes = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-indexes")
            .unwrap();
        let support_index = |name: &str, generated: &str, secondary: &str| OracleRow {
            values: vec![
                text("APP"),
                text(name),
                text("APP"),
                text("ORDERS"),
                text("NORMAL"),
                text("NONUNIQUE"),
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                text("NO"),
                text("VALID"),
                text(generated),
                text(secondary),
                text("VISIBLE"),
                text("NO"),
                text("YES"),
            ],
        };
        indexes.rows = vec![
            support_index("SYS_GENERATED_SUPPORT_IX", "Y", "NO"),
            support_index("DOMAIN_SECONDARY_IX", "N", "YES"),
        ];
        indexes.status = OracleQueryStatus::Executed { rows: 2 };

        let index_columns = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-index-columns")
            .unwrap();
        index_columns.rows = ["SYS_GENERATED_SUPPORT_IX", "DOMAIN_SECONDARY_IX"]
            .into_iter()
            .map(|name| OracleRow {
                values: vec![
                    text("APP"),
                    text(name),
                    text("ORDERS"),
                    text("SYS_NC_SUPPORT$"),
                    number(1),
                    text("ASC"),
                    text("APP"),
                ],
            })
            .collect();
        index_columns.status = OracleQueryStatus::Executed { rows: 2 };

        let expressions = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-index-expressions")
            .unwrap();
        expressions.rows = ["SYS_GENERATED_SUPPORT_IX", "DOMAIN_SECONDARY_IX"]
            .into_iter()
            .map(|name| OracleRow {
                values: vec![text("APP"), text(name), OracleValue::Null, text("APP")],
            })
            .collect();
        expressions.status = OracleQueryStatus::Executed { rows: 2 };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("support-index defects cannot withdraw customer index completeness");

        let table = blueprint.tables.values().next().unwrap();
        assert!(table.idxs.is_empty());
        assert!(!table
            .table_limitations
            .contains(&"index-inventory-unavailable".to_string()));
        assert_eq!(
            blueprint
                .structure_scope
                .as_ref()
                .unwrap()
                .index_inventory_completeness,
            "complete"
        );
    }

    #[test]
    fn missing_index_key_rows_degrade_index_inventory() {
        let mut capture = fixture();
        let indexes = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-indexes")
            .unwrap();
        indexes.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_NOTE_IX"),
                text("APP"),
                text("ORDERS"),
                text("NORMAL"),
                text("NONUNIQUE"),
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                text("NO"),
                text("VALID"),
                text("N"),
                text("NO"),
                text("VISIBLE"),
                text("NO"),
                text("YES"),
            ],
        }];
        indexes.status = OracleQueryStatus::Executed { rows: 1 };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        assert!(blueprint.tables.values().next().unwrap().idxs.is_empty());
        assert_eq!(
            blueprint
                .structure_scope
                .as_ref()
                .unwrap()
                .index_inventory_completeness,
            "incomplete"
        );
    }

    #[test]
    fn iot_top_index_remains_the_logical_primary_key() {
        let mut capture = fixture();
        let tables = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tables")
            .unwrap();
        tables.rows[0].values[9] = text("IOT");
        let indexes = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-indexes")
            .unwrap();
        indexes.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_PK"),
                text("APP"),
                text("ORDERS"),
                text("IOT - TOP"),
                text("UNIQUE"),
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                text("NO"),
                text("VALID"),
                text("N"),
                text("NO"),
                text("VISIBLE"),
                text("NO"),
                text("YES"),
            ],
        }];
        indexes.status = OracleQueryStatus::Executed { rows: 1 };
        let index_columns = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-index-columns")
            .unwrap();
        index_columns.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_PK"),
                text("ORDERS"),
                text("ID"),
                number(1),
                text("ASC"),
                text("APP"),
            ],
        }];
        index_columns.status = OracleQueryStatus::Executed { rows: 1 };
        let constraints = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-constraints")
            .unwrap();
        constraints.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_PK"),
                text("P"),
                text("ORDERS"),
                OracleValue::Null,
                OracleValue::Null,
                text("NO ACTION"),
                text("ENABLED"),
                text("NOT DEFERRABLE"),
                text("VALIDATED"),
                text("IMMEDIATE"),
                text("APP"),
                text("ORDERS_PK"),
                text("USER NAME"),
                text("N"),
            ],
        }];
        constraints.status = OracleQueryStatus::Executed { rows: 1 };
        let constraint_columns = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-constraint-columns")
            .unwrap();
        constraint_columns.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_PK"),
                text("ORDERS"),
                text("ID"),
                number(1),
            ],
        }];
        constraint_columns.status = OracleQueryStatus::Executed { rows: 1 };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        let index = blueprint
            .tables
            .values()
            .next()
            .unwrap()
            .idxs
            .values()
            .next()
            .unwrap();
        assert!(index.primary);
        assert!(index.unique);
        assert_eq!(index.index_type, "btree");
    }

    #[test]
    fn bitmap_join_index_is_preserved_without_false_local_column_ordinals() {
        let mut capture = fixture();
        let indexes = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-indexes")
            .unwrap();
        indexes.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_CUSTOMER_BJI"),
                text("APP"),
                text("ORDERS"),
                text("BITMAP"),
                text("NONUNIQUE"),
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                text("NO"),
                text("VALID"),
                text("N"),
                text("NO"),
                text("VISIBLE"),
                text("YES"),
                text("YES"),
            ],
        }];
        indexes.status = OracleQueryStatus::Executed { rows: 1 };
        let columns = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-index-columns")
            .unwrap();
        columns.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_CUSTOMER_BJI"),
                text("ORDERS"),
                text("REMOTE_CUSTOMER_ID"),
                number(1),
                text("ASC"),
                text("APP"),
            ],
        }];
        columns.status = OracleQueryStatus::Executed { rows: 1 };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        let index = blueprint
            .tables
            .values()
            .next()
            .unwrap()
            .idxs
            .values()
            .next()
            .unwrap();
        assert_eq!(index.index_type, "bitmap");
        assert!(index.expression);
        assert!(index.cols.is_empty());
    }

    #[test]
    fn multi_level_nested_segments_are_attributed_to_the_root_table() {
        let mut capture = fixture();
        let nested = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-nested-tables")
            .unwrap();
        nested.rows = vec![
            OracleRow {
                values: vec![
                    text("APP"),
                    text("OUTER_STORE"),
                    text("ORDERS"),
                    text("OUTER_ITEMS"),
                ],
            },
            OracleRow {
                values: vec![
                    text("APP"),
                    text("INNER_STORE"),
                    text("OUTER_STORE"),
                    text("INNER_ITEMS"),
                ],
            },
        ];
        nested.status = OracleQueryStatus::Executed { rows: 2 };
        let segments = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-segments")
            .unwrap();
        segments.rows = vec![
            OracleRow {
                values: vec![
                    text("APP"),
                    text("OUTER_STORE"),
                    text("NESTED TABLE"),
                    text("USERS"),
                    number(4096),
                    number(1),
                ],
            },
            OracleRow {
                values: vec![
                    text("APP"),
                    text("INNER_STORE"),
                    text("NESTED TABLE"),
                    text("USERS"),
                    number(8192),
                    number(1),
                ],
            },
        ];
        segments.status = OracleQueryStatus::Executed { rows: 2 };

        let parents = collect_nested_table_parents(&capture, 1).unwrap();
        let tables = collect_tables(&capture).unwrap();
        let lookup = build_segment_attribution_lookup(&tables, &[]);
        let collected = collect_segment_bytes(
            &capture,
            &BTreeMap::new(),
            &parents,
            &lookup,
            &tables,
            &[],
            1,
            "APP",
        )
        .unwrap();

        assert_eq!(
            collected
                .table_bytes
                .get(&("APP".to_string(), "ORDERS".to_string())),
            Some(&12_288)
        );
        assert!(collected.index_bytes.is_empty());
        assert!(!collected.unmapped_nested_segments);
        assert!(collected
            .incomplete_tables
            .contains(&("APP".to_string(), "ORDERS".to_string())));

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("nested storage must not mask a missing parent table segment");
        let table = blueprint.tables.values().next().unwrap();
        assert_eq!(table.table_bytes, format::round_bytes(12_288));
        assert_eq!(
            table.statistics.as_ref().unwrap().size_visibility,
            "partial"
        );
        assert_eq!(
            blueprint.dataset_scope.as_ref().unwrap().size_completeness,
            "incomplete"
        );
    }

    #[test]
    fn foreign_key_without_key_rows_degrades_relationship_inventory() {
        let mut capture = fixture();
        let constraints = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-constraints")
            .unwrap();
        let foreign_key = OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_PARENT_FK"),
                text("R"),
                text("ORDERS"),
                text("APP"),
                text("ORDERS_PK"),
                text("NO ACTION"),
                text("ENABLED"),
                text("NOT DEFERRABLE"),
                text("VALIDATED"),
                text("IMMEDIATE"),
                OracleValue::Null,
                OracleValue::Null,
                text("USER NAME"),
                text("N"),
            ],
        };
        let mut primary_key = foreign_key.clone();
        primary_key.values[1] = text("ORDERS_PK");
        primary_key.values[2] = text("P");
        primary_key.values[4] = OracleValue::Null;
        primary_key.values[5] = OracleValue::Null;
        constraints.rows = vec![foreign_key, primary_key];
        constraints.status = OracleQueryStatus::Executed { rows: 2 };
        let columns = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-constraint-columns")
            .unwrap();
        columns.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_PK"),
                text("ORDERS"),
                text("ID"),
                number(1),
            ],
        }];
        columns.status = OracleQueryStatus::Executed { rows: 1 };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        let structure = blueprint.structure_scope.as_ref().unwrap();
        assert_eq!(structure.relationship_inventory_completeness, "incomplete");
        assert!(structure
            .catalogs_read
            .contains(&"oracle-constraints".to_string()));
        assert!(!structure
            .catalogs_unreadable
            .contains(&"oracle-constraints".to_string()));
        assert!(blueprint
            .tables
            .values()
            .next()
            .unwrap()
            .table_limitations
            .contains(&"relationship-inventory-unavailable".to_string()));
    }

    #[test]
    fn unresolved_parent_key_is_attributed_to_the_child_relationship() {
        let mut capture = fixture();
        let tables = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tables")
            .unwrap();
        let mut child_table = tables.rows[0].clone();
        child_table.values[1] = text("CHILD");
        child_table.values[3] = number(900);
        tables.rows.push(child_table);
        tables.status = OracleQueryStatus::Executed { rows: 2 };

        let objects = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-objects")
            .unwrap();
        let mut child_object = objects.rows[0].clone();
        child_object.values[1] = text("CHILD");
        objects.rows.push(child_object);
        objects.status = OracleQueryStatus::Executed { rows: 2 };

        for query_id in ["oracle-tab-columns", "oracle-tab-cols-hidden"] {
            let outcome = capture
                .outcomes
                .iter_mut()
                .find(|outcome| outcome.query_id == query_id)
                .unwrap();
            let child_rows = outcome
                .rows
                .iter()
                .cloned()
                .map(|mut row| {
                    row.values[1] = text("CHILD");
                    row
                })
                .collect::<Vec<_>>();
            outcome.rows.extend(child_rows);
            outcome.status = OracleQueryStatus::Executed {
                rows: outcome.rows.len() as u64,
            };
        }

        let segments = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-segments")
            .unwrap();
        let mut child_segment = segments.rows[0].clone();
        child_segment.values[1] = text("CHILD");
        segments.rows.push(child_segment);
        segments.status = OracleQueryStatus::Executed { rows: 2 };

        let constraints = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-constraints")
            .unwrap();
        let constraint_row =
            |name: &str, kind: &str, table: &str, referenced: Option<&str>| OracleRow {
                values: vec![
                    text("APP"),
                    text(name),
                    text(kind),
                    text(table),
                    referenced.map(|_| text("APP")).unwrap_or(OracleValue::Null),
                    referenced.map(text).unwrap_or(OracleValue::Null),
                    text("NO ACTION"),
                    text("ENABLED"),
                    text("NOT DEFERRABLE"),
                    text("VALIDATED"),
                    text("IMMEDIATE"),
                    OracleValue::Null,
                    OracleValue::Null,
                    text("USER NAME"),
                    text("N"),
                ],
            };
        constraints.rows = vec![
            constraint_row("ORDERS_PK", "P", "ORDERS", None),
            constraint_row("CHILD_FK", "R", "CHILD", Some("ORDERS_PK")),
        ];
        constraints.status = OracleQueryStatus::Executed { rows: 2 };

        let columns = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-constraint-columns")
            .unwrap();
        columns.rows = vec![
            OracleRow {
                values: vec![
                    text("APP"),
                    text("ORDERS_PK"),
                    text("ORDERS"),
                    text("MISSING_PARENT_KEY"),
                    number(1),
                ],
            },
            OracleRow {
                values: vec![
                    text("APP"),
                    text("CHILD_FK"),
                    text("CHILD"),
                    text("ID"),
                    number(1),
                ],
            },
        ];
        columns.status = OracleQueryStatus::Executed { rows: 2 };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("an unrepresentable parent key must degrade rather than erase the floor");

        let parent = blueprint
            .tables
            .values()
            .find(|table| table.rows == 100)
            .expect("parent table");
        let child = blueprint
            .tables
            .values()
            .find(|table| table.rows != 100)
            .expect("child table");
        assert!(!parent
            .table_limitations
            .contains(&"relationship-inventory-unavailable".to_string()));
        assert!(child
            .table_limitations
            .contains(&"relationship-inventory-unavailable".to_string()));
        assert!(blueprint.fk_edges.is_empty());
        assert_eq!(
            blueprint
                .structure_scope
                .as_ref()
                .unwrap()
                .relationship_inventory_completeness,
            "incomplete"
        );
    }

    #[test]
    fn unreferenced_generated_object_key_does_not_degrade_relationship_inventory() {
        let mut capture = fixture();
        let base_table = capture
            .outcomes
            .iter()
            .find(|outcome| outcome.query_id == "oracle-tables")
            .unwrap()
            .rows[0]
            .clone();
        let object_tables = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-object-tables")
            .unwrap();
        object_tables.rows = vec![base_table];
        object_tables.status = OracleQueryStatus::Executed { rows: 1 };

        let constraints = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-constraints")
            .unwrap();
        constraints.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("SYS_C_OID"),
                text("P"),
                text("ORDERS"),
                OracleValue::Null,
                OracleValue::Null,
                text("NO ACTION"),
                text("ENABLED"),
                text("NOT DEFERRABLE"),
                text("VALIDATED"),
                text("IMMEDIATE"),
                OracleValue::Null,
                OracleValue::Null,
                text("GENERATED NAME"),
                text("N"),
            ],
        }];
        constraints.status = OracleQueryStatus::Executed { rows: 1 };
        let columns = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-constraint-columns")
            .unwrap();
        columns.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("SYS_C_OID"),
                text("ORDERS"),
                text("SYS_NC_OID$"),
                number(1),
            ],
        }];
        columns.status = OracleQueryStatus::Executed { rows: 1 };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("Oracle's hidden object identity is not a customer relationship gap");

        let table = blueprint.tables.values().next().unwrap();
        assert_eq!(table.object_kind, "object-table");
        assert!(!table
            .table_limitations
            .contains(&"relationship-inventory-unavailable".to_string()));
        assert_eq!(
            blueprint
                .structure_scope
                .as_ref()
                .unwrap()
                .relationship_inventory_completeness,
            "complete"
        );
    }

    #[test]
    fn foreign_key_to_unselected_owner_is_recorded_on_only_the_child_table() {
        let mut capture = fixture();
        let constraints = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-constraints")
            .unwrap();
        constraints.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_REF_FK"),
                text("R"),
                text("ORDERS"),
                text("REF"),
                text("REF_PK"),
                text("NO ACTION"),
                text("ENABLED"),
                text("NOT DEFERRABLE"),
                text("VALIDATED"),
                text("IMMEDIATE"),
                OracleValue::Null,
                OracleValue::Null,
                text("USER NAME"),
                text("N"),
            ],
        }];
        constraints.status = OracleQueryStatus::Executed { rows: 1 };
        let columns = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-constraint-columns")
            .unwrap();
        columns.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_REF_FK"),
                text("ORDERS"),
                text("ID"),
                number(1),
            ],
        }];
        columns.status = OracleQueryStatus::Executed { rows: 1 };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        let structure = blueprint.structure_scope.as_ref().unwrap();
        assert_eq!(structure.relationship_inventory_completeness, "complete");
        assert_eq!(structure.index_inventory_completeness, "complete");
        let child = blueprint.tables.values().next().unwrap();
        assert_eq!(child.check_count, Some(0));
        assert_eq!(
            child.table_limitations,
            vec!["relationship-target-outside-selected-scope"]
        );
        assert!(blueprint.fk_edges.is_empty());
    }

    #[test]
    fn foreign_key_to_unseen_owner_in_all_visible_mode_degrades_without_aborting() {
        let mut capture = fixture();
        let constraints = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-constraints")
            .unwrap();
        constraints.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_REF_FK"),
                text("R"),
                text("ORDERS"),
                text("REF"),
                text("REF_PK"),
                text("NO ACTION"),
                text("ENABLED"),
                text("NOT DEFERRABLE"),
                text("VALIDATED"),
                text("IMMEDIATE"),
                OracleValue::Null,
                OracleValue::Null,
                text("USER NAME"),
                text("N"),
            ],
        }];
        constraints.status = OracleQueryStatus::Executed { rows: 1 };
        let columns = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-constraint-columns")
            .unwrap();
        columns.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_REF_FK"),
                text("ORDERS"),
                text("ID"),
                number(1),
            ],
        }];
        columns.status = OracleQueryStatus::Executed { rows: 1 };
        let scope = resolve_oracle_owner_scope(
            OracleOwnerSelection::AllVisible { acknowledged: true },
            "APP",
            ["APP".to_string()],
        )
        .unwrap();

        let blueprint = map_oracle_basic_capture(
            &capture,
            &scope,
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        let structure = blueprint.structure_scope.as_ref().unwrap();
        assert_eq!(structure.relationship_inventory_completeness, "incomplete");
        assert!(structure
            .catalogs_read
            .contains(&"oracle-constraints".to_string()));
        assert!(!structure
            .catalogs_unreadable
            .contains(&"oracle-constraints".to_string()));
        assert_eq!(
            blueprint.tables.values().next().unwrap().table_limitations,
            vec!["relationship-target-visibility-unknown"]
        );
        assert!(blueprint.fk_edges.is_empty());
    }

    #[test]
    fn support_catalogs_identify_exact_log_and_queue_storage() {
        let mut capture = fixture();
        let queues = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-queue-tables")
            .unwrap();
        queues.rows = vec![OracleRow {
            values: vec![text("APP"), text("WORK_QUEUE")],
        }];
        queues.status = OracleQueryStatus::Executed { rows: 1 };
        let logs = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-mview-logs")
            .unwrap();
        logs.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("MLOG$_ORDERS"),
                text("ORDERS"),
                text("YES"),
            ],
        }];
        logs.status = OracleQueryStatus::Executed { rows: 1 };

        let queue_tables = collect_support_table_keys(&capture, "oracle-queue-tables", true, 1)
            .expect("queue support catalog");
        assert!(queue_tables.contains(&("APP".into(), "AQ$_WORK_QUEUE_R".into())));
        assert!(queue_tables.contains(&("APP".into(), "AQ$_WORK_QUEUE_G".into())));
        let log_tables = collect_support_table_keys(&capture, "oracle-mview-logs", false, 1)
            .expect("materialized-view log catalog");
        assert!(log_tables.contains(&("APP".into(), "MLOG$_ORDERS".into())));
        // PRIMARY_KEY does not prove a temporary updatable-snapshot log
        // exists. Modern releases ordinarily create no RUPD$_ table here;
        // deriving one would hide a legitimate customer table with that name.
        assert!(!log_tables.contains(&("APP".into(), "RUPD$_ORDERS".into())));
        assert!(collect_mview_update_candidates(&capture, 1)
            .expect("materialized-view update candidates")
            .contains(&("APP".into(), "RUPD$_ORDERS".into())));
    }

    #[test]
    fn unreadable_oracle_maintained_owner_catalog_withdraws_inventory_claims() {
        let mut capture = fixture();
        let users = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-users")
            .unwrap();
        users.status = OracleQueryStatus::Failed {
            class: OracleQueryFailure::PermissionDenied,
        };
        users.rows.clear();

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        let dataset = blueprint.dataset_scope.as_ref().unwrap();
        assert_eq!(dataset.table_inventory_completeness, "incomplete");
        assert!(dataset
            .limitations
            .contains(&"table-inventory-visibility-unknown".to_string()));
        let structure = blueprint.structure_scope.as_ref().unwrap();
        assert_eq!(structure.visibility, "unknown");
        assert_eq!(structure.table_inventory_completeness, "incomplete");
    }

    #[test]
    fn owner_catalog_without_maintained_flag_is_not_complete_evidence() {
        let mut capture = fixture();
        let users = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-users")
            .unwrap();
        users.rows[0].values.truncate(1);

        let (maintained, complete) = collect_oracle_maintained_owners(&capture, 1);
        assert!(maintained.is_empty());
        assert!(!complete);
    }

    #[test]
    fn object_attribute_constraint_does_not_erase_valid_foreign_keys() {
        let mut capture = fixture();
        let constraints = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-constraints")
            .unwrap();
        constraints.rows = vec![
            OracleRow {
                values: vec![
                    text("APP"),
                    text("ORDERS_PK"),
                    text("P"),
                    text("ORDERS"),
                    OracleValue::Null,
                    OracleValue::Null,
                    text("NO ACTION"),
                    text("ENABLED"),
                    text("NOT DEFERRABLE"),
                    text("VALIDATED"),
                    text("IMMEDIATE"),
                    text("APP"),
                    text("ORDERS_PK_IX"),
                    text("USER NAME"),
                    text("N"),
                ],
            },
            OracleRow {
                values: vec![
                    text("APP"),
                    text("ORDERS_SELF_FK"),
                    text("R"),
                    text("ORDERS"),
                    text("APP"),
                    text("ORDERS_PK"),
                    text("NO ACTION"),
                    text("ENABLED"),
                    text("NOT DEFERRABLE"),
                    text("VALIDATED"),
                    text("IMMEDIATE"),
                    OracleValue::Null,
                    OracleValue::Null,
                    text("USER NAME"),
                    text("N"),
                ],
            },
            OracleRow {
                values: vec![
                    text("APP"),
                    text("ORDERS_OBJECT_FK"),
                    text("R"),
                    text("ORDERS"),
                    text("APP"),
                    text("ORDERS_PK"),
                    text("NO ACTION"),
                    text("ENABLED"),
                    text("NOT DEFERRABLE"),
                    text("VALIDATED"),
                    text("IMMEDIATE"),
                    OracleValue::Null,
                    OracleValue::Null,
                    text("USER NAME"),
                    text("N"),
                ],
            },
        ];
        constraints.status = OracleQueryStatus::Executed { rows: 3 };
        let columns = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-constraint-columns")
            .unwrap();
        columns.rows = vec![
            OracleRow {
                values: vec![
                    text("APP"),
                    text("ORDERS_PK"),
                    text("ORDERS"),
                    text("ID"),
                    number(1),
                ],
            },
            OracleRow {
                values: vec![
                    text("APP"),
                    text("ORDERS_SELF_FK"),
                    text("ORDERS"),
                    text("ID"),
                    number(1),
                ],
            },
            OracleRow {
                values: vec![
                    text("APP"),
                    text("ORDERS_OBJECT_FK"),
                    text("ORDERS"),
                    text("ADDRESS.POSTCODE"),
                    number(1),
                ],
            },
        ];
        columns.status = OracleQueryStatus::Executed { rows: 3 };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();

        assert_eq!(
            blueprint
                .structure_scope
                .as_ref()
                .unwrap()
                .relationship_inventory_completeness,
            "incomplete"
        );
        assert_eq!(blueprint.fk_edges.values().flatten().count(), 1);
    }

    #[test]
    fn incomplete_partition_catalog_emits_unknown_not_partial_key_evidence() {
        let mut capture = fixture();
        capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tables")
            .unwrap()
            .rows[0]
            .values[7] = text("YES");
        let part_tables = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-part-tables")
            .unwrap();
        part_tables.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS"),
                text("RANGE"),
                text("NONE"),
                number(2),
                OracleValue::Null,
            ],
        }];
        part_tables.status = OracleQueryStatus::Executed { rows: 1 };
        let partitions = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tab-partitions")
            .unwrap();
        partitions.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS"),
                text("P1"),
                number(1),
                number(120),
                number(4),
            ],
        }];
        partitions.status = OracleQueryStatus::Executed { rows: 1 };
        let keys = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-part-key-columns")
            .unwrap();
        keys.status = OracleQueryStatus::Failed {
            class: OracleQueryFailure::PermissionDenied,
        };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        let table = blueprint.tables.values().next().unwrap();
        assert_eq!(table.partitioning, "unknown");
        assert_eq!(table.partition_count, None);
        assert!(table.partition_key_cols.is_empty());
    }

    #[test]
    fn zero_partition_snapshot_emits_unknown_partition_evidence_without_refusing_floor() {
        let mut capture = fixture();
        capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tables")
            .unwrap()
            .rows[0]
            .values[7] = text("YES");
        let part_tables = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-part-tables")
            .unwrap();
        part_tables.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS"),
                text("SYSTEM"),
                text("NONE"),
                number(0),
                OracleValue::Null,
            ],
        }];
        part_tables.status = OracleQueryStatus::Executed { rows: 1 };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("a zero-count concurrent snapshot must degrade, not refuse");
        let table = blueprint.tables.values().next().unwrap();
        assert_eq!(table.partitioning, "unknown");
        assert_eq!(table.partition_count, None);
        assert!(table.partition_key_cols.is_empty());
    }

    #[test]
    fn missing_object_table_family_downgrades_every_dependent_inventory() {
        let mut capture = fixture();
        let objects = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-objects")
            .unwrap();
        let mut object_table = objects.rows[0].clone();
        object_table.values[1] = text("OBJECT_PAYLOAD");
        objects.rows.push(object_table);
        objects.status = OracleQueryStatus::Executed { rows: 2 };
        let object_tables = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-object-tables")
            .unwrap();
        object_tables.status = OracleQueryStatus::Failed {
            class: OracleQueryFailure::PermissionDenied,
        };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        let structure = blueprint.structure_scope.as_ref().unwrap();
        assert_eq!(structure.table_inventory_completeness, "incomplete");
        assert_eq!(structure.column_inventory_completeness, "incomplete");
        assert_eq!(structure.index_inventory_completeness, "incomplete");
        assert_eq!(structure.relationship_inventory_completeness, "incomplete");
        let dataset = blueprint.dataset_scope.as_ref().unwrap();
        assert_eq!(dataset.row_count_completeness, "incomplete");
        assert_eq!(dataset.size_completeness, "incomplete");
    }

    #[test]
    fn external_table_is_excluded_without_downgrading_copy_totals() {
        let mut capture = fixture();
        capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tables")
            .unwrap()
            .rows[0]
            .values[3] = OracleValue::Null;
        let table = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tables")
            .unwrap();
        table.rows[0].values[14] = text("YES");
        let external = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-external-tables")
            .unwrap();
        external.rows = vec![OracleRow {
            values: vec![text("APP"), text("ORDERS")],
        }];
        external.status = OracleQueryStatus::Executed { rows: 1 };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        let table = blueprint.tables.values().next().unwrap();
        assert_eq!(table.object_kind, "external-table");
        assert_eq!(table.counted_in_totals, Some(false));
        assert_eq!(table.rows, 0);
        assert_eq!(
            table.statistics.as_ref().unwrap().row_count_quality,
            "unavailable"
        );
        assert_eq!(blueprint.totals.row_count, 0);
        assert_eq!(
            blueprint
                .dataset_scope
                .as_ref()
                .unwrap()
                .row_count_completeness,
            "complete"
        );
    }

    #[test]
    fn unavailable_external_catalog_uses_the_12_2_table_flag_without_degrading_floor() {
        let mut capture = fixture();
        let external = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-external-tables")
            .unwrap();
        external.status = OracleQueryStatus::Failed {
            class: OracleQueryFailure::PermissionDenied,
        };
        external.rows.clear();

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        assert_eq!(blueprint.tables.len(), 1);
        let structure = blueprint.structure_scope.as_ref().unwrap();
        assert_eq!(structure.table_inventory_completeness, "complete");
        assert_eq!(
            blueprint
                .dataset_scope
                .as_ref()
                .unwrap()
                .row_count_completeness,
            "complete"
        );
        assert_eq!(
            blueprint.dataset_scope.as_ref().unwrap().size_completeness,
            "complete"
        );
        assert!(!structure
            .limitations
            .contains(&"table-kinds-not-inventoried".to_string()));
    }

    #[test]
    fn unavailable_hidden_column_catalog_keeps_visible_column_inventory_complete() {
        let mut capture = fixture();
        let hidden = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tab-cols-hidden")
            .unwrap();
        hidden.status = OracleQueryStatus::Failed {
            class: OracleQueryFailure::PermissionDenied,
        };
        hidden.rows.clear();

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        assert_eq!(blueprint.tables.values().next().unwrap().cols.len(), 2);
        let structure = blueprint.structure_scope.as_ref().unwrap();
        assert_eq!(structure.column_inventory_completeness, "complete");
        assert!(!structure
            .limitations
            .contains(&"column-inventory-unavailable".to_string()));
    }

    #[test]
    fn unavailable_hidden_column_catalog_degrades_only_a_table_missing_an_ordinal() {
        let mut capture = fixture();
        let columns = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tab-columns")
            .unwrap();
        columns.rows[1].values[3] = OracleValue::Null;
        let hidden = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tab-cols-hidden")
            .unwrap();
        hidden.status = OracleQueryStatus::Failed {
            class: OracleQueryFailure::PermissionDenied,
        };
        hidden.rows.clear();

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("a missing invisible-column ordinal must degrade rather than abort");

        let table = blueprint.tables.values().next().unwrap();
        assert_eq!(table.cols.len(), 1);
        assert!(table
            .table_limitations
            .contains(&"column-inventory-unavailable".to_string()));
        let structure = blueprint.structure_scope.as_ref().unwrap();
        assert_eq!(structure.column_inventory_completeness, "incomplete");
        assert!(structure
            .limitations
            .contains(&"column-inventory-unavailable".to_string()));
    }

    #[test]
    fn nullable_catalog_flags_degrade_only_the_affected_inventory() {
        let mut capture = fixture();
        capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tables")
            .unwrap()
            .rows[0]
            .values[13] = OracleValue::Null;
        capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tab-columns")
            .unwrap()
            .rows[0]
            .values[8] = OracleValue::Null;

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        assert_eq!(blueprint.tables.values().next().unwrap().cols.len(), 1);
        let structure = blueprint.structure_scope.as_ref().unwrap();
        assert_eq!(structure.table_inventory_completeness, "incomplete");
        assert_eq!(structure.column_inventory_completeness, "incomplete");
        assert!(structure
            .limitations
            .contains(&"dependent-structure-suppressed".to_string()));
    }

    #[test]
    fn unavailable_segments_fall_back_to_catalog_logical_size() {
        let mut capture = fixture();
        let segments = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-segments")
            .unwrap();
        segments.status = OracleQueryStatus::Failed {
            class: OracleQueryFailure::PermissionDenied,
        };
        segments.rows.clear();

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        let dataset = blueprint.dataset_scope.as_ref().unwrap();
        assert_eq!(dataset.size_completeness, "incomplete");
        assert_eq!(dataset.size_method, "oracle-table-logical-estimate");
        assert!(dataset
            .limitations
            .contains(&"size-evidence-incomplete".to_string()));
        let table = blueprint.tables.values().next().unwrap();
        assert!(table.table_bytes > 0);
        let size = table.statistics.as_ref().unwrap();
        assert_eq!(size.size_method, "oracle-table-logical-estimate");
        assert_eq!(size.size_quality, "engine-estimate");
        assert_eq!(size.size_accounting, "logical-estimate");
        assert_eq!(size.size_visibility, "partial");
    }

    #[test]
    fn unreadable_lob_catalog_preserves_measured_segment_bytes_as_partial() {
        let mut capture = fixture();
        let lobs = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-lobs")
            .unwrap();
        lobs.status = OracleQueryStatus::Failed {
            class: OracleQueryFailure::PermissionDenied,
        };
        lobs.rows.clear();

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        let dataset = blueprint.dataset_scope.as_ref().unwrap();
        assert_eq!(dataset.size_completeness, "incomplete");
        assert_eq!(dataset.size_method, "oracle-segment-bytes");
        let table = blueprint.tables.values().next().unwrap();
        assert_eq!(table.table_bytes, format::round_bytes(8192));
        assert_eq!(table.index_bytes, 0);
        let size = table.statistics.as_ref().unwrap();
        assert_eq!(size.size_method, "oracle-segment-bytes");
        assert_eq!(size.size_quality, "exact-counter");
        assert_eq!(size.size_accounting, "allocated-segment");
        assert_eq!(size.size_visibility, "partial");
    }

    #[test]
    fn all_excluded_tables_do_not_claim_an_aggregate_segment_measurement() {
        let mut capture = fixture();
        capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-objects")
            .unwrap()
            .rows[0]
            .values[2] = text("MATERIALIZED VIEW");
        let segments = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-segments")
            .unwrap();
        segments.rows.clear();
        segments.status = OracleQueryStatus::Failed {
            class: OracleQueryFailure::PermissionDenied,
        };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("an excluded-only estate has a complete empty copy population");

        assert_eq!(blueprint.totals.table_count, 0);
        assert_eq!(blueprint.totals.row_count, 0);
        assert_eq!(blueprint.totals.table_bytes, 0);
        assert_eq!(blueprint.totals.index_bytes, 0);
        let table = blueprint.tables.values().next().unwrap();
        assert_eq!(table.object_kind, "materialized-view");
        assert_eq!(table.counted_in_totals, Some(false));
        assert_eq!(
            table.statistics.as_ref().unwrap().size_method,
            "oracle-table-logical-estimate"
        );
        let dataset = blueprint.dataset_scope.as_ref().unwrap();
        assert_eq!(dataset.row_count_method, "not-applicable");
        assert_eq!(dataset.size_completeness, "complete");
        assert_eq!(dataset.size_method, "not-applicable");
        assert!(!dataset
            .limitations
            .contains(&"statistics-stale".to_string()));
        let aggregate = blueprint.statistics_evidence.as_ref().unwrap();
        assert_eq!(aggregate.visibility, "unknown");
        assert!(!aggregate
            .catalogs_read
            .contains(&"oracle-segments".to_string()));
        assert!(aggregate
            .catalogs_unreadable
            .contains(&"oracle-segments".to_string()));
        assert!(aggregate
            .limitations
            .contains(&"statistics-partial".to_string()));
        assert!(aggregate
            .limitations
            .contains(&"statistics-visibility-unknown".to_string()));
        assert!(aggregate
            .limitations
            .contains(&"statistics-stale".to_string()));
        dbwarp_blueprint_core::validate_blueprint_contract(&blueprint).unwrap();
    }

    #[test]
    fn clustered_table_never_reports_unattributed_storage_as_exact_zero() {
        let mut capture = fixture();
        let table = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tables")
            .unwrap();
        table.rows[0].values[15] = text("ORDERS_CLUSTER");
        let segments = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-segments")
            .unwrap();
        segments.rows[0].values[1] = text("ORDERS_CLUSTER");
        segments.rows[0].values[2] = text("CLUSTER");

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        let table = blueprint.tables.values().next().unwrap();
        assert!(table.table_bytes > 0);
        let size = table.statistics.as_ref().unwrap();
        assert_eq!(size.size_method, "oracle-table-logical-estimate");
        assert_eq!(size.size_quality, "engine-estimate");
        assert_eq!(size.size_visibility, "partial");
        let dataset = blueprint.dataset_scope.as_ref().unwrap();
        assert_eq!(dataset.size_method, "oracle-table-logical-estimate");
        assert_eq!(dataset.size_completeness, "incomplete");
    }

    #[test]
    fn sub_bucket_segment_bytes_preserve_materialized_state_from_raw_evidence() {
        let mut capture = fixture();
        capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-segments")
            .unwrap()
            .rows[0]
            .values[4] = number(1);

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("privacy rounding must not turn attributed storage into deferred metadata");

        let table = blueprint.tables.values().next().unwrap();
        assert_eq!(table.table_bytes, 0);
        assert_eq!(table.index_bytes, 0);
        assert_eq!(table.segment_state, "created");
        assert_eq!(
            table.statistics.as_ref().unwrap().size_method,
            "oracle-segment-bytes"
        );
        dbwarp_blueprint_core::validate_blueprint_contract(&blueprint).unwrap();
    }

    #[test]
    fn sub_bucket_index_segment_uses_mixed_state_instead_of_measured_zero() {
        let mut capture = fixture();
        let indexes = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-indexes")
            .unwrap();
        indexes.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_NOTE_IX"),
                text("APP"),
                text("ORDERS"),
                text("NORMAL"),
                text("NONUNIQUE"),
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                text("NO"),
                text("VALID"),
                text("N"),
                text("NO"),
                text("VISIBLE"),
                text("NO"),
                text("YES"),
            ],
        }];
        indexes.status = OracleQueryStatus::Executed { rows: 1 };
        let index_columns = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-index-columns")
            .unwrap();
        index_columns.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_NOTE_IX"),
                text("ORDERS"),
                text("NOTE"),
                number(1),
                text("ASC"),
                text("APP"),
            ],
        }];
        index_columns.status = OracleQueryStatus::Executed { rows: 1 };
        let segments = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-segments")
            .unwrap();
        segments.rows.push(OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_NOTE_IX"),
                text("INDEX"),
                text("USERS"),
                number(1),
                number(1),
            ],
        });
        segments.status = OracleQueryStatus::Executed { rows: 2 };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("a rounded-away index allocation must remain distinguishable from absence");

        let table = blueprint.tables.values().next().unwrap();
        assert_eq!(table.table_bytes, 8192);
        assert_eq!(table.index_bytes, 0);
        assert_eq!(table.segment_state, "mixed");
        dbwarp_blueprint_core::validate_blueprint_contract(&blueprint).unwrap();
    }

    #[test]
    fn minimum_tier_iot_never_reports_unmapped_index_storage_as_exact_zero() {
        let mut capture = fixture();
        capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tables")
            .unwrap()
            .rows[0]
            .values[9] = text("IOT");
        let segments = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-segments")
            .unwrap();
        segments.rows[0].values[1] = text("ORDERS_PK");
        segments.rows[0].values[2] = text("INDEX");
        for query_id in [
            "oracle-indexes",
            "oracle-index-columns",
            "oracle-index-expressions",
        ] {
            let outcome = capture
                .outcomes
                .iter_mut()
                .find(|outcome| outcome.query_id == query_id)
                .unwrap();
            outcome.status = OracleQueryStatus::Failed {
                class: OracleQueryFailure::PermissionDenied,
            };
            outcome.rows.clear();
        }

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        let table = blueprint.tables.values().next().unwrap();
        assert!(table.table_bytes > 0);
        let size = table.statistics.as_ref().unwrap();
        assert_eq!(size.size_method, "oracle-table-logical-estimate");
        assert_eq!(size.size_quality, "engine-estimate");
        assert_eq!(size.size_visibility, "partial");
    }

    #[test]
    fn oracle_12_1_external_catalog_overrides_the_stable_table_placeholder() {
        let mut capture = fixture();
        capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-version")
            .unwrap()
            .rows[0]
            .values
            .truncate(2);
        capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tables")
            .unwrap()
            .rows[0]
            .values[3] = OracleValue::Null;
        let external = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-external-tables")
            .unwrap();
        external.rows = vec![OracleRow {
            values: vec![text("APP"), text("ORDERS")],
        }];
        external.status = OracleQueryStatus::Executed { rows: 1 };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[12, 1, 0, 2]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        let table = blueprint.tables.values().next().unwrap();
        assert_eq!(table.object_kind, "external-table");
        assert_eq!(table.counted_in_totals, Some(false));
    }

    #[test]
    fn hidden_catalog_flags_distinguish_invisible_from_system_columns() {
        let mut capture = fixture();
        capture
            .outcomes
            .retain(|outcome| outcome.query_id != "oracle-tab-cols-hidden");
        capture.outcomes.push(outcome(
            "oracle-tab-cols-hidden",
            "DBA_TAB_COLS",
            vec![
                OracleRow {
                    values: vec![
                        text("APP"),
                        text("ORDERS"),
                        text("VISIBLE"),
                        text("NO"),
                        text("NO"),
                        number(1),
                        text("YES"),
                    ],
                },
                OracleRow {
                    values: vec![
                        text("APP"),
                        text("ORDERS"),
                        text("INVISIBLE"),
                        text("YES"),
                        text("NO"),
                        number(2),
                        text("YES"),
                    ],
                },
                OracleRow {
                    values: vec![
                        text("APP"),
                        text("ORDERS"),
                        text("SYSTEM"),
                        text("YES"),
                        text("NO"),
                        number(3),
                        text("NO"),
                    ],
                },
                OracleRow {
                    values: vec![
                        text("APP"),
                        text("ORDERS"),
                        text("UNCERTAIN"),
                        text("NO"),
                        text("UNRECOGNISED"),
                        number(4),
                        text("YES"),
                    ],
                },
            ],
        ));
        let facts = collect_column_catalog_facts(&capture, 1).unwrap();
        let visible = &facts[&("APP".into(), "ORDERS".into(), "VISIBLE".into())];
        assert_eq!(
            (visible.hidden, visible.invisible),
            (Some(false), Some(false))
        );
        let invisible = &facts[&("APP".into(), "ORDERS".into(), "INVISIBLE".into())];
        assert_eq!((invisible.hidden, invisible.invisible), (None, Some(true)));
        let system = &facts[&("APP".into(), "ORDERS".into(), "SYSTEM".into())];
        assert_eq!((system.hidden, system.invisible), (Some(true), None));
        let uncertain = &facts[&("APP".into(), "ORDERS".into(), "UNCERTAIN".into())];
        assert_eq!(uncertain.generated_virtual, None);
    }

    #[test]
    fn null_lob_options_remain_unknown() {
        let mut capture = fixture();
        let lobs = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-lobs")
            .unwrap();
        lobs.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS"),
                text("NOTE"),
                text("SYS_LOB_SEG"),
                text("SYS_LOB_IX"),
                number(8192),
                text("YES"),
                text("YES"),
                OracleValue::Null,
                OracleValue::Null,
                text("NO"),
            ],
        }];
        lobs.status = OracleQueryStatus::Executed { rows: 1 };
        let facts = collect_lob_facts(&capture, 1).unwrap();
        let lob = &facts[&("APP".into(), "ORDERS".into(), "NOTE".into())];
        assert_eq!(lob.compression, "unknown");
        assert_eq!(lob.deduplication, "unknown");
    }

    #[test]
    fn denied_nested_catalog_does_not_degrade_an_estate_without_nested_storage() {
        let mut capture = fixture();
        // An object-typed scalar column is not proof of nested-table storage.
        // DBA_TABLES.NESTED is explicitly NO for every table in this estate.
        capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tab-columns")
            .unwrap()
            .rows[1]
            .values[4] = text("OBJECT_PAYLOAD_T");
        capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-nested-tables")
            .unwrap()
            .status = OracleQueryStatus::Failed {
            class: OracleQueryFailure::PermissionDenied,
        };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();

        assert!(blueprint
            .tables
            .values()
            .next()
            .unwrap()
            .cols
            .values()
            .any(|column| column.column_type == "user-defined"));
        assert_eq!(
            blueprint.dataset_scope.as_ref().unwrap().size_completeness,
            "complete"
        );
        assert_eq!(
            blueprint
                .tables
                .values()
                .next()
                .unwrap()
                .statistics
                .as_ref()
                .unwrap()
                .size_visibility,
            "full"
        );
    }

    #[test]
    fn denied_nested_catalog_degrades_only_a_possible_collection_parent() {
        let mut capture = fixture();

        let tables = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tables")
            .unwrap();
        let mut parent = tables.rows[0].clone();
        parent.values[1] = text("COLLECTION_HOLDER");
        parent.values[3] = number(1);
        let mut storage = tables.rows[0].clone();
        storage.values[1] = text("SYS_NT_UNEXPOSED");
        storage.values[3] = number(1);
        storage.values[10] = text("YES");
        tables.rows.extend([parent, storage]);
        tables.status = OracleQueryStatus::Executed {
            rows: tables.rows.len() as u64,
        };

        let objects = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-objects")
            .unwrap();
        for table_name in ["COLLECTION_HOLDER", "SYS_NT_UNEXPOSED"] {
            let mut object = objects.rows[0].clone();
            object.values[1] = text(table_name);
            objects.rows.push(object);
        }
        objects.status = OracleQueryStatus::Executed {
            rows: objects.rows.len() as u64,
        };

        let columns = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tab-columns")
            .unwrap();
        // XMLTYPE shares the Blueprint user-defined class but is a scalar
        // structured type, not evidence that this table owns nested storage.
        columns.rows[1].values[4] = text("XMLTYPE");
        let mut collection_column = columns.rows[0].clone();
        collection_column.values[1] = text("COLLECTION_HOLDER");
        collection_column.values[2] = text("ITEMS");
        collection_column.values[4] = text("NUMBER_LIST_T");
        columns.rows.push(collection_column);
        columns.status = OracleQueryStatus::Executed {
            rows: columns.rows.len() as u64,
        };

        let hidden = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tab-cols-hidden")
            .unwrap();
        let mut collection_hidden = hidden.rows[0].clone();
        collection_hidden.values[1] = text("COLLECTION_HOLDER");
        collection_hidden.values[2] = text("ITEMS");
        hidden.rows.push(collection_hidden);
        hidden.status = OracleQueryStatus::Executed {
            rows: hidden.rows.len() as u64,
        };

        let segments = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-segments")
            .unwrap();
        let mut parent_segment = segments.rows[0].clone();
        parent_segment.values[1] = text("COLLECTION_HOLDER");
        let mut nested_segment = segments.rows[0].clone();
        nested_segment.values[1] = text("SYS_NT_UNEXPOSED");
        nested_segment.values[2] = text("NESTED TABLE");
        segments.rows.extend([parent_segment, nested_segment]);
        segments.status = OracleQueryStatus::Executed {
            rows: segments.rows.len() as u64,
        };

        capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-nested-tables")
            .unwrap()
            .status = OracleQueryStatus::Failed {
            class: OracleQueryFailure::PermissionDenied,
        };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();

        assert_eq!(
            blueprint.dataset_scope.as_ref().unwrap().size_completeness,
            "incomplete"
        );
        assert_eq!(
            blueprint.dataset_scope.as_ref().unwrap().size_method,
            "oracle-segment-bytes"
        );
        let collection_parent = blueprint
            .tables
            .values()
            .find(|table| {
                table
                    .cols
                    .values()
                    .any(|column| column.column_type == "user-defined")
                    && table.statistics.as_ref().unwrap().size_visibility == "partial"
            })
            .unwrap();
        assert_eq!(
            collection_parent
                .statistics
                .as_ref()
                .unwrap()
                .size_visibility,
            "partial"
        );
        assert_eq!(
            collection_parent.statistics.as_ref().unwrap().size_quality,
            "exact-counter"
        );
        assert_eq!(
            collection_parent.statistics.as_ref().unwrap().size_method,
            "oracle-segment-bytes"
        );
        assert!(blueprint.tables.values().any(|table| {
            table
                .cols
                .values()
                .any(|column| column.column_type == "user-defined")
                && table.statistics.as_ref().unwrap().size_visibility == "full"
        }));
    }

    #[test]
    fn unavailable_support_catalogs_degrade_without_aborting_core_tables() {
        let mut capture = fixture();
        for query_id in ["oracle-mview-logs", "oracle-queue-tables"] {
            capture
                .outcomes
                .iter_mut()
                .find(|outcome| outcome.query_id == query_id)
                .unwrap()
                .status = OracleQueryStatus::Failed {
                class: OracleQueryFailure::PermissionDenied,
            };
        }

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("support-table classification is optional but must degrade visibly");

        let dataset = blueprint.dataset_scope.as_ref().unwrap();
        assert_eq!(dataset.table_inventory_completeness, "incomplete");
        assert_eq!(dataset.row_count_completeness, "incomplete");
        assert_eq!(dataset.size_completeness, "incomplete");
        assert_eq!(dataset.size_method, "oracle-segment-bytes");
        assert!(dataset
            .limitations
            .iter()
            .any(|value| value == "row-count-evidence-incomplete"));
        assert!(dataset
            .limitations
            .iter()
            .any(|value| value == "size-evidence-incomplete"));
        let structure = blueprint.structure_scope.as_ref().unwrap();
        assert_eq!(structure.table_inventory_completeness, "incomplete");
        assert!(structure
            .limitations
            .iter()
            .any(|value| value == "table-kinds-not-inventoried"));
    }

    #[test]
    fn temporary_table_without_statistics_does_not_claim_session_statistics() {
        let mut capture = fixture();
        capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tables")
            .unwrap()
            .rows[0]
            .values[8] = text("Y");

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        let table = blueprint.tables.values().next().unwrap();

        assert_eq!(table.object_kind, "temporary-table");
        assert_eq!(table.counted_in_totals, Some(false));
        assert_eq!(
            table.statistics.as_ref().unwrap().statistics_scope,
            "unknown"
        );
        let aggregate = blueprint.statistics_evidence.as_ref().unwrap();
        assert_eq!(aggregate.visibility, "unknown");
        assert!(!aggregate
            .limitations
            .contains(&"statistics-partial".to_string()));
        assert!(aggregate
            .limitations
            .contains(&"statistics-visibility-unknown".to_string()));
    }

    #[test]
    fn duplicate_statistics_rows_degrade_only_the_affected_table() {
        let mut capture = fixture();
        let statistics = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tab-statistics")
            .unwrap();
        let statistics_row = |table: &str, sample_size: u64, scope: &str| OracleRow {
            values: vec![
                text("APP"),
                text(table),
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                text("2026-09-21T00:00:00Z"),
                text("NO"),
                OracleValue::Null,
                number(sample_size),
                text("YES"),
                text("NO"),
                OracleValue::Null,
                text(scope),
            ],
        };
        statistics.rows = vec![
            statistics_row("ORDERS", 10, "SHARED"),
            statistics_row("ORDERS", 11, "SESSION"),
            statistics_row("CUSTOMERS", 12, "SHARED"),
        ];
        statistics.status = OracleQueryStatus::Executed { rows: 3 };

        let collected = collect_statistics_facts(&capture);
        assert!(!collected.unscoped_malformed);
        assert!(!collected
            .facts
            .contains_key(&("APP".to_string(), "ORDERS".to_string())));
        assert!(collected
            .facts
            .contains_key(&("APP".to_string(), "CUSTOMERS".to_string())));
        assert_eq!(
            collected.invalid_tables,
            BTreeSet::from([("APP".to_string(), "ORDERS".to_string())])
        );
        assert!(!table_statistics_catalog_complete(
            true,
            false,
            &collected.invalid_tables,
            &("APP".to_string(), "ORDERS".to_string()),
        ));
        assert!(table_statistics_catalog_complete(
            true,
            false,
            &collected.invalid_tables,
            &("APP".to_string(), "CUSTOMERS".to_string()),
        ));
        assert!(!table_statistics_catalog_complete(
            false,
            false,
            &collected.invalid_tables,
            &("APP".to_string(), "CUSTOMERS".to_string()),
        ));
    }

    #[test]
    fn invalid_statistics_on_excluded_table_still_degrade_aggregate_provenance() {
        let mut capture = fixture();
        capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-objects")
            .unwrap()
            .rows[0]
            .values[2] = text("MATERIALIZED VIEW");
        let statistics = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tab-statistics")
            .unwrap();
        let duplicate = OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS"),
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                text("2026-09-21T00:00:00Z"),
                text("NO"),
                OracleValue::Null,
                number(10),
                text("YES"),
                text("NO"),
                OracleValue::Null,
                text("SHARED"),
            ],
        };
        statistics.rows = vec![duplicate.clone(), duplicate];
        statistics.status = OracleQueryStatus::Executed { rows: 2 };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("bad refinement evidence on an excluded table must degrade, not abort");

        let table = blueprint.tables.values().next().unwrap();
        assert_eq!(table.counted_in_totals, Some(false));
        assert_eq!(
            table.statistics.as_ref().unwrap().statistics_state,
            "unknown"
        );
        let aggregate = blueprint.statistics_evidence.as_ref().unwrap();
        assert_eq!(aggregate.visibility, "unknown");
        assert!(aggregate
            .limitations
            .contains(&"statistics-partial".to_string()));
        assert!(aggregate
            .limitations
            .contains(&"statistics-visibility-unknown".to_string()));
    }

    #[test]
    fn malformed_statistics_value_degrades_only_its_native_table() {
        let mut capture = fixture();
        let statistics = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tab-statistics")
            .unwrap();
        let statistics_row = |table: &str, sample_size: OracleValue| OracleRow {
            values: vec![
                text("APP"),
                text(table),
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                text("2026-09-21T00:00:00Z"),
                text("NO"),
                OracleValue::Null,
                sample_size,
                text("YES"),
                text("NO"),
                OracleValue::Null,
                text("SHARED"),
            ],
        };
        statistics.rows = vec![
            statistics_row("ORDERS", text("not-a-number")),
            statistics_row("CUSTOMERS", number(12)),
        ];
        statistics.status = OracleQueryStatus::Executed { rows: 2 };

        let collected = collect_statistics_facts(&capture);
        assert!(!collected.unscoped_malformed);
        assert!(!collected
            .facts
            .contains_key(&("APP".to_string(), "ORDERS".to_string())));
        assert!(collected
            .facts
            .contains_key(&("APP".to_string(), "CUSTOMERS".to_string())));
        assert_eq!(
            collected.invalid_tables,
            BTreeSet::from([("APP".to_string(), "ORDERS".to_string())])
        );
    }

    #[test]
    fn missing_generated_parent_key_column_never_emits_a_truncated_foreign_key() {
        let mut capture = fixture();
        let constraint_rows = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-constraint-columns")
            .unwrap();
        constraint_rows.rows = vec![
            OracleRow {
                values: vec![
                    text("APP"),
                    text("SYS_PARENT_PK"),
                    text("PARENT"),
                    text("VISIBLE_ID"),
                    number(1),
                ],
            },
            OracleRow {
                values: vec![
                    text("APP"),
                    text("SYS_PARENT_PK"),
                    text("PARENT"),
                    text("SYS_NC_HIDDEN"),
                    number(2),
                ],
            },
            OracleRow {
                values: vec![
                    text("APP"),
                    text("CHILD_FK"),
                    text("CHILD"),
                    text("PARENT_ID"),
                    number(1),
                ],
            },
        ];
        constraint_rows.status = OracleQueryStatus::Executed { rows: 3 };
        let constraint = |name: &str,
                          kind: &str,
                          table: &str,
                          referenced: Option<&str>,
                          generated_name: bool| RawConstraint {
            owner: "APP".to_string(),
            name: name.to_string(),
            constraint_type: kind.to_string(),
            table_name: table.to_string(),
            referenced_owner: referenced.map(|_| "APP".to_string()),
            referenced_constraint: referenced.map(str::to_string),
            delete_rule: "NO ACTION".to_string(),
            status: "ENABLED".to_string(),
            deferrable: false,
            initially_deferred: false,
            validated: true,
            index_owner: None,
            index_name: None,
            generated_name,
            is_not_null_constraint: None,
        };
        let constraints = vec![
            constraint("SYS_PARENT_PK", "P", "PARENT", None, true),
            constraint("CHILD_FK", "R", "CHILD", Some("SYS_PARENT_PK"), false),
        ];
        let ordinals = BTreeMap::from([
            (
                (
                    "APP".to_string(),
                    "PARENT".to_string(),
                    "VISIBLE_ID".to_string(),
                ),
                1,
            ),
            (
                (
                    "APP".to_string(),
                    "CHILD".to_string(),
                    "PARENT_ID".to_string(),
                ),
                1,
            ),
        ]);
        let constraints_by_key = constraints
            .iter()
            .map(|constraint| {
                (
                    (constraint.owner.clone(), constraint.name.clone()),
                    constraint,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let (columns, _, invalid) =
            collect_constraint_columns(&capture, &ordinals, &constraints_by_key, 1).unwrap();
        let parent_key = ("APP".to_string(), "SYS_PARENT_PK".to_string());
        assert!(invalid.contains(&parent_key));
        assert!(!columns.contains_key(&parent_key));

        let (edges, _) = build_fk_edges(
            &constraints,
            &columns,
            &BTreeMap::from([
                (
                    ("APP".to_string(), "PARENT".to_string()),
                    "table-001".to_string(),
                ),
                (
                    ("APP".to_string(), "CHILD".to_string()),
                    "table-002".to_string(),
                ),
            ]),
            &invalid,
            &BTreeMap::from([
                (
                    ("APP".to_string(), "PARENT".to_string()),
                    BTreeSet::from([1]),
                ),
                (
                    ("APP".to_string(), "CHILD".to_string()),
                    BTreeSet::from([1]),
                ),
            ]),
        );
        assert!(edges.is_empty());

        let representable_constraints = vec![
            constraint("PARENT_PK", "P", "PARENT", None, false),
            constraint("CHILD_FK", "R", "CHILD", Some("PARENT_PK"), false),
        ];
        let representable_columns = BTreeMap::from([
            (("APP".to_string(), "PARENT_PK".to_string()), vec![1]),
            (("APP".to_string(), "CHILD_FK".to_string()), vec![1]),
        ]);
        let (suppressed_edges, suppressed_tables) = build_fk_edges(
            &representable_constraints,
            &representable_columns,
            &BTreeMap::from([
                (
                    ("APP".to_string(), "PARENT".to_string()),
                    "table-001".to_string(),
                ),
                (
                    ("APP".to_string(), "CHILD".to_string()),
                    "table-002".to_string(),
                ),
            ]),
            &BTreeSet::new(),
            &BTreeMap::from([
                (
                    ("APP".to_string(), "PARENT".to_string()),
                    BTreeSet::from([1]),
                ),
                (("APP".to_string(), "CHILD".to_string()), BTreeSet::new()),
            ]),
        );
        assert!(suppressed_edges.is_empty());
        assert_eq!(
            suppressed_tables,
            BTreeSet::from([("APP".to_string(), "CHILD".to_string())])
        );
    }

    #[test]
    fn malformed_constraint_status_degrades_relationship_inventory() {
        let mut capture = fixture();
        let constraints = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-constraints")
            .unwrap();
        constraints.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("CUSTOMER_RULE"),
                text("C"),
                text("ORDERS"),
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                text("UNRECOGNISED"),
                text("NOT DEFERRABLE"),
                text("VALIDATED"),
                text("IMMEDIATE"),
                OracleValue::Null,
                OracleValue::Null,
                text("USER NAME"),
                text("N"),
            ],
        }];
        constraints.status = OracleQueryStatus::Executed { rows: 1 };
        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::Summary,
            },
        )
        .expect("an optional relationship defect must preserve core tables");
        assert_eq!(blueprint.tables.values().next().unwrap().check_count, None);
        let structure = blueprint.structure_scope.as_ref().unwrap();
        assert_eq!(structure.relationship_inventory_completeness, "incomplete");
        assert!(structure
            .catalogs_read
            .contains(&"oracle-constraints".to_string()));
        assert!(!structure
            .catalogs_unreadable
            .contains(&"oracle-constraints".to_string()));
    }

    #[test]
    fn null_check_constraint_position_does_not_hide_valid_relationship_metadata() {
        let mut capture = fixture();
        let constraints = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-constraints")
            .unwrap();
        constraints.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_AMOUNT_CK"),
                text("C"),
                text("ORDERS"),
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                text("ENABLED"),
                text("NOT DEFERRABLE"),
                text("VALIDATED"),
                text("IMMEDIATE"),
                OracleValue::Null,
                OracleValue::Null,
                text("USER NAME"),
                text("N"),
            ],
        }];
        constraints.status = OracleQueryStatus::Executed { rows: 1 };
        let constraint_columns = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-constraint-columns")
            .unwrap();
        constraint_columns.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_AMOUNT_CK"),
                text("ORDERS"),
                text("ID"),
                OracleValue::Null,
            ],
        }];
        constraint_columns.status = OracleQueryStatus::Executed { rows: 1 };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("a valid CHECK constraint may have a null key position");
        let table = blueprint.tables.values().next().unwrap();
        assert_eq!(table.check_count, Some(1));
        assert_eq!(
            blueprint
                .structure_scope
                .as_ref()
                .unwrap()
                .relationship_inventory_completeness,
            "complete"
        );
    }

    #[test]
    fn denied_constraint_columns_preserve_independent_check_count() {
        let mut capture = fixture();
        let constraints = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-constraints")
            .unwrap();
        constraints.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_AMOUNT_CK"),
                text("C"),
                text("ORDERS"),
                OracleValue::Null,
                OracleValue::Null,
                OracleValue::Null,
                text("ENABLED"),
                text("NOT DEFERRABLE"),
                text("VALIDATED"),
                text("IMMEDIATE"),
                OracleValue::Null,
                OracleValue::Null,
                text("USER NAME"),
                text("N"),
            ],
        }];
        constraints.status = OracleQueryStatus::Executed { rows: 1 };
        let constraint_columns = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-constraint-columns")
            .unwrap();
        constraint_columns.rows.clear();
        constraint_columns.status = OracleQueryStatus::Failed {
            class: OracleQueryFailure::PermissionDenied,
        };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();

        assert_eq!(
            blueprint.tables.values().next().unwrap().check_count,
            Some(1)
        );
        assert_eq!(
            blueprint
                .structure_scope
                .as_ref()
                .unwrap()
                .relationship_inventory_completeness,
            "incomplete"
        );
    }

    #[test]
    fn generated_index_over_only_hidden_system_columns_is_not_a_logical_index() {
        let generated = RawIndex {
            owner: "APP".to_string(),
            name: "SYS_OBJECT_SUPPORT".to_string(),
            table_owner: "APP".to_string(),
            table_name: "OBJECT_TABLE".to_string(),
            index_type: "NORMAL".to_string(),
            unique: true,
            partitioned: false,
            status: "VALID".to_string(),
            generated: true,
            secondary: false,
            visibility: "VISIBLE".to_string(),
            join_index: false,
            segment_created: Some(true),
        };
        let key = (generated.owner.clone(), generated.name.clone());
        let mut columns = IndexColumnsByIndex::new();
        columns.insert(
            key.clone(),
            vec![RawIndexColumn {
                position: 1,
                ordinal: None,
                descending: false,
            }],
        );
        assert!(is_generated_hidden_support_index(&generated, &columns));

        let mut user_named = generated.clone();
        user_named.generated = false;
        assert!(!is_generated_hidden_support_index(&user_named, &columns));

        columns.insert(
            key,
            vec![RawIndexColumn {
                position: 1,
                ordinal: Some(1),
                descending: false,
            }],
        );
        assert!(!is_generated_hidden_support_index(&generated, &columns));
    }

    #[test]
    fn oracle_type_semantics_preserve_only_proven_bounds() {
        let unconstrained = oracle_column_semantics("NUMBER", Some(22), None, "", None, None);
        assert_eq!(unconstrained.numeric_model, "unconstrained-decimal");
        assert_eq!(unconstrained.numeric_precision, None);
        assert_eq!(unconstrained.numeric_scale, None);

        let scaled = oracle_column_semantics("NUMBER", Some(22), None, "", Some(8), Some(-2));
        assert_eq!(scaled.numeric_model, "fixed-decimal");
        assert_eq!(scaled.numeric_precision, Some(8));
        assert_eq!(scaled.numeric_scale, Some(-2));

        let chars = oracle_column_semantics("VARCHAR2", Some(400), Some(100), "C", None, None);
        assert_eq!(chars.declared_max_chars, 100);
        assert_eq!(chars.declared_max_bytes, 400);
        assert_eq!(chars.length_semantics, "characters");

        let raw = oracle_column_semantics("RAW", Some(2000), None, "", None, None);
        let blob = oracle_column_semantics("BLOB", Some(4000), None, "", None, None);
        assert_eq!(raw.declared_max_bytes, 2000);
        assert_eq!(
            blob.declared_max_bytes, 0,
            "LOB locator width is not a payload bound"
        );

        let timestamp = oracle_column_semantics("TIMESTAMP(9)", Some(11), None, "", None, Some(9));
        assert_eq!(timestamp.datetime_precision, 9);
    }

    #[test]
    fn unknown_storage_and_index_tokens_are_not_guessed() {
        let mut table = collect_tables(&fixture()).unwrap().remove(0);
        table.iot_type = Some("FUTURE_ORGANIZATION".to_string());
        assert_eq!(oracle_storage_organization(&table, false), "unknown");
        assert_eq!(normalize_index_type("FUTURE INDEX"), "unknown");
        assert_eq!(normalize_index_type("NORMAL"), "btree");
        assert_eq!(normalize_index_type("FUNCTION-BASED BITMAP"), "bitmap");
    }

    #[test]
    fn oracle_storage_helpers_use_catalog_flags_not_customer_name_prefixes() {
        let base = collect_tables(&fixture()).unwrap().remove(0);
        let facts = OracleObjectFacts::default();
        let maintained = BTreeSet::new();
        let no_support_tables = BTreeSet::new();
        assert!(oracle_table_is_customer_data(
            &base,
            &facts,
            &maintained,
            &no_support_tables
        ));
        let mut materialized_view_log = base.clone();
        materialized_view_log.name = "MLOG$_ORDERS".to_string();
        assert!(oracle_table_is_customer_data(
            &materialized_view_log,
            &facts,
            &maintained,
            &no_support_tables
        ));
        let mut queue = base.clone();
        queue.name = "AQ$_ORDERS_QTAB_H".to_string();
        assert!(oracle_table_is_customer_data(
            &queue,
            &facts,
            &maintained,
            &no_support_tables
        ));
        let catalog_support_tables = BTreeSet::from([
            ("APP".to_string(), "MLOG$_ORDERS".to_string()),
            ("APP".to_string(), "AQ$_ORDERS_QTAB_H".to_string()),
        ]);
        assert!(!oracle_table_is_customer_data(
            &materialized_view_log,
            &facts,
            &maintained,
            &catalog_support_tables
        ));
        assert!(!oracle_table_is_customer_data(
            &queue,
            &facts,
            &maintained,
            &catalog_support_tables
        ));
        let generated = OracleObjectFacts {
            generated: ObservedOptionalBool::True,
            ..OracleObjectFacts::default()
        };
        assert!(!oracle_table_is_customer_data(
            &materialized_view_log,
            &generated,
            &maintained,
            &no_support_tables
        ));
        let secondary = OracleObjectFacts {
            secondary: ObservedOptionalBool::True,
            ..OracleObjectFacts::default()
        };
        assert!(!oracle_table_is_customer_data(
            &queue,
            &secondary,
            &maintained,
            &no_support_tables
        ));
        let mut dropped = base.clone();
        dropped.dropped = true;
        assert!(!oracle_table_is_customer_data(
            &dropped,
            &facts,
            &maintained,
            &no_support_tables
        ));
        let mut nested = base.clone();
        nested.nested = Some(true);
        assert!(!oracle_table_is_customer_data(
            &nested,
            &facts,
            &maintained,
            &no_support_tables
        ));
        let mut mapping = base.clone();
        mapping.iot_type = Some("IOT_MAPPING".to_string());
        assert!(!oracle_table_is_customer_data(
            &mapping,
            &facts,
            &maintained,
            &no_support_tables
        ));
        let maintained = BTreeSet::from(["APP".to_string()]);
        assert!(!oracle_table_is_customer_data(
            &base,
            &facts,
            &maintained,
            &no_support_tables
        ));
    }

    #[test]
    fn aq_support_identities_are_anchored_to_the_queue_catalog_row() {
        let mut capture = fixture();
        let queues = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-queue-tables")
            .unwrap();
        queues.rows = vec![OracleRow {
            values: vec![text("APP"), text("ORDER_EVENT_QTAB")],
        }];
        queues.status = OracleQueryStatus::Executed { rows: 1 };

        let support = collect_support_table_keys(&capture, "oracle-queue-tables", true, 1).unwrap();
        for table in [
            "ORDER_EVENT_QTAB",
            "AQ$_ORDER_EVENT_QTAB_C",
            "AQ$_ORDER_EVENT_QTAB_D",
            "AQ$_ORDER_EVENT_QTAB_H",
            "AQ$_ORDER_EVENT_QTAB_I",
            "AQ$_ORDER_EVENT_QTAB_P",
            "AQ$_ORDER_EVENT_QTAB_T",
            "AQ$_ORDER_EVENT_QTAB_S",
            "AQ$_ORDER_EVENT_QTAB_L",
        ] {
            assert!(support.contains(&("APP".to_string(), table.to_string())));
        }
        assert!(!support.contains(&(
            "APP".to_string(),
            "AQ$_UNRELATED_CUSTOMER_TABLE_H".to_string()
        )));
    }

    #[test]
    fn oracle_sample_fraction_bands_are_closed_at_the_boundaries() {
        assert_eq!(sample_fraction_band(None, Some(1)), "unknown");
        assert_eq!(sample_fraction_band(Some(0), Some(0)), "unknown");
        assert_eq!(sample_fraction_band(Some(100), Some(24)), "under-25pct");
        assert_eq!(sample_fraction_band(Some(100), Some(25)), "25-49pct");
        assert_eq!(sample_fraction_band(Some(100), Some(50)), "50-74pct");
        assert_eq!(sample_fraction_band(Some(100), Some(75)), "75-99pct");
        assert_eq!(sample_fraction_band(Some(100), Some(100)), "full");
    }

    #[test]
    fn normalized_decimal_catalog_numbers_produce_a_valid_floor_blueprint() {
        let mut capture = fixture();
        let table = &mut capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tables")
            .unwrap()
            .rows[0];
        table.values[3] = text("120.0");
        table.values[4] = text("4.00");
        table.values[5] = text("20.000");
        let columns = &mut capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tab-columns")
            .unwrap()
            .rows;
        columns[0].values[3] = text("1.0");
        columns[0].values[5] = text("22.0");
        columns[0].values[6] = text("38.0");
        columns[0].values[7] = text("0.0");
        columns[1].values[3] = text("2.0");
        columns[1].values[5] = text("200.0");
        columns[1].values[9] = text("100.0");
        columns[1].values[11] = text("3.0");
        let hidden = &mut capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tab-cols-hidden")
            .unwrap()
            .rows;
        hidden[0].values[5] = text("1.0");
        hidden[1].values[5] = text("2.0");
        let segment = &mut capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-segments")
            .unwrap()
            .rows[0];
        segment.values[4] = text("8192.0");
        segment.values[5] = text("1.0");

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("the adapter's pinned Oracle decimal punctuation must preserve the floor");
        let table = blueprint.tables.values().next().unwrap();
        assert_eq!(table.cols.len(), 2);
        assert!(table.rows > 0);
        assert!(table.table_bytes > 0);
        assert_eq!(
            blueprint
                .structure_scope
                .as_ref()
                .unwrap()
                .column_inventory_completeness,
            "complete"
        );

        assert_eq!(parse_oracle_u64("42.0").unwrap(), 42);
        assert_eq!(parse_oracle_u64("42.000").unwrap(), 42);
        assert_eq!(parse_oracle_i64("-7.00").unwrap(), -7);
        assert!(parse_oracle_u64("42.5").is_err());
        assert!(parse_oracle_u64("42,0").is_err());
        assert!(parse_oracle_u64("42,000").is_err());
    }

    #[test]
    fn nullable_object_flags_reduce_independently_of_row_order() {
        let reduce = |values: &[Option<bool>]| {
            let mut reduced = ObservedOptionalBool::default();
            for value in values {
                reduced.observe(*value);
            }
            reduced.value()
        };

        assert_eq!(reduce(&[Some(false), None]), None);
        assert_eq!(reduce(&[None, Some(false)]), None);
        assert_eq!(reduce(&[Some(true), None, Some(false)]), Some(true));
        assert_eq!(reduce(&[None, Some(false), Some(true)]), Some(true));
        assert_eq!(reduce(&[Some(false), Some(false)]), Some(false));
    }

    #[derive(Deserialize)]
    struct ExternalCaptureFixture {
        server_version: Vec<u16>,
        owners: Vec<String>,
        outcomes: Vec<ExternalCaptureOutcome>,
        expected: ExternalCaptureExpected,
        forbidden_strings: Vec<String>,
    }

    #[derive(Deserialize)]
    struct ExternalCaptureOutcome {
        query_id: String,
        owner_ordinal: Option<u32>,
        rows: Vec<Vec<Option<String>>>,
    }

    #[derive(Deserialize)]
    struct ExternalCaptureExpected {
        table_count: usize,
        column_count: usize,
        partitioned_table_count: usize,
        external_table_count: usize,
    }

    #[test]
    fn mismatched_foreign_key_vectors_degrade_relationships_instead_of_dropping_silently() {
        let mut capture = fixture();
        let constraints = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-constraints")
            .unwrap();
        let row = |name: &str, kind: &str, referenced: Option<&str>| OracleRow {
            values: vec![
                text("APP"),
                text(name),
                text(kind),
                text("ORDERS"),
                referenced.map(|_| text("APP")).unwrap_or(OracleValue::Null),
                referenced.map(text).unwrap_or(OracleValue::Null),
                text("NO ACTION"),
                text("ENABLED"),
                text("NOT DEFERRABLE"),
                text("VALIDATED"),
                text("IMMEDIATE"),
                OracleValue::Null,
                OracleValue::Null,
                text("USER NAME"),
                text("N"),
            ],
        };
        constraints.rows = vec![
            row("ORDERS_PK", "P", None),
            row("ORDERS_PARENT_FK", "R", Some("ORDERS_PK")),
        ];
        constraints.status = OracleQueryStatus::Executed { rows: 2 };
        let columns = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-constraint-columns")
            .unwrap();
        let key_column = |constraint: &str, column: &str, position: u64| OracleRow {
            values: vec![
                text("APP"),
                text(constraint),
                text("ORDERS"),
                text(column),
                number(position),
            ],
        };
        columns.rows = vec![
            key_column("ORDERS_PK", "ID", 1),
            key_column("ORDERS_PARENT_FK", "ID", 1),
            key_column("ORDERS_PARENT_FK", "NOTE", 2),
        ];
        columns.status = OracleQueryStatus::Executed { rows: 3 };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        assert!(blueprint.fk_edges.is_empty());
        let table = blueprint.tables.values().next().unwrap();
        assert!(table
            .table_limitations
            .contains(&"relationship-inventory-unavailable".to_string()));
        assert_eq!(
            blueprint
                .structure_scope
                .as_ref()
                .unwrap()
                .relationship_inventory_completeness,
            "incomplete"
        );
    }

    #[test]
    fn foreign_key_without_referenced_owner_is_not_a_complete_silent_drop() {
        let mut capture = fixture();
        let constraints = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-constraints")
            .unwrap();
        constraints.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_PARENT_FK"),
                text("R"),
                text("ORDERS"),
                OracleValue::Null,
                text("ORDERS_PK"),
                text("NO ACTION"),
                text("ENABLED"),
                text("NOT DEFERRABLE"),
                text("VALIDATED"),
                text("IMMEDIATE"),
                OracleValue::Null,
                OracleValue::Null,
                text("USER NAME"),
                text("N"),
            ],
        }];
        constraints.status = OracleQueryStatus::Executed { rows: 1 };
        let columns = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-constraint-columns")
            .unwrap();
        columns.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS_PARENT_FK"),
                text("ORDERS"),
                text("ID"),
                number(1),
            ],
        }];
        columns.status = OracleQueryStatus::Executed { rows: 1 };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        assert!(blueprint.fk_edges.is_empty());
        assert!(blueprint
            .tables
            .values()
            .next()
            .unwrap()
            .table_limitations
            .contains(&"relationship-inventory-unavailable".to_string()));
    }

    #[test]
    fn unattempted_capacity_query_is_not_reported_as_unreadable() {
        let mut capture = fixture();
        let capacity = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-capacity-parameters")
            .unwrap();
        capacity.rows.clear();
        capacity.status = OracleQueryStatus::NotReached;
        let environment = build_source_environment(&capture);
        assert!(environment.catalogs_read.is_empty());
        assert!(environment.catalogs_unreadable.is_empty());
        assert_eq!(environment.capacity_visibility, "unknown");
    }

    #[test]
    fn partially_read_capacity_catalog_keeps_read_provenance_but_withholds_values() {
        let mut capture = fixture();
        capture.outcomes.push(OracleQueryOutcome {
            query_id: "oracle-capacity-parameters",
            view: "V_$PARAMETER",
            owner_ordinal: None,
            status: OracleQueryStatus::Failed {
                class: OracleQueryFailure::PermissionDenied,
            },
            rows: Vec::new(),
        });
        let environment = build_source_environment(&capture);
        assert_eq!(
            environment.catalogs_read,
            vec!["oracle-capacity-parameters".to_string()]
        );
        assert!(environment.catalogs_unreadable.is_empty());
        assert_eq!(environment.capacity_visibility, "unknown");
        assert_eq!(environment.cpu_capacity_band, "unknown");
        assert_eq!(environment.memory_capacity_band, "unknown");
    }

    #[test]
    fn optional_column_and_relationship_catalogues_keep_their_own_provenance() {
        for (query_id, expected_catalog, expected_limitation) in [
            (
                "oracle-identity-columns",
                "oracle-identity-columns",
                Some("metadata-visibility-unknown"),
            ),
            (
                "oracle-constraint-columns",
                "oracle-constraint-columns",
                None,
            ),
        ] {
            let mut capture = fixture();
            let outcome = capture
                .outcomes
                .iter_mut()
                .find(|outcome| outcome.query_id == query_id)
                .unwrap();
            outcome.rows.clear();
            outcome.status = OracleQueryStatus::Failed {
                class: OracleQueryFailure::PermissionDenied,
            };
            let blueprint = map_oracle_basic_capture(
                &capture,
                &OracleOwnerScope::one("APP"),
                OracleVersion::from_components(&[21]).unwrap(),
                &OracleBasicOptions {
                    source_kind: "production".to_string(),
                    generated_at_pin: None,
                    artifact_detail: ArtifactDetail::None,
                },
            )
            .unwrap();
            let structure = blueprint.structure_scope.as_ref().unwrap();
            assert!(structure
                .catalogs_unreadable
                .contains(&expected_catalog.to_string()));
            assert!(structure
                .catalogs_read
                .iter()
                .all(|value| value != expected_catalog));
            if let Some(limitation) = expected_limitation {
                assert!(structure.limitations.contains(&limitation.to_string()));
            }
        }
    }

    #[test]
    fn table_segment_created_no_proves_deferred_storage_without_a_segment_row() {
        let mut capture = fixture();
        capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-tables")
            .unwrap()
            .rows[0]
            .values[17] = text("NO");
        let segments = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-segments")
            .unwrap();
        segments.rows.clear();
        segments.status = OracleQueryStatus::Executed { rows: 0 };
        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        let table = blueprint.tables.values().next().unwrap();
        assert_eq!(table.segment_state, "deferred");
        assert_eq!(table.table_bytes, 0);
        assert_eq!(
            table.statistics.as_ref().unwrap().size_quality,
            "exact-counter"
        );
        assert_eq!(table.statistics.as_ref().unwrap().size_visibility, "full");
    }

    #[test]
    fn lob_bytes_do_not_prove_that_the_table_segment_was_observed() {
        let mut capture = fixture();
        let lobs = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-lobs")
            .unwrap();
        lobs.rows.push(OracleRow {
            values: vec![
                text("APP"),
                text("ORDERS"),
                text("PAYLOAD"),
                text("SYS_LOB_ORDERS_PAYLOAD"),
                text("SYS_IL_ORDERS_PAYLOAD"),
                number(8192),
                text("YES"),
                text("YES"),
                text("NONE"),
                text("NO"),
                text("NO"),
            ],
        });
        lobs.status = OracleQueryStatus::Executed { rows: 1 };
        let segments = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-segments")
            .unwrap();
        segments.rows = vec![OracleRow {
            values: vec![
                text("APP"),
                text("SYS_LOB_ORDERS_PAYLOAD"),
                text("LOBSEGMENT"),
                text("USERS"),
                number(8192),
                number(1),
            ],
        }];
        segments.status = OracleQueryStatus::Executed { rows: 1 };

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("LOB evidence must survive without masking the missing table segment");
        let table = blueprint.tables.values().next().unwrap();
        assert_eq!(table.table_bytes, format::round_bytes(8192));
        assert_eq!(
            table.statistics.as_ref().unwrap().size_visibility,
            "partial"
        );
        assert_eq!(
            blueprint.dataset_scope.as_ref().unwrap().size_completeness,
            "incomplete"
        );
    }

    #[test]
    fn domain_index_keeps_known_bytes_but_never_claims_full_index_scope() {
        let mut capture = fixture();
        append_test_index(
            &mut capture,
            1,
            None,
            "APP",
            "ORDERS_TEXT_IX",
            "APP",
            "ORDERS",
            0,
        );
        capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-indexes")
            .unwrap()
            .rows
            .last_mut()
            .unwrap()
            .values[4] = text("DOMAIN");

        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .expect("domain-index storage gaps must degrade rather than erase known bytes");
        let table = blueprint.tables.values().next().unwrap();
        assert_eq!(table.table_bytes, format::round_bytes(8192));
        assert_eq!(table.statistics.as_ref().unwrap().size_scope, "unknown");
        assert_eq!(
            table.statistics.as_ref().unwrap().size_visibility,
            "partial"
        );
        assert_eq!(
            blueprint.dataset_scope.as_ref().unwrap().size_completeness,
            "incomplete"
        );
    }

    #[test]
    fn positive_sub_bucket_table_and_index_allocations_both_survive_rounding() {
        let mut capture = fixture();
        capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-segments")
            .unwrap()
            .rows[0]
            .values[4] = number(200);
        append_test_index(
            &mut capture,
            1,
            Some(1),
            "APP",
            "ORDERS_SMALL_IX",
            "APP",
            "ORDERS",
            200,
        );
        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        let table = blueprint.tables.values().next().unwrap();
        assert_eq!((table.table_bytes, table.index_bytes), (0, 0));
        assert_eq!(table.segment_state, "mixed-table-and-index");
    }

    #[test]
    fn index_segment_created_after_index_snapshot_is_outside_capture_population() {
        let mut capture = fixture();
        append_second_owner(&mut capture, "OTHER", 21);
        let scope = resolve_oracle_owner_scope(
            OracleOwnerSelection::AllVisible { acknowledged: true },
            "APP",
            ["APP".to_string(), "OTHER".to_string()],
        )
        .unwrap();
        let segments = capture
            .outcomes
            .iter_mut()
            .find(|outcome| {
                outcome.query_id == "oracle-segments" && outcome.owner_ordinal == Some(2)
            })
            .unwrap();
        segments.rows.push(OracleRow {
            values: vec![
                text("OTHER"),
                text("UNMAPPED_OTHER_IX"),
                text("INDEX"),
                text("USERS"),
                number(8192),
                number(1),
            ],
        });
        segments.status = OracleQueryStatus::Executed { rows: 2 };
        let blueprint = map_oracle_basic_capture(
            &capture,
            &scope,
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        let visibility = blueprint
            .tables
            .values()
            .map(|table| table.statistics.as_ref().unwrap().size_visibility.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            visibility.iter().filter(|value| **value == "full").count(),
            2
        );
        assert!(!visibility.contains(&"partial"));
        assert_eq!(
            blueprint.dataset_scope.as_ref().unwrap().size_completeness,
            "complete"
        );
    }

    #[test]
    fn selection_limited_unmatched_index_is_not_assigned_to_local_tables() {
        let mut capture = fixture();
        let segments = capture
            .outcomes
            .iter_mut()
            .find(|outcome| outcome.query_id == "oracle-segments")
            .unwrap();
        segments.rows.push(OracleRow {
            values: vec![
                text("APP"),
                text("INDEX_OVER_UNSELECTED_TABLE"),
                text("INDEX"),
                text("USERS"),
                number(8192),
                number(1),
            ],
        });
        segments.status = OracleQueryStatus::Executed { rows: 2 };
        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one("APP"),
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        assert_eq!(
            blueprint
                .tables
                .values()
                .next()
                .unwrap()
                .statistics
                .as_ref()
                .unwrap()
                .size_visibility,
            "full"
        );
        assert_eq!(
            blueprint.dataset_scope.as_ref().unwrap().size_completeness,
            "complete"
        );
    }

    #[test]
    fn denied_index_catalog_owner_does_not_contaminate_another_owners_sizes() {
        let mut capture = fixture();
        let scope = append_second_owner(&mut capture, "OTHER", 21);
        let indexes = capture
            .outcomes
            .iter_mut()
            .find(|outcome| {
                outcome.query_id == "oracle-indexes" && outcome.owner_ordinal == Some(2)
            })
            .unwrap();
        indexes.rows.clear();
        indexes.status = OracleQueryStatus::Failed {
            class: OracleQueryFailure::PermissionDenied,
        };
        let segments = capture
            .outcomes
            .iter_mut()
            .find(|outcome| {
                outcome.query_id == "oracle-segments" && outcome.owner_ordinal == Some(1)
            })
            .unwrap();
        segments.rows.push(OracleRow {
            values: vec![
                text("APP"),
                text("CROSS_OWNER_INDEX_WITH_DENIED_TABLE_OWNER"),
                text("INDEX"),
                text("USERS"),
                number(8192),
                number(1),
            ],
        });
        segments.status = OracleQueryStatus::Executed { rows: 2 };
        let blueprint = map_oracle_basic_capture(
            &capture,
            &scope,
            OracleVersion::from_components(&[21]).unwrap(),
            &OracleBasicOptions {
                source_kind: "production".to_string(),
                generated_at_pin: None,
                artifact_detail: ArtifactDetail::None,
            },
        )
        .unwrap();
        let visibility = blueprint
            .tables
            .values()
            .map(|table| table.statistics.as_ref().unwrap().size_visibility.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            visibility.iter().filter(|value| **value == "full").count(),
            1
        );
        assert_eq!(
            visibility
                .iter()
                .filter(|value| **value == "partial")
                .count(),
            1
        );
    }

    /// Maps externally captured catalogue rows through the normal validator.
    #[test]
    #[ignore]
    fn map_external_oracle_basic_capture() {
        let fixture_path = std::env::var("DBWARP_BLUEPRINT_ORACLE_TEST_CAPTURE_IN")
            .expect("DBWARP_BLUEPRINT_ORACLE_TEST_CAPTURE_IN must name the JSON input");
        let output_path = std::env::var("DBWARP_BLUEPRINT_ORACLE_TEST_BLUEPRINT_OUT")
            .expect("DBWARP_BLUEPRINT_ORACLE_TEST_BLUEPRINT_OUT must name the TOML output");
        let fixture: ExternalCaptureFixture = serde_json::from_slice(
            &std::fs::read(&fixture_path).expect("read live Oracle fixture"),
        )
        .expect("parse live Oracle fixture");
        let major = *fixture
            .server_version
            .first()
            .expect("live Oracle fixture has a server major version");
        let query_by_id = CATALOG_QUERIES
            .iter()
            .map(|query| (query.query_id, query))
            .collect::<BTreeMap<_, _>>();
        let mut outcomes = Vec::new();
        let mut rows_consumed = 0_u64;
        let mut bytes_consumed = 0_u64;
        for input in fixture.outcomes {
            let query = query_by_id
                .get(input.query_id.as_str())
                .unwrap_or_else(|| panic!("unknown live Oracle query id {}", input.query_id));
            let expected_width = query.columns.len()
                + query
                    .versioned_columns
                    .iter()
                    .filter(|(minimum, _)| major >= *minimum)
                    .count();
            let mapped_rows = input
                .rows
                .into_iter()
                .map(|values| {
                    assert_eq!(
                        values.len(),
                        expected_width,
                        "{} live row width",
                        query.query_id
                    );
                    OracleRow {
                        values: values
                            .into_iter()
                            .map(|value| match value {
                                Some(value) => {
                                    bytes_consumed = bytes_consumed
                                        .checked_add(value.len() as u64)
                                        .expect("live Oracle fixture byte count overflow");
                                    OracleValue::Text(value)
                                }
                                None => OracleValue::Null,
                            })
                            .collect(),
                    }
                })
                .collect::<Vec<_>>();
            rows_consumed = rows_consumed
                .checked_add(mapped_rows.len() as u64)
                .expect("live Oracle fixture row count overflow");
            outcomes.push(OracleQueryOutcome {
                query_id: query.query_id,
                view: query.view,
                owner_ordinal: input.owner_ordinal,
                status: OracleQueryStatus::Executed {
                    rows: mapped_rows.len() as u64,
                },
                rows: mapped_rows,
            });
        }
        for query in queries_for_tier(CatalogTier::Basic).filter(|query| query.applies_to(major)) {
            let expected = if query.owner_column.is_some() {
                fixture.owners.len()
            } else {
                1
            };
            assert_eq!(
                outcomes
                    .iter()
                    .filter(|outcome| outcome.query_id == query.query_id)
                    .count(),
                expected,
                "live Oracle fixture outcome count for {}",
                query.query_id
            );
        }
        let catalogs_read = outcomes
            .iter()
            .map(|outcome| outcome.view)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let capture = OracleCatalogCapture {
            outcomes,
            catalogs_read,
            catalogs_unreadable: Vec::new(),
            abort: None,
            session_discarded: false,
            unconfirmed_server_work: false,
            rows_consumed,
            bytes_consumed,
            lob_bytes_consumed: 0,
            elapsed_ms: 0,
            client_version_attestation: None,
        };
        let scope = resolve_oracle_owner_scope(
            OracleOwnerSelection::Selected(fixture.owners.clone()),
            fixture
                .owners
                .first()
                .expect("live Oracle fixture has an owner"),
            fixture.owners.clone(),
        )
        .expect("resolve live Oracle owner scope");
        let blueprint = map_oracle_basic_capture(
            &capture,
            &scope,
            OracleVersion::from_components(&fixture.server_version)
                .expect("valid live Oracle server version"),
            &OracleBasicOptions {
                source_kind: "test".to_string(),
                generated_at_pin: Some("2026-09-13T00:00:00Z".to_string()),
                artifact_detail: ArtifactDetail::Summary,
            },
        )
        .expect("map live Oracle Basic capture");
        assert_eq!(blueprint.tables.len(), fixture.expected.table_count);
        assert_eq!(
            blueprint
                .tables
                .values()
                .map(|table| table.cols.len())
                .sum::<usize>(),
            fixture.expected.column_count
        );
        assert_eq!(
            blueprint
                .tables
                .values()
                .filter(|table| table.partitioning != "none")
                .count(),
            fixture.expected.partitioned_table_count
        );
        assert_eq!(
            blueprint
                .tables
                .values()
                .filter(|table| table.object_kind == "external-table")
                .count(),
            fixture.expected.external_table_count
        );
        let serialized = crate::format::emit_toml(&blueprint).expect("serialize live Blueprint");
        let leaked = first_forbidden_substring(&serialized, &fixture.forbidden_strings);
        assert!(
            leaked.is_none(),
            "native synthetic identifier leaked into mapped Blueprint: {leaked:?}"
        );
        std::fs::write(output_path, serialized).expect("write mapped live Oracle Blueprint");
    }
}
