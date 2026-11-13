//! Asset storage abstraction and path policy.

use crate::compression::{compress_file, compress_reader_to_writer, should_compress_asset};
use crate::hashing::{hash_file_sha256, hash_zstd_file_sha256};
use caliberate_core::config::{ControlPlane, DuplicateCompare, DuplicatePolicy};
use caliberate_core::error::{CoreError, CoreResult};
use std::fs::{self, File, OpenOptions};
use std::io;
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

    /// Bypass duplicate skips without ever clobbering an existing managed file.
    /// For reference imports there is no managed destination to allocate.
    pub fn store_forced(&self, source: &Path, mode: StorageMode) -> CoreResult<StoreOutcome> {
        if mode == StorageMode::Reference {
            return self.store(source, mode);
        }

        self.ensure_root()?;
        let metadata = fs::metadata(source)
            .map_err(|err| CoreError::Io("read asset metadata".to_string(), err))?;
        let file_name = source.file_name().ok_or_else(|| {
            CoreError::ConfigValidate("asset has no filename".to_string())
        })?;
        let basename = file_name.to_string_lossy();
        let stem = source.file_stem().unwrap_or(file_name).to_string_lossy();
        let extension = source.extension().map(|ext| ext.to_string_lossy());
        let checksum = if self.hash_on_ingest {
            Some(hash_file_sha256(source)?)
        } else {
            None
        };

        for attempt in 0..10_000 {
            let name = if attempt == 0 {
                basename.to_string()
            } else if let Some(ext) = &extension {
                format!("{stem} (copy {}).{ext}", attempt + 1)
            } else {
                format!("{stem} (copy {})", attempt + 1)
            };
            let name = if self.compress_assets {
                format!("{name}.zst")
            } else {
                name
            };
            let dest_path = self.root_dir.join(name);
            // create_new is atomic: even concurrent forced imports cannot
            // overwrite an existing file or follow a pre-existing symlink.
            let output = match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&dest_path)
            {
                Ok(output) => output,
                Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(err) => return Err(CoreError::Io("create forced asset".to_string(), err)),
            };
            let written = (|| -> CoreResult<()> {
                let mut input = File::open(source)
                    .map_err(|err| CoreError::Io("open forced asset source".to_string(), err))?;
                if self.compress_assets {
                    compress_reader_to_writer(&mut input, output, self.compression_level)?;
                } else {
                    let mut output = output;
                    io::copy(&mut input, &mut output)
                        .map_err(|err| CoreError::Io("copy forced asset".to_string(), err))?;
                }
                Ok(())
            })();
            if let Err(err) = written {
                let _ = fs::remove_file(&dest_path);
                return Err(err);
            }
            let stored_size_bytes = fs::metadata(&dest_path)
                .map_err(|err| CoreError::Io("read forced asset metadata".to_string(), err))?
                .len();
            info!(
                component = "assets",
                action = "force-new-copy",
                source = %source.display(),
                dest = %dest_path.display(),
                "forced asset stored without overwriting"
            );
            return Ok(StoreOutcome::Stored(AssetRecord {
                storage_mode: StorageMode::Copy,
                stored_path: dest_path,
                source_path: Some(source.to_path_buf()),
                size_bytes: metadata.len(),
                stored_size_bytes,
                checksum,
                is_compressed: self.compress_assets,
            }));
        }
        Err(CoreError::ConfigValidate(
            "could not allocate a unique managed filename for --force".to_string(),
        ))
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

#[cfg(test)]
mod forced_storage_tests {
    use super::{AssetStore, LocalAssetStore, StorageMode, StoreOutcome};
    use crate::hashing::{hash_file_sha256, hash_zstd_file_sha256};
    use caliberate_core::config::ControlPlane;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn forced_copy_preserves_original_and_allocates_new_filename() {
        for compress in [false, true] {
            let tmp = tempdir().unwrap();
            let source = tmp.path().join("book.epub");
            fs::write(&source, b"fixture book contents").unwrap();
            let example = concat!(env!("CARGO_MANIFEST_DIR"), "/../../config/control-plane.toml");
            let mut config = ControlPlane::load_from_path(example).unwrap();
            config.paths.library_dir = tmp.path().join("managed");
            config.assets.compress_raw_assets = compress;
            let store = LocalAssetStore::from_config(&config);

            let first = match store.store(&source, StorageMode::Copy).unwrap() {
                StoreOutcome::Stored(asset) => asset,
                _ => panic!("initial copy must be stored"),
            };
            let second = match store.store_forced(&source, StorageMode::Copy).unwrap() {
                StoreOutcome::Stored(asset) => asset,
                _ => panic!("forced copy must be stored"),
            };
            assert_ne!(first.stored_path, second.stored_path);
            assert!(first.stored_path.exists());
            assert!(second.stored_path.exists());
            let original = hash_file_sha256(&source).unwrap();
            for asset in [&first, &second] {
                let persisted = if compress {
                    hash_zstd_file_sha256(&asset.stored_path).unwrap()
                } else {
                    hash_file_sha256(&asset.stored_path).unwrap()
                };
                assert_eq!(persisted, original);
            }
        }
    }

    #[test]
    fn forced_reference_never_copies_or_changes_source() {
        let tmp = tempdir().unwrap();
        let source = tmp.path().join("book.epub");
        fs::write(&source, b"reference fixture").unwrap();
        let example = concat!(env!("CARGO_MANIFEST_DIR"), "/../../config/control-plane.toml");
        let mut config = ControlPlane::load_from_path(example).unwrap();
        config.paths.library_dir = tmp.path().join("managed");
        let store = LocalAssetStore::from_config(&config);
        let result = match store.store_forced(&source, StorageMode::Reference).unwrap() {
            StoreOutcome::Stored(asset) => asset,
            _ => panic!("reference import must be registered"),
        };
        assert_eq!(result.stored_path, source);
        assert!(!result.is_compressed);
        assert_eq!(fs::read(&source).unwrap(), b"reference fixture");
    }
}
