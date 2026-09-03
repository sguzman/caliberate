use caliberate_db::database::Database;
use caliberate_db::query::BookQuery;
use tempfile::TempDir;

#[test]
fn summaries_batch_metadata_and_preserve_query_pages() {
    let (db, _temp_dir, first_id, second_id) = seeded_db();
    let summaries = db
        .search_book_summaries_query(&BookQuery::new().with_limit(1))
        .expect("summary page");
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].id, first_id);
    assert_eq!(summaries[0].authors, ["Alice", "Bob"]);
    assert_eq!(summaries[0].tags, ["fiction", "science"]);
    assert_eq!(
        summaries[0]
            .series
            .as_ref()
            .map(|series| (&series.name, series.index)),
        Some((&"Series A".to_string(), 2.5))
    );
    assert_eq!(summaries[0].publisher.as_deref(), Some("Orbit"));
    assert_eq!(summaries[0].rating, Some(8));
    assert_eq!(summaries[0].languages, ["en", "fr"]);
    assert!(summaries[0].has_cover);
    assert_eq!(
        summaries[0].timestamp.as_deref(),
        Some("2026-04-02T00:00:00Z")
    );
    assert_eq!(
        summaries[0].last_modified.as_deref(),
        Some("2026-04-03T00:00:00Z")
    );
    assert_eq!(summaries[0].pubdate.as_deref(), Some("2026-01-01"));

    let offset = db
        .search_book_summaries_query(&BookQuery::new().with_limit(1).with_offset(1))
        .expect("summary offset page");
    assert_eq!(
        offset.iter().map(|book| book.id).collect::<Vec<_>>(),
        [second_id]
    );

    let empty = db
        .search_book_summaries_query(&BookQuery::new().with_offset(10))
        .expect("empty summary page");
    assert!(empty.is_empty());
}

fn seeded_db() -> (Database, TempDir, i64, i64) {
    let temp_dir = tempfile::Builder::new()
        .prefix("caliberate-test-summary-")
        .tempdir()
        .expect("tempdir");
    let mut db = Database::open_path(temp_dir.path().join("summary.db"), 100).expect("open db");
    let first_id = db
        .add_book(
            "First Book",
            "epub",
            "/library/first.epub",
            "2026-04-01T00:00:00Z",
        )
        .expect("add first book");
    let second_id = db
        .add_book(
            "Second Book",
            "pdf",
            "/library/second.pdf",
            "2026-04-01T00:00:00Z",
        )
        .expect("add second book");
    db.add_book_authors(first_id, &["Alice".to_string(), "Bob".to_string()])
        .expect("add authors");
    db.add_book_authors(second_id, &["Alice".to_string()])
        .expect("add shared author");
    db.add_book_tags(first_id, &["fiction".to_string(), "science".to_string()])
        .expect("add tags");
    db.add_book_tags(second_id, &["fiction".to_string()])
        .expect("add shared tag");
    db.set_book_series(first_id, "Series A", 2.5)
        .expect("set series");
    db.set_book_publisher(first_id, "Orbit")
        .expect("set publisher");
    db.set_book_rating(first_id, 8).expect("set rating");
    db.set_book_languages(first_id, &["en".to_string(), "fr".to_string()])
        .expect("set languages");
    db.update_book_timestamp(first_id, "2026-04-02T00:00:00Z")
        .expect("set timestamp");
    db.update_book_last_modified(first_id, "2026-04-03T00:00:00Z")
        .expect("set modified");
    db.update_book_pubdate(first_id, "2026-01-01")
        .expect("set pubdate");
    db.update_book_has_cover(first_id, true).expect("set cover");
    (db, temp_dir, first_id, second_id)
}
