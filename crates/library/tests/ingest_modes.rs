use caliberate_assets::storage::{LocalAssetStore, StorageMode};
use caliberate_core::config::{ControlPlane, IngestMode};
use caliberate_library::ingest::{IngestRequest, Ingestor};
use std::fs;
use tempfile::tempdir;

#[test]
fn ingest_copy_and_reference_modes() {
    let source_dir = tempdir().expect("source dir");
    let library_dir = tempdir().expect("library dir");

    let source_path = source_dir.path().join("book.epub");
    fs::write(&source_path, b"book data").expect("write source");

    let config_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../config/control-plane.toml");
    let mut config = ControlPlane::load_from_path(&config_path).expect("load config");
    config.paths.library_dir = library_dir.path().to_path_buf();
    config.assets.compress_raw_assets = false;
    config.assets.hash_on_ingest = true;

    let store = LocalAssetStore::from_config(&config);
    let ingestor = Ingestor::new(std::sync::Arc::new(store), config.clone());

    let copy_result = ingestor
        .ingest(IngestRequest {
            source_path: &source_path,
            mode: Some(IngestMode::Copy),
        })
        .expect("copy ingest");
    assert_eq!(copy_result.asset.storage_mode, StorageMode::Copy);
    assert!(
        copy_result
            .asset
            .stored_path
            .starts_with(library_dir.path())
    );
    assert!(copy_result.asset.stored_path.exists());
    assert!(copy_result.asset.checksum.is_some());

    let ref_result = ingestor
        .ingest(IngestRequest {
            source_path: &source_path,
            mode: Some(IngestMode::Reference),
        })
        .expect("reference ingest");
    assert_eq!(ref_result.asset.storage_mode, StorageMode::Reference);
    assert_eq!(ref_result.asset.stored_path, source_path);
    assert!(ref_result.asset.checksum.is_some());
}
