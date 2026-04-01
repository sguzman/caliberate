use caliberate_db::database::Database;
use tempfile::TempDir;

#[test]
fn metadata_relations_roundtrip() {
    let temp_dir = temp_db_dir();
    let path = temp_dir.path().join("metadata-relations.db");
    let mut db = Database::open_path(&path, 100).expect("open db");
    let created_at = "2026-04-01T00:00:00Z";
    let book_id = db
        .add_book("The Hobbit", "epub", "/library/hobbit.epub", created_at)
        .expect("add book");

    db.add_book_authors(book_id, &vec!["J.R.R. Tolkien".to_string()])
        .expect("add authors");
    db.add_book_tags(
        book_id,
        &vec!["fantasy".to_string(), "adventure".to_string()],
    )
    .expect("add tags");
    db.set_book_series(book_id, "Middle-earth", 1.0)
        .expect("set series");
    db.add_book_identifiers(
        book_id,
        &[("isbn".to_string(), "9780345339683".to_string())],
    )
    .expect("add identifiers");
    db.set_book_comment(book_id, "Classic adventure")
        .expect("set comment");

    let authors = db.list_book_authors(book_id).expect("list authors");
    assert_eq!(authors, vec!["J.R.R. Tolkien".to_string()]);

    let tags = db.list_book_tags(book_id).expect("list tags");
    assert_eq!(tags, vec!["adventure".to_string(), "fantasy".to_string()]);

    let series = db.get_book_series(book_id).expect("get series");
    let series = series.expect("series missing");
    assert_eq!(series.name, "Middle-earth");
    assert_eq!(series.index, 1.0);

    let identifiers = db.list_book_identifiers(book_id).expect("list identifiers");
    assert_eq!(identifiers.len(), 1);
    assert_eq!(identifiers[0].id_type, "isbn");
    assert_eq!(identifiers[0].value, "9780345339683");

    let comment = db.get_book_comment(book_id).expect("get comment");
    assert_eq!(comment.as_deref(), Some("Classic adventure"));
}

#[test]
fn search_books_like_matches_metadata() {
    let temp_dir = temp_db_dir();
    let path = temp_dir.path().join("metadata-search.db");
    let mut db = Database::open_path(&path, 100).expect("open db");
    let created_at = "2026-04-01T00:00:00Z";
    let book_id = db
        .add_book("Sample Title", "pdf", "/library/sample.pdf", created_at)
        .expect("add book");
    db.add_book_authors(book_id, &vec!["Jane Doe".to_string()])
        .expect("add authors");
    db.add_book_tags(book_id, &vec!["Testing".to_string()])
        .expect("add tags");
    db.set_book_series(book_id, "QA Suite", 2.0)
        .expect("set series");

    let matches = db.search_books_like("Jane").expect("search Jane");
    assert_eq!(matches.len(), 1);
    let matches = db.search_books_like("Testing").expect("search Testing");
    assert_eq!(matches.len(), 1);
    let matches = db.search_books_like("QA Suite").expect("search QA Suite");
    assert_eq!(matches.len(), 1);
}

fn temp_db_dir() -> TempDir {
    tempfile::Builder::new()
        .prefix("caliberate-test-meta-")
        .tempdir()
        .expect("tempdir")
}
