use caliberate_db::database::Database;
use tempfile::TempDir;

#[test]
fn schema_includes_calibre_parity_tables() {
    let (db, _tmp) = open_db();
    let tables = [
        "books_plugin_data",
        "books_pages_link",
        "conversion_options",
        "custom_columns",
        "data",
        "feeds",
        "library_id",
        "metadata_dirtied",
        "annotations_dirtied",
        "preferences",
        "last_read_positions",
        "annotations",
        "annotations_fts",
        "annotations_fts_stemmed",
    ];
    for table in tables {
        assert!(
            db.schema_object_exists("table", table).unwrap_or(false),
            "missing table {table}"
        );
    }
}

#[test]
fn schema_columns_match_calibre_naming() {
    let (db, _tmp) = open_db();
    let comments = table_columns(&db, "comments");
    assert!(comments.contains("id"));
    assert!(comments.contains("book"));
    assert!(comments.contains("text"));
    assert!(!comments.contains("book_id"));

    let identifiers = table_columns(&db, "identifiers");
    assert!(identifiers.contains("id"));
    assert!(identifiers.contains("book"));
    assert!(identifiers.contains("type"));
    assert!(identifiers.contains("val"));
    assert!(!identifiers.contains("identifier_type"));

    let authors_link = table_columns(&db, "books_authors_link");
    assert!(authors_link.contains("book"));
    assert!(authors_link.contains("author"));
    assert!(!authors_link.contains("book_id"));
    assert!(!authors_link.contains("author_id"));

    let tags_link = table_columns(&db, "books_tags_link");
    assert!(tags_link.contains("book"));
    assert!(tags_link.contains("tag"));
    assert!(!tags_link.contains("tag_id"));

    let series_link = table_columns(&db, "books_series_link");
    assert!(series_link.contains("book"));
    assert!(series_link.contains("series"));
    assert!(!series_link.contains("series_id"));
    assert!(!series_link.contains("series_index"));

    let publishers_link = table_columns(&db, "books_publishers_link");
    assert!(publishers_link.contains("book"));
    assert!(publishers_link.contains("publisher"));
    assert!(!publishers_link.contains("publisher_id"));

    let ratings_link = table_columns(&db, "books_ratings_link");
    assert!(ratings_link.contains("book"));
    assert!(ratings_link.contains("rating"));
    assert!(!ratings_link.contains("rating_id"));

    let languages_link = table_columns(&db, "books_languages_link");
    assert!(languages_link.contains("book"));
    assert!(languages_link.contains("lang_code"));
    assert!(!languages_link.contains("language_id"));
}

#[test]
fn schema_includes_calibre_views_triggers_indices() {
    let (db, _tmp) = open_db();
    let views = [
        "meta",
        "tag_browser_authors",
        "tag_browser_filtered_authors",
        "tag_browser_filtered_publishers",
        "tag_browser_filtered_ratings",
        "tag_browser_filtered_series",
        "tag_browser_filtered_tags",
        "tag_browser_publishers",
        "tag_browser_ratings",
        "tag_browser_series",
        "tag_browser_tags",
    ];
    for view in views {
        assert!(
            db.schema_object_exists("view", view).unwrap_or(false),
            "missing view {view}"
        );
    }

    let triggers = [
        "books_delete_trg",
        "books_insert_trg",
        "books_update_trg",
        "fkc_comments_insert",
        "fkc_comments_update",
        "fkc_data_insert",
        "fkc_data_update",
        "fkc_lrp_insert",
        "fkc_lrp_update",
        "fkc_annot_insert",
        "fkc_annot_update",
        "fkc_delete_on_authors",
        "fkc_delete_on_languages",
        "fkc_delete_on_languages_link",
        "fkc_delete_on_publishers",
        "fkc_delete_on_series",
        "fkc_delete_on_tags",
        "fkc_insert_books_authors_link",
        "fkc_insert_books_publishers_link",
        "fkc_insert_books_ratings_link",
        "fkc_insert_books_series_link",
        "fkc_insert_books_tags_link",
        "fkc_update_books_authors_link_a",
        "fkc_update_books_authors_link_b",
        "fkc_update_books_languages_link_a",
        "fkc_update_books_languages_link_b",
        "fkc_update_books_publishers_link_a",
        "fkc_update_books_publishers_link_b",
        "fkc_update_books_ratings_link_a",
        "fkc_update_books_ratings_link_b",
        "fkc_update_books_series_link_a",
        "fkc_update_books_series_link_b",
        "fkc_update_books_tags_link_a",
        "fkc_update_books_tags_link_b",
        "series_insert_trg",
        "series_update_trg",
    ];
    for trigger in triggers {
        assert!(
            db.schema_object_exists("trigger", trigger).unwrap_or(false),
            "missing trigger {trigger}"
        );
    }

    let indices = [
        "authors_idx",
        "books_authors_link_aidx",
        "books_authors_link_bidx",
        "books_idx",
        "books_languages_link_aidx",
        "books_languages_link_bidx",
        "books_publishers_link_aidx",
        "books_publishers_link_bidx",
        "books_ratings_link_aidx",
        "books_ratings_link_bidx",
        "books_series_link_aidx",
        "books_series_link_bidx",
        "books_tags_link_aidx",
        "books_tags_link_bidx",
        "comments_idx",
        "conversion_options_idx_a",
        "conversion_options_idx_b",
        "custom_columns_idx",
        "data_idx",
        "lrp_idx",
        "annot_idx",
        "formats_idx",
        "languages_idx",
        "publishers_idx",
        "series_idx",
        "tags_idx",
    ];
    for index in indices {
        assert!(
            db.schema_object_exists("index", index).unwrap_or(false),
            "missing index {index}"
        );
    }
}

#[test]
fn calibre_sql_functions_behave() {
    let (db, _tmp) = open_db();
    let title = db
        .query_scalar_string("SELECT title_sort('The Hobbit')")
        .expect("title sort")
        .expect("value");
    assert_eq!(title, "hobbit");

    let list_filter = db
        .query_scalar_i64("SELECT books_list_filter(1)")
        .expect("list filter")
        .expect("value");
    assert_eq!(list_filter, 1);

    let concat = db
        .query_scalar_string(
            "SELECT concat(val) FROM (SELECT 'b' AS val UNION ALL SELECT 'a' AS val ORDER BY val)",
        )
        .expect("concat")
        .expect("value");
    assert_eq!(concat, "a, b");

    let sortconcat = db
        .query_scalar_string(
            "SELECT sortconcat(ord, val) FROM (SELECT 2 AS ord, 'b' AS val UNION ALL SELECT 1 AS ord, 'a' AS val)",
        )
        .expect("sortconcat")
        .expect("value");
    assert_eq!(sortconcat, "a, b");

    let uuid = db
        .query_scalar_string("SELECT uuid4()")
        .expect("uuid4")
        .expect("value");
    assert_eq!(uuid.len(), 36);
    assert!(uuid.contains('-'));
}

#[test]
fn books_pages_link_auto_creates_entry() {
    let (db, _tmp) = open_db();
    let book_id = db
        .add_book(
            "Pages",
            "epub",
            "/library/pages.epub",
            "2026-04-01T00:00:00Z",
        )
        .expect("add book");
    let entry = db
        .get_books_pages_entry(book_id)
        .expect("query pages")
        .expect("missing pages entry");
    assert_eq!(entry.book_id, book_id);
    assert_eq!(entry.pages, 0);
    assert!(!entry.needs_scan);
}

fn open_db() -> (Database, TempDir) {
    let temp_dir = tempfile::Builder::new()
        .prefix("caliberate-test-schema-")
        .tempdir()
        .expect("tempdir");
    let path = temp_dir.path().join("schema.db");
    let db = Database::open_path(&path, 100).expect("open db");
    (db, temp_dir)
}

fn table_columns(db: &Database, table: &str) -> std::collections::BTreeSet<String> {
    db.table_columns(table)
        .expect("table columns")
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>()
}
