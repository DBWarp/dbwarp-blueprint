//! The Oracle query execution boundary: one negotiated session driving the
//! catalog contract under enforced limits.
//!
//! Sits between `oracle_provider` (which admits a session and fixes its
//! capabilities) and `oracle_catalog` (which fixes what runs). An adapter
//! supplies raw execution; this module owns the normalized result interface,
//! query execution boundary, deadline accounting, cancellation semantics, and
//! degradation rules - every catalog failure is retained per owner/family so
//! the mapper can apply the named Blueprint floor after all readable owners
//! have run, and
//! an option-owned view's absence is attributed to its component rather
//! than reported as a version gap or a failure.
//!
//! Definition transience is enforced here, not merely documented: below
//! analyzed detail the definition-bearing queries are never issued, so no
//! source text crosses the boundary that the mapper would then have to
//! discard. Their kinds stay inventoried through the object listing; the
//! skip is recorded, never silent.

#![allow(dead_code)]

use crate::artifacts::ArtifactDetail;
use crate::oracle_catalog::{queries_for_tier, CatalogQuery, Presence};
#[cfg(test)]
use crate::oracle_provider::OracleCaptureTier;
use crate::oracle_provider::{
    OracleCancellation, OracleClientVersionAttestation, OracleNegotiatedCapabilities,
};
use crate::oracle_scope::OracleOwnerScope;
use std::collections::BTreeSet;

/// Typed scalar as the adapter observed it, lexically preserved. The mapper
/// owns interpretation; this boundary owns only shape and bounds. Text from
/// definition-bearing queries is source material - the mapper must analyze
/// transiently and discard, matching the audit's trust assertion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OracleValue {
    Null,
    Number(String),
    Text(String),
    Timestamp(String),
    /// Text fetched from an Oracle LOB/LONG result. It remains transient and
    /// is separately charged to the negotiated LOB budget.
    LobText(String),
}

impl OracleValue {
    fn byte_len(&self) -> Option<u64> {
        match self {
            Self::Null => Some(0),
            Self::Number(value)
            | Self::Text(value)
            | Self::Timestamp(value)
            | Self::LobText(value) => u64::try_from(value.len()).ok(),
        }
    }

    fn is_lob(&self) -> bool {
        matches!(self, Self::LobText(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleRow {
    pub values: Vec<OracleValue>,
}

/// Failure classes an adapter reports. Deliberately closed: raw ORA text
/// stays inside the adapter, which is responsible for sanitizing it into a
/// class before it crosses this boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OracleQueryFailure {
    PermissionDenied,
    ObjectAbsent,
    /// A sanitized database error outside the narrower classes above. The
    /// raw ORA text and values stay inside the adapter; optional catalog
    /// families may degrade without calling valid error output malformed.
    DatabaseError,
    /// The command provider reached a documented transport allocation bound
    /// before it could deliver a complete framed row.
    TransportLimitExceeded,
    /// A decoded scalar exceeded the per-value limit supplied to the adapter.
    TransientValueLimitExceeded,
    Timeout,
    SessionLost,
    Malformed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OracleQueryStatus {
    Executed {
        rows: u64,
    },
    /// The query ran - server work happened - and its result breached a
    /// hard limit, so the rows were discarded whole. Distinct from
    /// NotReached, which claims no execution: conflating them would
    /// misdescribe what the source actually did.
    ResultDiscarded {
        reason: OracleCaptureAbort,
    },
    Failed {
        class: OracleQueryFailure,
    },
    /// A complete preceding DBA_REGISTRY read proves that the owning option is
    /// not installed; `component` is the closed contract token.
    OptionAbsent {
        component: &'static str,
    },
    SkippedVersion,
    SkippedDetail,
    NotReached,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleQueryOutcome {
    pub query_id: &'static str,
    pub view: &'static str,
    /// One-based ordinal in the transient resolved owner set.  The native
    /// owner name is deliberately not copied into status/error evidence.
    /// Unscoped catalog reads carry `None` and execute exactly once.
    pub owner_ordinal: Option<u32>,
    pub status: OracleQueryStatus,
    pub rows: Vec<OracleRow>,
}

/// Why a capture stopped before its query list was exhausted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OracleCaptureAbort {
    /// A floor-family failure recorded as a capture-wide reason. Sessions
    /// retain that failure on its owner/query outcome so another readable
    /// owner can still satisfy the floor; the mapper treats this as truncation
    /// rather than a hard refusal.
    CoreInventoryUnavailable,
    /// Owner selection was not resolved to at least one native catalog value.
    OwnerScopeUnavailable,
    /// The negotiated capabilities describe no live, version-attested
    /// session - an offline provider or an unverified proof.
    NoLiveSession,
    RowLimitExceeded,
    ByteLimitExceeded,
    TransientValueLimitExceeded,
    LobLimitExceeded,
    DeadlineExceeded,
    SessionLost,
    /// The adapter broke the execution contract, for example by reporting a
    /// regressing clock or materializing a scalar above the supplied limit.
    AdapterContractViolated,
    /// The negotiation promised server-confirmed cancellation and the
    /// adapter could not confirm it.
    UnconfirmedCancellation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OracleCancelOutcome {
    ServerConfirmed,
    Unconfirmed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleCatalogCapture {
    pub outcomes: Vec<OracleQueryOutcome>,
    pub catalogs_read: Vec<&'static str>,
    pub catalogs_unreadable: Vec<&'static str>,
    pub abort: Option<OracleCaptureAbort>,
    /// True when cancellation semantics forced discarding the session; no
    /// further statement may be issued on it by anyone.
    pub session_discarded: bool,
    /// True when only the local client could be stopped and server-side
    /// work may still be running - evidence the audit must carry.
    pub unconfirmed_server_work: bool,
    pub rows_consumed: u64,
    /// Value bytes only, per the limit's meaning: the sum of scalar
    /// lengths, not wire or row-structure overhead.
    pub bytes_consumed: u64,
    pub lob_bytes_consumed: u64,
    pub elapsed_ms: u64,
    /// Command-client provenance from a live provider. Offline providers have
    /// no local client and therefore leave this absent.
    pub client_version_attestation: Option<OracleClientVersionAttestation>,
}

/// What the driver asks of one adapter call. `deadline_ms` is the remaining
/// capture budget, never the per-query limit alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleExecution<'a> {
    pub sql: &'a str,
    pub owner: Option<&'a str>,
    /// Exact result width from the version-gated catalog registry. Adapters
    /// must reject a different width before associating values with fields.
    pub expected_columns: usize,
    /// Exact, registry-owned projection labels in row order. Command clients
    /// use these to encode one bounded output row without executing a
    /// server-side buffering block.
    pub projected_columns: &'a [&'static str],
    pub deadline_ms: u64,
    /// Remaining capture-wide budgets at statement launch. The adapter must
    /// stop before materializing a row, scalar or LOB that would exceed any
    /// value. The receiver independently rechecks every delivered result.
    pub max_catalog_rows: u64,
    pub max_catalog_bytes: u64,
    pub max_transient_value_bytes: u64,
    pub max_lob_bytes: u64,
    pub max_nesting_depth: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OracleExecuteResult {
    Complete,
    Failed(OracleQueryFailure),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OracleRowControl {
    Continue,
    Stop,
}

/// Incremental result receiver owned by the session boundary. Adapters must
/// stop fetching as soon as it returns `Stop`; this prevents a complete query
/// result from being allocated before negotiated limits are inspected.
pub trait OracleRowReceiver {
    fn receive(&mut self, row: OracleRow) -> OracleRowControl;
}

/// One live session an adapter polices. Implementations sit below the
/// negotiation boundary and never see an unadmitted request.
pub trait OracleSessionAdapter {
    /// Execute under the supplied per-call deadline and parser limits, yielding
    /// rows incrementally. Returning after `deadline_ms` without `Timeout`
    /// violates the adapter contract; the boundary also checks the monotonic
    /// session clock when control returns.
    fn execute(
        &mut self,
        execution: &OracleExecution<'_>,
        receiver: &mut dyn OracleRowReceiver,
    ) -> OracleExecuteResult;
    /// Attempt to cancel in-flight database work. What the return value can
    /// honestly say is bounded by the negotiated cancellation semantics.
    fn cancel(&mut self) -> OracleCancelOutcome;
    /// Milliseconds of wall clock this session has consumed so far, from
    /// the adapter's own clock so the driver never needs one.
    fn elapsed_ms(&self) -> u64;
}

fn skip_for_detail(query: &CatalogQuery, detail: ArtifactDetail) -> bool {
    query.definition_bearing && detail != ArtifactDetail::Analyzed
}

/// Per-query streaming collector. Cumulative counters start at the preceding
/// queries' totals, so no individual query can consume a fresh copy of the
/// capture budget.
struct BoundedRows {
    rows: Vec<OracleRow>,
    query_rows: u64,
    rows_consumed: u64,
    bytes_consumed: u64,
    lob_bytes_consumed: u64,
    limits: crate::oracle_provider::OracleProviderLimits,
    abort: Option<OracleCaptureAbort>,
}

impl BoundedRows {
    fn new(
        capture: &OracleCatalogCapture,
        limits: crate::oracle_provider::OracleProviderLimits,
    ) -> Self {
        Self {
            rows: Vec::new(),
            query_rows: 0,
            rows_consumed: capture.rows_consumed,
            bytes_consumed: capture.bytes_consumed,
            lob_bytes_consumed: capture.lob_bytes_consumed,
            limits,
            abort: None,
        }
    }

    fn fail(&mut self, reason: OracleCaptureAbort) -> OracleRowControl {
        self.abort = Some(reason);
        self.rows.clear();
        OracleRowControl::Stop
    }

    fn commit_to(&self, capture: &mut OracleCatalogCapture) {
        capture.rows_consumed = self.rows_consumed;
        capture.bytes_consumed = self.bytes_consumed;
        capture.lob_bytes_consumed = self.lob_bytes_consumed;
    }
}

impl OracleRowReceiver for BoundedRows {
    fn receive(&mut self, row: OracleRow) -> OracleRowControl {
        if self.abort.is_some() {
            return OracleRowControl::Stop;
        }

        let mut row_bytes = Some(0_u64);
        let mut row_lob_bytes = Some(0_u64);
        let mut transient_limit_exceeded = false;
        for value in &row.values {
            let Some(value_bytes) = value.byte_len() else {
                return self.fail(OracleCaptureAbort::AdapterContractViolated);
            };
            if value_bytes > self.limits.transient_value_bytes {
                transient_limit_exceeded = true;
            }
            row_bytes = row_bytes.and_then(|count| count.checked_add(value_bytes));
            if value.is_lob() {
                row_lob_bytes = row_lob_bytes.and_then(|count| count.checked_add(value_bytes));
            }
        }

        // The adapter has already delivered this row, so account for its
        // source impact even when it violates the pre-allocation contract.
        // Arithmetic overflow is an adapter-contract violation; saturating it
        // would make a logically unbounded result resemble a valid counter.
        let Some(rows_consumed) = self.rows_consumed.checked_add(1) else {
            return self.fail(OracleCaptureAbort::AdapterContractViolated);
        };
        let Some(query_rows) = self.query_rows.checked_add(1) else {
            return self.fail(OracleCaptureAbort::AdapterContractViolated);
        };
        let Some(row_bytes) = row_bytes else {
            return self.fail(OracleCaptureAbort::AdapterContractViolated);
        };
        let Some(row_lob_bytes) = row_lob_bytes else {
            return self.fail(OracleCaptureAbort::AdapterContractViolated);
        };
        let Some(bytes_consumed) = self.bytes_consumed.checked_add(row_bytes) else {
            return self.fail(OracleCaptureAbort::AdapterContractViolated);
        };
        let Some(lob_bytes_consumed) = self.lob_bytes_consumed.checked_add(row_lob_bytes) else {
            return self.fail(OracleCaptureAbort::AdapterContractViolated);
        };
        self.rows_consumed = rows_consumed;
        self.query_rows = query_rows;
        self.bytes_consumed = bytes_consumed;
        self.lob_bytes_consumed = lob_bytes_consumed;

        if transient_limit_exceeded {
            return self.fail(OracleCaptureAbort::TransientValueLimitExceeded);
        }
        if rows_consumed > self.limits.catalog_rows {
            return self.fail(OracleCaptureAbort::RowLimitExceeded);
        }
        if bytes_consumed > self.limits.catalog_bytes {
            return self.fail(OracleCaptureAbort::ByteLimitExceeded);
        }
        if lob_bytes_consumed > self.limits.lob_bytes {
            return self.fail(OracleCaptureAbort::LobLimitExceeded);
        }

        self.rows.push(row);
        OracleRowControl::Continue
    }
}

/// Drive the negotiated tier's catalog queries for one resolved owner.
///
/// The server version gating version-dependent SQL comes from the
/// negotiation's own proof, never from the caller, which would be unverified
/// self-assertion. `owner` is the resolved selected owner the queries bind.
pub fn run_catalog_capture(
    negotiated: &OracleNegotiatedCapabilities,
    adapter: &mut dyn OracleSessionAdapter,
    owner: &str,
) -> OracleCatalogCapture {
    let owner = owner.to_string();
    run_catalog_capture_for_owners(negotiated, adapter, std::slice::from_ref(&owner))
}

/// Drive a resolved Oracle owner scope through one admitted adapter session.
///
/// Owner-scoped dictionary queries execute once per resolved owner. Global
/// queries execute once for the complete capture. Counters and deadlines are
/// never reset between owners, so requesting more owners consumes the same
/// capture-wide row, byte, LOB and elapsed-time budgets.
pub fn run_catalog_capture_for_scope(
    negotiated: &OracleNegotiatedCapabilities,
    adapter: &mut dyn OracleSessionAdapter,
    scope: &OracleOwnerScope,
) -> OracleCatalogCapture {
    run_catalog_capture_for_owners(negotiated, adapter, scope.owners())
}

#[derive(Debug, Clone, Copy)]
struct PlannedQuery {
    query: &'static CatalogQuery,
    owner_index: Option<usize>,
}

fn run_catalog_capture_for_owners(
    negotiated: &OracleNegotiatedCapabilities,
    adapter: &mut dyn OracleSessionAdapter,
    owners: &[String],
) -> OracleCatalogCapture {
    let queries: Vec<&'static CatalogQuery> = queries_for_tier(negotiated.tier()).collect();
    let planned = queries
        .iter()
        .flat_map(|query| {
            if query.owner_column.is_some() {
                (0..owners.len())
                    .map(|owner_index| PlannedQuery {
                        query,
                        owner_index: Some(owner_index),
                    })
                    .collect::<Vec<_>>()
            } else {
                vec![PlannedQuery {
                    query,
                    owner_index: None,
                }]
            }
        })
        .collect::<Vec<_>>();
    let mut capture = OracleCatalogCapture {
        outcomes: Vec::with_capacity(planned.len().max(queries.len())),
        catalogs_read: Vec::new(),
        catalogs_unreadable: Vec::new(),
        abort: None,
        session_discarded: false,
        unconfirmed_server_work: false,
        rows_consumed: 0,
        bytes_consumed: 0,
        lob_bytes_consumed: 0,
        elapsed_ms: 0,
        client_version_attestation: negotiated.proof().client_version_attestation(),
    };
    if owners.is_empty() {
        for query in &queries {
            capture.outcomes.push(OracleQueryOutcome {
                query_id: query.query_id,
                view: query.view,
                owner_ordinal: None,
                status: OracleQueryStatus::NotReached,
                rows: Vec::new(),
            });
        }
        capture.abort = Some(OracleCaptureAbort::OwnerScopeUnavailable);
        return capture;
    }
    // An offline capability set has no session to drive, and a proof that
    // attests no server version cannot gate version-dependent SQL. Both are
    // admission-layer impossibilities for a live capture; if one arrives
    // here anyway, refusing is the only honest response.
    let server_version = match (
        negotiated.provider(),
        negotiated.proof().attested_server_version(),
    ) {
        (crate::oracle_provider::OracleProviderKind::Offline, _) | (_, None) => {
            for item in &planned {
                capture.outcomes.push(OracleQueryOutcome {
                    query_id: item.query.query_id,
                    view: item.query.view,
                    owner_ordinal: item
                        .owner_index
                        .and_then(|index| u32::try_from(index + 1).ok()),
                    status: OracleQueryStatus::NotReached,
                    rows: Vec::new(),
                });
            }
            capture.abort = Some(OracleCaptureAbort::NoLiveSession);
            return capture;
        }
        (_, Some(version)) => version,
    };
    let server_major = server_version.components()[0];
    let limits = negotiated.limits();
    let mut installed_components: Option<BTreeSet<String>> = None;

    let mut abort: Option<OracleCaptureAbort> = None;
    for item in planned {
        let query = item.query;
        let owner_ordinal = item
            .owner_index
            .and_then(|index| u32::try_from(index + 1).ok());
        if let Some(reason) = abort {
            // Everything after an abort is declared unreached, never
            // silently missing from the outcome list.
            capture.outcomes.push(OracleQueryOutcome {
                query_id: query.query_id,
                view: query.view,
                owner_ordinal,
                status: OracleQueryStatus::NotReached,
                rows: Vec::new(),
            });
            capture.abort = Some(reason);
            continue;
        }
        if !query.applies_to(server_major) {
            capture.outcomes.push(OracleQueryOutcome {
                query_id: query.query_id,
                view: query.view,
                owner_ordinal,
                status: OracleQueryStatus::SkippedVersion,
                rows: Vec::new(),
            });
            continue;
        }
        if skip_for_detail(query, negotiated.artifact_detail()) {
            capture.outcomes.push(OracleQueryOutcome {
                query_id: query.query_id,
                view: query.view,
                owner_ordinal,
                status: OracleQueryStatus::SkippedDetail,
                rows: Vec::new(),
            });
            continue;
        }

        let elapsed = adapter.elapsed_ms();
        capture.elapsed_ms = elapsed;
        if elapsed >= limits.elapsed_ms {
            // No statement is in flight. Claiming cancellation here would be
            // false, especially for local-process providers where it would
            // also invent unconfirmed server work.
            abort = Some(OracleCaptureAbort::DeadlineExceeded);
            capture.outcomes.push(OracleQueryOutcome {
                query_id: query.query_id,
                view: query.view,
                owner_ordinal,
                status: OracleQueryStatus::NotReached,
                rows: Vec::new(),
            });
            capture.abort = abort;
            continue;
        }

        let sql = query.render_sql_for_version(server_version);
        let projected_columns = query.output_columns_for_version(server_version);
        let execution = OracleExecution {
            sql: &sql,
            owner: item.owner_index.map(|index| owners[index].as_str()),
            expected_columns: projected_columns.len(),
            projected_columns: &projected_columns,
            deadline_ms: limits.elapsed_ms - elapsed,
            max_catalog_rows: limits.catalog_rows.saturating_sub(capture.rows_consumed),
            max_catalog_bytes: limits.catalog_bytes.saturating_sub(capture.bytes_consumed),
            max_transient_value_bytes: limits.transient_value_bytes,
            max_lob_bytes: limits.lob_bytes.saturating_sub(capture.lob_bytes_consumed),
            max_nesting_depth: limits.result_nesting_depth,
        };
        let mut bounded_rows = BoundedRows::new(&capture, limits);
        let execute_result = adapter.execute(&execution, &mut bounded_rows);
        let post_elapsed = adapter.elapsed_ms();
        let clock_invalid = post_elapsed < elapsed;
        capture.elapsed_ms = post_elapsed;

        if let Some(reason) = bounded_rows.abort {
            bounded_rows.commit_to(&mut capture);
            let reason =
                cancel_and_classify(adapter, negotiated.cancellation(), &mut capture, reason);
            abort = Some(reason);
            capture.outcomes.push(OracleQueryOutcome {
                query_id: query.query_id,
                view: query.view,
                owner_ordinal,
                status: OracleQueryStatus::ResultDiscarded { reason },
                rows: Vec::new(),
            });
            capture.abort = abort;
            continue;
        }

        if clock_invalid
            || (post_elapsed >= limits.elapsed_ms
                && execute_result != OracleExecuteResult::Failed(OracleQueryFailure::Timeout))
        {
            bounded_rows.commit_to(&mut capture);
            // execute() has returned, so there is no in-flight statement to
            // cancel. Discard the session because its timing contract is no
            // longer trustworthy, but do not invent server work.
            capture.session_discarded = true;
            let reason = if clock_invalid {
                OracleCaptureAbort::AdapterContractViolated
            } else {
                OracleCaptureAbort::DeadlineExceeded
            };
            abort = Some(reason);
            capture.outcomes.push(OracleQueryOutcome {
                query_id: query.query_id,
                view: query.view,
                owner_ordinal,
                status: OracleQueryStatus::ResultDiscarded { reason },
                rows: Vec::new(),
            });
            capture.abort = abort;
            continue;
        }

        bounded_rows.commit_to(&mut capture);
        match execute_result {
            OracleExecuteResult::Complete => {
                if query.query_id == "oracle-registry" {
                    installed_components = registry_components(&bounded_rows.rows);
                }
                capture.catalogs_read.push(query.view);
                capture.outcomes.push(OracleQueryOutcome {
                    query_id: query.query_id,
                    view: query.view,
                    owner_ordinal,
                    status: OracleQueryStatus::Executed {
                        rows: bounded_rows.query_rows,
                    },
                    rows: bounded_rows.rows,
                });
            }
            OracleExecuteResult::Failed(class) => {
                if class == OracleQueryFailure::TransportLimitExceeded {
                    let reason = cancel_and_classify(
                        adapter,
                        negotiated.cancellation(),
                        &mut capture,
                        OracleCaptureAbort::ByteLimitExceeded,
                    );
                    abort = Some(reason);
                    capture.outcomes.push(OracleQueryOutcome {
                        query_id: query.query_id,
                        view: query.view,
                        owner_ordinal,
                        status: OracleQueryStatus::ResultDiscarded { reason },
                        rows: Vec::new(),
                    });
                    capture.abort = abort;
                    continue;
                }
                if class == OracleQueryFailure::Malformed {
                    // A malformed provider stream means framing or typing can
                    // no longer be trusted. Continuing on the same session
                    // could associate later rows with the wrong catalog.
                    abort = Some(OracleCaptureAbort::AdapterContractViolated);
                    capture.session_discarded = true;
                }
                if class == OracleQueryFailure::Timeout {
                    abort = Some(cancel_and_classify(
                        adapter,
                        negotiated.cancellation(),
                        &mut capture,
                        OracleCaptureAbort::DeadlineExceeded,
                    ));
                }
                let status = match (class, query.presence) {
                    (OracleQueryFailure::ObjectAbsent, Presence::OptionComponent(component))
                        if installed_components.as_ref().is_some_and(|installed| {
                            !installed.contains(&component.to_ascii_uppercase())
                        }) =>
                    {
                        OracleQueryStatus::OptionAbsent { component }
                    }
                    _ => OracleQueryStatus::Failed { class },
                };
                let session_lost = class == OracleQueryFailure::SessionLost;
                if matches!(status, OracleQueryStatus::Failed { .. }) {
                    capture.catalogs_unreadable.push(query.view);
                }
                capture.outcomes.push(OracleQueryOutcome {
                    query_id: query.query_id,
                    view: query.view,
                    owner_ordinal,
                    status,
                    rows: Vec::new(),
                });
                if session_lost {
                    abort = Some(OracleCaptureAbort::SessionLost);
                    capture.session_discarded = true;
                }
                if abort.is_some() {
                    capture.abort = abort;
                }
            }
        }
    }

    capture.catalogs_read.sort_unstable();
    capture.catalogs_read.dedup();
    capture.catalogs_unreadable.sort_unstable();
    capture.catalogs_unreadable.dedup();
    capture
}

/// A complete, well-shaped DBA_REGISTRY result is the only evidence that an
/// option-owned view is absent because its component is not installed. Any
/// malformed row makes the evidence unusable and the later ORA-00942 remains
/// an ordinary query failure.
fn registry_components(rows: &[OracleRow]) -> Option<BTreeSet<String>> {
    let mut installed = BTreeSet::new();
    for row in rows {
        let component = match row.values.first()? {
            OracleValue::Text(value) if !value.trim().is_empty() => value,
            _ => return None,
        };
        row.values.get(1)?;
        installed.insert(component.trim().to_ascii_uppercase());
    }
    Some(installed)
}

/// Enforce the negotiated cancellation semantics after a hard stop.
fn cancel_and_classify(
    adapter: &mut dyn OracleSessionAdapter,
    negotiated: OracleCancellation,
    capture: &mut OracleCatalogCapture,
    reason: OracleCaptureAbort,
) -> OracleCaptureAbort {
    let outcome = adapter.cancel();
    if reason == OracleCaptureAbort::AdapterContractViolated {
        // Once the adapter's own timing or allocation contract is false, no
        // successful cancellation claim makes the session safe to reuse.
        capture.session_discarded = true;
    }
    match negotiated {
        OracleCancellation::ServerConfirmed => {
            if outcome != OracleCancelOutcome::ServerConfirmed {
                // The session was admitted on the promise of confirmed
                // cancellation; an unconfirmed one is a stronger failure
                // than the limit that triggered it.
                capture.session_discarded = true;
                return OracleCaptureAbort::UnconfirmedCancellation;
            }
        }
        OracleCancellation::SessionDiscardRequired => {
            capture.session_discarded = true;
        }
        OracleCancellation::LocalProcessOnly => {
            capture.session_discarded = true;
            capture.unconfirmed_server_work = true;
        }
        OracleCancellation::NotApplicable => {}
    }
    reason
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oracle_provider::{
        test_negotiated_capabilities, OracleProviderKind, OracleProviderLimits,
    };
    use crate::oracle_scope::{resolve_oracle_owner_scope, OracleOwnerSelection};
    use std::collections::BTreeMap;

    #[derive(Clone)]
    enum ScriptedResponse {
        Rows(Vec<OracleRow>),
        Failed(OracleQueryFailure),
    }

    /// Scripted adapter: responses by query id, a controllable clock, and a
    /// record of every call so tests assert what actually crossed the
    /// boundary rather than what the driver reports about itself.
    struct ScriptedAdapter {
        responses: BTreeMap<&'static str, ScriptedResponse>,
        default_rows: u64,
        cancel_outcome: OracleCancelOutcome,
        clock_step_ms: u64,
        clock_ms: std::cell::Cell<u64>,
        regress_on_execute: bool,
        executed_sql: Vec<String>,
        executed_owners: Vec<Option<String>>,
        executed_limits: Vec<(u64, u64, u64, u64, u32, u64)>,
        cancel_calls: u32,
    }

    impl ScriptedAdapter {
        fn new() -> Self {
            Self {
                responses: BTreeMap::new(),
                default_rows: 1,
                cancel_outcome: OracleCancelOutcome::ServerConfirmed,
                clock_step_ms: 1,
                clock_ms: std::cell::Cell::new(0),
                regress_on_execute: false,
                executed_sql: Vec::new(),
                executed_owners: Vec::new(),
                executed_limits: Vec::new(),
                cancel_calls: 0,
            }
        }

        fn rows(count: u64) -> ScriptedResponse {
            ScriptedResponse::Rows(
                (0..count)
                    .map(|index| OracleRow {
                        values: vec![OracleValue::Number(index.to_string())],
                    })
                    .collect(),
            )
        }
    }

    impl OracleSessionAdapter for ScriptedAdapter {
        fn execute(
            &mut self,
            execution: &OracleExecution<'_>,
            receiver: &mut dyn OracleRowReceiver,
        ) -> OracleExecuteResult {
            if self.regress_on_execute {
                self.clock_ms.set(self.clock_ms.get().saturating_sub(1));
            } else {
                self.clock_ms
                    .set(self.clock_ms.get().saturating_add(self.clock_step_ms));
            }
            self.executed_sql.push(execution.sql.to_string());
            self.executed_owners
                .push(execution.owner.map(str::to_string));
            self.executed_limits.push((
                execution.max_catalog_rows,
                execution.max_catalog_bytes,
                execution.max_transient_value_bytes,
                execution.max_lob_bytes,
                execution.max_nesting_depth,
                execution.deadline_ms,
            ));
            let query_id = self
                .executed_sql
                .len()
                .checked_sub(1)
                .map(|_| ())
                .and_then(|()| {
                    // Recover the id from the FROM clause: SYS.<VIEW>.
                    execution
                        .sql
                        .split("FROM SYS.")
                        .nth(1)
                        .map(|rest| rest.split_whitespace().next().unwrap_or(""))
                });
            for (id, response) in &self.responses {
                if let Some(view) = query_id {
                    if crate::oracle_catalog::CATALOG_QUERIES
                        .iter()
                        .any(|q| q.query_id == *id && q.view == view)
                    {
                        return match response.clone() {
                            ScriptedResponse::Rows(rows) => {
                                for row in rows {
                                    if receiver.receive(row) == OracleRowControl::Stop {
                                        break;
                                    }
                                }
                                OracleExecuteResult::Complete
                            }
                            ScriptedResponse::Failed(class) => OracleExecuteResult::Failed(class),
                        };
                    }
                }
            }
            for index in 0..self.default_rows {
                if receiver.receive(OracleRow {
                    values: vec![OracleValue::Number(index.to_string())],
                }) == OracleRowControl::Stop
                {
                    break;
                }
            }
            OracleExecuteResult::Complete
        }

        fn cancel(&mut self) -> OracleCancelOutcome {
            self.cancel_calls += 1;
            self.cancel_outcome
        }

        fn elapsed_ms(&self) -> u64 {
            self.clock_ms.get()
        }
    }

    fn negotiated_for(
        server_major: u16,
        tier: OracleCaptureTier,
        detail: ArtifactDetail,
        cancellation: OracleCancellation,
        limits: OracleProviderLimits,
    ) -> OracleNegotiatedCapabilities {
        test_negotiated_capabilities(
            OracleProviderKind::Oci,
            Some(server_major),
            tier,
            detail,
            cancellation,
            limits,
        )
    }

    fn negotiated(
        tier: OracleCaptureTier,
        detail: ArtifactDetail,
        cancellation: OracleCancellation,
        limits: OracleProviderLimits,
    ) -> OracleNegotiatedCapabilities {
        negotiated_for(23, tier, detail, cancellation, limits)
    }

    fn roomy_limits() -> OracleProviderLimits {
        OracleProviderLimits {
            catalog_rows: 1_000_000,
            catalog_bytes: 1_000_000_000,
            result_nesting_depth: 4,
            elapsed_ms: 1_000_000,
            transient_value_bytes: 1_000_000,
            lob_bytes: 1_000_000,
        }
    }

    fn outcome<'a>(capture: &'a OracleCatalogCapture, query_id: &str) -> &'a OracleQueryOutcome {
        capture
            .outcomes
            .iter()
            .find(|outcome| outcome.query_id == query_id)
            .expect("query outcome present")
    }

    #[test]
    fn basic_capture_executes_every_query_and_binds_the_owner() {
        let mut adapter = ScriptedAdapter::new();
        let capture = run_catalog_capture(
            &negotiated(
                OracleCaptureTier::Basic,
                ArtifactDetail::None,
                OracleCancellation::ServerConfirmed,
                roomy_limits(),
            ),
            &mut adapter,
            "APP_OWNER",
        );
        assert!(capture.abort.is_none());
        assert!(capture
            .outcomes
            .iter()
            .all(|o| matches!(o.status, OracleQueryStatus::Executed { .. })));
        // Owner-scoped queries bind the owner; unscoped ones must not.
        assert!(adapter
            .executed_owners
            .iter()
            .any(|owner| owner.as_deref() == Some("APP_OWNER")));
        assert!(adapter.executed_owners.iter().any(Option::is_none));
        assert!(
            !adapter
                .executed_sql
                .iter()
                .any(|sql| sql.contains("APP_OWNER")),
            "the owner travels as a bind, never interpolated"
        );
        assert_eq!(capture.catalogs_read.len(), capture.outcomes.len());
        assert_eq!(
            adapter.executed_limits.first(),
            Some(&(1_000_000, 1_000_000_000, 1_000_000, 1_000_000, 4, 1_000_000)),
            "every adapter receives the negotiated allocation and deadline limits"
        );
        assert_eq!(
            adapter
                .executed_limits
                .get(1)
                .map(|limits| (limits.0, limits.1)),
            Some((999_999, 999_999_999)),
            "later statements receive remaining rather than reset budgets"
        );
    }

    #[test]
    fn multi_owner_capture_scopes_every_owner_query_and_runs_globals_once() {
        let scope = resolve_oracle_owner_scope(
            OracleOwnerSelection::Selected(vec!["B_OWNER".into(), "A_OWNER".into()]),
            "A_OWNER",
            ["A_OWNER".into(), "B_OWNER".into()],
        )
        .unwrap();
        let mut adapter = ScriptedAdapter::new();
        let capture = run_catalog_capture_for_scope(
            &negotiated(
                OracleCaptureTier::Basic,
                ArtifactDetail::None,
                OracleCancellation::ServerConfirmed,
                roomy_limits(),
            ),
            &mut adapter,
            &scope,
        );
        assert!(capture.abort.is_none());
        assert_eq!(
            capture
                .outcomes
                .iter()
                .filter(|outcome| outcome.query_id == "oracle-tables")
                .map(|outcome| outcome.owner_ordinal)
                .collect::<Vec<_>>(),
            vec![Some(1), Some(2)]
        );
        assert_eq!(
            capture
                .outcomes
                .iter()
                .filter(|outcome| outcome.query_id == "oracle-database")
                .count(),
            1,
            "database-wide catalogs must not be repeated for each owner"
        );
        assert_eq!(
            adapter
                .executed_owners
                .iter()
                .filter_map(|owner| owner.as_deref())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["A_OWNER", "B_OWNER"])
        );
        assert_eq!(
            capture.catalogs_read.len(),
            queries_for_tier(OracleCaptureTier::Basic)
                .map(|query| query.view)
                .collect::<BTreeSet<_>>()
                .len()
        );
        assert_eq!(capture.rows_consumed, adapter.executed_sql.len() as u64);
        assert!(adapter
            .executed_limits
            .windows(2)
            .all(|pair| pair[1].0 < pair[0].0 && pair[1].1 < pair[0].1));
    }

    #[test]
    fn floor_family_failure_is_evidenced_without_stopping_other_families() {
        let mut adapter = ScriptedAdapter::new();
        adapter.responses.insert(
            "oracle-tables",
            ScriptedResponse::Failed(OracleQueryFailure::PermissionDenied),
        );
        let capture = run_catalog_capture(
            &negotiated(
                OracleCaptureTier::Basic,
                ArtifactDetail::None,
                OracleCancellation::ServerConfirmed,
                roomy_limits(),
            ),
            &mut adapter,
            "APP_OWNER",
        );
        assert_eq!(capture.abort, None);
        assert!(matches!(
            outcome(&capture, "oracle-tables").status,
            OracleQueryStatus::Failed {
                class: OracleQueryFailure::PermissionDenied
            }
        ));
        assert!(matches!(
            outcome(&capture, "oracle-tab-columns").status,
            OracleQueryStatus::Executed { .. }
        ));
        assert!(capture.catalogs_unreadable.contains(&"DBA_TABLES"));
    }

    #[test]
    fn family_failures_degrade_and_the_capture_continues() {
        let mut adapter = ScriptedAdapter::new();
        adapter.responses.insert(
            "oracle-indexes",
            ScriptedResponse::Failed(OracleQueryFailure::PermissionDenied),
        );
        let capture = run_catalog_capture(
            &negotiated(
                OracleCaptureTier::Basic,
                ArtifactDetail::None,
                OracleCancellation::ServerConfirmed,
                roomy_limits(),
            ),
            &mut adapter,
            "APP_OWNER",
        );
        assert!(capture.abort.is_none());
        assert!(capture.catalogs_unreadable.contains(&"DBA_INDEXES"));
        assert!(matches!(
            outcome(&capture, "oracle-tab-statistics").status,
            OracleQueryStatus::Executed { .. }
        ));
    }

    #[test]
    fn option_absence_is_attributed_not_failed() {
        let mut adapter = ScriptedAdapter::new();
        adapter.responses.insert(
            "oracle-registry",
            ScriptedResponse::Rows(vec![OracleRow {
                values: vec![
                    OracleValue::Text("XDB".to_string()),
                    OracleValue::Text("VALID".to_string()),
                    OracleValue::Text("23.0".to_string()),
                ],
            }]),
        );
        adapter.responses.insert(
            "oracle-java-classes",
            ScriptedResponse::Failed(OracleQueryFailure::ObjectAbsent),
        );
        let capture = run_catalog_capture(
            &negotiated(
                OracleCaptureTier::Enhanced,
                ArtifactDetail::Analyzed,
                OracleCancellation::ServerConfirmed,
                roomy_limits(),
            ),
            &mut adapter,
            "APP_OWNER",
        );
        assert!(matches!(
            outcome(&capture, "oracle-java-classes").status,
            OracleQueryStatus::OptionAbsent {
                component: "JAVAVM"
            }
        ));
        // Attributed absence is not unreadability.
        assert!(!capture.catalogs_unreadable.contains(&"DBA_JAVA_CLASSES"));
        assert!(capture.abort.is_none());
    }

    #[test]
    fn option_absence_is_not_invented_when_registry_says_installed() {
        let mut adapter = ScriptedAdapter::new();
        adapter.responses.insert(
            "oracle-registry",
            ScriptedResponse::Rows(vec![OracleRow {
                values: vec![
                    OracleValue::Text("JAVAVM".to_string()),
                    OracleValue::Text("VALID".to_string()),
                    OracleValue::Text("23.0".to_string()),
                ],
            }]),
        );
        adapter.responses.insert(
            "oracle-java-classes",
            ScriptedResponse::Failed(OracleQueryFailure::ObjectAbsent),
        );
        let capture = run_catalog_capture(
            &negotiated(
                OracleCaptureTier::Enhanced,
                ArtifactDetail::Analyzed,
                OracleCancellation::ServerConfirmed,
                roomy_limits(),
            ),
            &mut adapter,
            "APP_OWNER",
        );
        assert!(matches!(
            outcome(&capture, "oracle-java-classes").status,
            OracleQueryStatus::Failed {
                class: OracleQueryFailure::ObjectAbsent
            }
        ));
        assert!(capture.catalogs_unreadable.contains(&"DBA_JAVA_CLASSES"));
    }

    #[test]
    fn option_absence_is_not_invented_without_readable_registry_evidence() {
        let mut adapter = ScriptedAdapter::new();
        adapter.responses.insert(
            "oracle-registry",
            ScriptedResponse::Failed(OracleQueryFailure::PermissionDenied),
        );
        adapter.responses.insert(
            "oracle-java-classes",
            ScriptedResponse::Failed(OracleQueryFailure::ObjectAbsent),
        );
        let capture = run_catalog_capture(
            &negotiated(
                OracleCaptureTier::Enhanced,
                ArtifactDetail::Analyzed,
                OracleCancellation::ServerConfirmed,
                roomy_limits(),
            ),
            &mut adapter,
            "APP_OWNER",
        );
        assert!(matches!(
            outcome(&capture, "oracle-java-classes").status,
            OracleQueryStatus::Failed {
                class: OracleQueryFailure::ObjectAbsent
            }
        ));
    }

    #[test]
    fn version_gates_skip_and_never_render_newer_columns() {
        let mut adapter = ScriptedAdapter::new();
        let capture = run_catalog_capture(
            &negotiated_for(
                11,
                OracleCaptureTier::Basic,
                ArtifactDetail::None,
                OracleCancellation::ServerConfirmed,
                roomy_limits(),
            ),
            &mut adapter,
            "APP_OWNER",
        );
        assert!(matches!(
            outcome(&capture, "oracle-identity-columns").status,
            OracleQueryStatus::SkippedVersion
        ));
        assert!(!adapter.executed_sql.iter().any(|sql| sql.contains("cdb")));
        assert!(!adapter
            .executed_sql
            .iter()
            .any(|sql| sql.contains("DBA_TAB_IDENTITY_COLS")));
    }

    #[test]
    fn definitions_are_never_requested_below_analyzed_detail() {
        let mut adapter = ScriptedAdapter::new();
        let capture = run_catalog_capture(
            &negotiated(
                OracleCaptureTier::Enhanced,
                ArtifactDetail::Graph,
                OracleCancellation::ServerConfirmed,
                roomy_limits(),
            ),
            &mut adapter,
            "APP_OWNER",
        );
        for query_id in crate::oracle_catalog::CATALOG_QUERIES
            .iter()
            .filter(|query| query.definition_bearing)
            .map(|query| query.query_id)
        {
            assert!(matches!(
                outcome(&capture, query_id).status,
                OracleQueryStatus::SkippedDetail
            ));
        }
        assert!(!adapter
            .executed_sql
            .iter()
            .any(|sql| sql.contains("DBA_SOURCE")));
        // The object inventory those kinds live in still ran.
        assert!(matches!(
            outcome(&capture, "oracle-objects").status,
            OracleQueryStatus::Executed { .. }
        ));
    }

    #[test]
    fn row_limit_breach_cancels_and_discards_the_overrun() {
        let mut adapter = ScriptedAdapter::new();
        adapter
            .responses
            .insert("oracle-tab-columns", ScriptedAdapter::rows(50));
        let mut limits = roomy_limits();
        limits.catalog_rows = 30;
        let capture = run_catalog_capture(
            &negotiated(
                OracleCaptureTier::Basic,
                ArtifactDetail::None,
                OracleCancellation::ServerConfirmed,
                limits,
            ),
            &mut adapter,
            "APP_OWNER",
        );
        assert_eq!(capture.abort, Some(OracleCaptureAbort::RowLimitExceeded));
        assert_eq!(adapter.cancel_calls, 1);
        let overrun = outcome(&capture, "oracle-tab-columns");
        assert!(
            matches!(
                overrun.status,
                OracleQueryStatus::ResultDiscarded {
                    reason: OracleCaptureAbort::RowLimitExceeded
                }
            ),
            "the query executed; saying otherwise misdescribes the source"
        );
        assert!(
            overrun.rows.is_empty(),
            "an overrun result must be discarded"
        );
    }

    #[test]
    fn delivered_transient_value_overrun_stops_and_discards_the_capture() {
        let mut adapter = ScriptedAdapter::new();
        adapter.responses.insert(
            "oracle-users",
            ScriptedResponse::Rows(vec![OracleRow {
                values: vec![OracleValue::Text("12345".to_string())],
            }]),
        );
        let mut limits = roomy_limits();
        limits.transient_value_bytes = 4;
        let capture = run_catalog_capture(
            &negotiated(
                OracleCaptureTier::Basic,
                ArtifactDetail::None,
                OracleCancellation::ServerConfirmed,
                limits,
            ),
            &mut adapter,
            "APP_OWNER",
        );
        assert_eq!(
            capture.abort,
            Some(OracleCaptureAbort::TransientValueLimitExceeded)
        );
        assert_eq!(adapter.cancel_calls, 1);
        assert!(matches!(
            outcome(&capture, "oracle-users").status,
            OracleQueryStatus::ResultDiscarded {
                reason: OracleCaptureAbort::TransientValueLimitExceeded
            }
        ));
    }

    #[test]
    fn provider_classified_long_value_degrades_an_optional_family() {
        let mut adapter = ScriptedAdapter::new();
        adapter.responses.insert(
            "oracle-users",
            ScriptedResponse::Failed(OracleQueryFailure::TransientValueLimitExceeded),
        );
        let capture = run_catalog_capture(
            &negotiated(
                OracleCaptureTier::Basic,
                ArtifactDetail::None,
                OracleCancellation::ServerConfirmed,
                roomy_limits(),
            ),
            &mut adapter,
            "APP_OWNER",
        );
        assert_eq!(capture.abort, None);
        assert_eq!(adapter.cancel_calls, 0);
        assert!(matches!(
            outcome(&capture, "oracle-users").status,
            OracleQueryStatus::Failed {
                class: OracleQueryFailure::TransientValueLimitExceeded
            }
        ));
        assert!(capture
            .outcomes
            .iter()
            .skip_while(|outcome| outcome.query_id != "oracle-users")
            .skip(1)
            .any(|outcome| matches!(outcome.status, OracleQueryStatus::Executed { .. })));
    }

    #[test]
    fn cumulative_lob_limit_is_independent_of_the_catalog_byte_limit() {
        let mut adapter = ScriptedAdapter::new();
        adapter.responses.insert(
            "oracle-users",
            ScriptedResponse::Rows(vec![
                OracleRow {
                    values: vec![OracleValue::LobText("123".to_string())],
                },
                OracleRow {
                    values: vec![OracleValue::LobText("456".to_string())],
                },
            ]),
        );
        let mut limits = roomy_limits();
        limits.lob_bytes = 5;
        let capture = run_catalog_capture(
            &negotiated(
                OracleCaptureTier::Basic,
                ArtifactDetail::None,
                OracleCancellation::ServerConfirmed,
                limits,
            ),
            &mut adapter,
            "APP_OWNER",
        );
        assert_eq!(capture.abort, Some(OracleCaptureAbort::LobLimitExceeded));
        assert_eq!(capture.rows_consumed, 2);
        assert_eq!(capture.bytes_consumed, 6);
        assert_eq!(capture.lob_bytes_consumed, 6);
        assert!(outcome(&capture, "oracle-users").rows.is_empty());
    }

    #[test]
    fn adapter_timeout_is_a_capture_deadline_not_a_degraded_family() {
        let mut adapter = ScriptedAdapter::new();
        adapter.responses.insert(
            "oracle-users",
            ScriptedResponse::Failed(OracleQueryFailure::Timeout),
        );
        let capture = run_catalog_capture(
            &negotiated(
                OracleCaptureTier::Basic,
                ArtifactDetail::None,
                OracleCancellation::ServerConfirmed,
                roomy_limits(),
            ),
            &mut adapter,
            "APP_OWNER",
        );
        assert_eq!(capture.abort, Some(OracleCaptureAbort::DeadlineExceeded));
        assert_eq!(adapter.cancel_calls, 1);
        assert!(matches!(
            outcome(&capture, "oracle-version").status,
            OracleQueryStatus::NotReached
        ));
    }

    #[test]
    fn a_regressing_adapter_clock_invalidates_and_discards_the_session() {
        let mut adapter = ScriptedAdapter::new();
        adapter.clock_ms.set(10);
        adapter.regress_on_execute = true;
        let capture = run_catalog_capture(
            &negotiated(
                OracleCaptureTier::Basic,
                ArtifactDetail::None,
                OracleCancellation::ServerConfirmed,
                roomy_limits(),
            ),
            &mut adapter,
            "APP_OWNER",
        );
        assert_eq!(
            capture.abort,
            Some(OracleCaptureAbort::AdapterContractViolated)
        );
        assert!(capture.session_discarded);
        assert_eq!(adapter.cancel_calls, 0);
        assert!(!capture.unconfirmed_server_work);
    }

    #[test]
    fn completed_statement_crossing_deadline_is_discarded_without_false_cancellation() {
        let mut adapter = ScriptedAdapter::new();
        adapter.clock_step_ms = 400;
        let mut limits = roomy_limits();
        limits.elapsed_ms = 1_000;
        let capture = run_catalog_capture(
            &negotiated(
                OracleCaptureTier::Basic,
                ArtifactDetail::None,
                OracleCancellation::ServerConfirmed,
                limits,
            ),
            &mut adapter,
            "APP_OWNER",
        );
        assert_eq!(capture.abort, Some(OracleCaptureAbort::DeadlineExceeded));
        assert!(capture
            .outcomes
            .iter()
            .any(|o| matches!(o.status, OracleQueryStatus::Executed { .. })));
        assert!(capture
            .outcomes
            .iter()
            .any(|o| matches!(o.status, OracleQueryStatus::NotReached)));
        assert_eq!(adapter.cancel_calls, 0);
        assert!(capture.session_discarded);
        assert!(!capture.unconfirmed_server_work);
    }

    #[test]
    fn deadline_exhausted_before_launch_does_not_cancel_or_invent_server_work() {
        let mut adapter = ScriptedAdapter::new();
        adapter.clock_ms.set(1_000);
        let mut limits = roomy_limits();
        limits.elapsed_ms = 1_000;
        let capture = run_catalog_capture(
            &negotiated(
                OracleCaptureTier::Basic,
                ArtifactDetail::None,
                OracleCancellation::LocalProcessOnly,
                limits,
            ),
            &mut adapter,
            "APP_OWNER",
        );
        assert_eq!(capture.abort, Some(OracleCaptureAbort::DeadlineExceeded));
        assert_eq!(adapter.cancel_calls, 0);
        assert!(!capture.session_discarded);
        assert!(!capture.unconfirmed_server_work);
        assert!(adapter.executed_sql.is_empty());
    }

    #[test]
    fn promised_server_cancellation_must_actually_confirm() {
        let mut adapter = ScriptedAdapter::new();
        adapter.cancel_outcome = OracleCancelOutcome::Unconfirmed;
        adapter
            .responses
            .insert("oracle-users", ScriptedAdapter::rows(50));
        let mut limits = roomy_limits();
        limits.catalog_rows = 10;
        let capture = run_catalog_capture(
            &negotiated(
                OracleCaptureTier::Basic,
                ArtifactDetail::None,
                OracleCancellation::ServerConfirmed,
                limits,
            ),
            &mut adapter,
            "APP_OWNER",
        );
        assert_eq!(
            capture.abort,
            Some(OracleCaptureAbort::UnconfirmedCancellation)
        );
        assert!(capture.session_discarded);
    }

    #[test]
    fn local_process_cancellation_records_unconfirmed_server_work() {
        let mut adapter = ScriptedAdapter::new();
        adapter
            .responses
            .insert("oracle-users", ScriptedAdapter::rows(50));
        let mut limits = roomy_limits();
        limits.catalog_rows = 10;
        let capture = run_catalog_capture(
            &negotiated(
                OracleCaptureTier::Basic,
                ArtifactDetail::None,
                OracleCancellation::LocalProcessOnly,
                limits,
            ),
            &mut adapter,
            "APP_OWNER",
        );
        assert_eq!(capture.abort, Some(OracleCaptureAbort::RowLimitExceeded));
        assert!(capture.session_discarded);
        assert!(capture.unconfirmed_server_work);
    }

    #[test]
    fn malformed_adapter_output_aborts_and_discards_the_session() {
        let mut adapter = ScriptedAdapter::new();
        adapter.responses.insert(
            "oracle-indexes",
            ScriptedResponse::Failed(OracleQueryFailure::Malformed),
        );
        let capture = run_catalog_capture(
            &negotiated(
                OracleCaptureTier::Basic,
                ArtifactDetail::None,
                OracleCancellation::LocalProcessOnly,
                roomy_limits(),
            ),
            &mut adapter,
            "APP_OWNER",
        );
        assert_eq!(
            capture.abort,
            Some(OracleCaptureAbort::AdapterContractViolated)
        );
        assert!(capture.session_discarded);
        assert!(matches!(
            outcome(&capture, "oracle-tab-statistics").status,
            OracleQueryStatus::NotReached
        ));
    }

    #[test]
    fn row_counter_overflow_is_an_adapter_contract_violation() {
        let limits = roomy_limits();
        let mut rows = BoundedRows {
            rows: Vec::new(),
            query_rows: 0,
            rows_consumed: u64::MAX,
            bytes_consumed: 0,
            lob_bytes_consumed: 0,
            limits,
            abort: None,
        };
        assert_eq!(
            rows.receive(OracleRow { values: Vec::new() }),
            OracleRowControl::Stop
        );
        assert_eq!(
            rows.abort,
            Some(OracleCaptureAbort::AdapterContractViolated)
        );
    }

    #[test]
    fn byte_counter_overflow_is_an_adapter_contract_violation() {
        let limits = roomy_limits();
        let mut rows = BoundedRows {
            rows: Vec::new(),
            query_rows: 0,
            rows_consumed: 0,
            bytes_consumed: u64::MAX,
            lob_bytes_consumed: 0,
            limits,
            abort: None,
        };
        assert_eq!(
            rows.receive(OracleRow {
                values: vec![OracleValue::Text("x".to_string())],
            }),
            OracleRowControl::Stop
        );
        assert_eq!(
            rows.abort,
            Some(OracleCaptureAbort::AdapterContractViolated)
        );
    }

    #[test]
    fn an_unattested_proof_never_drives_a_session() {
        let mut adapter = ScriptedAdapter::new();
        let caps = test_negotiated_capabilities(
            OracleProviderKind::Oci,
            None,
            OracleCaptureTier::Basic,
            ArtifactDetail::None,
            OracleCancellation::ServerConfirmed,
            roomy_limits(),
        );
        let capture = run_catalog_capture(&caps, &mut adapter, "APP_OWNER");
        assert_eq!(capture.abort, Some(OracleCaptureAbort::NoLiveSession));
        assert!(
            adapter.executed_sql.is_empty(),
            "nothing may reach the adapter"
        );
        assert!(capture
            .outcomes
            .iter()
            .all(|o| matches!(o.status, OracleQueryStatus::NotReached)));
    }

    #[test]
    fn an_offline_capability_set_never_drives_a_session() {
        let mut adapter = ScriptedAdapter::new();
        let caps = test_negotiated_capabilities(
            OracleProviderKind::Offline,
            Some(23),
            OracleCaptureTier::Basic,
            ArtifactDetail::None,
            OracleCancellation::ServerConfirmed,
            roomy_limits(),
        );
        let capture = run_catalog_capture(&caps, &mut adapter, "APP_OWNER");
        assert_eq!(capture.abort, Some(OracleCaptureAbort::NoLiveSession));
        assert!(adapter.executed_sql.is_empty());
    }

    #[test]
    fn a_lost_session_stops_everything_and_discards_it() {
        let mut adapter = ScriptedAdapter::new();
        adapter.responses.insert(
            "oracle-nls",
            ScriptedResponse::Failed(OracleQueryFailure::SessionLost),
        );
        let capture = run_catalog_capture(
            &negotiated(
                OracleCaptureTier::Basic,
                ArtifactDetail::None,
                OracleCancellation::ServerConfirmed,
                roomy_limits(),
            ),
            &mut adapter,
            "APP_OWNER",
        );
        assert_eq!(capture.abort, Some(OracleCaptureAbort::SessionLost));
        assert!(capture.session_discarded);
        assert!(matches!(
            outcome(&capture, "oracle-version").status,
            OracleQueryStatus::NotReached
        ));
    }
}
