//! TLS policy shared by all live engines, plus the rustls connector used by
//! PostgreSQL and MySQL. SQL Server applies the validated policy through
//! tiberius in `engine_mssql` and has engine-specific trust limitations.
//!
//! Goals (the trust contract):
//!   * `--tls-ca PATH` is restrictive: when supplied, ONLY that CA bundle
//!     is trusted. The system trust store is NOT consulted. This prevents
//!     "I supplied my internal CA but the tool also trusted public CAs"
//!     surprises in environments with TLS-intercepting corporate proxies.
//!   * `--tls-mode disable|prefer|require|verify-ca|verify-full` defaults to
//!     `verify-full`. PostgreSQL `prefer` fallback is loopback-only so an
//!     authentication or handshake failure can never cause a credential to be
//!     retried over plaintext across a network boundary.
//!   * `--tls-skip-verify` is loud: emits stderr warning, recorded in
//!     audit, refused on non-loopback addresses unless the second flag
//!     `--i-know-what-im-doing` is also passed.
//!   * PostgreSQL/MySQL support multi-cert PEM bundles and mTLS via
//!     `--tls-cert PATH` + `--tls-key PATH`.
//!   * SQL Server uses native roots by default, accepts one explicit CA, checks
//!     hostnames in both verifying modes, and rejects client-certificate auth.

use std::fs::File;
use std::io::BufReader;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};
use rustls::client::danger::{ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use rustls::{
    CertificateError, ClientConfig, DigitallySignedStruct, Error as RustlsError, RootCertStore,
    SignatureScheme,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsMode {
    /// No TLS. Connection is plain TCP regardless of target. Same
    /// semantics as libpq's `sslmode=disable`. Customers wanting
    /// loopback-only plain TCP can layer their own check (e.g.,
    /// refuse `disable` in their invocation script if `--connect`
    /// doesn't resolve to 127.0.0.1).
    Disable,
    /// Try TLS; fall back to plain on server rejection. Loopback-only.
    Prefer,
    /// TLS required. Connection fails if server rejects TLS.
    Require,
    /// TLS required, server cert verified against CA bundle. Hostname not checked.
    VerifyCa,
    /// TLS required, server cert verified, hostname matched. Recommended.
    VerifyFull,
}

impl TlsMode {
    pub fn parse(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "disable" => Ok(Self::Disable),
            "prefer" => Ok(Self::Prefer),
            "require" => Ok(Self::Require),
            "verify-ca" | "verify_ca" => Ok(Self::VerifyCa),
            "verify-full" | "verify_full" => Ok(Self::VerifyFull),
            other => bail!(
                "unknown --tls-mode '{other}'; expected one of \
                 disable | prefer | require | verify-ca | verify-full"
            ),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disable => "disable",
            Self::Prefer => "prefer",
            Self::Require => "require",
            Self::VerifyCa => "verify-ca",
            Self::VerifyFull => "verify-full",
        }
    }

    pub fn verifies_cert(self) -> bool {
        matches!(self, Self::VerifyCa | Self::VerifyFull)
    }

    pub fn verifies_hostname(self) -> bool {
        matches!(self, Self::VerifyFull)
    }
}

#[derive(Debug, Clone)]
pub struct TlsParams {
    pub mode: TlsMode,
    pub ca_bundle: Option<PathBuf>,
    pub client_cert: Option<PathBuf>,
    pub client_key: Option<PathBuf>,
    pub server_name_override: Option<String>,
    /// `--tls-skip-verify`. Loud, refused on non-loopback unless
    /// `--i-know-what-im-doing` is also set.
    pub skip_verify: bool,
    pub override_safety: bool,
}

impl Default for TlsParams {
    fn default() -> Self {
        Self {
            mode: TlsMode::VerifyFull,
            ca_bundle: None,
            client_cert: None,
            client_key: None,
            server_name_override: None,
            skip_verify: false,
            override_safety: false,
        }
    }
}

/// Validate the params consistency and reject obviously dangerous combinations.
/// `host` is the connection target (used for the loopback-only check on
/// `skip_verify`).
pub fn validate(p: &TlsParams, host: &str) -> Result<()> {
    if p.client_cert.is_some() ^ p.client_key.is_some() {
        bail!("--tls-cert and --tls-key must be supplied together (or neither)");
    }
    if p.skip_verify && !p.override_safety {
        if !is_loopback(host) {
            bail!(
                "--tls-skip-verify against non-loopback host '{host}' is refused. \
                 Add --i-know-what-im-doing to override (NOT RECOMMENDED — this \
                 disables certificate verification entirely)."
            );
        }
    }
    if p.mode == TlsMode::Prefer && !is_loopback(host) {
        bail!(
            "--tls-mode=prefer is restricted to loopback targets because an automatic \
             plaintext retry can expose credentials after a TLS failure. Use \
             --tls-mode=verify-full (recommended), or choose another explicit TLS policy."
        );
    }
    if matches!(p.mode, TlsMode::Disable | TlsMode::Require)
        && !is_loopback(host)
        && !p.override_safety
    {
        bail!(
            "--tls-mode={} does not verify the remote database identity. Against a \
             non-loopback host, use --tls-mode=verify-full (recommended) or add \
             --i-know-what-im-doing after explicit security approval.",
            p.mode.as_str()
        );
    }
    if p.skip_verify && p.mode == TlsMode::VerifyFull {
        // verify-full asks the tool to verify; skip-verify says don't. Reject
        // the contradiction explicitly rather than silently doing one or the
        // other.
        bail!("--tls-mode=verify-full and --tls-skip-verify are mutually exclusive");
    }
    if p.server_name_override.is_some() {
        bail!(
            "--tls-server-name is not supported by this release; use a --connect hostname \
             that matches the certificate, or use --tls-mode=verify-ca if your policy \
             permits CA validation without hostname validation"
        );
    }
    if let Some(p) = &p.ca_bundle {
        if !p.exists() {
            bail!("--tls-ca '{}' does not exist", p.display());
        }
    }
    if let Some(p) = &p.client_cert {
        if !p.exists() {
            bail!("--tls-cert '{}' does not exist", p.display());
        }
    }
    if let Some(p) = &p.client_key {
        if !p.exists() {
            bail!("--tls-key '{}' does not exist", p.display());
        }
        // SECURITY.md and AUDIT.md claim --tls-key gets the same mode
        // check as --password-file. Enforced here so the docs match
        // reality: refuses 0644+ on Unix; emits a loud warning on
        // Windows (same posture as --password-file).
        crate::secret::check_sensitive_file_mode(p, "--tls-key")?;
    }
    Ok(())
}

fn is_loopback(host: &str) -> bool {
    if host == "localhost" {
        return true;
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        match ip {
            IpAddr::V4(v4) => v4 == Ipv4Addr::LOCALHOST || v4.is_loopback(),
            IpAddr::V6(v6) => v6 == Ipv6Addr::LOCALHOST || v6.is_loopback(),
        }
    } else {
        false
    }
}

/// Build a rustls ClientConfig for the given params. Returns None if
/// `mode == Disable`. `audit_ca_only` is set to true iff the resulting
/// config trusts ONLY the supplied --tls-ca (no system roots, no webpki-roots).
pub fn build_client_config(p: &TlsParams) -> Result<Option<(Arc<ClientConfig>, bool)>> {
    if p.mode == TlsMode::Disable {
        return Ok(None);
    }

    let mut roots = RootCertStore::empty();
    let mut ca_only = false;

    if let Some(ca_path) = &p.ca_bundle {
        // Load ONLY the supplied CA bundle. Restrictive-CA semantics.
        let mut rd = BufReader::new(
            File::open(ca_path)
                .with_context(|| format!("opening --tls-ca '{}'", ca_path.display()))?,
        );
        let mut added = 0usize;
        for cert in rustls_pemfile::certs(&mut rd) {
            let cert = cert.with_context(|| {
                format!(
                    "parsing PEM certificate in --tls-ca '{}'",
                    ca_path.display()
                )
            })?;
            roots
                .add(cert)
                .map_err(|e| anyhow!("rustls rejected certificate in --tls-ca: {e}"))?;
            added += 1;
        }
        if added == 0 {
            bail!("--tls-ca '{}' contained no certificates", ca_path.display());
        }
        ca_only = true;
    } else if p.mode.verifies_cert() {
        // No --tls-ca but verify-* requested. Use webpki-roots (Mozilla CA
        // bundle compiled into the binary). NOT the system trust store.
        // This keeps the trust set deterministic and verifiable per-build.
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    }

    // Build the verifier: real one (with optional skip-verify) or a no-op
    // verifier when skip_verify is set.
    let cfg_builder = ClientConfig::builder().with_root_certificates(roots);

    let cfg = if let (Some(cert), Some(key)) = (&p.client_cert, &p.client_key) {
        let cert_chain = load_cert_chain(cert)?;
        let key = load_private_key(key)?;
        let mut cfg = cfg_builder
            .with_client_auth_cert(cert_chain, key)
            .map_err(|e| anyhow!("client mTLS configuration failed: {e}"))?;
        // Note: if skip_verify is set, we replace the cert verifier below.
        if p.skip_verify {
            cfg.dangerous()
                .set_certificate_verifier(Arc::new(SkipVerifyAnyCert));
        } else if !p.mode.verifies_hostname() {
            // verify-ca: verify cert chain but skip hostname match.
            cfg.dangerous()
                .set_certificate_verifier(Arc::new(VerifyChainOnlyVerifier {
                    inner: WebPkiServerVerifierWrapper::new(&p.ca_bundle, p.mode)?,
                }));
        }
        cfg
    } else {
        let mut cfg = cfg_builder.with_no_client_auth();
        if p.skip_verify {
            cfg.dangerous()
                .set_certificate_verifier(Arc::new(SkipVerifyAnyCert));
        } else if !p.mode.verifies_hostname() {
            cfg.dangerous()
                .set_certificate_verifier(Arc::new(VerifyChainOnlyVerifier {
                    inner: WebPkiServerVerifierWrapper::new(&p.ca_bundle, p.mode)?,
                }));
        }
        cfg
    };

    Ok(Some((Arc::new(cfg), ca_only)))
}

/// Validate the narrower CA-file contract exposed by tiberius.
///
/// Unlike the PostgreSQL/MySQL connector, tiberius accepts one explicit CA
/// certificate rather than a bundle. It also selects parsers by extension, so
/// reject inputs here before connection setup can misclassify them as a
/// generic database failure.
pub fn validate_mssql_ca(p: &TlsParams) -> Result<()> {
    let Some(ca_path) = p.ca_bundle.as_ref() else {
        return Ok(());
    };
    let extension = ca_path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !matches!(extension.as_str(), "pem" | "crt") {
        bail!(
            "SQL Server --tls-ca '{}' must use a .pem or .crt file containing exactly one certificate",
            ca_path.display()
        );
    }
    let mut reader = BufReader::new(
        File::open(ca_path)
            .with_context(|| format!("opening SQL Server --tls-ca '{}'", ca_path.display()))?,
    );
    let certificates = rustls_pemfile::certs(&mut reader)
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| format!("parsing SQL Server --tls-ca '{}'", ca_path.display()))?;
    if certificates.len() != 1 {
        bail!(
            "SQL Server --tls-ca '{}' must contain exactly one certificate (found {})",
            ca_path.display(),
            certificates.len()
        );
    }
    let certificate = certificates
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("SQL Server --tls-ca contained no certificate"))?;
    let mut roots = RootCertStore::empty();
    roots
        .add(certificate)
        .map_err(|error| anyhow!("rustls rejected SQL Server --tls-ca: {error}"))?;
    Ok(())
}

fn load_cert_chain(path: &Path) -> Result<Vec<CertificateDer<'static>>> {
    let mut rd = BufReader::new(
        File::open(path).with_context(|| format!("opening --tls-cert '{}'", path.display()))?,
    );
    let chain = rustls_pemfile::certs(&mut rd)
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| format!("parsing --tls-cert '{}'", path.display()))?;
    if chain.is_empty() {
        bail!("--tls-cert '{}' contained no certificates", path.display());
    }
    Ok(chain)
}

fn load_private_key(path: &Path) -> Result<PrivateKeyDer<'static>> {
    let mut rd = BufReader::new(
        File::open(path).with_context(|| format!("opening --tls-key '{}'", path.display()))?,
    );
    // Accept any of: PKCS8, RSA, SEC1, generic.
    match rustls_pemfile::private_key(&mut rd)
        .with_context(|| format!("parsing --tls-key '{}'", path.display()))?
    {
        Some(k) => Ok(k),
        None => bail!("--tls-key '{}' contained no private key", path.display()),
    }
}

/// A certificate verifier that accepts any cert. Used only with
/// --tls-skip-verify (which is gated by the loopback / override-safety check).
#[derive(Debug)]
struct SkipVerifyAnyCert;

impl ServerCertVerifier for SkipVerifyAnyCert {
    fn verify_server_cert(
        &self,
        _end: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, RustlsError> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _msg: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, RustlsError> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _msg: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, RustlsError> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::RSA_PKCS1_SHA512,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
            SignatureScheme::ED25519,
        ]
    }
}

/// Verifier that runs the full webpki chain check but skips hostname matching.
/// Used for --tls-mode=verify-ca.
#[derive(Debug)]
struct VerifyChainOnlyVerifier {
    inner: Arc<dyn ServerCertVerifier>,
}

impl ServerCertVerifier for VerifyChainOnlyVerifier {
    fn verify_server_cert(
        &self,
        end: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> std::result::Result<ServerCertVerified, RustlsError> {
        // Use a placeholder server name so the inner verifier checks chain
        // validity but its hostname-match step is satisfied trivially.
        let placeholder = ServerName::try_from("dbwarp-blueprint.invalid").unwrap();
        match self
            .inner
            .verify_server_cert(end, intermediates, &placeholder, ocsp_response, now)
        {
            Ok(v) => Ok(v),
            // verify-ca semantics: chain must validate, but the SAN/CN
            // mismatch against our placeholder is the *expected* result
            // and is the only failure we suppress. We narrowly match
            // only the two name-mismatch variants `webpki-rs` reports
            // (NotValidForName + NotValidForNameContext) — masking the
            // full `InvalidCertificate(_)` family would also swallow
            // Expired, Revoked, BadEncoding, BadSignature, UnknownIssuer
            // and every other rustls cert failure. Anything else
            // propagates.
            Err(RustlsError::InvalidCertificate(CertificateError::NotValidForName))
            | Err(RustlsError::InvalidCertificate(CertificateError::NotValidForNameContext {
                ..
            })) => Ok(ServerCertVerified::assertion()),
            Err(e) => Err(e),
        }
    }

    fn verify_tls12_signature(
        &self,
        msg: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, RustlsError> {
        self.inner.verify_tls12_signature(msg, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        msg: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, RustlsError> {
        self.inner.verify_tls13_signature(msg, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.inner.supported_verify_schemes()
    }
}

/// Wrap rustls's WebPkiServerVerifier so we can hand it an Arc<dyn ServerCertVerifier>.
struct WebPkiServerVerifierWrapper;

impl WebPkiServerVerifierWrapper {
    fn new(ca_bundle: &Option<PathBuf>, _mode: TlsMode) -> Result<Arc<dyn ServerCertVerifier>> {
        let mut roots = RootCertStore::empty();
        if let Some(p) = ca_bundle {
            let mut rd = BufReader::new(File::open(p)?);
            for cert in rustls_pemfile::certs(&mut rd) {
                let cert = cert?;
                roots
                    .add(cert)
                    .map_err(|e| anyhow!("rustls rejected --tls-ca cert: {e}"))?;
            }
        } else {
            roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        }
        let v = rustls::client::WebPkiServerVerifier::builder(Arc::new(roots))
            .build()
            .map_err(|e| anyhow!("building WebPki verifier: {e}"))?;
        Ok(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tls_modes() {
        assert_eq!(TlsMode::parse("disable").unwrap(), TlsMode::Disable);
        assert_eq!(TlsMode::parse("PREFER").unwrap(), TlsMode::Prefer);
        assert_eq!(TlsMode::parse("verify-full").unwrap(), TlsMode::VerifyFull);
        assert_eq!(TlsMode::parse("verify_ca").unwrap(), TlsMode::VerifyCa);
        assert!(TlsMode::parse("garbage").is_err());
    }

    #[test]
    fn verified_tls_is_the_default() {
        assert_eq!(TlsParams::default().mode, TlsMode::VerifyFull);
    }

    #[test]
    fn remote_plaintext_and_unverified_tls_require_explicit_approval() {
        for mode in [TlsMode::Disable, TlsMode::Require] {
            let params = TlsParams {
                mode,
                ..TlsParams::default()
            };
            assert!(validate(&params, "db.example.com").is_err());

            let approved = TlsParams {
                override_safety: true,
                ..params
            };
            assert!(validate(&approved, "db.example.com").is_ok());
        }
    }

    #[test]
    fn tls_prefer_plaintext_fallback_is_loopback_only() {
        let params = TlsParams {
            mode: TlsMode::Prefer,
            override_safety: true,
            ..TlsParams::default()
        };
        assert!(validate(&params, "db.example.com").is_err());
        assert!(validate(&params, "localhost").is_ok());
    }

    #[test]
    fn loopback_detection() {
        assert!(is_loopback("localhost"));
        assert!(is_loopback("127.0.0.1"));
        assert!(is_loopback("::1"));
        assert!(!is_loopback("10.0.0.1"));
        assert!(!is_loopback("db.example.com"));
    }

    #[test]
    fn skip_verify_refused_on_non_loopback() {
        let p = TlsParams {
            skip_verify: true,
            ..TlsParams::default()
        };
        assert!(validate(&p, "db.example.com").is_err());
    }

    #[test]
    fn skip_verify_allowed_on_loopback() {
        let p = TlsParams {
            skip_verify: true,
            mode: TlsMode::Require, // not VerifyFull, so no contradiction
            ..TlsParams::default()
        };
        assert!(validate(&p, "127.0.0.1").is_ok());
    }

    #[test]
    fn skip_verify_with_override_allowed() {
        let p = TlsParams {
            skip_verify: true,
            override_safety: true,
            mode: TlsMode::Require,
            ..TlsParams::default()
        };
        assert!(validate(&p, "db.example.com").is_ok());
    }

    #[test]
    fn skip_verify_contradicts_verify_full() {
        let p = TlsParams {
            skip_verify: true,
            mode: TlsMode::VerifyFull,
            ..TlsParams::default()
        };
        assert!(validate(&p, "127.0.0.1").is_err());
    }

    #[test]
    fn cert_without_key_rejected() {
        let p = TlsParams {
            client_cert: Some(PathBuf::from("/nonexistent.crt")),
            client_key: None,
            ..TlsParams::default()
        };
        assert!(validate(&p, "127.0.0.1").is_err());
    }

    #[test]
    fn mssql_ca_rejects_an_extension_the_driver_cannot_parse() {
        let p = TlsParams {
            ca_bundle: Some(PathBuf::from("internal-ca.bundle")),
            ..TlsParams::default()
        };
        let error = validate_mssql_ca(&p).expect_err("unsupported CA extension must fail");
        assert!(
            format!("{error:#}").contains(".pem or .crt"),
            "unexpected error: {error:#}"
        );
    }

    /// `--tls-key` must get the same mode check as `--password-file`.
    /// Create a temp key file, chmod 0644, expect validate() to refuse
    /// before the file is opened.
    #[cfg(unix)]
    #[test]
    fn tls_key_world_readable_rejected() {
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tmp/tests")
            .join(format!(
                "dbwarp-blueprint-tls-key-mode-test-{}",
                std::process::id()
            ));
        std::fs::create_dir_all(&dir).unwrap();
        let cert_path = dir.join("client.crt");
        let key_path = dir.join("client.key");
        std::fs::File::create(&cert_path)
            .and_then(|mut f| {
                f.write_all(b"-----BEGIN CERTIFICATE-----\n-----END CERTIFICATE-----\n")
            })
            .unwrap();
        std::fs::File::create(&key_path)
            .and_then(|mut f| {
                f.write_all(b"-----BEGIN PRIVATE KEY-----\n-----END PRIVATE KEY-----\n")
            })
            .unwrap();
        std::fs::set_permissions(&cert_path, std::fs::Permissions::from_mode(0o644)).unwrap();
        std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o644)).unwrap();

        let p = TlsParams {
            client_cert: Some(cert_path),
            client_key: Some(key_path.clone()),
            ..TlsParams::default()
        };
        let res = validate(&p, "127.0.0.1");
        let _ = std::fs::remove_dir_all(&dir);

        let err = res.expect_err("0644 --tls-key must be refused");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("--tls-key") && msg.contains("0o644"),
            "expected --tls-key 0644 refusal; got: {msg}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn tls_key_mode_0600_accepted() {
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tmp/tests")
            .join(format!(
                "dbwarp-blueprint-tls-key-mode-ok-{}",
                std::process::id()
            ));
        std::fs::create_dir_all(&dir).unwrap();
        let cert_path = dir.join("client.crt");
        let key_path = dir.join("client.key");
        std::fs::File::create(&cert_path)
            .and_then(|mut f| {
                f.write_all(b"-----BEGIN CERTIFICATE-----\n-----END CERTIFICATE-----\n")
            })
            .unwrap();
        std::fs::File::create(&key_path)
            .and_then(|mut f| {
                f.write_all(b"-----BEGIN PRIVATE KEY-----\n-----END PRIVATE KEY-----\n")
            })
            .unwrap();
        std::fs::set_permissions(&cert_path, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600)).unwrap();

        let p = TlsParams {
            client_cert: Some(cert_path),
            client_key: Some(key_path),
            ..TlsParams::default()
        };
        let res = validate(&p, "127.0.0.1");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(res.is_ok(), "0600 --tls-key should pass; got: {res:?}");
    }

    // Regression suite: verify-ca semantics are *narrow* — only
    // hostname mismatch is suppressed, every other cert failure
    // (expired, revoked, bad signature, unknown issuer, bad encoding)
    // must propagate. Previously a single `RustlsError::InvalidCertificate(_)`
    // arm masked all of them.
    use std::sync::Mutex;

    #[derive(Debug)]
    struct MockInnerVerifier {
        next: Mutex<Option<RustlsError>>,
    }

    impl MockInnerVerifier {
        fn fail_with(err: RustlsError) -> Arc<Self> {
            Arc::new(Self {
                next: Mutex::new(Some(err)),
            })
        }
        fn always_ok() -> Arc<Self> {
            Arc::new(Self {
                next: Mutex::new(None),
            })
        }
    }

    impl ServerCertVerifier for MockInnerVerifier {
        fn verify_server_cert(
            &self,
            _end: &CertificateDer<'_>,
            _intermediates: &[CertificateDer<'_>],
            _server_name: &ServerName<'_>,
            _ocsp_response: &[u8],
            _now: UnixTime,
        ) -> std::result::Result<ServerCertVerified, RustlsError> {
            match self.next.lock().unwrap().take() {
                None => Ok(ServerCertVerified::assertion()),
                Some(e) => Err(e),
            }
        }
        fn verify_tls12_signature(
            &self,
            _: &[u8],
            _: &CertificateDer<'_>,
            _: &DigitallySignedStruct,
        ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, RustlsError>
        {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }
        fn verify_tls13_signature(
            &self,
            _: &[u8],
            _: &CertificateDer<'_>,
            _: &DigitallySignedStruct,
        ) -> std::result::Result<rustls::client::danger::HandshakeSignatureValid, RustlsError>
        {
            Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
        }
        fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
            vec![SignatureScheme::ED25519]
        }
    }

    fn run_verify_with(inner: Arc<dyn ServerCertVerifier>) -> std::result::Result<(), RustlsError> {
        let v = VerifyChainOnlyVerifier { inner };
        // Use a syntactically-valid name so ServerName::try_from succeeds;
        // the mock ignores it anyway.
        let name = ServerName::try_from("example.test").unwrap();
        // Empty cert is fine — mock doesn't inspect.
        let cert = CertificateDer::from(vec![0u8]);
        v.verify_server_cert(
            &cert,
            &[],
            &name,
            &[],
            UnixTime::since_unix_epoch(std::time::Duration::from_secs(0)),
        )
        .map(|_| ())
    }

    #[test]
    fn verify_ca_passes_when_inner_ok() {
        let r = run_verify_with(MockInnerVerifier::always_ok());
        assert!(r.is_ok(), "ok-inner must yield ok-outer; got {r:?}");
    }

    #[test]
    fn verify_ca_swallows_not_valid_for_name() {
        let r = run_verify_with(MockInnerVerifier::fail_with(
            RustlsError::InvalidCertificate(CertificateError::NotValidForName),
        ));
        assert!(
            r.is_ok(),
            "verify-ca semantics: name mismatch must be accepted; got {r:?}"
        );
    }

    #[test]
    fn verify_ca_propagates_expired() {
        let r = run_verify_with(MockInnerVerifier::fail_with(
            RustlsError::InvalidCertificate(CertificateError::Expired),
        ));
        assert!(
            matches!(
                r,
                Err(RustlsError::InvalidCertificate(CertificateError::Expired))
            ),
            "expired cert must propagate; got {r:?}"
        );
    }

    #[test]
    fn verify_ca_propagates_revoked() {
        let r = run_verify_with(MockInnerVerifier::fail_with(
            RustlsError::InvalidCertificate(CertificateError::Revoked),
        ));
        assert!(
            matches!(
                r,
                Err(RustlsError::InvalidCertificate(CertificateError::Revoked))
            ),
            "revoked cert must propagate; got {r:?}"
        );
    }

    #[test]
    fn verify_ca_propagates_bad_signature() {
        let r = run_verify_with(MockInnerVerifier::fail_with(
            RustlsError::InvalidCertificate(CertificateError::BadSignature),
        ));
        assert!(
            matches!(
                r,
                Err(RustlsError::InvalidCertificate(
                    CertificateError::BadSignature
                ))
            ),
            "bad-signature cert must propagate; got {r:?}"
        );
    }

    #[test]
    fn verify_ca_propagates_unknown_issuer() {
        let r = run_verify_with(MockInnerVerifier::fail_with(
            RustlsError::InvalidCertificate(CertificateError::UnknownIssuer),
        ));
        assert!(
            matches!(
                r,
                Err(RustlsError::InvalidCertificate(
                    CertificateError::UnknownIssuer
                ))
            ),
            "unknown-issuer cert must propagate; got {r:?}"
        );
    }

    #[test]
    fn verify_ca_propagates_bad_encoding() {
        let r = run_verify_with(MockInnerVerifier::fail_with(
            RustlsError::InvalidCertificate(CertificateError::BadEncoding),
        ));
        assert!(
            matches!(
                r,
                Err(RustlsError::InvalidCertificate(
                    CertificateError::BadEncoding
                ))
            ),
            "bad-encoding cert must propagate; got {r:?}"
        );
    }
}
