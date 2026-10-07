//! Privacy-safe, bounded payload profiling for sampled binary values.
//!
//! The profiler recognizes only standard container signatures at the start of
//! an already-bounded database value. It never parses, decompresses, or retains
//! the value, and it emits one coarse style label without exposing the format.

pub const PRECOMPRESSED_STYLE: &str = "precompressed";

// Blueprint sampling-policy thresholds. A profile needs enough sampled
// material to be representative and a clear byte majority so one attachment
// cannot label a mixed binary column.
const MIN_PROFILE_SAMPLE_BYTES: u64 = 16 * 1024;
const MIN_PROFILE_VALUE_BYTES: usize = 4 * 1024;
const PROFILE_DOMINANCE_NUMERATOR: u64 = 3;
const PROFILE_DOMINANCE_DENOMINATOR: u64 = 4;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PayloadProfileAccumulator {
    sampled_binary_bytes: u64,
    precompressed_bytes: u64,
    precompressed_values: u64,
}

impl PayloadProfileAccumulator {
    /// Observe raw binary bytes at one value boundary.
    pub fn observe_raw_binary(&mut self, value: &[u8]) {
        self.observe(
            value.len() as u64,
            has_precompressed_container_signature(value),
        );
    }

    /// Observe PostgreSQL's canonical text representation of one `bytea`
    /// value. The live sampler uses the simple-query protocol, so a raw value
    /// is represented as `\x` followed by hexadecimal pairs.
    pub fn observe_postgres_bytea_text(&mut self, value: &[u8]) {
        let (sampled_binary_bytes, recognized) = postgres_bytea_text_precompressed_profile(value);
        self.observe(sampled_binary_bytes, recognized);
    }

    fn observe(&mut self, sampled_binary_bytes: u64, recognized: bool) {
        self.sampled_binary_bytes = self
            .sampled_binary_bytes
            .saturating_add(sampled_binary_bytes);
        if sampled_binary_bytes >= MIN_PROFILE_VALUE_BYTES as u64 && recognized {
            self.precompressed_bytes = self
                .precompressed_bytes
                .saturating_add(sampled_binary_bytes);
            self.precompressed_values = self.precompressed_values.saturating_add(1);
        }
    }

    /// Return the single privacy-safe serialized style label, or an empty
    /// label when evidence is insufficient or mixed.
    pub fn style(self) -> &'static str {
        if self.precompressed_values > 0
            && self.sampled_binary_bytes >= MIN_PROFILE_SAMPLE_BYTES
            && self
                .precompressed_bytes
                .saturating_mul(PROFILE_DOMINANCE_DENOMINATOR)
                >= self
                    .sampled_binary_bytes
                    .saturating_mul(PROFILE_DOMINANCE_NUMERATOR)
        {
            PRECOMPRESSED_STYLE
        } else {
            ""
        }
    }
}

pub fn is_precompressed_style(style: &str) -> bool {
    style.eq_ignore_ascii_case(PRECOMPRESSED_STYLE)
}

/// Recognize standard image, archive, and compressed-media container
/// signatures. The caller must supply the beginning of one value, not an
/// arbitrary stream window.
pub fn has_precompressed_container_signature(value: &[u8]) -> bool {
    value.starts_with(&[0xff, 0xd8, 0xff]) // JPEG
        || value.starts_with(b"\x89PNG\r\n\x1a\n")
        || value.starts_with(b"GIF87a")
        || value.starts_with(b"GIF89a")
        || (value.len() >= 12 && value.starts_with(b"RIFF") && &value[8..12] == b"WEBP")
        || is_compressed_iso_bmff_image(value)
        || value.starts_with(b"PK\x03\x04")
        || value.starts_with(b"PK\x05\x06")
        || value.starts_with(b"PK\x07\x08")
        || value.starts_with(&[0x1f, 0x8b]) // gzip
        || value.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]) // zstd
        || value.starts_with(&[0x04, 0x22, 0x4d, 0x18]) // LZ4 frame
        || value.starts_with(&[0xfd, b'7', b'z', b'X', b'Z', 0x00]) // xz
        || value.starts_with(b"BZh")
        || value.starts_with(&[0x37, 0x7a, 0xbc, 0xaf, 0x27, 0x1c]) // 7-Zip
        || value.starts_with(b"Rar!\x1a\x07\x00")
        || value.starts_with(b"Rar!\x1a\x07\x01\x00")
        || value.starts_with(b"OggS")
        || value.starts_with(b"fLaC")
        || value.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]) // WebM / Matroska
}

fn is_compressed_iso_bmff_image(value: &[u8]) -> bool {
    if value.len() < 12 || &value[4..8] != b"ftyp" {
        return false;
    }
    matches!(
        &value[8..12],
        b"avif" | b"avis" | b"heic" | b"heix" | b"hevc" | b"hevx" | b"mif1" | b"msf1"
    )
}

fn postgres_bytea_text_precompressed_profile(value: &[u8]) -> (u64, bool) {
    let Some(hex) = value.strip_prefix(b"\\x") else {
        // Keep this tolerant of a future binary-format query implementation.
        return (
            value.len() as u64,
            has_precompressed_container_signature(value),
        );
    };
    let mut prefix = [0_u8; 16];
    let decoded_len = (hex.len() / 2).min(prefix.len());
    for index in 0..decoded_len {
        let Some(high) = hex_digit(hex[index * 2]) else {
            return (0, false);
        };
        let Some(low) = hex_digit(hex[index * 2 + 1]) else {
            return (0, false);
        };
        prefix[index] = (high << 4) | low;
    }
    (
        (hex.len() / 2) as u64,
        has_precompressed_container_signature(&prefix[..decoded_len]),
    )
}

fn hex_digit(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(prefix: &[u8], len: usize) -> Vec<u8> {
        let mut value = vec![0x5a; len];
        value[..prefix.len()].copy_from_slice(prefix);
        value
    }

    #[test]
    fn recognizes_standard_container_signatures_without_exposing_the_family() {
        for prefix in [
            &b"\xff\xd8\xff\xe0"[..],
            &b"\x89PNG\r\n\x1a\n"[..],
            &b"GIF89a"[..],
            &b"PK\x03\x04"[..],
            &b"\x28\xb5\x2f\xfd"[..],
        ] {
            assert!(has_precompressed_container_signature(prefix));
        }
        let mut webp = value(b"RIFF", 16);
        webp[8..12].copy_from_slice(b"WEBP");
        assert!(has_precompressed_container_signature(&webp));

        let mut avif = value(&[0, 0, 0, 24], 16);
        avif[4..12].copy_from_slice(b"ftypavif");
        assert!(has_precompressed_container_signature(&avif));
    }

    #[test]
    fn rejects_textual_encodings_and_noncompressed_formats() {
        for prefix in [
            &b"/9j/4AAQSkZJRg"[..],
            &b"data:image/png;base64,"[..],
            &b"%PDF-"[..],
            &b"BM"[..],
            &b"II*\x00"[..],
        ] {
            assert!(!has_precompressed_container_signature(prefix));
        }
    }

    #[test]
    fn emits_only_a_dominant_material_profile() {
        let jpeg = value(b"\xff\xd8\xff\xe0", 16 * 1024);
        let mut dominant = PayloadProfileAccumulator::default();
        dominant.observe_raw_binary(&jpeg);
        dominant.observe_raw_binary(&vec![b'x'; 5 * 1024]);
        assert_eq!(dominant.style(), PRECOMPRESSED_STYLE);

        let mut mixed = PayloadProfileAccumulator::default();
        mixed.observe_raw_binary(&jpeg);
        mixed.observe_raw_binary(&vec![b'x'; 6 * 1024]);
        assert_eq!(mixed.style(), "");

        let mut too_small = PayloadProfileAccumulator::default();
        too_small.observe_raw_binary(&value(b"\xff\xd8\xff\xe0", 8 * 1024));
        assert_eq!(too_small.style(), "");
    }

    #[test]
    fn recognizes_postgres_bytea_hex_without_serializing_a_signature() {
        let raw = value(b"\x89PNG\r\n\x1a\n", 8 * 1024);
        let mut encoded = Vec::with_capacity(raw.len() * 2 + 2);
        encoded.extend_from_slice(b"\\x");
        for byte in raw {
            const HEX: &[u8; 16] = b"0123456789abcdef";
            encoded.push(HEX[(byte >> 4) as usize]);
            encoded.push(HEX[(byte & 0x0f) as usize]);
        }
        let mut profile = PayloadProfileAccumulator::default();
        profile.observe_postgres_bytea_text(&encoded);
        assert_eq!(profile.style(), "");
        profile.observe_postgres_bytea_text(&encoded);
        assert_eq!(profile.style(), PRECOMPRESSED_STYLE);
    }
}
