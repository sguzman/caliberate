use caliberate_db::database::Database;
use tempfile::TempDir;

#[test]
fn updates_book_title() {
    let (mut db, _tmp) = open_db();
    let book_id = db
        .add_book(
            "Old Title",
            "epub",
            "/library/book.epub",
            "2026-04-01T00:00:00Z",
        )
        .expect("add book");
    let updated = db
        .update_book_title(book_id, "New Title")
        .expect("update title");
    assert!(updated);
    let book = db.get_book(book_id).expect("get book").expect("book");
    assert_eq!(book.title, "New Title");
}

#[test]
fn replaces_metadata_relations() {
    let (mut db, _tmp) = open_db();
    let book_id = db
        .add_book(
            "Title",
            "epub",
            "/library/book.epub",
            "2026-04-01T00:00:00Z",
        )
        .expect("add book");
    db.add_book_authors(book_id, &vec!["Alice".to_string()])
        .expect("add authors");
    db.add_book_tags(book_id, &vec!["alpha".to_string()])
        .expect("add tags");
    db.set_book_series(book_id, "Series One", 1.0)
        .expect("set series");
    db.add_book_identifiers(book_id, &[("isbn".to_string(), "111".to_string())])
        .expect("add identifiers");
    db.set_book_comment(book_id, "Initial")
        .expect("set comment");

    db.replace_book_authors(book_id, &vec!["Bob".to_string(), "Cara".to_string()])
        .expect("replace authors");
    db.replace_book_tags(book_id, &vec!["beta".to_string()])
        .expect("replace tags");
    db.clear_book_series(book_id).expect("clear series");
    db.replace_book_identifiers(
        book_id,
        &[
            ("asin".to_string(), "222".to_string()),
            ("isbn".to_string(), "333".to_string()),
        ],
    )
    .expect("replace identifiers");
    db.clear_book_comment(book_id).expect("clear comment");

    let authors = db.list_book_authors(book_id).expect("list authors");
    assert_eq!(authors, vec!["Bob".to_string(), "Cara".to_string()]);
    let tags = db.list_book_tags(book_id).expect("list tags");
    assert_eq!(tags, vec!["beta".to_string()]);
    let series = db.get_book_series(book_id).expect("get series");
    assert!(series.is_none());
    let identifiers = db.list_book_identifiers(book_id).expect("list identifiers");
    assert_eq!(identifiers.len(), 2);
    let comment = db.get_book_comment(book_id).expect("get comment");
    assert!(comment.is_none());
}

fn open_db() -> (Database, TempDir) {
    let temp_dir = tempfile::Builder::new()
        .prefix("caliberate-test-meta-update-")
        .tempdir()
        .expect("tempdir");
    let path = temp_dir.path().join("metadata-update.db");
    let db = Database::open_path(&path, 100).expect("open db");
    (db, temp_dir)
}
