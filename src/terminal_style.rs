//! Terminal presentation policy and the shared DBWarp colour palette.
//!
//! This module is deliberately presentation-only. Structured logs, JSON,
//! evidence files, telemetry, and protocol payloads must never pass through it.

use std::io::{self, IsTerminal, Write};
use std::sync::atomic::{AtomicU8, Ordering};

use anstyle::{Ansi256Color, AnsiColor, Color, RgbColor, Style};

#[cfg(test)]
use crate::terminal_palette::detect_color_capability_from;
use crate::terminal_palette::{detect_color_capability, palette, ColorCapability};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorMode {
    #[default]
    Auto,
    Always,
    Never,
}

impl ColorMode {
    const fn as_u8(self) -> u8 {
        match self {
            Self::Auto => 0,
            Self::Always => 1,
            Self::Never => 2,
        }
    }

    const fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Always,
            2 => Self::Never,
            _ => Self::Auto,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputStream {
    Stdout,
    Stderr,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Brand,
    Accent,
    Muted,
    Success,
    Warning,
    Error,
    Postgres,
    MySql,
    SqlServer,
    Compression,
    Wire,
}

static COLOR_MODE: AtomicU8 = AtomicU8::new(ColorMode::Auto.as_u8());

pub fn configure(mode: ColorMode) {
    COLOR_MODE.store(mode.as_u8(), Ordering::Relaxed);
}

pub fn configured_mode() -> ColorMode {
    ColorMode::from_u8(COLOR_MODE.load(Ordering::Relaxed))
}

/// Read the colour override before Clap renders help or argument errors.
pub fn mode_from_args(args: &[String]) -> ColorMode {
    let mut iter = args.iter().skip(1);
    while let Some(argument) = iter.next() {
        if argument == "--" {
            break;
        }
        let value = if let Some(value) = argument.strip_prefix("--color=") {
            Some(value)
        } else if argument == "--color" {
            iter.next().map(String::as_str)
        } else {
            None
        };
        match value {
            Some("always") => return ColorMode::Always,
            Some("never") => return ColorMode::Never,
            Some("auto") => return ColorMode::Auto,
            _ => {}
        }
    }
    ColorMode::Auto
}

pub fn is_terminal(stream: OutputStream) -> bool {
    match stream {
        OutputStream::Stdout => io::stdout().is_terminal(),
        OutputStream::Stderr => io::stderr().is_terminal(),
    }
}

fn color_enabled_from(mode: ColorMode, is_terminal: bool, capability: ColorCapability) -> bool {
    capability != ColorCapability::Mono
        && match mode {
            ColorMode::Auto => is_terminal,
            ColorMode::Always => true,
            ColorMode::Never => false,
        }
}

pub fn enabled(stream: OutputStream) -> bool {
    color_enabled_from(
        configured_mode(),
        is_terminal(stream),
        detect_color_capability(),
    )
}

fn role_rgb(role: Role) -> (u8, u8, u8) {
    match role {
        Role::Brand | Role::Accent => palette::TEAL,
        Role::Muted => palette::MUTED,
        Role::Success => palette::GOOD,
        Role::Warning => palette::WARN,
        Role::Error => palette::BAD,
        Role::Postgres => palette::PG,
        Role::MySql => palette::MYSQL,
        Role::SqlServer => palette::TDS,
        Role::Compression => palette::COMPRESS,
        Role::Wire => palette::WIRE,
    }
}

fn role_ansi256(role: Role) -> u8 {
    match role {
        Role::Brand | Role::Accent | Role::Compression => 43,
        Role::Muted => 103,
        Role::Success => 42,
        Role::Warning | Role::MySql => 220,
        Role::Error => 203,
        Role::Postgres => 45,
        Role::SqlServer => 141,
        Role::Wire => 87,
    }
}

fn role_ansi16(role: Role) -> AnsiColor {
    match role {
        Role::Brand | Role::Accent | Role::Compression | Role::Wire => AnsiColor::BrightCyan,
        Role::Muted => AnsiColor::BrightBlack,
        Role::Success => AnsiColor::BrightGreen,
        Role::Warning | Role::MySql => AnsiColor::BrightYellow,
        Role::Error => AnsiColor::BrightRed,
        Role::Postgres => AnsiColor::BrightBlue,
        Role::SqlServer => AnsiColor::BrightMagenta,
    }
}

fn style(role: Role, capability: ColorCapability, bold: bool) -> Style {
    let color = match capability {
        ColorCapability::TrueColor => {
            let (red, green, blue) = role_rgb(role);
            Color::Rgb(RgbColor(red, green, blue))
        }
        ColorCapability::Ansi256 => Color::Ansi256(Ansi256Color(role_ansi256(role))),
        ColorCapability::Ansi16 | ColorCapability::Mono => Color::Ansi(role_ansi16(role)),
    };
    let style = Style::new().fg_color(Some(color));
    if bold {
        style.bold()
    } else {
        style
    }
}

fn paint_with_capability(
    role: Role,
    text: &str,
    capability: ColorCapability,
    bold: bool,
) -> String {
    let style = style(role, capability, bold);
    format!("{style}{text}{style:#}")
}

fn style_help_line(line: &str, capability: ColorCapability) -> String {
    let newline = if line.ends_with('\n') { "\n" } else { "" };
    let content = line.strip_suffix('\n').unwrap_or(line);
    let trimmed = content.trim_start();
    let indent_len = content.len() - trimmed.len();
    let indent = &content[..indent_len];

    if indent_len == 0 && (trimmed.ends_with(':') || trimmed.ends_with('：')) {
        return format!(
            "{}{newline}",
            paint_with_capability(Role::Brand, content, capability, true)
        );
    }
    if indent_len == 0 && trimmed.starts_with("dbwarp") {
        let brand_len = trimmed.split_whitespace().next().unwrap_or("dbwarp").len();
        return format!(
            "{}{}{newline}",
            paint_with_capability(Role::Brand, &content[..brand_len], capability, true),
            &content[brand_len..]
        );
    }
    if indent_len == 0 {
        if let Some((colon, delimiter_len)) = content
            .find(':')
            .map(|index| (index, ':'.len_utf8()))
            .or_else(|| content.find('：').map(|index| (index, '：'.len_utf8())))
        {
            let label_end = colon + delimiter_len;
            if content[label_end..].contains("dbwarp") {
                let label = &content[..label_end];
                let remainder = &content[label_end..];
                return format!(
                    "{}{}{newline}",
                    paint_with_capability(Role::Brand, label, capability, true),
                    remainder
                );
            }
        }
    }
    if trimmed.starts_with('-') {
        let syntax_end = trimmed.find("  ").unwrap_or(trimmed.len());
        let (syntax, description) = trimmed.split_at(syntax_end);
        return format!(
            "{indent}{}{}{newline}",
            paint_with_capability(Role::Wire, syntax, capability, true),
            description
        );
    }

    let first = trimmed.split_whitespace().next().unwrap_or_default();
    let command = matches!(
        first,
        "bulk" | "archive" | "file" | "monitor" | "support-bundle" | "explain" | "locales" | "help"
    );
    if (indent_len > 0 && command) || first.starts_with("dbwarp") || first.starts_with("scripts/") {
        return format!(
            "{indent}{}{}{newline}",
            paint_with_capability(Role::Accent, first, capability, true),
            &trimmed[first.len()..]
        );
    }
    format!("{content}{newline}")
}

fn render_help_with_capability(plain: &str, capability: ColorCapability) -> String {
    plain
        .split_inclusive('\n')
        .map(|line| style_help_line(line, capability))
        .collect()
}

/// Apply colour only after Clap and i18n have produced their canonical text.
pub fn render_help(plain: &str, stream: OutputStream) -> String {
    if !enabled(stream) {
        return plain.to_string();
    }
    render_help_with_capability(plain, detect_color_capability())
}

fn diagnostic_role(code: &str) -> Role {
    match code.chars().last() {
        Some('E') => Role::Error,
        Some('W') => Role::Warning,
        _ => Role::Brand,
    }
}

fn is_dbwarp_message_code(code: &str) -> bool {
    let code = code.trim_matches(['[', ']']);
    let bytes = code.as_bytes();
    bytes.len() == 8
        && matches!(&bytes[..3], b"DBW" | b"DBP" | b"DWE")
        && bytes[3..7].iter().all(u8::is_ascii_digit)
        && matches!(bytes[7], b'E' | b'W' | b'I')
}

fn render_diagnostic_with_capability(plain: &str, capability: ColorCapability) -> String {
    let Some(close) = plain.find(']') else {
        return plain.to_string();
    };
    let prefix = &plain[..=close];
    let code = prefix.trim_matches(['[', ']']);
    if !is_dbwarp_message_code(code) {
        return plain.to_string();
    }
    format!(
        "{}{}",
        paint_with_capability(diagnostic_role(code), prefix, capability, true),
        &plain[close + 1..]
    )
}

pub fn render_status(plain: &str, stream: OutputStream) -> String {
    if !enabled(stream) {
        return plain.to_string();
    }
    render_status_with_capability(plain, detect_color_capability())
}

fn render_status_with_capability(plain: &str, capability: ColorCapability) -> String {
    if plain.starts_with('[') {
        return render_diagnostic_with_capability(plain, capability);
    }
    let trimmed = plain.trim_start();
    let indent = &plain[..plain.len() - trimmed.len()];
    let first = trimmed.split_whitespace().next().unwrap_or_default();
    if is_dbwarp_message_code(first) {
        format!(
            "{indent}{}{}",
            paint_with_capability(diagnostic_role(first), first, capability, true),
            &trimmed[first.len()..]
        )
    } else {
        plain.to_string()
    }
}

fn choice(stream: OutputStream) -> anstream::ColorChoice {
    if !enabled(stream) {
        anstream::ColorChoice::Never
    } else if is_terminal(stream) {
        anstream::ColorChoice::Always
    } else {
        anstream::ColorChoice::AlwaysAnsi
    }
}

pub fn write_stdout(text: &str) -> io::Result<()> {
    let mut output = anstream::AutoStream::new(io::stdout(), choice(OutputStream::Stdout));
    output.write_all(text.as_bytes())
}

pub fn write_stderr(text: &str) -> io::Result<()> {
    let mut output = anstream::AutoStream::new(io::stderr(), choice(OutputStream::Stderr));
    output.write_all(text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strip_ansi(value: &str) -> String {
        let mut output = String::new();
        let bytes = value.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] == 0x1b && bytes.get(index + 1) == Some(&b'[') {
                index += 2;
                while index < bytes.len() {
                    let byte = bytes[index];
                    index += 1;
                    if (0x40..=0x7e).contains(&byte) {
                        break;
                    }
                }
            } else {
                let character = value[index..].chars().next().expect("valid UTF-8");
                output.push(character);
                index += character.len_utf8();
            }
        }
        output
    }

    #[test]
    fn auto_requires_an_interactive_capable_stream() {
        assert!(color_enabled_from(
            ColorMode::Auto,
            true,
            ColorCapability::TrueColor
        ));
        assert!(!color_enabled_from(
            ColorMode::Auto,
            false,
            ColorCapability::TrueColor
        ));
        assert!(!color_enabled_from(
            ColorMode::Always,
            true,
            ColorCapability::Mono
        ));
        assert!(!color_enabled_from(
            ColorMode::Never,
            true,
            ColorCapability::TrueColor
        ));
    }

    #[test]
    fn no_color_and_dumb_term_force_monochrome() {
        assert_eq!(
            detect_color_capability_from(true, Some("xterm-256color"), Some("truecolor")),
            ColorCapability::Mono
        );
        assert_eq!(
            detect_color_capability_from(false, Some("dumb"), Some("truecolor")),
            ColorCapability::Mono
        );
    }

    #[test]
    fn detects_truecolor_and_256_color_terminals() {
        assert_eq!(
            detect_color_capability_from(false, Some("xterm-256color"), Some("truecolor")),
            ColorCapability::TrueColor
        );
        assert_eq!(
            detect_color_capability_from(false, Some("xterm-256color"), None),
            ColorCapability::Ansi256
        );
    }

    #[test]
    fn early_argument_scan_accepts_both_color_forms() {
        assert_eq!(
            mode_from_args(&["dbwarp".into(), "--color=always".into(), "--help".into()]),
            ColorMode::Always
        );
        assert_eq!(
            mode_from_args(&[
                "dbwarp".into(),
                "--color".into(),
                "never".into(),
                "--help".into()
            ]),
            ColorMode::Never
        );
        assert_eq!(
            mode_from_args(&[
                "dbwarp".into(),
                "bulk".into(),
                "--".into(),
                "--color=always".into()
            ]),
            ColorMode::Auto
        );
    }

    #[test]
    fn styled_help_preserves_every_plain_character() {
        let plain = "dbwarp — proxy\n\nUsage: dbwarp [OPTIONS]\n\nOptions:\n  --listen <ADDR>  Listen address\n";
        let styled = render_help_with_capability(plain, ColorCapability::TrueColor);
        assert!(styled.contains("\u{1b}["));
        assert_eq!(strip_ansi(&styled), plain);
    }

    #[test]
    fn diagnostic_severity_is_styled_without_changing_text() {
        let plain = "[DBW1001E] transfer failed";
        let styled = render_diagnostic_with_capability(plain, ColorCapability::Ansi16);
        assert!(styled.contains("\u{1b}["));
        assert_eq!(strip_ansi(&styled), plain);
    }

    #[test]
    fn indented_status_codes_keep_their_alignment() {
        let plain = "  DBW1002W warning";
        let styled = render_status_with_capability(plain, ColorCapability::Ansi16);
        assert_eq!(strip_ansi(&styled), plain);
    }

    #[test]
    fn product_family_message_codes_are_styled() {
        for code in ["DBW1001E", "DBP1001E", "DWE1001E"] {
            let plain = format!("{code} operation failed");
            let styled = render_status_with_capability(&plain, ColorCapability::TrueColor);
            assert!(styled.contains("\u{1b}["), "{code} was not styled");
            assert_eq!(strip_ansi(&styled), plain);
        }
    }
}
