use caliberate_db::database::Database;
use rusqlite::Connection;
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

#[test]
fn schema_upgrade_backfills_canonical_format_and_asset_link() {
    let dir = tempfile::tempdir().expect("temporary directory");
    let path = dir.path().join("old.db");
    let content = dir.path().join("book.epub");
    fs::write(&content, b"unchanged bytes").expect("write content");
    let connection = Connection::open(&path).expect("create old database");
    connection
        .execute_batch(
            "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY);
             INSERT INTO schema_migrations VALUES (10);
             CREATE TABLE books (
                 id INTEGER PRIMARY KEY, title TEXT NOT NULL, sort TEXT,
                 timestamp TEXT, pubdate TEXT, series_index REAL NOT NULL DEFAULT 1.0,
                 author_sort TEXT, path TEXT NOT NULL, uuid TEXT, has_cover INTEGER,
                 last_modified TEXT, format TEXT NOT NULL, created_at TEXT NOT NULL
             );
             CREATE TABLE assets (
                 id INTEGER PRIMARY KEY, book_id INTEGER NOT NULL,
                 storage_mode TEXT NOT NULL, stored_path TEXT NOT NULL,
                 source_path TEXT, size_bytes INTEGER NOT NULL,
                 stored_size_bytes INTEGER NOT NULL, checksum TEXT,
                 is_compressed INTEGER NOT NULL, created_at TEXT NOT NULL
             );
             INSERT INTO books VALUES (1,'Old Book',NULL,'2026-01-01',NULL,1.0,NULL,'/book','uuid',0,'2026-01-01','EPUB','2026-01-01');",
        )
        .expect("create old schema");
    connection
        .execute(
            "INSERT INTO assets VALUES (1,1,'reference',?1,?1,10,10,NULL,0,'2026-01-01')",
            rusqlite::params![content.to_string_lossy().to_string()],
        )
        .expect("insert old asset");
    drop(connection);

    let db = Database::open_path(&path, 1000).expect("upgrade old schema");
    let book = db.get_book(1).expect("read migrated book").expect("book");
    assert_eq!(book.id, 1);
    assert_eq!(book.title, "Old Book");
    assert_eq!(book.format, "EPUB");
    assert_eq!(book.path, "/book");
    let formats = db.list_book_formats(1).expect("list backfilled formats");
    assert_eq!(formats.len(), 1);
    assert_eq!(formats[0].format, "epub");
    let asset = db.list_assets_for_book(1).unwrap().pop().unwrap();
    assert_eq!(asset.book_format_id, Some(formats[0].id));
    assert_eq!(asset.source_id, None);
    assert_eq!(asset.stored_path, content.to_string_lossy());
    assert_eq!(fs::read(&content).unwrap(), b"unchanged bytes");
    assert!(db.list_library_sources().unwrap().is_empty());
    db.migrate().expect("rerun migration");
    assert_eq!(db.list_book_formats(1).unwrap().len(), 1);
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
