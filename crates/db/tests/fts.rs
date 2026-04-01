use caliberate_core::config::FtsConfig;
use caliberate_db::database::Database;
use std::path::PathBuf;

#[test]
fn fts_search_returns_results() {
    let path = temp_db_path("fts-search");
    let mut fts = FtsConfig::default();
    fts.enabled = true;
    fts.min_query_len = 2;
    fts.result_limit = 10;

    let db = Database::open_path_with_fts(&path, 100, &fts).expect("open db");
    db.add_book(
        "Rust Book",
        "epub",
        "/tmp/rust.epub",
        "2024-01-01T00:00:00Z",
    )
    .expect("add book");
    let results = db.search_books("Rust").expect("search");
    assert_eq!(results.len(), 1);
}

#[test]
fn fts_rebuild_matches_book_count() {
    let path = temp_db_path("fts-rebuild");
    let mut fts = FtsConfig::default();
    fts.enabled = true;

    let db = Database::open_path_with_fts(&path, 100, &fts).expect("open db");
    db.add_book("One", "epub", "/tmp/one.epub", "2024-01-01T00:00:00Z")
        .expect("add book");
    db.add_book("Two", "epub", "/tmp/two.epub", "2024-01-01T00:00:00Z")
        .expect("add book");

    db.rebuild_fts().expect("rebuild");
    let fts_count = db.fts_count().expect("fts count");
    let book_count = db.list_books().expect("list books").len() as i64;
    assert_eq!(fts_count, book_count);
}

#[test]
fn fts_min_query_len_falls_back_to_like() {
    let path = temp_db_path("fts-min");
    let mut fts = FtsConfig::default();
    fts.enabled = true;
    fts.min_query_len = 5;

    let db = Database::open_path_with_fts(&path, 100, &fts).expect("open db");
    db.add_book(
        "Rust Book",
        "epub",
        "/tmp/rust.epub",
        "2024-01-01T00:00:00Z",
    )
    .expect("add book");
    let results = db.search_books("Ru").expect("search");
    assert_eq!(results.len(), 1);
}

fn temp_db_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("time")
        .as_millis();
    path.push(format!("caliberate-{name}-{timestamp}.db"));
    path
}
