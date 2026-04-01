use caliberate_assets::stats::{
    AssetDescriptor, apply_compaction, compute_storage_stats, plan_compaction,
};
use caliberate_assets::storage::StorageMode;
use std::fs;
use tempfile::tempdir;

#[test]
fn plans_and_applies_compaction() {
    let dir = tempdir().expect("temp dir");
    let library_dir = dir.path().join("library");
    fs::create_dir_all(&library_dir).expect("create library dir");

    let tracked_path = library_dir.join("tracked.epub");
    fs::write(&tracked_path, b"tracked").expect("write tracked");

    let orphan_path = library_dir.join("orphan.epub");
    fs::write(&orphan_path, b"orphan").expect("write orphan");

    let missing_path = library_dir.join("missing.epub");

    let tracked_meta = fs::metadata(&tracked_path).expect("tracked meta");

    let assets = vec![
        AssetDescriptor {
            id: 1,
            stored_path: tracked_path.clone(),
            storage_mode: StorageMode::Copy,
            size_bytes: tracked_meta.len(),
            stored_size_bytes: tracked_meta.len(),
            checksum: None,
            is_compressed: false,
        },
        AssetDescriptor {
            id: 2,
            stored_path: missing_path,
            storage_mode: StorageMode::Copy,
            size_bytes: 5,
            stored_size_bytes: 5,
            checksum: None,
            is_compressed: false,
        },
    ];

    let plan = plan_compaction(&assets, &library_dir).expect("plan compaction");
    assert_eq!(plan.orphan_files.len(), 1);
    assert_eq!(plan.missing_asset_ids.len(), 1);

    let result = apply_compaction(&plan).expect("apply compaction");
    assert_eq!(result.orphan_files_removed, 1);
    assert!(!orphan_path.exists());

    let stats = compute_storage_stats(&assets, &library_dir).expect("stats");
    assert_eq!(stats.total_assets, 2);
    assert_eq!(stats.library_files, 1);
}
