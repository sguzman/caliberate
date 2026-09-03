use caliberate_db::database::Database;
use caliberate_db::query::{BookQuery, BookSortField};
use tempfile::TempDir;

#[test]
fn query_by_title() {
    let (db, _tmp, book_id, _) = seeded_db();
    let query = BookQuery::new().with_title("Rust");
    let results = db.search_books_query(&query).expect("query");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, book_id);
}

#[test]
fn query_by_author_tag_series() {
    let (db, _tmp, book_id, _) = seeded_db();
    let cases = [
        BookQuery::new().with_author("Alice"),
        BookQuery::new().with_tag("systems"),
        BookQuery::new().with_series("Series A"),
    ];
    for query in cases {
        let results = db.search_books_query(&query).expect("query");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, book_id);
    }
}

#[test]
fn query_by_publisher_language_identifier() {
    let (db, _tmp, _, book_id) = seeded_db();
    let cases = [
        BookQuery::new().with_publisher("Orbit"),
        BookQuery::new().with_language("en"),
        BookQuery::new().with_identifier("978-2"),
    ];
    for query in cases {
        let results = db.search_books_query(&query).expect("query");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, book_id);
    }
}

#[test]
fn query_by_format_and_limit() {
    let (db, _tmp, book_id, _) = seeded_db();
    let query = BookQuery::new().with_format("epub");
    let results = db.search_books_query(&query).expect("query");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, book_id);

    let query = BookQuery::new().with_limit(1);
    let results = db.search_books_query(&query).expect("query");
    assert_eq!(results.len(), 1);
}

#[test]
fn query_combined_filters() {
    let (db, _tmp, book_id, _) = seeded_db();
    let query = BookQuery::new().with_title("Rust").with_author("Alice");
    let results = db.search_books_query(&query).expect("query");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, book_id);

    let query = BookQuery::new().with_title("Rust").with_author("Bob");
    let results = db.search_books_query(&query).expect("query");
    assert!(results.is_empty());
}

#[test]
fn query_without_filters_returns_all() {
    let (db, _tmp, _, _) = seeded_db();
    let query = BookQuery::new();
    let results = db.search_books_query(&query).expect("query");
    assert_eq!(results.len(), 2);
}

#[test]
fn query_sorts_case_insensitively_with_deterministic_ties() {
    let (db, _tmp) = ordered_db();
    let ascending = db
        .search_books_query(&BookQuery::new().with_sort(BookSortField::Title))
        .expect("ascending query");
    assert_eq!(
        ascending.iter().map(|book| book.id).collect::<Vec<_>>(),
        [2, 3, 1]
    );

    let descending = db
        .search_books_query(
            &BookQuery::new()
                .with_sort(BookSortField::Title)
                .descending(),
        )
        .expect("descending query");
    assert_eq!(
        descending.iter().map(|book| book.id).collect::<Vec<_>>(),
        [1, 2, 3]
    );
}

#[test]
fn query_supports_limit_and_offset() {
    let (db, _tmp, first_id, second_id) = seeded_db();
    let page = db
        .search_books_query(&BookQuery::new().with_limit(1).with_offset(1))
        .expect("page query");
    assert_eq!(
        page.iter().map(|book| book.id).collect::<Vec<_>>(),
        [second_id]
    );

    let remaining = db
        .search_books_query(&BookQuery::new().with_offset(1))
        .expect("remaining query");
    assert_eq!(
        remaining.iter().map(|book| book.id).collect::<Vec<_>>(),
        [second_id]
    );
    assert_ne!(first_id, second_id);
}

#[test]
fn count_query_ignores_page_and_counts_distinct_filtered_books() {
    let (mut db, _tmp, first_id, second_id) = seeded_db();
    db.add_book_authors(second_id, &vec!["Alice".to_string()])
        .expect("add shared author");
    let query = BookQuery::new()
        .with_author("Alice")
        .with_limit(1)
        .with_offset(10)
        .with_sort(BookSortField::Title)
        .descending();
    assert_eq!(db.count_books_query(&query).expect("count query"), 2);
    assert_eq!(first_id, 1);
}

fn ordered_db() -> (Database, TempDir) {
    let temp_dir = tempfile::Builder::new()
        .prefix("caliberate-test-query-order-")
        .tempdir()
        .expect("tempdir");
    let path = temp_dir.path().join("query.db");
    let db = Database::open_path(&path, 100).expect("open db");
    db.add_book("zeta", "epub", "/library/zeta.epub", "2026-04-01T00:00:00Z")
        .expect("add zeta");
    db.add_book(
        "Alpha",
        "epub",
        "/library/alpha.epub",
        "2026-04-01T00:00:00Z",
    )
    .expect("add Alpha");
    db.add_book(
        "alpha",
        "epub",
        "/library/alpha-2.epub",
        "2026-04-01T00:00:00Z",
    )
    .expect("add alpha");
    (db, temp_dir)
}

fn seeded_db() -> (Database, TempDir, i64, i64) {
    let temp_dir = tempfile::Builder::new()
        .prefix("caliberate-test-query-")
        .tempdir()
        .expect("tempdir");
    let path = temp_dir.path().join("query.db");
    let mut db = Database::open_path(&path, 100).expect("open db");
    let created_at = "2026-04-01T00:00:00Z";

    let book_id = db
        .add_book("Rust Systems", "epub", "/library/rust.epub", created_at)
        .expect("add book");
    db.add_book_authors(book_id, &vec!["Alice".to_string()])
        .expect("add authors");
    db.add_book_tags(book_id, &vec!["systems".to_string()])
        .expect("add tags");
    db.set_book_series(book_id, "Series A", 1.0)
        .expect("set series");

    let other_id = db
        .add_book("Python Guide", "pdf", "/library/python.pdf", created_at)
        .expect("add book 2");
    db.add_book_authors(other_id, &vec!["Bob".to_string()])
        .expect("add authors 2");
    db.add_book_tags(other_id, &vec!["scripting".to_string()])
        .expect("add tags 2");
    db.set_book_series(other_id, "Series B", 2.0)
        .expect("set series 2");
    db.set_book_publisher(other_id, "Orbit")
        .expect("set publisher");
    db.set_book_languages(other_id, &vec!["en".to_string()])
        .expect("set languages");
    db.add_book_identifiers(other_id, &[("isbn".to_string(), "978-2-0000".to_string())])
        .expect("set identifiers");

    (db, temp_dir, book_id, other_id)
}
