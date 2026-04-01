use caliberate_assets::storage::LocalAssetStore;
use caliberate_core::config::{ControlPlane, DuplicateCompare, DuplicatePolicy, IngestMode};
use caliberate_library::ingest::{IngestOutcome, IngestRequest, Ingestor};
use std::fs;
use tempfile::tempdir;

#[test]
fn skips_identical_duplicates() {
    let source_dir = tempdir().expect("source dir");
    let library_dir = tempdir().expect("library dir");

    let source_path = source_dir.path().join("dup.epub");
    fs::write(&source_path, b"same data").expect("write source");

    let config_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../config/control-plane.toml");
    let mut config = ControlPlane::load_from_path(&config_path).expect("load config");
    config.paths.library_dir = library_dir.path().to_path_buf();
    config.assets.compress_raw_assets = false;
    config.ingest.duplicate_policy = DuplicatePolicy::Error;
    config.ingest.duplicate_identical_policy = DuplicatePolicy::Skip;
    config.ingest.duplicate_compare = DuplicateCompare::Checksum;

    let store = LocalAssetStore::from_config(&config);
    let ingestor = Ingestor::new(std::sync::Arc::new(store), config.clone());

    let _first = ingestor
        .ingest(IngestRequest {
            source_path: &source_path,
            mode: Some(IngestMode::Copy),
        })
        .expect("first ingest");

    let second = ingestor
        .ingest(IngestRequest {
            source_path: &source_path,
            mode: Some(IngestMode::Copy),
        })
        .expect("second ingest");

    let IngestOutcome::Skipped(skip) = second else {
        panic!("expected duplicate skip");
    };
    assert!(skip.existing_path.exists());
}

#[test]
fn skips_conflicting_duplicates_when_configured() {
    let source_dir = tempdir().expect("source dir");
    let source_dir_two = tempdir().expect("source dir 2");
    let library_dir = tempdir().expect("library dir");

    let source_path = source_dir.path().join("conflict.epub");
    let source_path_two = source_dir_two.path().join("conflict.epub");
    fs::write(&source_path, b"first data").expect("write source 1");
    fs::write(&source_path_two, b"second data").expect("write source 2");

    let config_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../config/control-plane.toml");
    let mut config = ControlPlane::load_from_path(&config_path).expect("load config");
    config.paths.library_dir = library_dir.path().to_path_buf();
    config.assets.compress_raw_assets = false;
    config.ingest.duplicate_policy = DuplicatePolicy::Skip;
    config.ingest.duplicate_identical_policy = DuplicatePolicy::Skip;
    config.ingest.duplicate_compare = DuplicateCompare::Checksum;

    let store = LocalAssetStore::from_config(&config);
    let ingestor = Ingestor::new(std::sync::Arc::new(store), config.clone());

    let _first = ingestor
        .ingest(IngestRequest {
            source_path: &source_path,
            mode: Some(IngestMode::Copy),
        })
        .expect("first ingest");

    let second = ingestor
        .ingest(IngestRequest {
            source_path: &source_path_two,
            mode: Some(IngestMode::Copy),
        })
        .expect("second ingest");

    let IngestOutcome::Skipped(skip) = second else {
        panic!("expected conflict skip");
    };
    assert!(skip.existing_path.exists());
}
