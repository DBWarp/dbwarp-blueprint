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

/// RGB values shared by CLI presentation and the monitor TUI.
#[allow(dead_code)]
pub mod palette {
    pub const INK: (u8, u8, u8) = (0x0b, 0x12, 0x20);
    pub const PANEL: (u8, u8, u8) = (0x11, 0x1a, 0x2c);
    pub const EDGE: (u8, u8, u8) = (0x1a, 0x24, 0x38);
    pub const SELECTED: (u8, u8, u8) = (0x18, 0x2f, 0x3a);
    pub const TEXT: (u8, u8, u8) = (0xe7, 0xec, 0xf3);
    pub const MUTED: (u8, u8, u8) = (0x8e, 0xa0, 0xb8);
    pub const DIM: (u8, u8, u8) = (0x5f, 0x71, 0x88);
    pub const TEAL: (u8, u8, u8) = (0x2d, 0xd4, 0xbf);
    pub const AQUA: (u8, u8, u8) = (0x5e, 0xea, 0xd4);
    pub const BLUE: (u8, u8, u8) = (0x38, 0xbd, 0xf8);
    pub const GOOD: (u8, u8, u8) = (0x34, 0xd3, 0x99);
    pub const WARN: (u8, u8, u8) = (0xfb, 0xbf, 0x24);
    pub const BAD: (u8, u8, u8) = (0xf8, 0x71, 0x71);
    pub const PG: (u8, u8, u8) = BLUE;
    pub const MYSQL: (u8, u8, u8) = WARN;
    pub const TDS: (u8, u8, u8) = (0xa7, 0x8b, 0xfa);
    pub const COMPRESS: (u8, u8, u8) = TEAL;
    pub const WIRE: (u8, u8, u8) = AQUA;
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
