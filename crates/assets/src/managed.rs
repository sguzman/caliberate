//! Content-addressed managed objects for explicit reference adoption.

use crate::compression::{compress_file, decompress_to_writer};
use crate::hashing::{hash_file_sha256, hash_zstd_file_sha256};
use caliberate_core::config::ControlPlane;
use caliberate_core::error::{CoreError, CoreResult};
use std::fs;
use std::io::sink;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct ManagedObjectStore {
    root: PathBuf,
    compress: bool,
    compression_level: i32,
}

#[derive(Debug, Clone)]
pub struct ManagedObject {
    pub stored_path: PathBuf,
    pub logical_size_bytes: u64,
    pub stored_size_bytes: u64,
    pub checksum_sha256: String,
    pub is_compressed: bool,
    pub reused_existing: bool,
}

impl ManagedObjectStore {
    pub fn from_config(config: &ControlPlane) -> Self {
        Self {
            root: config.paths.library_dir.clone(),
            compress: config.assets.compress_raw_assets,
            compression_level: config.assets.compression_level,
        }
    }

    pub fn adopt_file(&self, source: &Path, logical_format: &str) -> CoreResult<ManagedObject> {
        let format = normalize_format(logical_format)?;
        let source_metadata = fs::metadata(source)
            .map_err(|err| CoreError::Io("inspect adoption source".to_string(), err))?;
        if !source_metadata.is_file() {
            return Err(CoreError::ConfigValidate(
                "adoption source must be a regular file".into(),
            ));
        }
        let checksum = hash_file_sha256(source)?;
        let logical_size_bytes = source_metadata.len();
        let suffix = if self.compress { ".zst" } else { "" };
        let relative = PathBuf::from("objects")
            .join("sha256")
            .join(&checksum[..2])
            .join(format!("{checksum}.{format}{suffix}"));
        let stored_path = self.root.join(relative);
        if stored_path.exists() {
            return self.verify_existing(stored_path, checksum, logical_size_bytes, self.compress);
        }

        let parent = stored_path
            .parent()
            .ok_or_else(|| CoreError::ConfigValidate("managed object has no parent".into()))?;
        fs::create_dir_all(parent)
            .map_err(|err| CoreError::Io("create managed object directory".to_string(), err))?;
        let temporary = parent.join(format!(
            ".{}.{}.tmp",
            stored_path.file_name().unwrap().to_string_lossy(),
            temporary_suffix()
        ));
        let write_result = if self.compress {
            compress_file(source, &temporary, self.compression_level).map(|_| ())
        } else {
            fs::copy(source, &temporary)
                .map(|_| ())
                .map_err(|err| CoreError::Io("write managed object".to_string(), err))
        };
        if let Err(error) = write_result {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }

        let verified = verify_object(&temporary, &checksum, logical_size_bytes, self.compress);
        if let Err(error) = verified {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        if stored_path.exists() {
            let _ = fs::remove_file(&temporary);
            return self.verify_existing(stored_path, checksum, logical_size_bytes, self.compress);
        }
        if let Err(error) = fs::rename(&temporary, &stored_path) {
            let _ = fs::remove_file(&temporary);
            return Err(CoreError::Io("publish managed object".to_string(), error));
        }
        let stored_size_bytes = fs::metadata(&stored_path)
            .map_err(|err| CoreError::Io("inspect managed object".to_string(), err))?
            .len();
        Ok(ManagedObject {
            stored_path,
            logical_size_bytes,
            stored_size_bytes,
            checksum_sha256: checksum,
            is_compressed: self.compress,
            reused_existing: false,
        })
    }

    fn verify_existing(
        &self,
        stored_path: PathBuf,
        checksum: String,
        logical_size_bytes: u64,
        is_compressed: bool,
    ) -> CoreResult<ManagedObject> {
        verify_object(&stored_path, &checksum, logical_size_bytes, is_compressed)?;
        let stored_size_bytes = fs::metadata(&stored_path)
            .map_err(|err| CoreError::Io("inspect existing managed object".to_string(), err))?
            .len();
        Ok(ManagedObject {
            stored_path,
            logical_size_bytes,
            stored_size_bytes,
            checksum_sha256: checksum,
            is_compressed,
            reused_existing: true,
        })
    }
}

fn normalize_format(format: &str) -> CoreResult<String> {
    let format = format.trim().to_ascii_lowercase();
    if format.is_empty()
        || !format.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
    {
        return Err(CoreError::ConfigValidate(
            "logical format is not safe for managed object storage".into(),
        ));
    }
    Ok(format)
}

fn verify_object(
    path: &Path,
    expected_checksum: &str,
    expected_logical_size: u64,
    is_compressed: bool,
) -> CoreResult<()> {
    let checksum = if is_compressed {
        hash_zstd_file_sha256(path)?
    } else {
        hash_file_sha256(path)?
    };
    if checksum != expected_checksum {
        return Err(CoreError::ConfigValidate(format!(
            "managed object checksum mismatch at {}",
            path.display()
        )));
    }
    let logical_size = if is_compressed {
        let mut output = sink();
        decompress_to_writer(path, &mut output)?
    } else {
        fs::metadata(path)
            .map_err(|err| CoreError::Io("inspect managed object size".to_string(), err))?
            .len()
    };
    if logical_size != expected_logical_size {
        return Err(CoreError::ConfigValidate(format!(
            "managed object logical size mismatch at {}",
            path.display()
        )));
    }
    Ok(())
}

fn temporary_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::ManagedObjectStore;
    use caliberate_core::config::ControlPlane;
    use std::fs;
    use tempfile::TempDir;

    fn config(dir: &TempDir, compress: bool) -> ControlPlane {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../config/control-plane.toml");
        let mut config = ControlPlane::load_from_path(path).unwrap();
        config.paths.library_dir = dir.path().join("library");
        config.assets.compress_raw_assets = compress;
        config
    }

    #[test]
    fn stores_content_addressed_identity_object_and_reuses_it() {
        let dir = tempfile::tempdir().unwrap();
        let config = config(&dir, false);
        let source = dir.path().join("one.epub");
        fs::write(&source, b"same bytes").unwrap();
        let store = ManagedObjectStore::from_config(&config);
        let first = store.adopt_file(&source, "EPUB").unwrap();
        let second = store.adopt_file(&source, "epub").unwrap();
        assert!(
            first
                .stored_path
                .ends_with(format!("{}.epub", first.checksum_sha256))
        );
        assert_eq!(first.logical_size_bytes, 10);
        assert_eq!(first.stored_size_bytes, 10);
        assert!(!first.reused_existing);
        assert!(second.reused_existing);
        assert_eq!(first.stored_path, second.stored_path);
        assert_eq!(fs::read(&source).unwrap(), b"same bytes");
        assert_eq!(
            fs::read_dir(first.stored_path.parent().unwrap())
                .unwrap()
                .count(),
            1
        );
    }

    #[test]
    fn compresses_objects_and_converges_identical_content() {
        let dir = tempfile::tempdir().unwrap();
        let config = config(&dir, true);
        let first_source = dir.path().join("first.epub");
        let second_source = dir.path().join("second.epub");
        fs::write(&first_source, vec![b'x'; 4096]).unwrap();
        fs::write(&second_source, vec![b'x'; 4096]).unwrap();
        let store = ManagedObjectStore::from_config(&config);
        let first = store.adopt_file(&first_source, "epub").unwrap();
        let second = store.adopt_file(&second_source, "epub").unwrap();
        assert!(first.stored_path.to_string_lossy().ends_with(".epub.zst"));
        assert!(first.stored_size_bytes < first.logical_size_bytes);
        assert!(second.reused_existing);
        assert_eq!(first.stored_path, second.stored_path);
        fs::write(&first_source, b"different bytes").unwrap();
        let different = store.adopt_file(&first_source, "epub").unwrap();
        assert_ne!(different.stored_path, first.stored_path);
    }

    #[test]
    fn rejects_corrupt_preexisting_object_without_overwriting_it() {
        let dir = tempfile::tempdir().unwrap();
        let config = config(&dir, false);
        let source = dir.path().join("source.epub");
        fs::write(&source, b"known bytes").unwrap();
        let store = ManagedObjectStore::from_config(&config);
        let checksum = {
            use sha2::{Digest, Sha256};
            hex::encode(Sha256::digest(b"known bytes"))
        };
        let path = config
            .paths
            .library_dir
            .join("objects/sha256")
            .join(&checksum[..2])
            .join(format!("{checksum}.epub"));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"corrupt").unwrap();
        assert!(store.adopt_file(&source, "epub").is_err());
        assert_eq!(fs::read(path).unwrap(), b"corrupt");
    }
}
