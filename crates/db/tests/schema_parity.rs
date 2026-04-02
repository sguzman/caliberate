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
