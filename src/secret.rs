//! Credential handling — single-file surface, audit-grep-able.
//!
//! `Secret` wraps a credential string in a `Zeroizing` buffer that is wiped
//! on drop. It deliberately does NOT implement `Debug`, `Display`, `Clone`,
//! or any `Serialize` trait — passing a Secret to `tracing!`, `format!`,
//! `println!`, or `serde_json::to_string` is a compile error.
//!
//! There is exactly one method that exposes the bytes: `expose()`. Every
//! call site is grep-able as `\.expose\(\)`. The customer's audit reads
//! this isolated module plus the grep result, keeping the credential exposure
//! surface small and reviewable.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use zeroize::Zeroizing;

/// A credential value. Cannot be cloned, debugged, displayed, or serialized.
/// Zeroized on drop.
pub struct Secret(Zeroizing<String>);

impl Secret {
    /// Read a password from the controlling TTY (`/dev/tty`). Echo disabled.
    /// Reading from stdin is intentionally NOT supported — it would allow
    /// pipe-injection (`echo secret | tool`) which makes the credential
    /// visible in shell history.
    pub fn from_tty_prompt(prompt: &str) -> Result<Self> {
        // rpassword reads from /dev/tty when available; we use its
        // prompt_password for echo-off behavior.
        let raw =
            rpassword::prompt_password(prompt).context("reading password from controlling TTY")?;
        if raw.is_empty() {
            bail!("password from TTY was empty");
        }
        Ok(Self(Zeroizing::new(raw)))
    }

    /// Read a password from a file. Refuses to run if the file's mode is
    /// world-readable (0644+).
    pub fn from_file(path: &Path) -> Result<Self> {
        check_secret_file_mode(path)?;
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("reading password file '{}'", path.display()))?;
        // Trim only one trailing newline (common for `echo secret > file`),
        // not arbitrary whitespace — passwords can legitimately contain spaces.
        let trimmed = raw.strip_suffix('\n').unwrap_or(&raw);
        let trimmed = trimmed.strip_suffix('\r').unwrap_or(trimmed);
        if trimmed.is_empty() {
            bail!("password file '{}' was empty", path.display());
        }
        Ok(Self(Zeroizing::new(trimmed.to_string())))
    }

    /// Read a password from a named environment variable. Tool reads ONLY
    /// the variable the customer named — no fallback to PGPASSWORD,
    /// MYSQL_PWD, etc.
    pub fn from_env(var_name: &str) -> Result<Self> {
        let raw = std::env::var(var_name).map_err(|_| anyhow!("env var '{var_name}' not set"))?;
        if raw.is_empty() {
            bail!("env var '{var_name}' was empty");
        }
        Ok(Self(Zeroizing::new(raw)))
    }

    /// EXPLICIT exposure — every call site is the entire credential leak
    /// surface. Grep `\.expose\(\)` to find them all.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Placeholder Secret for the `--auth-mode integrated` path, where
    /// the actual credential lives in the OS-level Kerberos TGT cache
    /// (Linux) or the current Windows session (Windows). The engine's
    /// Integrated dispatch arm does not call `.expose()` — the
    /// placeholder exists only so the `engine_mssql::run(secret: &Secret, ...)`
    /// signature stays uniform across auth modes.
    pub fn placeholder_for_integrated_auth() -> Self {
        Self(Zeroizing::new(String::new()))
    }
}

fn check_secret_file_mode(path: &Path) -> Result<()> {
    check_sensitive_file_mode(path, "password file")
}

/// Shared mode check used by `--password-file`, `--tls-key`, and any
/// future sensitive-file flag. Refuses to read the file if group or
/// other can read it (mode bits `0o044` set). On non-Unix platforms it emits
/// a warning because Unix mode bits cannot express the host ACL policy.
///
/// `label` is included verbatim in error/warning messages so the
/// customer can tell which flag tripped the check.
#[cfg(unix)]
pub fn check_sensitive_file_mode(path: &Path, label: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let meta =
        std::fs::metadata(path).with_context(|| format!("stat {label} '{}'", path.display()))?;
    let mode = meta.permissions().mode() & 0o777;
    // Refuse if group OR other can read.
    if mode & 0o044 != 0 {
        bail!(
            "{label} '{}' has mode {:#o}; refusing to read \
             (group and other read bits must be clear; recommended: 0600)",
            path.display(),
            mode,
        );
    }
    Ok(())
}

#[cfg(not(unix))]
pub fn check_sensitive_file_mode(path: &Path, label: &str) -> Result<()> {
    // Emit a loud warning rather than silently accept. Windows ACLs
    // require a different API surface (e.g. `windows-acl` / WinAPI
    // GetSecurityInfo) that this version of the tool does not pull
    // in. Until ACL-aware enforcement ships, the customer at least
    // sees that the tool is NOT enforcing the same restriction it
    // does on Unix.
    eprintln!(
        "{}",
        crate::i18n::format(
            "security.mode_check_noop",
            &[
                ("code", "DBP1605W".to_string()),
                ("label", label.to_string()),
                ("path", path.display().to_string()),
            ]
        )
    );
    Ok(())
}

/// Describe how a credential was supplied — emitted into the audit log,
/// never the credential itself.
#[derive(Debug, Clone)]
pub enum SecretSource {
    Tty,
    File {
        path: PathBuf,
        mode: Option<u32>,
    },
    Env {
        var_name: String,
    },
    ConnectionString,
    /// Microsoft Entra ID (Azure AD) access token read from a file.
    EntraTokenFile {
        path: PathBuf,
        mode: Option<u32>,
    },
    /// Microsoft Entra ID access token read from a named env var.
    EntraTokenEnv {
        var_name: String,
    },
    /// Integrated authentication: Kerberos TGT cache (Linux via
    /// libgssapi) or current Windows session (SSPI). dbwarp-blueprint
    /// itself reads no credential — the OS-level cache supplies
    /// it during the SQL Server handshake.
    IntegratedAuth,
}

impl SecretSource {
    pub fn audit_str(&self) -> String {
        match self {
            Self::Tty => "tty_prompt".to_string(),
            Self::File { path, mode } => match mode {
                Some(m) => format!("file:{} (mode {:#o})", path.display(), m),
                None => format!("file:{}", path.display()),
            },
            Self::Env { var_name } => format!("env:{var_name}"),
            Self::ConnectionString => {
                "connection_string (insecure: visible in process list)".to_string()
            }
            Self::EntraTokenFile { path, mode } => match mode {
                Some(m) => format!("entra_token_file:{} (mode {:#o})", path.display(), m),
                None => format!("entra_token_file:{}", path.display()),
            },
            Self::EntraTokenEnv { var_name } => format!("entra_token_env:{var_name}"),
            Self::IntegratedAuth => {
                "integrated_auth (OS-level Kerberos TGT cache or Windows session)".to_string()
            }
        }
    }
}

#[cfg(unix)]
pub fn file_mode(path: &Path) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .ok()
        .map(|m| m.permissions().mode() & 0o777)
}

#[cfg(not(unix))]
pub fn file_mode(_path: &Path) -> Option<u32> {
    None
}

// Compile-time guarantee: Secret is intentionally not Clone, Debug, Display,
// Serialize. To verify, uncomment the body of the closure below — it MUST
// fail to compile.
//
//   #[cfg(test)]
//   mod _trait_negatives {
//       fn _forbid_clone(s: super::Secret) { fn need_clone<T: Clone>(_: T) {} need_clone(s); }
//   }

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    #[test]
    fn from_env_explicit_only() {
        std::env::set_var("__DBWARP_BLUEPRINT_TEST_PASS", "abc");
        let s = Secret::from_env("__DBWARP_BLUEPRINT_TEST_PASS").unwrap();
        assert_eq!(s.expose(), "abc");
        std::env::remove_var("__DBWARP_BLUEPRINT_TEST_PASS");
        // Default fallback names MUST NOT be consulted.
        std::env::set_var("PGPASSWORD", "should-not-be-read");
        // We do not have a from_env_with_fallback() and explicitly do not.
        assert!(Secret::from_env("__DBWARP_BLUEPRINT_TEST_NOT_SET").is_err());
        std::env::remove_var("PGPASSWORD");
    }

    #[test]
    #[cfg(unix)]
    fn from_file_strict_mode() {
        use std::os::unix::fs::PermissionsExt;
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tmp/tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!(
            "dbwarp-blueprint-secret-test-{}.pw",
            std::process::id()
        ));
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(b"hunter2\n").unwrap();
        drop(f);
        // World-readable: should fail.
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(Secret::from_file(&path).is_err());
        // 0600: should succeed.
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let s = Secret::from_file(&path).unwrap();
        assert_eq!(s.expose(), "hunter2");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn audit_str_never_contains_secret_value() {
        // Guard: no SecretSource variant uses the credential value itself.
        let cases = [
            SecretSource::Tty,
            SecretSource::File {
                path: PathBuf::from("test-output/x"),
                mode: Some(0o600),
            },
            SecretSource::Env {
                var_name: "FOO".to_string(),
            },
            SecretSource::ConnectionString,
        ];
        for c in &cases {
            let s = c.audit_str();
            assert!(!s.contains("hunter2"));
        }
    }
}
