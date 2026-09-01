//! Bounded, name-free non-table artifact inventory and language-feature census.
//!
//! Engine readers construct `RawArtifact` values containing source identities
//! and, only for `analyzed`, transient definitions. This module converts them
//! to closed-vocabulary anonymous records. Raw identities and SQL text never
//! enter the serialized Blueprint.

use std::collections::{BTreeMap, BTreeSet};

use clap::ValueEnum;
use zeroize::{Zeroize, Zeroizing};

use crate::audit::AuditLog;
use crate::format::{
    ArtifactInventory, BlueprintArtifact, BlueprintExternalPrerequisite, LanguageFeatureCensus,
    ARTIFACT_CONTRACT, LANGUAGE_CENSUS_CONTRACT,
};

#[derive(ValueEnum, Debug, Clone, Copy, Default, PartialEq, Eq)]
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
    pub parent_table_identity: Option<String>,
    pub dependencies: Vec<String>,
    pub unresolved_dependency_count: u64,
    pub definition_visibility: &'static str,
    pub security_mode: &'static str,
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
            parent_table_identity: None,
            dependencies: Vec::new(),
            unresolved_dependency_count: 0,
            definition_visibility: "not_applicable",
            security_mode: "",
            external: None,
            analysis: None,
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

#[derive(Debug, Clone, Default)]
pub struct RawLanguageAnalysis {
    pub definition: Option<Zeroizing<String>>,
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
    pub dependencies_complete: bool,
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
    mut raw: Vec<RawArtifact>,
    schema_ids: &BTreeMap<String, String>,
    table_ids: &BTreeMap<String, String>,
    mut completeness: CaptureCompleteness,
) -> ArtifactInventory {
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
        *ordinal += 1;
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
        *counts_by_kind.entry(item.kind.to_string()).or_insert(0) += 1;
        if let Some(external) = &item.external {
            external_prerequisite_count += 1;
            *counts_by_external_class
                .entry(external.class.to_string())
                .or_insert(0) += 1;
        }

        if !detail.emits_graph() {
            continue;
        }

        let mut dependencies = Vec::new();
        let mut unresolved = item.unresolved_dependency_count;
        for dependency in &item.dependencies {
            if let Some(id) = artifact_ids
                .get(dependency)
                .or_else(|| table_ids.get(dependency))
            {
                dependencies.push(id.clone());
            } else {
                unresolved += 1;
            }
        }
        dependencies.sort();
        dependencies.dedup();
        dependency_edge_count += dependencies.len() as u64;

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
        artifacts.insert(
            id,
            BlueprintArtifact {
                kind: item.kind.to_string(),
                subkind: item.subkind.to_string(),
                tier: item.tier.to_string(),
                schema: item
                    .schema_identity
                    .as_ref()
                    .and_then(|schema| schema_ids.get(schema))
                    .cloned()
                    .unwrap_or_default(),
                parent: item
                    .parent_table_identity
                    .as_ref()
                    .and_then(|table| table_ids.get(table))
                    .cloned()
                    .unwrap_or_default(),
                dependencies,
                unresolved_dependency_count: unresolved,
                definition_visibility: item.definition_visibility.to_string(),
                security_mode: item.security_mode.to_string(),
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

    let object_count = counts_by_kind.values().sum();
    ArtifactInventory {
        contract: ARTIFACT_CONTRACT.to_string(),
        detail: detail.as_str().to_string(),
        visibility: if completeness.visibility.is_empty() {
            "unknown".to_string()
        } else {
            completeness.visibility
        },
        inventory_complete: completeness.inventory_complete
            && completeness.catalogs_unreadable.is_empty()
            && completeness.families_not_inventoried.is_empty(),
        dependencies_complete: completeness.dependencies_complete
            && completeness.catalogs_unreadable.is_empty(),
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
        families_not_inventoried: completeness.families_not_inventoried,
        artifacts,
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
    let Some(definition) = raw.definition.as_deref() else {
        return LanguageFeatureCensus {
            contract: LANGUAGE_CENSUS_CONTRACT.to_string(),
            status: "unavailable".to_string(),
            dialect: normalize_dialect(&raw.dialect),
            grammar_profile: raw.grammar_profile.clone(),
            analyzer_version: "lexical-v1".to_string(),
            sql_mode_flags: normalize_sql_modes(&raw.sql_mode_flags),
            compatibility_level: raw.compatibility_level.clone(),
            ansi_nulls: normalize_switch(&raw.ansi_nulls),
            quoted_identifier: normalize_switch(&raw.quoted_identifier),
            ..LanguageFeatureCensus::default()
        };
    };

    let scrubbed = scrub_sql(definition, &raw.dialect);
    let tokens = sql_tokens(&scrubbed);
    let mut features = BTreeMap::new();
    let feature_specs: &[(&str, &[&str])] = &[
        ("control.if", &["IF"]),
        ("control.case", &["CASE"]),
        ("control.loop", &["LOOP"]),
        ("control.while", &["WHILE"]),
        ("control.repeat", &["REPEAT"]),
        ("control.exception", &["EXCEPTION", "CATCH", "HANDLER"]),
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

    let maximum_nesting = maximum_nesting(&tokens);
    let branches = branch_count(&tokens);
    let statement_count = tokens.iter().filter(|token| **token == ";").count().max(1);
    let opaque_count = dynamic_count;

    LanguageFeatureCensus {
        contract: LANGUAGE_CENSUS_CONTRACT.to_string(),
        // This first analyzer is deliberately lexical. It never claims that a
        // dialect grammar or semantic binder accepted the source definition.
        status: "partial".to_string(),
        dialect: normalize_dialect(&raw.dialect),
        grammar_profile: raw.grammar_profile.clone(),
        analyzer_version: "lexical-v1".to_string(),
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

fn normalize_dialect(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "sql" | "plpgsql" | "plpython" | "plperl" | "mysql-sql-psm" | "tsql" | "clr" | "c"
        | "internal" => value.trim().to_ascii_lowercase(),
        _ => "unknown".to_string(),
    }
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
            if matches!(c, '(' | ')' | ';') {
                tokens.push(match c {
                    '(' => "(",
                    ')' => ")",
                    ';' => ";",
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
        "AVG",
        "BEFORE",
        "BEGIN",
        "BY",
        "CASE",
        "CATCH",
        "COUNT",
        "CREATE",
        "CURSOR",
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
        "GROUP",
        "HANDLER",
        "IF",
        "INSERT",
        "INTERSECT",
        "INSTEAD",
        "INVOKER",
        "JOIN",
        "JSON",
        "JSONB",
        "JSON_QUERY",
        "JSON_VALUE",
        "LIMIT",
        "LOOP",
        "MAX",
        "MERGE",
        "MIN",
        "NATIVE_COMPILATION",
        "ORDER",
        "OR",
        "OVER",
        "PREPARE",
        "PROCEDURE",
        "RECURSIVE",
        "REPEAT",
        "SCHEMABINDING",
        "SELECT",
        "SP_EXECUTESQL",
        "SUM",
        "TEMP",
        "TEMPORARY",
        "TOP",
        "TRIGGER",
        "TRUNCATE",
        "UNION",
        "UPDATE",
        "VECTOR",
        "WHEN",
        "WHILE",
        "WITH",
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

fn maximum_nesting(tokens: &[&str]) -> u64 {
    let mut depth = 0_u64;
    let mut maximum = 0_u64;
    for token in tokens {
        match *token {
            "(" | "BEGIN" | "CASE" | "LOOP" => {
                depth += 1;
                maximum = maximum.max(depth);
            }
            ")" | "END" => depth = depth.saturating_sub(1),
            _ => {}
        }
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
    use super::*;

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
            CaptureCompleteness {
                visibility: "full".into(),
                inventory_complete: true,
                catalogs_read: vec!["pg_proc".into(), "pg_extension".into()],
                ..CaptureCompleteness::default()
            },
        );
        let encoded = toml::to_string(&inventory).unwrap();
        assert_eq!(inventory.external_prerequisite_count, 1);
        assert_eq!(inventory.counts_by_kind["function"], 1);
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
            CaptureCompleteness::default(),
        );
        assert_eq!(inventory.object_count, 1);
        assert_eq!(inventory.counts_by_kind["view"], 1);
        assert!(inventory.artifacts.is_empty());
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
            dialect: "tsql".into(),
            grammar_profile: "sqlserver-16".into(),
            ..RawLanguageAnalysis::default()
        };
        let analysis = analyze_language(&raw);
        assert!(!analysis.features.contains_key("query.cte"));
        assert_eq!(analysis.features["security.impersonation"], "1");
    }

    #[test]
    fn real_cte_dynamic_sql_and_trigger_body_mutation_are_detected() {
        let raw = RawLanguageAnalysis {
            definition: Some(Zeroizing::new(
                "CREATE TRIGGER t AFTER INSERT ON x AS BEGIN INSERT INTO audit SELECT * FROM inserted; WITH cte AS (SELECT 1 AS n) SELECT n FROM cte; EXECUTE command_text; END;".into(),
            )),
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
            CaptureCompleteness::default(),
        );
        assert!(!inventory.analysis_complete);
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
}
