use caliberate_db::database::Database;
use tempfile::TempDir;

#[test]
fn custom_columns_create_list_set_remove() {
    let (db, _tmp) = open_db();
    let id = db
        .create_custom_column("rating_text", "Rating Text", "text", "{}")
        .expect("create custom column");
    let columns = db.list_custom_columns().expect("list columns");
    assert!(columns.iter().any(|col| col.id == id));

    let book_id = db
        .add_book(
            "Custom",
            "epub",
            "/library/custom.epub",
            "2026-04-02T00:00:00Z",
        )
        .expect("add book");
    db.set_custom_value(book_id, "rating_text", "Loved it")
        .expect("set custom value");
    let value = db
        .get_custom_value(book_id, "rating_text")
        .expect("get custom value");
    assert_eq!(value.as_deref(), Some("Loved it"));

    let removed = db
        .delete_custom_column("rating_text")
        .expect("remove column");
    assert!(removed);
}

fn open_db() -> (Database, TempDir) {
    let temp_dir = tempfile::Builder::new()
        .prefix("caliberate-test-custom-columns-")
        .tempdir()
        .expect("tempdir");
    let path = temp_dir.path().join("custom.db");
    let db = Database::open_path(&path, 100).expect("open db");
    (db, temp_dir)
}
