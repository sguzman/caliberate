//! Storage stats and auditing.

use crate::hashing::{hash_file_sha256, hash_zstd_file_sha256};
use crate::storage::StorageMode;
use caliberate_core::config::AssetsConfig;
use caliberate_core::error::{CoreError, CoreResult};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct AssetDescriptor {
    pub id: i64,
    pub stored_path: PathBuf,
    pub storage_mode: StorageMode,
    pub size_bytes: u64,
    pub stored_size_bytes: u64,
    pub checksum: Option<String>,
    pub is_compressed: bool,
}

#[derive(Debug, Clone)]
pub struct StorageStats {
    pub total_assets: usize,
    pub copied_assets: usize,
    pub referenced_assets: usize,
    pub compressed_assets: usize,
    pub total_bytes: u64,
    pub stored_bytes: u64,
    pub library_files: usize,
    pub library_bytes: u64,
    pub orphan_files: usize,
    pub orphan_bytes: u64,
}

#[derive(Debug, Clone)]
pub enum IntegrityIssueKind {
    Missing,
    SizeMismatch,
    ChecksumMismatch,
}

#[derive(Debug, Clone)]
pub struct IntegrityIssue {
    pub asset_id: i64,
    pub stored_path: PathBuf,
    pub kind: IntegrityIssueKind,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub struct CompactionPlan {
    pub orphan_files: Vec<PathBuf>,
    pub missing_asset_ids: Vec<i64>,
}

#[derive(Debug, Clone)]
pub struct CompactionResult {
    pub orphan_files_removed: usize,
    pub orphan_bytes_removed: u64,
    pub missing_assets_pruned: usize,
}

pub fn compute_storage_stats(
    assets: &[AssetDescriptor],
    library_root: &Path,
) -> CoreResult<StorageStats> {
    let mut copied_assets = 0;
    let mut referenced_assets = 0;
    let mut compressed_assets = 0;
    let mut total_bytes: u64 = 0;
    let mut stored_bytes: u64 = 0;

    for asset in assets {
        total_bytes = total_bytes.saturating_add(asset.size_bytes);
        stored_bytes = stored_bytes.saturating_add(asset.stored_size_bytes);
        match asset.storage_mode {
            StorageMode::Copy => copied_assets += 1,
            StorageMode::Reference => referenced_assets += 1,
        }
        if asset.is_compressed {
            compressed_assets += 1;
        }
    }

    let mut library_files = 0;
    let mut library_bytes: u64 = 0;
    if library_root.exists() {
        for entry in WalkDir::new(library_root)
            .into_iter()
            .filter_map(Result::ok)
        {
            if entry.file_type().is_file() {
                library_files += 1;
                library_bytes = library_bytes.saturating_add(
                    entry
                        .metadata()
                        .map_err(|err| {
                            CoreError::Io("read library file metadata".to_string(), err.into())
                        })?
                        .len(),
                );
            }
        }
    }

    let plan = plan_compaction(assets, library_root)?;
    let mut orphan_bytes: u64 = 0;
    for orphan in &plan.orphan_files {
        if let Ok(metadata) = fs::metadata(orphan) {
            orphan_bytes = orphan_bytes.saturating_add(metadata.len());
        }
    }

    Ok(StorageStats {
        total_assets: assets.len(),
        copied_assets,
        referenced_assets,
        compressed_assets,
        total_bytes,
        stored_bytes,
        library_files,
        library_bytes,
        orphan_files: plan.orphan_files.len(),
        orphan_bytes,
    })
}

pub fn verify_assets(
    assets: &[AssetDescriptor],
    config: &AssetsConfig,
) -> CoreResult<Vec<IntegrityIssue>> {
    let mut issues = Vec::new();

    for asset in assets {
        if !asset.stored_path.exists() {
            issues.push(IntegrityIssue {
                asset_id: asset.id,
                stored_path: asset.stored_path.clone(),
                kind: IntegrityIssueKind::Missing,
                detail: "stored path missing".to_string(),
            });
            continue;
        }

        let metadata = fs::metadata(&asset.stored_path)
            .map_err(|err| CoreError::Io("read stored asset metadata".to_string(), err))?;
        let actual_size = metadata.len();
        if actual_size != asset.stored_size_bytes {
            issues.push(IntegrityIssue {
                asset_id: asset.id,
                stored_path: asset.stored_path.clone(),
                kind: IntegrityIssueKind::SizeMismatch,
                detail: format!(
                    "stored size mismatch: expected {}, got {}",
                    asset.stored_size_bytes, actual_size
                ),
            });
        }

        if config.verify_checksum {
            if let Some(expected) = &asset.checksum {
                let actual = if asset.is_compressed {
                    hash_zstd_file_sha256(&asset.stored_path)?
                } else {
                    hash_file_sha256(&asset.stored_path)?
                };
                if &actual != expected {
                    issues.push(IntegrityIssue {
                        asset_id: asset.id,
                        stored_path: asset.stored_path.clone(),
                        kind: IntegrityIssueKind::ChecksumMismatch,
                        detail: "checksum mismatch".to_string(),
                    });
                }
            }
        }
    }

    info!(
        component = "assets",
        issues = issues.len(),
        "asset verification complete"
    );

    Ok(issues)
}

pub fn plan_compaction(
    assets: &[AssetDescriptor],
    library_root: &Path,
) -> CoreResult<CompactionPlan> {
    let mut tracked_files: HashSet<PathBuf> = HashSet::new();
    let mut missing_asset_ids = Vec::new();

    for asset in assets {
        tracked_files.insert(asset.stored_path.clone());
        if !asset.stored_path.exists() {
            missing_asset_ids.push(asset.id);
        }
    }

    let mut orphan_files = Vec::new();
    if library_root.exists() {
        for entry in WalkDir::new(library_root)
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path().to_path_buf();
            if !tracked_files.contains(&path) {
                orphan_files.push(path);
            }
        }
    }

    Ok(CompactionPlan {
        orphan_files,
        missing_asset_ids,
    })
}

pub fn apply_compaction(plan: &CompactionPlan) -> CoreResult<CompactionResult> {
    let mut orphan_files_removed = 0;
    let mut orphan_bytes_removed: u64 = 0;

    for orphan in &plan.orphan_files {
        if let Ok(metadata) = fs::metadata(orphan) {
            orphan_bytes_removed = orphan_bytes_removed.saturating_add(metadata.len());
        }
        fs::remove_file(orphan)
            .map_err(|err| CoreError::Io("remove orphaned asset".to_string(), err))?;
        orphan_files_removed += 1;
    }

    Ok(CompactionResult {
        orphan_files_removed,
        orphan_bytes_removed,
        missing_assets_pruned: plan.missing_asset_ids.len(),
    })
}
