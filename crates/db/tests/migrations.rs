use caliberate_db::database::Database;
use std::fs;
use std::path::PathBuf;

#[test]
fn migration_creates_schema() {
    let path = temp_db_path();
    let db = Database::open_path(&path, 100).expect("open db");
    let books = db.list_books().expect("list books");
    assert!(books.is_empty());
    let _ = fs::remove_file(path);
}

fn temp_db_path() -> PathBuf {
    let mut path = std::env::temp_dir();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_millis();
    path.push(format!("caliberate-test-{timestamp}.db"));
    path
}
