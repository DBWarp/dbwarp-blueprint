//! Embedded DBWarp terminal lockup.
//!
//! Default (`auto` / `ascii`) is the drawn slant lockup from
//! `assets/ascii/dbwarp-ascii.txt` and `dbwarp-ansi-glyph.txt`.
//! `half`, `blocks`, and `braille` remain the SVG-sampled rasters.

use crate::terminal_style::{self, OutputStream};
use std::sync::atomic::{AtomicBool, Ordering};

static COLUMNS_ENV_READ: AtomicBool = AtomicBool::new(false);

const TAG1: &str = "Global Data";
const SEP: &str = "·";
const TAG2: &str = "Local Speeds";
const HAND_ASCII: &str = include_str!("../assets/ascii/dbwarp-ascii.txt");
const HAND_ANSI: &str = include_str!("../assets/ascii/dbwarp-ansi-glyph.txt");
const HAND_WIDTH: usize = 56;

macro_rules! asset {
    ($w:literal, $d:literal, $ext:literal) => {
        include_str!(concat!(
            "../assets/ascii/lockup-dark-",
            $w,
            "-",
            $d,
            ".",
            $ext
        ))
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Lockup only (version).
    Lockup,
    /// Lockup plus the official tagline (help / --banner).
    Help,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Dialect {
    #[default]
    Auto,
    Half,
    Blocks,
    Ascii,
    Braille,
}

impl Dialect {
    pub fn parse_name(raw: &str) -> Option<Self> {
        match raw {
            "auto" => Some(Self::Auto),
            "half" => Some(Self::Half),
            "blocks" => Some(Self::Blocks),
            "ascii" => Some(Self::Ascii),
            "braille" => Some(Self::Braille),
            _ => None,
        }
    }
}

pub fn dialect_from_args(args: &[String]) -> Dialect {
    let mut iter = args.iter().skip(1);
    while let Some(argument) = iter.next() {
        if argument == "--" {
            break;
        }
        let value = if let Some(value) = argument.strip_prefix("--banner-mode=") {
            Some(value)
        } else if argument == "--banner-mode" {
            iter.next().map(String::as_str)
        } else {
            None
        };
        if let Some(value) = value.and_then(Dialect::parse_name) {
            return value;
        }
    }
    Dialect::Auto
}

pub fn render(kind: Kind, stream: OutputStream, dialect: Dialect) -> String {
    render_with(
        kind,
        terminal_style::enabled(stream),
        dialect,
        terminal_cols(),
    )
}

pub fn render_with(kind: Kind, color: bool, dialect: Dialect, cols: usize) -> String {
    let resolved = resolve_dialect(dialect, color);
    if matches!(resolved, Dialect::Auto | Dialect::Ascii) {
        return render_hand(kind, color, cols);
    }
    let width = width_bucket(cols);
    let lockup = asset_for(width, resolved, color);
    let mut out = lockup.trim_end().to_string();
    out.push('\n');
    if kind == Kind::Help {
        out.push('\n');
        let measure = asset_for(width, resolved, false);
        out.push_str(&tagline_line(visible_width(measure), color));
        out.push('\n');
    }
    out.push('\n');
    out
}

pub fn resolve_dialect(hint: Dialect, _color: bool) -> Dialect {
    if hint == Dialect::Auto {
        Dialect::Ascii
    } else {
        hint
    }
}

fn render_hand(kind: Kind, color: bool, cols: usize) -> String {
    let mut body = if color {
        HAND_ANSI.to_string()
    } else {
        HAND_ASCII.to_string()
    };
    if kind == Kind::Lockup {
        body = body
            .lines()
            .filter(|line| !visible_line(line).contains("Global Data"))
            .collect::<Vec<_>>()
            .join("\n");
    } else if !color && !visible_line(&body).contains("Global Data") {
        body = body.trim_end().to_string();
        body.push('\n');
        body.push('\n');
        body.push_str(&tagline_line(HAND_WIDTH, false));
    }
    body = body.trim_end().to_string();
    body.push('\n');
    body.push('\n');
    center_art(&body, cols)
}

fn visible_line(line: &str) -> String {
    let mut out = String::new();
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b && bytes.get(i + 1) == Some(&b'[') {
            i += 2;
            while i < bytes.len() {
                let b = bytes[i];
                i += 1;
                if (0x40..=0x7e).contains(&b) {
                    break;
                }
            }
            continue;
        }
        let ch = line[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn center_art(art: &str, cols: usize) -> String {
    let pad = cols.saturating_sub(HAND_WIDTH) / 2;
    if pad == 0 {
        return if art.ends_with('\n') {
            art.to_string()
        } else {
            format!("{art}\n")
        };
    }
    let prefix = " ".repeat(pad);
    let mut out = String::new();
    for line in art.lines() {
        if line.is_empty() {
            out.push('\n');
        } else {
            out.push_str(&prefix);
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn width_bucket(cols: usize) -> u16 {
    if cols < 68 {
        56
    } else if cols < 110 {
        80
    } else {
        120
    }
}

fn asset_for(width: u16, dialect: Dialect, color: bool) -> &'static str {
    let ext_ans = color;
    match (width, dialect, ext_ans) {
        (56, Dialect::Half, true) => asset!(56, "half", "ans"),
        (56, Dialect::Half, false) => asset!(56, "half", "txt"),
        (56, Dialect::Blocks, true) => asset!(56, "blocks", "ans"),
        (56, Dialect::Blocks, false) => asset!(56, "blocks", "txt"),
        (56, Dialect::Ascii, true) => asset!(56, "ascii", "ans"),
        (56, Dialect::Ascii, false) => asset!(56, "ascii", "txt"),
        (56, Dialect::Braille, true) => asset!(56, "braille", "ans"),
        (56, Dialect::Braille, false) => asset!(56, "braille", "txt"),
        (80, Dialect::Half, true) => asset!(80, "half", "ans"),
        (80, Dialect::Half, false) => asset!(80, "half", "txt"),
        (80, Dialect::Blocks, true) => asset!(80, "blocks", "ans"),
        (80, Dialect::Blocks, false) => asset!(80, "blocks", "txt"),
        (80, Dialect::Ascii, true) => asset!(80, "ascii", "ans"),
        (80, Dialect::Ascii, false) => asset!(80, "ascii", "txt"),
        (80, Dialect::Braille, true) => asset!(80, "braille", "ans"),
        (80, Dialect::Braille, false) => asset!(80, "braille", "txt"),
        (120, Dialect::Half, true) => asset!(120, "half", "ans"),
        (120, Dialect::Half, false) => asset!(120, "half", "txt"),
        (120, Dialect::Blocks, true) => asset!(120, "blocks", "ans"),
        (120, Dialect::Blocks, false) => asset!(120, "blocks", "txt"),
        (120, Dialect::Ascii, true) => asset!(120, "ascii", "ans"),
        (120, Dialect::Ascii, false) => asset!(120, "ascii", "txt"),
        (120, Dialect::Braille, true) => asset!(120, "braille", "ans"),
        (120, Dialect::Braille, false) => asset!(120, "braille", "txt"),
        (_, Dialect::Auto, _) => unreachable!("auto resolved before asset lookup"),
        (_, _, _) => asset!(80, "half", "txt"),
    }
}

fn visible_width(plain: &str) -> usize {
    plain
        .lines()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0)
}

fn tagline_line(width: usize, color: bool) -> String {
    let text = format!("{TAG1} {SEP} {TAG2}");
    let text_width = text.chars().count();
    let pad = width.saturating_sub(text_width) / 2;
    let pad_s = " ".repeat(pad);
    if !color {
        return format!("{pad_s}{text}");
    }
    format!(
        "{pad_s}\x1b[3m\x1b[38;2;45;212;191m{TAG1}\x1b[38;2;159;178;199m {SEP} \x1b[38;2;231;236;243m{TAG2}\x1b[0m"
    )
}

fn terminal_cols() -> usize {
    COLUMNS_ENV_READ.store(true, Ordering::Release);
    if let Ok(value) = std::env::var("COLUMNS") {
        if let Ok(n) = value.parse::<usize>() {
            return n.max(40);
        }
    }
    unix_stdout_cols().unwrap_or(80)
}

pub fn env_vars_read() -> Vec<&'static str> {
    COLUMNS_ENV_READ
        .load(Ordering::Acquire)
        .then_some("COLUMNS")
        .into_iter()
        .collect()
}

#[cfg(unix)]
fn unix_stdout_cols() -> Option<usize> {
    use std::io::IsTerminal;
    if !std::io::stdout().is_terminal() {
        return None;
    }
    #[repr(C)]
    struct WinSize {
        row: u16,
        col: u16,
        xpixel: u16,
        ypixel: u16,
    }
    #[cfg(any(target_os = "linux", target_os = "android"))]
    const TIOCGWINSZ: usize = 0x5413;
    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    const TIOCGWINSZ: usize = 0x4008_7468;
    #[cfg(not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    )))]
    const TIOCGWINSZ: usize = 0x5413;

    extern "C" {
        fn ioctl(fd: i32, request: usize, argp: *mut WinSize) -> i32;
    }
    use std::os::fd::AsRawFd;
    let mut size = WinSize {
        row: 0,
        col: 0,
        xpixel: 0,
        ypixel: 0,
    };
    let ok = unsafe { ioctl(std::io::stdout().as_raw_fd(), TIOCGWINSZ, &mut size) == 0 };
    if ok && size.col > 0 {
        Some(size.col as usize)
    } else {
        None
    }
}

#[cfg(not(unix))]
fn unix_stdout_cols() -> Option<usize> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_resolves_to_drawn_ascii() {
        assert_eq!(resolve_dialect(Dialect::Auto, false), Dialect::Ascii);
        assert_eq!(resolve_dialect(Dialect::Auto, true), Dialect::Ascii);
    }

    #[test]
    fn explicit_dialect_is_not_overridden() {
        assert_eq!(resolve_dialect(Dialect::Half, false), Dialect::Half);
        assert_eq!(resolve_dialect(Dialect::Ascii, true), Dialect::Ascii);
        assert_eq!(resolve_dialect(Dialect::Braille, false), Dialect::Braille);
    }

    #[test]
    fn half_plain_is_half_block_art() {
        let art = render_with(Kind::Lockup, false, Dialect::Half, 80);
        assert!(art.contains('\u{2580}'), "missing half-block:\n{art}");
        assert!(!art.contains('\u{1b}'));
    }

    #[test]
    fn blocks_plain_uses_shade_ramp() {
        let art = render_with(Kind::Lockup, false, Dialect::Blocks, 80);
        assert!(
            art.contains('░') || art.contains('█'),
            "missing blocks:\n{art}"
        );
        assert!(!art.contains('\u{1b}'));
    }

    #[test]
    fn ascii_plain_is_drawn_slant_lockup() {
        let art = render_with(Kind::Lockup, false, Dialect::Ascii, 80);
        assert!(art.contains("____"), "missing wordmark:\n{art}");
        assert!(art.contains("\\\\"), "missing chevrons:\n{art}");
        assert!(!art.contains('\u{2580}'));
        assert!(!art.contains('\u{1b}'));
        assert!(!art.contains("Global Data"));
    }

    #[test]
    fn ascii_help_includes_tagline() {
        let art = render_with(Kind::Help, false, Dialect::Auto, 80);
        assert!(art.contains("____"));
        assert!(art.contains("Global Data"));
        assert!(art.contains("Local Speeds"));
    }

    #[test]
    fn help_kind_appends_canonical_tagline() {
        let art = render_with(Kind::Help, false, Dialect::Blocks, 80);
        assert!(art.contains("Global Data"));
        assert!(art.contains("Local Speeds"));
        let lockup = render_with(Kind::Lockup, false, Dialect::Blocks, 80);
        assert!(!lockup.contains("Global Data"));
    }

    #[test]
    fn colour_half_uses_truecolour_csi() {
        let art = render_with(Kind::Help, true, Dialect::Half, 80);
        assert!(art.contains("\u{1b}[38;2;"));
        assert!(art.contains("\u{1b}[48;2;"));
        assert!(art.contains("45;212;191"));
    }

    #[test]
    fn dialect_from_args_reads_flag() {
        let args = ["x".into(), "--banner-mode".into(), "ascii".into()];
        assert_eq!(dialect_from_args(&args), Dialect::Ascii);
        let args = ["x".into(), "--banner-mode=braille".into()];
        assert_eq!(dialect_from_args(&args), Dialect::Braille);
        let args = ["x".into(), "--help".into()];
        assert_eq!(dialect_from_args(&args), Dialect::Auto);
    }

    #[test]
    fn narrow_terminal_selects_compact_lockup() {
        let compact = render_with(Kind::Lockup, false, Dialect::Half, 56);
        let standard = render_with(Kind::Lockup, false, Dialect::Half, 80);
        let compact_width = compact
            .lines()
            .map(|line| line.chars().count())
            .max()
            .unwrap();
        let standard_width = standard
            .lines()
            .map(|line| line.chars().count())
            .max()
            .unwrap();
        assert!(compact_width < standard_width);
        assert!(compact_width <= 56);
    }
}
