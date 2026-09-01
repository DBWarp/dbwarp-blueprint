//! Content style classifier.
//!
//! Reads a small bounded byte buffer locally, returns ONE LABEL describing
//! the dominant content style. The bytes themselves are never emitted.
//!
//! Categories: json | xml | natural-text | base64 | hex | numeric-text | mixed
//! Empty / unclassifiable input returns "" (skipped in output).
//!
//! Conservative: when uncertain, returns "mixed" rather than guessing.

const SAMPLE_LIMIT: usize = 4096;

/// Classify the first SAMPLE_LIMIT bytes of `buf`.
pub fn classify(buf: &[u8]) -> &'static str {
    if buf.is_empty() {
        return "";
    }
    let s = &buf[..buf.len().min(SAMPLE_LIMIT)];

    // Hex: every char in [0-9a-fA-F], length even, ≥ 16.
    if s.len() >= 16 && s.iter().all(|b| b.is_ascii_hexdigit()) && s.len() % 2 == 0 {
        return "hex";
    }

    // Base64: a-zA-Z0-9+/= only, length multiple of 4 (after stripping
    // trailing whitespace/newlines), ≥ 16.
    let trimmed = strip_ascii_ws_end(s);
    if trimmed.len() >= 16
        && trimmed.len() % 4 == 0
        && trimmed.iter().all(|b| {
            matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'+' | b'/' | b'=')
        })
        // base64 strings have a wide alphabet — exclude all-hex strings (already caught above).
        && has_alpha_mix(trimmed)
    {
        return "base64";
    }

    // Numeric-text: digits, dots, commas, dashes, plus, e, scientific notation.
    if s.iter().all(|b| {
        matches!(
            b,
            b'0'..=b'9' | b'.' | b',' | b'-' | b'+' | b'e' | b'E' | b' ' | b'\t' | b'\n' | b'\r'
        )
    }) && s.iter().any(|b| b.is_ascii_digit())
    {
        return "numeric-text";
    }

    // From here on we work with valid-UTF8 prefix; if not UTF-8, classify as mixed.
    let text = match std::str::from_utf8(s) {
        Ok(t) => t,
        Err(_) => {
            // Try to recover the longest valid prefix; if that's still small, treat as mixed.
            let prefix_len = std::str::from_utf8(s)
                .err()
                .map(|e| e.valid_up_to())
                .unwrap_or(0);
            if prefix_len < 32 {
                return "mixed";
            }
            // Safe because valid_up_to() is a UTF-8 boundary.
            unsafe { std::str::from_utf8_unchecked(&s[..prefix_len]) }
        }
    };
    let trimmed_text = text.trim_start();

    // JSON: starts with { or [, contains ":" with reasonable structure.
    if trimmed_text.starts_with('{') || trimmed_text.starts_with('[') {
        // very light heuristic — count balanced quotes and presence of ":"
        if looks_like_json(trimmed_text) {
            return "json";
        }
    }

    // XML: starts with `<?xml` or with `<` followed by an alpha char and includes `>`.
    if trimmed_text.starts_with("<?xml") {
        return "xml";
    }
    if let Some(rest) = trimmed_text.strip_prefix('<') {
        if rest
            .chars()
            .next()
            .map(|c| c.is_ascii_alphabetic() || c == '!')
            .unwrap_or(false)
            && trimmed_text.contains('>')
        {
            return "xml";
        }
    }

    // Natural text: letters dominate, has spaces, low symbol density.
    let mut letters = 0u32;
    let mut spaces = 0u32;
    let mut digits = 0u32;
    let mut symbols = 0u32;
    for c in text.chars() {
        if c.is_ascii_alphabetic() {
            letters += 1;
        } else if c == ' ' || c == '\t' || c == '\n' || c == '\r' {
            spaces += 1;
        } else if c.is_ascii_digit() {
            digits += 1;
        } else {
            symbols += 1;
        }
    }
    let total = letters + spaces + digits + symbols;
    if total >= 32 && letters > total / 2 && spaces > 0 && symbols < total / 4 {
        return "natural-text";
    }

    "mixed"
}

fn strip_ascii_ws_end(s: &[u8]) -> &[u8] {
    let mut end = s.len();
    while end > 0 {
        let b = s[end - 1];
        if b == b' ' || b == b'\t' || b == b'\n' || b == b'\r' {
            end -= 1;
        } else {
            break;
        }
    }
    &s[..end]
}

fn has_alpha_mix(s: &[u8]) -> bool {
    let mut upper = false;
    let mut lower = false;
    for &b in s {
        if b.is_ascii_uppercase() {
            upper = true;
        }
        if b.is_ascii_lowercase() {
            lower = true;
        }
        if upper && lower {
            return true;
        }
    }
    false
}

fn looks_like_json(s: &str) -> bool {
    // Shallow heuristic: contains :"...", or :{}, or :[]; balanced overall braces/brackets.
    let mut braces = 0i32;
    let mut brackets = 0i32;
    let mut in_string = false;
    let mut prev_escape = false;
    let mut saw_colon = false;
    for c in s.chars() {
        if in_string {
            if prev_escape {
                prev_escape = false;
            } else if c == '\\' {
                prev_escape = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '{' => braces += 1,
            '}' => braces -= 1,
            '[' => brackets += 1,
            ']' => brackets -= 1,
            ':' => saw_colon = true,
            _ => {}
        }
    }
    saw_colon && braces == 0 && brackets == 0 && !in_string
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_json_object() {
        let s = br#"{"id":1,"name":"alpha","tags":["a","b"]}"#;
        assert_eq!(classify(s), "json");
    }

    #[test]
    fn classifies_xml() {
        let s = br#"<?xml version="1.0"?><root><child>data</child></root>"#;
        assert_eq!(classify(s), "xml");
    }

    #[test]
    fn classifies_xml_no_decl() {
        let s = br#"<root><child>data</child></root>"#;
        assert_eq!(classify(s), "xml");
    }

    #[test]
    fn classifies_natural_text() {
        let s = b"The quick brown fox jumps over the lazy dog and many others.";
        assert_eq!(classify(s), "natural-text");
    }

    #[test]
    fn classifies_numeric_text() {
        let s = b"123.45,678.90,1000.00,2500.50,0.01,-3.14,42";
        assert_eq!(classify(s), "numeric-text");
    }

    #[test]
    fn classifies_hex() {
        let s = b"deadbeefcafef00d0123456789abcdef";
        assert_eq!(classify(s), "hex");
    }

    #[test]
    fn classifies_base64() {
        // plain "Hello, dbwarp-blueprint! base64-encoded sample" → b64
        let s = b"SGVsbG8sIGRid2FycC1zaGFwZSEgYmFzZTY0LWVuY29kZWQgc2FtcGxl";
        assert_eq!(classify(s), "base64");
    }

    #[test]
    fn falls_through_to_mixed() {
        let s = b"x4!@#$ jumbled \xff\xff\xff \xab\xcd random binary";
        assert_eq!(classify(s), "mixed");
    }

    #[test]
    fn empty_returns_empty() {
        assert_eq!(classify(b""), "");
    }
}
