use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const LANGUAGES: &[&str] = &["de", "fr", "es", "pl", "ja", "zh"];
const ROOT_ABOUT: &str = "Collect sanitized database or structured-file Blueprint metadata for DBWarp sizing, synthetic fixture generation, and migration planning.\n\nLive database modes read catalog/statistics metadata. Tier 2 compression measurement is opt-in with --measure-compression --yes; sampled bytes are encoded and compressed in memory, then discarded. Offline modes read local TOML, Parquet, Avro, or bundle files and do not connect to a database.";

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_dbwarp-blueprint")
}

fn scratch(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("i18n-{name}-{nonce}"));
    fs::create_dir_all(&path).unwrap();
    path
}

fn run(args: &[&str]) -> Output {
    Command::new(bin())
        .args(args)
        .env_remove("DBWARP_BLUEPRINT_LANG")
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env("LANG", "C")
        .output()
        .unwrap()
}

fn catalog(language: &str, kind: &str) -> Value {
    let source = match (kind, language) {
        ("messages", "de") => include_str!("../locales/messages.de.json"),
        ("messages", "fr") => include_str!("../locales/messages.fr.json"),
        ("messages", "es") => include_str!("../locales/messages.es.json"),
        ("messages", "pl") => include_str!("../locales/messages.pl.json"),
        ("messages", "ja") => include_str!("../locales/messages.ja.json"),
        ("messages", "zh") => include_str!("../locales/messages.zh.json"),
        ("ui", "de") => include_str!("../locales/ui.de.json"),
        ("ui", "fr") => include_str!("../locales/ui.fr.json"),
        ("ui", "es") => include_str!("../locales/ui.es.json"),
        ("ui", "pl") => include_str!("../locales/ui.pl.json"),
        ("ui", "ja") => include_str!("../locales/ui.ja.json"),
        ("ui", "zh") => include_str!("../locales/ui.zh.json"),
        _ => panic!("unsupported catalog {kind}/{language}"),
    };
    serde_json::from_str(source).unwrap()
}

fn option_names(help: &str) -> BTreeSet<String> {
    let pattern =
        regex::Regex::new(r"--[A-Za-z0-9][A-Za-z0-9-]*(?:/-[A-Za-z0-9-]+)?").expect("option regex");
    pattern
        .find_iter(help)
        .map(|matched| matched.as_str().to_string())
        .collect()
}

#[test]
fn every_language_localizes_complete_help_without_translating_options() {
    let english = run(&["--help"]);
    assert!(english.status.success());
    let english = String::from_utf8(english.stdout).unwrap();
    let expected_options = option_names(&english);
    assert!(
        expected_options.len() >= 45,
        "only found {expected_options:?}"
    );

    for language in LANGUAGES {
        let output = run(&["--lang", language, "--help"]);
        assert!(
            output.status.success(),
            "{language}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let help = String::from_utf8(output.stdout).unwrap();
        let ui = catalog(language, "ui");
        let translated_about = ui["help_phrases"][ROOT_ABOUT].as_str().unwrap();
        let translated_usage = ui["help_phrases"]["Usage:"].as_str().unwrap();
        assert!(
            help.contains(translated_about),
            "{language} root description missing"
        );
        assert!(
            help.contains(translated_usage),
            "{language} usage label missing"
        );
        assert!(
            !help.contains(ROOT_ABOUT),
            "{language} leaked canonical root description"
        );
        assert_eq!(
            option_names(&help),
            expected_options,
            "{language} changed option tokens"
        );
        for token in [
            "disable",
            "prefer",
            "require",
            "verify-ca",
            "verify-full",
            "balanced",
            "strict",
            "exact",
        ] {
            assert!(
                help.contains(token),
                "{language} lost canonical value {token}"
            );
        }
    }
}

#[test]
fn environment_locale_and_invalid_language_are_explicit() {
    let output = Command::new(bin())
        .arg("--help")
        .env("DBWARP_BLUEPRINT_LANG", "pl_PL.UTF-8")
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env("LANG", "C")
        .output()
        .unwrap();
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(help.contains(
        catalog("pl", "ui")["help_phrases"]["Usage:"]
            .as_str()
            .unwrap()
    ));

    let output = run(&["--lang", "xx", "--dry-run"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("DBP1011E"));
}

#[test]
fn canonical_language_environment_precedes_posix_locale() {
    let canonical = Command::new(bin())
        .arg("--help")
        .env("DBWARP_BLUEPRINT_LANG", "pl_PL.UTF-8")
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env("LANG", "de_DE.UTF-8")
        .output()
        .unwrap();
    assert!(canonical.status.success());
    let canonical_help = String::from_utf8(canonical.stdout).unwrap();
    assert!(canonical_help.contains(
        catalog("pl", "ui")["help_phrases"]["Usage:"]
            .as_str()
            .unwrap()
    ));
}

#[test]
fn localized_diagnostics_keep_codes_and_operational_tokens() {
    let dir = scratch("diagnostics");
    for language in LANGUAGES {
        let out = dir.join(format!("{language}.toml"));
        let output = run(&[
            "--lang",
            language,
            "--connect",
            "postgresql://app:secret@localhost/db",
            "--out",
            out.to_str().unwrap(),
            "--yes",
        ]);
        assert_eq!(output.status.code(), Some(1));
        let stderr = String::from_utf8_lossy(&output.stderr);
        let messages = catalog(language, "messages");
        let summary = messages["messages"]["DBP1001E"]["summary"]
            .as_str()
            .unwrap();
        assert!(stderr.contains("DBP1001E"), "{language}: {stderr}");
        assert!(stderr.contains(summary), "{language}: {stderr}");
        assert!(stderr.contains("--password-file"), "{language}: {stderr}");
        assert!(
            !stderr.contains("Password in connection URI was refused"),
            "{language} leaked English summary"
        );
    }
}

fn write_bundle_fixture(dir: &Path) -> PathBuf {
    let blueprint = r#"schema_version = 1
generated_at = "2026-08-03T00:00:00Z"
engine = "postgresql"
engine_version = "test"
source_kind = "synthetic"

[totals]
table_count = 1
row_count = 10
table_bytes = 160
index_bytes = 0

[tables.table-001]
rows = 10
table_bytes = 160
index_bytes = 0
schema = "schema-A"
has_clustered_index = false

[tables.table-001.cols.col-1]
ordinal = 1
type = "int"
nullable = false
len_avg = 8
len_p95 = 8
"#;
    fs::write(dir.join("source.blueprint.toml"), blueprint).unwrap();
    let bundle = r#"schema_version = 1
kind = "dbwarp-blueprint-bundle"
generated_at = "2026-08-03T00:00:00Z"

[bundle_totals]
source_count = 1
table_count = 1
row_count = 10
table_bytes = 160
index_bytes = 0

[sources.source_1]
kind = "database"
engine = "postgresql"
blueprint_path = "source.blueprint.toml"
tags = ["test"]
table_count = 1
row_count = 10
table_bytes = 160
index_bytes = 0
"#;
    let path = dir.join("bundle.toml");
    fs::write(&path, bundle).unwrap();
    path
}

#[test]
fn localization_does_not_change_emitted_blueprint_toml() {
    let dir = scratch("blueprint-invariance");
    let bundle = write_bundle_fixture(&dir);
    let mut outputs = Vec::new();
    for language in ["en", "de", "ja", "zh"] {
        let out = dir.join(format!("blueprint-{language}.toml"));
        let output = run(&[
            "--lang",
            language,
            "--bundle-extract",
            bundle.to_str().unwrap(),
            "--select",
            "source=source_1",
            "--out",
            out.to_str().unwrap(),
        ]);
        assert!(
            output.status.success(),
            "{language}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        outputs.push(fs::read(out).unwrap());
    }
    assert!(outputs.windows(2).all(|pair| pair[0] == pair[1]));
}

#[test]
fn generated_deck_uses_locale_but_preserves_identifiers() {
    let dir = scratch("deck");
    let deck = dir.join("blueprint-ja.pptx");
    let output = run(&[
        "--lang",
        "ja",
        "--from-toml",
        "samples/saas-medium.toml",
        "--deck",
        deck.to_str().unwrap(),
        "--deck-confidentiality",
        "confidential",
        "--yes",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let ui = catalog("ja", "ui");
    let fidelity_prefix = ui["text"]["status.fidelity"]
        .as_str()
        .unwrap()
        .split("{overall}")
        .next()
        .unwrap();
    assert!(stdout.contains(fidelity_prefix), "{stdout}");
    assert!(stdout.contains(
        ui["text"]["status.fidelity_qualification"]
            .as_str()
            .unwrap()
    ));
    assert!(stderr.contains("blueprint_fidelity_estimate:"));
    assert!(stderr.contains("qualification: evidence estimate, not source-truth accuracy"));
    let bytes = fs::read(deck).unwrap();
    let package = String::from_utf8_lossy(&bytes);
    assert!(package.contains(ui["text"]["deck.report"].as_str().unwrap()));
    assert!(package.contains(
        ui["text"]["deck.confidentiality.confidential"]
            .as_str()
            .unwrap()
    ));
    assert!(package.contains("lang=\"ja-JP\""));
    assert!(
        regex::Regex::new(r"table-[0-9]{3}")
            .unwrap()
            .is_match(&package),
        "deck lost canonical anonymized table identifiers"
    );
    assert!(!package.contains("Database Blueprint report"));
    assert!(!package.contains(">Confidential<"));
}
