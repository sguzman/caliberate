use caliberate_assets::hashing::hash_file_sha256;
use caliberate_assets::stats::{AssetDescriptor, IntegrityIssueKind, verify_assets};
use caliberate_assets::storage::StorageMode;
use caliberate_core::config::AssetsConfig;
use std::fs;
use tempfile::tempdir;

fn test_assets_config() -> AssetsConfig {
    AssetsConfig {
        compress_raw_assets: false,
        compress_metadata_db: false,
        hash_algorithm: "sha256".to_string(),
        hash_on_ingest: true,
        verify_checksum: true,
        compression_level: 3,
    }
}

#[test]
fn reports_missing_assets() {
    let missing_path = std::env::temp_dir().join("caliberate-missing.asset");
    let assets = vec![AssetDescriptor {
        id: 1,
        stored_path: missing_path,
        storage_mode: StorageMode::Reference,
        size_bytes: 1,
        stored_size_bytes: 1,
        checksum: None,
        is_compressed: false,
    }];

    let issues = verify_assets(&assets, &test_assets_config()).expect("verify");
    assert_eq!(issues.len(), 1);
    assert!(matches!(issues[0].kind, IntegrityIssueKind::Missing));
}

#[test]
fn detects_checksum_mismatch() {
    let dir = tempdir().expect("temp dir");
    let file_path = dir.path().join("asset.bin");
    fs::write(&file_path, b"original").expect("write");
    let checksum = hash_file_sha256(&file_path).expect("hash");

    fs::write(&file_path, b"modified").expect("rewrite");

    let assets = vec![AssetDescriptor {
        id: 2,
        stored_path: file_path,
        storage_mode: StorageMode::Copy,
        size_bytes: 8,
        stored_size_bytes: 8,
        checksum: Some(checksum),
        is_compressed: false,
    }];

    let issues = verify_assets(&assets, &test_assets_config()).expect("verify");
    assert_eq!(issues.len(), 1);
    assert!(matches!(
        issues[0].kind,
        IntegrityIssueKind::ChecksumMismatch
    ));
}
