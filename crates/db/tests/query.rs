use caliberate_db::database::Database;
use caliberate_db::query::{
    BookMetadataFilterField, BookMetadataFilterMode, BookQuery, BookSortField,
};
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
fn metadata_filters_support_all_fields_and_modes() {
    let (db, _tmp) = sort_metadata_db();
    let cases = [
        (BookMetadataFilterField::Authors, "alpha", vec![1]),
        (BookMetadataFilterField::Tags, "bet", vec![2]),
        (BookMetadataFilterField::Series, "SER", vec![1, 2]),
        (BookMetadataFilterField::Publishers, "LPH", vec![2]),
        (BookMetadataFilterField::Ratings, "8", vec![1]),
        (BookMetadataFilterField::Languages, "zulu", vec![1]),
    ];

    for (field, value, expected) in cases {
        let included = db
            .search_books_query(&BookQuery::new().with_metadata_filter(
                field,
                BookMetadataFilterMode::Include,
                value,
            ))
            .expect("include metadata filter");
        assert_eq!(
            included.iter().map(|book| book.id).collect::<Vec<_>>(),
            expected
        );

        let excluded = db
            .search_books_query(&BookQuery::new().with_metadata_filter(
                field,
                BookMetadataFilterMode::Exclude,
                value,
            ))
            .expect("exclude metadata filter");
        assert_eq!(
            excluded.iter().map(|book| book.id).collect::<Vec<_>>(),
            (1..=3)
                .filter(|id| !expected.contains(id))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn metadata_filters_are_anded_and_preserve_legacy_filters() {
    let (db, _tmp) = sort_metadata_db();
    let query = BookQuery::new()
        .with_metadata_filter(
            BookMetadataFilterField::Tags,
            BookMetadataFilterMode::Include,
            "alpha",
        )
        .with_metadata_filter(
            BookMetadataFilterField::Tags,
            BookMetadataFilterMode::Include,
            "zulu",
        );
    let results = db
        .search_books_query(&query)
        .expect("same-category includes");
    assert_eq!(results.iter().map(|book| book.id).collect::<Vec<_>>(), [1]);

    let query = BookQuery::new()
        .with_metadata_filter(
            BookMetadataFilterField::Authors,
            BookMetadataFilterMode::Include,
            "alpha",
        )
        .with_metadata_filter(
            BookMetadataFilterField::Tags,
            BookMetadataFilterMode::Exclude,
            "zulu",
        )
        .with_metadata_filter(
            BookMetadataFilterField::Languages,
            BookMetadataFilterMode::Include,
            "zulu",
        );
    assert!(
        db.search_books_query(&query)
            .expect("cross-category filters")
            .is_empty()
    );

    let query = BookQuery::new().with_author("alpha").with_metadata_filter(
        BookMetadataFilterField::Authors,
        BookMetadataFilterMode::Exclude,
        "beta",
    );
    assert_eq!(
        db.search_books_query(&query)
            .expect("legacy plus structured")
            .len(),
        1
    );
}

#[test]
fn metadata_filters_handle_missing_relations_and_numeric_ratings() {
    let (db, _tmp) = sort_metadata_db();
    let missing_author = BookQuery::new().with_metadata_filter(
        BookMetadataFilterField::Authors,
        BookMetadataFilterMode::Include,
        "missing",
    );
    assert!(
        db.search_books_query(&missing_author)
            .expect("missing include")
            .is_empty()
    );

    let missing_tag = BookQuery::new().with_metadata_filter(
        BookMetadataFilterField::Tags,
        BookMetadataFilterMode::Exclude,
        "missing",
    );
    assert_eq!(
        db.search_books_query(&missing_tag)
            .expect("missing exclude")
            .len(),
        3
    );

    let exact_rating = BookQuery::new().with_metadata_filter(
        BookMetadataFilterField::Ratings,
        BookMetadataFilterMode::Include,
        "8",
    );
    assert_eq!(
        db.search_books_query(&exact_rating)
            .expect("exact rating")
            .len(),
        1
    );
    let substring_rating = BookQuery::new().with_metadata_filter(
        BookMetadataFilterField::Ratings,
        BookMetadataFilterMode::Include,
        "1",
    );
    assert_eq!(
        db.search_books_query(&substring_rating)
            .expect("numeric rating")
            .len(),
        0
    );

    let invalid_rating = BookQuery::new().with_metadata_filter(
        BookMetadataFilterField::Ratings,
        BookMetadataFilterMode::Include,
        "8x",
    );
    let error = db
        .search_books_query(&invalid_rating)
        .expect_err("invalid rating");
    assert!(format!("{error}").contains("rating metadata filter"));
}

#[test]
fn metadata_filter_count_matches_paginated_results_without_duplicates() {
    let (db, _tmp) = sort_metadata_db();
    let query = BookQuery::new()
        .with_metadata_filter(
            BookMetadataFilterField::Series,
            BookMetadataFilterMode::Include,
            "series",
        )
        .with_metadata_filter(
            BookMetadataFilterField::Authors,
            BookMetadataFilterMode::Include,
            "a",
        )
        .with_limit(1);
    let results = db.search_books_query(&query).expect("filtered page");
    assert_eq!(results.len(), 1);
    assert_eq!(db.count_books_query(&query).expect("filtered count"), 2);
}

#[test]
fn metadata_string_filters_escape_like_metacharacters() {
    let (mut db, _tmp) = sort_metadata_db();
    let percent_book = db
        .add_book(
            "Percent",
            "epub",
            "/library/percent.epub",
            "2026-04-04T00:00:00Z",
        )
        .expect("add percent book");
    let wildcard_percent_book = db
        .add_book(
            "Wildcard percent",
            "epub",
            "/library/wildcard.epub",
            "2026-04-05T00:00:00Z",
        )
        .expect("add wildcard percent book");
    let underscore_book = db
        .add_book(
            "Underscore",
            "epub",
            "/library/underscore.epub",
            "2026-04-06T00:00:00Z",
        )
        .expect("add underscore book");
    let wildcard_underscore_book = db
        .add_book(
            "Wildcard underscore",
            "epub",
            "/library/wildcard-underscore.epub",
            "2026-04-07T00:00:00Z",
        )
        .expect("add wildcard underscore book");
    db.add_book_authors(percent_book, &["100% literal".to_string()])
        .expect("add literal percent author");
    db.add_book_authors(wildcard_percent_book, &["100abc literal".to_string()])
        .expect("add wildcard percent author");
    db.add_book_tags(underscore_book, &["under_score".to_string()])
        .expect("add literal underscore tag");
    db.add_book_tags(wildcard_underscore_book, &["underXscore".to_string()])
        .expect("add wildcard underscore tag");

    let percent_include = BookQuery::new().with_metadata_filter(
        BookMetadataFilterField::Authors,
        BookMetadataFilterMode::Include,
        "%",
    );
    let percent_results = db
        .search_books_query(&percent_include)
        .expect("literal percent include");
    assert_eq!(
        percent_results
            .iter()
            .map(|book| book.id)
            .collect::<Vec<_>>(),
        [percent_book]
    );

    let percent_exclude = BookQuery::new().with_metadata_filter(
        BookMetadataFilterField::Authors,
        BookMetadataFilterMode::Exclude,
        "%",
    );
    let percent_excluded = db
        .search_books_query(&percent_exclude)
        .expect("literal percent exclude");
    assert!(!percent_excluded.iter().any(|book| book.id == percent_book));
    assert!(
        percent_excluded
            .iter()
            .any(|book| book.id == wildcard_percent_book)
    );

    let underscore_include = BookQuery::new().with_metadata_filter(
        BookMetadataFilterField::Tags,
        BookMetadataFilterMode::Include,
        "_",
    );
    let underscore_results = db
        .search_books_query(&underscore_include)
        .expect("literal underscore include");
    assert_eq!(
        underscore_results
            .iter()
            .map(|book| book.id)
            .collect::<Vec<_>>(),
        [underscore_book]
    );

    let underscore_exclude = BookQuery::new().with_metadata_filter(
        BookMetadataFilterField::Tags,
        BookMetadataFilterMode::Exclude,
        "_",
    );
    let underscore_excluded = db
        .search_books_query(&underscore_exclude)
        .expect("literal underscore exclude");
    assert!(
        !underscore_excluded
            .iter()
            .any(|book| book.id == underscore_book)
    );
    assert!(
        underscore_excluded
            .iter()
            .any(|book| book.id == wildcard_underscore_book)
    );
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
fn query_supports_all_library_sort_fields() {
    let (db, _tmp, _, _) = seeded_db();
    let fields = [
        BookSortField::Authors,
        BookSortField::Series,
        BookSortField::Tags,
        BookSortField::Rating,
        BookSortField::Publisher,
        BookSortField::Languages,
        BookSortField::DateAdded,
        BookSortField::DateModified,
        BookSortField::PubDate,
    ];

    for field in fields {
        db.search_books_query(&BookQuery::new().with_sort(field))
            .expect("sort query");
    }
}

#[test]
fn query_sorts_metadata_with_missing_values_and_ties() {
    let (db, _tmp) = sort_metadata_db();

    let authors = db
        .search_books_query(&BookQuery::new().with_sort(BookSortField::Authors))
        .expect("author sort");
    assert_eq!(
        authors.iter().map(|book| book.id).collect::<Vec<_>>(),
        [3, 1, 2]
    );

    let tags = db
        .search_books_query(&BookQuery::new().with_sort(BookSortField::Tags))
        .expect("tag sort");
    assert_eq!(
        tags.iter().map(|book| book.id).collect::<Vec<_>>(),
        [3, 1, 2]
    );

    let series = db
        .search_books_query(&BookQuery::new().with_sort(BookSortField::Series))
        .expect("series sort");
    assert_eq!(
        series.iter().map(|book| book.id).collect::<Vec<_>>(),
        [3, 2, 1]
    );

    let ratings = db
        .search_books_query(&BookQuery::new().with_sort(BookSortField::Rating))
        .expect("rating sort");
    assert_eq!(
        ratings.iter().map(|book| book.id).collect::<Vec<_>>(),
        [2, 3, 1]
    );

    let publishers = db
        .search_books_query(&BookQuery::new().with_sort(BookSortField::Publisher))
        .expect("publisher sort");
    assert_eq!(
        publishers.iter().map(|book| book.id).collect::<Vec<_>>(),
        [3, 2, 1]
    );

    let languages = db
        .search_books_query(&BookQuery::new().with_sort(BookSortField::Languages))
        .expect("language sort");
    assert_eq!(
        languages.iter().map(|book| book.id).collect::<Vec<_>>(),
        [3, 2, 1]
    );

    for (field, expected) in [
        (BookSortField::DateAdded, vec![1, 2, 3]),
        (BookSortField::DateModified, vec![1, 2, 3]),
        (BookSortField::PubDate, vec![1, 2, 3]),
    ] {
        let results = db
            .search_books_query(&BookQuery::new().with_sort(field))
            .expect("date sort");
        assert_eq!(
            results.iter().map(|book| book.id).collect::<Vec<_>>(),
            expected
        );

        let descending = db
            .search_books_query(&BookQuery::new().with_sort(field).descending())
            .expect("descending date sort");
        assert_eq!(
            descending.iter().map(|book| book.id).collect::<Vec<_>>(),
            [3, 2, 1]
        );
    }

    let tied = db
        .search_books_query(&BookQuery::new().with_sort(BookSortField::Publisher))
        .expect("publisher tie sort");
    assert_eq!(
        tied.iter().map(|book| book.id).collect::<Vec<_>>(),
        [3, 2, 1]
    );
}

#[test]
fn query_summary_sort_preserves_structured_query_order() {
    let (db, _tmp) = sort_metadata_db();
    let summaries = db
        .search_book_summaries_query(&BookQuery::new().with_sort(BookSortField::Rating))
        .expect("summary sort");
    assert_eq!(
        summaries.iter().map(|book| book.id).collect::<Vec<_>>(),
        [2, 3, 1]
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

fn sort_metadata_db() -> (Database, TempDir) {
    let temp_dir = tempfile::Builder::new()
        .prefix("caliberate-test-query-sort-")
        .tempdir()
        .expect("tempdir");
    let mut db = Database::open_path(temp_dir.path().join("query.db"), 100).expect("open db");
    let ids = [
        db.add_book(
            "First",
            "epub",
            "/library/first.epub",
            "2026-04-01T00:00:00Z",
        )
        .expect("add first"),
        db.add_book(
            "Second",
            "epub",
            "/library/second.epub",
            "2026-04-02T00:00:00Z",
        )
        .expect("add second"),
        db.add_book(
            "Third",
            "epub",
            "/library/third.epub",
            "2026-04-03T00:00:00Z",
        )
        .expect("add third"),
    ];
    db.add_book_authors(ids[0], &["Zulu".to_string(), "alpha".to_string()])
        .expect("add first authors");
    db.add_book_authors(ids[1], &["Beta".to_string()])
        .expect("add second authors");
    db.add_book_tags(ids[0], &["Zulu".to_string(), "alpha".to_string()])
        .expect("add first tags");
    db.add_book_tags(ids[1], &["Beta".to_string()])
        .expect("add second tags");
    db.set_book_series(ids[0], "Series", 2.0)
        .expect("set first series");
    db.set_book_series(ids[1], "Series", 1.0)
        .expect("set second series");
    db.set_book_rating(ids[0], 8).expect("set first rating");
    db.set_book_rating(ids[2], 3).expect("set third rating");
    db.set_book_publisher(ids[0], "zeta")
        .expect("set first publisher");
    db.set_book_publisher(ids[1], "Alpha")
        .expect("set second publisher");
    db.set_book_languages(ids[0], &["Zulu".to_string(), "Alpha".to_string()])
        .expect("set first languages");
    db.set_book_languages(ids[1], &["beta".to_string()])
        .expect("set second languages");
    for (id, suffix) in ids.into_iter().zip(["01", "02", "03"]) {
        db.update_book_timestamp(id, &format!("2026-04-{suffix}T00:00:00Z"))
            .expect("set timestamp");
        db.update_book_last_modified(id, &format!("2026-05-{suffix}T00:00:00Z"))
            .expect("set modified");
        db.update_book_pubdate(id, &format!("2026-03-{suffix}"))
            .expect("set pubdate");
    }
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
