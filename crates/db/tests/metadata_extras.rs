use caliberate_db::database::Database;
use tempfile::TempDir;

#[test]
fn updates_book_extras() {
    let (mut db, _tmp) = open_db();
    let book_id = db
        .add_book(
            "Title",
            "epub",
            "/library/book.epub",
            "2026-04-01T00:00:00Z",
        )
        .expect("add book");

    db.update_book_sort(book_id, "Title, The")
        .expect("update sort");
    db.update_book_author_sort(book_id, "Doe, Jane")
        .expect("update author sort");
    db.update_book_timestamp(book_id, "2026-04-02T00:00:00Z")
        .expect("update timestamp");
    db.update_book_pubdate(book_id, "2026-01-01")
        .expect("update pubdate");
    db.update_book_last_modified(book_id, "2026-04-03T00:00:00Z")
        .expect("update last modified");
    db.update_book_uuid(book_id, "uuid-123")
        .expect("update uuid");
    db.update_book_has_cover(book_id, true)
        .expect("update cover");
    db.set_book_publisher(book_id, "Publisher One")
        .expect("set publisher");
    db.set_book_rating(book_id, 7).expect("set rating");
    db.set_book_languages(book_id, &vec!["en".to_string(), "fr".to_string()])
        .expect("set languages");

    let extras = db.get_book_extras(book_id).expect("get extras");
    assert_eq!(extras.sort.as_deref(), Some("Title, The"));
    assert_eq!(extras.author_sort.as_deref(), Some("Doe, Jane"));
    assert_eq!(extras.timestamp.as_deref(), Some("2026-04-02T00:00:00Z"));
    assert_eq!(extras.pubdate.as_deref(), Some("2026-01-01"));
    assert_eq!(
        extras.last_modified.as_deref(),
        Some("2026-04-03T00:00:00Z")
    );
    assert_eq!(extras.uuid.as_deref(), Some("uuid-123"));
    assert!(extras.has_cover);
    assert_eq!(extras.publisher.as_deref(), Some("Publisher One"));
    assert_eq!(extras.rating, Some(7));
    assert_eq!(extras.languages, vec!["en".to_string(), "fr".to_string()]);
}

#[test]
fn clears_publisher_and_rating() {
    let (mut db, _tmp) = open_db();
    let book_id = db
        .add_book(
            "Title",
            "epub",
            "/library/book.epub",
            "2026-04-01T00:00:00Z",
        )
        .expect("add book");
    db.set_book_publisher(book_id, "Publisher One")
        .expect("set publisher");
    db.set_book_rating(book_id, 5).expect("set rating");
    db.set_book_rating(book_id, 0).expect("clear rating");
    db.clear_book_publisher(book_id).expect("clear publisher");

    let extras = db.get_book_extras(book_id).expect("get extras");
    assert!(extras.publisher.is_none());
    assert!(extras.rating.is_none());
}

fn open_db() -> (Database, TempDir) {
    let temp_dir = tempfile::Builder::new()
        .prefix("caliberate-test-extras-")
        .tempdir()
        .expect("tempdir");
    let path = temp_dir.path().join("metadata-extras.db");
    let db = Database::open_path(&path, 100).expect("open db");
    (db, temp_dir)
}
