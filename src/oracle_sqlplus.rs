//! Bounded SQL*Plus command-client adapter for the Oracle Basic tier.
//!
//! The operator supplies an exact executable. It is launched directly with
//! `/nolog`; no shell is involved, and the credential is written to the
//! child's protected stdin rather than argv or an environment variable. Each
//! catalog statement runs inside one persistent SQL*Plus session and emits a
//! random, null-aware, hex-encoded row frame directly from the SELECT. It does
//! not use DBMS_OUTPUT, whose server-side buffering would defeat incremental
//! row limits. Raw client or Oracle error text never crosses the adapter
//! boundary.
//!
//! This module is deliberately a Basic-only transport building block. Its
//! callers own URI parsing, audit rendering and operator-facing diagnostics.

#![allow(dead_code)]

use crate::artifacts::ArtifactDetail;
use crate::oracle_provider::{
    oracle_live_probe_evidence, OracleAuthentication, OracleCancellation, OracleCapabilityEvidence,
    OracleCapabilitySet, OracleCaptureTier, OracleClientVersionAttestation,
    OracleConnectionProfile, OracleEndpointMode, OracleLocalInput, OracleProviderDeclaration,
    OracleProviderKind, OracleProviderLimits, OracleTransport, OracleTrustSource, OracleVersion,
    ORACLE_PROVIDER_CONTRACT_VERSION,
};
use crate::oracle_session::{
    OracleCancelOutcome, OracleExecuteResult, OracleExecution, OracleQueryFailure, OracleRow,
    OracleRowControl, OracleRowReceiver, OracleSessionAdapter, OracleValue,
};
use crate::secret::Secret;
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, VecDeque};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
#[cfg(test)]
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

const PIPE_CHUNK_BYTES: usize = 4096;
const PIPE_CHANNEL_DEPTH: usize = 32;
// SQL RAW values are limited to 2,000 bytes when MAX_STRING_SIZE=STANDARD.
// Keep the UTF-8 payload below that server-side boundary so a long catalog
// value becomes a classified provider limit instead of aborting its query.
const MAX_SCALAR_UTF8_BYTES: usize = 1_000;
const STARTUP_LINE_BYTES: usize = 4096;
const SQLPLUS_RESULT_LINE_BYTES: usize = 32_767;
// Leave room below LINESIZE for SQL*Plus's hidden NEW_VALUE column and column
// separator. A32767 makes the 19c client emit the unframed informational line
// `rows will be truncated` even when the actual frame is short; A32000 keeps
// normal frames silent, while a genuinely oversized frame still fails closed.
const SQLPLUS_FRAME_DISPLAY_BYTES: usize = 32_000;
const MAX_CLIENT_EXECUTABLE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_SQLPLUS_USERNAME_BYTES: usize = 128;
const MAX_SQLPLUS_PASSWORD_BYTES: usize = 1024;
pub(crate) const SQLPLUS_BASIC_MAX_LIMITS: OracleProviderLimits = OracleProviderLimits {
    catalog_rows: 1_000_000,
    catalog_bytes: 256 * 1024 * 1024,
    result_nesting_depth: 2,
    elapsed_ms: 10 * 60 * 1000,
    transient_value_bytes: MAX_SCALAR_UTF8_BYTES as u64,
    lob_bytes: 1,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OracleSqlplusStartFailure {
    ExecutablePathNotAbsolute,
    ExecutableUnavailable,
    ExecutableTooLarge,
    ExecutableReadFailed,
    ExecutableChangedDuringLaunch,
    NetworkSandboxUnavailable,
    InvalidUsername,
    InvalidConnectIdentifier,
    UnsupportedPassword,
    RandomnessUnavailable,
    ProcessLaunch,
    ProcessPipeUnavailable,
    ProcessWrite,
    StartupTimedOut,
    LoginRejected,
    ClientExitedBeforeReady,
    RootContainerUnsupported,
    StartupOutputLimitExceeded,
    MalformedStartup,
}

/// Sealed direct-service input. A caller is responsible for
/// constructing the connect identifier from separately validated URI fields;
/// arbitrary Oracle connect descriptors are intentionally not accepted here.
pub struct OracleSqlplusConfig<'a> {
    pub executable: &'a Path,
    pub username: &'a str,
    /// Canonical direct-service form, for example `//db.example:1521/APPPDB`.
    pub connect_identifier: &'a str,
    pub password: &'a Secret,
    /// Absolute, existing, private and empty directory assigned to `TNS_ADMIN`
    /// for a direct-service session. This prevents implicit `tnsnames.ora`,
    /// `sqlnet.ora`, and wallet discovery. SQL*Plus can still execute its
    /// installation-owned `glogin.sql`; the adapter resets every setting on
    /// which the framing protocol depends after authentication.
    pub isolated_network_config_dir: &'a Path,
    pub startup_timeout: Duration,
}

#[derive(Debug)]
enum PipeEvent {
    Stdout(Vec<u8>),
    Stderr(Vec<u8>),
    StdoutClosed,
    StderrClosed,
    ReadFailed,
}

/// One persistent SQL*Plus process and database session.
pub struct OracleSqlplusAdapter {
    child: Child,
    stdin: Option<ChildStdin>,
    events: Option<Receiver<PipeEvent>>,
    readers: Vec<JoinHandle<()>>,
    stdout_pending: Vec<u8>,
    stdout_lines: VecDeque<String>,
    stderr_tail: Vec<u8>,
    stdout_closed: bool,
    started: Instant,
    marker: String,
    /// Best readable version for protocol feature selection. The in-session
    /// value is authoritative when available; the executable banner is only a
    /// fallback. Absence selects the conservative oldest protocol shape.
    protocol_client_version: Option<OracleVersion>,
    client_version_attestation: OracleClientVersionAttestation,
    last_oracle_code: Option<i32>,
    login_rejection_observed: bool,
    stderr_activity: bool,
    terminated: bool,
    provider_artifact_sha256: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OracleSqlplusProbeFailure {
    Start(OracleSqlplusStartFailure),
    LimitUnavailable,
    Query(OracleQueryFailure),
    MalformedEvidence,
}

pub struct OracleSqlplusProbedSession {
    adapter: OracleSqlplusAdapter,
    declaration: OracleProviderDeclaration,
    evidence: OracleCapabilityEvidence,
    server_version: OracleVersion,
    effective_principal: String,
    database_identity: String,
}

impl OracleSqlplusProbedSession {
    pub fn adapter_mut(&mut self) -> &mut OracleSqlplusAdapter {
        &mut self.adapter
    }

    pub fn declaration(&self) -> &OracleProviderDeclaration {
        &self.declaration
    }

    pub fn evidence(&self) -> &OracleCapabilityEvidence {
        &self.evidence
    }

    pub fn server_version(&self) -> OracleVersion {
        self.server_version
    }

    /// Effective database principal for the audit record. It is never used as
    /// the owner selector and is not copied into the Blueprint artifact.
    pub fn effective_principal(&self) -> &str {
        &self.effective_principal
    }

    /// Database identity observed through the admitted session. It is used
    /// only in the customer-local audit and never copied into Blueprint.
    pub fn database_identity(&self) -> &str {
        &self.database_identity
    }
}

impl OracleSqlplusAdapter {
    pub fn start(config: OracleSqlplusConfig<'_>) -> Result<Self, OracleSqlplusStartFailure> {
        validate_config(&config)?;
        let executable = config
            .executable
            .canonicalize()
            .map_err(|_| OracleSqlplusStartFailure::ExecutableUnavailable)?;
        let network_sandbox = config
            .isolated_network_config_dir
            .canonicalize()
            .map_err(|_| OracleSqlplusStartFailure::NetworkSandboxUnavailable)?;
        if !network_sandbox_is_private_and_empty(&network_sandbox) {
            return Err(OracleSqlplusStartFailure::NetworkSandboxUnavailable);
        }
        let metadata = executable
            .metadata()
            .map_err(|_| OracleSqlplusStartFailure::ExecutableUnavailable)?;
        if !metadata.is_file() {
            return Err(OracleSqlplusStartFailure::ExecutableUnavailable);
        }
        if metadata.len() > MAX_CLIENT_EXECUTABLE_BYTES {
            return Err(OracleSqlplusStartFailure::ExecutableTooLarge);
        }
        let provider_artifact_sha256 = sha256_file(&executable)?;
        // `&_SQLPLUS_RELEASE` is convenient inside the persistent process but
        // installation-owned glogin.sql can redefine substitution variables.
        // Probe the executable independently before any credential is written.
        // A missing or disagreeing reading weakens provenance but must not
        // prevent a sound catalog capture.
        let executable_client_version =
            probe_sqlplus_executable_version(&executable, &network_sandbox, config.startup_timeout);

        let mut random = [0_u8; 16];
        getrandom::fill(&mut random)
            .map_err(|_| OracleSqlplusStartFailure::RandomnessUnavailable)?;
        let marker = format!("__DBWARP_BP_{}__", hex::encode(random));

        let mut command = configured_sqlplus_command(&executable, &network_sandbox);
        command
            .args(["-R", "3", "-L", "-S", "/nolog"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command
            .spawn()
            .map_err(|_| OracleSqlplusStartFailure::ProcessLaunch)?;
        // Recheck the complete directory chain after process creation. This
        // closes the useful parent-directory swap window for an unprivileged
        // peer; a path controlled by another user is never admitted.
        if !network_sandbox_is_private_and_empty(&network_sandbox) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(OracleSqlplusStartFailure::NetworkSandboxUnavailable);
        }
        let post_launch_sha256 = match sha256_file(&executable) {
            Ok(digest) => digest,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        };
        if post_launch_sha256 != provider_artifact_sha256 {
            let _ = child.kill();
            let _ = child.wait();
            return Err(OracleSqlplusStartFailure::ExecutableChangedDuringLaunch);
        }
        let (Some(stdin), Some(stdout), Some(stderr)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(OracleSqlplusStartFailure::ProcessPipeUnavailable);
        };
        let (sender, receiver) = mpsc::sync_channel(PIPE_CHANNEL_DEPTH);
        let stdout_reader = spawn_pipe_reader(stdout, sender.clone(), true);
        let stderr_reader = spawn_pipe_reader(stderr, sender, false);
        let mut adapter = Self {
            child,
            stdin: Some(stdin),
            events: Some(receiver),
            readers: vec![stdout_reader, stderr_reader],
            stdout_pending: Vec::new(),
            stdout_lines: VecDeque::new(),
            stderr_tail: Vec::new(),
            stdout_closed: false,
            started: Instant::now(),
            marker,
            protocol_client_version: None,
            client_version_attestation: OracleClientVersionAttestation::Unreadable,
            last_oracle_code: None,
            login_rejection_observed: false,
            stderr_activity: false,
            terminated: false,
            provider_artifact_sha256,
        };

        let deadline = Instant::now()
            .checked_add(config.startup_timeout)
            .ok_or(OracleSqlplusStartFailure::StartupTimedOut)?;
        if adapter.write_client_probe().is_err() {
            adapter.terminate_local();
            return Err(OracleSqlplusStartFailure::ProcessWrite);
        }
        let in_session_client_version = match adapter.wait_for_client_version(deadline) {
            Ok(version) => version,
            Err(error) => {
                adapter.terminate_local();
                return Err(error);
            }
        };
        adapter.client_version_attestation = sqlplus_client_versions_compatible(
            in_session_client_version,
            executable_client_version,
        );
        // The running process describes its own feature set. Only when that
        // value is unreadable may the independent banner select settings.
        // If neither is readable, Option::None deliberately selects the
        // conservative settings floor.
        adapter.protocol_client_version = in_session_client_version.or(executable_client_version);
        if adapter
            .write_login(
                config.username,
                config.connect_identifier,
                config.password,
                adapter.protocol_client_version,
            )
            .is_err()
        {
            adapter.terminate_local();
            return Err(OracleSqlplusStartFailure::ProcessWrite);
        }
        match adapter.wait_for_ready(deadline) {
            Ok(()) => Ok(adapter),
            Err(error) => {
                adapter.terminate_local();
                Err(error)
            }
        }
    }

    /// Stable pre/post-launch fingerprint of the operator-selected executable.
    /// The operator-local path is deliberately not retained after launch.
    pub fn provider_artifact_sha256(&self) -> [u8; 32] {
        self.provider_artifact_sha256
    }

    pub fn protocol_client_version(&self) -> Option<OracleVersion> {
        self.protocol_client_version
    }

    pub fn client_version_attestation(&self) -> OracleClientVersionAttestation {
        self.client_version_attestation
    }

    fn write_client_probe(&mut self) -> std::io::Result<()> {
        let marker = &self.marker;
        let input = self.stdin.as_mut().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::BrokenPipe, "SQL*Plus stdin closed")
        })?;
        // Probe with settings supported by every admitted SQL*Plus release.
        // Version-specific settings are emitted only after this value is
        // parsed, before any credential is written to the child.
        writeln!(input, "PROMPT {marker}|LOGIN-BEGIN")?;
        writeln!(input, "SET ECHO OFF")?;
        writeln!(input, "SET VERIFY OFF")?;
        writeln!(input, "SET DEFINE ON")?;
        writeln!(input, "PROMPT {marker}|CLIENT|&_SQLPLUS_RELEASE")?;
        writeln!(input, "SET DEFINE OFF")?;
        input.flush()
    }

    fn wait_for_client_version(
        &mut self,
        deadline: Instant,
    ) -> Result<Option<OracleVersion>, OracleSqlplusStartFailure> {
        let mut input_started = false;
        loop {
            let line = match self.next_stdout_line(deadline, STARTUP_LINE_BYTES) {
                Ok(line) => line,
                Err(ReadFailure::Timeout) => {
                    return Err(OracleSqlplusStartFailure::StartupTimedOut)
                }
                Err(ReadFailure::Closed | ReadFailure::Io) => {
                    return Err(OracleSqlplusStartFailure::ClientExitedBeforeReady)
                }
                Err(ReadFailure::LimitExceeded) => {
                    return Err(OracleSqlplusStartFailure::StartupOutputLimitExceeded)
                }
                Err(ReadFailure::Malformed) => {
                    return Err(OracleSqlplusStartFailure::MalformedStartup)
                }
            };
            if line == format!("{}|LOGIN-BEGIN", self.marker) {
                if input_started {
                    return Err(OracleSqlplusStartFailure::MalformedStartup);
                }
                input_started = true;
                self.last_oracle_code = None;
                self.login_rejection_observed = false;
                self.stderr_activity = false;
                self.stderr_tail.clear();
                continue;
            }
            if let Some(release) = line.strip_prefix(&format!("{}|CLIENT|", self.marker)) {
                // The unpredictable marker is the boundary. Some clients
                // suppress the preceding PROMPT while still
                // returning this exact version frame, so do not require the
                // informational LOGIN-BEGIN line as a second proof.
                let version = parse_sqlplus_release(release);
                // Installation-owned stderr before the version frame is not
                // connection evidence. A loader failure cannot reach this
                // positive frame; connection diagnostics start from here.
                self.last_oracle_code = None;
                self.login_rejection_observed = false;
                self.stderr_activity = false;
                self.stderr_tail.clear();
                return Ok(version);
            }
        }
    }

    fn write_login(
        &mut self,
        username: &str,
        connect_identifier: &str,
        password: &Secret,
        client_version: Option<OracleVersion>,
    ) -> std::io::Result<()> {
        let rendered_username = render_sqlplus_username(username).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid Oracle username")
        })?;
        let quoted_password = Zeroizing::new(password.expose().replace('"', "\"\""));
        let marker = &self.marker;
        let input = self.stdin.as_mut().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::BrokenPipe, "SQL*Plus stdin closed")
        })?;
        // The complete CONNECT command is still written only to the protected
        // child stdin. Quoting it suppresses the interactive password prompt,
        // which otherwise contaminates the first framed stdout line. The one
        // short-lived formatted credential copy is zeroized on return.
        writeln!(input, "SET ECHO OFF")?;
        writeln!(input, "SET VERIFY OFF")?;
        writeln!(input, "SET DEFINE OFF")?;
        writeln!(input, "SET SQLCASE MIXED")?;
        writeln!(input, "SET TIMING OFF")?;
        writeln!(input, "SET AUTOPRINT OFF")?;
        writeln!(input, "SET AUTOTRACE OFF")?;
        writeln!(input, "SET PAUSE OFF")?;
        // An unreadable release must not inherit a site ROWLIMIT. Try the
        // reset and tolerate only the older-client unknown-option diagnostic
        // inside the explicitly framed settings window below.
        writeln!(input, "SET ROWLIMIT OFF")?;
        writeln!(input, "SET CMDSEP OFF")?;
        writeln!(input, "SET SQLTERMINATOR \";\"")?;
        writeln!(input, "SET MARKUP HTML OFF")?;
        if client_version.is_none_or(sqlplus_supports_markup_csv) {
            writeln!(input, "SET MARKUP CSV OFF")?;
        }
        writeln!(input, "WHENEVER OSERROR EXIT FAILURE")?;
        // A failed CONNECT must be classified from its Oracle code, not made
        // indistinguishable from a client/loader exit by SQL*Plus itself.
        // Arm fail-closed SQL setup only after the positive AUTH query below.
        writeln!(input, "WHENEVER SQLERROR CONTINUE NONE")?;
        writeln!(
            input,
            "CONNECT {}/\"{}\"@{}",
            rendered_username,
            quoted_password.as_str(),
            connect_identifier,
        )?;
        // A site glogin.sql runs before stdin is read. Reassert all protocol
        // settings after CONNECT so benign site formatting cannot corrupt the
        // frame. Restricted mode blocks HOST and @ script execution from the
        // stream, but cannot suppress the installation-owned profile itself.
        write_authenticated_protocol_settings(input, client_version)?;
        writeln!(input, "SELECT '{marker}|AUTH' FROM SYS.DUAL;")?;
        writeln!(input, "WHENEVER SQLERROR EXIT FAILURE")?;
        // Repeat every protocol-critical client setting after the positive
        // AUTH frame. The parser deliberately clears installation-profile
        // diagnostics at AUTH; this second pass makes an unsupported or
        // rejected adapter-owned setting visible in the fail-closed window.
        writeln!(input, "PROMPT {marker}|SETTINGS-BEGIN")?;
        write_authenticated_protocol_settings(input, client_version)?;
        writeln!(input, "PROMPT {marker}|SETTINGS-END")?;
        writeln!(input, "ALTER SESSION SET NLS_NUMERIC_CHARACTERS = '.,';")?;
        writeln!(input, "ALTER SESSION SET NLS_CALENDAR = 'GREGORIAN';")?;
        writeln!(
            input,
            "ALTER SESSION SET NLS_DATE_FORMAT = 'YYYY-MM-DD\"T\"HH24:MI:SS';"
        )?;
        writeln!(
            input,
            "ALTER SESSION SET NLS_TIMESTAMP_FORMAT = 'YYYY-MM-DD\"T\"HH24:MI:SS.FF9';"
        )?;
        // Login and session setup fail closed above. Individual catalog
        // statements must instead return their classified failure and leave
        // the persistent session alive so optional families can degrade.
        writeln!(input, "WHENEVER SQLERROR CONTINUE NONE")?;
        writeln!(input, "PROMPT {marker}|READY")?;
        input.flush()
    }

    fn wait_for_ready(&mut self, deadline: Instant) -> Result<(), OracleSqlplusStartFailure> {
        let mut oracle_error = None;
        let mut authenticated = false;
        let mut error_context = false;
        let mut client_error = false;
        let mut settings_window = false;
        let login_input_started = true;
        loop {
            let line = match self.next_stdout_line(deadline, STARTUP_LINE_BYTES) {
                Ok(line) => line,
                Err(ReadFailure::Timeout) => {
                    return Err(OracleSqlplusStartFailure::StartupTimedOut)
                }
                Err(ReadFailure::Closed | ReadFailure::Io) => {
                    let oracle_error = oracle_error.or(self.last_oracle_code);
                    return Err(
                        match classify_login_observation(LoginEvidence {
                            authenticated,
                            oracle_error,
                            login_rejection_observed: self.login_rejection_observed,
                            client_error,
                        }) {
                            LoginObservation::Rejected => OracleSqlplusStartFailure::LoginRejected,
                            LoginObservation::Accepted
                            | LoginObservation::ClientFailure
                            | LoginObservation::Incomplete => {
                                OracleSqlplusStartFailure::ClientExitedBeforeReady
                            }
                        },
                    );
                }
                Err(ReadFailure::LimitExceeded) => {
                    return Err(OracleSqlplusStartFailure::StartupOutputLimitExceeded)
                }
                Err(ReadFailure::Malformed) => {
                    return Err(OracleSqlplusStartFailure::MalformedStartup)
                }
            };
            if line == format!("{}|READY", self.marker) {
                if settings_window {
                    return Err(OracleSqlplusStartFailure::MalformedStartup);
                }
                if oracle_error.is_none() {
                    if let Some(code) = self.last_oracle_code {
                        if code != -28002 {
                            oracle_error = Some(code);
                        }
                    }
                }
                match classify_login_observation(LoginEvidence {
                    authenticated,
                    oracle_error,
                    login_rejection_observed: self.login_rejection_observed,
                    client_error,
                }) {
                    LoginObservation::Accepted => {}
                    LoginObservation::Rejected => {
                        return Err(OracleSqlplusStartFailure::LoginRejected)
                    }
                    LoginObservation::ClientFailure => {
                        return Err(OracleSqlplusStartFailure::ClientExitedBeforeReady)
                    }
                    LoginObservation::Incomplete => {
                        return Err(OracleSqlplusStartFailure::MalformedStartup)
                    }
                }
                return Ok(());
            }
            if line == format!("{}|LOGIN-BEGIN", self.marker) {
                return Err(OracleSqlplusStartFailure::MalformedStartup);
            }
            if line == format!("{}|AUTH", self.marker) {
                if !login_input_started {
                    return Err(OracleSqlplusStartFailure::MalformedStartup);
                }
                // A site glogin.sql can run a best-effort query after CONNECT
                // and print an ORA diagnostic before our unpredictable AUTH
                // marker. Reaching that marker proves the connection and
                // SYS.DUAL query succeeded, so pre-marker diagnostics are not
                // login failures. Start the fail-closed session-setup window
                // here; errors after AUTH still reject admission.
                authenticated = true;
                oracle_error = None;
                self.last_oracle_code = None;
                self.login_rejection_observed = false;
                client_error = false;
                self.stderr_activity = false;
                self.stderr_tail.clear();
                error_context = false;
                continue;
            }
            if line == format!("{}|SETTINGS-BEGIN", self.marker) {
                if settings_window {
                    return Err(OracleSqlplusStartFailure::MalformedStartup);
                }
                // SQLPlus continues consuming queued input after a rejected
                // CONNECT. Such a marker is not proof that AUTH succeeded;
                // leave the login classifier to report the observed ORA code.
                settings_window = authenticated;
                continue;
            }
            if line == format!("{}|SETTINGS-END", self.marker) {
                if authenticated && !settings_window {
                    return Err(OracleSqlplusStartFailure::MalformedStartup);
                }
                settings_window = false;
                error_context = false;
                continue;
            }
            if line.starts_with("SP2-") {
                if settings_window && line.starts_with("SP2-0158:") {
                    // ROWLIMIT is asserted even when the client version is
                    // unknown or predates the setting. An unsupported reset
                    // is safe only inside this exact adapter-owned window;
                    // every other client diagnostic remains fatal.
                    error_context = false;
                    continue;
                }
                client_error = true;
                error_context = false;
                continue;
            }
            if let Some(code) = oracle_error_code(line.as_bytes()) {
                // ORA-28002 is a successful login with a password-expiry
                // warning. Preserve availability while requiring the positive
                // authenticated marker before admitting the session.
                if code != -28002 {
                    oracle_error = Some(code);
                }
                if is_login_rejection_code(code) {
                    self.login_rejection_observed = true;
                }
                error_context = false;
            } else if !line.trim().is_empty()
                && !line.starts_with("Connected to:")
                && !line.starts_with("Last Successful login time:")
            {
                // SQL*Plus prints `ERROR:` and source/caret context before a
                // numeric ORA line. Do not misclassify that prefix as a broken
                // adapter; a READY without a following ORA code is still
                // rejected below as malformed startup material.
                error_context = true;
            }
            if authenticated && error_context {
                return Err(OracleSqlplusStartFailure::MalformedStartup);
            }
        }
    }

    fn write_query(&mut self, execution: &OracleExecution<'_>) -> Result<(), ()> {
        let program = render_query_program(&self.marker, execution).ok_or(())?;
        let input = self.stdin.as_mut().ok_or(())?;
        input.write_all(program.as_bytes()).map_err(|_| ())?;
        input.flush().map_err(|_| ())
    }

    fn next_stdout_line(
        &mut self,
        deadline: Instant,
        max_line_bytes: usize,
    ) -> Result<String, ReadFailure> {
        loop {
            if let Some(line) = self.stdout_lines.pop_front() {
                return Ok(line);
            }
            if self.stdout_closed {
                return Err(ReadFailure::Closed);
            }
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or(ReadFailure::Timeout)?;
            let event = self
                .events
                .as_ref()
                .ok_or(ReadFailure::Closed)?
                .recv_timeout(remaining)
                .map_err(|error| match error {
                    RecvTimeoutError::Timeout => ReadFailure::Timeout,
                    RecvTimeoutError::Disconnected => ReadFailure::Closed,
                })?;
            match event {
                PipeEvent::Stdout(chunk) => self.accept_stdout(&chunk, max_line_bytes)?,
                PipeEvent::Stderr(chunk) => {
                    if !chunk.is_empty() {
                        self.stderr_activity = true;
                    }
                    self.stderr_tail.extend_from_slice(&chunk);
                    if self.stderr_tail.len() > STARTUP_LINE_BYTES {
                        let drain = self.stderr_tail.len() - STARTUP_LINE_BYTES;
                        self.stderr_tail.drain(..drain);
                    }
                    if let Some(code) = oracle_error_code(&self.stderr_tail) {
                        self.last_oracle_code = Some(code);
                    }
                    if contains_login_rejection_code(&self.stderr_tail) {
                        self.login_rejection_observed = true;
                    }
                }
                PipeEvent::StdoutClosed => self.stdout_closed = true,
                PipeEvent::StderrClosed => {}
                PipeEvent::ReadFailed => return Err(ReadFailure::Io),
            }
        }
    }

    fn accept_stdout(&mut self, chunk: &[u8], max_line_bytes: usize) -> Result<(), ReadFailure> {
        self.stdout_pending.extend_from_slice(chunk);
        while let Some(end) = self.stdout_pending.iter().position(|byte| *byte == b'\n') {
            let mut bytes = self.stdout_pending.drain(..=end).collect::<Vec<_>>();
            bytes.pop();
            if bytes.last() == Some(&b'\r') {
                bytes.pop();
            }
            if bytes.len() > max_line_bytes {
                return Err(ReadFailure::LimitExceeded);
            }
            let line = String::from_utf8(bytes).map_err(|_| ReadFailure::Malformed)?;
            self.stdout_lines.push_back(line);
        }
        if self.stdout_pending.len() > max_line_bytes {
            return Err(ReadFailure::LimitExceeded);
        }
        Ok(())
    }

    fn terminate_local(&mut self) {
        if self.terminated {
            return;
        }
        self.terminated = true;
        self.stdin.take();
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }

    fn malformed(&mut self) -> OracleExecuteResult {
        self.terminate_local();
        OracleExecuteResult::Failed(OracleQueryFailure::Malformed)
    }

    fn timeout(&mut self) -> OracleExecuteResult {
        self.terminate_local();
        OracleExecuteResult::Failed(OracleQueryFailure::Timeout)
    }
}

fn write_authenticated_protocol_settings(
    input: &mut impl Write,
    client_version: Option<OracleVersion>,
) -> std::io::Result<()> {
    writeln!(input, "SET SQLCASE MIXED")?;
    writeln!(input, "SET ECHO OFF")?;
    writeln!(input, "SET FEEDBACK OFF")?;
    writeln!(input, "SET HEADING OFF")?;
    writeln!(input, "SET PAGESIZE 0")?;
    writeln!(input, "SET LINESIZE 32767")?;
    writeln!(input, "SET WRAP OFF")?;
    writeln!(input, "SET TRIMOUT ON")?;
    writeln!(input, "SET TRIMSPOOL ON")?;
    writeln!(input, "SET VERIFY OFF")?;
    writeln!(input, "SET TERMOUT ON")?;
    writeln!(input, "SET TIMING OFF")?;
    writeln!(input, "SET AUTOPRINT OFF")?;
    writeln!(input, "SET AUTOTRACE OFF")?;
    writeln!(input, "SET PAUSE OFF")?;
    writeln!(input, "SET ROWLIMIT OFF")?;
    writeln!(input, "SET CMDSEP OFF")?;
    writeln!(input, "SET SQLTERMINATOR \";\"")?;
    writeln!(input, "SET MARKUP HTML OFF")?;
    if client_version.is_none_or(sqlplus_supports_markup_csv) {
        writeln!(input, "SET MARKUP CSV OFF")?;
    }
    Ok(())
}

impl OracleSessionAdapter for OracleSqlplusAdapter {
    fn execute(
        &mut self,
        execution: &OracleExecution<'_>,
        receiver: &mut dyn OracleRowReceiver,
    ) -> OracleExecuteResult {
        if self.terminated {
            return OracleExecuteResult::Failed(OracleQueryFailure::SessionLost);
        }
        self.last_oracle_code = None;
        self.stderr_activity = false;
        self.stderr_tail.clear();
        if execution.expected_columns == 0
            || execution.max_nesting_depth == 0
            || self.write_query(execution).is_err()
        {
            return self.malformed();
        }

        let Some(deadline) =
            Instant::now().checked_add(Duration::from_millis(execution.deadline_ms))
        else {
            return self.malformed();
        };
        // SQL*Plus itself cannot deliver a longer unwrapped result line. This
        // is a transport allocation bound; decoded value bytes are charged
        // independently by the shared receiver.
        let max_line = SQLPLUS_RESULT_LINE_BYTES;
        let mut begun = false;
        let mut completed_rows = 0_u64;
        let mut statement_failure = None;
        let mut transient_value_limit_exceeded = false;
        let mut unframed_error_context = false;

        loop {
            let line = match self.next_stdout_line(deadline, max_line) {
                Ok(line) => line,
                Err(ReadFailure::Timeout) => return self.timeout(),
                Err(ReadFailure::Closed | ReadFailure::Io) => {
                    self.terminate_local();
                    return OracleExecuteResult::Failed(OracleQueryFailure::SessionLost);
                }
                Err(ReadFailure::LimitExceeded) => {
                    self.terminate_local();
                    return OracleExecuteResult::Failed(OracleQueryFailure::TransportLimitExceeded);
                }
                Err(ReadFailure::Malformed) => return self.malformed(),
            };
            if line.trim().is_empty() {
                continue;
            }
            if !line.starts_with(&self.marker) {
                if let Some(code) = oracle_error_code(line.as_bytes()) {
                    self.last_oracle_code = Some(code);
                    statement_failure = Some(classify_oracle_error(code));
                    continue;
                }
                if begun {
                    // SQL*Plus prints source/caret context before its numeric
                    // ORA line. Retain only the fact that context occurred;
                    // END accepts it only when an ORA code follows.
                    unframed_error_context = true;
                    continue;
                }
                return self.malformed();
            }
            let fields = line.split('|').collect::<Vec<_>>();
            if fields.first().copied() != Some(self.marker.as_str()) {
                return self.malformed();
            }
            match fields.get(1).copied() {
                Some("BEGIN") if !begun && fields.len() == 3 => {
                    let Ok(width) = fields[2].parse::<usize>() else {
                        return self.malformed();
                    };
                    if width != execution.expected_columns {
                        return self.malformed();
                    }
                    begun = true;
                }
                Some("R")
                    if begun
                        && fields.len() == execution.expected_columns + 4
                        && fields.last().copied() == Some("Z") =>
                {
                    let row_number = fields.get(2).and_then(|value| value.parse::<u64>().ok());
                    if row_number != completed_rows.checked_add(1) {
                        return self.malformed();
                    }
                    let mut values = Vec::with_capacity(execution.expected_columns);
                    for token in &fields[3..fields.len() - 1] {
                        if *token == "N" {
                            values.push(OracleValue::Null);
                            continue;
                        }
                        if *token == "L" {
                            // The server query completed and the frame is
                            // still synchronized. Drain it, discard the whole
                            // query result at END, and let the session layer
                            // decide whether this optional family can degrade.
                            // Killing the process here would turn one long value
                            // in a non-core catalog into a capture-wide abort.
                            transient_value_limit_exceeded = true;
                            values.push(OracleValue::Null);
                            continue;
                        }
                        let Some(encoded) = token.strip_prefix('T') else {
                            return self.malformed();
                        };
                        let Ok(raw) = hex::decode(encoded) else {
                            return self.malformed();
                        };
                        let too_large = u64::try_from(raw.len())
                            .map_or(true, |bytes| bytes > execution.max_transient_value_bytes);
                        if too_large {
                            self.terminate_local();
                            return OracleExecuteResult::Failed(
                                OracleQueryFailure::TransientValueLimitExceeded,
                            );
                        }
                        let Ok(value) = String::from_utf8(raw) else {
                            return self.malformed();
                        };
                        values.push(OracleValue::Text(value));
                    }
                    completed_rows = match completed_rows.checked_add(1) {
                        Some(value) => value,
                        None => return self.malformed(),
                    };
                    if !transient_value_limit_exceeded
                        && receiver.receive(OracleRow { values }) == OracleRowControl::Stop
                    {
                        self.terminate_local();
                        return OracleExecuteResult::Complete;
                    }
                }
                Some("END") if begun && fields.len() == 3 => {
                    // NEW_VALUE receives an explicitly TO_CHAR-formatted
                    // value, so site NUMFORMAT/NUMWIDTH settings cannot turn
                    // it into hashes or scientific notation. Accept only
                    // ASCII surrounding whitespace around that integer.
                    let framed_rows = fields[2].trim().parse::<u64>().ok();
                    if framed_rows != Some(completed_rows) {
                        return self.malformed();
                    }
                    if statement_failure.is_none() {
                        statement_failure = self.last_oracle_code.map(classify_oracle_error);
                    }
                    if self.stderr_activity && statement_failure.is_none() {
                        return self.malformed();
                    }
                    if unframed_error_context && statement_failure.is_none() {
                        return self.malformed();
                    }
                    if transient_value_limit_exceeded {
                        return OracleExecuteResult::Failed(
                            OracleQueryFailure::TransientValueLimitExceeded,
                        );
                    }
                    return statement_failure
                        .map(OracleExecuteResult::Failed)
                        .unwrap_or(OracleExecuteResult::Complete);
                }
                _ => return self.malformed(),
            }
        }
    }

    fn cancel(&mut self) -> OracleCancelOutcome {
        // SQL*Plus process termination does not prove that Oracle stopped the
        // statement. The negotiated profile must therefore be
        // LocalProcessOnly and the shared boundary records unconfirmed work.
        self.terminate_local();
        OracleCancelOutcome::Unconfirmed
    }

    fn elapsed_ms(&self) -> u64 {
        self.started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
    }
}

impl Drop for OracleSqlplusAdapter {
    fn drop(&mut self) {
        self.terminate_local();
        // Drop the receiver before joining: a reader blocked on the bounded
        // channel then observes disconnection and exits instead of deadlocking
        // teardown.
        self.events.take();
        for reader in self.readers.drain(..) {
            let _ = reader.join();
        }
    }
}

/// Launch SQL*Plus and bind a fixed live probe to the exact process/session
/// before admission. Only numeric versions, a closed client-attestation state,
/// and digests enter capability evidence; native database identity is hashed,
/// while the effective principal is returned separately for the customer audit.
pub fn start_and_probe_sqlplus(
    config: OracleSqlplusConfig<'_>,
    limits: OracleProviderLimits,
) -> Result<OracleSqlplusProbedSession, OracleSqlplusProbeFailure> {
    if !sqlplus_limits_admissible(limits) {
        return Err(OracleSqlplusProbeFailure::LimitUnavailable);
    }
    let mut adapter =
        OracleSqlplusAdapter::start(config).map_err(OracleSqlplusProbeFailure::Start)?;
    let client_version = adapter.protocol_client_version();
    let client_version_attestation = adapter.client_version_attestation();
    let remaining_ms = limits.elapsed_ms.saturating_sub(adapter.elapsed_ms());
    if remaining_ms == 0 {
        return Err(OracleSqlplusProbeFailure::LimitUnavailable);
    }
    const PROBE_COLUMNS: &[&str] = &[
        "effective_principal",
        "database_identity",
        "container_identity",
        "server_version",
    ];
    let primary_probe = OracleExecution {
        sql: "SELECT SYS_CONTEXT('USERENV','CURRENT_USER') AS effective_principal, \
              SYS_CONTEXT('USERENV','DB_NAME') || '/' || SYS_CONTEXT('USERENV','CON_NAME') AS database_identity, \
              SYS_CONTEXT('USERENV','CON_ID') AS container_identity, \
              version AS server_version FROM SYS.V_$INSTANCE",
        owner: None,
        expected_columns: PROBE_COLUMNS.len(),
        projected_columns: PROBE_COLUMNS,
        deadline_ms: remaining_ms,
        max_catalog_rows: 2,
        max_catalog_bytes: 64 * 1024,
        max_transient_value_bytes: 4096,
        max_lob_bytes: 1,
        max_nesting_depth: 1,
    };
    let mut receiver = ProbeRows(Vec::new());
    let mut server_version_source = b"v-instance".as_slice();
    if let OracleExecuteResult::Failed(_) = adapter.execute(&primary_probe, &mut receiver) {
        // The minimum grant includes V_$INSTANCE, but a positive collector
        // should still use the widely available PRODUCT_COMPONENT_VERSION
        // fallback when a site has independently withdrawn or filtered that
        // primary authority. Tests exercise the fallback; it is resilience,
        // not an ungranted minimum-tier dependency.
        let fallback_remaining_ms = limits.elapsed_ms.saturating_sub(adapter.elapsed_ms());
        if fallback_remaining_ms == 0 {
            return Err(OracleSqlplusProbeFailure::LimitUnavailable);
        }
        let fallback_probe = OracleExecution {
            sql: "SELECT SYS_CONTEXT('USERENV','CURRENT_USER') AS effective_principal, \
                  SYS_CONTEXT('USERENV','DB_NAME') || '/' || SYS_CONTEXT('USERENV','CON_NAME') AS database_identity, \
                  SYS_CONTEXT('USERENV','CON_ID') AS container_identity, \
                  (SELECT version FROM SYS.PRODUCT_COMPONENT_VERSION \
                    WHERE product LIKE 'Oracle Database%' AND ROWNUM = 1) AS server_version \
                  FROM SYS.DUAL",
            owner: None,
            expected_columns: PROBE_COLUMNS.len(),
            projected_columns: PROBE_COLUMNS,
            deadline_ms: fallback_remaining_ms,
            max_catalog_rows: 2,
            max_catalog_bytes: 64 * 1024,
            max_transient_value_bytes: 4096,
            max_lob_bytes: 1,
            max_nesting_depth: 1,
        };
        receiver = ProbeRows(Vec::new());
        server_version_source = b"product-component-version";
        if let OracleExecuteResult::Failed(failure) =
            adapter.execute(&fallback_probe, &mut receiver)
        {
            return Err(OracleSqlplusProbeFailure::Query(failure));
        }
    }
    if receiver.0.len() != 1 || receiver.0[0].values.len() != PROBE_COLUMNS.len() {
        return Err(OracleSqlplusProbeFailure::MalformedEvidence);
    }
    let values = receiver
        .0
        .pop()
        .ok_or(OracleSqlplusProbeFailure::MalformedEvidence)?
        .values;
    let mut fields = values
        .into_iter()
        .map(|value| match value {
            OracleValue::Text(value) if !value.trim().is_empty() => Some(value),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(OracleSqlplusProbeFailure::MalformedEvidence)?;
    let server_version_text = fields
        .pop()
        .ok_or(OracleSqlplusProbeFailure::MalformedEvidence)?;
    let container_identity = fields
        .pop()
        .ok_or(OracleSqlplusProbeFailure::MalformedEvidence)?;
    let database_identity = fields
        .pop()
        .ok_or(OracleSqlplusProbeFailure::MalformedEvidence)?;
    let effective_principal = fields
        .pop()
        .ok_or(OracleSqlplusProbeFailure::MalformedEvidence)?;
    let server_version = parse_numeric_version(&server_version_text)
        .ok_or(OracleSqlplusProbeFailure::MalformedEvidence)?;
    let container_id = container_identity
        .parse::<u64>()
        .map_err(|_| OracleSqlplusProbeFailure::MalformedEvidence)?;
    require_supported_container(server_version, container_id)
        .map_err(OracleSqlplusProbeFailure::Start)?;

    let provider_artifact_sha256 = adapter.provider_artifact_sha256();
    let session_binding_sha256 = hash_bound_fields(
        adapter.marker.as_bytes(),
        &[
            effective_principal.as_bytes(),
            database_identity.as_bytes(),
            container_identity.as_bytes(),
            server_version_text.as_bytes(),
            server_version_source,
        ],
    );
    let client_version_bytes = client_version.map(version_bytes).unwrap_or_default();
    let client_attestation_bytes = client_version_attestation.evidence_token().as_bytes();
    let server_version_bytes = version_bytes(server_version);
    let transcript_sha256 = hash_bound_fields(
        b"dbwarp-blueprint-oracle-sqlplus-probe-v1",
        &[
            &client_version_bytes,
            client_attestation_bytes,
            &server_version_bytes,
            &provider_artifact_sha256,
            &session_binding_sha256,
            b"direct-service",
            b"tcp",
            b"password",
            b"local-process-only",
        ],
    );
    let profile = OracleConnectionProfile {
        endpoint_mode: OracleEndpointMode::DirectService,
        transport: OracleTransport::Tcp,
        authentication: OracleAuthentication::Password,
        trust_source: OracleTrustSource::NotApplicable,
        cancellation: OracleCancellation::LocalProcessOnly,
        local_inputs: BTreeSet::from([OracleLocalInput::ClientExecutable]),
    };
    let observed = OracleCapabilitySet {
        bind_variables: true,
        typed_scalar_results: true,
        limits,
        connection_profiles: BTreeSet::from([profile]),
        tiers: BTreeSet::from([OracleCaptureTier::Basic]),
        artifact_details: BTreeSet::from([ArtifactDetail::None, ArtifactDetail::Summary]),
    };
    let declaration = OracleProviderDeclaration {
        contract_version: ORACLE_PROVIDER_CONTRACT_VERSION,
        provider: OracleProviderKind::Sqlplus,
        maximum: sqlplus_basic_capabilities(SQLPLUS_BASIC_MAX_LIMITS),
    };
    let evidence = oracle_live_probe_evidence(
        OracleProviderKind::Sqlplus,
        client_version_attestation,
        server_version,
        provider_artifact_sha256,
        transcript_sha256,
        session_binding_sha256,
        observed,
    );
    Ok(OracleSqlplusProbedSession {
        adapter,
        declaration,
        evidence,
        server_version,
        effective_principal,
        database_identity,
    })
}

struct ProbeRows(Vec<OracleRow>);

impl OracleRowReceiver for ProbeRows {
    fn receive(&mut self, row: OracleRow) -> OracleRowControl {
        if self.0.len() >= 2 {
            return OracleRowControl::Stop;
        }
        self.0.push(row);
        OracleRowControl::Continue
    }
}

fn parse_numeric_version(value: &str) -> Option<OracleVersion> {
    let components = value
        .trim()
        .split('.')
        .map(|component| {
            if component.is_empty() || !component.bytes().all(|byte| byte.is_ascii_digit()) {
                None
            } else {
                component.parse::<u16>().ok()
            }
        })
        .collect::<Option<Vec<_>>>()?;
    OracleVersion::from_components(&components)
}

fn sqlplus_limits_admissible(limits: OracleProviderLimits) -> bool {
    limits.catalog_rows > 0
        && limits.catalog_rows <= SQLPLUS_BASIC_MAX_LIMITS.catalog_rows
        && limits.catalog_bytes > 0
        && limits.catalog_bytes <= SQLPLUS_BASIC_MAX_LIMITS.catalog_bytes
        && limits.result_nesting_depth > 0
        && limits.result_nesting_depth <= SQLPLUS_BASIC_MAX_LIMITS.result_nesting_depth
        && limits.elapsed_ms > 0
        && limits.elapsed_ms <= SQLPLUS_BASIC_MAX_LIMITS.elapsed_ms
        && limits.transient_value_bytes > 0
        && limits.transient_value_bytes <= SQLPLUS_BASIC_MAX_LIMITS.transient_value_bytes
        && limits.transient_value_bytes <= limits.catalog_bytes
        && limits.lob_bytes > 0
        && limits.lob_bytes <= SQLPLUS_BASIC_MAX_LIMITS.lob_bytes
        && limits.lob_bytes <= limits.catalog_bytes
}

fn sqlplus_basic_capabilities(limits: OracleProviderLimits) -> OracleCapabilitySet {
    OracleCapabilitySet {
        bind_variables: true,
        typed_scalar_results: true,
        limits,
        connection_profiles: BTreeSet::from([OracleConnectionProfile {
            endpoint_mode: OracleEndpointMode::DirectService,
            transport: OracleTransport::Tcp,
            authentication: OracleAuthentication::Password,
            trust_source: OracleTrustSource::NotApplicable,
            cancellation: OracleCancellation::LocalProcessOnly,
            local_inputs: BTreeSet::from([OracleLocalInput::ClientExecutable]),
        }]),
        tiers: BTreeSet::from([OracleCaptureTier::Basic]),
        artifact_details: BTreeSet::from([ArtifactDetail::None, ArtifactDetail::Summary]),
    }
}

fn version_bytes(version: OracleVersion) -> Vec<u8> {
    version
        .components()
        .iter()
        .flat_map(|component| component.to_be_bytes())
        .collect()
}

fn require_supported_container(
    server_version: OracleVersion,
    container_id: u64,
) -> Result<(), OracleSqlplusStartFailure> {
    if server_version.components()[0] >= 12 && container_id == 1 {
        Err(OracleSqlplusStartFailure::RootContainerUnsupported)
    } else {
        Ok(())
    }
}

fn hash_bound_fields(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    for field in fields {
        digest.update((field.len() as u128).to_be_bytes());
        digest.update(field);
    }
    digest.finalize().into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadFailure {
    Timeout,
    Closed,
    Io,
    LimitExceeded,
    Malformed,
}

fn configured_sqlplus_command(executable: &Path, network_sandbox: &Path) -> Command {
    let mut command = Command::new(executable);
    command
        .current_dir(network_sandbox)
        // Do not disclose the collector's credential/path environment to a
        // vendor command process or installation profile. Retain only loader
        // variables needed to start the explicitly selected executable.
        .env_clear()
        .env("NLS_LANG", ".AL32UTF8")
        .env("ORA_SDTZ", "UTC")
        .env("TNS_ADMIN", network_sandbox)
        .env_remove("TWO_TASK")
        .env_remove("LOCAL")
        .env_remove("SQLPATH")
        .env_remove("ORACLE_PATH");
    for variable in SQLPLUS_FORWARDED_LOADER_VARIABLES {
        if let Some(value) = std::env::var_os(variable) {
            command.env(variable, value);
        }
    }
    command
}

/// Loader variables forwarded from the collector environment so the explicitly
/// selected SQL*Plus executable can start. They choose the client libraries,
/// which the executable fingerprint does not cover, so callers audit which of
/// them were present.
pub const SQLPLUS_FORWARDED_LOADER_VARIABLES: &[&str] = &[
    "PATH",
    "SystemRoot",
    "WINDIR",
    "ORACLE_HOME",
    "LD_LIBRARY_PATH",
    "DYLD_LIBRARY_PATH",
    "LIBPATH",
    "SHLIB_PATH",
];

fn probe_sqlplus_executable_version(
    executable: &Path,
    network_sandbox: &Path,
    timeout: Duration,
) -> Option<OracleVersion> {
    // Version provenance must not consume the whole connection budget before
    // the real process is even launched. A slow or hung `-V` is unverified
    // provenance, not a reason to delay catalog admission indefinitely.
    let deadline = Instant::now().checked_add(timeout.min(Duration::from_secs(2)))?;
    let mut command = configured_sqlplus_command(executable, network_sandbox);
    let mut child = command
        .arg("-V")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
        let _ = child.kill();
        let _ = child.wait();
        return None;
    };
    let (sender, receiver) = mpsc::sync_channel(PIPE_CHANNEL_DEPTH);
    let readers = vec![
        spawn_pipe_reader(stdout, sender.clone(), true),
        spawn_pipe_reader(stderr, sender, false),
    ];
    let mut output = Vec::new();
    let mut closed_streams = 0_u8;
    let mut failure = None;
    while closed_streams < 2 {
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            failure = Some(OracleSqlplusStartFailure::StartupTimedOut);
            break;
        };
        match receiver.recv_timeout(remaining) {
            Ok(PipeEvent::Stdout(chunk) | PipeEvent::Stderr(chunk)) => {
                if output
                    .len()
                    .checked_add(chunk.len())
                    .is_none_or(|length| length > STARTUP_LINE_BYTES)
                {
                    failure = Some(OracleSqlplusStartFailure::StartupOutputLimitExceeded);
                    break;
                }
                output.extend_from_slice(&chunk);
            }
            Ok(PipeEvent::StdoutClosed | PipeEvent::StderrClosed) => {
                closed_streams += 1;
            }
            Ok(PipeEvent::ReadFailed) => {
                failure = Some(OracleSqlplusStartFailure::ClientExitedBeforeReady);
                break;
            }
            Err(RecvTimeoutError::Timeout) => {
                failure = Some(OracleSqlplusStartFailure::StartupTimedOut);
                break;
            }
            Err(RecvTimeoutError::Disconnected) => {
                failure = Some(OracleSqlplusStartFailure::ClientExitedBeforeReady);
                break;
            }
        }
    }
    if failure.is_some() {
        let _ = child.kill();
    }
    let status = child.wait().ok()?;
    drop(receiver);
    for reader in readers {
        let _ = reader.join();
    }
    if failure.is_some() || !status.success() {
        return None;
    }
    parse_sqlplus_version_banner(&output)
}

fn parse_sqlplus_version_banner(output: &[u8]) -> Option<OracleVersion> {
    let text = std::str::from_utf8(output).ok()?;
    let mut release_version = None;
    for line in text.lines() {
        let line = line.trim();
        if let Some(version) = line.strip_prefix("Version ") {
            if let Some(version) = parse_dot_version(version) {
                return Some(version);
            }
            continue;
        }
        if !line.contains("SQL*Plus") {
            continue;
        }
        let Some((_, release)) = line.split_once("Release") else {
            continue;
        };
        release_version = parse_sqlplus_base_release(release);
    }
    release_version
}

/// A `Release 19.0.0.0.0` line identifies the base release, not a claim that
/// the installed release update is zero. Preserve only the components that
/// line actually carries; a following `Version` line, when present, remains
/// the authoritative full client version.
fn parse_sqlplus_base_release(value: &str) -> Option<OracleVersion> {
    let version = parse_dot_version(value)?;
    let components = version.components();
    let carried = components
        .iter()
        .rposition(|component| *component != 0)
        .map_or(1, |index| index + 1);
    OracleVersion::from_components(&components[..carried])
}

fn parse_dot_version(value: &str) -> Option<OracleVersion> {
    let numeric = value
        .split_ascii_whitespace()
        .next()?
        .trim_matches(|character: char| !character.is_ascii_digit() && character != '.');
    let components = numeric
        .split('.')
        .map(|component| component.parse::<u16>().ok())
        .collect::<Option<Vec<_>>>()?;
    if components.iter().any(|component| *component > 99) {
        return None;
    }
    OracleVersion::from_components(&components)
}

fn spawn_pipe_reader(
    mut pipe: impl Read + Send + 'static,
    sender: SyncSender<PipeEvent>,
    stdout: bool,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let mut buffer = vec![0_u8; PIPE_CHUNK_BYTES];
        loop {
            match pipe.read(&mut buffer) {
                Ok(0) => {
                    let _ = sender.send(if stdout {
                        PipeEvent::StdoutClosed
                    } else {
                        PipeEvent::StderrClosed
                    });
                    break;
                }
                Ok(count) => {
                    let event = if stdout {
                        PipeEvent::Stdout(buffer[..count].to_vec())
                    } else {
                        PipeEvent::Stderr(buffer[..count].to_vec())
                    };
                    if sender.send(event).is_err() {
                        break;
                    }
                }
                Err(_) => {
                    let _ = sender.send(PipeEvent::ReadFailed);
                    break;
                }
            }
        }
    })
}

fn sha256_file(path: &Path) -> Result<[u8; 32], OracleSqlplusStartFailure> {
    let mut file = File::open(path).map_err(|_| OracleSqlplusStartFailure::ExecutableReadFailed)?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    let mut bytes_read = 0_u64;
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| OracleSqlplusStartFailure::ExecutableReadFailed)?;
        if count == 0 {
            break;
        }
        bytes_read = bytes_read
            .checked_add(
                u64::try_from(count).map_err(|_| OracleSqlplusStartFailure::ExecutableTooLarge)?,
            )
            .ok_or(OracleSqlplusStartFailure::ExecutableTooLarge)?;
        if bytes_read > MAX_CLIENT_EXECUTABLE_BYTES {
            return Err(OracleSqlplusStartFailure::ExecutableTooLarge);
        }
        digest.update(&buffer[..count]);
    }
    Ok(digest.finalize().into())
}

fn validate_config(config: &OracleSqlplusConfig<'_>) -> Result<(), OracleSqlplusStartFailure> {
    if !config.executable.is_absolute() {
        return Err(OracleSqlplusStartFailure::ExecutablePathNotAbsolute);
    }
    if render_sqlplus_username(config.username).is_none() {
        return Err(OracleSqlplusStartFailure::InvalidUsername);
    }
    if !valid_direct_service(config.connect_identifier) {
        return Err(OracleSqlplusStartFailure::InvalidConnectIdentifier);
    }
    if invalid_line_value(config.password.expose())
        || config.password.expose().len() > MAX_SQLPLUS_PASSWORD_BYTES
    {
        return Err(OracleSqlplusStartFailure::UnsupportedPassword);
    }
    if !network_sandbox_is_private_and_empty(config.isolated_network_config_dir) {
        return Err(OracleSqlplusStartFailure::NetworkSandboxUnavailable);
    }
    if config.startup_timeout.is_zero() {
        return Err(OracleSqlplusStartFailure::StartupTimedOut);
    }
    Ok(())
}

fn render_sqlplus_username(value: &str) -> Option<String> {
    if value.is_empty() || invalid_line_value(value) {
        return None;
    }
    if value.starts_with('"') && value.ends_with('"') && value.len() > 2 {
        let inner = &value[1..value.len() - 1];
        if inner.len() > MAX_SQLPLUS_USERNAME_BYTES {
            return None;
        }
        let mut characters = inner.chars();
        let first = characters.next()?;
        if inner.contains('"')
            || !first.is_ascii_alphabetic()
            || !characters.all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '$' | '#')
            })
        {
            return None;
        }
        return Some(value.to_string());
    }
    if value.len() > MAX_SQLPLUS_USERNAME_BYTES {
        return None;
    }
    let mut characters = value.chars();
    let first = characters.next()?;
    if !first.is_ascii_alphabetic()
        || !characters.all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '$' | '#')
        })
    {
        return None;
    }
    // Unquoted Oracle identifiers are deliberately left unquoted so the
    // server applies its ordinary uppercase folding. Callers representing a
    // genuinely quoted account must supply the surrounding double quotes.
    Some(value.to_string())
}

fn network_sandbox_is_private_and_empty(path: &Path) -> bool {
    if !path.is_absolute()
        || !path
            .read_dir()
            .is_ok_and(|mut entries| entries.next().is_none())
    {
        return false;
    }
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    if !metadata.is_dir() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        // SAFETY: geteuid has no pointer arguments or preconditions.
        let effective_uid = unsafe { libc::geteuid() };
        if metadata.uid() != effective_uid || metadata.permissions().mode() & 0o077 != 0 {
            return false;
        }
        // Every parent must be controlled by the effective user or root and
        // must not be writable by group/other. Checking only the leaf allowed
        // another user to replace an ancestor between validation and launch.
        let mut ancestor = path.parent();
        while let Some(directory) = ancestor {
            let Ok(metadata) = directory.metadata() else {
                return false;
            };
            if !metadata.is_dir()
                || (metadata.uid() != 0 && metadata.uid() != effective_uid)
                || metadata.permissions().mode() & 0o022 != 0
            {
                return false;
            }
            ancestor = directory.parent();
        }
    }
    #[cfg(not(unix))]
    {
        // Unix mode checks do not establish a private directory on another
        // platform, so this check does not admit one there.
        return false;
    }
    true
}

fn invalid_line_value(value: &str) -> bool {
    value.chars().any(char::is_control)
}

fn valid_direct_service(value: &str) -> bool {
    if !value.starts_with("//") || value.len() > 1024 || invalid_line_value(value) {
        return false;
    }
    let Some((host_port, service)) = value[2..].split_once('/') else {
        return false;
    };
    if host_port.is_empty()
        || service.is_empty()
        || service.contains('/')
        || !service.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        })
    {
        return false;
    }
    let (host, port) = if let Some(bracketed) = host_port.strip_prefix('[') {
        let Some((host, port)) = bracketed.split_once("]:") else {
            return false;
        };
        if host.parse::<std::net::Ipv6Addr>().is_err() {
            return false;
        }
        (host, port)
    } else {
        let Some((host, port)) = host_port.rsplit_once(':') else {
            return false;
        };
        if host.is_empty()
            || !host.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
            })
        {
            return false;
        }
        (host, port)
    };
    !host.is_empty() && port.parse::<u16>().is_ok_and(|port| port != 0)
}

fn render_query_program(marker: &str, execution: &OracleExecution<'_>) -> Option<String> {
    if execution.expected_columns == 0
        || execution.expected_columns > 256
        || execution.projected_columns.len() != execution.expected_columns
        || execution.projected_columns.iter().any(|column| {
            column.is_empty()
                || !column
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '_')
        })
    {
        return None;
    }
    let sql = execution.sql;
    // DEFINE is enabled only around the per-query row-count variable. Registry
    // SQL must therefore contain no substitution marker of its own.
    if sql.len() > 24_000 || sql.contains(';') || sql.contains('&') {
        return None;
    }
    if execution
        .owner
        .is_some_and(|owner| owner.is_empty() || owner.len() > 128 || invalid_line_value(owner))
    {
        return None;
    }
    let bind = execution.owner.map_or_else(String::new, |owner| {
        let encoded = hex::encode(owner.as_bytes());
        format!(
            "VARIABLE owner VARCHAR2(128)\n\
             BEGIN\n\
               :owner := UTL_I18N.RAW_TO_CHAR(HEXTORAW('{encoded}'), 'AL32UTF8');\n\
             END;\n\
             /\n"
        )
    });
    let encoded_columns = execution
        .projected_columns
        .iter()
        .map(|column| {
            format!(
                "CASE WHEN q.{column} IS NULL THEN 'N' \
                 WHEN LENGTHB(TO_CHAR(q.{column})) > {MAX_SCALAR_UTF8_BYTES} THEN 'L' \
                 ELSE 'T' || RAWTOHEX(UTL_I18N.STRING_TO_RAW(TO_CHAR(q.{column}), \
                 'AL32UTF8')) END"
            )
        })
        .collect::<Vec<_>>()
        // SQL*Plus 21c rejects an input line longer than 2,499 characters.
        // Keep one bounded encoder expression per line so wider catalog
        // projections cannot trip that client-side ceiling.
        .join(" || '|' ||\n");
    Some(format!(
        "{bind}\
         SET DEFINE ON\n\
         DEFINE DBWARP_BP_ROW_COUNT = 0\n\
         COLUMN DBWARP_BP_ROW_COUNT NEW_VALUE DBWARP_BP_ROW_COUNT NOPRINT\n\
         COLUMN DBWARP_BP_FRAME FORMAT A{frame_display_bytes}\n\
         PROMPT {marker}|BEGIN|{expected}\n\
         SELECT '{marker}|R|' || TO_CHAR(ROWNUM, 'FM99999999999999999990') || '|' ||\n\
           {encoded_columns} || '|Z' AS DBWARP_BP_FRAME,\n\
           TO_CHAR(ROWNUM, 'FM99999999999999999990') AS DBWARP_BP_ROW_COUNT\n\
         FROM ({sql}) q;\n\
         PROMPT {marker}|END|&DBWARP_BP_ROW_COUNT\n\
         UNDEFINE DBWARP_BP_ROW_COUNT\n\
         COLUMN DBWARP_BP_ROW_COUNT CLEAR\n\
         COLUMN DBWARP_BP_FRAME CLEAR\n\
         SET DEFINE OFF\n",
        expected = execution.expected_columns,
        frame_display_bytes = SQLPLUS_FRAME_DISPLAY_BYTES,
    ))
}

fn oracle_error_code(bytes: &[u8]) -> Option<i32> {
    // Retain the most recent code. A successful password-grace warning can be
    // followed by a real post-login setup failure in the same stderr tail;
    // keeping the first code would admit that broken session.
    let position = bytes.windows(4).rposition(|window| window == b"ORA-")?;
    let digits = bytes.get(position + 4..position + 9)?;
    if !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let value = std::str::from_utf8(digits).ok()?.parse::<i32>().ok()?;
    Some(-value)
}

fn is_login_rejection_code(code: i32) -> bool {
    matches!(code, -1017 | -1045 | -28000 | -28001)
}

fn contains_login_rejection_code(bytes: &[u8]) -> bool {
    bytes.windows(9).any(|candidate| {
        candidate.starts_with(b"ORA-")
            && candidate[4..9].iter().all(u8::is_ascii_digit)
            && std::str::from_utf8(&candidate[4..9])
                .ok()
                .and_then(|digits| digits.parse::<i32>().ok())
                .is_some_and(|code| is_login_rejection_code(-code))
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoginObservation {
    Accepted,
    Rejected,
    ClientFailure,
    Incomplete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LoginEvidence {
    authenticated: bool,
    oracle_error: Option<i32>,
    login_rejection_observed: bool,
    client_error: bool,
}

/// One classifier owns both READY and early-exit interpretation. In
/// particular, listener/setup/loader errors must never be rendered as bad
/// credentials merely because they occurred during startup.
fn classify_login_observation(evidence: LoginEvidence) -> LoginObservation {
    if !evidence.authenticated && evidence.login_rejection_observed {
        return LoginObservation::Rejected;
    }
    if evidence.client_error {
        return LoginObservation::ClientFailure;
    }
    match (evidence.authenticated, evidence.oracle_error) {
        (true, None | Some(-28002)) => LoginObservation::Accepted,
        (false, Some(code)) if is_login_rejection_code(code) => LoginObservation::Rejected,
        (_, Some(_)) => LoginObservation::ClientFailure,
        (false, None) => LoginObservation::Incomplete,
    }
}

fn classify_oracle_error(code: i32) -> OracleQueryFailure {
    match code.abs() {
        942 => OracleQueryFailure::ObjectAbsent,
        1031 | 1017 => OracleQueryFailure::PermissionDenied,
        1013 => OracleQueryFailure::Timeout,
        28 | 3113 | 3114 | 3135 | 12537 | 12547 => OracleQueryFailure::SessionLost,
        _ => OracleQueryFailure::DatabaseError,
    }
}

fn parse_sqlplus_release(value: &str) -> Option<OracleVersion> {
    let digits = value.trim();
    if digits.len() != 10 || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let components = (0..5)
        .map(|index| digits[index * 2..index * 2 + 2].parse::<u16>().ok())
        .collect::<Option<Vec<_>>>()?;
    OracleVersion::from_components(&components)
}

fn sqlplus_supports_rowlimit(version: OracleVersion) -> bool {
    version.is_at_least(18, 0)
}

fn sqlplus_supports_markup_csv(version: OracleVersion) -> bool {
    version.is_at_least(12, 2)
}

fn sqlplus_client_versions_compatible(
    in_session: Option<OracleVersion>,
    executable_banner: Option<OracleVersion>,
) -> OracleClientVersionAttestation {
    let (Some(in_session), Some(executable_banner)) = (in_session, executable_banner) else {
        return OracleClientVersionAttestation::Unreadable;
    };
    let shared_components = in_session
        .components()
        .len()
        .min(executable_banner.components().len());
    let in_session_components = in_session.components();
    let executable_components = executable_banner.components();
    if in_session_components[..shared_components] != executable_components[..shared_components] {
        return OracleClientVersionAttestation::Mismatched;
    }
    // Matching a base-release prefix is compatible, but it is not full
    // attestation when either reading carries a non-zero component the other
    // did not independently establish. Continue the capture with weakened
    // provenance rather than labelling a healthy base-release banner drift.
    if in_session_components[shared_components..]
        .iter()
        .chain(executable_components[shared_components..].iter())
        .any(|component| *component != 0)
    {
        return OracleClientVersionAttestation::Unreadable;
    }
    OracleClientVersionAttestation::Attested(in_session)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static PROCESS_ENVIRONMENT_LOCK: Mutex<()> = Mutex::new(());

    struct Collected(Vec<OracleRow>);

    impl OracleRowReceiver for Collected {
        fn receive(&mut self, row: OracleRow) -> OracleRowControl {
            self.0.push(row);
            OracleRowControl::Continue
        }
    }

    fn execution<'a>(sql: &'a str, owner: Option<&'a str>) -> OracleExecution<'a> {
        OracleExecution {
            sql,
            owner,
            expected_columns: 2,
            projected_columns: &["owner", "value"],
            deadline_ms: 1_000,
            max_catalog_rows: 100,
            max_catalog_bytes: 1_000_000,
            max_transient_value_bytes: 16_000,
            max_lob_bytes: 1_000,
            max_nesting_depth: 2,
        }
    }

    #[test]
    fn direct_service_validation_rejects_sqlplus_command_injection() {
        assert!(valid_direct_service("//db.example:1521/APPPDB"));
        assert!(valid_direct_service("//[2001:db8::1]:1521/APPPDB"));
        assert!(!valid_direct_service("db.example:1521/APPPDB"));
        assert!(!valid_direct_service("///APPPDB"));
        assert!(!valid_direct_service("//db.example/APPPDB"));
        assert!(!valid_direct_service("//db.example:0/APPPDB"));
        assert!(!valid_direct_service("//db.example:1521/"));
        assert!(!valid_direct_service("//db.example:1521/A/B"));
        assert!(!valid_direct_service("//db.example:1521/APPPDB\nHOST x"));
        assert!(!valid_direct_service("//db.example:1521/APPPDB@evil"));
    }

    #[test]
    fn username_rendering_preserves_ordinary_oracle_folding() {
        assert_eq!(
            render_sqlplus_username("collector").as_deref(),
            Some("collector")
        );
        assert_eq!(
            render_sqlplus_username("\"MixedCase\"").as_deref(),
            Some("\"MixedCase\"")
        );
        assert!(render_sqlplus_username("mixed-case").is_none());
        assert!(render_sqlplus_username("\"bad\"quote\"").is_none());
        assert!(render_sqlplus_username("\" \"").is_none());
        assert!(render_sqlplus_username("\"a/b\"").is_none());
        assert!(render_sqlplus_username("\"x@y\"").is_none());
        let maximum = format!("\"A{}\"", "B".repeat(127));
        assert_eq!(
            render_sqlplus_username(&maximum).as_deref(),
            Some(maximum.as_str())
        );
        assert!(render_sqlplus_username(&format!("\"A{}\"", "B".repeat(128))).is_none());
    }

    #[test]
    fn every_client_resets_rowlimit_while_pre_12_2_clients_skip_markup_csv() {
        assert!(!sqlplus_supports_rowlimit(
            OracleVersion::from_components(&[12, 2]).unwrap()
        ));
        assert!(sqlplus_supports_rowlimit(
            OracleVersion::from_components(&[18, 0]).unwrap()
        ));
        assert!(!sqlplus_supports_markup_csv(
            OracleVersion::from_components(&[12, 1]).unwrap()
        ));
        assert!(sqlplus_supports_markup_csv(
            OracleVersion::from_components(&[12, 2]).unwrap()
        ));

        let mut conservative = Vec::new();
        write_authenticated_protocol_settings(&mut conservative, None).unwrap();
        let conservative = String::from_utf8(conservative).unwrap();
        assert!(conservative.contains("SET ROWLIMIT OFF"));
        assert!(conservative.contains("SET MARKUP CSV OFF"));

        let mut pre_12_2 = Vec::new();
        write_authenticated_protocol_settings(
            &mut pre_12_2,
            Some(OracleVersion::from_components(&[11, 2]).unwrap()),
        )
        .unwrap();
        let pre_12_2 = String::from_utf8(pre_12_2).unwrap();
        assert!(pre_12_2.contains("SET ROWLIMIT OFF"));
        assert!(!pre_12_2.contains("SET MARKUP CSV"));

        let mut current = Vec::new();
        write_authenticated_protocol_settings(
            &mut current,
            Some(OracleVersion::from_components(&[21, 3]).unwrap()),
        )
        .unwrap();
        let current = String::from_utf8(current).unwrap();
        assert!(current.contains("SET ROWLIMIT OFF"));
        assert!(current.contains("SET MARKUP CSV OFF"));
    }

    #[cfg(unix)]
    #[test]
    fn login_parser_distinguishes_rejection_from_password_grace_warning() {
        use std::os::unix::fs::PermissionsExt;

        let _environment_guard = PROCESS_ENVIRONMENT_LOCK.lock().unwrap();

        fn shell_literal(value: &str) -> String {
            format!("'{}'", value.replace('\'', "'\"'\"'"))
        }

        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tmp/tests")
            .join(format!("oracle-sqlplus-login-{}", std::process::id()));
        let network = root.join("network-empty");
        std::fs::create_dir_all(&network).unwrap();
        std::fs::set_permissions(&network, std::fs::Permissions::from_mode(0o700)).unwrap();
        let rejected = root.join("sqlplus-rejected.sh");
        let rejected_transcript =
            include_str!("../tests/fixtures/oracle_sqlplus/wrong-password.txt");
        std::fs::write(
            &rejected,
            format!(
                "#!/bin/sh\n[ \"${{1:-}}\" != \"-V\" ] || {{ printf 'SQL*Plus: Release 21.0.0.0.0 - Production\\nVersion 21.3.0.0.0\\n'; exit 0; }}\ntranscript={}\nmarker=\nwhile IFS= read -r line; do\n  case \"$line\" in\n    PROMPT\\ __DBWARP_BP_*\\|LOGIN-BEGIN) marker=${{line#PROMPT }}; marker=${{marker%|LOGIN-BEGIN}}; printf '%s|LOGIN-BEGIN\\n' \"$marker\";;\n    PROMPT\\ __DBWARP_BP_*\\|CLIENT\\|*) printf '%s|CLIENT|2103000000\\n' \"$marker\";;\n    CONNECT\\ *) printf '%s' \"$transcript\"; printf 'ORA-01012: not logged on\\nSP2-0640: Not connected\\n';;\n    PROMPT\\ *\\|READY) exit 1;;\n  esac\ndone\n",
                shell_literal(rejected_transcript)
            ),
        )
        .unwrap();
        std::fs::set_permissions(&rejected, std::fs::Permissions::from_mode(0o700)).unwrap();
        let expired = root.join("sqlplus-expired.sh");
        let expired_transcript =
            include_str!("../tests/fixtures/oracle_sqlplus/expired-password.txt");
        std::fs::write(
            &expired,
            format!(
                "#!/bin/sh\n[ \"${{1:-}}\" != \"-V\" ] || {{ printf 'SQL*Plus: Release 21.0.0.0.0 - Production\\nVersion 21.3.0.0.0\\n'; exit 0; }}\ntranscript={}\nmarker=\nwhile IFS= read -r line; do\n  case \"$line\" in\n    PROMPT\\ __DBWARP_BP_*\\|LOGIN-BEGIN) marker=${{line#PROMPT }}; marker=${{marker%|LOGIN-BEGIN}}; printf '%s|LOGIN-BEGIN\\n' \"$marker\";;\n    PROMPT\\ __DBWARP_BP_*\\|CLIENT\\|*) printf '%s|CLIENT|2103000000\\n' \"$marker\";;\n    CONNECT\\ *) printf '%s' \"$transcript\";;\n    PROMPT\\ *\\|READY) exit 1;;\n  esac\ndone\n",
                shell_literal(expired_transcript)
            ),
        )
        .unwrap();
        std::fs::set_permissions(&expired, std::fs::Permissions::from_mode(0o700)).unwrap();
        let accepted = root.join("sqlplus-grace.sh");
        let grace_transcript = include_str!("../tests/fixtures/oracle_sqlplus/password-grace.txt");
        std::fs::write(
            &accepted,
            format!(
                r#"#!/bin/sh
[ "${{1:-}}" != "-V" ] || {{ printf 'SQL*Plus: Release 21.0.0.0.0 - Production\nVersion 21.3.0.0.0\n'; exit 0; }}
transcript={}
marker=
printf 'ORA-01012: not logged on\n'
printf 'SP2-0640: Not connected\n'
while IFS= read -r line; do
  case "$line" in
    PROMPT\ __DBWARP_BP_*\|LOGIN-BEGIN)
      marker=$(printf '%s' "$line" | cut -d' ' -f2 | cut -d'|' -f1)
      printf '%s|LOGIN-BEGIN\n' "$marker"
      ;;
    CONNECT\ *) printf '%s' "$transcript";;
    "SET ROWLIMIT OFF") rowlimit=1;;
    "SET CMDSEP OFF") cmdsep=1;;
    "SET SQLTERMINATOR \";\"") terminator=1;;
    "SET MARKUP HTML OFF") markup=1;;
    "SET MARKUP CSV OFF") markup_csv=1;;
    "SET AUTOTRACE OFF") autotrace=1;;
    "SET PAUSE OFF") pause=1;;
    PROMPT\ __DBWARP_BP_*\|CLIENT\|*)
      marker=$(printf '%s' "$line" | cut -d' ' -f2 | cut -d'|' -f1)
      printf '%s|CLIENT|2103000000\n' "$marker"
      printf 'loader advisory without an Oracle error\n' >&2
      ;;
    SELECT\ \'__DBWARP_BP_*\|AUTH\'\ FROM\ SYS.DUAL*)
      # Clear the first settings pass. READY is admitted only if the adapter
      # reasserts every protocol-critical setting after authentication.
      rowlimit=0; cmdsep=0; terminator=0; markup=0; markup_csv=0; autotrace=0; pause=0
      printf '%s|AUTH\n' "$marker";;
    PROMPT\ __DBWARP_BP_*\|READY)
      [ "$rowlimit$cmdsep$terminator$markup$markup_csv$autotrace$pause" = 1111111 ] || exit 2
      printf '%s|READY\n' "$marker";;
  esac
done
"#,
                shell_literal(grace_transcript)
            ),
        )
        .unwrap();
        std::fs::set_permissions(&accepted, std::fs::Permissions::from_mode(0o700)).unwrap();
        let broken = root.join("sqlplus-broken-client.sh");
        // Drain the complete login program before exiting. An immediate exit
        // races the parent's pipe writes and nondeterministically tests
        // ProcessWrite instead of the intended pre-READY classification.
        std::fs::write(
            &broken,
            "#!/bin/sh\n[ \"${1:-}\" != \"-V\" ] || { printf 'SQL*Plus: Release 21.0.0.0.0 - Production\\nVersion 21.3.0.0.0\\n'; exit 0; }\nmarker=\nwhile IFS= read -r line; do\n  case \"$line\" in\n    PROMPT\\ __DBWARP_BP_*\\|LOGIN-BEGIN) marker=${line#PROMPT }; marker=${marker%|LOGIN-BEGIN}; printf '%s|LOGIN-BEGIN\\n' \"$marker\";;\n    PROMPT\\ __DBWARP_BP_*\\|CLIENT\\|*) printf '%s|CLIENT|2103000000\\n' \"$marker\";;\n    PROMPT\\ *\\|READY) exit 127;;\n  esac\ndone\n",
        )
        .unwrap();
        std::fs::set_permissions(&broken, std::fs::Permissions::from_mode(0o700)).unwrap();
        let broken_profile = root.join("sqlplus-broken-profile.sh");
        std::fs::write(
            &broken_profile,
            "#!/bin/sh\n[ \"${1:-}\" != \"-V\" ] || { printf 'SQL*Plus: Release 21.0.0.0.0 - Production\\nVersion 21.3.0.0.0\\n'; exit 0; }\nprintf 'ORA-00942: table or view does not exist\\n'\nmarker=\nwhile IFS= read -r line; do\n  case \"$line\" in\n    PROMPT\\ __DBWARP_BP_*\\|LOGIN-BEGIN) marker=${line#PROMPT }; marker=${marker%|LOGIN-BEGIN}; printf '%s|LOGIN-BEGIN\\n' \"$marker\";;\n    PROMPT\\ __DBWARP_BP_*\\|CLIENT\\|*) printf '%s|CLIENT|2103000000\\n' \"$marker\";;\n    PROMPT\\ *\\|READY) exit 1;;\n  esac\ndone\n",
        )
        .unwrap();
        std::fs::set_permissions(&broken_profile, std::fs::Permissions::from_mode(0o700)).unwrap();

        let secret_variable = format!("DBWARP_ORACLE_SQLPLUS_LOGIN_{}", std::process::id());
        std::env::set_var(&secret_variable, "unit-test-only");
        let secret = Secret::from_env(&secret_variable).unwrap();
        std::env::remove_var(&secret_variable);
        let config = |executable| OracleSqlplusConfig {
            executable,
            username: "collector",
            connect_identifier: "//127.0.0.1:1521/APPPDB",
            password: &secret,
            isolated_network_config_dir: &network,
            startup_timeout: Duration::from_secs(2),
        };
        assert!(matches!(
            OracleSqlplusAdapter::start(config(&rejected)),
            Err(OracleSqlplusStartFailure::LoginRejected)
        ));
        assert!(matches!(
            OracleSqlplusAdapter::start(config(&expired)),
            Err(OracleSqlplusStartFailure::LoginRejected)
        ));
        assert!(matches!(
            OracleSqlplusAdapter::start(config(&broken)),
            Err(OracleSqlplusStartFailure::ClientExitedBeforeReady)
        ));
        assert!(matches!(
            OracleSqlplusAdapter::start(config(&broken_profile)),
            Err(OracleSqlplusStartFailure::ClientExitedBeforeReady)
        ));
        let adapter = OracleSqlplusAdapter::start(config(&accepted)).unwrap();
        assert_eq!(
            adapter.protocol_client_version().unwrap().components(),
            &[21, 3, 0, 0, 0]
        );
        drop(adapter);

        std::fs::remove_file(rejected).unwrap();
        std::fs::remove_file(expired).unwrap();
        std::fs::remove_file(accepted).unwrap();
        std::fs::remove_file(broken).unwrap();
        std::fs::remove_file(broken_profile).unwrap();
        std::fs::remove_dir(network).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn login_observation_has_one_fail_closed_classification_table() {
        assert_eq!(
            classify_login_observation(LoginEvidence {
                authenticated: true,
                oracle_error: None,
                login_rejection_observed: false,
                client_error: false,
            }),
            LoginObservation::Accepted
        );
        assert_eq!(
            classify_login_observation(LoginEvidence {
                authenticated: true,
                oracle_error: Some(-28002),
                login_rejection_observed: false,
                client_error: false,
            }),
            LoginObservation::Accepted
        );
        assert_eq!(
            classify_login_observation(LoginEvidence {
                authenticated: false,
                oracle_error: Some(-1012),
                login_rejection_observed: true,
                client_error: true,
            }),
            LoginObservation::Rejected
        );
        assert_eq!(
            classify_login_observation(LoginEvidence {
                authenticated: false,
                oracle_error: Some(-1045),
                login_rejection_observed: true,
                client_error: false,
            }),
            LoginObservation::Rejected
        );
        assert_eq!(
            classify_login_observation(LoginEvidence {
                authenticated: false,
                oracle_error: Some(-12541),
                login_rejection_observed: false,
                client_error: false,
            }),
            LoginObservation::ClientFailure
        );
        assert_eq!(
            classify_login_observation(LoginEvidence {
                authenticated: false,
                oracle_error: None,
                login_rejection_observed: false,
                client_error: true,
            }),
            LoginObservation::ClientFailure
        );
        assert_eq!(
            classify_login_observation(LoginEvidence {
                authenticated: false,
                oracle_error: None,
                login_rejection_observed: false,
                client_error: false,
            }),
            LoginObservation::Incomplete
        );
        assert!(contains_login_rejection_code(
            b"ORA-01017: rejected\nORA-01012: follow-on"
        ));
    }

    #[test]
    fn sqlplus_transcript_fixtures_cover_login_and_startup_shapes() {
        let clean = include_str!("../tests/fixtures/oracle_sqlplus/clean-login.txt");
        let wrong = include_str!("../tests/fixtures/oracle_sqlplus/wrong-password.txt");
        let grace = include_str!("../tests/fixtures/oracle_sqlplus/password-grace.txt");
        let expired = include_str!("../tests/fixtures/oracle_sqlplus/expired-password.txt");
        let site = include_str!("../tests/fixtures/oracle_sqlplus/site-settings-equivalent.txt");
        let sp2 = include_str!("../tests/fixtures/oracle_sqlplus/sp2-error.txt");
        let denied = include_str!("../tests/fixtures/oracle_sqlplus/query-permission-denied.txt");

        assert!(clean
            .lines()
            .all(|line| oracle_error_code(line.as_bytes()).is_none()));
        assert!(wrong
            .lines()
            .any(|line| oracle_error_code(line.as_bytes()) == Some(-1017)));
        assert!(grace
            .lines()
            .any(|line| oracle_error_code(line.as_bytes()) == Some(-28002)));
        assert!(expired
            .lines()
            .any(|line| oracle_error_code(line.as_bytes()) == Some(-28001)));
        assert!(site.contains("rowlimit reached") && site.contains("Elapsed:"));
        assert!(sp2.lines().any(|line| line.starts_with("SP2-0735:")));
        assert!(denied
            .lines()
            .any(|line| oracle_error_code(line.as_bytes()) == Some(-942)));
    }

    #[test]
    fn query_program_uses_bind_hex_and_never_interpolates_owner() {
        let program = render_query_program(
            "__MARKER__",
            &execution(
                "SELECT owner, table_name FROM SYS.DBA_TABLES WHERE owner = :owner",
                Some("MiXeD Owner"),
            ),
        )
        .unwrap();
        assert!(!program.contains("MiXeD Owner"));
        assert!(program.contains(&hex::encode("MiXeD Owner".as_bytes())));
        assert!(program.contains("VARIABLE owner VARCHAR2(128)"));
        assert!(program.contains("THEN 'L'"));
        assert!(program.contains("LENGTHB(TO_CHAR"));
        assert!(program.contains("COLUMN DBWARP_BP_FRAME FORMAT A32000"));
        assert!(program.contains("|| '|Z' AS DBWARP_BP_FRAME"));
        assert!(program.contains("AS DBWARP_BP_FRAME"));
        assert!(program.contains("COLUMN DBWARP_BP_FRAME CLEAR"));
        assert!(program.contains("|BEGIN|"));
        assert!(program.contains("|END|&DBWARP_BP_ROW_COUNT"));
        assert!(
            program.contains("TO_CHAR(ROWNUM, 'FM99999999999999999990') AS DBWARP_BP_ROW_COUNT")
        );
        assert!(render_query_program(
            "__MARKER__",
            &execution("SELECT owner, 42 FROM SYS.DBA_TABLES", Some("BAD\nOWNER")),
        )
        .is_none());
    }

    #[test]
    fn query_program_wraps_only_semicolon_free_registry_sql() {
        let program = render_query_program(
            "__MARKER__",
            &execution("SELECT 'x' AS a, 'y' AS b FROM SYS.DUAL", None),
        )
        .unwrap();
        assert!(program.contains("FROM (SELECT 'x' AS a, 'y' AS b FROM SYS.DUAL) q"));
        assert!(render_query_program(
            "__MARKER__",
            &execution("SELECT a, b FROM SYS.DUAL; DROP TABLE x", None),
        )
        .is_none());
        assert!(render_query_program(
            "__MARKER__",
            &execution("SELECT '&unexpected' AS a, 'y' AS b FROM SYS.DUAL", None),
        )
        .is_none());
    }

    #[test]
    fn wide_query_program_keeps_every_sqlplus_input_line_below_2499_bytes() {
        let projected = vec!["column"; 256];
        let mut wide = execution("SELECT 1 AS column FROM SYS.DUAL", None);
        wide.expected_columns = projected.len();
        wide.projected_columns = &projected;
        let program = render_query_program("__MARKER__", &wide).unwrap();

        assert!(program.lines().all(|line| line.len() < 2_499));
    }

    #[test]
    fn error_classification_never_needs_raw_oracle_text() {
        assert_eq!(
            oracle_error_code(b"ORA-01031: insufficient privileges"),
            Some(-1031)
        );
        assert_eq!(
            oracle_error_code(b"ORA-28002: password grace warning\nORA-01031: setup was denied"),
            Some(-1031)
        );
        assert_eq!(
            classify_oracle_error(-942),
            OracleQueryFailure::ObjectAbsent
        );
        assert_eq!(
            classify_oracle_error(-3113),
            OracleQueryFailure::SessionLost
        );
        assert_eq!(
            classify_oracle_error(-1722),
            OracleQueryFailure::DatabaseError
        );
    }

    #[test]
    fn sqlplus_release_is_numeric_and_fixed_width() {
        assert_eq!(
            parse_sqlplus_release("2103000000").unwrap().components(),
            &[21, 3, 0, 0, 0]
        );
        assert!(parse_sqlplus_release("SQL*Plus 21.3").is_none());
        assert!(parse_sqlplus_release("210300000").is_none());
        let v11 = OracleVersion::from_components(&[11, 2]).unwrap();
        assert!(!sqlplus_supports_rowlimit(v11));
        assert!(!sqlplus_supports_markup_csv(v11));
        assert_eq!(
            OracleClientVersionAttestation::Attested(v11).limitation_token(),
            Some("oracle-client-version-below-tested-floor")
        );
    }

    #[test]
    fn executable_version_banner_is_narrow_without_being_an_admission_gate() {
        assert_eq!(
            parse_sqlplus_version_banner(
                b"unrelated launcher text\nSQL*Plus: Release 21.0.0.0.0 - Production\nVersion 21.3.0.0.0\n"
            )
            .unwrap()
            .components(),
            &[21, 3, 0, 0, 0]
        );
        assert_eq!(
            parse_sqlplus_version_banner(b"SQL*Plus: Release 11.2.0.4.0 - Production\n")
                .unwrap()
                .components(),
            &[11, 2, 0, 4]
        );
        assert!(
            parse_sqlplus_version_banner(b"SQL*Plus: Release 21.100.0.0.0 - Production\n")
                .is_none()
        );
        assert!(parse_sqlplus_version_banner(b"Release 21.3.0.0.0\n").is_none());
        assert!(parse_sqlplus_version_banner(b"SQL*Plus 21.3.0.0.0\n").is_none());
    }

    #[test]
    fn recorded_client_banners_and_sqlplus_release_share_the_protocol_version() {
        let fixtures = [
            (
                include_str!("../tests/fixtures/oracle_sqlplus/client-version-12c.txt"),
                &[12, 1, 0, 2][..],
                &[12, 1, 0, 2, 0][..],
            ),
            (
                include_str!("../tests/fixtures/oracle_sqlplus/client-version-19c.txt"),
                &[19, 3, 0, 0, 0][..],
                &[19, 3, 0, 0, 0][..],
            ),
            (
                include_str!("../tests/fixtures/oracle_sqlplus/client-version-21c.txt"),
                &[21, 3, 0, 0, 0][..],
                &[21, 3, 0, 0, 0][..],
            ),
            (
                include_str!("../tests/fixtures/oracle_sqlplus/client-version-23ai.txt"),
                &[23, 26, 3, 0, 0][..],
                &[23, 26, 3, 0, 0][..],
            ),
        ];
        for (fixture, expected_banner, expected_session) in fixtures {
            let executable = parse_sqlplus_version_banner(fixture.as_bytes())
                .expect("recorded SQLPlus -V banner");
            let substitution = fixture
                .lines()
                .find_map(|line| line.strip_prefix("_SQLPLUS_RELEASE="))
                .and_then(parse_sqlplus_release)
                .expect("recorded &_SQLPLUS_RELEASE value");
            assert_eq!(executable.components(), expected_banner);
            assert_eq!(substitution.components(), expected_session);
            assert_eq!(
                sqlplus_client_versions_compatible(Some(substitution), Some(executable)),
                OracleClientVersionAttestation::Attested(substitution)
            );
        }
    }

    #[test]
    fn sqlplus_version_attestation_classifies_drift_and_unreadable_inputs() {
        let patch_variant = OracleVersion::from_components(&[21, 3, 0, 0, 7]).unwrap();
        let same_protocol = OracleVersion::from_components(&[21, 3, 0, 0, 0]).unwrap();
        let different_protocol = OracleVersion::from_components(&[21, 4, 0, 0, 0]).unwrap();

        assert_eq!(
            sqlplus_client_versions_compatible(Some(patch_variant), Some(same_protocol)),
            OracleClientVersionAttestation::Mismatched
        );
        assert_eq!(
            sqlplus_client_versions_compatible(Some(different_protocol), Some(same_protocol)),
            OracleClientVersionAttestation::Mismatched
        );
        assert_eq!(
            sqlplus_client_versions_compatible(Some(same_protocol), None),
            OracleClientVersionAttestation::Unreadable
        );
        let major_only = OracleVersion::from_components(&[21]).unwrap();
        assert_eq!(
            sqlplus_client_versions_compatible(Some(same_protocol), Some(major_only)),
            OracleClientVersionAttestation::Unreadable
        );
        let base_19 =
            parse_sqlplus_version_banner(b"SQL*Plus: Release 19.0.0.0.0 - Production\n").unwrap();
        let update_19 = OracleVersion::from_components(&[19, 3, 0, 0, 0]).unwrap();
        assert_eq!(base_19.components(), &[19]);
        assert_eq!(
            sqlplus_client_versions_compatible(Some(update_19), Some(base_19)),
            OracleClientVersionAttestation::Unreadable
        );
    }

    #[cfg(unix)]
    #[test]
    fn version_mismatch_weakens_provenance_but_does_not_block_login() {
        use std::os::unix::fs::PermissionsExt;

        let _environment_guard = PROCESS_ENVIRONMENT_LOCK.lock().unwrap();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tmp/tests")
            .join(format!(
                "oracle-sqlplus-version-mismatch-{}",
                std::process::id()
            ));
        let network = root.join("network-empty");
        std::fs::create_dir_all(&network).unwrap();
        std::fs::set_permissions(&network, std::fs::Permissions::from_mode(0o700)).unwrap();
        let client = root.join("sqlplus-version-mismatch.sh");
        std::fs::write(
            &client,
            r#"#!/bin/sh
[ "${1:-}" != "-V" ] || { printf 'SQL*Plus: Release 21.0.0.0.0 - Production\nVersion 21.3.0.0.0\n'; exit 0; }
marker=
while IFS= read -r line; do
  case "$line" in
    PROMPT\ __DBWARP_BP_*\|LOGIN-BEGIN)
      marker=${line#PROMPT }; marker=${marker%|LOGIN-BEGIN}
      printf '%s|LOGIN-BEGIN\n' "$marker";;
    PROMPT\ __DBWARP_BP_*\|CLIENT\|*) printf '%s|CLIENT|1900000000\n' "$marker";;
    SELECT\ \'__DBWARP_BP_*\|AUTH\'\ FROM\ SYS.DUAL*) printf '%s|AUTH\n' "$marker";;
    PROMPT\ __DBWARP_BP_*\|READY) printf '%s|READY\n' "$marker";;
  esac
done
"#,
        )
        .unwrap();
        std::fs::set_permissions(&client, std::fs::Permissions::from_mode(0o700)).unwrap();

        let secret_variable = format!("DBWARP_ORACLE_SQLPLUS_VERSION_{}", std::process::id());
        std::env::set_var(&secret_variable, "unit-test-only");
        let secret = Secret::from_env(&secret_variable).unwrap();
        std::env::remove_var(&secret_variable);
        let adapter = OracleSqlplusAdapter::start(OracleSqlplusConfig {
            executable: &client,
            username: "COLLECTOR",
            connect_identifier: "//127.0.0.1:1521/APPPDB",
            password: &secret,
            isolated_network_config_dir: &network,
            startup_timeout: Duration::from_secs(2),
        })
        .expect("version drift is provenance, not an admission failure");
        assert_eq!(
            adapter.client_version_attestation(),
            OracleClientVersionAttestation::Mismatched
        );
        assert_eq!(
            adapter.protocol_client_version().unwrap().components(),
            &[19, 0, 0, 0, 0]
        );
        drop(adapter);

        std::fs::remove_file(client).unwrap();
        std::fs::remove_dir(network).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn unverified_and_pre_12_clients_attempt_protocol_without_a_version_refusal() {
        use std::os::unix::fs::PermissionsExt;

        let _environment_guard = PROCESS_ENVIRONMENT_LOCK.lock().unwrap();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tmp/tests")
            .join(format!(
                "oracle-sqlplus-version-unreadable-{}",
                std::process::id()
            ));
        let network = root.join("network-empty");
        std::fs::create_dir_all(&network).unwrap();
        std::fs::set_permissions(&network, std::fs::Permissions::from_mode(0o700)).unwrap();

        let v11 = OracleVersion::from_components(&[11, 2, 0, 4, 0]).unwrap();
        let cases = [
            (
                "nine-digit-session",
                "SQL*Plus: Release 21.0.0.0.0 - Production\\nVersion 21.3.0.0.0\\n",
                "210300000",
                Some(&[21, 3, 0, 0, 0][..]),
                OracleClientVersionAttestation::Unreadable,
                false,
            ),
            (
                "oversized-banner-component",
                "SQL*Plus: Release 21.100.0.0.0 - Production\\n",
                "2103000000",
                Some(&[21, 3, 0, 0, 0][..]),
                OracleClientVersionAttestation::Unreadable,
                false,
            ),
            (
                "both-unreadable",
                "SQL*Plus: Release 21.100.0.0.0 - Production\\n",
                "210300000",
                None,
                OracleClientVersionAttestation::Unreadable,
                true,
            ),
            (
                "both-unreadable-pre-12-2",
                "SQL*Plus: Release 21.100.0.0.0 - Production\\n",
                "210300000",
                None,
                OracleClientVersionAttestation::Unreadable,
                false,
            ),
            (
                "pre-12-client",
                "SQL*Plus: Release 11.2.0.4.0 - Production\\n",
                "1102000400",
                Some(&[11, 2, 0, 4, 0][..]),
                OracleClientVersionAttestation::Attested(v11),
                false,
            ),
        ];
        let secret_variable = format!("DBWARP_ORACLE_SQLPLUS_UNREADABLE_{}", std::process::id());
        std::env::set_var(&secret_variable, "unit-test-only");
        let secret = Secret::from_env(&secret_variable).unwrap();
        std::env::remove_var(&secret_variable);

        for (name, banner, release, expected_protocol, expected_attestation, prove_rows) in cases {
            let client = root.join(format!("sqlplus-{name}.sh"));
            let settings_diagnostic = if name == "both-unreadable-pre-12-2" {
                "printf 'SP2-0158: unknown SET option \\\"OPTION\\\"\\n'"
            } else {
                ":"
            };
            std::fs::write(
                &client,
                format!(
                    r#"#!/bin/sh
[ "${{1:-}}" != "-V" ] || {{ printf '{banner}'; exit 0; }}
marker=
rowlimit=200
while IFS= read -r line; do
  case "$line" in
    PROMPT\ __DBWARP_BP_*\|LOGIN-BEGIN)
      marker=${{line#PROMPT }}; marker=${{marker%|LOGIN-BEGIN}}
      printf '%s|LOGIN-BEGIN\n' "$marker";;
    PROMPT\ __DBWARP_BP_*\|CLIENT\|*) printf '%s|CLIENT|{release}\n' "$marker";;
    SELECT\ \'__DBWARP_BP_*\|AUTH\'\ FROM\ SYS.DUAL*) printf '%s|AUTH\n' "$marker";;
    "SET ROWLIMIT OFF") rowlimit=0; {settings_diagnostic};;
    "SET MARKUP CSV OFF") {settings_diagnostic};;
    PROMPT\ __DBWARP_BP_*\|SETTINGS-BEGIN) printf '%s|SETTINGS-BEGIN\n' "$marker";;
    PROMPT\ __DBWARP_BP_*\|SETTINGS-END) printf '%s|SETTINGS-END\n' "$marker";;
    PROMPT\ __DBWARP_BP_*\|READY) printf '%s|READY\n' "$marker";;
    PROMPT\ __DBWARP_BP_*\|BEGIN\|2)
      printf '%s|BEGIN|2\n' "$marker"
      printf '%s|R|1|T415050|T31|Z\n' "$marker"
      if [ "$rowlimit" -eq 0 ]; then
        printf '%s|R|2|T415050|T32|Z\n' "$marker"
        query_rows=2
      else
        query_rows=1
      fi;;
    PROMPT\ __DBWARP_BP_*\|END\|*) printf '%s|END|%s\n' "$marker" "${{query_rows:-0}}";;
  esac
done
"#
                ),
            )
            .unwrap();
            std::fs::set_permissions(&client, std::fs::Permissions::from_mode(0o700)).unwrap();
            let mut adapter = OracleSqlplusAdapter::start(OracleSqlplusConfig {
                executable: &client,
                username: "COLLECTOR",
                connect_identifier: "//127.0.0.1:1521/APPPDB",
                password: &secret,
                isolated_network_config_dir: &network,
                startup_timeout: Duration::from_secs(2),
            })
            .expect("unreadable version provenance must not block login");
            assert_eq!(adapter.client_version_attestation(), expected_attestation);
            assert_eq!(
                adapter
                    .protocol_client_version()
                    .map(|version| version.components().to_vec()),
                expected_protocol.map(<[u16]>::to_vec)
            );
            if prove_rows {
                let mut received = Collected(Vec::new());
                assert_eq!(
                    adapter.execute(
                        &execution("SELECT owner, 1 AS value FROM SYS.DBA_TABLES", None),
                        &mut received,
                    ),
                    OracleExecuteResult::Complete
                );
                assert_eq!(
                    received.0.len(),
                    2,
                    "unknown version inherited a site ROWLIMIT"
                );
            }
            drop(adapter);
            std::fs::remove_file(client).unwrap();
        }

        std::fs::remove_dir(network).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn multitenant_probe_refuses_root_and_accepts_a_pdb() {
        let v21 = OracleVersion::from_components(&[21, 3]).unwrap();
        assert_eq!(
            require_supported_container(v21, 1),
            Err(OracleSqlplusStartFailure::RootContainerUnsupported)
        );
        assert_eq!(require_supported_container(v21, 3), Ok(()));
        let v11 = OracleVersion::from_components(&[11, 2]).unwrap();
        assert_eq!(require_supported_container(v11, 1), Ok(()));
    }

    #[cfg(unix)]
    #[test]
    fn network_sandbox_requires_an_empty_private_leaf_and_parent_chain() {
        use std::os::unix::fs::PermissionsExt;

        let parent = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tmp/tests")
            .join(format!("oracle-sqlplus-network-{}", std::process::id()));
        let leaf = parent.join("empty");
        std::fs::create_dir_all(&leaf).unwrap();
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o777)).unwrap();
        std::fs::set_permissions(&leaf, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert!(!network_sandbox_is_private_and_empty(&leaf));
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert!(network_sandbox_is_private_and_empty(&leaf));
        std::fs::write(leaf.join("sqlnet.ora"), "NAMES.DIRECTORY_PATH=()").unwrap();
        assert!(!network_sandbox_is_private_and_empty(&leaf));
        std::fs::remove_file(leaf.join("sqlnet.ora")).unwrap();
        std::fs::remove_dir(leaf).unwrap();
        std::fs::remove_dir(parent).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn persistent_command_process_delivers_null_aware_hex_frames() {
        use std::os::unix::fs::PermissionsExt;

        let _environment_guard = PROCESS_ENVIRONMENT_LOCK.lock().unwrap();

        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tmp/tests")
            .join(format!("oracle-sqlplus-adapter-{}", std::process::id()));
        let network = root.join("network-empty");
        std::fs::create_dir_all(&network).unwrap();
        std::fs::set_permissions(&network, std::fs::Permissions::from_mode(0o700)).unwrap();
        let client = root.join("sqlplus-fixture.sh");
        std::fs::write(
            &client,
            "#!/bin/sh\n\
             [ \"${1:-}\" != \"-V\" ] || { printf 'SQL*Plus: Release 21.0.0.0.0 - Production\\nVersion 21.3.0.0.0\\n'; exit 0; }\n\
             [ -z \"${DBWARP_ORACLE_INHERITANCE_CANARY:-}\" ] || exit 91\n\
             marker=\n\
             query=0\n\
             while IFS= read -r line; do\n\
               case \"$line\" in\n\
                 PROMPT\\ __DBWARP_BP_*\\|READY)\n\
                   marker=${line#PROMPT }\n\
                   marker=${marker%|READY}\n\
                   printf '%s|READY\\n' \"$marker\"\n\
                   ;;\n\
                 PROMPT\\ __DBWARP_BP_*\\|CLIENT\\|*)\n\
                   client_marker=${line#PROMPT }\n\
                   client_marker=${client_marker%|CLIENT|*}\n\
                   marker=$client_marker\n\
                   printf '%s|CLIENT|2103000000\\n' \"$client_marker\"\n\
                   ;;\n\
                 SELECT\\ \\\'__DBWARP_BP_*\\|AUTH\\\'\\ FROM\\ SYS.DUAL*)\n\
                   printf '%s|AUTH\\n' \"$marker\"\n\
                   ;;\n\
                 PROMPT\\ __DBWARP_BP_*\\|BEGIN\\|2)\n\
                   query=$((query + 1))\n\
                   printf '%s|BEGIN|2\\n' \"$marker\"\n\
                   if [ \"$query\" -eq 1 ]; then\n\
                     printf '%s|R|1|T4f574e4552|T3432|Z\\n' \"$marker\"\n\
                   else\n\
                     printf '%s|R|1|T4f574e4552|T3432\\n' \"$marker\"\n\
                   fi\n\
                   printf '%s|END|      1\\n' \"$marker\"\n\
                   ;;\n\
               esac\n\
             done\n",
        )
        .unwrap();
        std::fs::set_permissions(&client, std::fs::Permissions::from_mode(0o700)).unwrap();

        let secret_variable = format!("DBWARP_ORACLE_SQLPLUS_TEST_{}", std::process::id());
        std::env::set_var(&secret_variable, "unit-test-only");
        let secret = Secret::from_env(&secret_variable).unwrap();
        std::env::remove_var(&secret_variable);
        std::env::set_var("DBWARP_ORACLE_INHERITANCE_CANARY", "must-not-reach-client");
        let mut adapter = OracleSqlplusAdapter::start(OracleSqlplusConfig {
            executable: &client,
            username: "COLLECTOR",
            connect_identifier: "//127.0.0.1:1521/APPPDB",
            password: &secret,
            isolated_network_config_dir: &network,
            startup_timeout: Duration::from_secs(2),
        })
        .unwrap();
        std::env::remove_var("DBWARP_ORACLE_INHERITANCE_CANARY");
        assert_ne!(adapter.provider_artifact_sha256(), [0; 32]);
        assert_eq!(
            adapter.protocol_client_version().unwrap().components(),
            &[21, 3, 0, 0, 0]
        );
        let mut rows = Collected(Vec::new());
        assert_eq!(
            adapter.execute(
                &execution("SELECT owner, 42 FROM SYS.DBA_TABLES", None),
                &mut rows,
            ),
            OracleExecuteResult::Complete
        );
        assert_eq!(
            rows.0,
            vec![OracleRow {
                values: vec![
                    OracleValue::Text("OWNER".to_string()),
                    OracleValue::Text("42".to_string()),
                ],
            }]
        );
        // The fixture's second query deliberately omits the terminal frame
        // sentinel, modelling SQL*Plus truncation that still leaves valid hex
        // and the expected delimiter count. It must fail before delivering a
        // shortened catalog value.
        let mut truncated_rows = Collected(Vec::new());
        assert_eq!(
            adapter.execute(
                &execution("SELECT owner, 42 FROM SYS.DBA_TABLES", None),
                &mut truncated_rows,
            ),
            OracleExecuteResult::Failed(OracleQueryFailure::Malformed)
        );
        assert!(truncated_rows.0.is_empty());
        drop(adapter);
        std::fs::remove_file(client).unwrap();
        std::fs::remove_dir(network).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn optional_statement_error_keeps_the_sqlplus_session_usable() {
        use std::os::unix::fs::PermissionsExt;

        let _environment_guard = PROCESS_ENVIRONMENT_LOCK.lock().unwrap();

        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tmp/tests")
            .join(format!("oracle-sqlplus-degrade-{}", std::process::id()));
        let network = root.join("network-empty");
        std::fs::create_dir_all(&network).unwrap();
        std::fs::set_permissions(&network, std::fs::Permissions::from_mode(0o700)).unwrap();
        let client = root.join("sqlplus-degrade-fixture.sh");
        std::fs::write(
            &client,
            r#"#!/bin/sh
[ "${1:-}" != "-V" ] || { printf 'SQL*Plus: Release 21.0.0.0.0 - Production\nVersion 21.3.0.0.0\n'; exit 0; }
[ "${ORACLE_HOME:-}" = "/oracle/test-home" ] || exit 3
[ "${LD_LIBRARY_PATH:-}" = "/oracle/test-libs" ] || exit 4
marker=
continue_on_error=0
query=0
rowlimit=0
cmdsep=0
terminator=0
markup=0
markup_csv=0
autotrace=0
pause=0
while IFS= read -r line; do
  case "$line" in
    "WHENEVER SQLERROR EXIT"*) continue_on_error=0;;
    "WHENEVER SQLERROR CONTINUE NONE") continue_on_error=1;;
    "SET ROWLIMIT OFF") rowlimit=1;;
    "SET CMDSEP OFF") cmdsep=1;;
    "SET SQLTERMINATOR \";\"") terminator=1;;
    "SET MARKUP HTML OFF") markup=1;;
    "SET MARKUP CSV OFF") markup_csv=1;;
    "SET AUTOTRACE OFF") autotrace=1;;
    "SET PAUSE OFF") pause=1;;
    PROMPT\ __DBWARP_BP_*\|CLIENT\|*)
      marker=${line#PROMPT }; marker=${marker%|CLIENT|*}
      printf '%s|CLIENT|2103000000\n' "$marker";;
    CONNECT\ *) printf 'ORA-00942: site profile probe is not visible\n';;
    SELECT\ \'__DBWARP_BP_*\|AUTH\'\ FROM\ SYS.DUAL*) printf '%s|AUTH\n' "$marker";;
    PROMPT\ __DBWARP_BP_*\|READY)
      [ "$rowlimit$cmdsep$terminator$markup$markup_csv$autotrace$pause" = 1111111 ] || exit 2
      printf '%s|READY\n' "$marker";;
    PROMPT\ __DBWARP_BP_*\|BEGIN\|2)
      query=$((query + 1)); printf '%s|BEGIN|2\n' "$marker"
      if [ "$query" -eq 1 ]; then
        printf 'ORA-01031: denied optional catalog\n'
        [ "$continue_on_error" -eq 1 ] || exit 1
      elif [ "$query" -eq 2 ]; then
        printf '%s|R|1|L|T3432|Z\n' "$marker"
      else
        printf '%s|R|1|T4f574e4552|T3432|Z\n' "$marker"
      fi;;
    PROMPT\ __DBWARP_BP_*\|END\|*)
      if [ "$query" -ge 2 ]; then
        printf '%s|END|1\n' "$marker"
      else
        printf '%s|END|0\n' "$marker"
      fi;;
  esac
done
"#,
        )
        .unwrap();
        std::fs::set_permissions(&client, std::fs::Permissions::from_mode(0o700)).unwrap();

        let secret_variable = format!("DBWARP_ORACLE_SQLPLUS_DEGRADE_{}", std::process::id());
        std::env::set_var(&secret_variable, "unit-test-only");
        let secret = Secret::from_env(&secret_variable).unwrap();
        std::env::remove_var(&secret_variable);
        let old_oracle_home = std::env::var_os("ORACLE_HOME");
        let old_ld_library_path = std::env::var_os("LD_LIBRARY_PATH");
        std::env::set_var("ORACLE_HOME", "/oracle/test-home");
        std::env::set_var("LD_LIBRARY_PATH", "/oracle/test-libs");
        let mut adapter = OracleSqlplusAdapter::start(OracleSqlplusConfig {
            executable: &client,
            username: "COLLECTOR",
            connect_identifier: "//127.0.0.1:1521/APPPDB",
            password: &secret,
            isolated_network_config_dir: &network,
            startup_timeout: Duration::from_secs(2),
        })
        .unwrap();
        match old_oracle_home {
            Some(value) => std::env::set_var("ORACLE_HOME", value),
            None => std::env::remove_var("ORACLE_HOME"),
        }
        match old_ld_library_path {
            Some(value) => std::env::set_var("LD_LIBRARY_PATH", value),
            None => std::env::remove_var("LD_LIBRARY_PATH"),
        }
        let mut rows = Collected(Vec::new());
        assert_eq!(
            adapter.execute(
                &execution("SELECT owner, 42 FROM SYS.DBA_IND_EXPRESSIONS", None),
                &mut rows,
            ),
            OracleExecuteResult::Failed(OracleQueryFailure::PermissionDenied)
        );
        assert!(!adapter.terminated);
        assert_eq!(
            adapter.execute(
                &execution("SELECT owner, 42 FROM SYS.DBA_TABLES", None),
                &mut rows,
            ),
            OracleExecuteResult::Failed(OracleQueryFailure::TransientValueLimitExceeded)
        );
        assert!(!adapter.terminated);
        assert_eq!(
            adapter.execute(
                &execution("SELECT owner, 42 FROM SYS.DBA_TABLES", None),
                &mut rows,
            ),
            OracleExecuteResult::Complete
        );
        assert!(!adapter.terminated);
        drop(adapter);
        std::fs::remove_file(client).unwrap();
        std::fs::remove_dir(network).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn live_probe_receipt_admits_the_same_persistent_session() {
        use std::os::unix::fs::PermissionsExt;

        let _environment_guard = PROCESS_ENVIRONMENT_LOCK.lock().unwrap();

        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tmp/tests")
            .join(format!("oracle-sqlplus-probe-{}", std::process::id()));
        let network = root.join("network-empty");
        std::fs::create_dir_all(&network).unwrap();
        std::fs::set_permissions(&network, std::fs::Permissions::from_mode(0o700)).unwrap();
        let client = root.join("sqlplus-probe-fixture.sh");
        std::fs::write(
            &client,
            r#"#!/bin/sh
[ "${1:-}" != "-V" ] || { printf 'SQL*Plus: Release 21.0.0.0.0 - Production\nVersion 21.3.0.0.0\n'; exit 0; }
marker=
while IFS= read -r line; do
  case "$line" in
    PROMPT\ __DBWARP_BP_*\|READY)
      marker=$(printf '%s' "$line" | cut -d' ' -f2 | cut -d'|' -f1)
      printf '%s|READY\n' "$marker"
      ;;
	    PROMPT\ __DBWARP_BP_*\|CLIENT\|*)
	      client_marker=$(printf '%s' "$line" | cut -d' ' -f2 | cut -d'|' -f1)
	      marker=$client_marker
	      printf '%s|CLIENT|2103000000\n' "$client_marker"
	      ;;
	    SELECT\ \'__DBWARP_BP_*\|AUTH\'\ FROM\ SYS.DUAL*)
	      printf '%s|AUTH\n' "$marker"
	      ;;
    PROMPT\ __DBWARP_BP_*\|BEGIN\|4)
      printf '%s|BEGIN|4\n' "$marker"
      printf '%s|R|1|T434f4c4c4543544f52|T544553544442|T33|T32312e332e302e302e30|Z\n' "$marker"
      printf '%s|END|1\n' "$marker"
      ;;
  esac
done
"#,
        )
        .unwrap();
        std::fs::set_permissions(&client, std::fs::Permissions::from_mode(0o700)).unwrap();

        let secret_variable = format!("DBWARP_ORACLE_SQLPLUS_PROBE_{}", std::process::id());
        std::env::set_var(&secret_variable, "unit-test-only");
        let secret = Secret::from_env(&secret_variable).unwrap();
        std::env::remove_var(&secret_variable);
        let limits = OracleProviderLimits {
            catalog_rows: 100,
            catalog_bytes: 1_000_000,
            result_nesting_depth: 2,
            elapsed_ms: 10_000,
            transient_value_bytes: MAX_SCALAR_UTF8_BYTES as u64,
            lob_bytes: 1,
        };
        let probed = start_and_probe_sqlplus(
            OracleSqlplusConfig {
                executable: &client,
                username: "COLLECTOR",
                connect_identifier: "//127.0.0.1:1521/APPPDB",
                password: &secret,
                isolated_network_config_dir: &network,
                startup_timeout: Duration::from_secs(2),
            },
            limits,
        )
        .unwrap();
        assert_eq!(probed.server_version().components(), &[21, 3, 0, 0, 0]);
        assert_eq!(probed.effective_principal(), "COLLECTOR");
        assert_eq!(probed.database_identity(), "TESTDB");
        let request = crate::oracle_provider::OracleCaptureRequest {
            contract_version: ORACLE_PROVIDER_CONTRACT_VERSION,
            provider: OracleProviderKind::Sqlplus,
            tier: OracleCaptureTier::Basic,
            artifact_detail: ArtifactDetail::Summary,
            endpoint_mode: OracleEndpointMode::DirectService,
            transport: OracleTransport::Tcp,
            authentication: OracleAuthentication::Password,
            trust_source: OracleTrustSource::NotApplicable,
            authorized_local_inputs: BTreeSet::from([OracleLocalInput::ClientExecutable]),
            require_confirmed_server_cancel: false,
        };
        let negotiated = crate::oracle_provider::negotiate_oracle_capabilities(
            &request,
            probed.declaration(),
            probed.evidence(),
        )
        .unwrap();
        assert_eq!(negotiated.limits(), limits);
        assert_eq!(
            negotiated
                .proof()
                .attested_client_version()
                .unwrap()
                .components(),
            &[21, 3, 0, 0, 0]
        );
        assert_ne!(
            negotiated.proof().session_binding_sha256().unwrap(),
            [0; 32]
        );

        drop(probed);
        std::fs::remove_file(client).unwrap();
        std::fs::remove_dir(network).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn minimum_floor_admission_falls_back_when_v_instance_is_denied() {
        use std::os::unix::fs::PermissionsExt;

        let _environment_guard = PROCESS_ENVIRONMENT_LOCK.lock().unwrap();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tmp/tests")
            .join(format!("oracle-sqlplus-floor-probe-{}", std::process::id()));
        let network = root.join("network-empty");
        std::fs::create_dir_all(&network).unwrap();
        std::fs::set_permissions(&network, std::fs::Permissions::from_mode(0o700)).unwrap();
        let client = root.join("sqlplus-floor-probe.sh");
        std::fs::write(
            &client,
            r#"#!/bin/sh
[ "${1:-}" != "-V" ] || { printf 'SQL*Plus: Release 21.0.0.0.0 - Production\nVersion 21.3.0.0.0\n'; exit 0; }
marker=
query=
primary_seen=0
while IFS= read -r line; do
  case "$line" in
    PROMPT\ __DBWARP_BP_*\|READY)
      marker=$(printf '%s' "$line" | cut -d' ' -f2 | cut -d'|' -f1)
      printf '%s|READY\n' "$marker";;
    PROMPT\ __DBWARP_BP_*\|CLIENT\|*)
      marker=$(printf '%s' "$line" | cut -d' ' -f2 | cut -d'|' -f1)
      printf '%s|CLIENT|2103000000\n' "$marker";;
    SELECT\ \'__DBWARP_BP_*\|AUTH\'\ FROM\ SYS.DUAL*)
      printf '%s|AUTH\n' "$marker";;
    PROMPT\ __DBWARP_BP_*\|BEGIN\|4)
      query=; printf '%s|BEGIN|4\n' "$marker";;
    *SYS.V_\$INSTANCE*)
      query=primary; primary_seen=1;;
    *SYS.PRODUCT_COMPONENT_VERSION*)
      [ "$primary_seen" -eq 1 ] || exit 92
      query=fallback;;
    PROMPT\ __DBWARP_BP_*\|END\|*)
      if [ "$query" = primary ]; then
        printf 'ORA-01031: insufficient privileges\n'
        printf '%s|END|0\n' "$marker"
      elif [ "$query" = fallback ]; then
        printf '%s|R|1|T434f4c4c4543544f52|T544553544442|T33|T32312e332e302e302e30|Z\n' "$marker"
        printf '%s|END|1\n' "$marker"
      else
        exit 93
      fi
      query=;;
  esac
done
"#,
        )
        .unwrap();
        std::fs::set_permissions(&client, std::fs::Permissions::from_mode(0o700)).unwrap();

        let secret_variable = format!("DBWARP_ORACLE_SQLPLUS_FLOOR_{}", std::process::id());
        std::env::set_var(&secret_variable, "unit-test-only");
        let secret = Secret::from_env(&secret_variable).unwrap();
        std::env::remove_var(&secret_variable);
        let probed = start_and_probe_sqlplus(
            OracleSqlplusConfig {
                executable: &client,
                username: "COLLECTOR",
                connect_identifier: "//127.0.0.1:1521/APPPDB",
                password: &secret,
                isolated_network_config_dir: &network,
                startup_timeout: Duration::from_secs(2),
            },
            OracleProviderLimits {
                catalog_rows: 100,
                catalog_bytes: 1_000_000,
                result_nesting_depth: 2,
                elapsed_ms: 10_000,
                transient_value_bytes: MAX_SCALAR_UTF8_BYTES as u64,
                lob_bytes: 1,
            },
        )
        .expect("the version fallback must follow a denied V_$INSTANCE probe");
        assert_eq!(probed.server_version().components(), &[21, 3, 0, 0, 0]);
        assert_eq!(probed.effective_principal(), "COLLECTOR");
        assert_eq!(probed.database_identity(), "TESTDB");

        drop(probed);
        std::fs::remove_file(client).unwrap();
        std::fs::remove_dir(network).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn oracle_error_text_is_reduced_to_a_closed_failure_class() {
        use std::os::unix::fs::PermissionsExt;

        let _environment_guard = PROCESS_ENVIRONMENT_LOCK.lock().unwrap();

        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tmp/tests")
            .join(format!("oracle-sqlplus-error-{}", std::process::id()));
        let network = root.join("network-empty");
        std::fs::create_dir_all(&network).unwrap();
        std::fs::set_permissions(&network, std::fs::Permissions::from_mode(0o700)).unwrap();
        let client = root.join("sqlplus-error-fixture.sh");
        std::fs::write(
            &client,
            r#"#!/bin/sh
[ "${1:-}" != "-V" ] || { printf 'SQL*Plus: Release 21.0.0.0.0 - Production\nVersion 21.3.0.0.0\n'; exit 0; }
marker=
while IFS= read -r line; do
  case "$line" in
    PROMPT\ __DBWARP_BP_*\|READY)
      marker=$(printf '%s' "$line" | cut -d' ' -f2 | cut -d'|' -f1)
      printf '%s|READY\n' "$marker"
      ;;
	    PROMPT\ __DBWARP_BP_*\|CLIENT\|*)
	      client_marker=$(printf '%s' "$line" | cut -d' ' -f2 | cut -d'|' -f1)
	      marker=$client_marker
	      printf '%s|CLIENT|2103000000\n' "$client_marker"
	      ;;
	    SELECT\ \'__DBWARP_BP_*\|AUTH\'\ FROM\ SYS.DUAL*)
	      printf '%s|AUTH\n' "$marker"
	      ;;
    PROMPT\ __DBWARP_BP_*\|BEGIN\|2)
      printf '%s|BEGIN|2\n' "$marker"
      printf 'SELECT customer_identifier FROM unavailable_object\n'
      printf '       *\n'
      printf 'ORA-00942: synthetic native text that must not cross the adapter\n'
      printf '%s|END|0\n' "$marker"
      ;;
  esac
done
"#,
        )
        .unwrap();
        std::fs::set_permissions(&client, std::fs::Permissions::from_mode(0o700)).unwrap();

        let secret_variable = format!("DBWARP_ORACLE_SQLPLUS_ERROR_{}", std::process::id());
        std::env::set_var(&secret_variable, "unit-test-only");
        let secret = Secret::from_env(&secret_variable).unwrap();
        std::env::remove_var(&secret_variable);
        let mut adapter = OracleSqlplusAdapter::start(OracleSqlplusConfig {
            executable: &client,
            username: "COLLECTOR",
            connect_identifier: "//127.0.0.1:1521/APPPDB",
            password: &secret,
            isolated_network_config_dir: &network,
            startup_timeout: Duration::from_secs(2),
        })
        .unwrap();
        let mut rows = Collected(Vec::new());
        assert_eq!(
            adapter.execute(
                &execution("SELECT owner, 42 FROM SYS.DBA_TABLES", None),
                &mut rows,
            ),
            OracleExecuteResult::Failed(OracleQueryFailure::ObjectAbsent)
        );
        assert!(rows.0.is_empty());
        assert_eq!(adapter.last_oracle_code, Some(-942));

        drop(adapter);
        std::fs::remove_file(client).unwrap();
        std::fs::remove_dir(network).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn silent_command_client_is_terminated_at_the_statement_deadline() {
        use std::os::unix::fs::PermissionsExt;

        let _environment_guard = PROCESS_ENVIRONMENT_LOCK.lock().unwrap();

        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tmp/tests")
            .join(format!("oracle-sqlplus-timeout-{}", std::process::id()));
        let network = root.join("network-empty");
        std::fs::create_dir_all(&network).unwrap();
        std::fs::set_permissions(&network, std::fs::Permissions::from_mode(0o700)).unwrap();
        let client = root.join("sqlplus-timeout-fixture.sh");
        std::fs::write(
            &client,
            r#"#!/bin/sh
[ "${1:-}" != "-V" ] || { printf 'SQL*Plus: Release 21.0.0.0.0 - Production\nVersion 21.3.0.0.0\n'; exit 0; }
while IFS= read -r line; do
  case "$line" in
    PROMPT\ __DBWARP_BP_*\|READY)
      marker=$(printf '%s' "$line" | cut -d' ' -f2 | cut -d'|' -f1)
      printf '%s|READY\n' "$marker"
      ;;
	    PROMPT\ __DBWARP_BP_*\|CLIENT\|*)
	      client_marker=$(printf '%s' "$line" | cut -d' ' -f2 | cut -d'|' -f1)
	      marker=$client_marker
	      printf '%s|CLIENT|2103000000\n' "$client_marker"
	      ;;
	    SELECT\ \'__DBWARP_BP_*\|AUTH\'\ FROM\ SYS.DUAL*)
	      printf '%s|AUTH\n' "$marker"
	      ;;
  esac
done
"#,
        )
        .unwrap();
        std::fs::set_permissions(&client, std::fs::Permissions::from_mode(0o700)).unwrap();

        let secret_variable = format!("DBWARP_ORACLE_SQLPLUS_TIMEOUT_{}", std::process::id());
        std::env::set_var(&secret_variable, "unit-test-only");
        let secret = Secret::from_env(&secret_variable).unwrap();
        std::env::remove_var(&secret_variable);
        let mut adapter = OracleSqlplusAdapter::start(OracleSqlplusConfig {
            executable: &client,
            username: "COLLECTOR",
            connect_identifier: "//127.0.0.1:1521/APPPDB",
            password: &secret,
            isolated_network_config_dir: &network,
            startup_timeout: Duration::from_secs(2),
        })
        .unwrap();
        let mut timed = execution("SELECT owner, 42 FROM SYS.DBA_TABLES", None);
        timed.deadline_ms = 20;
        let mut rows = Collected(Vec::new());
        assert_eq!(
            adapter.execute(&timed, &mut rows),
            OracleExecuteResult::Failed(OracleQueryFailure::Timeout)
        );
        assert!(adapter.terminated);
        assert!(rows.0.is_empty());

        drop(adapter);
        std::fs::remove_file(client).unwrap();
        std::fs::remove_dir(network).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    /// External integration test for a real SQL*Plus executable. It
    /// reads a protected password file and never emits its contents.
    #[test]
    #[ignore]
    fn external_sqlplus_rejects_invalid_login() {
        let executable = PathBuf::from(
            std::env::var("DBWARP_BLUEPRINT_ORACLE_TEST_CLIENT")
                .expect("DBWARP_BLUEPRINT_ORACLE_TEST_CLIENT must be set"),
        );
        let network = PathBuf::from(
            std::env::var("DBWARP_BLUEPRINT_ORACLE_TEST_NETWORK_DIR")
                .expect("DBWARP_BLUEPRINT_ORACLE_TEST_NETWORK_DIR must be set"),
        );
        let username = std::env::var("DBWARP_BLUEPRINT_ORACLE_TEST_USER")
            .expect("DBWARP_BLUEPRINT_ORACLE_TEST_USER must be set");
        let connect_identifier = std::env::var("DBWARP_BLUEPRINT_ORACLE_TEST_CONNECT")
            .expect("DBWARP_BLUEPRINT_ORACLE_TEST_CONNECT must be set");
        let password_file = PathBuf::from(
            std::env::var("DBWARP_BLUEPRINT_ORACLE_TEST_PASSWORD_FILE")
                .expect("DBWARP_BLUEPRINT_ORACLE_TEST_PASSWORD_FILE must be set"),
        );
        let secret = Secret::from_file(&password_file).expect("read protected test password");
        let result = OracleSqlplusAdapter::start(OracleSqlplusConfig {
            executable: &executable,
            username: &username,
            connect_identifier: &connect_identifier,
            password: &secret,
            isolated_network_config_dir: &network,
            startup_timeout: Duration::from_secs(30),
        });
        assert!(matches!(
            result,
            Err(OracleSqlplusStartFailure::LoginRejected)
        ));
        println!("DBWARP_BLUEPRINT_ORACLE_TEST_INVALID_LOGIN=true");
    }

    /// External integration test for a real SQL*Plus executable. It
    /// reads a protected password file and never emits its contents.
    #[test]
    #[ignore]
    fn external_sqlplus_basic_protocol() {
        use crate::artifacts::ArtifactDetail;
        use crate::oracle_basic::{map_oracle_basic_capture, OracleBasicOptions};
        use crate::oracle_provider::{
            negotiate_oracle_capabilities, OracleAuthentication, OracleCaptureRequest,
            OracleCaptureTier, OracleEndpointMode, OracleLocalInput, OracleProviderKind,
            OracleProviderLimits, OracleTransport, OracleTrustSource,
            ORACLE_PROVIDER_CONTRACT_VERSION,
        };
        use crate::oracle_scope::OracleOwnerScope;
        use crate::oracle_session::{run_catalog_capture, OracleQueryFailure, OracleQueryStatus};

        let executable = PathBuf::from(
            std::env::var("DBWARP_BLUEPRINT_ORACLE_TEST_CLIENT")
                .expect("DBWARP_BLUEPRINT_ORACLE_TEST_CLIENT must be set"),
        );
        let network = PathBuf::from(
            std::env::var("DBWARP_BLUEPRINT_ORACLE_TEST_NETWORK_DIR")
                .expect("DBWARP_BLUEPRINT_ORACLE_TEST_NETWORK_DIR must be set"),
        );
        let username = std::env::var("DBWARP_BLUEPRINT_ORACLE_TEST_USER")
            .expect("DBWARP_BLUEPRINT_ORACLE_TEST_USER must be set");
        let connect_identifier = std::env::var("DBWARP_BLUEPRINT_ORACLE_TEST_CONNECT")
            .expect("DBWARP_BLUEPRINT_ORACLE_TEST_CONNECT must be set");
        let password_file = PathBuf::from(
            std::env::var("DBWARP_BLUEPRINT_ORACLE_TEST_PASSWORD_FILE")
                .expect("DBWARP_BLUEPRINT_ORACLE_TEST_PASSWORD_FILE must be set"),
        );
        let secret = Secret::from_file(&password_file).expect("read protected test password");
        let mut probed = start_and_probe_sqlplus(
            OracleSqlplusConfig {
                executable: &executable,
                username: &username,
                connect_identifier: &connect_identifier,
                password: &secret,
                isolated_network_config_dir: &network,
                startup_timeout: Duration::from_secs(30),
            },
            OracleProviderLimits {
                catalog_rows: 50_000,
                catalog_bytes: 64 * 1024 * 1024,
                result_nesting_depth: 2,
                elapsed_ms: 120_000,
                transient_value_bytes: MAX_SCALAR_UTF8_BYTES as u64,
                lob_bytes: 1,
            },
        )
        .expect("start and probe SQLPlus Basic adapter");
        assert_eq!(probed.effective_principal(), username);
        println!("DBWARP_BLUEPRINT_ORACLE_TEST_PRINCIPAL_MATCH=true");
        let request = OracleCaptureRequest {
            contract_version: ORACLE_PROVIDER_CONTRACT_VERSION,
            provider: OracleProviderKind::Sqlplus,
            tier: OracleCaptureTier::Basic,
            artifact_detail: ArtifactDetail::Summary,
            endpoint_mode: OracleEndpointMode::DirectService,
            transport: OracleTransport::Tcp,
            authentication: OracleAuthentication::Password,
            trust_source: OracleTrustSource::NotApplicable,
            authorized_local_inputs: BTreeSet::from([OracleLocalInput::ClientExecutable]),
            require_confirmed_server_cancel: false,
        };
        let negotiated =
            negotiate_oracle_capabilities(&request, probed.declaration(), probed.evidence())
                .expect("admit live SQLPlus Basic evidence");
        assert_eq!(
            negotiated
                .proof()
                .attested_server_version()
                .expect("live proof has server version"),
            probed.server_version()
        );
        println!("DBWARP_BLUEPRINT_ORACLE_TEST_CAPABILITY=true");
        let owner = "APP";
        let capture = run_catalog_capture(&negotiated, probed.adapter_mut(), owner);
        let expected_denied = std::env::var("DBWARP_BLUEPRINT_ORACLE_TEST_DENIED_QUERY").ok();
        let expected_minimum_floor =
            std::env::var_os("DBWARP_BLUEPRINT_ORACLE_TEST_MINIMUM_FLOOR").is_some();
        let incomplete = capture
            .outcomes
            .iter()
            .filter(|outcome| !matches!(outcome.status, OracleQueryStatus::Executed { .. }))
            .map(|outcome| (outcome.query_id, &outcome.status))
            .collect::<Vec<_>>();
        assert_eq!(
            capture.abort, None,
            "sanitized outcomes: {incomplete:?}; numeric Oracle code: {:?}",
            probed.adapter.last_oracle_code
        );
        assert_eq!(
            capture.outcomes.len(),
            crate::oracle_catalog::queries_for_tier(OracleCaptureTier::Basic).count(),
            "the live adapter must execute the complete current Basic registry"
        );
        println!(
            "DBWARP_BLUEPRINT_ORACLE_TEST_OUTCOMES={}",
            capture.outcomes.len()
        );
        let (denied_query_observed, later_query_succeeded) = if expected_minimum_floor {
            for query_id in [
                "oracle-objects",
                "oracle-tables",
                "oracle-tab-columns",
                "oracle-segments",
                "oracle-instance",
            ] {
                let outcome = capture
                    .outcomes
                    .iter()
                    .find(|outcome| outcome.query_id == query_id)
                    .expect("minimum-floor query belongs to the Basic registry");
                assert!(
                    matches!(outcome.status, OracleQueryStatus::Executed { .. }),
                    "minimum-floor query did not execute: {query_id}={:?}",
                    outcome.status
                );
            }
            assert!(capture.outcomes.iter().any(|outcome| matches!(
                outcome.status,
                OracleQueryStatus::Failed {
                    class: OracleQueryFailure::PermissionDenied | OracleQueryFailure::ObjectAbsent
                }
            )));
            (true, true)
        } else if let Some(query_id) = expected_denied.as_deref() {
            let denied_position = capture
                .outcomes
                .iter()
                .position(|outcome| outcome.query_id == query_id)
                .expect("expected denied query belongs to the Basic registry");
            assert!(matches!(
                capture.outcomes[denied_position].status,
                OracleQueryStatus::Failed {
                    class: OracleQueryFailure::PermissionDenied | OracleQueryFailure::ObjectAbsent
                }
            ));
            assert!(capture.outcomes[denied_position + 1..]
                .iter()
                .any(|outcome| matches!(outcome.status, OracleQueryStatus::Executed { .. })));
            (true, true)
        } else {
            assert!(capture
                .outcomes
                .iter()
                .all(|outcome| matches!(outcome.status, OracleQueryStatus::Executed { .. })));
            (false, false)
        };
        let blueprint = map_oracle_basic_capture(
            &capture,
            &OracleOwnerScope::one(owner),
            probed.server_version(),
            &OracleBasicOptions {
                source_kind: "test".to_string(),
                generated_at_pin: Some("2026-09-14T00:00:00Z".to_string()),
                artifact_detail: ArtifactDetail::Summary,
            },
        )
        .expect("map SQLPlus Basic capture");
        assert!(!blueprint.tables.is_empty());
        if expected_minimum_floor {
            assert!(blueprint
                .tables
                .values()
                .all(|table| !table.cols.is_empty()));
            assert!(blueprint.tables.values().any(|table| table.table_bytes > 0));
            println!("DBWARP_BLUEPRINT_ORACLE_TEST_MINIMUM_CAPTURE=true");
        }
        if let Ok(receipt_path) = std::env::var("DBWARP_BLUEPRINT_ORACLE_TEST_RECEIPT_OUT") {
            let proof = negotiated.proof();
            let receipt = serde_json::json!({
                "contract": "dbwarp-blueprint-oracle-test-receipt/v1",
                "provider_artifact_sha256": proof
                    .provider_artifact_sha256()
                    .map(hex::encode)
                    .expect("live SQLPlus proof has provider digest"),
                "client_version_attestation": proof
                    .client_version_attestation()
                    .expect("live SQLPlus proof has client attestation")
                    .evidence_token(),
                "client_version": proof
                    .attested_client_version()
                    .map(|version| version.components().to_vec()),
                "client_version_unverified_reason": proof
                    .client_version_attestation()
                    .and_then(|attestation| attestation.limitation_token()),
                "server_version": proof
                    .attested_server_version()
                    .expect("live SQLPlus proof has server version")
                    .components(),
                "session_binding_sha256": proof
                    .session_binding_sha256()
                    .map(hex::encode)
                    .expect("live SQLPlus proof has session binding"),
                "effective_principal_match": true,
                "catalog_outcome_count": capture.outcomes.len(),
                "denied_query_observed": denied_query_observed,
                "later_query_succeeded": later_query_succeeded,
                "minimum_floor_capture": expected_minimum_floor,
                "emitted_table_count": blueprint.tables.len(),
            });
            std::fs::write(
                receipt_path,
                serde_json::to_vec_pretty(&receipt).expect("serialize Oracle test receipt"),
            )
            .expect("write Oracle test receipt");
        }
    }
}
