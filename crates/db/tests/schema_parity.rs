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
