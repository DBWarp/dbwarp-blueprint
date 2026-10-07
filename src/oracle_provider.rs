//! Fail-closed capability negotiation shared by Oracle connection providers.
//!
//! Oracle access can be supplied by a customer-installed OCI library, a local
//! command client, or a checksummed offline capture. Each adapter has different
//! typing, cancellation, transport, and local-file properties. This module
//! keeps those differences outside the catalog mapper: an adapter declares its
//! maximum capability envelope, proves the capabilities observed for the
//! current source, and receives a negotiated contract before capture begins.
//!
//! This module contains no Oracle client or database code. Adapters remain
//! responsible for producing the evidence that this boundary validates.

#![allow(dead_code)]

use std::collections::BTreeSet;

use crate::artifacts::ArtifactDetail;

pub const ORACLE_PROVIDER_CONTRACT_VERSION: u16 = 2;
pub const ORACLE_LIVE_PROBE_CONTRACT_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OracleProviderKind {
    Oci,
    Sqlcl,
    Sqlplus,
    Offline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OracleCaptureTier {
    Basic,
    Standard,
    Enhanced,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OracleEndpointMode {
    DirectService,
    Sid,
    Alias,
    Offline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OracleTransport {
    Tcp,
    TcpsVerifyFull,
    Offline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OracleAuthentication {
    Password,
    External,
    Wallet,
    Offline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OracleTrustSource {
    NotApplicable,
    SystemTrust,
    PemCaFile,
    Wallet,
}

/// Categories of local input an operator must authorize explicitly. Paths and
/// file contents are intentionally not part of the capability record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OracleLocalInput {
    ClientExecutable,
    ClientLibrary,
    OracleNetConfig,
    TrustAnchor,
    Wallet,
    OfflineCapture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum OracleCancellation {
    /// The provider confirms that database-side work was cancelled.
    ServerConfirmed,
    /// The session must be discarded because server cancellation is uncertain.
    SessionDiscardRequired,
    /// Only the local client process can be stopped; server work is unconfirmed.
    LocalProcessOnly,
    /// No database session exists, as with an offline capture.
    NotApplicable,
}

/// Numeric components only: never retain an Oracle product banner or
/// installation label in capability evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OracleVersion {
    components: [u16; 5],
    component_count: u8,
}

/// Provenance strength for the local Oracle command client.  This never
/// controls catalog feature selection or admission: a banner is supporting
/// provenance, not an authority over the database catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OracleClientVersionAttestation {
    Attested(OracleVersion),
    Mismatched,
    Unreadable,
}

impl OracleClientVersionAttestation {
    pub fn attested_version(self) -> Option<OracleVersion> {
        match self {
            Self::Attested(version) => Some(version),
            Self::Mismatched | Self::Unreadable => None,
        }
    }

    pub fn evidence_token(self) -> &'static str {
        match self {
            Self::Attested(_) => "attested",
            Self::Mismatched => "mismatched",
            Self::Unreadable => "unreadable",
        }
    }

    pub fn limitation_token(self) -> Option<&'static str> {
        match self {
            Self::Attested(version) if !version.is_at_least(12, 1) => {
                Some("oracle-client-version-below-tested-floor")
            }
            Self::Attested(_) => None,
            Self::Mismatched => Some("oracle-client-version-mismatch"),
            Self::Unreadable => Some("oracle-client-version-unreadable"),
        }
    }
}

impl OracleVersion {
    pub fn from_components(components: &[u16]) -> Option<Self> {
        if components.is_empty() || components.len() > 5 || components[0] == 0 {
            return None;
        }
        let mut normalized = [0; 5];
        normalized[..components.len()].copy_from_slice(components);
        Some(Self {
            components: normalized,
            component_count: components.len() as u8,
        })
    }

    pub fn components(&self) -> &[u16] {
        &self.components[..usize::from(self.component_count)]
    }

    pub fn is_at_least(&self, major: u16, minor: u16) -> bool {
        (self.components[0], self.components[1]) >= (major, minor)
    }
}

/// Hard limits enforced by the provider, not estimates of ordinary usage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OracleProviderLimits {
    pub catalog_rows: u64,
    pub catalog_bytes: u64,
    pub result_nesting_depth: u32,
    pub elapsed_ms: u64,
    pub transient_value_bytes: u64,
    pub lob_bytes: u64,
}

impl OracleProviderLimits {
    fn are_bounded(self) -> bool {
        self.catalog_rows > 0
            && self.catalog_bytes > 0
            && self.result_nesting_depth > 0
            && self.elapsed_ms > 0
            && self.transient_value_bytes > 0
            && self.lob_bytes > 0
            && self.transient_value_bytes <= self.catalog_bytes
            && self.lob_bytes <= self.catalog_bytes
    }

    fn fits_within(self, declared: Self) -> bool {
        self.catalog_rows <= declared.catalog_rows
            && self.catalog_bytes <= declared.catalog_bytes
            && self.result_nesting_depth <= declared.result_nesting_depth
            && self.elapsed_ms <= declared.elapsed_ms
            && self.transient_value_bytes <= declared.transient_value_bytes
            && self.lob_bytes <= declared.lob_bytes
    }
}

/// One connection shape that an adapter can establish and police as a unit.
///
/// These properties deliberately remain coupled. Recording them in independent
/// sets would incorrectly claim the Cartesian product of every endpoint,
/// transport, authentication and trust mode the adapter has ever exercised.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct OracleConnectionProfile {
    pub endpoint_mode: OracleEndpointMode,
    pub transport: OracleTransport,
    pub authentication: OracleAuthentication,
    pub trust_source: OracleTrustSource,
    pub cancellation: OracleCancellation,
    pub local_inputs: BTreeSet<OracleLocalInput>,
}

impl OracleConnectionProfile {
    fn matches_request(&self, request: &OracleCaptureRequest) -> bool {
        self.endpoint_mode == request.endpoint_mode
            && self.transport == request.transport
            && self.authentication == request.authentication
            && self.trust_source == request.trust_source
    }

    fn same_connection_shape(&self, other: &Self) -> bool {
        self.endpoint_mode == other.endpoint_mode
            && self.transport == other.transport
            && self.authentication == other.authentication
            && self.trust_source == other.trust_source
            && self.local_inputs == other.local_inputs
    }
}

/// Closed capability set. In `OracleProviderDeclaration` this is the adapter's
/// declared maximum; inside probe evidence it is what one current
/// client/server session actually proved and will enforce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleCapabilitySet {
    pub bind_variables: bool,
    pub typed_scalar_results: bool,
    pub limits: OracleProviderLimits,
    pub connection_profiles: BTreeSet<OracleConnectionProfile>,
    pub tiers: BTreeSet<OracleCaptureTier>,
    pub artifact_details: BTreeSet<ArtifactDetail>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleProviderDeclaration {
    pub contract_version: u16,
    pub provider: OracleProviderKind,
    pub maximum: OracleCapabilitySet,
}

/// Evidence from the exact live probe that established an observed capability
/// set. Hashes identify the exact operator-supplied provider artifact, sanitized
/// probe transcript and live session without retaining paths, descriptors, SQL
/// or banners. Client-version provenance may be explicitly weaker without
/// weakening the catalog session proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleLiveProbeReceipt {
    probe_contract_version: u16,
    client_version_attestation: OracleClientVersionAttestation,
    server_version: OracleVersion,
    /// Adapter-defined fingerprint of the operator-selected executable or
    /// client library. A provider must document whether it can attest the
    /// mapped process image or only a stable pre/post-launch path selection.
    /// The Blueprint binary has separate build provenance.
    provider_artifact_sha256: [u8; 32],
    transcript_sha256: [u8; 32],
    session_binding_sha256: [u8; 32],
}

impl OracleLiveProbeReceipt {
    fn new(
        client_version_attestation: OracleClientVersionAttestation,
        server_version: OracleVersion,
        provider_artifact_sha256: [u8; 32],
        transcript_sha256: [u8; 32],
        session_binding_sha256: [u8; 32],
    ) -> Self {
        Self {
            probe_contract_version: ORACLE_LIVE_PROBE_CONTRACT_VERSION,
            client_version_attestation,
            server_version,
            provider_artifact_sha256,
            transcript_sha256,
            session_binding_sha256,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleLiveProbeEvidence {
    receipt: OracleLiveProbeReceipt,
    observed: OracleCapabilitySet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleOfflineEvidence {
    query_pack_version: u16,
    query_pack_sha256: [u8; 32],
    server_version: OracleVersion,
    stream_sha256: [u8; 32],
    observed: OracleCapabilitySet,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OracleCapabilityProof {
    Unverified,
    LiveProbe(OracleLiveProbeEvidence),
    ChecksummedOffline(OracleOfflineEvidence),
}

impl OracleCapabilityProof {
    fn observed(&self) -> Option<&OracleCapabilitySet> {
        match self {
            Self::Unverified => None,
            Self::LiveProbe(proof) => Some(&proof.observed),
            Self::ChecksummedOffline(proof) => Some(&proof.observed),
        }
    }

    /// The server version this proof attested. The execution boundary gates
    /// version-dependent SQL on this, never on a caller-supplied number, which
    /// would be unverified self-assertion.
    pub fn attested_server_version(&self) -> Option<OracleVersion> {
        match self {
            Self::Unverified => None,
            Self::LiveProbe(proof) => Some(proof.receipt.server_version),
            Self::ChecksummedOffline(proof) => Some(proof.server_version),
        }
    }

    /// Numeric client version from live proof. Offline evidence has no client
    /// execution, and a mismatched or unreadable live attestation exposes no
    /// version claim.
    pub fn attested_client_version(&self) -> Option<OracleVersion> {
        match self {
            Self::LiveProbe(proof) => proof.receipt.client_version_attestation.attested_version(),
            Self::Unverified | Self::ChecksummedOffline(_) => None,
        }
    }

    /// Live command-client version provenance. Offline captures have no local
    /// command client, and unverified provider evidence exposes no claim.
    pub fn client_version_attestation(&self) -> Option<OracleClientVersionAttestation> {
        match self {
            Self::LiveProbe(proof) => Some(proof.receipt.client_version_attestation),
            Self::Unverified | Self::ChecksummedOffline(_) => None,
        }
    }

    /// Provider artifact fingerprint for audit provenance. Returning the digest,
    /// never a path, keeps operator-local coordinates out of evidence. The
    /// provider's own contract defines whether this attests the mapped process
    /// image or a stable launch-path selection.
    pub fn provider_artifact_sha256(&self) -> Option<[u8; 32]> {
        match self {
            Self::LiveProbe(proof) => Some(proof.receipt.provider_artifact_sha256),
            Self::Unverified | Self::ChecksummedOffline(_) => None,
        }
    }

    pub fn probe_transcript_sha256(&self) -> Option<[u8; 32]> {
        match self {
            Self::LiveProbe(proof) => Some(proof.receipt.transcript_sha256),
            Self::Unverified | Self::ChecksummedOffline(_) => None,
        }
    }

    pub fn session_binding_sha256(&self) -> Option<[u8; 32]> {
        match self {
            Self::LiveProbe(proof) => Some(proof.receipt.session_binding_sha256),
            Self::Unverified | Self::ChecksummedOffline(_) => None,
        }
    }

    pub fn offline_query_pack_version(&self) -> Option<u16> {
        match self {
            Self::ChecksummedOffline(proof) => Some(proof.query_pack_version),
            Self::Unverified | Self::LiveProbe(_) => None,
        }
    }

    pub fn offline_query_pack_sha256(&self) -> Option<[u8; 32]> {
        match self {
            Self::ChecksummedOffline(proof) => Some(proof.query_pack_sha256),
            Self::Unverified | Self::LiveProbe(_) => None,
        }
    }

    pub fn offline_stream_sha256(&self) -> Option<[u8; 32]> {
        match self {
            Self::ChecksummedOffline(proof) => Some(proof.stream_sha256),
            Self::Unverified | Self::LiveProbe(_) => None,
        }
    }
}

/// Probe evidence is intentionally opaque outside this module. Provider
/// adapters live below this module and create it only after completing their
/// fixed probe protocol; callers cannot pair a version-only marker with an
/// independently constructed capability set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleCapabilityEvidence {
    contract_version: u16,
    provider: OracleProviderKind,
    proof: OracleCapabilityProof,
}

impl OracleCapabilityEvidence {
    fn unverified(provider: OracleProviderKind) -> Self {
        Self {
            contract_version: ORACLE_PROVIDER_CONTRACT_VERSION,
            provider,
            proof: OracleCapabilityProof::Unverified,
        }
    }

    fn from_live_probe(
        provider: OracleProviderKind,
        receipt: OracleLiveProbeReceipt,
        observed: OracleCapabilitySet,
    ) -> Self {
        Self {
            contract_version: ORACLE_PROVIDER_CONTRACT_VERSION,
            provider,
            proof: OracleCapabilityProof::LiveProbe(OracleLiveProbeEvidence { receipt, observed }),
        }
    }

    fn from_offline_capture(
        server_version: OracleVersion,
        query_pack_version: u16,
        query_pack_sha256: [u8; 32],
        stream_sha256: [u8; 32],
        observed: OracleCapabilitySet,
    ) -> Self {
        Self {
            contract_version: ORACLE_PROVIDER_CONTRACT_VERSION,
            provider: OracleProviderKind::Offline,
            proof: OracleCapabilityProof::ChecksummedOffline(OracleOfflineEvidence {
                query_pack_version,
                query_pack_sha256,
                server_version,
                stream_sha256,
                observed,
            }),
        }
    }
}

/// Provider-adapter handoff after its fixed live probe has completed. This is
/// crate-private so a CLI caller cannot manufacture proof; the admission gate
/// still validates the returned capability shape and every nonzero receipt
/// binding before it can authorize a capture.
pub(crate) fn oracle_live_probe_evidence(
    provider: OracleProviderKind,
    client_version_attestation: OracleClientVersionAttestation,
    server_version: OracleVersion,
    provider_artifact_sha256: [u8; 32],
    transcript_sha256: [u8; 32],
    session_binding_sha256: [u8; 32],
    observed: OracleCapabilitySet,
) -> OracleCapabilityEvidence {
    OracleCapabilityEvidence::from_live_probe(
        provider,
        OracleLiveProbeReceipt::new(
            client_version_attestation,
            server_version,
            provider_artifact_sha256,
            transcript_sha256,
            session_binding_sha256,
        ),
        observed,
    )
}

/// Build the declaration and evidence for one validated offline Basic stream.
///
/// This remains crate-private so every admitted document has passed framing,
/// checksum, query-pack, row-shape, and counter validation. The checksums bind
/// the document internally and detect accidental damage; they do not
/// authenticate its author or protect it from an operator who can rewrite the
/// document and recompute its digest.
pub(crate) fn oracle_offline_provider_contract(
    server_version: OracleVersion,
    query_pack_version: u16,
    query_pack_sha256: [u8; 32],
    stream_sha256: [u8; 32],
    limits: OracleProviderLimits,
) -> (OracleProviderDeclaration, OracleCapabilityEvidence) {
    let profile = OracleConnectionProfile {
        endpoint_mode: OracleEndpointMode::Offline,
        transport: OracleTransport::Offline,
        authentication: OracleAuthentication::Offline,
        trust_source: OracleTrustSource::NotApplicable,
        cancellation: OracleCancellation::NotApplicable,
        local_inputs: BTreeSet::from([OracleLocalInput::OfflineCapture]),
    };
    let capabilities = OracleCapabilitySet {
        bind_variables: false,
        typed_scalar_results: true,
        limits,
        connection_profiles: BTreeSet::from([profile]),
        tiers: BTreeSet::from([OracleCaptureTier::Basic]),
        artifact_details: BTreeSet::from([ArtifactDetail::None, ArtifactDetail::Summary]),
    };
    let declaration = OracleProviderDeclaration {
        contract_version: ORACLE_PROVIDER_CONTRACT_VERSION,
        provider: OracleProviderKind::Offline,
        maximum: capabilities.clone(),
    };
    let evidence = OracleCapabilityEvidence::from_offline_capture(
        server_version,
        query_pack_version,
        query_pack_sha256,
        stream_sha256,
        capabilities,
    );
    (declaration, evidence)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleCaptureRequest {
    pub contract_version: u16,
    pub provider: OracleProviderKind,
    pub tier: OracleCaptureTier,
    pub artifact_detail: ArtifactDetail,
    pub endpoint_mode: OracleEndpointMode,
    pub transport: OracleTransport,
    pub authentication: OracleAuthentication,
    pub trust_source: OracleTrustSource,
    pub authorized_local_inputs: BTreeSet<OracleLocalInput>,
    pub require_confirmed_server_cancel: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleNegotiatedCapabilities {
    provider: OracleProviderKind,
    proof: OracleCapabilityProof,
    tier: OracleCaptureTier,
    artifact_detail: ArtifactDetail,
    endpoint_mode: OracleEndpointMode,
    transport: OracleTransport,
    authentication: OracleAuthentication,
    trust_source: OracleTrustSource,
    limits: OracleProviderLimits,
    cancellation: OracleCancellation,
    local_inputs: BTreeSet<OracleLocalInput>,
}

impl OracleNegotiatedCapabilities {
    pub fn provider(&self) -> OracleProviderKind {
        self.provider
    }

    pub fn proof(&self) -> &OracleCapabilityProof {
        &self.proof
    }

    pub fn tier(&self) -> OracleCaptureTier {
        self.tier
    }

    pub fn artifact_detail(&self) -> ArtifactDetail {
        self.artifact_detail
    }

    pub fn endpoint_mode(&self) -> OracleEndpointMode {
        self.endpoint_mode
    }

    pub fn transport(&self) -> OracleTransport {
        self.transport
    }

    pub fn authentication(&self) -> OracleAuthentication {
        self.authentication
    }

    pub fn trust_source(&self) -> OracleTrustSource {
        self.trust_source
    }

    pub fn limits(&self) -> OracleProviderLimits {
        self.limits
    }

    pub fn cancellation(&self) -> OracleCancellation {
        self.cancellation
    }

    pub fn local_inputs(&self) -> &BTreeSet<OracleLocalInput> {
        &self.local_inputs
    }
}

/// Stable provider reasons; callers map them to localized DBP diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OracleAdmissionFailure {
    ContractVersion,
    ProviderMismatch,
    InvalidDeclaration,
    InvalidEvidence,
    CapabilityUnproven,
    EvidenceExceedsDeclaration,
    UnsupportedTier,
    UnsupportedArtifactDetail,
    UnsupportedEndpointMode,
    UnsupportedTransport,
    UnsupportedAuthentication,
    UnsupportedTrustSource,
    UnsupportedConnectionProfile,
    UnauthorizedLocalInput,
    UnconfirmedCancellation,
    InconsistentOfflineMode,
    InconsistentTransportTrust,
}

pub fn negotiate_oracle_capabilities(
    request: &OracleCaptureRequest,
    declaration: &OracleProviderDeclaration,
    evidence: &OracleCapabilityEvidence,
) -> Result<OracleNegotiatedCapabilities, OracleAdmissionFailure> {
    if request.contract_version != ORACLE_PROVIDER_CONTRACT_VERSION
        || declaration.contract_version != ORACLE_PROVIDER_CONTRACT_VERSION
        || evidence.contract_version != ORACLE_PROVIDER_CONTRACT_VERSION
    {
        return Err(OracleAdmissionFailure::ContractVersion);
    }
    if request.provider != declaration.provider || request.provider != evidence.provider {
        return Err(OracleAdmissionFailure::ProviderMismatch);
    }

    validate_capability_shape(declaration.provider, &declaration.maximum)
        .map_err(|_| OracleAdmissionFailure::InvalidDeclaration)?;
    let observed = evidence
        .proof
        .observed()
        .ok_or(OracleAdmissionFailure::CapabilityUnproven)?;
    validate_proof_kind(evidence.provider, &evidence.proof)?;
    validate_capability_shape(evidence.provider, observed)
        .map_err(|_| OracleAdmissionFailure::InvalidEvidence)?;
    if !capabilities_fit(observed, &declaration.maximum) {
        return Err(OracleAdmissionFailure::EvidenceExceedsDeclaration);
    }
    validate_request_shape(request)?;

    if !observed.tiers.contains(&request.tier) {
        return Err(OracleAdmissionFailure::UnsupportedTier);
    }
    if !observed.artifact_details.contains(&request.artifact_detail) {
        return Err(OracleAdmissionFailure::UnsupportedArtifactDetail);
    }
    declaration
        .maximum
        .connection_profiles
        .iter()
        .find(|profile| profile.matches_request(request))
        .ok_or_else(|| {
            unsupported_profile_reason(request, &declaration.maximum.connection_profiles)
        })?;
    let profile = observed
        .connection_profiles
        .iter()
        .find(|profile| profile.matches_request(request))
        .ok_or(OracleAdmissionFailure::InvalidEvidence)?;
    if !profile
        .local_inputs
        .is_subset(&request.authorized_local_inputs)
    {
        return Err(OracleAdmissionFailure::UnauthorizedLocalInput);
    }

    if request.require_confirmed_server_cancel
        && profile.cancellation != OracleCancellation::ServerConfirmed
    {
        return Err(OracleAdmissionFailure::UnconfirmedCancellation);
    }

    Ok(OracleNegotiatedCapabilities {
        provider: request.provider,
        proof: evidence.proof.clone(),
        tier: request.tier,
        artifact_detail: request.artifact_detail,
        endpoint_mode: request.endpoint_mode,
        transport: request.transport,
        authentication: request.authentication,
        trust_source: request.trust_source,
        limits: observed.limits,
        cancellation: profile.cancellation,
        local_inputs: profile.local_inputs.clone(),
    })
}

fn validate_capability_shape(
    provider: OracleProviderKind,
    capabilities: &OracleCapabilitySet,
) -> Result<(), OracleAdmissionFailure> {
    if !capabilities.limits.are_bounded()
        || capabilities.connection_profiles.is_empty()
        || capabilities.tiers.is_empty()
        || capabilities.artifact_details.is_empty()
    {
        return Err(OracleAdmissionFailure::InvalidEvidence);
    }
    if !capabilities.tiers.contains(&OracleCaptureTier::Basic)
        || (capabilities.tiers.contains(&OracleCaptureTier::Enhanced)
            && !capabilities.tiers.contains(&OracleCaptureTier::Standard))
    {
        return Err(OracleAdmissionFailure::InvalidEvidence);
    }
    if !capabilities.typed_scalar_results {
        return Err(OracleAdmissionFailure::InvalidEvidence);
    }
    if (capabilities
        .artifact_details
        .contains(&ArtifactDetail::Graph)
        || capabilities
            .artifact_details
            .contains(&ArtifactDetail::Analyzed))
        && !capabilities.tiers.contains(&OracleCaptureTier::Enhanced)
    {
        return Err(OracleAdmissionFailure::InvalidEvidence);
    }
    if capabilities
        .connection_profiles
        .iter()
        .any(|profile| validate_connection_profile(provider, profile).is_err())
    {
        return Err(OracleAdmissionFailure::InvalidEvidence);
    }

    match provider {
        OracleProviderKind::Offline => {
            if capabilities.connection_profiles.len() != 1
                || capabilities.tiers != BTreeSet::from([OracleCaptureTier::Basic])
                || capabilities
                    .artifact_details
                    .contains(&ArtifactDetail::Graph)
                || capabilities
                    .artifact_details
                    .contains(&ArtifactDetail::Analyzed)
                || capabilities.bind_variables
            {
                return Err(OracleAdmissionFailure::InconsistentOfflineMode);
            }
        }
        OracleProviderKind::Oci => {
            if !capabilities.bind_variables {
                return Err(OracleAdmissionFailure::InvalidEvidence);
            }
        }
        OracleProviderKind::Sqlcl | OracleProviderKind::Sqlplus => {
            if !capabilities.bind_variables {
                return Err(OracleAdmissionFailure::InvalidEvidence);
            }
        }
    }
    Ok(())
}

fn validate_proof_kind(
    provider: OracleProviderKind,
    proof: &OracleCapabilityProof,
) -> Result<(), OracleAdmissionFailure> {
    match (provider, proof) {
        (OracleProviderKind::Offline, OracleCapabilityProof::ChecksummedOffline(proof))
            if proof.query_pack_version > 0
                && proof.query_pack_sha256 != [0; 32]
                && proof.stream_sha256 != [0; 32] =>
        {
            Ok(())
        }
        (OracleProviderKind::Offline, _) => Err(OracleAdmissionFailure::InvalidEvidence),
        (_, OracleCapabilityProof::LiveProbe(proof))
            if proof.receipt.probe_contract_version == ORACLE_LIVE_PROBE_CONTRACT_VERSION
                && proof.receipt.provider_artifact_sha256 != [0; 32]
                && proof.receipt.transcript_sha256 != [0; 32]
                && proof.receipt.session_binding_sha256 != [0; 32]
                && proof.observed.connection_profiles.len() == 1 =>
        {
            Ok(())
        }
        _ => Err(OracleAdmissionFailure::InvalidEvidence),
    }
}

fn validate_request_shape(request: &OracleCaptureRequest) -> Result<(), OracleAdmissionFailure> {
    let offline = request.provider == OracleProviderKind::Offline;
    let offline_shape = request.endpoint_mode == OracleEndpointMode::Offline
        && request.transport == OracleTransport::Offline
        && request.authentication == OracleAuthentication::Offline
        && request.trust_source == OracleTrustSource::NotApplicable;
    if offline != offline_shape {
        return Err(OracleAdmissionFailure::InconsistentOfflineMode);
    }
    if request.tier != OracleCaptureTier::Enhanced
        && matches!(
            request.artifact_detail,
            ArtifactDetail::Graph | ArtifactDetail::Analyzed
        )
    {
        return Err(OracleAdmissionFailure::UnsupportedArtifactDetail);
    }
    match request.transport {
        OracleTransport::Tcp | OracleTransport::Offline
            if request.trust_source != OracleTrustSource::NotApplicable =>
        {
            return Err(OracleAdmissionFailure::InconsistentTransportTrust);
        }
        OracleTransport::TcpsVerifyFull
            if request.trust_source == OracleTrustSource::NotApplicable =>
        {
            return Err(OracleAdmissionFailure::InconsistentTransportTrust);
        }
        _ => {}
    }
    Ok(())
}

fn validate_connection_profile(
    provider: OracleProviderKind,
    profile: &OracleConnectionProfile,
) -> Result<(), OracleAdmissionFailure> {
    let offline = provider == OracleProviderKind::Offline;
    let offline_shape = profile.endpoint_mode == OracleEndpointMode::Offline
        && profile.transport == OracleTransport::Offline
        && profile.authentication == OracleAuthentication::Offline
        && profile.trust_source == OracleTrustSource::NotApplicable
        && profile.cancellation == OracleCancellation::NotApplicable;
    if offline != offline_shape {
        return Err(OracleAdmissionFailure::InconsistentOfflineMode);
    }
    if !offline
        && (profile.endpoint_mode == OracleEndpointMode::Offline
            || profile.transport == OracleTransport::Offline
            || profile.authentication == OracleAuthentication::Offline
            || profile.cancellation == OracleCancellation::NotApplicable)
    {
        return Err(OracleAdmissionFailure::InvalidEvidence);
    }
    match profile.transport {
        OracleTransport::Tcp | OracleTransport::Offline
            if profile.trust_source != OracleTrustSource::NotApplicable =>
        {
            return Err(OracleAdmissionFailure::InconsistentTransportTrust);
        }
        OracleTransport::TcpsVerifyFull
            if profile.trust_source == OracleTrustSource::NotApplicable =>
        {
            return Err(OracleAdmissionFailure::InconsistentTransportTrust);
        }
        _ => {}
    }

    let required = required_local_inputs(
        provider,
        profile.endpoint_mode,
        profile.authentication,
        profile.trust_source,
    );
    if profile.local_inputs != required {
        return Err(OracleAdmissionFailure::InvalidEvidence);
    }
    Ok(())
}

fn required_local_inputs(
    provider: OracleProviderKind,
    endpoint_mode: OracleEndpointMode,
    authentication: OracleAuthentication,
    trust_source: OracleTrustSource,
) -> BTreeSet<OracleLocalInput> {
    let mut required = match provider {
        OracleProviderKind::Oci => BTreeSet::from([OracleLocalInput::ClientLibrary]),
        OracleProviderKind::Sqlcl | OracleProviderKind::Sqlplus => {
            BTreeSet::from([OracleLocalInput::ClientExecutable])
        }
        OracleProviderKind::Offline => BTreeSet::from([OracleLocalInput::OfflineCapture]),
    };
    if endpoint_mode == OracleEndpointMode::Alias {
        required.insert(OracleLocalInput::OracleNetConfig);
    }
    if authentication == OracleAuthentication::Wallet {
        required.insert(OracleLocalInput::Wallet);
    }
    match trust_source {
        OracleTrustSource::PemCaFile => {
            required.insert(OracleLocalInput::TrustAnchor);
        }
        OracleTrustSource::Wallet => {
            required.insert(OracleLocalInput::Wallet);
        }
        OracleTrustSource::NotApplicable | OracleTrustSource::SystemTrust => {}
    }
    required
}

fn capabilities_fit(observed: &OracleCapabilitySet, maximum: &OracleCapabilitySet) -> bool {
    (!observed.bind_variables || maximum.bind_variables)
        && (!observed.typed_scalar_results || maximum.typed_scalar_results)
        && observed.limits.fits_within(maximum.limits)
        && observed.connection_profiles.iter().all(|observed_profile| {
            maximum.connection_profiles.iter().any(|maximum_profile| {
                observed_profile.same_connection_shape(maximum_profile)
                    && cancellation_strength(observed_profile.cancellation)
                        <= cancellation_strength(maximum_profile.cancellation)
            })
        })
        && observed.tiers.is_subset(&maximum.tiers)
        && observed
            .artifact_details
            .is_subset(&maximum.artifact_details)
}

fn cancellation_strength(cancellation: OracleCancellation) -> u8 {
    match cancellation {
        OracleCancellation::NotApplicable => 0,
        OracleCancellation::LocalProcessOnly => 1,
        OracleCancellation::SessionDiscardRequired => 2,
        OracleCancellation::ServerConfirmed => 3,
    }
}

fn unsupported_profile_reason(
    request: &OracleCaptureRequest,
    profiles: &BTreeSet<OracleConnectionProfile>,
) -> OracleAdmissionFailure {
    if profiles
        .iter()
        .all(|profile| profile.endpoint_mode != request.endpoint_mode)
    {
        return OracleAdmissionFailure::UnsupportedEndpointMode;
    }
    if profiles
        .iter()
        .all(|profile| profile.transport != request.transport)
    {
        return OracleAdmissionFailure::UnsupportedTransport;
    }
    if profiles
        .iter()
        .all(|profile| profile.authentication != request.authentication)
    {
        return OracleAdmissionFailure::UnsupportedAuthentication;
    }
    if profiles
        .iter()
        .all(|profile| profile.trust_source != request.trust_source)
    {
        return OracleAdmissionFailure::UnsupportedTrustSource;
    }
    OracleAdmissionFailure::UnsupportedConnectionProfile
}

/// Build an admitted capability set for sibling-module unit tests without
/// exposing a production constructor that could bypass negotiation.
#[cfg(test)]
pub(crate) fn test_negotiated_capabilities(
    provider: OracleProviderKind,
    server_major: Option<u16>,
    tier: OracleCaptureTier,
    artifact_detail: ArtifactDetail,
    cancellation: OracleCancellation,
    limits: OracleProviderLimits,
) -> OracleNegotiatedCapabilities {
    let proof = match server_major {
        Some(server_major) => {
            let client_version = OracleVersion::from_components(&[23, 1]).expect("test version");
            let server_version =
                OracleVersion::from_components(&[server_major, 1]).expect("test version");
            OracleCapabilityProof::LiveProbe(OracleLiveProbeEvidence {
                receipt: OracleLiveProbeReceipt::new(
                    OracleClientVersionAttestation::Attested(client_version),
                    server_version,
                    [1; 32],
                    [2; 32],
                    [3; 32],
                ),
                observed: OracleCapabilitySet {
                    bind_variables: true,
                    typed_scalar_results: true,
                    limits,
                    connection_profiles: BTreeSet::new(),
                    tiers: BTreeSet::new(),
                    artifact_details: BTreeSet::new(),
                },
            })
        }
        None => OracleCapabilityProof::Unverified,
    };
    OracleNegotiatedCapabilities {
        provider,
        proof,
        tier,
        artifact_detail,
        endpoint_mode: OracleEndpointMode::DirectService,
        transport: OracleTransport::TcpsVerifyFull,
        authentication: OracleAuthentication::Password,
        trust_source: OracleTrustSource::PemCaFile,
        limits,
        cancellation,
        local_inputs: BTreeSet::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> OracleProviderLimits {
        OracleProviderLimits {
            catalog_rows: 1_000_000,
            catalog_bytes: 256 * 1024 * 1024,
            result_nesting_depth: 16,
            elapsed_ms: 300_000,
            transient_value_bytes: 16 * 1024 * 1024,
            lob_bytes: 1024 * 1024,
        }
    }

    fn connection_profile(
        provider: OracleProviderKind,
        endpoint_mode: OracleEndpointMode,
        transport: OracleTransport,
        authentication: OracleAuthentication,
        trust_source: OracleTrustSource,
        cancellation: OracleCancellation,
    ) -> OracleConnectionProfile {
        OracleConnectionProfile {
            endpoint_mode,
            transport,
            authentication,
            trust_source,
            cancellation,
            local_inputs: required_local_inputs(
                provider,
                endpoint_mode,
                authentication,
                trust_source,
            ),
        }
    }

    fn direct_oci_profile(cancellation: OracleCancellation) -> OracleConnectionProfile {
        connection_profile(
            OracleProviderKind::Oci,
            OracleEndpointMode::DirectService,
            OracleTransport::Tcp,
            OracleAuthentication::Password,
            OracleTrustSource::NotApplicable,
            cancellation,
        )
    }

    fn wallet_alias_profile() -> OracleConnectionProfile {
        connection_profile(
            OracleProviderKind::Oci,
            OracleEndpointMode::Alias,
            OracleTransport::TcpsVerifyFull,
            OracleAuthentication::Wallet,
            OracleTrustSource::Wallet,
            OracleCancellation::ServerConfirmed,
        )
    }

    fn oci_capabilities() -> OracleCapabilitySet {
        OracleCapabilitySet {
            bind_variables: true,
            typed_scalar_results: true,
            limits: limits(),
            connection_profiles: BTreeSet::from([direct_oci_profile(
                OracleCancellation::ServerConfirmed,
            )]),
            tiers: BTreeSet::from([
                OracleCaptureTier::Basic,
                OracleCaptureTier::Standard,
                OracleCaptureTier::Enhanced,
            ]),
            artifact_details: BTreeSet::from([
                ArtifactDetail::None,
                ArtifactDetail::Summary,
                ArtifactDetail::Graph,
                ArtifactDetail::Analyzed,
            ]),
        }
    }

    fn live_evidence(
        provider: OracleProviderKind,
        observed: OracleCapabilitySet,
    ) -> OracleCapabilityEvidence {
        OracleCapabilityEvidence::from_live_probe(
            provider,
            OracleLiveProbeReceipt::new(
                OracleClientVersionAttestation::Attested(
                    OracleVersion::from_components(&[23, 6]).unwrap(),
                ),
                OracleVersion::from_components(&[19, 24, 0, 0, 0]).unwrap(),
                [0x11; 32],
                [0x22; 32],
                [0x33; 32],
            ),
            observed,
        )
    }

    fn oci_request() -> OracleCaptureRequest {
        OracleCaptureRequest {
            contract_version: ORACLE_PROVIDER_CONTRACT_VERSION,
            provider: OracleProviderKind::Oci,
            tier: OracleCaptureTier::Basic,
            artifact_detail: ArtifactDetail::Summary,
            endpoint_mode: OracleEndpointMode::DirectService,
            transport: OracleTransport::Tcp,
            authentication: OracleAuthentication::Password,
            trust_source: OracleTrustSource::NotApplicable,
            authorized_local_inputs: BTreeSet::from([OracleLocalInput::ClientLibrary]),
            require_confirmed_server_cancel: true,
        }
    }

    fn declaration(
        provider: OracleProviderKind,
        maximum: OracleCapabilitySet,
    ) -> OracleProviderDeclaration {
        OracleProviderDeclaration {
            contract_version: ORACLE_PROVIDER_CONTRACT_VERSION,
            provider,
            maximum,
        }
    }

    fn offline_capabilities() -> OracleCapabilitySet {
        OracleCapabilitySet {
            bind_variables: false,
            typed_scalar_results: true,
            limits: limits(),
            connection_profiles: BTreeSet::from([connection_profile(
                OracleProviderKind::Offline,
                OracleEndpointMode::Offline,
                OracleTransport::Offline,
                OracleAuthentication::Offline,
                OracleTrustSource::NotApplicable,
                OracleCancellation::NotApplicable,
            )]),
            tiers: BTreeSet::from([OracleCaptureTier::Basic]),
            artifact_details: BTreeSet::from([ArtifactDetail::None, ArtifactDetail::Summary]),
        }
    }

    fn offline_request() -> OracleCaptureRequest {
        OracleCaptureRequest {
            contract_version: ORACLE_PROVIDER_CONTRACT_VERSION,
            provider: OracleProviderKind::Offline,
            tier: OracleCaptureTier::Basic,
            artifact_detail: ArtifactDetail::Summary,
            endpoint_mode: OracleEndpointMode::Offline,
            transport: OracleTransport::Offline,
            authentication: OracleAuthentication::Offline,
            trust_source: OracleTrustSource::NotApplicable,
            authorized_local_inputs: BTreeSet::from([OracleLocalInput::OfflineCapture]),
            require_confirmed_server_cancel: false,
        }
    }

    #[test]
    fn numeric_versions_never_retain_a_product_banner() {
        let version = OracleVersion::from_components(&[12, 2, 0, 1, 0]).unwrap();
        assert_eq!(version.components(), &[12, 2, 0, 1, 0]);
        assert!(OracleVersion::from_components(&[]).is_none());
        assert!(OracleVersion::from_components(&[1, 2, 3, 4, 5, 6]).is_none());
    }

    #[test]
    fn proof_exposes_audit_fingerprints_without_local_paths_or_banners() {
        let live = live_evidence(OracleProviderKind::Oci, oci_capabilities());
        assert_eq!(
            live.proof.attested_client_version().unwrap().components(),
            &[23, 6]
        );
        assert_eq!(live.proof.provider_artifact_sha256(), Some([0x11; 32]));
        assert_eq!(live.proof.probe_transcript_sha256(), Some([0x22; 32]));
        assert_eq!(live.proof.session_binding_sha256(), Some([0x33; 32]));
        assert_eq!(live.proof.offline_query_pack_version(), None);

        let offline = OracleCapabilityEvidence::from_offline_capture(
            OracleVersion::from_components(&[21, 3]).unwrap(),
            7,
            [0x44; 32],
            [0x55; 32],
            offline_capabilities(),
        );
        assert_eq!(offline.proof.offline_query_pack_version(), Some(7));
        assert_eq!(offline.proof.offline_query_pack_sha256(), Some([0x44; 32]));
        assert_eq!(offline.proof.offline_stream_sha256(), Some([0x55; 32]));
        assert_eq!(offline.proof.provider_artifact_sha256(), None);
    }

    #[test]
    fn live_proof_remains_admissible_when_client_version_is_unverified() {
        let mut evidence = live_evidence(
            OracleProviderKind::Sqlplus,
            OracleCapabilitySet {
                bind_variables: true,
                typed_scalar_results: true,
                limits: limits(),
                connection_profiles: BTreeSet::from([connection_profile(
                    OracleProviderKind::Sqlplus,
                    OracleEndpointMode::DirectService,
                    OracleTransport::Tcp,
                    OracleAuthentication::Password,
                    OracleTrustSource::NotApplicable,
                    OracleCancellation::LocalProcessOnly,
                )]),
                tiers: BTreeSet::from([OracleCaptureTier::Basic]),
                artifact_details: BTreeSet::from([ArtifactDetail::None, ArtifactDetail::Summary]),
            },
        );
        let OracleCapabilityProof::LiveProbe(proof) = &mut evidence.proof else {
            panic!("test helper must construct live proof");
        };
        proof.receipt.client_version_attestation = OracleClientVersionAttestation::Mismatched;

        assert_eq!(evidence.proof.attested_client_version(), None);
        assert_eq!(
            evidence.proof.client_version_attestation(),
            Some(OracleClientVersionAttestation::Mismatched)
        );
        assert_eq!(
            validate_proof_kind(OracleProviderKind::Sqlplus, &evidence.proof),
            Ok(())
        );
    }

    #[test]
    fn per_value_and_lob_limits_must_fit_inside_the_catalog_byte_limit() {
        let mut invalid = oci_capabilities();
        invalid.limits.transient_value_bytes = invalid.limits.catalog_bytes + 1;
        assert_eq!(
            validate_capability_shape(OracleProviderKind::Oci, &invalid),
            Err(OracleAdmissionFailure::InvalidEvidence)
        );

        let mut invalid = oci_capabilities();
        invalid.limits.lob_bytes = invalid.limits.catalog_bytes + 1;
        assert_eq!(
            validate_capability_shape(OracleProviderKind::Oci, &invalid),
            Err(OracleAdmissionFailure::InvalidEvidence)
        );
    }

    #[test]
    fn proved_bounded_oci_basic_request_is_admitted() {
        let capabilities = oci_capabilities();
        let negotiated = negotiate_oracle_capabilities(
            &oci_request(),
            &declaration(OracleProviderKind::Oci, capabilities.clone()),
            &live_evidence(OracleProviderKind::Oci, capabilities),
        )
        .unwrap();
        assert_eq!(negotiated.provider, OracleProviderKind::Oci);
        assert_eq!(negotiated.tier, OracleCaptureTier::Basic);
        assert_eq!(negotiated.cancellation, OracleCancellation::ServerConfirmed);
        assert_eq!(
            negotiated.local_inputs,
            BTreeSet::from([OracleLocalInput::ClientLibrary])
        );
    }

    #[test]
    fn declaration_without_session_proof_is_rejected() {
        let capabilities = oci_capabilities();
        let error = negotiate_oracle_capabilities(
            &oci_request(),
            &declaration(OracleProviderKind::Oci, capabilities.clone()),
            &OracleCapabilityEvidence::unverified(OracleProviderKind::Oci),
        )
        .unwrap_err();
        assert_eq!(error, OracleAdmissionFailure::CapabilityUnproven);
    }

    #[test]
    fn runtime_evidence_cannot_exceed_the_declared_maximum() {
        let mut maximum = oci_capabilities();
        maximum.tiers.remove(&OracleCaptureTier::Enhanced);
        maximum.artifact_details.remove(&ArtifactDetail::Graph);
        maximum.artifact_details.remove(&ArtifactDetail::Analyzed);
        let observed = oci_capabilities();
        let error = negotiate_oracle_capabilities(
            &oci_request(),
            &declaration(OracleProviderKind::Oci, maximum),
            &live_evidence(OracleProviderKind::Oci, observed),
        )
        .unwrap_err();
        assert_eq!(error, OracleAdmissionFailure::EvidenceExceedsDeclaration);
    }

    #[test]
    fn local_input_categories_require_operator_authorization() {
        let capabilities = oci_capabilities();
        let mut request = oci_request();
        request.authorized_local_inputs.clear();
        let error = negotiate_oracle_capabilities(
            &request,
            &declaration(OracleProviderKind::Oci, capabilities.clone()),
            &live_evidence(OracleProviderKind::Oci, capabilities),
        )
        .unwrap_err();
        assert_eq!(error, OracleAdmissionFailure::UnauthorizedLocalInput);
    }

    #[test]
    fn command_client_cannot_claim_server_cancellation_without_proof() {
        let mut capabilities = oci_capabilities();
        capabilities.connection_profiles = BTreeSet::from([connection_profile(
            OracleProviderKind::Sqlcl,
            OracleEndpointMode::DirectService,
            OracleTransport::Tcp,
            OracleAuthentication::Password,
            OracleTrustSource::NotApplicable,
            OracleCancellation::LocalProcessOnly,
        )]);
        let mut request = oci_request();
        request.provider = OracleProviderKind::Sqlcl;
        request.authorized_local_inputs = BTreeSet::from([OracleLocalInput::ClientExecutable]);
        let error = negotiate_oracle_capabilities(
            &request,
            &declaration(OracleProviderKind::Sqlcl, capabilities.clone()),
            &live_evidence(OracleProviderKind::Sqlcl, capabilities),
        )
        .unwrap_err();
        assert_eq!(error, OracleAdmissionFailure::UnconfirmedCancellation);
    }

    #[test]
    fn observed_cancellation_can_degrade_below_the_declared_maximum() {
        let maximum = oci_capabilities();
        let mut observed = maximum.clone();
        observed.connection_profiles = BTreeSet::from([direct_oci_profile(
            OracleCancellation::SessionDiscardRequired,
        )]);
        let mut request = oci_request();
        request.require_confirmed_server_cancel = false;

        let negotiated = negotiate_oracle_capabilities(
            &request,
            &declaration(OracleProviderKind::Oci, maximum),
            &live_evidence(OracleProviderKind::Oci, observed),
        )
        .unwrap();
        assert_eq!(
            negotiated.cancellation,
            OracleCancellation::SessionDiscardRequired
        );
    }

    #[test]
    fn observed_cancellation_cannot_exceed_the_declared_maximum() {
        let mut maximum = oci_capabilities();
        maximum.connection_profiles =
            BTreeSet::from([direct_oci_profile(OracleCancellation::LocalProcessOnly)]);
        let observed = oci_capabilities();
        let mut request = oci_request();
        request.require_confirmed_server_cancel = false;

        let error = negotiate_oracle_capabilities(
            &request,
            &declaration(OracleProviderKind::Oci, maximum),
            &live_evidence(OracleProviderKind::Oci, observed),
        )
        .unwrap_err();
        assert_eq!(error, OracleAdmissionFailure::EvidenceExceedsDeclaration);
    }

    #[test]
    fn graph_or_analysis_requires_the_enhanced_tier() {
        let capabilities = oci_capabilities();
        let mut request = oci_request();
        request.artifact_detail = ArtifactDetail::Graph;
        let error = negotiate_oracle_capabilities(
            &request,
            &declaration(OracleProviderKind::Oci, capabilities.clone()),
            &live_evidence(OracleProviderKind::Oci, capabilities),
        )
        .unwrap_err();
        assert_eq!(error, OracleAdmissionFailure::UnsupportedArtifactDetail);
    }

    #[test]
    fn tcps_requires_an_explicit_proved_trust_source() {
        let capabilities = oci_capabilities();
        let mut request = oci_request();
        request.transport = OracleTransport::TcpsVerifyFull;
        let error = negotiate_oracle_capabilities(
            &request,
            &declaration(OracleProviderKind::Oci, capabilities.clone()),
            &live_evidence(OracleProviderKind::Oci, capabilities),
        )
        .unwrap_err();
        assert_eq!(error, OracleAdmissionFailure::InconsistentTransportTrust);
    }

    #[test]
    fn declaration_does_not_admit_a_cartesian_product_of_profiles() {
        let mut maximum = oci_capabilities();
        maximum.connection_profiles.insert(wallet_alias_profile());
        let observed = oci_capabilities();
        let mut request = oci_request();
        request.transport = OracleTransport::TcpsVerifyFull;
        request.authentication = OracleAuthentication::Wallet;
        request.trust_source = OracleTrustSource::Wallet;
        request.authorized_local_inputs =
            BTreeSet::from([OracleLocalInput::ClientLibrary, OracleLocalInput::Wallet]);

        let error = negotiate_oracle_capabilities(
            &request,
            &declaration(OracleProviderKind::Oci, maximum),
            &live_evidence(OracleProviderKind::Oci, observed),
        )
        .unwrap_err();
        assert_eq!(error, OracleAdmissionFailure::UnsupportedConnectionProfile);
    }

    #[test]
    fn one_live_probe_cannot_attest_multiple_connection_profiles() {
        let mut capabilities = oci_capabilities();
        capabilities
            .connection_profiles
            .insert(wallet_alias_profile());
        let error = negotiate_oracle_capabilities(
            &oci_request(),
            &declaration(OracleProviderKind::Oci, capabilities.clone()),
            &live_evidence(OracleProviderKind::Oci, capabilities),
        )
        .unwrap_err();
        assert_eq!(error, OracleAdmissionFailure::InvalidEvidence);
    }

    #[test]
    fn evidence_from_another_declared_profile_cannot_authorize_this_session() {
        let mut maximum = oci_capabilities();
        maximum.connection_profiles.insert(wallet_alias_profile());
        let mut observed = oci_capabilities();
        observed.connection_profiles = BTreeSet::from([wallet_alias_profile()]);

        let error = negotiate_oracle_capabilities(
            &oci_request(),
            &declaration(OracleProviderKind::Oci, maximum),
            &live_evidence(OracleProviderKind::Oci, observed),
        )
        .unwrap_err();
        assert_eq!(error, OracleAdmissionFailure::InvalidEvidence);
    }

    #[test]
    fn live_probe_requires_adapter_transcript_and_session_bindings() {
        let capabilities = oci_capabilities();
        let evidence = OracleCapabilityEvidence::from_live_probe(
            OracleProviderKind::Oci,
            OracleLiveProbeReceipt::new(
                OracleClientVersionAttestation::Attested(
                    OracleVersion::from_components(&[23, 6]).unwrap(),
                ),
                OracleVersion::from_components(&[19, 24]).unwrap(),
                [0; 32],
                [0; 32],
                [0; 32],
            ),
            capabilities.clone(),
        );
        let error = negotiate_oracle_capabilities(
            &oci_request(),
            &declaration(OracleProviderKind::Oci, capabilities),
            &evidence,
        )
        .unwrap_err();
        assert_eq!(error, OracleAdmissionFailure::InvalidEvidence);
    }

    #[test]
    fn unsupported_tier_is_never_silently_downgraded() {
        let maximum = oci_capabilities();
        let mut observed = maximum.clone();
        observed.tiers = BTreeSet::from([OracleCaptureTier::Basic]);
        observed.artifact_details = BTreeSet::from([ArtifactDetail::None, ArtifactDetail::Summary]);
        let mut request = oci_request();
        request.tier = OracleCaptureTier::Standard;
        let error = negotiate_oracle_capabilities(
            &request,
            &declaration(OracleProviderKind::Oci, maximum),
            &live_evidence(OracleProviderKind::Oci, observed),
        )
        .unwrap_err();
        assert_eq!(error, OracleAdmissionFailure::UnsupportedTier);
    }

    #[test]
    fn offline_provider_requires_a_checksummed_basic_capture() {
        let offline = offline_capabilities();
        let proof = OracleCapabilityEvidence::from_offline_capture(
            OracleVersion::from_components(&[21, 3]).unwrap(),
            1,
            [0x44; 32],
            [0x5a; 32],
            offline.clone(),
        );
        let negotiated = negotiate_oracle_capabilities(
            &offline_request(),
            &declaration(OracleProviderKind::Offline, offline.clone()),
            &proof,
        )
        .unwrap();
        assert_eq!(negotiated.cancellation, OracleCancellation::NotApplicable);
    }

    #[test]
    fn offline_evidence_cannot_be_used_for_a_live_provider() {
        let capabilities = oci_capabilities();
        let mut proof = OracleCapabilityEvidence::from_offline_capture(
            OracleVersion::from_components(&[26, 1]).unwrap(),
            1,
            [0x44; 32],
            [0x11; 32],
            capabilities.clone(),
        );
        proof.provider = OracleProviderKind::Oci;
        let error = negotiate_oracle_capabilities(
            &oci_request(),
            &declaration(OracleProviderKind::Oci, capabilities.clone()),
            &proof,
        )
        .unwrap_err();
        assert_eq!(error, OracleAdmissionFailure::InvalidEvidence);
    }

    #[test]
    fn offline_proof_requires_a_nonzero_pack_and_checksum() {
        let offline = offline_capabilities();
        let proof = OracleCapabilityEvidence::from_offline_capture(
            OracleVersion::from_components(&[19]).unwrap(),
            0,
            [0; 32],
            [0; 32],
            offline.clone(),
        );
        let error = negotiate_oracle_capabilities(
            &offline_request(),
            &declaration(OracleProviderKind::Offline, offline.clone()),
            &proof,
        )
        .unwrap_err();
        assert_eq!(error, OracleAdmissionFailure::InvalidEvidence);
    }
}
