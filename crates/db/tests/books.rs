use caliberate_core::config::{DbConfig, FtsConfig};
use caliberate_db::database::Database;
use tempfile::tempdir;

#[test]
fn delete_book_removes_assets() {
    let dir = tempdir().expect("tempdir");
    let db_path = dir.path().join("books.db");
    let db = Database::open_with_fts(
        &DbConfig {
            sqlite_path: db_path.clone(),
            pool_size: 1,
            busy_timeout_ms: 1000,
        },
        &FtsConfig::default(),
    )
    .expect("open db");

    let book_id = db
        .add_book("Test", "epub", "/tmp/test.epub", "2024-01-01T00:00:00Z")
        .expect("add book");
    let _asset_id = db
        .add_asset(
            book_id,
            "copy",
            "/tmp/test.epub",
            None,
            10,
            10,
            None,
            false,
            "2024-01-01T00:00:00Z",
        )
        .expect("add asset");

    let assets = db.list_assets_for_book(book_id).expect("list assets");
    assert_eq!(assets.len(), 1);

    let mut db = db;
    let summary = db.delete_book_with_assets(book_id).expect("delete book");
    assert!(summary.book_deleted);
    assert_eq!(summary.assets_deleted, 1);

    let book = db.get_book(book_id).expect("get book");
    assert!(book.is_none());
    let assets = db.list_assets_for_book(book_id).expect("list assets");
    assert!(assets.is_empty());
}
