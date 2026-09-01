use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

#[test]
fn canonical_blueprint_core_matches_its_source_manifest() {
    let core = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("crates/dbwarp-blueprint-core");
    let manifest = fs::read_to_string(core.join("SOURCE_MANIFEST.sha256"))
        .expect("canonical blueprint-core source manifest must be readable");

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
        assert_eq!(
            actual, expected,
            "canonical blueprint-core source changed at {relative}; regenerate SOURCE_MANIFEST.sha256 and synchronize every embedded mirror"
        );
    }
}
