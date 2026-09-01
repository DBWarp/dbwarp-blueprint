//! Live database schema selection without serializing source identifiers.
//!
//! The CLI accepts repeatable `--schema NAME` values. Engines resolve those
//! names through their native catalogs, then use the resolved spelling for
//! filtering. Only the count and the closed `selection-limited` token are
//! allowed into audit/Blueprint output.

use std::collections::BTreeSet;

use anyhow::{bail, Result};

use crate::format::DatasetScope;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SchemaSelection {
    names: Vec<String>,
}

impl SchemaSelection {
    pub fn new(names: impl IntoIterator<Item = String>) -> Self {
        let mut names = names
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        names.sort();
        Self { names }
    }

    pub fn is_active(&self) -> bool {
        !self.names.is_empty()
    }

    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn names(&self) -> &[String] {
        &self.names
    }

    pub fn includes(&self, schema: &str) -> bool {
        !self.is_active() || self.names.iter().any(|name| name == schema)
    }

    /// SQL fragment for a predicate appended to an existing WHERE clause.
    /// The column expression is selected by trusted source code; schema names
    /// are string literals with single quotes doubled.
    pub fn and_sql(&self, column: &str) -> String {
        if !self.is_active() {
            return String::new();
        }
        format!(" AND {column} IN ({})", self.sql_literals())
    }

    pub fn qualify_dataset_scope(&self, scope: &mut DatasetScope) {
        if !self.is_active() {
            return;
        }
        scope.limitations.push("selection-limited".to_string());
        scope.limitations.sort();
        scope.limitations.dedup();
    }

    fn sql_literals(&self) -> String {
        self.names
            .iter()
            .map(|name| format!("'{}'", name.replace('\'', "''")))
            .collect::<Vec<_>>()
            .join(",")
    }
}

pub fn parse_schema_name(raw: &str) -> std::result::Result<String, String> {
    if raw.is_empty() {
        return Err("schema name must not be empty".to_string());
    }
    if raw.len() > 512 {
        return Err("schema name must not exceed 512 UTF-8 bytes".to_string());
    }
    if raw.chars().any(|ch| ch == '\0' || ch.is_control()) {
        return Err("schema name must not contain NUL or control characters".to_string());
    }
    Ok(raw.to_string())
}

pub fn resolved_selection(
    requested: &SchemaSelection,
    resolved_names: impl IntoIterator<Item = String>,
    native_comparison_was_applied: bool,
) -> Result<SchemaSelection> {
    if !requested.is_active() {
        return Ok(SchemaSelection::default());
    }
    let resolved = SchemaSelection::new(resolved_names);
    // MySQL and SQL Server have already compared the requested literals using
    // the database's native identifier collation. Comparing their returned
    // spelling again in Rust would incorrectly reject valid non-ASCII,
    // accent-insensitive, or locale-specific matches. Schema names are unique
    // under those engines' catalog rules, so row-count reconciliation proves
    // that each requested selector resolved. PostgreSQL uses exact spelling
    // and is reconciled explicitly here.
    let missing = if native_comparison_was_applied {
        requested.len().saturating_sub(resolved.len())
    } else {
        requested
            .names()
            .iter()
            .filter(|requested_name| !resolved.names().contains(requested_name))
            .count()
    };
    if missing > 0 {
        bail!(
            "DBP1420E {missing} requested schema selector(s) were not visible in the connected database. Next: verify the database, spelling, and metadata grants; no Blueprint was written."
        );
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_is_sorted_deduplicated_and_sql_escaped() {
        let selection = SchemaSelection::new([
            "sales'west".to_string(),
            "app".to_string(),
            "app".to_string(),
        ]);
        assert_eq!(selection.names(), &["app", "sales'west"]);
        assert_eq!(
            selection.and_sql("n.nspname"),
            " AND n.nspname IN ('app','sales''west')"
        );
    }

    #[test]
    fn native_resolution_refuses_any_missing_requested_schema() {
        let requested = SchemaSelection::new(["app".to_string(), "missing".to_string()]);
        let error = resolved_selection(&requested, ["app".to_string()], false).unwrap_err();
        assert!(error.to_string().starts_with("DBP1420E 1 requested"));
    }

    #[test]
    fn mysql_and_sqlserver_resolution_accepts_native_catalog_spelling() {
        let requested = SchemaSelection::new(["Cafe".to_string()]);
        let resolved = resolved_selection(&requested, ["Café".to_string()], true).unwrap();
        assert_eq!(resolved.names(), &["Café"]);
    }

    #[test]
    fn active_selection_qualifies_totals_without_declaring_incompleteness() {
        let selection = SchemaSelection::new(["app".to_string()]);
        let mut scope = DatasetScope {
            limitations: vec!["row-counts-statistical".to_string()],
            ..DatasetScope::default()
        };
        selection.qualify_dataset_scope(&mut scope);
        assert_eq!(
            scope.limitations,
            vec!["row-counts-statistical", "selection-limited"]
        );
    }
}
