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
    // A cross-compiled Windows test binary must not retain the Unix build
    // host's CARGO_TARGET_TMPDIR as its runtime scratch directory.
    #[cfg(windows)]
    let test_root = std::env::temp_dir();
    #[cfg(not(windows))]
    let test_root = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let dir = test_root.join(format!("dbwarp-blueprint-{name}-{nonce}"));
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
#[test]
fn pg_partial_sampling_warning_cannot_accept_source_identifiers_or_driver_errors() {
    let source = include_str!("../src/engine_pg_sampling.rs");
    let warning = source
        .split("fn record_pg_partial_sample_warning(")
        .nth(1)
        .unwrap()
        .split("#[cfg(test)]")
        .next()
        .unwrap();
    assert!(warning.contains("audit.record_warning(\"DBP1407W\", detail)"));
    assert!(!warning.contains("qname"));
    assert!(!warning.contains("fallback_error"));
    assert!(!warning.contains("schema_name"));
}

#[test]
fn sqlserver_row_security_visibility_warning_is_stable_and_identifier_safe() {
    let source = include_str!("../src/engine_mssql.rs");
    let warning = source
        .split("fn warn_security_policy_visibility_unavailable(")
        .nth(1)
        .unwrap()
        .split("fn mssql_row_scope_intrinsically_incomplete(")
        .next()
        .unwrap();
    assert!(warning.contains("DBP1424W"));
    assert!(warning.contains("table_id"));
    assert!(!warning.contains("schema_name"));
    assert!(!warning.contains("table_name"));
    assert!(include_str!("../docs/MESSAGES.md").contains("`DBP1424W`"));
}

#[test]
fn sqlserver_active_row_security_warning_is_stable_and_identifier_safe() {
    let source = include_str!("../src/engine_mssql.rs");
    let warning = source
        .split("fn warn_active_security_filter(")
        .nth(1)
        .unwrap()
        .split("fn mssql_row_scope_intrinsically_incomplete(")
        .next()
        .unwrap();
    assert!(warning.contains("DBP1425W"));
    assert!(warning.contains("table_id"));
    assert!(!warning.contains("schema_name"));
    assert!(!warning.contains("table_name"));
    assert!(include_str!("../docs/MESSAGES.md").contains("`DBP1425W`"));
}

#[test]
fn oracle_preview_requires_explicit_acknowledgement_before_local_inputs() {
    assert_primary_code(
        "Oracle preview acknowledgement",
        vec![
            "--connect".into(),
            "oracle://db.example:1521/APPPDB".into(),
            "--dry-run".into(),
        ],
        &[],
        "DBP1426E",
    );
    assert_primary_code(
        "Oracle offline preview acknowledgement",
        vec![
            "--from-oracle-basic".into(),
            "must-not-be-opened.capture".into(),
        ],
        &[],
        "DBP1426E",
    );
    assert_primary_code(
        "Oracle DBA script preview acknowledgement",
        vec![
            "--oracle-basic-script-out".into(),
            "must-not-be-written.sql".into(),
            "--oracle-basic-script-family".into(),
            "21c".into(),
        ],
        &[],
        "DBP1426E",
    );
}

#[test]
fn oracle_dba_script_renderer_rejects_irrelevant_blueprint_controls() {
    let dir = temp_dir("oracle-script-irrelevant-options");
    let output_path = dir.join("must-not-be-written.sql");
    let output = run(
        &[
            "--oracle-basic-script-out".into(),
            output_path.display().to_string(),
            "--oracle-basic-script-family".into(),
            "21c".into(),
            "--acknowledge-oracle-preview".into(),
            "--generated-at".into(),
            "2026-09-28T00:00:00Z".into(),
        ],
        &[],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(first_message_code(&stderr), Some("DBP1426E"));
    assert!(stderr.contains("do not apply to Oracle DBA script rendering"));
    assert!(!output_path.exists());
}

#[test]
fn oracle_live_preview_uses_plain_tcp_and_rejects_any_explicit_tls_mode() {
    for mode in ["disable", "verify-full"] {
        let output = run(
            &[
                "--connect".into(),
                "oracle://db.example:1521/APPPDB".into(),
                "--acknowledge-oracle-preview".into(),
                "--tls-mode".into(),
                mode.into(),
                "--dry-run".into(),
            ],
            &[],
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(first_message_code(&stderr), Some("DBP1426E"));
        assert!(stderr.contains("--tls-mode is not available"), "{stderr}");
    }
}

#[test]
fn oracle_dry_run_preflight_discloses_plain_transport_and_credential_scope() {
    #[cfg(windows)]
    let (sqlplus, network_dir) = (r"C:\not-opened\sqlplus.exe", r"C:\not-opened\network");
    #[cfg(not(windows))]
    let (sqlplus, network_dir) = ("/not-opened/sqlplus", "/not-opened/network");
    let output = Command::new(bin())
        .args([
            "--connect",
            "oracle://db.example:1521/APPPDB",
            "--acknowledge-oracle-preview",
            "--oracle-sqlplus",
            sqlplus,
            "--oracle-network-config-dir",
            network_dir,
            "--user",
            "APP",
            "--schema",
            "APP",
            "--dry-run",
        ])
        .env_remove("DBWARP_BLUEPRINT_LANG")
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env("LANG", "C")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    for expected in [
        "oracle://db.example:1521/APPPDB",
        "db.example:1521",
        "APP",
        "disable (plain TCP)",
        "password",
    ] {
        assert!(stderr.contains(expected), "missing {expected:?}:\n{stderr}");
    }
}

#[test]
fn oracle_uri_userinfo_is_refused_without_disclosing_it() {
    let canary = "oracle-user-canary";
    let output = run(
        &[
            "--connect".into(),
            format!("oracle://{canary}@db.example:1521/APPPDB"),
            "--acknowledge-oracle-preview".into(),
            "--dry-run".into(),
        ],
        &[],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(first_message_code(&stderr), Some("DBP1001E"));
    assert!(!stderr.contains(canary), "stderr disclosed Oracle userinfo");
}

#[test]
fn malformed_oracle_offline_stream_has_a_specific_identifier_safe_error() {
    let dir = temp_dir("oracle-offline-malformed");
    let stream = dir.join("capture.json");
    let audit = dir.join("capture.audit.txt");
    let canary = "native-owner-canary-must-not-escape";
    fs::write(
        &stream,
        format!(
            "{{\"contract\":\"dbwarp-blueprint-oracle-basic-capture/v1\",\"stream_version\":\"{canary}\"}}\n"
        ),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&stream, fs::Permissions::from_mode(0o600)).unwrap();
    }
    let output = run(
        &[
            "--from-oracle-basic".into(),
            stream.display().to_string(),
            "--acknowledge-oracle-preview".into(),
            "--audit-log".into(),
            audit.display().to_string(),
        ],
        &[],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    let audit_body = fs::read_to_string(audit).unwrap();
    assert_eq!(first_message_code(&stderr), Some("DBP1505E"));
    assert!(!stderr.contains("DBP1429W"));
    assert!(!stderr.contains(canary));
    assert!(!audit_body.contains(canary));
    let operator_message = stderr
        .split("=== dbwarp-blueprint audit ===")
        .next()
        .unwrap();
    assert_eq!(operator_message.matches("DBP1505E").count(), 1, "{stderr}");
}

#[cfg(unix)]
#[test]
fn oracle_offline_stream_refuses_group_or_other_read_access() {
    use std::os::unix::fs::PermissionsExt;

    let dir = temp_dir("oracle-offline-permissions");
    let stream = dir.join("capture.json");
    fs::write(&stream, b"{}\n").unwrap();
    fs::set_permissions(&stream, fs::Permissions::from_mode(0o644)).unwrap();
    let output = run(
        &[
            "--from-oracle-basic".into(),
            stream.display().to_string(),
            "--acknowledge-oracle-preview".into(),
        ],
        &[],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(first_message_code(&stderr), Some("DBP1505E"));
    assert!(stderr.contains("mode 0600"));
}

#[test]
fn oracle_offline_stream_cannot_overwrite_its_own_input() {
    let dir = temp_dir("oracle-offline-self-overwrite");
    let stream = dir.join("capture.json");
    fs::write(&stream, b"{}\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&stream, fs::Permissions::from_mode(0o600)).unwrap();
    }
    let output = run(
        &[
            "--from-oracle-basic".into(),
            stream.display().to_string(),
            "--acknowledge-oracle-preview".into(),
            "--out".into(),
            stream.display().to_string(),
        ],
        &[],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(first_message_code(&stderr), Some("DBP1505E"));
    assert!(stderr.contains("must not share --out"));
    assert_eq!(fs::read(&stream).unwrap(), b"{}\n");
}

#[test]
fn oracle_preview_flags_are_not_silently_ignored_by_other_engines() {
    let output = run(
        &[
            "--connect".into(),
            "postgresql://localhost/example".into(),
            "--acknowledge-oracle-preview".into(),
            "--dry-run".into(),
        ],
        &[],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(first_message_code(&stderr), Some("DBP1011E"));
    assert!(!stderr.contains("connection refused"));

    let offline = run(
        &[
            "--from-toml".into(),
            "must-not-be-opened.toml".into(),
            "--deck".into(),
            "must-not-be-written.pptx".into(),
            "--acknowledge-oracle-preview".into(),
        ],
        &[],
    );
    let offline_stderr = String::from_utf8_lossy(&offline.stderr);
    assert_eq!(first_message_code(&offline_stderr), Some("DBP1011E"));
    assert!(!offline_stderr.contains("must-not-be-opened"));

    let batch = run(
        &[
            "--batch-manifest".into(),
            "must-not-be-opened.json".into(),
            "--acknowledge-oracle-preview".into(),
        ],
        &[],
    );
    let batch_stderr = String::from_utf8_lossy(&batch.stderr);
    assert_eq!(first_message_code(&batch_stderr), Some("DBP1011E"));
    assert!(batch_stderr.contains("standalone-only"));
    assert!(!batch_stderr.contains("rerun with --acknowledge-oracle-preview"));
}

#[test]
fn oracle_optional_stream_failure_warning_is_stable_and_identifier_safe() {
    let source = include_str!("../src/app_oracle.rs");
    let live = source
        .split("pub(super) fn run_oracle_live(")
        .nth(1)
        .unwrap()
        .split("pub(super) fn run_oracle_offline(")
        .next()
        .unwrap();
    let publication = live
        .find("publish_oracle_blueprint(cli, audit, blueprint)?")
        .expect("live Oracle path must publish its Blueprint");
    let stream = live
        .find("write_oracle_basic_stream(")
        .expect("live Oracle path must attempt the optional stream");
    assert!(publication < stream);
    let warning = source
        .split("fn record_optional_oracle_stream(")
        .nth(1)
        .unwrap()
        .split("#[cfg(unix)]")
        .next()
        .unwrap();
    assert!(warning.contains("DBP1430W"));
    assert!(warning.contains("redacted_oracle_stream_write_failure(&error)"));
    assert!(!warning.contains("error.to_string()"));
    assert!(include_str!("../docs/MESSAGES.md").contains("`DBP1430W`"));
}
