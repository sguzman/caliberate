use caliberate_db::cache::MetadataCache;
use caliberate_db::database::Database;
use tempfile::TempDir;

#[test]
fn cache_refreshes_and_loads_details() {
    let (mut db, _tmp) = open_db();
    let book_id = db
        .add_book(
            "Title",
            "epub",
            "/library/book.epub",
            "2026-04-01T00:00:00Z",
        )
        .expect("add book");
    db.add_note(book_id, "Note", "2026-04-02T00:00:00Z")
        .expect("add note");

    let mut cache = MetadataCache::new();
    cache.refresh_books(&db).expect("refresh");
    assert_eq!(cache.list_books().len(), 1);
    let details = cache
        .get_book_details(&db, book_id)
        .expect("details")
        .expect("cached");
    assert_eq!(details.book.id, book_id);
    assert_eq!(details.notes.len(), 1);
}

fn open_db() -> (Database, TempDir) {
    let temp_dir = tempfile::Builder::new()
        .prefix("caliberate-test-cache-")
        .tempdir()
        .expect("tempdir");
    let path = temp_dir.path().join("cache.db");
    let db = Database::open_path(&path, 100).expect("open db");
    (db, temp_dir)
}
