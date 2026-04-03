use caliberate_db::database::Database;
use tempfile::TempDir;

#[test]
fn schema_includes_calibre_parity_tables() {
    let (db, _tmp) = open_db();
    let tables = [
        "books_plugin_data",
        "books_pages_link",
        "conversion_options",
        "custom_columns",
        "data",
        "feeds",
        "library_id",
        "metadata_dirtied",
        "annotations_dirtied",
        "preferences",
        "last_read_positions",
        "annotations",
        "annotations_fts",
        "annotations_fts_stemmed",
    ];
    for table in tables {
        assert!(
            db.schema_object_exists("table", table).unwrap_or(false),
            "missing table {table}"
        );
    }
}

#[test]
fn schema_columns_match_calibre_naming() {
    let (db, _tmp) = open_db();
    let comments = table_columns(&db, "comments");
    assert!(comments.contains("id"));
    assert!(comments.contains("book"));
    assert!(comments.contains("text"));
    assert!(!comments.contains("book_id"));

    let identifiers = table_columns(&db, "identifiers");
    assert!(identifiers.contains("id"));
    assert!(identifiers.contains("book"));
    assert!(identifiers.contains("type"));
    assert!(identifiers.contains("val"));
    assert!(!identifiers.contains("identifier_type"));

    let authors_link = table_columns(&db, "books_authors_link");
    assert!(authors_link.contains("book"));
    assert!(authors_link.contains("author"));
    assert!(!authors_link.contains("book_id"));
    assert!(!authors_link.contains("author_id"));

    let tags_link = table_columns(&db, "books_tags_link");
    assert!(tags_link.contains("book"));
    assert!(tags_link.contains("tag"));
    assert!(!tags_link.contains("tag_id"));

    let series_link = table_columns(&db, "books_series_link");
    assert!(series_link.contains("book"));
    assert!(series_link.contains("series"));
    assert!(!series_link.contains("series_id"));
    assert!(!series_link.contains("series_index"));

    let publishers_link = table_columns(&db, "books_publishers_link");
    assert!(publishers_link.contains("book"));
    assert!(publishers_link.contains("publisher"));
    assert!(!publishers_link.contains("publisher_id"));

    let ratings_link = table_columns(&db, "books_ratings_link");
    assert!(ratings_link.contains("book"));
    assert!(ratings_link.contains("rating"));
    assert!(!ratings_link.contains("rating_id"));

    let languages_link = table_columns(&db, "books_languages_link");
    assert!(languages_link.contains("book"));
    assert!(languages_link.contains("lang_code"));
    assert!(!languages_link.contains("language_id"));
}

#[test]
fn schema_books_columns_match_calibre_core_fields() {
    let (db, _tmp) = open_db();
    assert_columns(
        &db,
        "books",
        &[
            "id",
            "title",
            "sort",
            "timestamp",
            "pubdate",
            "series_index",
            "author_sort",
            "path",
            "uuid",
            "has_cover",
            "last_modified",
        ],
    );
}

#[test]
fn schema_books_table_defaults_and_collations_match_calibre() {
    let (db, _tmp) = open_db();
    assert_table_sql_contains(
        &db,
        "books",
        "title TEXT NOT NULL DEFAULT 'Unknown' COLLATE NOCASE",
    );
    assert_table_sql_contains(&db, "books", "sort TEXT COLLATE NOCASE");
    assert_table_sql_contains(
        &db,
        "books",
        "timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP",
    );
    assert_table_sql_contains(&db, "books", "pubdate TIMESTAMP DEFAULT CURRENT_TIMESTAMP");
    assert_table_sql_contains(&db, "books", "series_index REAL NOT NULL DEFAULT 1.0");
    assert_table_sql_contains(&db, "books", "author_sort TEXT COLLATE NOCASE");
    assert_table_sql_contains(&db, "books", "path TEXT NOT NULL DEFAULT ''");
    assert_table_sql_contains(&db, "books", "has_cover BOOL DEFAULT 0");
    assert_table_sql_contains(
        &db,
        "books",
        "last_modified TIMESTAMP NOT NULL DEFAULT '2000-01-01 00:00:00+00:00'",
    );
}

#[test]
fn schema_authors_columns_match_calibre_fields() {
    let (db, _tmp) = open_db();
    assert_columns(&db, "authors", &["id", "name", "sort", "link"]);
}

#[test]
fn schema_authors_table_collation_defaults_match_calibre() {
    let (db, _tmp) = open_db();
    assert_table_sql_contains(&db, "authors", "name TEXT NOT NULL COLLATE NOCASE");
    assert_table_sql_contains(&db, "authors", "sort TEXT COLLATE NOCASE");
}

#[test]
fn schema_tags_columns_match_calibre_fields() {
    let (db, _tmp) = open_db();
    assert_columns(&db, "tags", &["id", "name", "link"]);
}

#[test]
fn schema_tags_table_collation_defaults_match_calibre() {
    let (db, _tmp) = open_db();
    assert_table_sql_contains(&db, "tags", "name TEXT NOT NULL COLLATE NOCASE");
}

#[test]
fn schema_series_columns_match_calibre_fields() {
    let (db, _tmp) = open_db();
    assert_columns(&db, "series", &["id", "name", "sort", "link"]);
}

#[test]
fn schema_series_table_collation_defaults_match_calibre() {
    let (db, _tmp) = open_db();
    assert_table_sql_contains(&db, "series", "name TEXT NOT NULL COLLATE NOCASE");
    assert_table_sql_contains(&db, "series", "sort TEXT COLLATE NOCASE");
}

#[test]
fn schema_publishers_columns_match_calibre_fields() {
    let (db, _tmp) = open_db();
    assert_columns(&db, "publishers", &["id", "name", "sort", "link"]);
}

#[test]
fn schema_publishers_table_collation_defaults_match_calibre() {
    let (db, _tmp) = open_db();
    assert_table_sql_contains(&db, "publishers", "name TEXT NOT NULL COLLATE NOCASE");
    assert_table_sql_contains(&db, "publishers", "sort TEXT COLLATE NOCASE");
}

#[test]
fn schema_ratings_columns_match_calibre_fields() {
    let (db, _tmp) = open_db();
    assert_columns(&db, "ratings", &["id", "rating", "link"]);
}

#[test]
fn schema_ratings_table_check_constraint_matches_calibre() {
    let (db, _tmp) = open_db();
    assert_table_sql_contains(
        &db,
        "ratings",
        "rating INTEGER CHECK(rating > -1 AND rating < 11)",
    );
}

#[test]
fn schema_languages_columns_match_calibre_fields() {
    let (db, _tmp) = open_db();
    assert_columns(&db, "languages", &["id", "lang_code", "link"]);
}

#[test]
fn schema_languages_table_collation_defaults_match_calibre() {
    let (db, _tmp) = open_db();
    assert_table_sql_contains(&db, "languages", "lang_code TEXT NOT NULL COLLATE NOCASE");
}

#[test]
fn schema_books_authors_link_columns_match_calibre_fields() {
    let (db, _tmp) = open_db();
    assert_columns(&db, "books_authors_link", &["id", "book", "author"]);
}

#[test]
fn schema_books_tags_link_columns_match_calibre_fields() {
    let (db, _tmp) = open_db();
    assert_columns(&db, "books_tags_link", &["id", "book", "tag"]);
}

#[test]
fn schema_books_series_link_columns_match_calibre_fields() {
    let (db, _tmp) = open_db();
    assert_columns(&db, "books_series_link", &["id", "book", "series"]);
}

#[test]
fn schema_books_publishers_link_columns_match_calibre_fields() {
    let (db, _tmp) = open_db();
    assert_columns(&db, "books_publishers_link", &["id", "book", "publisher"]);
}

#[test]
fn schema_books_ratings_link_columns_match_calibre_fields() {
    let (db, _tmp) = open_db();
    assert_columns(&db, "books_ratings_link", &["id", "book", "rating"]);
}

#[test]
fn schema_books_languages_link_columns_match_calibre_fields() {
    let (db, _tmp) = open_db();
    assert_columns(
        &db,
        "books_languages_link",
        &["id", "book", "lang_code", "item_order"],
    );
}

#[test]
fn schema_identifiers_columns_match_calibre_fields() {
    let (db, _tmp) = open_db();
    assert_columns(&db, "identifiers", &["id", "book", "type", "val"]);
}

#[test]
fn schema_identifiers_table_collation_defaults_match_calibre() {
    let (db, _tmp) = open_db();
    assert_table_sql_contains(
        &db,
        "identifiers",
        "type TEXT NOT NULL DEFAULT 'isbn' COLLATE NOCASE",
    );
    assert_table_sql_contains(&db, "identifiers", "val TEXT NOT NULL COLLATE NOCASE");
}

#[test]
fn schema_comments_columns_match_calibre_fields() {
    let (db, _tmp) = open_db();
    assert_columns(&db, "comments", &["id", "book", "text"]);
}

#[test]
fn schema_comments_table_collation_defaults_match_calibre() {
    let (db, _tmp) = open_db();
    assert_table_sql_contains(&db, "comments", "text TEXT NOT NULL COLLATE NOCASE");
}

#[test]
fn schema_books_plugin_data_columns_match_calibre_fields() {
    let (db, _tmp) = open_db();
    assert_columns(&db, "books_plugin_data", &["id", "book", "name", "val"]);
}

#[test]
fn schema_books_pages_link_columns_match_calibre_fields() {
    let (db, _tmp) = open_db();
    assert_columns(
        &db,
        "books_pages_link",
        &[
            "book",
            "pages",
            "algorithm",
            "format",
            "format_size",
            "timestamp",
            "needs_scan",
        ],
    );
}

#[test]
fn schema_books_pages_link_format_collation_matches_calibre() {
    let (db, _tmp) = open_db();
    assert_table_sql_contains(
        &db,
        "books_pages_link",
        "format TEXT NOT NULL DEFAULT '' COLLATE NOCASE",
    );
}

#[test]
fn schema_conversion_options_columns_match_calibre_fields() {
    let (db, _tmp) = open_db();
    assert_columns(&db, "conversion_options", &["id", "format", "book", "data"]);
}

#[test]
fn schema_custom_columns_columns_match_calibre_fields() {
    let (db, _tmp) = open_db();
    assert_columns(
        &db,
        "custom_columns",
        &[
            "id",
            "label",
            "name",
            "datatype",
            "mark_for_delete",
            "editable",
            "display",
            "is_multiple",
            "normalized",
        ],
    );
}

#[test]
fn schema_data_columns_match_calibre_fields() {
    let (db, _tmp) = open_db();
    assert_columns(
        &db,
        "data",
        &["id", "book", "format", "uncompressed_size", "name"],
    );
}

#[test]
fn schema_data_table_format_collation_matches_calibre() {
    let (db, _tmp) = open_db();
    assert_table_sql_contains(&db, "data", "format TEXT NOT NULL COLLATE NOCASE");
}

#[test]
fn schema_feeds_columns_match_calibre_fields() {
    let (db, _tmp) = open_db();
    assert_columns(&db, "feeds", &["id", "title", "script"]);
}

#[test]
fn schema_library_id_columns_match_calibre_fields() {
    let (db, _tmp) = open_db();
    assert_columns(&db, "library_id", &["id", "uuid"]);
}

#[test]
fn schema_metadata_dirtied_columns_match_calibre_fields() {
    let (db, _tmp) = open_db();
    assert_columns(&db, "metadata_dirtied", &["id", "book"]);
}

#[test]
fn schema_annotations_dirtied_columns_match_calibre_fields() {
    let (db, _tmp) = open_db();
    assert_columns(&db, "annotations_dirtied", &["id", "book"]);
}

#[test]
fn schema_preferences_columns_match_calibre_fields() {
    let (db, _tmp) = open_db();
    assert_columns(&db, "preferences", &["id", "key", "val"]);
}

#[test]
fn schema_last_read_positions_columns_match_calibre_fields() {
    let (db, _tmp) = open_db();
    assert_columns(
        &db,
        "last_read_positions",
        &[
            "id", "book", "format", "user", "device", "cfi", "epoch", "pos_frac",
        ],
    );
}

#[test]
fn schema_last_read_positions_format_collation_matches_calibre() {
    let (db, _tmp) = open_db();
    assert_table_sql_contains(
        &db,
        "last_read_positions",
        "format TEXT NOT NULL COLLATE NOCASE",
    );
}

#[test]
fn schema_tables_avoid_autoincrement_for_calibre_parity() {
    let (db, _tmp) = open_db();
    let tables = [
        "authors",
        "books_authors_link",
        "books_languages_link",
        "books_plugin_data",
        "books_publishers_link",
        "books_ratings_link",
        "books_series_link",
        "books_tags_link",
        "comments",
        "conversion_options",
        "custom_columns",
        "data",
        "feeds",
        "identifiers",
        "languages",
        "library_id",
        "metadata_dirtied",
        "annotations_dirtied",
        "preferences",
        "publishers",
        "ratings",
        "series",
        "tags",
        "last_read_positions",
        "annotations",
        "notes",
    ];
    for table in tables {
        assert_table_sql_not_contains(&db, table, "AUTOINCREMENT");
    }
}

#[test]
fn schema_annotations_columns_match_calibre_fields() {
    let (db, _tmp) = open_db();
    assert_columns(
        &db,
        "annotations",
        &[
            "id",
            "book",
            "format",
            "user_type",
            "user",
            "timestamp",
            "annot_id",
            "annot_type",
            "annot_data",
            "searchable_text",
        ],
    );
}

#[test]
fn schema_annotations_fts_columns_match_calibre_fields() {
    let (db, _tmp) = open_db();
    assert_columns(&db, "annotations_fts", &["searchable_text"]);
}

#[test]
fn schema_annotations_fts_stemmed_columns_match_calibre_fields() {
    let (db, _tmp) = open_db();
    assert_columns(&db, "annotations_fts_stemmed", &["searchable_text"]);
}

#[test]
fn schema_includes_calibre_views_triggers_indices() {
    let (db, _tmp) = open_db();
    let views = [
        "meta",
        "tag_browser_authors",
        "tag_browser_filtered_authors",
        "tag_browser_filtered_publishers",
        "tag_browser_filtered_ratings",
        "tag_browser_filtered_series",
        "tag_browser_filtered_tags",
        "tag_browser_publishers",
        "tag_browser_ratings",
        "tag_browser_series",
        "tag_browser_tags",
    ];
    for view in views {
        assert!(
            db.schema_object_exists("view", view).unwrap_or(false),
            "missing view {view}"
        );
    }

    let triggers = [
        "books_delete_trg",
        "books_insert_trg",
        "books_update_trg",
        "fkc_comments_insert",
        "fkc_comments_update",
        "fkc_data_insert",
        "fkc_data_update",
        "fkc_lrp_insert",
        "fkc_lrp_update",
        "fkc_annot_insert",
        "fkc_annot_update",
        "fkc_delete_on_authors",
        "fkc_delete_on_languages",
        "fkc_delete_on_languages_link",
        "fkc_delete_on_publishers",
        "fkc_delete_on_series",
        "fkc_delete_on_tags",
        "fkc_insert_books_authors_link",
        "fkc_insert_books_publishers_link",
        "fkc_insert_books_ratings_link",
        "fkc_insert_books_series_link",
        "fkc_insert_books_tags_link",
        "fkc_update_books_authors_link_a",
        "fkc_update_books_authors_link_b",
        "fkc_update_books_languages_link_a",
        "fkc_update_books_languages_link_b",
        "fkc_update_books_publishers_link_a",
        "fkc_update_books_publishers_link_b",
        "fkc_update_books_ratings_link_a",
        "fkc_update_books_ratings_link_b",
        "fkc_update_books_series_link_a",
        "fkc_update_books_series_link_b",
        "fkc_update_books_tags_link_a",
        "fkc_update_books_tags_link_b",
        "series_insert_trg",
        "series_update_trg",
    ];
    for trigger in triggers {
        assert!(
            db.schema_object_exists("trigger", trigger).unwrap_or(false),
            "missing trigger {trigger}"
        );
    }

    let indices = [
        "authors_idx",
        "books_authors_link_aidx",
        "books_authors_link_bidx",
        "books_idx",
        "books_languages_link_aidx",
        "books_languages_link_bidx",
        "books_publishers_link_aidx",
        "books_publishers_link_bidx",
        "books_ratings_link_aidx",
        "books_ratings_link_bidx",
        "books_series_link_aidx",
        "books_series_link_bidx",
        "books_tags_link_aidx",
        "books_tags_link_bidx",
        "comments_idx",
        "conversion_options_idx_a",
        "conversion_options_idx_b",
        "custom_columns_idx",
        "data_idx",
        "lrp_idx",
        "annot_idx",
        "formats_idx",
        "languages_idx",
        "publishers_idx",
        "series_idx",
        "tags_idx",
    ];
    for index in indices {
        assert!(
            db.schema_object_exists("index", index).unwrap_or(false),
            "missing index {index}"
        );
    }
}

#[test]
fn calibre_sql_functions_behave() {
    let (db, _tmp) = open_db();
    let title = db
        .query_scalar_string("SELECT title_sort('The Hobbit')")
        .expect("title sort")
        .expect("value");
    assert_eq!(title, "hobbit");

    let list_filter = db
        .query_scalar_i64("SELECT books_list_filter(1)")
        .expect("list filter")
        .expect("value");
    assert_eq!(list_filter, 1);

    let concat = db
        .query_scalar_string(
            "SELECT concat(val) FROM (SELECT 'b' AS val UNION ALL SELECT 'a' AS val ORDER BY val)",
        )
        .expect("concat")
        .expect("value");
    assert_eq!(concat, "a, b");

    let sortconcat = db
        .query_scalar_string(
            "SELECT sortconcat(ord, val) FROM (SELECT 2 AS ord, 'b' AS val UNION ALL SELECT 1 AS ord, 'a' AS val)",
        )
        .expect("sortconcat")
        .expect("value");
    assert_eq!(sortconcat, "a, b");

    let uuid = db
        .query_scalar_string("SELECT uuid4()")
        .expect("uuid4")
        .expect("value");
    assert_eq!(uuid.len(), 36);
    assert!(uuid.contains('-'));
}

#[test]
fn books_insert_trigger_sets_sort_and_uuid() {
    let (db, _tmp) = open_db();
    let book_id = db
        .add_book(
            "The Hobbit",
            "epub",
            "/library/hobbit.epub",
            "2026-04-01T00:00:00Z",
        )
        .expect("add book");
    let sort = db
        .query_scalar_string(&format!("SELECT sort FROM books WHERE id = {book_id}"))
        .expect("query sort")
        .expect("sort");
    assert_eq!(sort, "hobbit");
    let uuid = db
        .query_scalar_string(&format!("SELECT uuid FROM books WHERE id = {book_id}"))
        .expect("query uuid")
        .expect("uuid");
    assert_eq!(uuid.len(), 36);
}

#[test]
fn books_update_trigger_updates_sort() {
    let (db, _tmp) = open_db();
    let book_id = db
        .add_book(
            "The Hobbit",
            "epub",
            "/library/hobbit.epub",
            "2026-04-01T00:00:00Z",
        )
        .expect("add book");
    let mut db = db;
    db.update_book_title(book_id, "A Tale of Two Cities")
        .expect("update title");
    let sort = db
        .query_scalar_string(&format!("SELECT sort FROM books WHERE id = {book_id}"))
        .expect("query sort")
        .expect("sort");
    assert_eq!(sort, "tale of two cities");
}

#[test]
fn books_delete_trigger_cleans_link_tables() {
    let (db, _tmp) = open_db();
    let book_id = db
        .add_book(
            "Delete Me",
            "epub",
            "/library/delete.epub",
            "2026-04-01T00:00:00Z",
        )
        .expect("add book");
    let mut db = db;
    db.add_book_authors(book_id, &["Author One".to_string()])
        .expect("add author");
    db.add_book_tags(book_id, &["tag-one".to_string()])
        .expect("add tag");
    let authors_before = db
        .query_scalar_i64(&format!(
            "SELECT COUNT(*) FROM books_authors_link WHERE book = {book_id}"
        ))
        .expect("count authors")
        .expect("count");
    assert_eq!(authors_before, 1);
    let tags_before = db
        .query_scalar_i64(&format!(
            "SELECT COUNT(*) FROM books_tags_link WHERE book = {book_id}"
        ))
        .expect("count tags")
        .expect("count");
    assert_eq!(tags_before, 1);

    let summary = db.delete_book_with_assets(book_id).expect("delete book");
    assert!(summary.book_deleted);

    let authors_after = db
        .query_scalar_i64(&format!(
            "SELECT COUNT(*) FROM books_authors_link WHERE book = {book_id}"
        ))
        .expect("count authors after")
        .expect("count");
    assert_eq!(authors_after, 0);
    let tags_after = db
        .query_scalar_i64(&format!(
            "SELECT COUNT(*) FROM books_tags_link WHERE book = {book_id}"
        ))
        .expect("count tags after")
        .expect("count");
    assert_eq!(tags_after, 0);
}

#[test]
fn meta_view_exposes_metadata() {
    let (db, _tmp) = open_db();
    let book_id = db
        .add_book(
            "Meta Book",
            "epub",
            "/library/meta.epub",
            "2026-04-01T00:00:00Z",
        )
        .expect("add book");
    let mut db = db;
    db.add_book_authors(book_id, &["Ada Lovelace".to_string()])
        .expect("add authors");
    db.add_book_tags(book_id, &["computing".to_string()])
        .expect("add tags");
    db.set_book_series(book_id, "Meta Series", 1.0)
        .expect("set series");
    db.set_book_comment(book_id, "Notes for meta view")
        .expect("set comment");
    db.set_book_publisher(book_id, "Meta Press")
        .expect("set publisher");

    let authors = db
        .query_scalar_string(&format!("SELECT authors FROM meta WHERE id = {book_id}"))
        .expect("authors")
        .expect("authors");
    assert!(authors.contains("Ada Lovelace"));
    let tags = db
        .query_scalar_string(&format!("SELECT tags FROM meta WHERE id = {book_id}"))
        .expect("tags")
        .expect("tags");
    assert!(tags.contains("computing"));
    let series = db
        .query_scalar_string(&format!("SELECT series FROM meta WHERE id = {book_id}"))
        .expect("series")
        .expect("series");
    assert_eq!(series, "Meta Series");
    let publisher = db
        .query_scalar_string(&format!("SELECT publisher FROM meta WHERE id = {book_id}"))
        .expect("publisher")
        .expect("publisher");
    assert_eq!(publisher, "Meta Press");
}

#[test]
fn series_insert_trigger_sets_sort() {
    let (db, _tmp) = open_db();
    let book_id = db
        .add_book(
            "Series Book",
            "epub",
            "/library/series.epub",
            "2026-04-01T00:00:00Z",
        )
        .expect("add book");
    let mut db = db;
    db.set_book_series(book_id, "The Series", 1.0)
        .expect("set series");
    let sort = db
        .query_scalar_string("SELECT sort FROM series WHERE name = 'The Series'")
        .expect("query sort")
        .expect("sort");
    assert_eq!(sort, "series");
}

#[test]
fn books_pages_link_auto_creates_entry() {
    let (db, _tmp) = open_db();
    let book_id = db
        .add_book(
            "Pages",
            "epub",
            "/library/pages.epub",
            "2026-04-01T00:00:00Z",
        )
        .expect("add book");
    let entry = db
        .get_books_pages_entry(book_id)
        .expect("query pages")
        .expect("missing pages entry");
    assert_eq!(entry.book_id, book_id);
    assert_eq!(entry.pages, 0);
    assert!(!entry.needs_scan);
}

fn open_db() -> (Database, TempDir) {
    let temp_dir = tempfile::Builder::new()
        .prefix("caliberate-test-schema-")
        .tempdir()
        .expect("tempdir");
    let path = temp_dir.path().join("schema.db");
    let db = Database::open_path(&path, 100).expect("open db");
    (db, temp_dir)
}

fn table_columns(db: &Database, table: &str) -> std::collections::BTreeSet<String> {
    db.table_columns(table)
        .expect("table columns")
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>()
}

fn assert_columns(db: &Database, table: &str, expected: &[&str]) {
    let columns = table_columns(db, table);
    for column in expected {
        assert!(
            columns.contains(*column),
            "missing column {column} on {table}"
        );
    }
}

fn table_sql(db: &Database, table: &str) -> String {
    db.query_scalar_string(&format!(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = '{table}'"
    ))
    .expect("read table sql")
    .expect("missing table sql")
}

fn assert_table_sql_contains(db: &Database, table: &str, needle: &str) {
    let sql = table_sql(db, table);
    assert!(
        sql.contains(needle),
        "expected {table} sql to contain {needle}"
    );
}

fn assert_table_sql_not_contains(db: &Database, table: &str, needle: &str) {
    let sql = table_sql(db, table);
    assert!(
        !sql.contains(needle),
        "expected {table} sql to omit {needle}"
    );
}
