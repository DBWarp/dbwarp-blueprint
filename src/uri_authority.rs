//! Bracket-aware host:port parser shared by all three engine URI parsers.
//!
//! This module implements the minimal bracket-aware splitter described in
//! RFC 3986 §3.2.2 (host = IP-literal / IPv4address / reg-name) without
//! pulling in the full `url` crate as a direct dependency. The trust pitch
//! favors a small parser whose IPv6 and port boundary behavior is easy to
//! audit.

use anyhow::{anyhow, bail, Context, Result};

/// Split the scheme-stripped URI into authority and path components.
///
/// Search for the path separator only after the final `@`. A raw `/` is not
/// valid inside RFC 3986 userinfo, but treating an early slash as the
/// authority terminator can move part of a supplied password into `hostport`.
/// Any later host/port error could then disclose credential material. The
/// caller rejects embedded passwords on policy grounds after parsing; this
/// helper ensures malformed input reaches that safe boundary.
pub fn split_authority_and_path(rest: &str) -> (&str, &str) {
    let path_search_start = rest.rfind('@').map_or(0, |at| at + 1);
    match rest[path_search_start..].find('/') {
        Some(relative) => {
            let separator = path_search_start + relative;
            (&rest[..separator], &rest[separator + 1..])
        }
        None => (rest, ""),
    }
}

/// Split a `host[:port]` (or `[ipv6][:port]`) authority component.
///
/// - `host:5432`           → ("host", 5432)
/// - `host`                → ("host", default_port)
/// - `[::1]:5432`          → ("::1", 5432)             [brackets stripped]
/// - `[::1]`               → ("::1", default_port)
/// - `[fe80::1%eth0]:5432` → ("fe80::1%eth0", 5432)    [zone id preserved]
/// - `127.0.0.1:5432`      → ("127.0.0.1", 5432)
///
/// Returns Err on malformed input (unclosed bracket, junk between `]` and
/// `:`, non-numeric port).
pub fn split_host_port(hostport: &str, default_port: u16) -> Result<(String, u16)> {
    if hostport.is_empty() {
        bail!("authority host is empty");
    }
    if let Some(rest) = hostport.strip_prefix('[') {
        let close = rest
            .find(']')
            .ok_or_else(|| anyhow!("unclosed '[' in IPv6 host"))?;
        let host = &rest[..close];
        if host.is_empty() {
            bail!("empty IPv6 host inside brackets");
        }
        let after = &rest[close + 1..];
        if after.is_empty() {
            return Ok((host.to_string(), default_port));
        }
        // Allowed forms after `]`: `:PORT` (URL convention).
        if let Some(port_str) = after.strip_prefix(':') {
            let port = port_str.parse::<u16>().context("parsing authority port")?;
            return Ok((host.to_string(), port));
        }
        bail!("junk after IPv6 ']': expected ':PORT' or end-of-authority");
    }
    // No brackets — must NOT contain a colon-IPv6 form (those require brackets).
    // We distinguish "127.0.0.1:5432" (one colon, port follows) from
    // "::1" (two colons, IPv6 without brackets — rejected; tell user to bracket).
    let colon_count = hostport.bytes().filter(|&b| b == b':').count();
    if colon_count == 0 {
        return Ok((hostport.to_string(), default_port));
    }
    if colon_count == 1 {
        let i = hostport.rfind(':').unwrap();
        let port = hostport[i + 1..]
            .parse::<u16>()
            .context("parsing authority port")?;
        let host = &hostport[..i];
        if host.is_empty() {
            bail!("authority host is empty");
        }
        return Ok((host.to_string(), port));
    }
    // Multiple colons without brackets — almost certainly a bare IPv6 address.
    // Refuse rather than guess; the customer should bracket it.
    bail!(
        "authority contains multiple ':' without IPv6 brackets; \
         wrap an IPv6 address as '[::1]:port'"
    );
}

/// SQL Server allows `host,port` in addition to `host:port`. This wrapper
/// recognizes the comma form (with bracket awareness so a `,` inside an
/// IPv6 zone identifier doesn't mis-split — though currently zone ids don't
/// contain commas in practice, the bracket guard is cheap insurance).
pub fn split_host_port_mssql(hostport: &str, default_port: u16) -> Result<(String, u16)> {
    if hostport.is_empty() {
        bail!("authority host is empty");
    }
    // Find a comma that is *not* inside `[...]`.
    let mut depth = 0i32;
    let mut comma_pos: Option<usize> = None;
    for (i, b) in hostport.bytes().enumerate() {
        match b {
            b'[' => depth += 1,
            b']' => depth -= 1,
            b',' if depth == 0 => {
                comma_pos = Some(i);
                break;
            }
            _ => {}
        }
    }
    if let Some(i) = comma_pos {
        let host_str = &hostport[..i];
        let port_str = &hostport[i + 1..];
        let port = port_str.parse::<u16>().context("parsing authority port")?;
        // Host part may still be `[ipv6]` (bracket form without trailing colon-port).
        let host = if let Some(rest) = host_str.strip_prefix('[') {
            let close = rest
                .find(']')
                .ok_or_else(|| anyhow!("unclosed '[' in IPv6 host"))?;
            let h = &rest[..close];
            if h.is_empty() {
                bail!("empty IPv6 host inside brackets");
            }
            if !rest[close + 1..].is_empty() {
                bail!("junk after IPv6 ']'");
            }
            h.to_string()
        } else {
            if host_str.is_empty() {
                bail!("authority host is empty");
            }
            host_str.to_string()
        };
        return Ok((host, port));
    }
    split_host_port(hostport, default_port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authority_path_split_ignores_slash_before_final_userinfo_delimiter() {
        let marker = "credential-canary/";
        let rest = format!("app:{marker}@db.internal:5432/payments");
        let (authority, path) = split_authority_and_path(&rest);
        assert_eq!(authority, format!("app:{marker}@db.internal:5432"));
        assert_eq!(path, "payments");
    }

    #[test]
    fn plain_hostname_with_port() {
        let (h, p) = split_host_port("db.internal:5432", 9999).unwrap();
        assert_eq!(h, "db.internal");
        assert_eq!(p, 5432);
    }

    #[test]
    fn plain_hostname_without_port_uses_default() {
        let (h, p) = split_host_port("db.internal", 5432).unwrap();
        assert_eq!(h, "db.internal");
        assert_eq!(p, 5432);
    }

    #[test]
    fn ipv4_with_port() {
        let (h, p) = split_host_port("127.0.0.1:5432", 9999).unwrap();
        assert_eq!(h, "127.0.0.1");
        assert_eq!(p, 5432);
    }

    #[test]
    fn ipv6_bracketed_with_port() {
        let (h, p) = split_host_port("[::1]:5432", 9999).unwrap();
        assert_eq!(h, "::1");
        assert_eq!(p, 5432);
    }

    #[test]
    fn ipv6_bracketed_without_port_uses_default() {
        // Earlier hand-rolled parsers crashed by trying to parse
        // ":1]" as a port number.
        let (h, p) = split_host_port("[::1]", 5432).unwrap();
        assert_eq!(h, "::1");
        assert_eq!(p, 5432);
    }

    #[test]
    fn ipv6_zone_id_preserved() {
        let (h, p) = split_host_port("[fe80::1%eth0]:5432", 9999).unwrap();
        assert_eq!(h, "fe80::1%eth0");
        assert_eq!(p, 5432);
    }

    #[test]
    fn ipv6_unbracketed_is_rejected() {
        // A bare IPv6 address can't be disambiguated from host:port; the
        // user must bracket. We refuse rather than guess wrong.
        let err = split_host_port("::1", 5432).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("multiple ':'"), "got: {msg}");
    }

    #[test]
    fn ipv6_unclosed_bracket_rejected() {
        let err = split_host_port("[::1", 5432).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("unclosed"), "got: {msg}");
    }

    #[test]
    fn ipv6_empty_brackets_rejected() {
        let err = split_host_port("[]", 5432).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("empty IPv6 host"), "got: {msg}");
    }

    #[test]
    fn ipv6_junk_after_bracket_rejected() {
        let err = split_host_port("[::1]xyz", 5432).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("junk after IPv6"), "got: {msg}");
    }

    #[test]
    fn empty_authority_rejected() {
        let err = split_host_port("", 5432).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("empty"), "got: {msg}");
    }

    #[test]
    fn bad_port_rejected() {
        let unsafe_input = "host:credential-canary";
        let err = split_host_port(unsafe_input, 5432).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("parsing authority port"), "got: {msg}");
        assert!(!msg.contains(unsafe_input), "input was echoed: {msg}");
        assert!(
            !msg.contains("credential-canary"),
            "input was echoed: {msg}"
        );
    }

    #[test]
    fn mssql_comma_form() {
        let (h, p) = split_host_port_mssql("db.internal,1433", 9999).unwrap();
        assert_eq!(h, "db.internal");
        assert_eq!(p, 1433);
    }

    #[test]
    fn mssql_colon_form_still_works() {
        let (h, p) = split_host_port_mssql("db.internal:1433", 9999).unwrap();
        assert_eq!(h, "db.internal");
        assert_eq!(p, 1433);
    }

    #[test]
    fn mssql_default_port_when_omitted() {
        let (h, p) = split_host_port_mssql("db.internal", 1433).unwrap();
        assert_eq!(h, "db.internal");
        assert_eq!(p, 1433);
    }

    #[test]
    fn mssql_ipv6_with_comma_port() {
        let (h, p) = split_host_port_mssql("[::1],1433", 9999).unwrap();
        assert_eq!(h, "::1");
        assert_eq!(p, 1433);
    }

    #[test]
    fn mssql_ipv6_with_colon_port() {
        let (h, p) = split_host_port_mssql("[::1]:1433", 9999).unwrap();
        assert_eq!(h, "::1");
        assert_eq!(p, 1433);
    }

    #[test]
    fn mssql_ipv6_no_port_uses_default() {
        let (h, p) = split_host_port_mssql("[::1]", 1433).unwrap();
        assert_eq!(h, "::1");
        assert_eq!(p, 1433);
    }
}
