//! The normalized Oracle catalog query contract, tier by tier.
//!
//! Each entry fixes a query id, tier, version range, output fields, owner
//! scoping and degradation class. Byte, row, time and corruption limits belong
//! to the session layer that executes this registry.
//!
//! Row sampling is deliberately absent because it reads customer tables rather
//! than the dictionary. Only Minimum and Basic grant scripts are provided.

#![allow(dead_code)]

/// Catalog selection uses the exact negotiated tier vocabulary so admission
/// and execution cannot drift onto separate representations.
pub use crate::oracle_provider::OracleCaptureTier as CatalogTier;

fn catalog_tier_label(tier: CatalogTier) -> &'static str {
    match tier {
        CatalogTier::Basic => "basic",
        CatalogTier::Standard => "standard",
        CatalogTier::Enhanced => "enhanced",
    }
}

/// How a query's failure degrades the capture:
/// The named table/object/column floor is evaluated by the mapper after all
/// owners have had a chance to run; this marker identifies those families.
/// Refinements always degrade independently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Degradation {
    Fatal,
    DegradeFamily,
}

/// Why a view can be absent without the release being at fault.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    /// Present on every supported release.
    Always,
    /// Introduced at this major release.
    FromMajor(u16),
    /// Owned by a separately-installed option; absence must be attributed
    /// through DBA_REGISTRY, never reported as a version gap.
    OptionComponent(&'static str),
}

#[derive(Debug, Clone, Copy)]
pub struct CatalogQuery {
    pub query_id: &'static str,
    pub tier: CatalogTier,
    pub view: &'static str,
    /// Projected on every release.
    pub columns: &'static [&'static str],
    /// (first major release, column) - projected only from that release.
    pub versioned_columns: &'static [(u16, &'static str)],
    /// Dictionary column the owner scope binds against; None for unscoped
    /// database-wide views.
    pub owner_column: Option<&'static str>,
    /// Optional hard-coded server-side restriction. It is part of the query
    /// registry, never operator input; use it to avoid fetching unrelated
    /// free-text configuration rows into the collector process.
    pub fixed_predicate: Option<&'static str>,
    pub presence: Presence,
    pub degradation: Degradation,
    /// True when the projection carries customer definition text. The session
    /// boundary consumes this flag so adding another definition query cannot
    /// silently bypass the negotiated detail level.
    pub definition_bearing: bool,
}

impl CatalogQuery {
    /// Whether this view can exist on the given major release at all.
    pub fn applies_to(&self, major: u16) -> bool {
        match self.presence {
            Presence::FromMajor(minimum) => major >= minimum,
            _ => true,
        }
    }

    /// The exact SQL the provider issues. The owner arrives as a bind
    /// variable, never interpolated: a schema name is operator input.
    pub fn render_sql(&self, major: u16) -> String {
        let version = crate::oracle_provider::OracleVersion::from_components(&[major])
            .expect("catalog major release must be nonzero");
        self.render_sql_for_version(version)
    }

    pub fn render_sql_for_version(&self, version: crate::oracle_provider::OracleVersion) -> String {
        let columns = self.projected_columns_for_version(version);
        let projection = columns.join(", ");
        let mut predicates = Vec::new();
        if let Some(owner) = self.owner_column {
            predicates.push(format!("{owner} = :owner"));
        }
        if let Some(predicate) = self.fixed_predicate {
            predicates.push(predicate.to_string());
        }
        let where_clause = if predicates.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", predicates.join(" AND "))
        };
        format!(
            "SELECT {projection} FROM SYS.{view}{where_clause}",
            view = self.view
        )
    }

    /// Columns in the exact order a row-producing adapter must return them.
    /// The mapper uses the same list to reject malformed or cross-owner rows
    /// before any native identifiers can influence the Blueprint.
    pub fn projected_columns(&self, major: u16) -> Vec<&'static str> {
        let version = crate::oracle_provider::OracleVersion::from_components(&[major])
            .expect("catalog major release must be nonzero");
        self.projected_columns_for_version(version)
    }

    pub fn projected_columns_for_version(
        &self,
        version: crate::oracle_provider::OracleVersion,
    ) -> Vec<&'static str> {
        let major = version.components()[0];
        let mut columns: Vec<&str> = self
            .columns
            .iter()
            .map(|column| {
                if self.query_id == "oracle-constraints"
                    && !version.is_at_least(12, 2)
                    && column.ends_with(" AS is_not_null_constraint")
                {
                    // SEARCH_CONDITION_VC is absent on 11g/12.1. Preserve the
                    // family but make its exact CHECK count unavailable.
                    "CAST(NULL AS VARCHAR2(1)) AS is_not_null_constraint"
                } else if matches!(self.query_id, "oracle-tables" | "oracle-object-tables")
                    && *column == "external"
                    && !version.is_at_least(12, 2)
                {
                    // DBA_TABLES.EXTERNAL was added in 12.2. Keep the row
                    // shape stable on 12.1 and obtain the authoritative
                    // classification from the separately granted
                    // DBA_EXTERNAL_TABLES family.
                    "CAST('NO' AS VARCHAR2(3)) AS external"
                } else {
                    *column
                }
            })
            .collect();
        columns.extend(
            self.versioned_columns
                .iter()
                .filter(|(minimum, _)| major >= *minimum)
                .map(|(_, column)| *column),
        );
        columns
    }

    /// Stable output labels for the projected expressions. Most catalog
    /// fields are direct columns; the small number of privacy-preserving
    /// server-side classifications use an explicit `AS label` projection so
    /// raw dictionary text never crosses the provider boundary.
    pub fn output_columns(&self, major: u16) -> Vec<&'static str> {
        let version = crate::oracle_provider::OracleVersion::from_components(&[major])
            .expect("catalog major release must be nonzero");
        self.output_columns_for_version(version)
    }

    pub fn output_columns_for_version(
        &self,
        version: crate::oracle_provider::OracleVersion,
    ) -> Vec<&'static str> {
        self.projected_columns_for_version(version)
            .into_iter()
            .map(|projection| {
                projection
                    .rsplit_once(" AS ")
                    .map_or(projection, |(_, alias)| alias)
            })
            .collect()
    }
}

use CatalogTier::{Basic, Enhanced, Standard};
use Degradation::{DegradeFamily, Fatal};
use Presence::{Always, FromMajor, OptionComponent};

macro_rules! query {
    ($id:literal, $tier:expr, $view:literal, $cols:expr) => {
        query!(
            $id,
            $tier,
            $view,
            $cols,
            &[],
            Some("owner"),
            Always,
            DegradeFamily,
            false,
            None
        )
    };
    ($id:literal, $tier:expr, $view:literal, $cols:expr, $vcols:expr, $owner:expr, $presence:expr, $degradation:expr) => {
        query!(
            $id,
            $tier,
            $view,
            $cols,
            $vcols,
            $owner,
            $presence,
            $degradation,
            false,
            None
        )
    };
    ($id:literal, $tier:expr, $view:literal, $cols:expr, $vcols:expr, $owner:expr, $presence:expr, $degradation:expr, $definition_bearing:expr) => {
        query!(
            $id,
            $tier,
            $view,
            $cols,
            $vcols,
            $owner,
            $presence,
            $degradation,
            $definition_bearing,
            None
        )
    };
    ($id:literal, $tier:expr, $view:literal, $cols:expr, $vcols:expr, $owner:expr, $presence:expr, $degradation:expr, $definition_bearing:expr, $fixed_predicate:expr) => {
        CatalogQuery {
            query_id: $id,
            tier: $tier,
            view: $view,
            columns: $cols,
            versioned_columns: $vcols,
            owner_column: $owner,
            fixed_predicate: $fixed_predicate,
            presence: $presence,
            degradation: $degradation,
            definition_bearing: $definition_bearing,
        }
    };
}

pub const CATALOG_QUERIES: &[CatalogQuery] = &[
    // --- basic: structure, estimates, sizes, freshness, identity ----------
    query!(
        "oracle-users",
        Basic,
        "ALL_USERS",
        &["username"],
        &[(12, "oracle_maintained")],
        Some("username"),
        Always,
        DegradeFamily
    ),
    query!(
        "oracle-objects",
        Basic,
        "DBA_OBJECTS",
        &[
            "owner",
            "object_name",
            "object_type",
            "status",
            "temporary",
            "generated",
            "secondary"
        ],
        &[(12, "editionable")],
        Some("owner"),
        Always,
        Fatal
    ),
    query!(
        "oracle-tables",
        Basic,
        "DBA_TABLES",
        &[
            "owner",
            "table_name",
            "tablespace_name",
            "num_rows",
            "blocks",
            "avg_row_len",
            "last_analyzed",
            "partitioned",
            "temporary",
            "iot_type",
            "nested",
            "compression",
            "iot_name",
            "secondary",
            "external",
            "cluster_name",
            "dropped",
            "segment_created"
        ],
        &[],
        Some("owner"),
        Always,
        Fatal
    ),
    query!(
        "oracle-object-tables",
        Basic,
        "DBA_OBJECT_TABLES",
        &[
            "owner",
            "table_name",
            "tablespace_name",
            "num_rows",
            "blocks",
            "avg_row_len",
            "last_analyzed",
            "partitioned",
            "temporary",
            "iot_type",
            "nested",
            "compression",
            "iot_name",
            "secondary",
            "external",
            "cluster_name",
            "dropped",
            "segment_created"
        ],
        &[],
        Some("owner"),
        Always,
        DegradeFamily
    ),
    query!(
        "oracle-external-tables",
        Basic,
        "DBA_EXTERNAL_TABLES",
        &["owner", "table_name"],
        &[],
        Some("owner"),
        Always,
        DegradeFamily
    ),
    query!(
        "oracle-nested-tables",
        Basic,
        "DBA_NESTED_TABLES",
        &[
            "owner",
            "table_name",
            "parent_table_name",
            "parent_table_column"
        ],
        &[],
        Some("owner"),
        Always,
        DegradeFamily
    ),
    query!(
        "oracle-mview-logs",
        Basic,
        "DBA_MVIEW_LOGS",
        &["log_owner", "log_table", "master", "primary_key"],
        &[],
        Some("log_owner"),
        Always,
        DegradeFamily
    ),
    query!(
        "oracle-queue-tables",
        Basic,
        "DBA_QUEUE_TABLES",
        &["owner", "queue_table"],
        &[],
        Some("owner"),
        Always,
        DegradeFamily
    ),
    query!(
        "oracle-tab-columns",
        Basic,
        "DBA_TAB_COLUMNS",
        &[
            "owner",
            "table_name",
            "column_name",
            "column_id",
            "data_type",
            "data_length",
            "data_precision",
            "data_scale",
            "nullable",
            "char_length",
            "char_used",
            "default_length"
        ],
        &[(12, "default_on_null")],
        Some("owner"),
        Always,
        Fatal
    ),
    query!(
        "oracle-tab-cols-hidden",
        Basic,
        "DBA_TAB_COLS",
        &[
            "owner",
            "table_name",
            "column_name",
            "hidden_column",
            "virtual_column",
            "internal_column_id"
        ],
        &[(12, "user_generated")],
        Some("owner"),
        Always,
        DegradeFamily
    ),
    query!(
        "oracle-identity-columns",
        Basic,
        "DBA_TAB_IDENTITY_COLS",
        &[
            "owner",
            "table_name",
            "column_name",
            "generation_type",
            "sequence_name"
        ],
        &[],
        Some("owner"),
        FromMajor(12),
        DegradeFamily
    ),
    query!(
        "oracle-indexes",
        Basic,
        "DBA_INDEXES",
        &[
            "owner",
            "index_name",
            "table_owner",
            "table_name",
            "index_type",
            "uniqueness",
            "distinct_keys",
            "clustering_factor",
            "leaf_blocks",
            "num_rows",
            "last_analyzed",
            "partitioned",
            "status",
            "generated",
            "secondary",
            "visibility",
            "join_index",
            "segment_created"
        ],
        &[],
        Some("table_owner"),
        Always,
        DegradeFamily
    ),
    query!(
        "oracle-index-columns",
        Basic,
        "DBA_IND_COLUMNS",
        &[
            "index_owner",
            "index_name",
            "table_name",
            "column_name",
            "column_position",
            "descend",
            "table_owner"
        ],
        &[],
        Some("table_owner"),
        Always,
        DegradeFamily
    ),
    query!(
        "oracle-index-expressions",
        Basic,
        "DBA_IND_EXPRESSIONS",
        &["index_owner", "index_name", "column_position", "table_owner"],
        &[],
        Some("table_owner"),
        Always,
        DegradeFamily
    ),
    query!(
        "oracle-constraints",
        Basic,
        "DBA_CONSTRAINTS",
        &[
            "owner",
            "constraint_name",
            "constraint_type",
            "table_name",
            "r_owner",
            "r_constraint_name",
            "delete_rule",
            "status",
            "deferrable",
            "validated",
            "deferred",
            "index_owner",
            "index_name",
            "generated",
            "CASE WHEN constraint_type = 'C' AND REGEXP_LIKE(search_condition_vc, '^\"([^\"]|\"\")+\" IS NOT NULL$') THEN 'Y' ELSE 'N' END AS is_not_null_constraint"
        ]
    ),
    query!(
        "oracle-constraint-columns",
        Basic,
        "DBA_CONS_COLUMNS",
        &[
            "owner",
            "constraint_name",
            "table_name",
            "column_name",
            "position"
        ]
    ),
    query!(
        "oracle-part-tables",
        Basic,
        "DBA_PART_TABLES",
        &[
            "owner",
            "table_name",
            "partitioning_type",
            "subpartitioning_type",
            "partition_count",
            "interval"
        ]
    ),
    query!(
        "oracle-tab-partitions",
        Basic,
        "DBA_TAB_PARTITIONS",
        &[
            "table_owner",
            "table_name",
            "partition_name",
            "partition_position",
            "num_rows",
            "blocks"
        ],
        &[],
        Some("table_owner"),
        Always,
        DegradeFamily
    ),
    query!(
        "oracle-tab-subpartitions",
        Basic,
        "DBA_TAB_SUBPARTITIONS",
        &[
            "table_owner",
            "table_name",
            "partition_name",
            "subpartition_name",
            "subpartition_position",
            "num_rows",
            "blocks"
        ],
        &[],
        Some("table_owner"),
        Always,
        DegradeFamily
    ),
    query!(
        "oracle-part-key-columns",
        Basic,
        "DBA_PART_KEY_COLUMNS",
        &[
            "owner",
            "name",
            "object_type",
            "column_name",
            "column_position"
        ]
    ),
    query!(
        "oracle-segments",
        Basic,
        "DBA_SEGMENTS",
        &[
            "owner",
            "segment_name",
            "segment_type",
            "tablespace_name",
            "bytes",
            "blocks"
        ]
    ),
    query!(
        "oracle-lobs",
        Basic,
        "DBA_LOBS",
        &[
            "owner",
            "table_name",
            "column_name",
            "segment_name",
            "index_name",
            "chunk",
            "in_row",
            "securefile",
            "compression",
            "deduplication",
            "encrypt"
        ]
    ),
    query!(
        "oracle-tab-statistics",
        Basic,
        "DBA_TAB_STATISTICS",
        &[
            "owner",
            "table_name",
            "partition_name",
            "num_rows",
            "blocks",
            "avg_row_len",
            "last_analyzed",
            "stale_stats",
            "object_type",
            "sample_size",
            "global_stats",
            "user_stats",
            "stattype_locked"
        ],
        &[(12, "scope")],
        Some("owner"),
        Always,
        DegradeFamily
    ),
    query!(
        "oracle-nls",
        Basic,
        "NLS_DATABASE_PARAMETERS",
        &["parameter", "value"],
        &[],
        None,
        Always,
        DegradeFamily
    ),
    query!(
        "oracle-version",
        Basic,
        "PRODUCT_COMPONENT_VERSION",
        &["product", "version"],
        &[(18, "version_full")],
        None,
        Always,
        DegradeFamily
    ),
    query!(
        "oracle-database",
        Basic,
        "V_$DATABASE",
        &["platform_name", "database_role", "open_mode"],
        &[(12, "cdb")],
        None,
        Always,
        DegradeFamily
    ),
    query!(
        "oracle-instance",
        Basic,
        "V_$INSTANCE",
        &["version", "status", "parallel"],
        &[],
        None,
        Always,
        DegradeFamily
    ),
    query!(
        "oracle-containers",
        Basic,
        "V_$CONTAINERS",
        &["con_id", "open_mode", "restricted"],
        &[],
        None,
        FromMajor(12),
        DegradeFamily
    ),
    query!(
        "oracle-capacity-parameters",
        Basic,
        "V_$PARAMETER",
        &["name", "value", "isdefault"],
        &[],
        None,
        Always,
        DegradeFamily,
        false,
        Some("LOWER(name) IN ('cpu_count','memory_target','sga_target','sga_max_size')")
    ),
    // --- standard: column statistics -------------------------------------
    query!(
        "oracle-column-statistics",
        Standard,
        "DBA_TAB_COL_STATISTICS",
        &[
            "owner",
            "table_name",
            "column_name",
            "num_distinct",
            "density",
            "num_nulls",
            "num_buckets",
            "histogram",
            "sample_size",
            "last_analyzed",
            "avg_col_len"
        ]
    ),
    query!(
        "oracle-histograms",
        Standard,
        "DBA_TAB_HISTOGRAMS",
        &[
            "owner",
            "table_name",
            "column_name",
            "endpoint_number",
            "endpoint_value"
        ]
    ),
    // --- enhanced: definitions, graph, non-table inventory ----------------
    // Read first: option-owned views below fail with ORA-00942 where their
    // component is not installed, and this is what lets the capture - the
    // offline pack included - attribute that to the component rather than
    // report a version gap or a failure.
    query!(
        "oracle-registry",
        Enhanced,
        "DBA_REGISTRY",
        &["comp_id", "status", "version"],
        &[],
        None,
        Always,
        DegradeFamily
    ),
    query!(
        "oracle-source",
        Enhanced,
        "DBA_SOURCE",
        &["owner", "name", "type", "line", "text"],
        &[],
        Some("owner"),
        Always,
        DegradeFamily,
        true
    ),
    query!(
        "oracle-views",
        Enhanced,
        "DBA_VIEWS",
        &["owner", "view_name", "text_length", "text"],
        &[(12, "text_vc")],
        Some("owner"),
        Always,
        DegradeFamily,
        true
    ),
    query!(
        "oracle-mviews",
        Enhanced,
        "DBA_MVIEWS",
        &[
            "owner",
            "mview_name",
            "container_name",
            "query",
            "rewrite_enabled",
            "refresh_mode",
            "refresh_method",
            "last_refresh_type",
            "last_refresh_date"
        ],
        &[],
        Some("owner"),
        Always,
        DegradeFamily,
        true
    ),
    query!(
        "oracle-triggers",
        Enhanced,
        "DBA_TRIGGERS",
        &[
            "owner",
            "trigger_name",
            "trigger_type",
            "triggering_event",
            "table_owner",
            "table_name",
            "status",
            "trigger_body"
        ],
        &[],
        Some("owner"),
        Always,
        DegradeFamily,
        true
    ),
    query!(
        "oracle-procedures",
        Enhanced,
        "DBA_PROCEDURES",
        &[
            "owner",
            "object_name",
            "procedure_name",
            "object_type",
            "aggregate",
            "pipelined",
            "parallel",
            "deterministic"
        ],
        &[(12, "authid")],
        Some("owner"),
        Always,
        DegradeFamily
    ),
    query!(
        "oracle-dependencies",
        Enhanced,
        "DBA_DEPENDENCIES",
        &[
            "owner",
            "name",
            "type",
            "referenced_owner",
            "referenced_name",
            "referenced_type",
            "dependency_type"
        ]
    ),
    query!(
        "oracle-types",
        Enhanced,
        "DBA_TYPES",
        &[
            "owner",
            "type_name",
            "typecode",
            "attributes",
            "methods",
            "incomplete"
        ]
    ),
    query!(
        "oracle-sequences",
        Enhanced,
        "DBA_SEQUENCES",
        &[
            "sequence_owner",
            "sequence_name",
            "min_value",
            "max_value",
            "increment_by",
            "cycle_flag",
            "cache_size",
            "last_number"
        ],
        &[],
        Some("sequence_owner"),
        Always,
        DegradeFamily
    ),
    query!(
        "oracle-synonyms",
        Enhanced,
        "DBA_SYNONYMS",
        &[
            "owner",
            "synonym_name",
            "table_owner",
            "table_name",
            "db_link"
        ]
    ),
    query!(
        "oracle-db-links",
        Enhanced,
        "DBA_DB_LINKS",
        &["owner", "db_link", "username", "host", "created"]
    ),
    query!(
        "oracle-directories",
        Enhanced,
        "DBA_DIRECTORIES",
        &["owner", "directory_name", "directory_path"]
    ),
    query!(
        "oracle-libraries",
        Enhanced,
        "DBA_LIBRARIES",
        &["owner", "library_name", "file_spec", "dynamic", "status"]
    ),
    query!(
        "oracle-java-classes",
        Enhanced,
        "DBA_JAVA_CLASSES",
        &["owner", "name", "accessibility", "kind"],
        &[],
        Some("owner"),
        OptionComponent("JAVAVM"),
        DegradeFamily
    ),
    query!(
        "oracle-scheduler-jobs",
        Enhanced,
        "DBA_SCHEDULER_JOBS",
        &[
            "owner",
            "job_name",
            "job_type",
            "enabled",
            "state",
            "repeat_interval"
        ]
    ),
    query!(
        "oracle-scheduler-programs",
        Enhanced,
        "DBA_SCHEDULER_PROGRAMS",
        &["owner", "program_name", "program_type", "enabled"]
    ),
    query!(
        "oracle-scheduler-schedules",
        Enhanced,
        "DBA_SCHEDULER_SCHEDULES",
        &["owner", "schedule_name", "schedule_type", "repeat_interval"]
    ),
    query!(
        "oracle-queues",
        Enhanced,
        "DBA_QUEUES",
        &[
            "owner",
            "name",
            "queue_table",
            "queue_type",
            "enqueue_enabled",
            "dequeue_enabled"
        ]
    ),
    query!(
        "oracle-editions",
        Enhanced,
        "DBA_EDITIONS",
        &["edition_name", "parent_edition_name", "usable"],
        &[],
        None,
        Always,
        DegradeFamily
    ),
    query!(
        "oracle-indextypes",
        Enhanced,
        "DBA_INDEXTYPES",
        &[
            "owner",
            "indextype_name",
            "implementation_schema",
            "implementation_name"
        ]
    ),
    query!(
        "oracle-operators",
        Enhanced,
        "DBA_OPERATORS",
        &["owner", "operator_name", "number_of_binds"]
    ),
    query!(
        "oracle-xml-schemas",
        Enhanced,
        "DBA_XML_SCHEMAS",
        &["owner", "schema_url", "local"],
        &[],
        Some("owner"),
        OptionComponent("XDB"),
        DegradeFamily
    ),
    query!(
        "oracle-dimensions",
        Enhanced,
        "DBA_DIMENSIONS",
        &["owner", "dimension_name", "invalid"]
    ),
    query!(
        "oracle-clusters",
        Enhanced,
        "DBA_CLUSTERS",
        &["owner", "cluster_name", "tablespace_name"]
    ),
    query!(
        "oracle-policies",
        Enhanced,
        "DBA_POLICIES",
        &[
            "object_owner",
            "object_name",
            "policy_name",
            "pf_owner",
            "enable"
        ],
        &[],
        Some("object_owner"),
        Always,
        DegradeFamily
    ),
];

/// A tier runs its own queries plus every lower tier's.
pub fn queries_for_tier(tier: CatalogTier) -> impl Iterator<Item = &'static CatalogQuery> {
    CATALOG_QUERIES
        .iter()
        .filter(move |query| query.tier <= tier)
}

/// Diagnostic SQL*Plus rendering of the catalogue registry.
/// It is not the offline capture format: it emits separate CSV files and has
/// no framing manifest, truncation proof, or stream checksum. The supported
/// offline path does not accept this output.
///
/// `SET MARKUP CSV` requires a SQL*Plus 12.2+ client. Rendering this text does
/// not validate a client/server pairing, grant set, version, option, or tier.
pub fn render_offline_pack(
    tier: CatalogTier,
    major: u16,
    owner: &str,
) -> Result<String, OracleProjectionPackFailure> {
    if owner.is_empty() || owner.len() > 128 || owner.chars().any(char::is_control) {
        return Err(OracleProjectionPackFailure::InvalidOwner);
    }
    let owner_hex = hex::encode(owner.as_bytes());
    let mut out = String::new();
    out.push_str(&format!(
        "-- DBWarp Blueprint Oracle inspection projection pack\n\
         -- NOT A SUPPORTED OFFLINE CAPTURE FORMAT\n\
         -- tier: {tier}, rendered for major release {major}\n\
         -- Inspection output only; not accepted as capture input.\n\
         VARIABLE bp_owner VARCHAR2(128)\n\
         BEGIN\n\
           :bp_owner := UTL_I18N.RAW_TO_CHAR(HEXTORAW('{owner_hex}'), 'AL32UTF8');\n\
         END;\n\
         /\n\
         SET HEADING ON PAGESIZE 0 LINESIZE 32767 TRIMSPOOL ON FEEDBACK OFF\n\
         SET MARKUP CSV ON QUOTE ON\n",
        tier = catalog_tier_label(tier),
    ));
    for query in queries_for_tier(tier) {
        if !query.applies_to(major) {
            out.push_str(&format!(
                "-- {id}: not applicable before release {major}\n",
                id = query.query_id,
            ));
            continue;
        }
        let sql = query.render_sql(major);
        if let Presence::OptionComponent(component) = query.presence {
            out.push_str(&format!(
                "-- {id}: owned by the {component} option; ORA-00942 here is\n\
                 -- expected where oracle-registry.csv lacks that component\n",
                id = query.query_id,
            ));
        }
        out.push_str(&format!(
            "SPOOL {id}.csv\n{sql};\nSPOOL OFF\n",
            id = query.query_id,
        ));
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OracleProjectionPackFailure {
    InvalidOwner,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn query_ids_are_unique_and_counts_match_the_registry() {
        let ids: BTreeSet<_> = CATALOG_QUERIES.iter().map(|q| q.query_id).collect();
        assert_eq!(ids.len(), CATALOG_QUERIES.len(), "duplicate query id");
        // The machine-readable registry test below provides the exact guard;
        // these counts also catch accidental tier movement.
        assert_eq!(queries_for_tier(CatalogTier::Basic).count(), 29);
        assert_eq!(queries_for_tier(CatalogTier::Standard).count(), 31);
        assert_eq!(queries_for_tier(CatalogTier::Enhanced).count(), 56);
    }

    #[test]
    fn owner_scoping_uses_a_bind_variable_never_interpolation() {
        for query in CATALOG_QUERIES {
            let sql = query.render_sql(23);
            match query.owner_column {
                Some(owner) => assert!(
                    sql.ends_with(&format!("WHERE {owner} = :owner")),
                    "{}: owner must bind",
                    query.query_id
                ),
                None => assert!(
                    !sql.contains(":owner"),
                    "{}: unscoped query must not bind an owner",
                    query.query_id
                ),
            }
        }
    }

    #[test]
    fn derived_catalog_projection_exposes_only_its_closed_output_label() {
        let query = CATALOG_QUERIES
            .iter()
            .find(|query| query.query_id == "oracle-constraints")
            .unwrap();
        let sql = query.render_sql(21);
        assert!(sql.contains("REGEXP_LIKE(search_condition_vc"));
        assert!(!sql.contains("generated = 'GENERATED NAME'"));
        assert_eq!(
            query.output_columns(21).last().copied(),
            Some("is_not_null_constraint")
        );
        assert!(!query
            .output_columns(21)
            .iter()
            .any(|column| column.contains(' ')));
    }

    #[test]
    fn pre_12_2_constraint_projection_preserves_family_without_missing_column() {
        let query = CATALOG_QUERIES
            .iter()
            .find(|query| query.query_id == "oracle-constraints")
            .unwrap();
        let sql = query.render_sql(12);
        assert!(!sql.contains("search_condition_vc"));
        assert!(sql.contains("CAST(NULL AS VARCHAR2(1)) AS is_not_null_constraint"));
        assert_eq!(
            query.output_columns(12).last().copied(),
            Some("is_not_null_constraint")
        );
        let release_12_2 = crate::oracle_provider::OracleVersion::from_components(&[12, 2, 0, 1])
            .expect("12.2 version");
        assert!(query
            .render_sql_for_version(release_12_2)
            .contains("search_condition_vc"));
        let release_12_1 = crate::oracle_provider::OracleVersion::from_components(&[12, 1, 0, 2])
            .expect("12.1 version");
        assert!(!query
            .render_sql_for_version(release_12_1)
            .contains("search_condition_vc"));
    }

    #[test]
    fn pre_12_2_table_projection_preserves_external_flag_position() {
        let query = CATALOG_QUERIES
            .iter()
            .find(|query| query.query_id == "oracle-tables")
            .unwrap();
        let release_12_1 = crate::oracle_provider::OracleVersion::from_components(&[12, 1, 0, 2])
            .expect("12.1 version");
        let release_12_2 = crate::oracle_provider::OracleVersion::from_components(&[12, 2, 0, 1])
            .expect("12.2 version");

        assert!(query
            .render_sql_for_version(release_12_1)
            .contains("CAST('NO' AS VARCHAR2(3)) AS external"));
        assert!(query
            .render_sql_for_version(release_12_2)
            .contains(", external, cluster_name"));
        assert_eq!(
            query.output_columns_for_version(release_12_1),
            query.output_columns_for_version(release_12_2)
        );
        assert_eq!(
            query.output_columns_for_version(release_12_1)[14],
            "external"
        );
    }

    #[test]
    fn versioned_columns_appear_only_from_their_release() {
        let tables = CATALOG_QUERIES
            .iter()
            .find(|q| q.query_id == "oracle-database")
            .expect("oracle-database");
        assert!(!tables.render_sql(11).contains("cdb"));
        assert!(tables.render_sql(12).contains("cdb"));
        let identity = CATALOG_QUERIES
            .iter()
            .find(|q| q.query_id == "oracle-identity-columns")
            .expect("identity");
        assert!(!identity.applies_to(11));
        assert!(identity.applies_to(12));
    }

    #[test]
    fn only_the_named_blueprint_floor_families_are_fatal() {
        for query in CATALOG_QUERIES {
            let fatal = matches!(query.degradation, Degradation::Fatal);
            let core = matches!(
                query.query_id,
                "oracle-objects" | "oracle-tables" | "oracle-tab-columns"
            );
            assert_eq!(
                fatal, core,
                "{}: fatality must follow the capture contract",
                query.query_id
            );
        }
    }

    #[test]
    fn definition_bearing_queries_are_declared_once_in_catalog_metadata() {
        let definition_queries = CATALOG_QUERIES
            .iter()
            .filter(|query| query.definition_bearing)
            .map(|query| query.query_id)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            definition_queries,
            BTreeSet::from([
                "oracle-mviews",
                "oracle-source",
                "oracle-triggers",
                "oracle-views",
            ])
        );
        assert!(CATALOG_QUERIES
            .iter()
            .filter(|query| query.definition_bearing)
            .all(|query| query.tier == CatalogTier::Enhanced));
    }

    /// Writes the exact registry as JSON for an external database test.
    #[test]
    #[ignore]
    fn export_query_registry_json() {
        let output = std::env::var("DBWARP_BLUEPRINT_ORACLE_TEST_QUERY_REGISTRY_OUT")
            .expect("DBWARP_BLUEPRINT_ORACLE_TEST_QUERY_REGISTRY_OUT must be set");
        let contract = CATALOG_QUERIES
            .iter()
            .map(|query| {
                let (presence, option_component, min_major) = match query.presence {
                    Presence::Always => ("always", None, 0),
                    Presence::FromMajor(major) => ("from-major", None, major),
                    Presence::OptionComponent(component) => {
                        ("option-component", Some(component), 0)
                    }
                };
                serde_json::json!({
                    "query_id": query.query_id,
                    "tier": catalog_tier_label(query.tier),
                    "view": query.view,
                    "columns": query.columns,
                    "versioned_columns": query.versioned_columns,
                    "owner_column": query.owner_column,
                    "fixed_predicate": query.fixed_predicate,
                    "presence": presence,
                    "option_component": option_component,
                    "min_major": min_major,
                    "degradation": match query.degradation {
                        Degradation::Fatal => "fatal",
                        Degradation::DegradeFamily => "degrade-family",
                    },
                })
            })
            .collect::<Vec<_>>();
        std::fs::write(
            output,
            serde_json::to_vec_pretty(&contract).expect("serialize Oracle query contract"),
        )
        .expect("write Oracle query contract");
    }

    #[test]
    fn offline_pack_renders_owner_definition_and_skips_inapplicable() {
        let pack = render_offline_pack(CatalogTier::Basic, 11, "Mixed Owner").unwrap();
        assert!(pack.contains("VARIABLE bp_owner"));
        assert!(pack.contains(&hex::encode("Mixed Owner".as_bytes())));
        assert!(!pack.contains("Mixed Owner"));
        assert!(pack.contains("NOT A SUPPORTED OFFLINE CAPTURE FORMAT"));
        assert!(pack.contains("SPOOL oracle-tables.csv"));
        assert!(pack.contains("oracle-identity-columns: not applicable"));
        assert!(pack.contains("WHERE owner = :owner"));
        let modern = render_offline_pack(CatalogTier::Enhanced, 23, "APP_SCHEMA").unwrap();
        assert!(modern.contains("SPOOL oracle-java-classes.csv"));
        assert!(modern.contains("text_vc"));
        assert_eq!(
            render_offline_pack(CatalogTier::Basic, 21, "BAD\nOWNER"),
            Err(OracleProjectionPackFailure::InvalidOwner)
        );
    }
}
