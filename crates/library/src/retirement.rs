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
    let source = db.get_library_source(source_id)?.ok_or_else(|| {
        CoreError::ConfigValidate(format!("library source {source_id} does not exist"))
    })?;
    let counts = db.audit_source_counts(source_id)?;
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
        loop {
            let page = db.list_source_managed_candidates(source_id, cursor, page_size)?;
            if page.is_empty() {
                break;
            }
            for candidate in &page {
                verify_candidate(&mut audit, managed_root, candidate, problem_limit);
            }
            let last = page.last().expect("non-empty candidate page");
            cursor = Some((last.book_format_id, last.asset_id));
        }
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
    use super::{SourceRetirementAuditOptions, audit_source};
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
        db.upsert_source_book(source_id, 1, "a", None, None, None)
            .unwrap();
        db.upsert_source_book(source_id, 2, "b", None, None, None)
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
        assert_eq!(counts.mapped_books, 2);
        assert_eq!(counts.source_reference_assets, 2);
        assert_eq!(counts.source_backed_formats, 2);
        assert_eq!(counts.source_dependent_formats, 2);
        assert_eq!(counts.metadata_only_source_books, 0);
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
}
