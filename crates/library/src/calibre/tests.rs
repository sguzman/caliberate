#[cfg(test)]
mod tests {
    use crate::calibre::CalibreLibraryBackend;
    use crate::catalog::LibraryBackend;
    use crate::query::{
        LibraryFacetKind, LibraryMetadataFilterField, LibraryMetadataFilterMode, LibraryQuery,
        LibrarySortField,
    };
    use rusqlite::{Connection, OpenFlags};
    use std::fs;
    use tempfile::TempDir;

    fn fixture() -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        let db = Connection::open(dir.path().join("metadata.db")).unwrap();
        db.execute_batch(
            "CREATE TABLE books(id INTEGER PRIMARY KEY,title TEXT,timestamp TEXT,pubdate TEXT,series_index REAL,author_sort TEXT,path TEXT,uuid TEXT,has_cover INTEGER,last_modified TEXT);
             CREATE TABLE data(id INTEGER PRIMARY KEY,book INTEGER,format TEXT,uncompressed_size INTEGER,name TEXT,UNIQUE(book,format));
             CREATE TABLE authors(id INTEGER PRIMARY KEY,name TEXT);
             CREATE TABLE books_authors_link(id INTEGER PRIMARY KEY,book INTEGER,author INTEGER);
             CREATE TABLE tags(id INTEGER PRIMARY KEY,name TEXT);
             CREATE TABLE books_tags_link(id INTEGER PRIMARY KEY,book INTEGER,tag INTEGER);
             CREATE TABLE series(id INTEGER PRIMARY KEY,name TEXT);
             CREATE TABLE books_series_link(id INTEGER PRIMARY KEY,book INTEGER,series INTEGER);
             CREATE TABLE publishers(id INTEGER PRIMARY KEY,name TEXT);
             CREATE TABLE books_publishers_link(id INTEGER PRIMARY KEY,book INTEGER,publisher INTEGER);
             CREATE TABLE ratings(id INTEGER PRIMARY KEY,rating INTEGER);
             CREATE TABLE books_ratings_link(id INTEGER PRIMARY KEY,book INTEGER,rating INTEGER);
             CREATE TABLE languages(id INTEGER PRIMARY KEY,lang_code TEXT);
             CREATE TABLE books_languages_link(id INTEGER PRIMARY KEY,book INTEGER,lang_code INTEGER,item_order INTEGER);
             CREATE TABLE identifiers(id INTEGER PRIMARY KEY,book INTEGER,type TEXT,val TEXT);
             INSERT INTO books VALUES(1,'Book One','2026-01-01','2025-01-01',1.0,'Author A','Author A/Book One (1)','u1',1,'2026-01-02');
             INSERT INTO books VALUES(2,'Book Two','2026-02-01',NULL,1.0,'Author B','Author B/Book Two (2)','u2',0,NULL);
             INSERT INTO books VALUES(3,'Metadata Only','2026-03-01',NULL,1.0,'Metadata Only','','u3',0,NULL);
             INSERT INTO data VALUES(10,1,'PDF',20,'Book One - Author A');
             INSERT INTO data VALUES(11,1,'EPUB',10,'Book One - Author A');
             INSERT INTO data VALUES(20,2,'AZW3',30,'Book Two - Author B');
             INSERT INTO authors VALUES(1,'Author A'),(2,'Author B');
             INSERT INTO books_authors_link VALUES(1,1,1),(2,2,2);
             INSERT INTO tags VALUES(1,'fiction'); INSERT INTO books_tags_link VALUES(1,1,1);
             INSERT INTO series VALUES(1,'Series A'); INSERT INTO books_series_link VALUES(1,1,1);
             INSERT INTO publishers VALUES(1,'Publisher A'); INSERT INTO books_publishers_link VALUES(1,1,1);
             INSERT INTO ratings VALUES(1,8); INSERT INTO books_ratings_link VALUES(1,1,1);
             INSERT INTO languages VALUES(1,'en'); INSERT INTO books_languages_link VALUES(1,1,1,0);
             INSERT INTO identifiers VALUES(1,1,'isbn','abc-1');",
        ).unwrap();
        fs::create_dir_all(dir.path().join("Author A/Book One (1)")).unwrap();
        fs::write(
            dir.path()
                .join("Author A/Book One (1)/Book One - Author A.pdf"),
            b"pdf",
        )
        .unwrap();
        dir
    }

    #[test]
    fn reads_modern_fixture_without_writing_source() {
        let dir = fixture();
        let db_path = dir.path().join("metadata.db");
        let before = fs::read(&db_path).unwrap();
        let backend = CalibreLibraryBackend::open(dir.path()).unwrap();
        assert_eq!(backend.list_books().unwrap()[0].format, "pdf");
        assert_eq!(backend.get_book(1).unwrap().unwrap().title, "Book One");
        assert_eq!(backend.get_book(1).unwrap().unwrap().format, "pdf");
        let expected_path = backend
            .library_root()
            .join("Author A/Book One (1)/Book One - Author A.pdf");
        assert_eq!(
            backend.get_book(1).unwrap().unwrap().path,
            expected_path.to_string_lossy()
        );
        assert_eq!(
            backend
                .query_books(&LibraryQuery::default().with_title("Book One"))
                .unwrap()[0]
                .format,
            "pdf"
        );
        assert_eq!(
            backend
                .query_summary_page(&LibraryQuery::default().with_title("Book One"))
                .unwrap()
                .books[0]
                .format,
            "pdf"
        );
        assert_eq!(
            backend
                .query_summary_page(&LibraryQuery::default().with_title("Book One"))
                .unwrap()
                .books[0]
                .path,
            expected_path.to_string_lossy()
        );
        assert!(backend.search_books("fiction").unwrap().len() == 1);
        assert_eq!(backend.search_books("Book One").unwrap().len(), 1);
        assert_eq!(backend.search_books("Author A").unwrap().len(), 1);
        assert_eq!(backend.search_books("Series A").unwrap().len(), 1);
        assert_eq!(backend.resolve_content(1).unwrap().unwrap().format, "pdf");
        assert_eq!(
            backend.resolve_content(1).unwrap().unwrap().path,
            expected_path.to_string_lossy()
        );
        let summary = backend
            .query_summary_page(&LibraryQuery::default())
            .unwrap();
        assert_eq!(summary.books[0].authors, ["Author A"]);
        assert_eq!(summary.books[0].tags, ["fiction"]);
        assert_eq!(
            summary.books[0].series.as_ref().map(|s| (&s.name, s.index)),
            Some((&"Series A".to_string(), 1.0))
        );
        assert_eq!(summary.books[0].rating, Some(8));
        assert_eq!(summary.books[0].publisher.as_deref(), Some("Publisher A"));
        assert_eq!(summary.books[0].languages, ["en"]);
        assert!(summary.books[0].has_cover);
        assert_eq!(summary.books[0].date_added.as_deref(), Some("2026-01-01"));
        assert_eq!(
            summary.books[0].date_modified.as_deref(),
            Some("2026-01-02")
        );
        assert_eq!(summary.books[0].pubdate.as_deref(), Some("2025-01-01"));
        assert_eq!(
            backend.list_facets(LibraryFacetKind::Ratings).unwrap()[0].name,
            "8"
        );
        assert_eq!(
            backend.list_facets(LibraryFacetKind::Authors).unwrap()[0].count,
            1
        );
        assert_eq!(
            backend.list_facets(LibraryFacetKind::Authors).unwrap()[0].name,
            "Author A"
        );
        assert_eq!(
            backend.list_facets(LibraryFacetKind::Tags).unwrap()[0].name,
            "fiction"
        );
        assert_eq!(
            backend.list_facets(LibraryFacetKind::Tags).unwrap()[0].count,
            1
        );
        assert_eq!(
            backend.list_facets(LibraryFacetKind::Series).unwrap()[0].name,
            "Series A"
        );
        assert_eq!(
            backend.list_facets(LibraryFacetKind::Series).unwrap()[0].count,
            1
        );
        assert_eq!(
            backend.list_facets(LibraryFacetKind::Publishers).unwrap()[0].name,
            "Publisher A"
        );
        assert_eq!(
            backend.list_facets(LibraryFacetKind::Publishers).unwrap()[0].count,
            1
        );
        assert_eq!(
            backend.list_facets(LibraryFacetKind::Languages).unwrap()[0].name,
            "en"
        );
        assert_eq!(
            backend.list_facets(LibraryFacetKind::Languages).unwrap()[0].count,
            1
        );
        assert_eq!(
            backend.list_facets(LibraryFacetKind::Ratings).unwrap()[0].count,
            1
        );
        let filtered = LibraryQuery::default().with_metadata_filter(
            LibraryMetadataFilterField::Tags,
            LibraryMetadataFilterMode::Include,
            "FICT",
        );
        assert_eq!(backend.query_page(&filtered).unwrap().total, 1);
        assert_eq!(before, fs::read(&db_path).unwrap());
        let check =
            Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let migrations: i64 = check
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='schema_migrations'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(migrations, 0);
    }

    #[test]
    fn rejects_missing_metadata_and_unsafe_source_paths() {
        let empty = tempfile::tempdir().unwrap();
        assert!(CalibreLibraryBackend::open(empty.path()).is_err());
        let dir = fixture();
        let db = Connection::open(dir.path().join("metadata.db")).unwrap();
        db.execute("UPDATE books SET path='../outside' WHERE id=1", [])
            .unwrap();
        let backend = CalibreLibraryBackend::open(dir.path()).unwrap();
        assert!(backend.resolve_content(1).is_err());
        db.execute(
            "UPDATE books SET path='Author A/Book One (1)' WHERE id=1",
            [],
        )
        .unwrap();
        db.execute(
            "UPDATE data SET name='Book One - Author A/sub' WHERE id=10",
            [],
        )
        .unwrap();
        assert!(backend.resolve_content(1).is_err());
    }

    #[test]
    fn rejects_missing_required_schema_with_table_name() {
        let dir = tempfile::tempdir().unwrap();
        Connection::open(dir.path().join("metadata.db"))
            .unwrap()
            .execute_batch("CREATE TABLE books(id INTEGER PRIMARY KEY)")
            .unwrap();
        let error = CalibreLibraryBackend::open(dir.path())
            .unwrap_err()
            .to_string();
        assert!(error.contains("incompatible Calibre metadata schema"));
        assert!(
            error.contains("missing required column") || error.contains("missing required table")
        );
    }

    #[test]
    fn rejects_unsafe_format_and_preserves_metadata_only_books() {
        let dir = fixture();
        let db = Connection::open(dir.path().join("metadata.db")).unwrap();
        db.execute("UPDATE data SET format='PDF/../../outside' WHERE id=10", [])
            .unwrap();
        let backend = CalibreLibraryBackend::open(dir.path()).unwrap();
        assert!(backend.resolve_content(1).is_err());
        let book = backend.get_book(3).unwrap().unwrap();
        assert_eq!(book.format, "");
        assert_eq!(book.path, "");
        assert_eq!(backend.resolve_content(3).unwrap(), None);
    }

    #[test]
    fn identifier_filters_match_type_and_value_and_escape_wildcards() {
        let dir = fixture();
        let backend = CalibreLibraryBackend::open(dir.path()).unwrap();
        assert_eq!(
            backend
                .query_books(&LibraryQuery::default().with_identifier("ISBN"))
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            backend
                .query_books(&LibraryQuery::default().with_identifier("abc-1"))
                .unwrap()
                .len(),
            1
        );
        let db = Connection::open(dir.path().join("metadata.db")).unwrap();
        db.execute("INSERT INTO tags VALUES(2,'100% literal')", [])
            .unwrap();
        db.execute("INSERT INTO books_tags_link VALUES(2,2,2)", [])
            .unwrap();
        assert_eq!(
            backend
                .query_books(&LibraryQuery::default().with_tag("100%"))
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn structured_wildcards_are_literal_for_include_and_exclude() {
        let dir = fixture();
        let db = Connection::open(dir.path().join("metadata.db")).unwrap();
        db.execute("INSERT INTO tags VALUES(2,'100% literal'),(3,'100X literal'),(4,'under_score'),(5,'underXscore')", [])
            .unwrap();
        db.execute(
            "INSERT INTO books_tags_link VALUES(2,2,2),(3,3,3),(4,2,4),(5,3,5)",
            [],
        )
        .unwrap();
        let backend = CalibreLibraryBackend::open(dir.path()).unwrap();
        let include_percent = LibraryQuery::default().with_metadata_filter(
            LibraryMetadataFilterField::Tags,
            LibraryMetadataFilterMode::Include,
            "100%",
        );
        let exclude_percent = LibraryQuery::default().with_metadata_filter(
            LibraryMetadataFilterField::Tags,
            LibraryMetadataFilterMode::Exclude,
            "100%",
        );
        assert_eq!(backend.query_page(&include_percent).unwrap().total, 1);
        assert_eq!(backend.query_page(&exclude_percent).unwrap().total, 2);
        let include_underscore = LibraryQuery::default().with_metadata_filter(
            LibraryMetadataFilterField::Tags,
            LibraryMetadataFilterMode::Include,
            "under_",
        );
        let exclude_underscore = LibraryQuery::default().with_metadata_filter(
            LibraryMetadataFilterField::Tags,
            LibraryMetadataFilterMode::Exclude,
            "under_",
        );
        assert_eq!(backend.query_page(&include_underscore).unwrap().total, 1);
        assert_eq!(backend.query_page(&exclude_underscore).unwrap().total, 2);
    }

    #[test]
    fn multiple_structured_filters_match_only_the_intersection() {
        let dir = fixture();
        let backend = CalibreLibraryBackend::open(dir.path()).unwrap();
        let query = LibraryQuery::default()
            .with_metadata_filter(
                LibraryMetadataFilterField::Authors,
                LibraryMetadataFilterMode::Include,
                "Author A",
            )
            .with_metadata_filter(
                LibraryMetadataFilterField::Tags,
                LibraryMetadataFilterMode::Include,
                "fiction",
            );
        let page = backend.query_page(&query).unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.books[0].id, 1);
    }

    #[test]
    fn all_sorts_and_filter_modes_are_deterministic() {
        let dir = fixture();
        let db = Connection::open(dir.path().join("metadata.db")).unwrap();
        db.execute("UPDATE books SET series_index=2.0 WHERE id=2", [])
            .unwrap();
        db.execute("INSERT INTO books_series_link VALUES(2,2,1)", [])
            .unwrap();
        let backend = CalibreLibraryBackend::open(dir.path()).unwrap();
        let expected = [
            (LibrarySortField::Id, [1, 2, 3]),
            (LibrarySortField::Title, [1, 2, 3]),
            (LibrarySortField::Authors, [3, 1, 2]),
            (LibrarySortField::Series, [3, 1, 2]),
            (LibrarySortField::Tags, [2, 3, 1]),
            (LibrarySortField::Format, [3, 2, 1]),
            (LibrarySortField::Rating, [2, 3, 1]),
            (LibrarySortField::Publisher, [2, 3, 1]),
            (LibrarySortField::Languages, [2, 3, 1]),
            (LibrarySortField::DateAdded, [1, 2, 3]),
            (LibrarySortField::DateModified, [2, 3, 1]),
            (LibrarySortField::PubDate, [2, 3, 1]),
        ];
        for (field, ids) in expected {
            let page = backend
                .query_page(&LibraryQuery::default().with_sort(field))
                .unwrap();
            assert_eq!(page.total, 3);
            assert_eq!(page.books.len(), 3);
            assert_eq!(
                page.books.iter().map(|book| book.id).collect::<Vec<_>>(),
                ids
            );
        }
        assert_eq!(
            backend
                .query_books(
                    &LibraryQuery::default()
                        .with_sort(LibrarySortField::Title)
                        .descending()
                )
                .unwrap()
                .iter()
                .map(|book| book.id)
                .collect::<Vec<_>>(),
            [3, 2, 1]
        );
        assert_eq!(
            backend
                .query_books(
                    &LibraryQuery::default()
                        .with_sort(LibrarySortField::Series)
                        .descending()
                )
                .unwrap()
                .iter()
                .map(|book| book.id)
                .collect::<Vec<_>>(),
            [2, 1, 3]
        );
        for field in [
            LibraryMetadataFilterField::Authors,
            LibraryMetadataFilterField::Tags,
            LibraryMetadataFilterField::Series,
            LibraryMetadataFilterField::Publishers,
            LibraryMetadataFilterField::Ratings,
            LibraryMetadataFilterField::Languages,
        ] {
            let value = match field {
                LibraryMetadataFilterField::Authors => "Author A",
                LibraryMetadataFilterField::Tags => "fiction",
                LibraryMetadataFilterField::Series => "Series A",
                LibraryMetadataFilterField::Publishers => "Publisher A",
                LibraryMetadataFilterField::Ratings => "8",
                LibraryMetadataFilterField::Languages => "EN",
            };
            let include = LibraryQuery::default().with_metadata_filter(
                field,
                LibraryMetadataFilterMode::Include,
                value,
            );
            let exclude = LibraryQuery::default().with_metadata_filter(
                field,
                LibraryMetadataFilterMode::Exclude,
                value,
            );
            let expected_include = if field == LibraryMetadataFilterField::Series {
                2
            } else {
                1
            };
            assert_eq!(
                backend.query_page(&include).unwrap().total,
                expected_include
            );
            assert_eq!(
                backend.query_page(&exclude).unwrap().total,
                3 - expected_include
            );
        }
    }

    #[test]
    fn paging_and_combined_filters_keep_full_total() {
        let dir = fixture();
        let backend = CalibreLibraryBackend::open(dir.path()).unwrap();
        let query = LibraryQuery::default()
            .with_author("Author")
            .with_metadata_filter(
                LibraryMetadataFilterField::Tags,
                LibraryMetadataFilterMode::Include,
                "fiction",
            )
            .with_limit(1);
        let page = backend.query_page(&query).unwrap();
        assert_eq!(page.books.len(), 1);
        assert_eq!(page.total, 1);
        assert_eq!(backend.query_summary_page(&query).unwrap().books.len(), 1);
    }

    #[test]
    fn relation_sort_keys_use_case_insensitive_order_with_id_ties() {
        let dir = fixture();
        let db = Connection::open(dir.path().join("metadata.db")).unwrap();
        db.execute("UPDATE authors SET name='alpha' WHERE id=1", [])
            .unwrap();
        db.execute("UPDATE authors SET name='Alpha' WHERE id=2", [])
            .unwrap();
        let backend = CalibreLibraryBackend::open(dir.path()).unwrap();
        let ascending = backend
            .query_books(&LibraryQuery::default().with_sort(LibrarySortField::Authors))
            .unwrap();
        assert_eq!(
            ascending.iter().map(|b| b.id).collect::<Vec<_>>(),
            [3, 1, 2]
        );
        let descending = backend
            .query_books(
                &LibraryQuery::default()
                    .with_sort(LibrarySortField::Authors)
                    .descending(),
            )
            .unwrap();
        assert_eq!(
            descending.iter().map(|b| b.id).collect::<Vec<_>>(),
            [1, 2, 3]
        );
    }
}
