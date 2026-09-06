//! Read-only source dependency and managed-replacement audit.

use caliberate_assets::compression::decompress_to_writer;
use caliberate_assets::hashing::{hash_file_sha256, hash_zstd_file_sha256};
use caliberate_core::error::{CoreError, CoreResult};
use caliberate_db::database::{Database, LibrarySourceRow, SourceManagedCandidate};
use std::fs;
use std::io::sink;
use std::path::{Path, PathBuf};

pub const MAX_PAGE_SIZE: usize = 500;
pub const MAX_PROBLEM_LIMIT: usize = 1_000;

#[derive(Debug, Clone)]
pub struct SourceRetirementAuditOptions {
    pub verify_managed: bool,
    pub page_size: usize,
    pub problem_limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceRetirementAuditProgress {
    CatalogMetricComplete {
        name: &'static str,
        elapsed_ms: u128,
    },
    CatalogCountsComplete,
    VerificationStarted,
    VerificationPageComplete {
        processed: u64,
        verified: u64,
    },
    VerificationComplete {
        processed: u64,
        verified: u64,
    },
}

impl Default for SourceRetirementAuditOptions {
    fn default() -> Self {
        Self {
            verify_managed: false,
            page_size: MAX_PAGE_SIZE,
            problem_limit: 50,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SourceRetirementProblem {
    pub kind: String,
    pub book_id: Option<i64>,
    pub book_format_id: Option<i64>,
    pub format: Option<String>,
    pub asset_id: Option<i64>,
    pub path: Option<PathBuf>,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct SourceRetirementAudit {
    pub source: LibrarySourceRow,
    pub mapped_books: u64,
    pub source_reference_assets: u64,
    pub source_backed_formats: u64,
    pub managed_backed_formats: u64,
    pub source_dependent_formats: u64,
    pub metadata_only_source_books: u64,
    pub fully_managed_source_books: u64,
    pub source_books_with_dependencies: u64,
    pub unlinked_source_assets: u64,
    pub orphan_source_assets: u64,
    pub managed_coverage_percent: f64,
    pub catalog_ready: bool,
    pub verification_performed: bool,
    pub managed_candidates_verified: u64,
    pub missing_managed_files: u64,
    pub managed_paths_outside_root: u64,
    pub stored_size_mismatches: u64,
    pub logical_size_mismatches: u64,
    pub missing_checksums: u64,
    pub checksum_mismatches: u64,
    pub decode_errors: u64,
    pub verification_errors: u64,
    pub retirement_ready: bool,
    pub problems: Vec<SourceRetirementProblem>,
}

pub fn audit_source(
    db: &Database,
    managed_root: &Path,
    source_id: i64,
    options: SourceRetirementAuditOptions,
) -> CoreResult<SourceRetirementAudit> {
    audit_source_with_progress(db, managed_root, source_id, options, |_| {})
}

pub fn audit_source_with_progress<F>(
    db: &Database,
    managed_root: &Path,
    source_id: i64,
    options: SourceRetirementAuditOptions,
    mut progress: F,
) -> CoreResult<SourceRetirementAudit>
where
    F: FnMut(SourceRetirementAuditProgress),
{
    let source = db.get_library_source(source_id)?.ok_or_else(|| {
        CoreError::ConfigValidate(format!("library source {source_id} does not exist"))
    })?;
    let counts = db.audit_source_counts_with_timings(source_id, |name, elapsed_ms| {
        progress(SourceRetirementAuditProgress::CatalogMetricComplete { name, elapsed_ms });
    })?;
    progress(SourceRetirementAuditProgress::CatalogCountsComplete);
    let catalog_ready = counts.source_dependent_formats == 0
        && counts.unlinked_source_assets == 0
        && counts.orphan_source_assets == 0;
    let managed_coverage_percent = if counts.source_backed_formats == 0 {
        100.0
    } else {
        counts.managed_backed_formats as f64 / counts.source_backed_formats as f64 * 100.0
    };
    let mut audit = SourceRetirementAudit {
        source,
        mapped_books: counts.mapped_books,
        source_reference_assets: counts.source_reference_assets,
        source_backed_formats: counts.source_backed_formats,
        managed_backed_formats: counts.managed_backed_formats,
        source_dependent_formats: counts.source_dependent_formats,
        metadata_only_source_books: counts.metadata_only_source_books,
        fully_managed_source_books: counts.fully_managed_source_books,
        source_books_with_dependencies: counts.source_books_with_dependencies,
        unlinked_source_assets: counts.unlinked_source_assets,
        orphan_source_assets: counts.orphan_source_assets,
        managed_coverage_percent,
        catalog_ready,
        verification_performed: options.verify_managed,
        managed_candidates_verified: 0,
        missing_managed_files: 0,
        managed_paths_outside_root: 0,
        stored_size_mismatches: 0,
        logical_size_mismatches: 0,
        missing_checksums: 0,
        checksum_mismatches: 0,
        decode_errors: 0,
        verification_errors: 0,
        retirement_ready: false,
        problems: Vec::new(),
    };

    if options.verify_managed {
        let page_size = options.page_size.clamp(1, MAX_PAGE_SIZE);
        let problem_limit = options.problem_limit.min(MAX_PROBLEM_LIMIT);
        let mut cursor = None;
        let mut processed = 0;
        progress(SourceRetirementAuditProgress::VerificationStarted);
        loop {
            let page = db.list_source_managed_candidates(source_id, cursor, page_size)?;
            if page.is_empty() {
                break;
            }
            for candidate in &page {
                verify_candidate(&mut audit, managed_root, candidate, problem_limit);
            }
            processed += page.len() as u64;
            progress(SourceRetirementAuditProgress::VerificationPageComplete {
                processed,
                verified: audit.managed_candidates_verified,
            });
            let last = page.last().expect("non-empty candidate page");
            cursor = Some((last.book_format_id, last.asset_id));
        }
        progress(SourceRetirementAuditProgress::VerificationComplete {
            processed,
            verified: audit.managed_candidates_verified,
        });
    }
    audit.retirement_ready = audit.verification_performed
        && audit.catalog_ready
        && audit.managed_candidates_verified == audit.source_backed_formats
        && audit.missing_managed_files == 0
        && audit.managed_paths_outside_root == 0
        && audit.stored_size_mismatches == 0
        && audit.logical_size_mismatches == 0
        && audit.missing_checksums == 0
        && audit.checksum_mismatches == 0
        && audit.decode_errors == 0
        && audit.verification_errors == 0;
    Ok(audit)
}

fn verify_candidate(
    audit: &mut SourceRetirementAudit,
    managed_root: &Path,
    candidate: &SourceManagedCandidate,
    problem_limit: usize,
) {
    let path = Path::new(&candidate.stored_path);
    let mut failed = false;
    let mut problem = |kind: &str, message: String| {
        failed = true;
        audit.verification_errors += 1;
        if audit.problems.len() < problem_limit {
            audit.problems.push(SourceRetirementProblem {
                kind: kind.to_string(),
                book_id: Some(candidate.book_id),
                book_format_id: Some(candidate.book_format_id),
                format: Some(candidate.format.clone()),
                asset_id: Some(candidate.asset_id),
                path: Some(path.to_path_buf()),
                message,
            });
        }
    };

    if !path.starts_with(managed_root) {
        audit.managed_paths_outside_root += 1;
        problem(
            "path-outside-managed-root",
            "managed path is outside configured root".into(),
        );
        return;
    }
    if !path.exists() {
        audit.missing_managed_files += 1;
        problem("missing-managed-file", "managed file does not exist".into());
        return;
    }
    let metadata = match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => metadata,
        Ok(_) => {
            audit.missing_managed_files += 1;
            problem(
                "not-regular-file",
                "managed path is not a regular file".into(),
            );
            return;
        }
        Err(error) => {
            audit.missing_managed_files += 1;
            problem(
                "managed-file-error",
                format!("cannot inspect managed file: {error}"),
            );
            return;
        }
    };
    if metadata.len() != candidate.stored_size_bytes {
        audit.stored_size_mismatches += 1;
        problem(
            "stored-size-mismatch",
            format!(
                "expected {}, found {}",
                candidate.stored_size_bytes,
                metadata.len()
            ),
        );
    }
    let logical_size = if candidate.is_compressed {
        match decompress_to_writer(path, &mut sink()) {
            Ok(size) => Some(size),
            Err(error) => {
                audit.decode_errors += 1;
                problem("decode-error", error.to_string());
                None
            }
        }
    } else {
        Some(metadata.len())
    };
    if let Some(size) = logical_size {
        if size != candidate.size_bytes {
            audit.logical_size_mismatches += 1;
            problem(
                "logical-size-mismatch",
                format!("expected {}, found {}", candidate.size_bytes, size),
            );
        }
    }
    let Some(expected_checksum) = candidate.checksum.as_deref() else {
        audit.missing_checksums += 1;
        problem("missing-checksum", "managed asset has no checksum".into());
        return;
    };
    let checksum = if candidate.is_compressed {
        hash_zstd_file_sha256(path)
    } else {
        hash_file_sha256(path)
    };
    match checksum {
        Ok(actual) if actual == expected_checksum => {}
        Ok(actual) => {
            audit.checksum_mismatches += 1;
            problem(
                "checksum-mismatch",
                format!("expected {expected_checksum}, found {actual}"),
            );
        }
        Err(error) => problem("checksum-error", error.to_string()),
    }
    if !failed {
        audit.managed_candidates_verified += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SourceRetirementAuditOptions, SourceRetirementAuditProgress, audit_source,
        audit_source_with_progress,
    };
    use caliberate_core::config::ControlPlane;
    use caliberate_db::database::Database;
    use std::fs;
    use tempfile::TempDir;

    fn fixture() -> (TempDir, ControlPlane, Database, i64) {
        let dir = tempfile::tempdir().unwrap();
        let config_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../config/control-plane.toml");
        let mut config = ControlPlane::load_from_path(config_path).unwrap();
        config.db.sqlite_path = dir.path().join("library.db");
        config.paths.library_dir = dir.path().join("managed");
        let db = Database::open_with_fts(&config.db, &config.fts).unwrap();
        let source_id = db
            .upsert_library_source("calibre", "never-open", Some("fixture"), true)
            .unwrap();
        (dir, config, db, source_id)
    }

    #[test]
    fn catalog_counts_use_only_canonical_rows() {
        let (dir, config, db, source_id) = fixture();
        db.add_book("Book A", "epub", "", "2026-01-01").unwrap();
        db.add_book("Book B", "epub", "", "2026-01-01").unwrap();
        db.add_book("Book C", "", "", "2026-01-01").unwrap();
        db.upsert_source_book(source_id, 1, "a", None, None, None)
            .unwrap();
        db.upsert_source_book(source_id, 2, "b", None, None, None)
            .unwrap();
        db.upsert_source_book(source_id, 3, "c", None, None, None)
            .unwrap();
        let format_a = db.get_book_format(1, "epub").unwrap().unwrap();
        let format_b = db.get_book_format(2, "epub").unwrap().unwrap();
        db.add_asset_for_format(
            1,
            format_a.id,
            Some(source_id),
            "reference",
            "Z:\\never\\a.epub",
            None,
            1,
            1,
            None,
            false,
            "2026",
        )
        .unwrap();
        db.add_asset_for_format(
            2,
            format_b.id,
            Some(source_id),
            "reference",
            "Z:\\never\\b.epub",
            None,
            1,
            1,
            None,
            false,
            "2026",
        )
        .unwrap();
        let counts = db.audit_source_counts(source_id).unwrap();
        assert_eq!(counts.mapped_books, 3);
        assert_eq!(counts.source_reference_assets, 2);
        assert_eq!(counts.source_backed_formats, 2);
        assert_eq!(counts.source_dependent_formats, 2);
        assert_eq!(counts.metadata_only_source_books, 1);
        assert_eq!(counts.source_books_with_dependencies, 2);
        assert!(
            !audit_source(
                &db,
                &config.paths.library_dir,
                source_id,
                Default::default()
            )
            .unwrap()
            .catalog_ready
        );
        assert!(!dir.path().join("never-open").exists());

        db.add_asset_for_format(
            1,
            format_a.id,
            None,
            "copy",
            "managed-a.epub",
            None,
            1,
            1,
            Some("a"),
            false,
            "2026",
        )
        .unwrap();
        db.add_asset_for_format(
            2,
            format_b.id,
            None,
            "copy",
            "managed-b.epub",
            None,
            1,
            1,
            Some("b"),
            false,
            "2026",
        )
        .unwrap();
        let repaired = audit_source(
            &db,
            &config.paths.library_dir,
            source_id,
            Default::default(),
        )
        .unwrap();
        assert_eq!(repaired.mapped_books, 3);
        assert_eq!(repaired.source_backed_formats, 2);
        assert_eq!(repaired.managed_backed_formats, 2);
        assert_eq!(repaired.source_dependent_formats, 0);
        assert_eq!(repaired.metadata_only_source_books, 1);
        assert_eq!(repaired.fully_managed_source_books, 2);
        assert_eq!(repaired.source_books_with_dependencies, 0);
        assert!(repaired.catalog_ready);
    }

    #[test]
    fn verification_is_bounded_and_never_requires_reference_files() {
        let (dir, config, db, source_id) = fixture();
        db.add_book("Book", "epub", "", "2026-01-01").unwrap();
        db.upsert_source_book(source_id, 1, "book", None, None, None)
            .unwrap();
        let format = db.get_book_format(1, "epub").unwrap().unwrap();
        let managed = dir.path().join("managed/ok.epub");
        fs::create_dir_all(managed.parent().unwrap()).unwrap();
        fs::write(&managed, b"bytes").unwrap();
        let checksum = caliberate_assets::hashing::hash_file_sha256(&managed).unwrap();
        db.add_asset_for_format(
            1,
            format.id,
            Some(source_id),
            "reference",
            "Z:\\missing\\legacy.epub",
            None,
            5,
            5,
            None,
            false,
            "2026",
        )
        .unwrap();
        db.add_asset_for_format(
            1,
            format.id,
            None,
            "copy",
            &managed.to_string_lossy(),
            None,
            5,
            5,
            Some(&checksum),
            false,
            "2026",
        )
        .unwrap();
        let mut options = SourceRetirementAuditOptions {
            verify_managed: true,
            page_size: usize::MAX,
            problem_limit: 0,
        };
        let audit =
            audit_source(&db, &config.paths.library_dir, source_id, options.clone()).unwrap();
        assert_eq!(audit.managed_candidates_verified, 1);
        assert!(audit.retirement_ready);
        options.problem_limit = 1;
        fs::write(&managed, b"bad").unwrap();
        let audit = audit_source(&db, &config.paths.library_dir, source_id, options).unwrap();
        assert_eq!(audit.problems.len(), 1);
        assert_eq!(audit.checksum_mismatches, 1);
        assert!(!audit.retirement_ready);
    }

    #[test]
    fn audit_progress_reports_phase_order_and_catalog_only_boundary() {
        let (_dir, config, db, source_id) = fixture();
        let mut catalog_only = Vec::new();
        audit_source_with_progress(
            &db,
            &config.paths.library_dir,
            source_id,
            Default::default(),
            |event| catalog_only.push(event),
        )
        .unwrap();
        assert_eq!(
            catalog_only
                .iter()
                .filter(|event| {
                    matches!(event, SourceRetirementAuditProgress::CatalogCountsComplete)
                })
                .count(),
            1
        );
        assert!(!catalog_only.iter().any(|event| {
            matches!(
                event,
                SourceRetirementAuditProgress::VerificationStarted
                    | SourceRetirementAuditProgress::VerificationPageComplete { .. }
                    | SourceRetirementAuditProgress::VerificationComplete { .. }
            )
        }));
        assert!(catalog_only.iter().any(|event| {
            matches!(
                event,
                SourceRetirementAuditProgress::CatalogMetricComplete { .. }
            )
        }));

        for book_id in 1..=2 {
            db.add_book(&format!("Book {book_id}"), "epub", "", "2026")
                .unwrap();
            db.upsert_source_book(source_id, book_id, &book_id.to_string(), None, None, None)
                .unwrap();
            let format = db.get_book_format(book_id, "epub").unwrap().unwrap();
            db.add_asset_for_format(
                book_id,
                format.id,
                Some(source_id),
                "reference",
                &format!("Z:\\never-open\\legacy-{book_id}.epub"),
                None,
                4,
                4,
                None,
                false,
                "2026",
            )
            .unwrap();
            let managed = config
                .paths
                .library_dir
                .join(format!("managed-{book_id}.epub"));
            fs::create_dir_all(managed.parent().unwrap()).unwrap();
            fs::write(&managed, b"good").unwrap();
            let checksum = caliberate_assets::hashing::hash_file_sha256(&managed).unwrap();
            db.add_asset_for_format(
                book_id,
                format.id,
                None,
                "copy",
                &managed.to_string_lossy(),
                None,
                4,
                4,
                Some(&checksum),
                false,
                "2026",
            )
            .unwrap();
        }

        let mut events = Vec::new();
        let audit = audit_source_with_progress(
            &db,
            &config.paths.library_dir,
            source_id,
            SourceRetirementAuditOptions {
                verify_managed: true,
                page_size: 1,
                ..Default::default()
            },
            |event| events.push(event),
        )
        .unwrap();
        assert_eq!(audit.managed_candidates_verified, 2);
        let phase_events = events
            .into_iter()
            .filter(|event| {
                !matches!(
                    event,
                    SourceRetirementAuditProgress::CatalogMetricComplete { .. }
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            phase_events,
            vec![
                SourceRetirementAuditProgress::CatalogCountsComplete,
                SourceRetirementAuditProgress::VerificationStarted,
                SourceRetirementAuditProgress::VerificationPageComplete {
                    processed: 1,
                    verified: 1,
                },
                SourceRetirementAuditProgress::VerificationPageComplete {
                    processed: 2,
                    verified: 2,
                },
                SourceRetirementAuditProgress::VerificationComplete {
                    processed: 2,
                    verified: 2,
                },
            ]
        );
    }

    #[test]
    fn verifies_all_managed_failure_classes_with_bounded_problems() {
        use caliberate_assets::compression::compress_file;
        use std::fs;

        let (dir, config, db, source_id) = fixture();
        let root = &config.paths.library_dir;
        fs::create_dir_all(root).unwrap();
        let checksum =
            |path: &std::path::Path| caliberate_assets::hashing::hash_file_sha256(path).unwrap();
        let add = |book_id: i64,
                   path: &std::path::Path,
                   logical_size: u64,
                   stored_size: u64,
                   checksum: Option<&str>,
                   compressed: bool| {
            db.add_book(&format!("Book {book_id}"), "epub", "", "2026")
                .unwrap();
            db.upsert_source_book(source_id, book_id, &book_id.to_string(), None, None, None)
                .unwrap();
            let format = db.get_book_format(book_id, "epub").unwrap().unwrap();
            db.add_asset_for_format(
                book_id,
                format.id,
                Some(source_id),
                "reference",
                &format!("Z:\\never-open\\legacy-{book_id}.epub"),
                None,
                4,
                4,
                None,
                false,
                "2026",
            )
            .unwrap();
            db.add_asset_for_format(
                book_id,
                format.id,
                None,
                "copy",
                &path.to_string_lossy(),
                None,
                logical_size,
                stored_size,
                checksum,
                compressed,
                "2026",
            )
            .unwrap();
        };

        let healthy = root.join("healthy.epub");
        fs::write(&healthy, b"good").unwrap();
        let healthy_checksum = checksum(&healthy);
        add(1, &healthy, 4, 4, Some(&healthy_checksum), false);

        let zstd_source = dir.path().join("synthetic-zstd-source.epub");
        fs::write(&zstd_source, b"zstd").unwrap();
        let zstd = root.join("healthy.epub.zst");
        compress_file(&zstd_source, &zstd, 3).unwrap();
        let zstd_checksum = checksum(&zstd_source);
        add(
            2,
            &zstd,
            4,
            fs::metadata(&zstd).unwrap().len(),
            Some(&zstd_checksum),
            true,
        );

        add(3, &root.join("missing.epub"), 4, 4, Some("missing"), false);

        let outside = dir.path().join("outside.epub");
        fs::write(&outside, b"good").unwrap();
        add(4, &outside, 4, 4, Some(&healthy_checksum), false);

        let stored_mismatch = root.join("stored-mismatch.epub");
        fs::write(&stored_mismatch, b"good").unwrap();
        add(5, &stored_mismatch, 4, 99, Some(&healthy_checksum), false);

        let logical_mismatch = root.join("logical-mismatch.epub");
        fs::write(&logical_mismatch, b"bad!").unwrap();
        add(
            6,
            &logical_mismatch,
            5,
            4,
            Some(&checksum(&logical_mismatch)),
            false,
        );

        let missing_checksum = root.join("missing-checksum.epub");
        fs::write(&missing_checksum, b"good").unwrap();
        add(7, &missing_checksum, 4, 4, None, false);

        let checksum_mismatch = root.join("checksum-mismatch.epub");
        fs::write(&checksum_mismatch, b"good").unwrap();
        add(8, &checksum_mismatch, 4, 4, Some(&"0".repeat(64)), false);

        let corrupt_zstd = root.join("corrupt.epub.zst");
        fs::write(&corrupt_zstd, b"not zstd").unwrap();
        add(9, &corrupt_zstd, 4, 8, Some("corrupt"), true);

        let audit = audit_source(
            &db,
            root,
            source_id,
            SourceRetirementAuditOptions {
                verify_managed: true,
                page_size: 1,
                problem_limit: 2,
            },
        )
        .unwrap();
        assert_eq!(audit.source_backed_formats, 9);
        assert_eq!(audit.managed_backed_formats, 9);
        assert_eq!(audit.managed_candidates_verified, 2);
        assert_eq!(audit.missing_managed_files, 1);
        assert_eq!(audit.managed_paths_outside_root, 1);
        assert_eq!(audit.stored_size_mismatches, 1);
        assert_eq!(audit.logical_size_mismatches, 1);
        assert_eq!(audit.missing_checksums, 1);
        assert_eq!(audit.checksum_mismatches, 1);
        assert_eq!(audit.decode_errors, 1);
        assert_eq!(audit.verification_errors, 8);
        assert_eq!(audit.problems.len(), 2);
        assert!(audit.catalog_ready);
        assert!(!audit.retirement_ready);
    }

    #[test]
    fn scaled_audit_uses_production_path_without_materializing_source_ids() {
        let (_dir, config, db, source_id) = fixture();
        // Twenty-five thousand rows materially exercises the aggregate
        // production path while keeping the native Windows suite practical.
        const BOOKS: i64 = 25_000;
        let db_path = config.db.sqlite_path.clone();
        drop(db);
        let mut connection = rusqlite::Connection::open(&db_path).unwrap();
        let transaction = connection.transaction().unwrap();
        // This synthetic-only bulk fixture bypasses the canonical insert
        // trigger's Calibre title_sort function; the audit reads these rows
        // without relying on insert-time metadata behavior.
        transaction
            .execute_batch("DROP TRIGGER books_insert_trg;")
            .unwrap();
        for book_id in 1..=BOOKS {
            transaction
                .execute(
                    "INSERT INTO books(id,title,format,path,created_at) VALUES (?1,?2,'epub','', '2026')",
                    rusqlite::params![book_id, format!("Scaled {book_id}")],
                )
                .unwrap();
            transaction
                .execute(
                    "INSERT INTO book_formats(id,book_id,format,size_bytes) VALUES (?1,?1,'epub',1)",
                    rusqlite::params![book_id],
                )
                .unwrap();
            transaction
                .execute(
                    "INSERT INTO source_books(id,source_id,book_id,external_id) VALUES (?1,?2,?1,?3)",
                    rusqlite::params![book_id, source_id, book_id.to_string()],
                )
                .unwrap();
            transaction
                .execute(
                    "INSERT INTO assets(id,book_id,book_format_id,source_id,storage_mode,stored_path,size_bytes,stored_size_bytes,is_compressed,created_at) VALUES (?1,?1,?1,?2,'reference',?3,1,1,0,'2026')",
                    rusqlite::params![book_id, source_id, format!("Z:\\never-open\\scaled-{book_id}.epub")],
                )
                .unwrap();
            if book_id % 1_000 == 0 {
                transaction
                    .execute(
                        "INSERT INTO assets(id,book_id,book_format_id,source_id,storage_mode,stored_path,size_bytes,stored_size_bytes,is_compressed,created_at) VALUES (?1,?2,?2,NULL,'copy',?3,1,1,0,'2026')",
                        rusqlite::params![
                            BOOKS + book_id,
                            book_id,
                            format!("Z:\\never-open\\managed-{book_id}.epub")
                        ],
                    )
                    .unwrap();
            }
        }
        transaction.commit().unwrap();
        let db = Database::open_with_fts(&config.db, &config.fts).unwrap();
        let audit = audit_source(
            &db,
            &config.paths.library_dir,
            source_id,
            Default::default(),
        )
        .unwrap();
        assert_eq!(audit.mapped_books, BOOKS as u64);
        assert_eq!(audit.source_reference_assets, BOOKS as u64);
        assert_eq!(audit.source_backed_formats, BOOKS as u64);
        assert_eq!(audit.managed_backed_formats, 25);
        assert_eq!(audit.source_dependent_formats, BOOKS as u64 - 25);
        assert_eq!(audit.metadata_only_source_books, 0);
        assert_eq!(audit.fully_managed_source_books, 25);
        assert_eq!(audit.source_books_with_dependencies, BOOKS as u64 - 25);
        assert_eq!(audit.unlinked_source_assets, 0);
        assert_eq!(audit.orphan_source_assets, 0);
        assert!(!audit.catalog_ready);
    }
}
