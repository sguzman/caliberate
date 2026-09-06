//! Explicit single-format adoption from an external reference into managed storage.

use caliberate_assets::compression::decompress_to_writer;
use caliberate_assets::hashing::{hash_file_sha256, hash_zstd_file_sha256};
use caliberate_assets::managed::ManagedObjectStore;
use caliberate_core::error::{CoreError, CoreResult};
use caliberate_db::database::{AssetRow, Database};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct AdoptFormatRequest {
    pub book_id: i64,
    pub format: String,
    pub reference_asset_id: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct AdoptFormatResult {
    pub book_id: i64,
    pub format: String,
    pub source_asset_id: i64,
    pub managed_asset_id: i64,
    pub stored_path: std::path::PathBuf,
    pub logical_size_bytes: u64,
    pub stored_size_bytes: u64,
    pub checksum_sha256: String,
    pub is_compressed: bool,
    pub reused_existing_object: bool,
    pub already_adopted: bool,
}

pub fn adopt_format(
    db: &Database,
    store: &ManagedObjectStore,
    request: AdoptFormatRequest,
) -> CoreResult<AdoptFormatResult> {
    if db.get_book(request.book_id)?.is_none() {
        return Err(CoreError::ConfigValidate("book does not exist".into()));
    }
    let format = request.format.trim().to_ascii_lowercase();
    let logical = db
        .get_book_format(request.book_id, &format)?
        .ok_or_else(|| CoreError::ConfigValidate("logical format does not exist".into()))?;
    let assets = db.list_assets_for_book(request.book_id)?;
    let references = assets
        .iter()
        .filter(|asset| {
            asset.book_format_id == Some(logical.id) && asset.storage_mode == "reference"
        })
        .collect::<Vec<_>>();
    let reference = if let Some(asset_id) = request.reference_asset_id {
        references
            .iter()
            .find(|asset| asset.id == asset_id)
            .copied()
            .ok_or_else(|| {
                CoreError::ConfigValidate(
                    "reference asset does not belong to the requested logical format".into(),
                )
            })?
    } else {
        references
            .into_iter()
            .min_by_key(|asset| asset.id)
            .ok_or_else(|| {
                CoreError::ConfigValidate("logical format has no reference asset".into())
            })?
    };
    let source_path = Path::new(&reference.stored_path);
    let source_metadata = fs::metadata(source_path)
        .map_err(|_| CoreError::ConfigValidate("reference source file is unavailable".into()))?;
    if !source_metadata.is_file() {
        return Err(CoreError::ConfigValidate(
            "reference source must be a regular file".into(),
        ));
    }

    let managed = assets
        .iter()
        .filter(|asset| asset.book_format_id == Some(logical.id) && asset.storage_mode == "copy")
        .min_by_key(|asset| asset.id);
    if let Some(managed) = managed {
        verify_existing_asset(managed)?;
        return Ok(result_from_asset(
            request.book_id,
            format,
            reference.id,
            managed.id,
            managed,
            true,
        ));
    }

    let object = store.adopt_file(source_path, &format)?;
    let created_at = adoption_timestamp();
    let source_path_string = source_path.to_string_lossy().into_owned();
    let managed_asset_id = match db.add_asset_for_format(
        request.book_id,
        logical.id,
        None,
        "copy",
        &object.stored_path.to_string_lossy(),
        Some(&source_path_string),
        object.logical_size_bytes,
        object.stored_size_bytes,
        Some(&object.checksum_sha256),
        object.is_compressed,
        &created_at,
    ) {
        Ok(id) => id,
        Err(error) => {
            if !object.reused_existing {
                let _ = fs::remove_file(&object.stored_path);
            }
            return Err(error);
        }
    };
    Ok(AdoptFormatResult {
        book_id: request.book_id,
        format,
        source_asset_id: reference.id,
        managed_asset_id,
        stored_path: object.stored_path,
        logical_size_bytes: object.logical_size_bytes,
        stored_size_bytes: object.stored_size_bytes,
        checksum_sha256: object.checksum_sha256,
        is_compressed: object.is_compressed,
        reused_existing_object: object.reused_existing,
        already_adopted: false,
    })
}

fn verify_existing_asset(asset: &AssetRow) -> CoreResult<()> {
    let path = Path::new(&asset.stored_path);
    if !path.is_file() {
        return Err(CoreError::ConfigValidate(
            "existing managed asset file is unavailable".into(),
        ));
    }
    let stored_size = fs::metadata(path)
        .map_err(|err| CoreError::Io("inspect existing managed asset".to_string(), err))?
        .len();
    if stored_size != asset.stored_size_bytes {
        return Err(CoreError::ConfigValidate(
            "existing managed asset stored size does not match".into(),
        ));
    }
    let logical_size = if asset.is_compressed {
        decompress_to_writer(path, std::io::sink())?
    } else {
        stored_size
    };
    if logical_size != asset.size_bytes {
        return Err(CoreError::ConfigValidate(
            "existing managed asset logical size does not match".into(),
        ));
    }
    if let Some(expected) = &asset.checksum {
        let actual = if asset.is_compressed {
            hash_zstd_file_sha256(path)?
        } else {
            hash_file_sha256(path)?
        };
        if actual != *expected {
            return Err(CoreError::ConfigValidate(
                "existing managed asset checksum does not match".into(),
            ));
        }
    }
    Ok(())
}

fn result_from_asset(
    book_id: i64,
    format: String,
    source_asset_id: i64,
    managed_asset_id: i64,
    asset: &AssetRow,
    already_adopted: bool,
) -> AdoptFormatResult {
    AdoptFormatResult {
        book_id,
        format,
        source_asset_id,
        managed_asset_id,
        stored_path: asset.stored_path.clone().into(),
        logical_size_bytes: asset.size_bytes,
        stored_size_bytes: asset.stored_size_bytes,
        checksum_sha256: asset.checksum.clone().unwrap_or_default(),
        is_compressed: asset.is_compressed,
        reused_existing_object: true,
        already_adopted,
    }
}

fn adoption_timestamp() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

#[cfg(test)]
mod tests {
    use super::{AdoptFormatRequest, adopt_format};
    use caliberate_assets::managed::ManagedObjectStore;
    use caliberate_core::config::ControlPlane;
    use caliberate_db::database::Database;
    use std::fs;
    use tempfile::TempDir;

    fn fixture(compress: bool) -> (TempDir, ControlPlane, Database) {
        let dir = tempfile::tempdir().unwrap();
        let config_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../config/control-plane.toml");
        let mut config = ControlPlane::load_from_path(config_path).unwrap();
        config.db.sqlite_path = dir.path().join("library.db");
        config.paths.library_dir = dir.path().join("managed");
        config.assets.compress_raw_assets = compress;
        let db = Database::open_with_fts(&config.db, &config.fts).unwrap();
        db.add_book("Book", "epub", "", "2026-01-01").unwrap();
        let format = db.get_book_format(1, "epub").unwrap().unwrap();
        let source = dir.path().join("legacy.epub");
        fs::write(&source, vec![b'x'; 4096]).unwrap();
        db.add_asset_for_format(
            1,
            format.id,
            None,
            "reference",
            &source.to_string_lossy(),
            None,
            4096,
            4096,
            None,
            false,
            "2026-01-01",
        )
        .unwrap();
        (dir, config, db)
    }

    #[test]
    fn adopts_and_repeats_without_deleting_reference() {
        let (dir, config, db) = fixture(true);
        let source = dir.path().join("legacy.epub");
        let before = fs::read(&source).unwrap();
        let store = ManagedObjectStore::from_config(&config);
        let first = adopt_format(
            &db,
            &store,
            AdoptFormatRequest {
                book_id: 1,
                format: "EPUB".into(),
                reference_asset_id: None,
            },
        )
        .unwrap();
        assert_eq!(first.source_asset_id, 1);
        assert!(first.is_compressed);
        assert!(!first.already_adopted);
        assert_eq!(db.list_assets_for_book(1).unwrap().len(), 2);
        let second = adopt_format(
            &db,
            &store,
            AdoptFormatRequest {
                book_id: 1,
                format: "epub".into(),
                reference_asset_id: None,
            },
        )
        .unwrap();
        assert!(second.already_adopted);
        assert_eq!(second.managed_asset_id, first.managed_asset_id);
        assert_eq!(db.list_assets_for_book(1).unwrap().len(), 2);
        assert_eq!(fs::read(source).unwrap(), before);
    }

    #[test]
    fn isolates_formats_and_rejects_missing_logical_sources() {
        let (dir, config, db) = fixture(false);
        let pdf = db.upsert_book_format(1, "pdf", Some(3)).unwrap();
        let pdf_source = dir.path().join("legacy.pdf");
        fs::write(&pdf_source, b"pdf").unwrap();
        db.add_asset_for_format(
            1,
            pdf,
            None,
            "reference",
            &pdf_source.to_string_lossy(),
            None,
            3,
            3,
            None,
            false,
            "2026-01-01",
        )
        .unwrap();
        let store = ManagedObjectStore::from_config(&config);
        adopt_format(
            &db,
            &store,
            AdoptFormatRequest {
                book_id: 1,
                format: "epub".into(),
                reference_asset_id: None,
            },
        )
        .unwrap();
        let assets = db.list_assets_for_book(1).unwrap();
        assert_eq!(assets.iter().filter(|asset| asset.is_compressed).count(), 0);
        assert_eq!(
            assets
                .iter()
                .filter(|asset| asset.book_format_id == Some(pdf))
                .count(),
            1
        );
        assert!(
            adopt_format(
                &db,
                &store,
                AdoptFormatRequest {
                    book_id: 1,
                    format: "mobi".into(),
                    reference_asset_id: None,
                },
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_invalid_candidates_without_mutating_asset_rows() {
        let (dir, config, db) = fixture(false);
        let store = ManagedObjectStore::from_config(&config);
        let original_count = db.list_assets_for_book(1).unwrap().len();
        assert!(
            adopt_format(
                &db,
                &store,
                AdoptFormatRequest {
                    book_id: 999,
                    format: "epub".into(),
                    reference_asset_id: None,
                }
            )
            .is_err()
        );
        assert!(
            adopt_format(
                &db,
                &store,
                AdoptFormatRequest {
                    book_id: 1,
                    format: "mobi".into(),
                    reference_asset_id: None,
                }
            )
            .is_err()
        );
        assert!(
            adopt_format(
                &db,
                &store,
                AdoptFormatRequest {
                    book_id: 1,
                    format: "epub".into(),
                    reference_asset_id: Some(999),
                }
            )
            .is_err()
        );
        fs::remove_file(dir.path().join("legacy.epub")).unwrap();
        assert!(
            adopt_format(
                &db,
                &store,
                AdoptFormatRequest {
                    book_id: 1,
                    format: "epub".into(),
                    reference_asset_id: None,
                }
            )
            .is_err()
        );
        assert_eq!(db.list_assets_for_book(1).unwrap().len(), original_count);
    }

    #[test]
    fn rejects_broken_existing_managed_asset() {
        let (dir, config, db) = fixture(false);
        let logical = db.get_book_format(1, "epub").unwrap().unwrap();
        let missing = dir.path().join("missing-managed.epub");
        db.add_asset_for_format(
            1,
            logical.id,
            None,
            "copy",
            &missing.to_string_lossy(),
            None,
            4096,
            4096,
            Some("not-a-real-checksum"),
            false,
            "2026-01-02",
        )
        .unwrap();
        let store = ManagedObjectStore::from_config(&config);
        assert!(
            adopt_format(
                &db,
                &store,
                AdoptFormatRequest {
                    book_id: 1,
                    format: "epub".into(),
                    reference_asset_id: None,
                }
            )
            .is_err()
        );
    }
}
