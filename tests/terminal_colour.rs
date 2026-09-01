use std::process::{Command, Output};
use std::{fs, path::Path};

const ESCAPE: &[u8] = b"\x1b[";

fn dbwarp_blueprint(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_dbwarp-blueprint"))
        .args(arguments)
        .env_remove("NO_COLOR")
        .env("TERM", "xterm-256color")
        .env("COLORTERM", "truecolor")
        .output()
        .expect("run dbwarp-blueprint")
}

fn contains_ansi(value: &[u8]) -> bool {
    value.windows(ESCAPE.len()).any(|window| window == ESCAPE)
}

fn strip_ansi(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
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
fn redirected_banner_is_plain_lockup_with_tagline() {
    let output = dbwarp_blueprint(&["--banner"]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!contains_ansi(&output.stdout));
    let text = String::from_utf8(output.stdout).expect("UTF-8 banner");
    assert!(text.contains("____"), "missing drawn wordmark:\n{text}");
    assert!(text.contains("\\\\"), "missing chevrons:\n{text}");
    assert!(text.contains("Global Data"), "missing tagline:\n{text}");
    assert!(text.contains("Local Speeds"), "missing tagline:\n{text}");
}

#[test]
fn banner_mode_half_plain_uses_half_blocks() {
    let output = dbwarp_blueprint(&["--banner-mode", "half", "--banner"]);
    assert!(output.status.success());
    assert!(!contains_ansi(&output.stdout));
    let text = String::from_utf8(output.stdout).expect("UTF-8 banner");
    assert!(text.contains('\u{2580}'), "missing half-block:\n{text}");
}

#[test]
fn banner_mode_ascii_plain_is_drawn_slant() {
    let output = dbwarp_blueprint(&["--banner-mode", "ascii", "--banner"]);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).expect("UTF-8 banner");
    assert!(text.contains("____"), "missing drawn wordmark:\n{text}");
    assert!(!text.contains('\u{2580}'));
}

#[test]
fn forced_colour_banner_uses_truecolour() {
    let output = dbwarp_blueprint(&["--color", "always", "--banner"]);
    assert!(output.status.success());
    assert!(contains_ansi(&output.stdout));
    let text = String::from_utf8(output.stdout).expect("UTF-8 banner");
    assert!(text.contains("38;2;"), "missing truecolour CSI:\n{text:?}");
    assert!(text.contains("Global Data"));
}

#[test]
fn help_includes_lockup_and_banner_flag() {
    let output = dbwarp_blueprint(&["--help"]);
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).expect("UTF-8 help");
    assert!(text.contains("____"), "help missing drawn lockup:\n{text}");
    assert!(text.contains("Global Data"));
    assert!(text.contains("--banner"));
    assert!(text.contains("--banner-mode"));
    assert!(text.contains("--connect"));
}

#[test]
fn short_help_is_focused_and_long_help_keeps_advanced_options() {
    let short = dbwarp_blueprint(&["-h"]);
    assert!(short.status.success());
    let short = String::from_utf8(short.stdout).expect("UTF-8 short help");
    for option in [
        "--connect",
        "--schema",
        "--out",
        "--measure-compression",
        "--artifact-detail",
        "--auth-mode",
        "--audit-log",
        "--dry-run",
        "--tls-mode",
    ] {
        assert!(
            short.contains(option),
            "short help omitted {option}:\n{short}"
        );
    }
    for advanced in [
        "--banner-mode",
        "--deck-confidentiality",
        "--bundle-pack",
        "--compression-workers",
        "--expect-server-principal",
        "--tls-skip-verify",
    ] {
        assert!(
            !short.contains(advanced),
            "short help exposed advanced option {advanced}:\n{short}"
        );
    }
    assert!(short.contains("see more with '--help'"));

    let long = dbwarp_blueprint(&["--help"]);
    assert!(long.status.success());
    let long = String::from_utf8(long.stdout).expect("UTF-8 long help");
    for advanced in [
        "--banner-mode",
        "--deck-confidentiality",
        "--bundle-pack",
        "--compression-workers",
        "--expect-server-principal",
        "--tls-skip-verify",
    ] {
        assert!(
            long.contains(advanced),
            "long help omitted advanced option {advanced}:\n{long}"
        );
    }
    assert!(long.contains("Examples:"));
}

#[test]
fn redirected_operational_output_is_not_prefixed_with_the_logo() {
    let output = dbwarp_blueprint(&["--connect", "postgresql://app@localhost/db", "--dry-run"]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains("____"));
    assert!(!String::from_utf8_lossy(&output.stderr).contains("____"));
}

#[test]
fn redirected_auto_help_remains_plain() {
    let output = dbwarp_blueprint(&["--help"]);
    assert!(output.status.success());
    assert!(!contains_ansi(&output.stdout));
}

#[test]
fn every_locale_styles_canonical_option_declarations() {
    for locale in ["en", "de", "fr", "es", "pl", "ja", "zh"] {
        let output = dbwarp_blueprint(&["--lang", locale, "--color", "always", "--help"]);
        assert!(output.status.success(), "{locale} help failed");
        assert!(
            contains_ansi(&output.stdout),
            "{locale} help contains no ANSI styling"
        );
        let rendered = String::from_utf8(output.stdout).expect("UTF-8 help");
        let plain = strip_ansi(&rendered);

        for syntax in ["--color <COLOR>", "--connect <URI>", "-h, --help"] {
            assert!(plain.contains(syntax), "{locale} help omitted {syntax}");
            let line = rendered
                .lines()
                .find(|line| strip_ansi(line).trim_start().starts_with(syntax))
                .unwrap_or_else(|| panic!("{locale} help has no declaration line for {syntax}"));
            assert!(
                line.contains("\u{1b}["),
                "{locale} help did not style declaration {syntax}: {line:?}"
            );
        }

        for token in ["--color", "auto", "always", "never", "postgresql://"] {
            assert!(
                plain.contains(token),
                "{locale} help changed or omitted canonical token {token}"
            );
        }
    }
}

#[test]
fn no_color_overrides_explicit_always() {
    let output = Command::new(env!("CARGO_BIN_EXE_dbwarp-blueprint"))
        .args(["--color", "always", "--help"])
        .env("NO_COLOR", "1")
        .env("TERM", "xterm-256color")
        .env("COLORTERM", "truecolor")
        .output()
        .expect("run dbwarp-blueprint");
    assert!(output.status.success());
    assert!(!contains_ansi(&output.stdout));
}

#[test]
fn forced_colour_styles_human_diagnostics() {
    let output = dbwarp_blueprint(&["--color", "always", "--not-a-real-option"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(contains_ansi(&output.stderr));
    assert!(String::from_utf8_lossy(&output.stderr).contains("DBP1011E"));
}

#[test]
fn forced_colour_never_contaminates_audit_artifacts() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("terminal-colour-audit");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).expect("create test directory");
    let audit = root.join("blueprint.audit.txt");
    let audit_arg = audit.to_str().expect("UTF-8 test path");

    let output = dbwarp_blueprint(&[
        "--color",
        "always",
        "--connect",
        "unsupported://db.example/app",
        "--audit-log",
        audit_arg,
        "--yes",
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(contains_ansi(&output.stderr));

    let audit = fs::read(audit).expect("audit should be written on failure");
    assert!(!contains_ansi(&audit), "ANSI escaped into audit artifact");
}
