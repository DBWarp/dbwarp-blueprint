//! Shared DBWarp colour identity and terminal capability detection.

use std::sync::atomic::{AtomicU8, Ordering};

static TERMINAL_ENV_READS: AtomicU8 = AtomicU8::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorCapability {
    TrueColor,
    Ansi256,
    Ansi16,
    Mono,
}

/// RGB values for this binary's CLI presentation roles.
pub mod palette {
    pub const TEAL: (u8, u8, u8) = (0x2d, 0xd4, 0xbf);
    pub const WARN: (u8, u8, u8) = (0xfb, 0xbf, 0x24);
    pub const BAD: (u8, u8, u8) = (0xf8, 0x71, 0x71);
    pub const WIRE: (u8, u8, u8) = (0x5e, 0xea, 0xd4);
}

pub fn detect_color_capability() -> ColorCapability {
    TERMINAL_ENV_READS.fetch_or(0b111, Ordering::AcqRel);
    detect_color_capability_from(
        std::env::var_os("NO_COLOR").is_some(),
        std::env::var("TERM").ok().as_deref(),
        std::env::var("COLORTERM").ok().as_deref(),
    )
}

pub fn env_vars_read() -> Vec<&'static str> {
    let bits = TERMINAL_ENV_READS.load(Ordering::Acquire);
    ["NO_COLOR", "TERM", "COLORTERM"]
        .into_iter()
        .enumerate()
        .filter_map(|(index, name)| (bits & (1 << index) != 0).then_some(name))
        .collect()
}

pub(crate) fn detect_color_capability_from(
    no_color: bool,
    term: Option<&str>,
    colorterm: Option<&str>,
) -> ColorCapability {
    if no_color || matches!(term, Some("") | Some("dumb")) {
        return ColorCapability::Mono;
    }
    if colorterm.is_some_and(|value| {
        let value = value.to_ascii_lowercase();
        value.contains("truecolor") || value.contains("24bit")
    }) {
        return ColorCapability::TrueColor;
    }
    match term {
        Some(value) if value.contains("256color") => ColorCapability::Ansi256,
        Some(_) => ColorCapability::Ansi16,
        None => ColorCapability::Ansi16,
    }
}
