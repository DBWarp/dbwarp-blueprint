use std::fs;
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_dbwarp-blueprint")
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("dbwarp-blueprint-{name}-{nonce}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn run(args: &[String], removed_env: &[&str]) -> Output {
    let mut command = Command::new(bin());
    command
        .args(args)
        .env_remove("DBWARP_BLUEPRINT_LANG")
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env("LANG", "C");
    for name in removed_env {
        command.env_remove(name);
    }
    let output = command.output().unwrap();
    assert!(!output.status.success(), "command unexpectedly succeeded");
    output
}

fn run_refs(args: &[&str]) -> String {
    let args = args
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    String::from_utf8_lossy(&run(&args, &[]).stderr).to_string()
}

fn first_message_code(stderr: &str) -> Option<&str> {
    let code = regex::Regex::new(r"\bDBP[0-9]{4}[EWI]\b").unwrap();
    code.find(stderr).map(|matched| matched.as_str())
}

fn assert_primary_code(name: &str, args: Vec<String>, removed_env: &[&str], expected: &str) {
    let output = run(&args, removed_env);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        first_message_code(&stderr),
        Some(expected),
        "{name} emitted the wrong primary code:\n{stderr}"
    );
    assert!(
        !stderr.starts_with("DBP0001E"),
        "{name} fell back to the generic operator boundary:\n{stderr}"
    );
}

#[test]
fn embedded_password_error_has_message_code_and_next_step() {
    let dir = temp_dir("embedded-password");
    let stderr = run_refs(&[
        "--connect",
        "postgresql://app:secret@localhost/db",
        "--out",
        dir.join("unused.toml").to_str().unwrap(),
        "--yes",
    ]);
    assert!(stderr.contains("DBP1001E"), "stderr was:\n{stderr}");
    assert!(
        stderr.contains("--password-file PATH"),
        "stderr was:\n{stderr}"
    );
}

#[test]
fn missing_anonymization_key_has_specific_message_code() {
    let dir = temp_dir("missing-anonymization-key");
    let missing = dir.join("missing.key");
    assert_primary_code(
        "missing anonymization key",
        vec![
            "--connect".into(),
            "postgresql://app@localhost/db".into(),
            "--anonymization-key-file".into(),
            missing.display().to_string(),
            "--yes".into(),
        ],
        &[],
        "DBP1607E",
    );
}

#[test]
fn slash_in_uri_password_never_reaches_stderr_or_audit() {
    let dir = temp_dir("slash-password-redaction");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let canary = format!("credential-canary-{nonce}/");
    for (engine, scheme, port) in [
        ("postgresql", "postgresql", 5432),
        ("mysql", "mysql", 3306),
        ("sqlserver", "sqlserver", 1433),
    ] {
        let audit = dir.join(format!("{engine}.audit.txt"));
        let uri = format!("{scheme}://app:{canary}@db.example:{port}/payments");
        let args = vec![
            "--connect".to_string(),
            uri,
            "--dry-run".to_string(),
            "--audit-log".to_string(),
            audit.display().to_string(),
        ];
        let output = run(&args, &[]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let audit_body = fs::read_to_string(&audit).expect("failure audit should be written");

        assert_eq!(first_message_code(&stderr), Some("DBP1001E"));
        assert!(
            !stderr.contains(&canary),
            "{engine} stderr disclosed the credential canary"
        );
        assert!(
            !audit_body.contains(&canary),
            "{engine} audit disclosed the credential canary"
        );
        assert!(
            audit_body.contains("outcome:             error: DBP1001E"),
            "{engine} audit did not retain the safe coded refusal"
        );
    }
}

#[test]
fn empty_batch_manifest_error_has_message_code() {
    let dir = temp_dir("empty-batch");
    let manifest = dir.join("batch.toml");
    fs::write(&manifest, "# intentionally empty\n").unwrap();
    let stderr = run_refs(&[
        "--batch-manifest",
        manifest.to_str().unwrap(),
        "--out-dir",
        dir.join("out").to_str().unwrap(),
        "--yes",
    ]);
    assert!(stderr.contains("DBP1103E"), "stderr was:\n{stderr}");
    assert!(stderr.contains("[[source]]"), "stderr was:\n{stderr}");
}

#[test]
fn command_line_parse_failure_writes_requested_audit() {
    let dir = temp_dir("parse-audit");
    let audit = dir.join("parse.audit.txt");
    let args = vec![
        "--definitely-unknown".to_string(),
        "--audit-log".to_string(),
        audit.display().to_string(),
    ];
    let output = run(&args, &[]);
    assert_eq!(output.status.code(), Some(2));
    let body = fs::read_to_string(&audit).expect("parse failure audit should be written");
    assert!(body.contains("mode:                command-line"));
    assert!(body.contains("outcome:             error: DBP1011E"));
    assert!(body.contains("no connection attempted"));
}

#[test]
fn unresolved_schema_boundary_is_specific_and_fail_closed() {
    // Live resolution itself is exercised against every engine/version by the
    // optional manual database matrix. Keep a cheap adversarial contract here
    // so the boundary cannot regress to DBP0001E or silently emit an empty,
    // ambiguously scoped Blueprint in ordinary CI.
    let source = include_str!("../src/schema_scope.rs");
    assert!(source.contains("DBP1420E"));
    assert!(source.contains("no Blueprint was written"));
    assert!(!source.contains("DBP0001E"));
}

#[test]
fn expected_server_principal_rejects_wrong_engine_and_unsafe_value() {
    let wrong_engine = run_refs(&[
        "--connect",
        "postgresql://app@localhost/db",
        "--expect-server-principal",
        "DOMAIN\\svc-blueprint",
        "--dry-run",
    ]);
    assert_eq!(first_message_code(&wrong_engine), Some("DBP1005E"));

    let unsafe_value = run_refs(&[
        "--connect",
        "sqlserver://localhost/inventory",
        "--expect-server-principal",
        "DOMAIN\\svc\nforged",
        "--dry-run",
    ]);
    assert_eq!(first_message_code(&unsafe_value), Some("DBP1606E"));
}

#[test]
fn all_failed_continue_on_error_batch_is_nonzero_but_keeps_diagnostics() {
    let dir = temp_dir("all-failed-batch");
    let manifest = dir.join("batch.toml");
    let out = dir.join("bundle");
    fs::write(
        &manifest,
        "[defaults]\ncontinue_on_error = true\n\n[[source]]\nid = \"missing\"\nkind = \"parquet\"\npath = \"missing.parquet\"\n",
    )
    .unwrap();
    let stderr = run_refs(&[
        "--batch-manifest",
        manifest.to_str().unwrap(),
        "--out-dir",
        out.to_str().unwrap(),
        "--yes",
    ]);
    assert!(stderr.contains("DBP1115E"), "stderr was:\n{stderr}");
    assert!(out.join("bundle.toml").is_file());
    assert!(out.join("errors.txt").is_file());
    let bundle = fs::read_to_string(out.join("bundle.toml")).unwrap();
    assert!(bundle.contains("partial = true"));
    assert!(bundle.contains("failed_source_count = 1"));
}

#[test]
fn bundle_selector_no_source_lists_available_sources() {
    let dir = temp_dir("bundle-no-source");
    let blueprint = r#"
schema_version = 1
generated_at = "2026-07-07T00:00:00Z"
engine = "postgresql"
engine_version = "test"
source_kind = "production"

[totals]
table_count = 1
row_count = 10
table_bytes = 160
index_bytes = 0

[tables.table-001]
rows = 10
table_bytes = 160
index_bytes = 0
schema = "public"
has_clustered_index = false

[tables.table-001.cols.col-1]
ordinal = 1
type = "int"
nullable = false
len_avg = 8
len_p95 = 8
"#;
    fs::write(dir.join("erp.blueprint.toml"), blueprint).unwrap();
    let bundle = r#"
schema_version = 1
kind = "dbwarp-blueprint-bundle"
generated_at = "2026-07-07T00:00:00Z"

[bundle_totals]
source_count = 1
table_count = 1
row_count = 10
table_bytes = 160
index_bytes = 0

[sources.erp_pg]
kind = "database"
engine = "postgresql"
blueprint_path = "erp.blueprint.toml"
tags = ["critical", "erp"]
table_count = 1
row_count = 10
table_bytes = 160
index_bytes = 0
"#;
    let bundle_path = dir.join("bundle.toml");
    fs::write(&bundle_path, bundle).unwrap();
    let stderr = run_refs(&[
        "--bundle-extract",
        bundle_path.to_str().unwrap(),
        "--select",
        "source=missing",
        "--out",
        dir.join("out.blueprint.toml").to_str().unwrap(),
    ]);
    assert!(stderr.contains("DBP1201E"), "stderr was:\n{stderr}");
    assert!(stderr.contains("erp_pg"), "stderr was:\n{stderr}");
    assert!(stderr.contains("--bundle-list"), "stderr was:\n{stderr}");
}

#[test]
fn predictable_failures_have_specific_primary_codes() {
    let dir = temp_dir("decision-boundaries");
    let missing = dir.join("missing");
    let malformed_batch = dir.join("malformed-batch.toml");
    fs::write(&malformed_batch, "not = [valid toml\n").unwrap();
    let empty_batch = dir.join("empty-batch.toml");
    fs::write(&empty_batch, "# no sources\n").unwrap();
    let invalid_id_batch = dir.join("invalid-id-batch.toml");
    fs::write(
        &invalid_id_batch,
        "[[source]]\nid = \"!!!\"\nkind = \"parquet\"\npath = \"missing.parquet\"\n",
    )
    .unwrap();
    let valid_batch = dir.join("valid-batch.toml");
    fs::write(
        &valid_batch,
        "[[source]]\nid = \"source-1\"\nkind = \"parquet\"\npath = \"missing.parquet\"\n",
    )
    .unwrap();
    let malformed_bundle = dir.join("malformed-bundle.toml");
    fs::write(&malformed_bundle, "schema_version = [broken\n").unwrap();
    let blocked_parent = dir.join("not-a-directory");
    fs::write(&blocked_parent, "file blocks directory creation\n").unwrap();

    let cases = vec![
        ("missing connection", vec![], vec![], "DBP1011E"),
        (
            "unknown CLI option",
            vec!["--definitely-unknown".to_string()],
            vec![],
            "DBP1011E",
        ),
        (
            "zero sample rows",
            vec![
                "--connect".into(),
                "postgresql://app@localhost/db".into(),
                "--sample-rows".into(),
                "0".into(),
                "--dry-run".into(),
            ],
            vec![],
            "DBP1011E",
        ),
        (
            "zero wall-time limit",
            vec![
                "--connect".into(),
                "postgresql://app@localhost/db".into(),
                "--max-wall-secs".into(),
                "0".into(),
                "--dry-run".into(),
            ],
            vec![],
            "DBP1011E",
        ),
        (
            "malformed PostgreSQL URI",
            vec![
                "--connect".into(),
                "postgresql://localhost".into(),
                "--dry-run".into(),
            ],
            vec![],
            "DBP1012E",
        ),
        (
            "malformed MySQL URI",
            vec![
                "--connect".into(),
                "mysql://localhost".into(),
                "--dry-run".into(),
            ],
            vec![],
            "DBP1012E",
        ),
        (
            "malformed SQL Server URI",
            vec![
                "--connect".into(),
                "sqlserver://localhost".into(),
                "--dry-run".into(),
            ],
            vec![],
            "DBP1012E",
        ),
        (
            "invalid source kind",
            vec![
                "--connect".into(),
                "postgresql://app@localhost/db".into(),
                "--source-kind".into(),
                "unknown-kind".into(),
                "--dry-run".into(),
            ],
            vec![],
            "DBP1013E",
        ),
        (
            "artifact graph without explicit consent",
            vec![
                "--connect".into(),
                "postgresql://app@localhost/db".into(),
                "--artifact-detail".into(),
                "graph".into(),
            ],
            vec![],
            "DBP1014E",
        ),
        (
            "missing password file",
            vec![
                "--connect".into(),
                "postgresql://app@localhost/db".into(),
                "--password-file".into(),
                missing.display().to_string(),
                "--yes".into(),
            ],
            vec![],
            "DBP1601E",
        ),
        (
            "invalid TLS mode",
            vec![
                "--connect".into(),
                "postgresql://app@localhost/db".into(),
                "--tls-mode".into(),
                "impossible".into(),
                "--dry-run".into(),
            ],
            vec![],
            "DBP1602E",
        ),
        (
            "TLS certificate without key",
            vec![
                "--connect".into(),
                "postgresql://app@localhost/db".into(),
                "--tls-cert".into(),
                missing.display().to_string(),
                "--dry-run".into(),
            ],
            vec![],
            "DBP1602E",
        ),
        (
            "SQL Server TLS client certificate is unavailable",
            vec![
                "--connect".into(),
                "sqlserver://app@localhost/db".into(),
                "--tls-cert".into(),
                missing.display().to_string(),
                "--tls-key".into(),
                missing.display().to_string(),
                "--dry-run".into(),
            ],
            vec![],
            "DBP1015E",
        ),
        (
            "SQL Server authentication conflict",
            vec![
                "--connect".into(),
                "sqlserver://app@localhost/db".into(),
                "--auth-mode".into(),
                "entra-token".into(),
                "--azure-token-env".into(),
                "DBWARP_BLUEPRINT_TEST_TOKEN".into(),
                "--password-env".into(),
                "DBWARP_BLUEPRINT_TEST_PASSWORD".into(),
                "--dry-run".into(),
            ],
            vec![
                "DBWARP_BLUEPRINT_TEST_TOKEN",
                "DBWARP_BLUEPRINT_TEST_PASSWORD",
            ],
            "DBP1604E",
        ),
        (
            "cloud token without an external token source",
            vec![
                "--connect".into(),
                "postgresql://app@localhost/db".into(),
                "--auth-mode".into(),
                "cloud-token".into(),
                "--tls-mode".into(),
                "verify-full".into(),
                "--dry-run".into(),
            ],
            vec![],
            "DBP1604E",
        ),
        (
            "cloud token without verified TLS",
            vec![
                "--connect".into(),
                "mysql://app@localhost/db".into(),
                "--auth-mode".into(),
                "cloud-token".into(),
                "--tls-mode".into(),
                "require".into(),
                "--password-env".into(),
                "DBWARP_BLUEPRINT_TEST_TOKEN".into(),
                "--dry-run".into(),
            ],
            vec!["DBWARP_BLUEPRINT_TEST_TOKEN"],
            "DBP1604E",
        ),
        (
            "cloud token mode used with SQL Server",
            vec![
                "--connect".into(),
                "sqlserver://app@localhost/db".into(),
                "--auth-mode".into(),
                "cloud-token".into(),
                "--password-env".into(),
                "DBWARP_BLUEPRINT_TEST_TOKEN".into(),
                "--dry-run".into(),
            ],
            vec!["DBWARP_BLUEPRINT_TEST_TOKEN"],
            "DBP1005E",
        ),
        (
            "missing batch manifest",
            vec![
                "--batch-manifest".into(),
                missing.display().to_string(),
                "--yes".into(),
            ],
            vec![],
            "DBP1101E",
        ),
        (
            "malformed batch manifest",
            vec![
                "--batch-manifest".into(),
                malformed_batch.display().to_string(),
                "--yes".into(),
            ],
            vec![],
            "DBP1102E",
        ),
        (
            "empty batch manifest",
            vec![
                "--batch-manifest".into(),
                empty_batch.display().to_string(),
                "--yes".into(),
            ],
            vec![],
            "DBP1103E",
        ),
        (
            "invalid batch source id",
            vec![
                "--batch-manifest".into(),
                invalid_id_batch.display().to_string(),
                "--out-dir".into(),
                dir.join("invalid-id-out").display().to_string(),
                "--yes".into(),
            ],
            vec![],
            "DBP1109E",
        ),
        (
            "batch output directory failure",
            vec![
                "--batch-manifest".into(),
                valid_batch.display().to_string(),
                "--out-dir".into(),
                blocked_parent.join("out").display().to_string(),
                "--yes".into(),
            ],
            vec![],
            "DBP1113E",
        ),
        (
            "missing bundle",
            vec!["--bundle-list".into(), missing.display().to_string()],
            vec![],
            "DBP1204E",
        ),
        (
            "malformed bundle",
            vec![
                "--bundle-list".into(),
                malformed_bundle.display().to_string(),
            ],
            vec![],
            "DBP1205E",
        ),
        (
            "missing Blueprint TOML",
            vec![
                "--from-toml".into(),
                missing.display().to_string(),
                "--deck".into(),
                dir.join("out.pptx").display().to_string(),
            ],
            vec![],
            "DBP1503E",
        ),
        (
            "missing Parquet input",
            vec!["--from-parquet".into(), missing.display().to_string()],
            vec![],
            "DBP1501E",
        ),
    ];

    for (name, args, removed_env, expected) in cases {
        assert_primary_code(name, args, &removed_env, expected);
    }
}

#[test]
fn dry_run_does_not_parse_or_record_tls_material() {
    let dir = temp_dir("dry-run-tls");
    let invalid_ca = dir.join("invalid-ca.pem");
    fs::write(&invalid_ca, "not a certificate\n").unwrap();
    let args = vec![
        "--connect".to_string(),
        "postgresql://app@localhost/db".to_string(),
        "--tls-mode".to_string(),
        "require".to_string(),
        "--tls-ca".to_string(),
        invalid_ca.display().to_string(),
        "--dry-run".to_string(),
    ];
    let mut command = Command::new(bin());
    let output = command
        .args(args)
        .env_remove("DBWARP_BLUEPRINT_LANG")
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env("LANG", "C")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "dry-run parsed TLS content:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("files_read_local:\n  (none)"),
        "stderr:\n{stderr}"
    );
}

#[test]
fn dry_run_describes_but_does_not_read_username_sources() {
    let dir = temp_dir("dry-run-user-sources");
    let missing_user_file = dir.join("missing-user.txt");
    let cases = [
        (
            vec![
                "--connect".to_string(),
                "postgresql://localhost/db".to_string(),
                "--user-env".to_string(),
                "DBWARP_BLUEPRINT_TEST_MISSING_USER".to_string(),
                "--dry-run".to_string(),
            ],
            "env:DBWARP_BLUEPRINT_TEST_MISSING_USER".to_string(),
        ),
        (
            vec![
                "--connect".to_string(),
                "postgresql://localhost/db".to_string(),
                "--user-file".to_string(),
                missing_user_file.display().to_string(),
                "--dry-run".to_string(),
            ],
            format!("file:{}", missing_user_file.display()),
        ),
    ];

    for (args, expected_source) in cases {
        let output = Command::new(bin())
            .args(args)
            .env_remove("DBWARP_BLUEPRINT_TEST_MISSING_USER")
            .env_remove("DBWARP_BLUEPRINT_LANG")
            .env_remove("LC_ALL")
            .env_remove("LC_MESSAGES")
            .env("LANG", "C")
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "dry-run read username source:\n{stderr}"
        );
        assert!(stderr.contains(&expected_source), "stderr:\n{stderr}");
        assert!(
            stderr.contains("files_read_local:\n  (none)"),
            "stderr:\n{stderr}"
        );
        let env_reads = stderr
            .split_once("env_vars_read:")
            .and_then(|(_, tail)| tail.split_once("trust_assertions:"))
            .map(|(section, _)| section)
            .expect("audit env_vars_read section");
        assert!(
            !env_reads.contains("DBWARP_BLUEPRINT_TEST_MISSING_USER"),
            "dry-run recorded a named username variable as read:\n{stderr}"
        );
    }
}
