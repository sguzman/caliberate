//! Asset storage abstraction and path policy.

use crate::compression::{compress_file, should_compress_asset};
use crate::hashing::{hash_file_sha256, hash_zstd_file_sha256};
use caliberate_core::config::{ControlPlane, DuplicateCompare, DuplicatePolicy};
use caliberate_core::error::{CoreError, CoreResult};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageMode {
    Copy,
    Reference,
}

#[derive(Debug, Clone)]
pub struct AssetRecord {
    pub storage_mode: StorageMode,
    pub stored_path: PathBuf,
    pub source_path: Option<PathBuf>,
    pub size_bytes: u64,
    pub stored_size_bytes: u64,
    pub checksum: Option<String>,
    pub is_compressed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateSkipReason {
    Identical,
    Conflict,
}

#[derive(Debug, Clone)]
pub struct DuplicateSkip {
    pub existing_path: PathBuf,
    pub reason: DuplicateSkipReason,
}

#[derive(Debug, Clone)]
pub enum StoreOutcome {
    Stored(AssetRecord),
    Skipped(DuplicateSkip),
}

pub trait AssetStore: Send + Sync {
    fn store(&self, source: &Path, mode: StorageMode) -> CoreResult<StoreOutcome>;
}

#[derive(Debug, Clone)]
pub struct LocalAssetStore {
    root_dir: PathBuf,
    duplicate_policy: DuplicatePolicy,
    duplicate_identical_policy: DuplicatePolicy,
    duplicate_compare: DuplicateCompare,
    compress_assets: bool,
    hash_on_ingest: bool,
    compression_level: i32,
}

impl LocalAssetStore {
    pub fn from_config(config: &ControlPlane) -> Self {
        Self {
            root_dir: config.paths.library_dir.clone(),
            duplicate_policy: config.ingest.duplicate_policy,
            duplicate_identical_policy: config.ingest.duplicate_identical_policy,
            duplicate_compare: config.ingest.duplicate_compare,
            compress_assets: should_compress_asset(&config.assets),
            hash_on_ingest: config.assets.hash_on_ingest,
            compression_level: config.assets.compression_level,
        }
    }

    fn ensure_root(&self) -> CoreResult<()> {
        fs::create_dir_all(&self.root_dir)
            .map_err(|err| CoreError::Io("create library dir".to_string(), err))
    }
}

impl AssetStore for LocalAssetStore {
    fn store(&self, source: &Path, mode: StorageMode) -> CoreResult<StoreOutcome> {
        self.ensure_root()?;

        let metadata = fs::metadata(source)
            .map_err(|err| CoreError::Io("read asset metadata".to_string(), err))?;

        let file_name = source
            .file_name()
            .ok_or_else(|| CoreError::ConfigValidate("asset has no filename".to_string()))?;
        let dest_file_name = if self.compress_assets && mode == StorageMode::Copy {
            format!("{}.zst", file_name.to_string_lossy())
        } else {
            file_name.to_string_lossy().to_string()
        };
        let dest_path = self.root_dir.join(dest_file_name);
        let checksum = if self.hash_on_ingest {
            Some(hash_file_sha256(source)?)
        } else {
            None
        };

        let record = match mode {
            StorageMode::Copy => {
                if dest_path.exists() {
                    let identical = self.compare_duplicate(source, &dest_path)?;
                    if identical {
                        match self.duplicate_identical_policy {
                            DuplicatePolicy::Overwrite => {}
                            DuplicatePolicy::Skip => {
                                info!(
                                    component = "assets",
                                    action = "skip-identical",
                                    dest = %dest_path.display(),
                                    "duplicate asset matched; skipping copy"
                                );
                                return Ok(StoreOutcome::Skipped(DuplicateSkip {
                                    existing_path: dest_path,
                                    reason: DuplicateSkipReason::Identical,
                                }));
                            }
                            DuplicatePolicy::Error => {
                                return Err(CoreError::DuplicateAsset(dest_path));
                            }
                        }
                    } else {
                        match self.duplicate_policy {
                            DuplicatePolicy::Overwrite => {}
                            DuplicatePolicy::Skip => {
                                info!(
                                    component = "assets",
                                    action = "skip-conflict",
                                    dest = %dest_path.display(),
                                    "duplicate asset conflict; skipping copy"
                                );
                                return Ok(StoreOutcome::Skipped(DuplicateSkip {
                                    existing_path: dest_path,
                                    reason: DuplicateSkipReason::Conflict,
                                }));
                            }
                            DuplicatePolicy::Error => {
                                return Err(CoreError::DuplicateAsset(dest_path));
                            }
                        }
                    }
                }
                let stored_size = if self.compress_assets {
                    compress_file(source, &dest_path, self.compression_level)?
                } else {
                    fs::copy(source, &dest_path)
                        .map_err(|err| CoreError::Io("copy asset".to_string(), err))?
                };
                if self.compress_assets {
                    info!(
                        component = "assets",
                        action = "compress",
                        path = %dest_path.display(),
                        "asset compressed"
                    );
                }
                let stored_size_bytes = if self.compress_assets {
                    fs::metadata(&dest_path)
                        .map_err(|err| CoreError::Io("read compressed metadata".to_string(), err))?
                        .len()
                } else {
                    stored_size
                };
                info!(
                    component = "assets",
                    action = "copy",
                    source = %source.display(),
                    dest = %dest_path.display(),
                    "asset copied"
                );
                AssetRecord {
                    storage_mode: StorageMode::Copy,
                    stored_path: dest_path,
                    source_path: Some(source.to_path_buf()),
                    size_bytes: metadata.len(),
                    stored_size_bytes,
                    checksum,
                    is_compressed: self.compress_assets,
                }
            }
            StorageMode::Reference => {
                info!(
                    component = "assets",
                    action = "reference",
                    source = %source.display(),
                    "asset referenced"
                );
                AssetRecord {
                    storage_mode: StorageMode::Reference,
                    stored_path: source.to_path_buf(),
                    source_path: None,
                    size_bytes: metadata.len(),
                    stored_size_bytes: metadata.len(),
                    checksum,
                    is_compressed: false,
                }
            }
        };

        Ok(StoreOutcome::Stored(record))
    }
}

impl LocalAssetStore {
    fn compare_duplicate(&self, source: &Path, dest: &Path) -> CoreResult<bool> {
        match self.duplicate_compare {
            DuplicateCompare::Size => {
                let source_len = fs::metadata(source)
                    .map_err(|err| CoreError::Io("read source metadata".to_string(), err))?
                    .len();
                let dest_len = fs::metadata(dest)
                    .map_err(|err| CoreError::Io("read dest metadata".to_string(), err))?
                    .len();
                Ok(source_len == dest_len)
            }
            DuplicateCompare::Checksum => {
                let source_hash = hash_file_sha256(source)?;
                let dest_hash = if dest.extension().is_some_and(|ext| ext == "zst") {
                    hash_zstd_file_sha256(dest)?
                } else {
                    hash_file_sha256(dest)?
                };
                Ok(source_hash == dest_hash)
            }
        }
    }
}
