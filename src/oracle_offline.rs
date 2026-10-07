//! Versioned, checksummed Oracle Basic catalogue capture stream.
//!
//! The stream carries an Oracle Basic capture created by the SQL*Plus path and
//! consumed by offline Basic mapping. It contains native catalogue names
//! and therefore remains a customer-local sensitive input; only the mapped,
//! anonymised Blueprint is publishable.  Decoding replays no SQL and accepts
//! no query contract other than the exact registry compiled into this build.
//! Checksums detect accidental corruption and bind the file to that registry;
//! they are not signatures and do not make an operator-writable file authentic.

#![allow(dead_code)]

use crate::artifacts::ArtifactDetail;
use crate::oracle_catalog::{queries_for_tier, Degradation, Presence};
use crate::oracle_dba_spool::decode_oracle_basic_dba_spool_reader;
use crate::oracle_provider::{
    OracleCaptureTier, OracleClientVersionAttestation, OracleProviderLimits, OracleVersion,
};
use crate::oracle_scope::{OracleOwnerScope, OracleOwnerScopeKind};
use crate::oracle_session::{
    OracleCaptureAbort, OracleCatalogCapture, OracleQueryFailure, OracleQueryOutcome,
    OracleQueryStatus, OracleRow, OracleValue,
};
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;

pub const ORACLE_BASIC_OFFLINE_STREAM_VERSION: u16 = 1;
pub const ORACLE_BASIC_QUERY_PACK_VERSION: u16 = 1;
// The file bound is derived from the maximum capture limits rather than an
// unrelated serialization guess. Compact tuple values add at most a bounded
// amount of structural JSON per captured catalogue row; the fixed allowance
// covers query outcomes, owner scope, and envelope metadata even when queries
// return no rows.
const MAX_ENCODED_ROW_OVERHEAD_BYTES: u64 = 512;
const MAX_STREAM_FIXED_OVERHEAD_BYTES: u64 = 64 * 1024 * 1024;
// A single UTF-8 source byte can require six JSON bytes when escaped as
// `\u00XX`. This is an encoding bound, not an expectation about Oracle text.
const MAX_JSON_ESCAPED_BYTE_EXPANSION: u64 = 6;
pub const MAX_ORACLE_BASIC_OFFLINE_STREAM_BYTES: u64 = MAX_CAPTURE_VALUE_BYTES
    * MAX_JSON_ESCAPED_BYTE_EXPANSION
    + MAX_CAPTURE_ROWS * MAX_ENCODED_ROW_OVERHEAD_BYTES
    + MAX_STREAM_FIXED_OVERHEAD_BYTES;

const STREAM_CONTRACT: &str = "dbwarp-blueprint-oracle-basic-capture/v1";
const MAX_CAPTURE_ROWS: u64 = 1_000_000;
const MAX_CAPTURE_VALUE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_SCALAR_BYTES: usize = 1_000;
const MAX_OWNER_COUNT: usize = 16_384;

struct DigestingReader<R> {
    inner: R,
    digest: Sha256,
    bytes_read: u64,
}

impl<R> DigestingReader<R> {
    fn new(inner: R) -> Self {
        Self {
            inner,
            digest: Sha256::new(),
            bytes_read: 0,
        }
    }

    fn digest(&self) -> [u8; 32] {
        self.digest.clone().finalize().into()
    }
}

impl<R: Read> Read for DigestingReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let read = self.inner.read(buffer)?;
        self.digest.update(&buffer[..read]);
        self.bytes_read = self.bytes_read.saturating_add(read as u64);
        Ok(read)
    }
}

#[derive(Debug)]
pub struct DecodedOracleBasicCapture {
    pub capture: OracleCatalogCapture,
    pub scope: OracleOwnerScope,
    pub server_version: OracleVersion,
    pub artifact_detail: ArtifactDetail,
    pub limits: OracleProviderLimits,
    pub query_pack_sha256: [u8; 32],
    pub stream_sha256: [u8; 32],
    pub captured_at: String,
    pub capturing_principal: String,
    pub database_identity: String,
    pub recorded_client_version: Option<OracleVersion>,
    pub input_kind: OracleBasicOfflineInputKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OracleBasicOfflineInputKind {
    LiveStream,
    DbaSpool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StreamEnvelope {
    contract: String,
    stream_version: u16,
    payload_sha256: String,
    payload: StreamPayload,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StreamPayload {
    query_pack_version: u16,
    query_pack_sha256: String,
    server_version: Vec<u16>,
    artifact_detail: String,
    captured_at: String,
    capturing_principal: String,
    database_identity: String,
    owner_scope: WireOwnerScope,
    limits: WireLimits,
    capture: WireCapture,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireOwnerScope {
    kind: String,
    owners: Vec<String>,
    omitted_unrepresentable_owners: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireLimits {
    catalog_rows: u64,
    catalog_bytes: u64,
    result_nesting_depth: u32,
    elapsed_ms: u64,
    transient_value_bytes: u64,
    lob_bytes: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireCapture {
    outcomes: Vec<WireOutcome>,
    catalogs_read: Vec<String>,
    catalogs_unreadable: Vec<String>,
    abort: Option<WireAbort>,
    session_discarded: bool,
    unconfirmed_server_work: bool,
    rows_consumed: u64,
    bytes_consumed: u64,
    lob_bytes_consumed: u64,
    elapsed_ms: u64,
    client_version_attestation: Option<WireClientAttestation>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireOutcome {
    query_id: String,
    view: String,
    owner_ordinal: Option<u32>,
    status: WireStatus,
    rows: Vec<WireRow>,
}

#[derive(Debug, Serialize, Deserialize)]
struct WireRow(Vec<WireValue>);

/// Compact, exact-width `[tag, value]` representation. The tuple shape rejects
/// unknown object keys and avoids repeating JSON field names for every cell.
#[derive(Debug, Serialize, Deserialize)]
struct WireValue(String, Option<String>);

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
enum WireStatus {
    Executed { rows: u64 },
    ResultDiscarded { reason: WireAbort },
    Failed { class: WireFailure },
    OptionAbsent { component: String },
    SkippedVersion,
    SkippedDetail,
    NotReached,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum WireFailure {
    PermissionDenied,
    ObjectAbsent,
    DatabaseError,
    TransportLimitExceeded,
    TransientValueLimitExceeded,
    Timeout,
    SessionLost,
    Malformed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum WireAbort {
    CoreInventoryUnavailable,
    OwnerScopeUnavailable,
    NoLiveSession,
    RowLimitExceeded,
    ByteLimitExceeded,
    TransientValueLimitExceeded,
    LobLimitExceeded,
    DeadlineExceeded,
    SessionLost,
    AdapterContractViolated,
    UnconfirmedCancellation,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
enum WireClientAttestation {
    Attested { version: Vec<u16> },
    Mismatched,
    Unreadable,
}

pub fn write_oracle_basic_stream(
    path: &Path,
    capture: &OracleCatalogCapture,
    scope: &OracleOwnerScope,
    server_version: OracleVersion,
    artifact_detail: ArtifactDetail,
    limits: OracleProviderLimits,
    captured_at: &str,
    capturing_principal: &str,
    database_identity: &str,
) -> Result<(u64, String)> {
    let payload = StreamPayload::from_capture(
        capture,
        scope,
        server_version,
        artifact_detail,
        limits,
        captured_at,
        capturing_principal,
        database_identity,
    );
    let payload_bytes =
        serde_json::to_vec(&payload).context("DBP1505E encoding Oracle Basic offline payload")?;
    let payload_digest = Sha256::digest(&payload_bytes);
    let envelope = StreamEnvelope {
        contract: STREAM_CONTRACT.to_string(),
        stream_version: ORACLE_BASIC_OFFLINE_STREAM_VERSION,
        payload_sha256: hex::encode(payload_digest),
        payload,
    };
    let mut bytes =
        serde_json::to_vec(&envelope).context("DBP1505E encoding Oracle Basic offline stream")?;
    bytes.push(b'\n');
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_stream_bytes(limits) {
        bail!("DBP1505E Oracle Basic offline stream exceeds its bounded file size");
    }
    // Exercise the same decoder before publishing so the live provider cannot
    // write a stream that the offline provider compiled beside it rejects.
    decode_oracle_basic_bytes(&bytes).context("DBP1505E self-validating Oracle Basic stream")?;
    atomic_write(path, &bytes)?;
    let digest = hex::encode(Sha256::digest(&bytes));
    Ok((bytes.len() as u64, digest))
}

pub fn read_oracle_basic_stream(path: &Path) -> Result<DecodedOracleBasicCapture> {
    let file = File::open(path)
        .with_context(|| format!("DBP1505E opening Oracle Basic stream {}", path.display()))?;
    validate_private_stream_file(&file, path)?;
    let stream_bytes = file
        .metadata()
        .context("DBP1505E inspecting Oracle Basic stream length")?
        .len();
    if stream_bytes > MAX_ORACLE_BASIC_OFFLINE_STREAM_BYTES {
        bail!("DBP1505E Oracle Basic offline stream exceeds its bounded file size");
    }
    let mut reader = BufReader::new(DigestingReader::new(file));
    let is_dba_spool = reader
        .fill_buf()
        .context("DBP1505E reading Oracle Basic stream header")?
        .starts_with(b"DBWARP_BP|H|");
    if is_dba_spool {
        let spool = decode_oracle_basic_dba_spool_reader(&mut reader)?;
        if reader.get_ref().bytes_read != stream_bytes {
            bail!("DBP1505E Oracle Basic DBA spool length changed while it was read");
        }
        let query_pack_sha256 = query_pack_sha256(spool.server_version, spool.artifact_detail);
        return Ok(DecodedOracleBasicCapture {
            capture: spool.capture,
            scope: spool.scope,
            server_version: spool.server_version,
            artifact_detail: spool.artifact_detail,
            limits: spool.limits,
            query_pack_sha256,
            stream_sha256: reader.get_ref().digest(),
            captured_at: spool.captured_at,
            capturing_principal: spool.capturing_principal,
            database_identity: spool.database_identity,
            recorded_client_version: spool.recorded_client_version,
            input_kind: OracleBasicOfflineInputKind::DbaSpool,
        });
    }
    let mut bytes = Vec::new();
    std::io::Read::by_ref(&mut reader)
        .take(MAX_ORACLE_BASIC_OFFLINE_STREAM_BYTES + 1)
        .read_to_end(&mut bytes)
        .with_context(|| format!("DBP1505E reading Oracle Basic stream {}", path.display()))?;
    if bytes.len() as u64 > MAX_ORACLE_BASIC_OFFLINE_STREAM_BYTES {
        bail!("DBP1505E Oracle Basic offline stream exceeds its bounded file size");
    }
    decode_oracle_basic_bytes(&bytes)
}

fn decode_oracle_basic_bytes(bytes: &[u8]) -> Result<DecodedOracleBasicCapture> {
    let stream_sha256: [u8; 32] = Sha256::digest(bytes).into();
    let envelope: StreamEnvelope = serde_json::from_slice(bytes)
        .map_err(|_| anyhow!("DBP1505E parsing Oracle Basic offline stream failed"))?;
    if envelope.contract != STREAM_CONTRACT
        || envelope.stream_version != ORACLE_BASIC_OFFLINE_STREAM_VERSION
    {
        bail!("DBP1505E unsupported Oracle Basic offline stream contract");
    }
    let payload_bytes = serde_json::to_vec(&envelope.payload)
        .context("DBP1505E canonicalizing Oracle Basic offline payload")?;
    let expected_payload = decode_sha256(&envelope.payload_sha256, "payload")?;
    if Sha256::digest(&payload_bytes).as_slice() != expected_payload {
        bail!("DBP1505E Oracle Basic offline payload checksum mismatch");
    }
    envelope.payload.decode(stream_sha256, bytes.len() as u64)
}

impl StreamPayload {
    fn from_capture(
        capture: &OracleCatalogCapture,
        scope: &OracleOwnerScope,
        server_version: OracleVersion,
        artifact_detail: ArtifactDetail,
        limits: OracleProviderLimits,
        captured_at: &str,
        capturing_principal: &str,
        database_identity: &str,
    ) -> Self {
        Self {
            query_pack_version: ORACLE_BASIC_QUERY_PACK_VERSION,
            query_pack_sha256: hex::encode(query_pack_sha256(server_version, artifact_detail)),
            server_version: server_version.components().to_vec(),
            artifact_detail: artifact_detail.as_str().to_string(),
            captured_at: captured_at.to_string(),
            capturing_principal: capturing_principal.to_string(),
            database_identity: database_identity.to_string(),
            owner_scope: WireOwnerScope {
                kind: scope_kind_label(scope.kind()).to_string(),
                owners: scope.owners().to_vec(),
                omitted_unrepresentable_owners: scope.omitted_unrepresentable_owners(),
            },
            limits: WireLimits::from(limits),
            capture: WireCapture::from(capture),
        }
    }

    fn decode(
        self,
        stream_sha256: [u8; 32],
        stream_bytes: u64,
    ) -> Result<DecodedOracleBasicCapture> {
        if self.query_pack_version != ORACLE_BASIC_QUERY_PACK_VERSION {
            bail!("DBP1505E unsupported Oracle Basic query-pack version");
        }
        let server_version = OracleVersion::from_components(&self.server_version)
            .ok_or_else(|| anyhow!("DBP1505E invalid Oracle server version in offline stream"))?;
        let artifact_detail = match self.artifact_detail.as_str() {
            "none" => ArtifactDetail::None,
            "summary" => ArtifactDetail::Summary,
            _ => {
                bail!("DBP1505E offline Oracle Basic stream carries an unsupported artifact detail")
            }
        };
        if self.captured_at.is_empty()
            || self.captured_at.len() > 128
            || self.captured_at.chars().any(char::is_control)
            || chrono::DateTime::parse_from_rfc3339(&self.captured_at).is_err()
        {
            bail!("DBP1505E invalid Oracle capture timestamp in offline stream");
        }
        validate_capture_identity(&self.capturing_principal, 128)?;
        validate_capture_identity(&self.database_identity, 512)?;
        let query_pack_sha256 = decode_sha256(&self.query_pack_sha256, "query pack")?;
        if query_pack_sha256 != query_pack_sha256_for(server_version, artifact_detail) {
            bail!("DBP1505E Oracle Basic query-pack checksum does not match this build");
        }
        let kind = match self.owner_scope.kind.as_str() {
            "authenticated-owner" => OracleOwnerScopeKind::AuthenticatedOwner,
            "selected-owners" => OracleOwnerScopeKind::SelectedOwners,
            "all-visible-owners" => OracleOwnerScopeKind::AllVisibleOwners,
            _ => bail!("DBP1505E invalid owner-scope kind in Oracle Basic stream"),
        };
        if self.owner_scope.owners.len() > MAX_OWNER_COUNT {
            bail!("DBP1505E Oracle Basic stream exceeds its owner-count bound");
        }
        if !self
            .owner_scope
            .owners
            .windows(2)
            .all(|pair| pair[0] < pair[1])
        {
            bail!("DBP1505E Oracle Basic owner list is not sorted and unique");
        }
        if kind == OracleOwnerScopeKind::AuthenticatedOwner && self.owner_scope.owners.len() != 1 {
            bail!("DBP1505E authenticated Oracle owner scope must contain exactly one owner");
        }
        let scope = OracleOwnerScope::from_offline_capture(
            self.owner_scope.owners,
            kind,
            self.owner_scope.omitted_unrepresentable_owners,
        )
        .map_err(|reason| anyhow!("DBP1505E invalid Oracle owner scope: {reason:?}"))?;
        let limits = self.limits.decode()?;
        if stream_bytes > max_stream_bytes(limits) {
            bail!("DBP1505E Oracle Basic offline stream exceeds its captured file-size bound");
        }
        let capture = self
            .capture
            .decode(&scope, server_version, artifact_detail, limits)?;
        let recorded_client_version = capture_recorded_client_version(&capture);
        Ok(DecodedOracleBasicCapture {
            capture,
            scope,
            server_version,
            artifact_detail,
            limits,
            query_pack_sha256,
            stream_sha256,
            captured_at: self.captured_at,
            capturing_principal: self.capturing_principal,
            database_identity: self.database_identity,
            recorded_client_version,
            input_kind: OracleBasicOfflineInputKind::LiveStream,
        })
    }
}

fn validate_capture_identity(value: &str, max_bytes: usize) -> Result<()> {
    if value.is_empty()
        || value.len() > max_bytes
        || value.chars().any(|character| character.is_control())
    {
        bail!("DBP1505E invalid Oracle capture identity in offline stream");
    }
    Ok(())
}

fn capture_recorded_client_version(capture: &OracleCatalogCapture) -> Option<OracleVersion> {
    capture
        .client_version_attestation
        .and_then(OracleClientVersionAttestation::attested_version)
}

fn max_stream_bytes(limits: OracleProviderLimits) -> u64 {
    limits
        .catalog_bytes
        .saturating_mul(MAX_JSON_ESCAPED_BYTE_EXPANSION)
        .saturating_add(
            limits
                .catalog_rows
                .saturating_mul(MAX_ENCODED_ROW_OVERHEAD_BYTES),
        )
        .saturating_add(MAX_STREAM_FIXED_OVERHEAD_BYTES)
        .min(MAX_ORACLE_BASIC_OFFLINE_STREAM_BYTES)
}

impl From<OracleProviderLimits> for WireLimits {
    fn from(value: OracleProviderLimits) -> Self {
        Self {
            catalog_rows: value.catalog_rows,
            catalog_bytes: value.catalog_bytes,
            result_nesting_depth: value.result_nesting_depth,
            elapsed_ms: value.elapsed_ms,
            transient_value_bytes: value.transient_value_bytes,
            lob_bytes: value.lob_bytes,
        }
    }
}

impl WireLimits {
    fn decode(self) -> Result<OracleProviderLimits> {
        if self.catalog_rows == 0
            || self.catalog_rows > MAX_CAPTURE_ROWS
            || self.catalog_bytes == 0
            || self.catalog_bytes > MAX_CAPTURE_VALUE_BYTES
            || self.result_nesting_depth == 0
            || self.result_nesting_depth > 2
            || self.elapsed_ms == 0
            || self.elapsed_ms > 10 * 60 * 1_000
            || self.transient_value_bytes == 0
            || self.transient_value_bytes > MAX_SCALAR_BYTES as u64
            || self.lob_bytes == 0
            || self.lob_bytes > self.catalog_bytes
        {
            bail!("DBP1505E Oracle Basic stream carries values outside the allowed limits");
        }
        Ok(OracleProviderLimits {
            catalog_rows: self.catalog_rows,
            catalog_bytes: self.catalog_bytes,
            result_nesting_depth: self.result_nesting_depth,
            elapsed_ms: self.elapsed_ms,
            transient_value_bytes: self.transient_value_bytes,
            lob_bytes: self.lob_bytes,
        })
    }
}

impl WireCapture {
    fn decode(
        self,
        scope: &OracleOwnerScope,
        server_version: OracleVersion,
        artifact_detail: ArtifactDetail,
        limits: OracleProviderLimits,
    ) -> Result<OracleCatalogCapture> {
        let queries = queries_for_tier(OracleCaptureTier::Basic).collect::<Vec<_>>();
        let expected = queries
            .iter()
            .flat_map(|query| {
                if query.owner_column.is_some() {
                    (1..=scope.owners().len())
                        .map(|ordinal| (*query, Some(ordinal as u32)))
                        .collect::<Vec<_>>()
                } else {
                    vec![(*query, None)]
                }
            })
            .collect::<Vec<_>>();
        if self.outcomes.len() != expected.len() {
            bail!("DBP1505E Oracle Basic stream has the wrong query-outcome count");
        }
        let mut retained_rows = 0_u64;
        let mut retained_bytes = 0_u64;
        let mut retained_lob_bytes = 0_u64;
        let mut outcomes = Vec::with_capacity(expected.len());
        let mut reached_abort_boundary = false;
        let mut has_discarded_rows = false;
        let mut aborting_row_bytes_cap = 0_u64;
        for (wire, (query, owner_ordinal)) in self.outcomes.into_iter().zip(expected) {
            if wire.query_id != query.query_id
                || wire.view != query.view
                || wire.owner_ordinal != owner_ordinal
            {
                bail!("DBP1505E Oracle Basic stream does not match query-pack ordering");
            }
            let status = wire.status.decode(query.presence)?;
            if reached_abort_boundary && !matches!(status, OracleQueryStatus::NotReached) {
                bail!("DBP1505E Oracle Basic stream resumed after an abort boundary");
            }
            if matches!(
                status,
                OracleQueryStatus::NotReached
                    | OracleQueryStatus::ResultDiscarded { .. }
                    | OracleQueryStatus::Failed {
                        class: OracleQueryFailure::Timeout
                            | OracleQueryFailure::SessionLost
                            | OracleQueryFailure::Malformed
                    }
            ) {
                reached_abort_boundary = true;
            }
            has_discarded_rows |= matches!(
                status,
                OracleQueryStatus::Failed { .. } | OracleQueryStatus::ResultDiscarded { .. }
            );
            let row_count = u64::try_from(wire.rows.len())
                .map_err(|_| anyhow!("DBP1505E Oracle Basic row count overflow"))?;
            match status {
                OracleQueryStatus::Executed { rows } if rows == row_count => {}
                OracleQueryStatus::Executed { .. } => {
                    bail!("DBP1505E Oracle Basic executed-row count is inconsistent")
                }
                _ if wire.rows.is_empty() => {}
                _ => bail!("DBP1505E a non-executed Oracle query retained rows"),
            }
            if !matches!(status, OracleQueryStatus::NotReached)
                && !query.applies_to(server_version.components()[0])
                && !matches!(status, OracleQueryStatus::SkippedVersion)
            {
                bail!("DBP1505E Oracle Basic stream executed a version-inapplicable query");
            }
            if !matches!(status, OracleQueryStatus::NotReached)
                && query.applies_to(server_version.components()[0])
                && matches!(status, OracleQueryStatus::SkippedVersion)
            {
                bail!("DBP1505E Oracle Basic stream skipped an applicable query by version");
            }
            if matches!(status, OracleQueryStatus::SkippedDetail)
                && (!query.definition_bearing
                    || !matches!(
                        artifact_detail,
                        ArtifactDetail::None | ArtifactDetail::Summary
                    ))
            {
                bail!("DBP1505E Oracle Basic stream has an invalid detail skip");
            }
            let width = query.output_columns_for_version(server_version).len();
            if matches!(
                status,
                OracleQueryStatus::ResultDiscarded { .. }
                    | OracleQueryStatus::Failed {
                        class: OracleQueryFailure::Timeout
                            | OracleQueryFailure::SessionLost
                            | OracleQueryFailure::Malformed
                    }
            ) {
                aborting_row_bytes_cap =
                    (width as u64).saturating_mul(limits.transient_value_bytes.saturating_add(1));
            }
            let mut rows = Vec::with_capacity(wire.rows.len());
            for row in wire.rows {
                if row.0.len() != width {
                    bail!("DBP1505E Oracle Basic row width does not match the query pack");
                }
                let mut values = Vec::with_capacity(width);
                for value in row.0 {
                    let (decoded, bytes, lob_bytes) = value.decode(limits)?;
                    retained_bytes = retained_bytes
                        .checked_add(bytes)
                        .ok_or_else(|| anyhow!("DBP1505E Oracle Basic byte counter overflow"))?;
                    retained_lob_bytes = retained_lob_bytes
                        .checked_add(lob_bytes)
                        .ok_or_else(|| anyhow!("DBP1505E Oracle Basic LOB counter overflow"))?;
                    values.push(decoded);
                }
                rows.push(OracleRow { values });
            }
            retained_rows = retained_rows
                .checked_add(row_count)
                .ok_or_else(|| anyhow!("DBP1505E Oracle Basic row counter overflow"))?;
            outcomes.push(OracleQueryOutcome {
                query_id: query.query_id,
                view: query.view,
                owner_ordinal,
                status,
                rows,
            });
        }
        let accounted_bytes = retained_bytes;
        if retained_rows > self.rows_consumed
            || accounted_bytes > self.bytes_consumed
            || retained_lob_bytes > self.lob_bytes_consumed
        {
            bail!("DBP1505E Oracle Basic retained data exceeds its capture counters");
        }
        if !has_discarded_rows
            && (retained_rows != self.rows_consumed
                || accounted_bytes != self.bytes_consumed
                || retained_lob_bytes != self.lob_bytes_consumed)
        {
            bail!("DBP1505E Oracle Basic capture counters do not match retained rows");
        }
        let abort = self.abort.map(WireAbort::decode);
        if outcomes.iter().any(|outcome| {
            matches!(
                outcome.status,
                OracleQueryStatus::ResultDiscarded { reason }
                    if Some(reason) != abort
            )
        }) {
            bail!("DBP1505E discarded Oracle results do not match the capture abort reason");
        }
        if outcomes.iter().any(|outcome| match outcome.status {
            OracleQueryStatus::Failed {
                class: OracleQueryFailure::Malformed,
            } => abort != Some(OracleCaptureAbort::AdapterContractViolated),
            OracleQueryStatus::Failed {
                class: OracleQueryFailure::Timeout,
            } => !matches!(
                abort,
                Some(
                    OracleCaptureAbort::DeadlineExceeded
                        | OracleCaptureAbort::UnconfirmedCancellation
                )
            ),
            OracleQueryStatus::Failed {
                class: OracleQueryFailure::SessionLost,
            } => abort != Some(OracleCaptureAbort::SessionLost),
            _ => false,
        }) {
            bail!("DBP1505E Oracle failure class does not match the capture abort reason");
        }
        let has_abort_status = outcomes.iter().any(|outcome| {
            matches!(
                outcome.status,
                OracleQueryStatus::NotReached | OracleQueryStatus::ResultDiscarded { .. }
            ) || matches!(
                outcome.status,
                OracleQueryStatus::Failed {
                    class: OracleQueryFailure::Timeout
                        | OracleQueryFailure::SessionLost
                        | OracleQueryFailure::Malformed
                }
            )
        });
        if abort.is_some() != has_abort_status {
            bail!("DBP1505E Oracle Basic abort evidence is inconsistent with query outcomes");
        }
        validate_capture_counters(
            self.rows_consumed,
            self.bytes_consumed,
            self.lob_bytes_consumed,
            self.elapsed_ms,
            limits,
            abort,
            aborting_row_bytes_cap,
        )?;
        if self.unconfirmed_server_work && (!self.session_discarded || abort.is_none()) {
            bail!("DBP1505E unconfirmed Oracle work requires a discarded, aborted session");
        }
        let catalogs_read = decode_catalogs(self.catalogs_read)?;
        let catalogs_unreadable = decode_catalogs(self.catalogs_unreadable)?;
        let expected_read = catalogue_set_for(&outcomes, true);
        let expected_unreadable = catalogue_set_for(&outcomes, false);
        if catalogs_read.iter().copied().collect::<BTreeSet<_>>() != expected_read
            || catalogs_unreadable.iter().copied().collect::<BTreeSet<_>>() != expected_unreadable
        {
            bail!("DBP1505E Oracle Basic catalogue provenance does not match query outcomes");
        }
        Ok(OracleCatalogCapture {
            outcomes,
            catalogs_read,
            catalogs_unreadable,
            abort,
            session_discarded: self.session_discarded,
            unconfirmed_server_work: self.unconfirmed_server_work,
            rows_consumed: self.rows_consumed,
            bytes_consumed: self.bytes_consumed,
            lob_bytes_consumed: self.lob_bytes_consumed,
            elapsed_ms: self.elapsed_ms,
            client_version_attestation: self
                .client_version_attestation
                .map(WireClientAttestation::decode)
                .transpose()?,
        })
    }
}

fn validate_capture_counters(
    rows: u64,
    bytes: u64,
    lob_bytes: u64,
    elapsed_ms: u64,
    limits: OracleProviderLimits,
    abort: Option<OracleCaptureAbort>,
    aborting_row_bytes_cap: u64,
) -> Result<()> {
    let rows_valid = match abort {
        Some(OracleCaptureAbort::RowLimitExceeded) => rows == limits.catalog_rows.saturating_add(1),
        Some(OracleCaptureAbort::UnconfirmedCancellation) => {
            rows <= limits.catalog_rows.saturating_add(1)
        }
        _ => rows <= limits.catalog_rows,
    };
    let bytes_valid = match abort {
        Some(OracleCaptureAbort::ByteLimitExceeded) => {
            bytes > limits.catalog_bytes
                && bytes <= limits.catalog_bytes.saturating_add(aborting_row_bytes_cap)
        }
        Some(
            OracleCaptureAbort::TransientValueLimitExceeded
            | OracleCaptureAbort::UnconfirmedCancellation,
        ) => bytes <= limits.catalog_bytes.saturating_add(aborting_row_bytes_cap),
        _ => bytes <= limits.catalog_bytes,
    };
    let lob_bytes_valid = match abort {
        Some(OracleCaptureAbort::LobLimitExceeded) => {
            lob_bytes > limits.lob_bytes
                && lob_bytes <= limits.lob_bytes.saturating_add(aborting_row_bytes_cap)
        }
        Some(
            OracleCaptureAbort::TransientValueLimitExceeded
            | OracleCaptureAbort::UnconfirmedCancellation,
        ) => lob_bytes <= limits.lob_bytes.saturating_add(aborting_row_bytes_cap),
        _ => lob_bytes <= limits.lob_bytes,
    };
    let elapsed_valid = match abort {
        Some(OracleCaptureAbort::DeadlineExceeded) => elapsed_ms >= limits.elapsed_ms,
        Some(OracleCaptureAbort::UnconfirmedCancellation) => true,
        _ => elapsed_ms <= limits.elapsed_ms,
    };
    if !rows_valid || !bytes_valid || !lob_bytes_valid || !elapsed_valid {
        bail!("DBP1505E Oracle Basic stream counters exceed the bounds justified by its abort");
    }
    Ok(())
}

impl From<&OracleCatalogCapture> for WireCapture {
    fn from(value: &OracleCatalogCapture) -> Self {
        Self {
            outcomes: value
                .outcomes
                .iter()
                .map(WireOutcome::from_capture)
                .collect(),
            catalogs_read: value
                .catalogs_read
                .iter()
                .map(|v| (*v).to_string())
                .collect(),
            catalogs_unreadable: value
                .catalogs_unreadable
                .iter()
                .map(|v| (*v).to_string())
                .collect(),
            abort: value.abort.map(WireAbort::from),
            session_discarded: value.session_discarded,
            unconfirmed_server_work: value.unconfirmed_server_work,
            rows_consumed: value.rows_consumed,
            bytes_consumed: value.bytes_consumed,
            lob_bytes_consumed: value.lob_bytes_consumed,
            elapsed_ms: value.elapsed_ms,
            client_version_attestation: value
                .client_version_attestation
                .map(WireClientAttestation::from),
        }
    }
}

impl WireOutcome {
    fn from_capture(value: &OracleQueryOutcome) -> Self {
        Self {
            query_id: value.query_id.to_string(),
            view: value.view.to_string(),
            owner_ordinal: value.owner_ordinal,
            status: WireStatus::from(&value.status),
            rows: value.rows.iter().map(WireRow::from_capture).collect(),
        }
    }
}

impl WireRow {
    fn from_capture(value: &OracleRow) -> Self {
        Self(value.values.iter().map(WireValue::from).collect())
    }
}

impl From<&OracleValue> for WireValue {
    fn from(value: &OracleValue) -> Self {
        match value {
            OracleValue::Null => Self::null(),
            OracleValue::Number(value) => Self("d".to_string(), Some(value.clone())),
            OracleValue::Text(value) => Self("t".to_string(), Some(value.clone())),
            OracleValue::Timestamp(value) => Self("s".to_string(), Some(value.clone())),
            OracleValue::LobText(value) => Self("l".to_string(), Some(value.clone())),
        }
    }
}

impl WireValue {
    fn null() -> Self {
        Self("n".to_string(), None)
    }

    fn decode(self, limits: OracleProviderLimits) -> Result<(OracleValue, u64, u64)> {
        let (value, is_lob) = match (self.0.as_str(), self.1) {
            ("n", None) => return Ok((OracleValue::Null, 0, 0)),
            ("d", Some(value)) => (OracleValue::Number(value), false),
            ("t", Some(value)) => (OracleValue::Text(value), false),
            ("s", Some(value)) => (OracleValue::Timestamp(value), false),
            ("l", Some(value)) => (OracleValue::LobText(value), true),
            _ => bail!("DBP1505E Oracle Basic stream carries an invalid value cell"),
        };
        let bytes = match &value {
            OracleValue::Null => 0,
            OracleValue::Number(value)
            | OracleValue::Text(value)
            | OracleValue::Timestamp(value)
            | OracleValue::LobText(value) => value.len() as u64,
        };
        if bytes > limits.transient_value_bytes || (is_lob && bytes > limits.lob_bytes) {
            bail!("DBP1505E Oracle Basic value exceeds the captured scalar limits");
        }
        Ok((value, bytes, if is_lob { bytes } else { 0 }))
    }
}

impl From<&OracleQueryStatus> for WireStatus {
    fn from(value: &OracleQueryStatus) -> Self {
        match value {
            OracleQueryStatus::Executed { rows } => Self::Executed { rows: *rows },
            OracleQueryStatus::ResultDiscarded { reason } => Self::ResultDiscarded {
                reason: WireAbort::from(*reason),
            },
            OracleQueryStatus::Failed { class } => Self::Failed {
                class: WireFailure::from(*class),
            },
            OracleQueryStatus::OptionAbsent { component } => Self::OptionAbsent {
                component: (*component).to_string(),
            },
            OracleQueryStatus::SkippedVersion => Self::SkippedVersion,
            OracleQueryStatus::SkippedDetail => Self::SkippedDetail,
            OracleQueryStatus::NotReached => Self::NotReached,
        }
    }
}

impl WireStatus {
    fn decode(self, presence: Presence) -> Result<OracleQueryStatus> {
        Ok(match self {
            Self::Executed { rows } => OracleQueryStatus::Executed { rows },
            Self::ResultDiscarded { reason } => OracleQueryStatus::ResultDiscarded {
                reason: reason.decode(),
            },
            Self::Failed { class } => OracleQueryStatus::Failed {
                class: class.decode(),
            },
            Self::OptionAbsent { component } => match presence {
                Presence::OptionComponent(expected) if component == expected => {
                    OracleQueryStatus::OptionAbsent {
                        component: expected,
                    }
                }
                _ => bail!("DBP1505E invalid option-absence evidence in Oracle Basic stream"),
            },
            Self::SkippedVersion => OracleQueryStatus::SkippedVersion,
            Self::SkippedDetail => OracleQueryStatus::SkippedDetail,
            Self::NotReached => OracleQueryStatus::NotReached,
        })
    }
}

macro_rules! enum_wire_conversions {
    ($wire:ty, $native:ty, {$($variant:ident),+ $(,)?}) => {
        impl From<$native> for $wire {
            fn from(value: $native) -> Self {
                match value { $(<$native>::$variant => <$wire>::$variant,)+ }
            }
        }
        impl $wire {
            fn decode(self) -> $native {
                match self { $(Self::$variant => <$native>::$variant,)+ }
            }
        }
    };
}

enum_wire_conversions!(WireFailure, OracleQueryFailure, {
    PermissionDenied, ObjectAbsent, DatabaseError, TransportLimitExceeded,
    TransientValueLimitExceeded, Timeout, SessionLost, Malformed
});
enum_wire_conversions!(WireAbort, OracleCaptureAbort, {
    CoreInventoryUnavailable, OwnerScopeUnavailable, NoLiveSession,
    RowLimitExceeded, ByteLimitExceeded, TransientValueLimitExceeded,
    LobLimitExceeded, DeadlineExceeded, SessionLost, AdapterContractViolated,
    UnconfirmedCancellation
});

impl From<OracleClientVersionAttestation> for WireClientAttestation {
    fn from(value: OracleClientVersionAttestation) -> Self {
        match value {
            OracleClientVersionAttestation::Attested(version) => Self::Attested {
                version: version.components().to_vec(),
            },
            OracleClientVersionAttestation::Mismatched => Self::Mismatched,
            OracleClientVersionAttestation::Unreadable => Self::Unreadable,
        }
    }
}

impl WireClientAttestation {
    fn decode(self) -> Result<OracleClientVersionAttestation> {
        Ok(match self {
            Self::Attested { version } => OracleClientVersionAttestation::Attested(
                OracleVersion::from_components(&version).ok_or_else(|| {
                    anyhow!("DBP1505E invalid client version in Oracle Basic stream")
                })?,
            ),
            Self::Mismatched => OracleClientVersionAttestation::Mismatched,
            Self::Unreadable => OracleClientVersionAttestation::Unreadable,
        })
    }
}

fn catalogue_set_for(outcomes: &[OracleQueryOutcome], read: bool) -> BTreeSet<&'static str> {
    outcomes
        .iter()
        .filter(|outcome| {
            if read {
                matches!(outcome.status, OracleQueryStatus::Executed { .. })
            } else {
                matches!(outcome.status, OracleQueryStatus::Failed { .. })
            }
        })
        .map(|outcome| outcome.view)
        .collect()
}

fn decode_catalogs(values: Vec<String>) -> Result<Vec<&'static str>> {
    if !values.windows(2).all(|pair| pair[0] < pair[1]) {
        bail!("DBP1505E Oracle Basic catalogue list is not sorted and unique");
    }
    values
        .into_iter()
        .map(|value| {
            queries_for_tier(OracleCaptureTier::Basic)
                .find(|query| query.view == value)
                .map(|query| query.view)
                .ok_or_else(|| anyhow!("DBP1505E Oracle Basic stream names an unknown catalogue"))
        })
        .collect()
}

fn scope_kind_label(kind: OracleOwnerScopeKind) -> &'static str {
    match kind {
        OracleOwnerScopeKind::AuthenticatedOwner => "authenticated-owner",
        OracleOwnerScopeKind::SelectedOwners => "selected-owners",
        OracleOwnerScopeKind::AllVisibleOwners => "all-visible-owners",
    }
}

fn query_pack_sha256_for(
    server_version: OracleVersion,
    artifact_detail: ArtifactDetail,
) -> [u8; 32] {
    query_pack_sha256(server_version, artifact_detail)
}

pub fn query_pack_sha256(
    server_version: OracleVersion,
    artifact_detail: ArtifactDetail,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash_field(&mut hash, b"dbwarp-blueprint-oracle-basic-query-pack/v1");
    for component in server_version.components() {
        hash_field(&mut hash, &component.to_be_bytes());
    }
    hash_field(&mut hash, artifact_detail.as_str().as_bytes());
    for query in queries_for_tier(OracleCaptureTier::Basic) {
        hash_field(&mut hash, query.query_id.as_bytes());
        hash_field(&mut hash, query.view.as_bytes());
        hash_field(&mut hash, query.owner_column.unwrap_or("").as_bytes());
        hash_field(&mut hash, query.fixed_predicate.unwrap_or("").as_bytes());
        hash_field(&mut hash, &[u8::from(query.definition_bearing)]);
        match query.presence {
            Presence::Always => hash_field(&mut hash, b"always"),
            Presence::FromMajor(major) => {
                hash_field(&mut hash, b"from-major");
                hash_field(&mut hash, &major.to_be_bytes());
            }
            Presence::OptionComponent(component) => {
                hash_field(&mut hash, b"option-component");
                hash_field(&mut hash, component.as_bytes());
            }
        }
        hash_field(
            &mut hash,
            match query.degradation {
                Degradation::Fatal => b"fatal",
                Degradation::DegradeFamily => b"degrade-family",
            },
        );
        hash_field(
            &mut hash,
            query.render_sql_for_version(server_version).as_bytes(),
        );
        for column in query.output_columns_for_version(server_version) {
            hash_field(&mut hash, column.as_bytes());
        }
    }
    hash.finalize().into()
}

fn hash_field(hash: &mut Sha256, value: &[u8]) {
    hash.update((value.len() as u64).to_be_bytes());
    hash.update(value);
}

fn decode_sha256(value: &str, label: &str) -> Result<[u8; 32]> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("DBP1505E invalid Oracle Basic {label} checksum");
    }
    let decoded = hex::decode(value).context("DBP1505E decoding Oracle Basic checksum")?;
    decoded
        .try_into()
        .map_err(|_| anyhow!("DBP1505E invalid Oracle Basic {label} checksum width"))
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    if let Some(parent) = parent {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "DBP1505E creating Oracle stream directory {}",
                parent.display()
            )
        })?;
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| anyhow!("DBP1505E Oracle stream output path has no safe file name"))?;
    let temporary = path.with_file_name(format!(".{file_name}.partial-{}", std::process::id()));
    let result = (|| -> Result<()> {
        let mut options = File::options();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary).with_context(|| {
            format!(
                "DBP1505E creating temporary Oracle stream {}",
                temporary.display()
            )
        })?;
        file.write_all(bytes)
            .context("DBP1505E writing Oracle Basic offline stream")?;
        file.sync_all()
            .context("DBP1505E syncing Oracle Basic offline stream")?;
        drop(file);
        std::fs::rename(&temporary, path)
            .with_context(|| format!("DBP1505E publishing Oracle stream {}", path.display()))?;
        #[cfg(not(unix))]
        crate::secret::check_sensitive_file_mode(path, "Oracle Basic offline stream")
            .context("DBP1505E checking Oracle Basic offline stream ACL policy")?;
        if let Some(parent) = parent {
            sync_directory(parent)?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

fn validate_private_stream_file(file: &File, path: &Path) -> Result<()> {
    let metadata = file
        .metadata()
        .with_context(|| format!("DBP1505E inspecting Oracle stream {}", path.display()))?;
    if !metadata.is_file() {
        bail!("DBP1505E Oracle Basic offline stream is not a regular file");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        // SAFETY: geteuid has no pointer arguments or preconditions.
        let effective_uid = unsafe { libc::geteuid() };
        if metadata.uid() != effective_uid || metadata.permissions().mode() & 0o077 != 0 {
            bail!(
                "DBP1505E Oracle Basic offline stream must be owned by the current user and mode 0600"
            );
        }
    }
    #[cfg(not(unix))]
    crate::secret::check_sensitive_file_mode(path, "Oracle Basic offline stream")
        .context("DBP1505E checking Oracle Basic offline stream ACL policy")?;
    Ok(())
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<()> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .with_context(|| {
            format!(
                "DBP1505E syncing Oracle stream directory {}",
                path.display()
            )
        })
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oracle_scope::{resolve_oracle_owner_scope, OracleOwnerSelection};

    const CAPTURED_AT: &str = "2026-09-28T00:00:00Z";
    const CAPTURING_PRINCIPAL: &str = "COLLECTOR";
    const DATABASE_IDENTITY: &str = "TESTDB";

    fn scope() -> OracleOwnerScope {
        resolve_oracle_owner_scope(
            OracleOwnerSelection::Selected(vec!["APP".to_string()]),
            "COLLECTOR",
            ["APP".to_string()],
        )
        .unwrap()
    }

    fn limits() -> OracleProviderLimits {
        OracleProviderLimits {
            catalog_rows: 1_000,
            catalog_bytes: 1_000_000,
            result_nesting_depth: 2,
            elapsed_ms: 60_000,
            transient_value_bytes: 1_000,
            lob_bytes: 1,
        }
    }

    fn empty_success_capture(version: OracleVersion) -> OracleCatalogCapture {
        let mut outcomes = Vec::new();
        let mut catalogs = BTreeSet::new();
        for query in queries_for_tier(OracleCaptureTier::Basic) {
            let ordinals = if query.owner_column.is_some() {
                vec![Some(1)]
            } else {
                vec![None]
            };
            for owner_ordinal in ordinals {
                let status = if query.applies_to(version.components()[0]) {
                    catalogs.insert(query.view);
                    OracleQueryStatus::Executed { rows: 0 }
                } else {
                    OracleQueryStatus::SkippedVersion
                };
                outcomes.push(OracleQueryOutcome {
                    query_id: query.query_id,
                    view: query.view,
                    owner_ordinal,
                    status,
                    rows: Vec::new(),
                });
            }
        }
        OracleCatalogCapture {
            outcomes,
            catalogs_read: catalogs.into_iter().collect(),
            catalogs_unreadable: Vec::new(),
            abort: None,
            session_discarded: false,
            unconfirmed_server_work: false,
            rows_consumed: 0,
            bytes_consumed: 0,
            lob_bytes_consumed: 0,
            elapsed_ms: 1,
            client_version_attestation: Some(OracleClientVersionAttestation::Attested(version)),
        }
    }

    fn aborted_capture(
        version: OracleVersion,
        reason: OracleCaptureAbort,
        rows_consumed: u64,
        bytes_consumed: u64,
        elapsed_ms: u64,
    ) -> OracleCatalogCapture {
        let mut capture = empty_success_capture(version);
        for outcome in &mut capture.outcomes {
            outcome.status = OracleQueryStatus::NotReached;
            outcome.rows.clear();
        }
        if reason != OracleCaptureAbort::DeadlineExceeded {
            capture.outcomes[0].status = OracleQueryStatus::ResultDiscarded { reason };
        }
        capture.catalogs_read.clear();
        capture.catalogs_unreadable.clear();
        capture.abort = Some(reason);
        capture.session_discarded = reason != OracleCaptureAbort::DeadlineExceeded;
        capture.unconfirmed_server_work = reason != OracleCaptureAbort::DeadlineExceeded;
        capture.rows_consumed = rows_consumed;
        capture.bytes_consumed = bytes_consumed;
        capture.elapsed_ms = elapsed_ms;
        capture
    }

    fn test_stream_path(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tmp/tests")
            .join(format!("oracle-offline-{name}-{}.json", std::process::id()))
    }

    #[test]
    fn query_pack_digest_changes_with_version_and_detail() {
        let v19 = OracleVersion::from_components(&[19, 3]).unwrap();
        let v21 = OracleVersion::from_components(&[21, 3]).unwrap();
        assert_ne!(
            query_pack_sha256(v19, ArtifactDetail::Summary),
            query_pack_sha256(v21, ArtifactDetail::Summary)
        );
        assert_ne!(
            query_pack_sha256(v19, ArtifactDetail::Summary),
            query_pack_sha256(v19, ArtifactDetail::None)
        );
    }

    #[test]
    fn canonical_stream_round_trips_the_complete_query_plan() {
        let version = OracleVersion::from_components(&[19, 3]).unwrap();
        let payload = StreamPayload::from_capture(
            &empty_success_capture(version),
            &scope(),
            version,
            ArtifactDetail::Summary,
            limits(),
            CAPTURED_AT,
            CAPTURING_PRINCIPAL,
            DATABASE_IDENTITY,
        );
        let payload_bytes = serde_json::to_vec(&payload).unwrap();
        let envelope = StreamEnvelope {
            contract: STREAM_CONTRACT.to_string(),
            stream_version: ORACLE_BASIC_OFFLINE_STREAM_VERSION,
            payload_sha256: hex::encode(Sha256::digest(&payload_bytes)),
            payload,
        };
        let bytes = serde_json::to_vec(&envelope).unwrap();
        let decoded = decode_oracle_basic_bytes(&bytes).unwrap();
        assert_eq!(decoded.scope.owners(), &["APP"]);
        assert_eq!(
            decoded.capture.outcomes.len(),
            queries_for_tier(OracleCaptureTier::Basic).count()
        );
        assert_eq!(decoded.capture.rows_consumed, 0);
        assert_eq!(decoded.captured_at, CAPTURED_AT);
        assert_eq!(decoded.capturing_principal, CAPTURING_PRINCIPAL);
        assert_eq!(decoded.database_identity, DATABASE_IDENTITY);
    }

    #[test]
    fn compact_bound_is_derived_from_the_full_capture_envelope() {
        assert!(MAX_ORACLE_BASIC_OFFLINE_STREAM_BYTES > 384 * 1024 * 1024);
        assert_eq!(
            max_stream_bytes(OracleProviderLimits {
                catalog_rows: MAX_CAPTURE_ROWS,
                catalog_bytes: MAX_CAPTURE_VALUE_BYTES,
                result_nesting_depth: 2,
                elapsed_ms: 10 * 60 * 1_000,
                transient_value_bytes: MAX_SCALAR_BYTES as u64,
                lob_bytes: MAX_CAPTURE_VALUE_BYTES,
            }),
            MAX_ORACLE_BASIC_OFFLINE_STREAM_BYTES
        );
        let widest = [11_u16, 12, 18, 19, 21, 23, 26]
            .into_iter()
            .flat_map(|major| {
                let version = OracleVersion::from_components(&[major, 0]).unwrap();
                queries_for_tier(OracleCaptureTier::Basic)
                    .map(move |query| query.output_columns_for_version(version).len())
            })
            .max()
            .unwrap();
        assert!(widest * 12 + 32 <= MAX_ENCODED_ROW_OVERHEAD_BYTES as usize);
    }

    #[test]
    fn counter_overrun_must_match_the_recorded_abort_dimension() {
        assert!(validate_capture_counters(
            limits().catalog_rows + 1,
            0,
            0,
            1,
            limits(),
            Some(OracleCaptureAbort::ByteLimitExceeded),
            1,
        )
        .is_err());
        assert!(validate_capture_counters(
            0,
            limits().catalog_bytes + 1,
            0,
            1,
            limits(),
            Some(OracleCaptureAbort::RowLimitExceeded),
            1,
        )
        .is_err());
        assert!(validate_capture_counters(
            limits().catalog_rows,
            0,
            0,
            1,
            limits(),
            Some(OracleCaptureAbort::RowLimitExceeded),
            1,
        )
        .is_err());
        assert!(validate_capture_counters(
            0,
            limits().catalog_bytes,
            0,
            1,
            limits(),
            Some(OracleCaptureAbort::ByteLimitExceeded),
            1,
        )
        .is_err());
        assert!(validate_capture_counters(
            0,
            0,
            limits().lob_bytes,
            1,
            limits(),
            Some(OracleCaptureAbort::LobLimitExceeded),
            1,
        )
        .is_err());
        assert!(validate_capture_counters(
            0,
            0,
            0,
            limits().elapsed_ms - 1,
            limits(),
            Some(OracleCaptureAbort::DeadlineExceeded),
            1,
        )
        .is_err());
    }

    #[cfg(unix)]
    #[test]
    fn bounded_abort_captures_round_trip_through_the_private_file() {
        let version = OracleVersion::from_components(&[19, 3]).unwrap();
        let cases = [
            (
                "row-limit",
                aborted_capture(
                    version,
                    OracleCaptureAbort::RowLimitExceeded,
                    limits().catalog_rows + 1,
                    0,
                    5,
                ),
            ),
            (
                "byte-limit",
                aborted_capture(
                    version,
                    OracleCaptureAbort::ByteLimitExceeded,
                    1,
                    limits().catalog_bytes + 1,
                    5,
                ),
            ),
            (
                "deadline",
                aborted_capture(
                    version,
                    OracleCaptureAbort::DeadlineExceeded,
                    0,
                    0,
                    limits().elapsed_ms + 1,
                ),
            ),
        ];
        std::fs::create_dir_all(Path::new(env!("CARGO_MANIFEST_DIR")).join("tmp/tests")).unwrap();
        for (name, capture) in cases {
            let path = test_stream_path(name);
            write_oracle_basic_stream(
                &path,
                &capture,
                &scope(),
                version,
                ArtifactDetail::Summary,
                limits(),
                CAPTURED_AT,
                CAPTURING_PRINCIPAL,
                DATABASE_IDENTITY,
            )
            .unwrap();
            let decoded = read_oracle_basic_stream(&path).unwrap();
            assert_eq!(decoded.capture.abort, capture.abort);
            assert_eq!(decoded.capture.rows_consumed, capture.rows_consumed);
            assert_eq!(decoded.capture.bytes_consumed, capture.bytes_consumed);
            assert_eq!(decoded.capture.elapsed_ms, capture.elapsed_ms);
            std::fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn unused_native_identity_columns_are_absent_from_the_query_pack() {
        let version = OracleVersion::from_components(&[19, 3]).unwrap();
        let columns = |query_id| {
            queries_for_tier(OracleCaptureTier::Basic)
                .find(|query| query.query_id == query_id)
                .unwrap()
                .output_columns_for_version(version)
        };
        assert_eq!(
            columns("oracle-database"),
            vec!["platform_name", "database_role", "open_mode", "cdb"]
        );
        assert_eq!(
            columns("oracle-instance"),
            vec!["version", "status", "parallel"]
        );
        assert_eq!(
            columns("oracle-containers"),
            vec!["con_id", "open_mode", "restricted"]
        );
    }

    #[test]
    fn compact_cells_and_tagged_statuses_reject_extra_shape() {
        assert!(serde_json::from_str::<WireValue>(r#"["n",null,"extra"]"#).is_err());
        assert!(serde_json::from_str::<WireStatus>(
            r#"{"executed":{"rows":0,"extra":"native-name"}}"#
        )
        .is_err());
    }

    #[test]
    fn checksum_tampering_is_rejected_before_mapping() {
        let version = OracleVersion::from_components(&[19, 3]).unwrap();
        let capture = OracleCatalogCapture {
            outcomes: Vec::new(),
            catalogs_read: Vec::new(),
            catalogs_unreadable: Vec::new(),
            abort: None,
            session_discarded: false,
            unconfirmed_server_work: false,
            rows_consumed: 0,
            bytes_consumed: 0,
            lob_bytes_consumed: 0,
            elapsed_ms: 1,
            client_version_attestation: None,
        };
        let mut payload = StreamPayload::from_capture(
            &capture,
            &scope(),
            version,
            ArtifactDetail::Summary,
            limits(),
            CAPTURED_AT,
            CAPTURING_PRINCIPAL,
            DATABASE_IDENTITY,
        );
        // The outcome list is deliberately malformed, but a valid checksum
        // would reach that independent validation. Altering the payload after
        // the checksum must instead stop at the checksum boundary.
        let original = serde_json::to_vec(&payload).unwrap();
        let digest = hex::encode(Sha256::digest(&original));
        payload.artifact_detail = "none".to_string();
        let envelope = StreamEnvelope {
            contract: STREAM_CONTRACT.to_string(),
            stream_version: ORACLE_BASIC_OFFLINE_STREAM_VERSION,
            payload_sha256: digest,
            payload,
        };
        let bytes = serde_json::to_vec(&envelope).unwrap();
        let error = decode_oracle_basic_bytes(&bytes).unwrap_err().to_string();
        assert!(error.contains("checksum mismatch"), "{error}");
    }

    #[test]
    fn offline_stream_cannot_resume_after_a_discarded_result() {
        let version = OracleVersion::from_components(&[19, 3]).unwrap();
        let mut wire = WireCapture::from(&empty_success_capture(version));
        wire.abort = Some(WireAbort::RowLimitExceeded);
        wire.outcomes[0].status = WireStatus::ResultDiscarded {
            reason: WireAbort::RowLimitExceeded,
        };
        wire.catalogs_read
            .retain(|view| view != &wire.outcomes[0].view);
        let error = wire
            .decode(&scope(), version, ArtifactDetail::Summary, limits())
            .unwrap_err()
            .to_string();
        assert!(error.contains("resumed after an abort boundary"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn native_offline_stream_is_private_at_write_and_read_boundaries() {
        use std::os::unix::fs::PermissionsExt;

        let version = OracleVersion::from_components(&[19, 3]).unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tmp/tests")
            .join(format!("oracle-offline-private-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("capture.json");
        write_oracle_basic_stream(
            &path,
            &empty_success_capture(version),
            &scope(),
            version,
            ArtifactDetail::Summary,
            limits(),
            CAPTURED_AT,
            CAPTURING_PRINCIPAL,
            DATABASE_IDENTITY,
        )
        .unwrap();
        assert_eq!(path.metadata().unwrap().permissions().mode() & 0o777, 0o600);
        read_oracle_basic_stream(&path).unwrap();

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let error = read_oracle_basic_stream(&path).unwrap_err().to_string();
        assert!(error.contains("mode 0600"), "{error}");
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
