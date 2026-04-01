//! Asset storage abstraction and path policy.

use crate::compression::should_compress_asset;
use caliberate_core::config::{ControlPlane, DuplicatePolicy};
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
}

pub trait AssetStore: Send + Sync {
    fn store(&self, source: &Path, mode: StorageMode) -> CoreResult<AssetRecord>;
}

#[derive(Debug, Clone)]
pub struct LocalAssetStore {
    root_dir: PathBuf,
    duplicate_policy: DuplicatePolicy,
    compress_assets: bool,
}

impl LocalAssetStore {
    pub fn from_config(config: &ControlPlane) -> Self {
        Self {
            root_dir: config.paths.library_dir.clone(),
            duplicate_policy: config.ingest.duplicate_policy,
            compress_assets: should_compress_asset(&config.assets),
        }
    }

    fn ensure_root(&self) -> CoreResult<()> {
        fs::create_dir_all(&self.root_dir)
            .map_err(|err| CoreError::Io("create library dir".to_string(), err))
    }
}

impl AssetStore for LocalAssetStore {
    fn store(&self, source: &Path, mode: StorageMode) -> CoreResult<AssetRecord> {
        self.ensure_root()?;

        let metadata = fs::metadata(source)
            .map_err(|err| CoreError::Io("read asset metadata".to_string(), err))?;

        let file_name = source
            .file_name()
            .ok_or_else(|| CoreError::ConfigValidate("asset has no filename".to_string()))?;
        let dest_path = self.root_dir.join(file_name);

        let record = match mode {
            StorageMode::Copy => {
                if dest_path.exists() {
                    match self.duplicate_policy {
                        DuplicatePolicy::Overwrite => {}
                        DuplicatePolicy::Skip => {
                            return Err(CoreError::DuplicateAsset(dest_path));
                        }
                        DuplicatePolicy::Error => {
                            return Err(CoreError::DuplicateAsset(dest_path));
                        }
                    }
                }
                fs::copy(source, &dest_path)
                    .map_err(|err| CoreError::Io("copy asset".to_string(), err))?;
                if self.compress_assets {
                    info!(
                        component = "assets",
                        action = "compress",
                        path = %dest_path.display(),
                        "asset compression enabled (not yet applied)"
                    );
                }
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
                }
            }
        };

        Ok(record)
    }
}
