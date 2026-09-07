//! Bounded, restartable orchestration over the single-format adoption service.

use crate::adopt::{AdoptFormatRequest, adopt_format};
use crate::retirement::audit_source;
use caliberate_assets::managed::ManagedObjectStore;
use caliberate_core::error::CoreResult;
use caliberate_db::database::{Database, SourceAdoptionCandidate};
use std::path::Path;

pub const MAX_BULK_ADOPT_PAGE_SIZE: usize = 500;
pub const MAX_BULK_ADOPT_FORMATS: usize = 10_000;
pub const MAX_BULK_ADOPT_PROBLEMS: usize = 1_000;

#[derive(Debug, Clone)]
pub struct SourceBulkAdoptOptions {
    pub apply: bool,
    pub max_formats: usize,
    pub page_size: usize,
    pub problem_limit: usize,
}

impl Default for SourceBulkAdoptOptions {
    fn default() -> Self {
        Self {
            apply: false,
            max_formats: 25,
            page_size: MAX_BULK_ADOPT_PAGE_SIZE,
            problem_limit: 50,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceBulkAdoptProgress {
    SelectionStarted,
    PageComplete {
        selected: u64,
        attempted: u64,
    },
    AdoptionProgress {
        attempted: u64,
        adopted_new: u64,
        failed: u64,
    },
    Complete {
        selected: u64,
        attempted: u64,
        adopted_new: u64,
        failed: u64,
    },
}

#[derive(Debug, Clone)]
pub struct SourceBulkAdoptProblem {
    pub book_id: i64,
    pub book_format_id: i64,
    pub format: String,
    pub reference_asset_id: i64,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct SourceBulkAdoptResult {
    pub source_id: i64,
    pub apply: bool,
    pub selected: u64,
    pub attempted: u64,
    pub adopted_new: u64,
    pub already_adopted: u64,
    pub reused_existing_objects: u64,
    pub failed: u64,
    pub logical_bytes_adopted: u64,
    pub stored_bytes_adopted: u64,
    pub last_book_format_id: Option<i64>,
    pub dependent_formats_before: u64,
    pub dependent_formats_after: u64,
    pub managed_backed_formats_before: u64,
    pub managed_backed_formats_after: u64,
    pub candidates: Vec<SourceAdoptionCandidate>,
    pub problems: Vec<SourceBulkAdoptProblem>,
}

pub fn bulk_adopt_source<F>(
    db: &Database,
    store: &ManagedObjectStore,
    managed_root: &Path,
    source_id: i64,
    options: SourceBulkAdoptOptions,
    mut progress: F,
) -> CoreResult<SourceBulkAdoptResult>
where
    F: FnMut(SourceBulkAdoptProgress),
{
    let before = audit_source(db, managed_root, source_id, Default::default())?;
    let max_formats = options.max_formats.min(MAX_BULK_ADOPT_FORMATS);
    let page_size = options.page_size.clamp(1, MAX_BULK_ADOPT_PAGE_SIZE);
    let problem_limit = options.problem_limit.min(MAX_BULK_ADOPT_PROBLEMS);
    let mut result = SourceBulkAdoptResult {
        source_id,
        apply: options.apply,
        selected: 0,
        attempted: 0,
        adopted_new: 0,
        already_adopted: 0,
        reused_existing_objects: 0,
        failed: 0,
        logical_bytes_adopted: 0,
        stored_bytes_adopted: 0,
        last_book_format_id: None,
        dependent_formats_before: before.source_dependent_formats,
        dependent_formats_after: before.source_dependent_formats,
        managed_backed_formats_before: before.managed_backed_formats,
        managed_backed_formats_after: before.managed_backed_formats,
        candidates: Vec::new(),
        problems: Vec::new(),
    };
    progress(SourceBulkAdoptProgress::SelectionStarted);
    let mut cursor = None;
    while result.selected < max_formats as u64 {
        let remaining = max_formats - result.selected as usize;
        let page =
            db.list_source_adoption_candidates(source_id, cursor, page_size.min(remaining))?;
        if page.is_empty() {
            break;
        }
        for candidate in &page {
            if result.selected >= max_formats as u64 {
                break;
            }
            result.selected += 1;
            result.last_book_format_id = Some(candidate.book_format_id);
            result.candidates.push(candidate.clone());
            if options.apply {
                result.attempted += 1;
                match adopt_format(
                    db,
                    store,
                    AdoptFormatRequest {
                        book_id: candidate.book_id,
                        format: candidate.format.clone(),
                        reference_asset_id: Some(candidate.reference_asset_id),
                    },
                ) {
                    Ok(adopted) => {
                        if adopted.already_adopted {
                            result.already_adopted += 1;
                        } else {
                            result.adopted_new += 1;
                        }
                        if adopted.reused_existing_object {
                            result.reused_existing_objects += 1;
                        }
                        result.logical_bytes_adopted += adopted.logical_size_bytes;
                        result.stored_bytes_adopted += adopted.stored_size_bytes;
                    }
                    Err(error) => {
                        result.failed += 1;
                        if result.problems.len() < problem_limit {
                            result.problems.push(SourceBulkAdoptProblem {
                                book_id: candidate.book_id,
                                book_format_id: candidate.book_format_id,
                                format: candidate.format.clone(),
                                reference_asset_id: candidate.reference_asset_id,
                                message: error.to_string(),
                            });
                        }
                    }
                }
            }
            cursor = Some((candidate.book_format_id, candidate.reference_asset_id));
        }
        progress(SourceBulkAdoptProgress::PageComplete {
            selected: result.selected,
            attempted: result.attempted,
        });
        if options.apply {
            progress(SourceBulkAdoptProgress::AdoptionProgress {
                attempted: result.attempted,
                adopted_new: result.adopted_new,
                failed: result.failed,
            });
        }
    }
    if options.apply {
        let after = audit_source(db, managed_root, source_id, Default::default())?;
        result.dependent_formats_after = after.source_dependent_formats;
        result.managed_backed_formats_after = after.managed_backed_formats;
    }
    progress(SourceBulkAdoptProgress::Complete {
        selected: result.selected,
        attempted: result.attempted,
        adopted_new: result.adopted_new,
        failed: result.failed,
    });
    Ok(result)
}

pub fn normalize_bulk_limit(requested: usize) -> usize {
    requested.min(MAX_BULK_ADOPT_FORMATS)
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_BULK_ADOPT_FORMATS, SourceBulkAdoptOptions, SourceBulkAdoptProgress, bulk_adopt_source,
        normalize_bulk_limit,
    };
    use crate::adopt::{AdoptFormatRequest, adopt_format};
    use caliberate_assets::managed::ManagedObjectStore;
    use caliberate_core::config::ControlPlane;
    use caliberate_db::database::Database;
    use std::fs;
    use tempfile::TempDir;

    fn fixture() -> (TempDir, ControlPlane, Database, i64, i64) {
        let dir = tempfile::tempdir().unwrap();
        let config_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../config/control-plane.toml");
        let mut config = ControlPlane::load_from_path(config_path).unwrap();
        config.db.sqlite_path = dir.path().join("library.db");
        config.paths.library_dir = dir.path().join("managed");
        config.assets.compress_raw_assets = false;
        let db = Database::open_with_fts(&config.db, &config.fts).unwrap();
        let other_source = db
            .upsert_library_source("calibre", "other", Some("other"), true)
            .unwrap();
        let target_source = db
            .upsert_library_source("calibre", "target", Some("target"), true)
            .unwrap();
        (dir, config, db, other_source, target_source)
    }

    fn add_book_with_reference(db: &Database, source_id: i64, book_id: i64, path: &str) -> i64 {
        let format = db.get_book_format(book_id, "epub").unwrap().unwrap();
        db.add_asset_for_format(
            book_id,
            format.id,
            Some(source_id),
            "reference",
            path,
            None,
            4,
            4,
            None,
            false,
            "2026",
        )
        .unwrap()
    }

    fn add_book(db: &Database, source_id: i64, title: &str, path: &str) -> (i64, i64) {
        let book_id = db.add_book(title, "epub", "", "2026").unwrap();
        db.upsert_source_book(source_id, book_id, &book_id.to_string(), None, None, None)
            .unwrap();
        let asset_id = add_book_with_reference(db, source_id, book_id, path);
        (book_id, asset_id)
    }

    #[test]
    fn candidates_are_source_specific_lowest_id_paged_and_exclude_managed() {
        let (dir, config, db, other_source, target_source) = fixture();
        let first_path = dir.path().join("first.epub");
        let other_path = dir.path().join("other.epub");
        fs::write(&first_path, b"one").unwrap();
        fs::write(&other_path, b"two").unwrap();
        let first_book = db.add_book("First", "epub", "", "2026").unwrap();
        db.upsert_source_book(target_source, first_book, "first", None, None, None)
            .unwrap();
        db.upsert_source_book(other_source, first_book, "first", None, None, None)
            .unwrap();
        let other_asset =
            add_book_with_reference(&db, other_source, first_book, &other_path.to_string_lossy());
        let target_low = add_book_with_reference(
            &db,
            target_source,
            first_book,
            &first_path.to_string_lossy(),
        );
        let target_high = add_book_with_reference(
            &db,
            target_source,
            first_book,
            &other_path.to_string_lossy(),
        );
        assert!(target_low < target_high);
        assert!(other_asset < target_high);
        let (second_book, _) = add_book(&db, target_source, "Second", "missing-second.epub");
        let (third_book, _) = add_book(&db, target_source, "Third", &other_path.to_string_lossy());
        let (fourth_book, _) = add_book(&db, target_source, "Fourth", "missing-fourth.epub");
        let store = ManagedObjectStore::from_config(&config);
        adopt_format(
            &db,
            &store,
            AdoptFormatRequest {
                book_id: third_book,
                format: "epub".into(),
                reference_asset_id: None,
            },
        )
        .unwrap();
        let first_page = db
            .list_source_adoption_candidates(target_source, None, 2)
            .unwrap();
        assert_eq!(first_page.len(), 2);
        assert_eq!(first_page[0].book_id, first_book);
        assert_eq!(first_page[0].reference_asset_id, target_low);
        assert_eq!(first_page[1].book_id, second_book);
        let second_page = db
            .list_source_adoption_candidates(
                target_source,
                Some((
                    first_page[1].book_format_id,
                    first_page[1].reference_asset_id,
                )),
                2,
            )
            .unwrap();
        assert_eq!(second_page.len(), 1);
        assert_eq!(second_page[0].book_id, fourth_book);
    }

    #[test]
    fn dry_run_apply_failures_restart_and_progress_are_bounded() {
        let (dir, config, db, _other_source, target_source) = fixture();
        let valid_one = dir.path().join("one.epub");
        let valid_two = dir.path().join("two.epub");
        fs::write(&valid_one, b"same").unwrap();
        fs::write(&valid_two, b"same").unwrap();
        let (_book_one, _) = add_book(&db, target_source, "One", &valid_one.to_string_lossy());
        let (_book_two, _) = add_book(&db, target_source, "Two", "missing.epub");
        let (_book_three, _) = add_book(&db, target_source, "Three", &valid_two.to_string_lossy());
        let store = ManagedObjectStore::from_config(&config);
        let assets_before = db.list_assets_for_book(1).unwrap().len()
            + db.list_assets_for_book(2).unwrap().len()
            + db.list_assets_for_book(3).unwrap().len();
        let mut dry_events = Vec::new();
        let dry = bulk_adopt_source(
            &db,
            &store,
            &config.paths.library_dir,
            target_source,
            SourceBulkAdoptOptions {
                page_size: 1,
                max_formats: 10_000,
                ..Default::default()
            },
            |event| dry_events.push(event),
        )
        .unwrap();
        assert_eq!(dry.selected, 3);
        assert_eq!(dry.attempted, 0);
        assert_eq!(dry.failed, 0);
        assert_eq!(dry.dependent_formats_before, 3);
        assert_eq!(dry.dependent_formats_after, 3);
        let assets_after_dry = db.list_assets_for_book(1).unwrap().len()
            + db.list_assets_for_book(2).unwrap().len()
            + db.list_assets_for_book(3).unwrap().len();
        assert_eq!(assets_after_dry, assets_before);
        assert!(
            dry_events
                .iter()
                .any(|event| { matches!(event, SourceBulkAdoptProgress::PageComplete { .. }) })
        );

        let applied = bulk_adopt_source(
            &db,
            &store,
            &config.paths.library_dir,
            target_source,
            SourceBulkAdoptOptions {
                apply: true,
                page_size: 2,
                ..Default::default()
            },
            |_| {},
        )
        .unwrap();
        assert_eq!(applied.selected, 3);
        assert_eq!(applied.attempted, 3);
        assert_eq!(applied.adopted_new, 2);
        assert_eq!(applied.failed, 1);
        assert_eq!(applied.problems.len(), 1);
        assert_eq!(applied.dependent_formats_before, 3);
        assert_eq!(applied.dependent_formats_after, 1);
        assert_eq!(applied.managed_backed_formats_after, 2);
        assert_eq!(applied.candidates.len(), 3);
        assert_eq!(applied.reused_existing_objects, 1);
        assert_eq!(db.list_assets_for_book(1).unwrap().len(), 2);
        assert_eq!(db.list_assets_for_book(2).unwrap().len(), 1);
        assert_eq!(db.list_assets_for_book(3).unwrap().len(), 2);

        let resumed = bulk_adopt_source(
            &db,
            &store,
            &config.paths.library_dir,
            target_source,
            SourceBulkAdoptOptions {
                apply: true,
                problem_limit: 0,
                ..Default::default()
            },
            |_| {},
        )
        .unwrap();
        assert_eq!(resumed.selected, 1);
        assert_eq!(resumed.failed, 1);
        assert_eq!(resumed.dependent_formats_before, 1);
        assert_eq!(resumed.dependent_formats_after, 1);
        assert_eq!(normalize_bulk_limit(usize::MAX), MAX_BULK_ADOPT_FORMATS);
    }
}
