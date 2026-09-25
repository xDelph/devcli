//! Rust workspace manifest parsing (inherited package fields).

use app_detector::context::DetectionContext;
use app_detector::strategies::RustStrategy;
use app_detector::strategy::DetectionStrategy;
use app_detector::types::DetectionData;
use std::fs;
use tempfile::TempDir;

#[test]
fn rust_workspace_inherited_package_fields() {
    let temp_dir = TempDir::new().unwrap();
    fs::create_dir_all(temp_dir.path().join("src")).unwrap();
    fs::write(temp_dir.path().join("src/lib.rs"), "").unwrap();
    fs::write(
        temp_dir.path().join("Cargo.toml"),
        r#"[package]
name = "devcli-core"
version.workspace = true
edition.workspace = true
"#,
    )
    .unwrap();

    let ctx = DetectionContext::new(temp_dir.path()).unwrap();
    let result = RustStrategy.detect(&ctx).unwrap();

    match result.data {
        DetectionData::Language(info) => {
            assert_eq!(info.metadata.get("package_name").unwrap(), "devcli-core");
        }
        _ => panic!("Expected Language data"),
    }
}
