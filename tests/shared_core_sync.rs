use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

#[test]
fn canonical_blueprint_core_matches_its_source_manifest() {
    let core = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("crates/dbwarp-blueprint-core");
    let manifest = fs::read_to_string(core.join("SOURCE_MANIFEST.sha256"))
        .expect("canonical blueprint-core source manifest must be readable");
    let mut manifested_paths = BTreeSet::new();

    for (line_no, line) in manifest.lines().enumerate() {
        let (expected, relative) = line.split_once("  ").unwrap_or_else(|| {
            panic!(
                "invalid blueprint-core source manifest line {}: {line}",
                line_no + 1
            )
        });
        let bytes = fs::read(core.join(relative)).unwrap_or_else(|error| {
            panic!("could not read blueprint-core source {relative}: {error}")
        });
        let actual = format!("{:x}", Sha256::digest(bytes));
        assert!(
            manifested_paths.insert(PathBuf::from(relative)),
            "blueprint-core source manifest lists {relative} more than once"
        );
        assert_eq!(
            actual, expected,
            "canonical blueprint-core source changed at {relative}; regenerate SOURCE_MANIFEST.sha256 and synchronize every embedded mirror"
        );
    }

    let fixture_directory = core.join("tests/fixtures/artifact_complexity");
    let mut fixture_count = 0_u64;
    for entry in fs::read_dir(&fixture_directory)
        .expect("artifact-complexity oracle directory must be readable")
    {
        let path = entry
            .expect("artifact-complexity oracle entry must be readable")
            .path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("toml") {
            continue;
        }
        fixture_count += 1;
        let relative = path
            .strip_prefix(&core)
            .expect("artifact-complexity oracle must live under the canonical core");
        assert!(
            manifested_paths.contains(relative),
            "artifact-complexity oracle {} is outside SOURCE_MANIFEST.sha256 and would not reach the DBWarp mirror",
            relative.display()
        );
    }
    assert!(
        fixture_count > 0,
        "artifact-complexity semantic-oracle corpus must not be empty"
    );
}
