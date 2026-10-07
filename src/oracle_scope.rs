//! Oracle owner selection resolved before any customer catalog is scanned.
//!
//! Oracle schemas are users.  The provider obtains the authenticated
//! `SESSION_USER` and the visible `ALL_USERS` rows through its admitted
//! session, then this module resolves the operator request without changing
//! spelling or applying host-locale rules.  Resolved names remain transient;
//! only the count and scope classification reach Blueprint/audit output.

#![allow(dead_code)]

use std::collections::BTreeSet;

/// Operator intent.  `AllVisible` is deliberately separate from an empty
/// selector because it widens the capture beyond the authenticated owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OracleOwnerSelection {
    Default,
    Selected(Vec<String>),
    AllVisible { acknowledged: bool },
}

/// A resolved, non-empty owner set.  Construction is private so the catalog
/// driver never receives an unresolved selector or an empty capture scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleOwnerScope {
    owners: Vec<String>,
    kind: OracleOwnerScopeKind,
    omitted_unrepresentable_owners: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OracleOwnerScopeKind {
    AuthenticatedOwner,
    SelectedOwners,
    AllVisibleOwners,
}

impl OracleOwnerScope {
    pub fn owners(&self) -> &[String] {
        &self.owners
    }

    pub fn kind(&self) -> OracleOwnerScopeKind {
        self.kind
    }

    pub fn is_selection_limited(&self) -> bool {
        self.kind != OracleOwnerScopeKind::AllVisibleOwners
    }

    /// Whether an ALL_USERS result contained an identity that cannot safely
    /// cross the collector boundary. Such an unrelated owner must not prevent
    /// a requested, representable owner from being captured.
    pub fn omitted_unrepresentable_owners(&self) -> bool {
        self.omitted_unrepresentable_owners
    }

    pub(crate) fn capture_subset(&self, owners: Vec<String>) -> Self {
        Self {
            owners,
            kind: self.kind,
            omitted_unrepresentable_owners: self.omitted_unrepresentable_owners,
        }
    }

    /// Construct the provisional scope needed to issue owner-bound catalogue
    /// queries before the same capture's ALL_USERS result is available.  A
    /// production caller must resolve the selection again from the captured
    /// catalogue evidence before mapping or publishing the Blueprint.
    pub(crate) fn provisional(
        selection: &OracleOwnerSelection,
        session_user: &str,
    ) -> Result<Self, OracleOwnerResolutionFailure> {
        match selection {
            OracleOwnerSelection::Default => resolve_oracle_owner_scope(
                OracleOwnerSelection::Default,
                session_user,
                [session_user.to_string()],
            ),
            OracleOwnerSelection::Selected(owners) => resolve_oracle_owner_scope(
                OracleOwnerSelection::Selected(owners.clone()),
                session_user,
                owners.clone(),
            ),
            OracleOwnerSelection::AllVisible { .. } => {
                Err(OracleOwnerResolutionFailure::AllVisibleNotAcknowledged)
            }
        }
    }

    /// Rehydrate a scope only after a checksummed offline stream has validated
    /// every owner and its ordering.  Keeping this constructor crate-private
    /// prevents unverified external input from bypassing normal resolution.
    pub(crate) fn from_offline_capture(
        owners: Vec<String>,
        kind: OracleOwnerScopeKind,
        omitted_unrepresentable_owners: bool,
    ) -> Result<Self, OracleOwnerResolutionFailure> {
        if owners.is_empty() {
            return Err(OracleOwnerResolutionFailure::EmptyVisibleSet);
        }
        let mut unique = BTreeSet::new();
        for owner in &owners {
            validate_owner(owner)?;
            if !unique.insert(owner) {
                return Err(OracleOwnerResolutionFailure::InvalidOwner);
            }
        }
        Ok(Self {
            owners,
            kind,
            omitted_unrepresentable_owners,
        })
    }

    #[cfg(test)]
    pub(crate) fn one(owner: &str) -> Self {
        Self {
            owners: vec![owner.to_string()],
            kind: OracleOwnerScopeKind::SelectedOwners,
            omitted_unrepresentable_owners: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OracleOwnerResolutionFailure {
    InvalidOwner,
    EmptyVisibleSet,
    MissingRequestedOwner,
    AllVisibleNotAcknowledged,
}

/// Resolve Oracle-native owner spellings returned by the database.
///
/// The caller must supply the exact `SESSION_USER` and visible owner values
/// from the same admitted session.  Requested names are compared byte-for-byte
/// with those native values: upper-casing on the collector host would corrupt
/// quoted, mixed-case and non-ASCII Oracle identifiers.
pub fn resolve_oracle_owner_scope(
    selection: OracleOwnerSelection,
    session_user: &str,
    visible_owners: impl IntoIterator<Item = String>,
) -> Result<OracleOwnerScope, OracleOwnerResolutionFailure> {
    validate_owner(session_user)?;
    let mut omitted_unrepresentable_owners = false;
    let visible = visible_owners
        .into_iter()
        .filter_map(|owner| match validate_owner(&owner) {
            Ok(()) => Some(owner),
            Err(_) => {
                omitted_unrepresentable_owners = true;
                None
            }
        })
        .collect::<BTreeSet<_>>();

    match selection {
        OracleOwnerSelection::Default => {
            if !visible.contains(session_user) {
                return Err(OracleOwnerResolutionFailure::MissingRequestedOwner);
            }
            Ok(OracleOwnerScope {
                owners: vec![session_user.to_string()],
                kind: OracleOwnerScopeKind::AuthenticatedOwner,
                omitted_unrepresentable_owners,
            })
        }
        OracleOwnerSelection::Selected(requested) => {
            let requested = requested
                .into_iter()
                .map(|owner| {
                    validate_owner(&owner)?;
                    Ok(owner)
                })
                .collect::<Result<BTreeSet<_>, OracleOwnerResolutionFailure>>()?;
            if requested.is_empty() {
                return Err(OracleOwnerResolutionFailure::EmptyVisibleSet);
            }
            if !requested.is_subset(&visible) {
                return Err(OracleOwnerResolutionFailure::MissingRequestedOwner);
            }
            Ok(OracleOwnerScope {
                owners: requested.into_iter().collect(),
                kind: OracleOwnerScopeKind::SelectedOwners,
                omitted_unrepresentable_owners,
            })
        }
        OracleOwnerSelection::AllVisible { acknowledged } => {
            if !acknowledged {
                return Err(OracleOwnerResolutionFailure::AllVisibleNotAcknowledged);
            }
            if visible.is_empty() {
                return Err(OracleOwnerResolutionFailure::EmptyVisibleSet);
            }
            Ok(OracleOwnerScope {
                owners: visible.into_iter().collect(),
                kind: OracleOwnerScopeKind::AllVisibleOwners,
                omitted_unrepresentable_owners,
            })
        }
    }
}

fn validate_owner(owner: &str) -> Result<(), OracleOwnerResolutionFailure> {
    if owner.is_empty()
        || owner.len() > 128
        || owner.chars().any(|ch| ch == '\0' || ch.is_control())
    {
        return Err(OracleOwnerResolutionFailure::InvalidOwner);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_scope_is_exactly_the_authenticated_owner() {
        let scope = resolve_oracle_owner_scope(
            OracleOwnerSelection::Default,
            "APP",
            ["APP".to_string(), "OTHER".to_string()],
        )
        .unwrap();
        assert_eq!(scope.owners(), &["APP"]);
        assert_eq!(scope.kind(), OracleOwnerScopeKind::AuthenticatedOwner);
    }

    #[test]
    fn selected_scope_is_sorted_deduplicated_and_requires_every_owner() {
        let scope = resolve_oracle_owner_scope(
            OracleOwnerSelection::Selected(vec![
                "Sales".to_string(),
                "APP".to_string(),
                "Sales".to_string(),
            ]),
            "APP",
            ["APP".to_string(), "Sales".to_string()],
        )
        .unwrap();
        assert_eq!(scope.owners(), &["APP", "Sales"]);
        assert_eq!(scope.kind(), OracleOwnerScopeKind::SelectedOwners);

        assert_eq!(
            resolve_oracle_owner_scope(
                OracleOwnerSelection::Selected(vec!["app".to_string()]),
                "APP",
                ["APP".to_string()],
            ),
            Err(OracleOwnerResolutionFailure::MissingRequestedOwner),
            "collector-side case folding would break quoted identifiers"
        );
    }

    #[test]
    fn all_visible_requires_separate_acknowledgement_and_nonempty_evidence() {
        assert_eq!(
            resolve_oracle_owner_scope(
                OracleOwnerSelection::AllVisible {
                    acknowledged: false,
                },
                "APP",
                ["APP".to_string()],
            ),
            Err(OracleOwnerResolutionFailure::AllVisibleNotAcknowledged)
        );
        assert_eq!(
            resolve_oracle_owner_scope(
                OracleOwnerSelection::AllVisible { acknowledged: true },
                "APP",
                [],
            ),
            Err(OracleOwnerResolutionFailure::EmptyVisibleSet)
        );
    }

    #[test]
    fn invalid_owner_text_never_crosses_the_scope_boundary() {
        assert_eq!(
            resolve_oracle_owner_scope(
                OracleOwnerSelection::Selected(vec!["BAD\nOWNER".to_string()]),
                "APP",
                ["APP".to_string()],
            ),
            Err(OracleOwnerResolutionFailure::InvalidOwner)
        );
    }

    #[test]
    fn unrelated_unrepresentable_visible_owner_does_not_block_requested_owner() {
        let scope = resolve_oracle_owner_scope(
            OracleOwnerSelection::Default,
            "APP",
            ["APP".to_string(), "BAD\nOWNER".to_string()],
        )
        .unwrap();
        assert_eq!(scope.owners(), &["APP"]);
        assert!(scope.omitted_unrepresentable_owners());
    }
}
