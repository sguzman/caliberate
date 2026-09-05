use crate::query::{LibraryFacetKind, LibraryFacetValue, LibraryQuery, LibraryQueryPage};
use crate::summary::{LibraryBookSummary, LibrarySeriesSummary, LibrarySummaryPage};
use caliberate_core::error::CoreResult;
use caliberate_db::database::{BookRecord, BookSummaryRecord, Database};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryBook {
    pub id: i64,
    pub title: String,
    pub format: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryContent {
    pub book_id: i64,
    pub format: String,
    pub path: String,
    pub storage_mode: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryFormat {
    pub format: String,
    pub size_bytes: Option<u64>,
}

impl From<BookRecord> for LibraryBook {
    fn from(book: BookRecord) -> Self {
        Self {
            id: book.id,
            title: book.title,
            format: book.format,
            path: book.path,
        }
    }
}

pub trait LibraryBackend {
    fn list_books(&self) -> CoreResult<Vec<LibraryBook>>;
    fn get_book(&self, id: i64) -> CoreResult<Option<LibraryBook>>;
    fn search_books(&self, query: &str) -> CoreResult<Vec<LibraryBook>>;
    fn query_books(&self, query: &LibraryQuery) -> CoreResult<Vec<LibraryBook>>;
    fn query_page(&self, query: &LibraryQuery) -> CoreResult<LibraryQueryPage>;
    fn query_summary_page(&self, query: &LibraryQuery) -> CoreResult<LibrarySummaryPage>;
    fn list_facets(&self, kind: LibraryFacetKind) -> CoreResult<Vec<LibraryFacetValue>>;
    fn resolve_content(&self, book_id: i64) -> CoreResult<Option<LibraryContent>>;
    fn list_formats(&self, book_id: i64) -> CoreResult<Vec<LibraryFormat>>;
    fn resolve_content_format(
        &self,
        book_id: i64,
        format: &str,
    ) -> CoreResult<Option<LibraryContent>>;
}

pub struct LibraryCatalog<'a> {
    backend: &'a dyn LibraryBackend,
}

impl<'a> LibraryCatalog<'a> {
    pub fn new(backend: &'a dyn LibraryBackend) -> Self {
        Self { backend }
    }

    pub fn list_books(&self) -> CoreResult<Vec<LibraryBook>> {
        self.backend.list_books()
    }

    pub fn get_book(&self, id: i64) -> CoreResult<Option<LibraryBook>> {
        self.backend.get_book(id)
    }

    pub fn search_books(&self, query: &str) -> CoreResult<Vec<LibraryBook>> {
        self.backend.search_books(query)
    }

    pub fn query_books(&self, query: &LibraryQuery) -> CoreResult<Vec<LibraryBook>> {
        self.backend.query_books(query)
    }

    pub fn query_page(&self, query: &LibraryQuery) -> CoreResult<LibraryQueryPage> {
        self.backend.query_page(query)
    }

    pub fn query_summary_page(&self, query: &LibraryQuery) -> CoreResult<LibrarySummaryPage> {
        self.backend.query_summary_page(query)
    }

    pub fn list_facets(&self, kind: LibraryFacetKind) -> CoreResult<Vec<LibraryFacetValue>> {
        self.backend.list_facets(kind)
    }

    pub fn resolve_content(&self, book_id: i64) -> CoreResult<Option<LibraryContent>> {
        self.backend.resolve_content(book_id)
    }

    pub fn list_formats(&self, book_id: i64) -> CoreResult<Vec<LibraryFormat>> {
        self.backend.list_formats(book_id)
    }

    pub fn resolve_content_format(
        &self,
        book_id: i64,
        format: &str,
    ) -> CoreResult<Option<LibraryContent>> {
        self.backend.resolve_content_format(book_id, format)
    }
}

impl LibraryBackend for Database {
    fn list_books(&self) -> CoreResult<Vec<LibraryBook>> {
        self.list_books()
            .map(|books| books.into_iter().map(LibraryBook::from).collect())
    }

    fn get_book(&self, id: i64) -> CoreResult<Option<LibraryBook>> {
        self.get_book(id).map(|book| book.map(LibraryBook::from))
    }

    fn search_books(&self, query: &str) -> CoreResult<Vec<LibraryBook>> {
        self.search_books(query)
            .map(|books| books.into_iter().map(LibraryBook::from).collect())
    }

    fn query_books(&self, query: &LibraryQuery) -> CoreResult<Vec<LibraryBook>> {
        self.search_books_query(&query.to_db_query())
            .map(|books| books.into_iter().map(LibraryBook::from).collect())
    }

    fn query_page(&self, query: &LibraryQuery) -> CoreResult<LibraryQueryPage> {
        let books = self.query_books(query)?;
        let total = self.count_books_query(&query.to_db_query())?;
        Ok(LibraryQueryPage {
            books,
            total,
            offset: query.offset.unwrap_or(0),
            limit: query.limit,
        })
    }

    fn query_summary_page(&self, query: &LibraryQuery) -> CoreResult<LibrarySummaryPage> {
        let records = self.search_book_summaries_query(&query.to_db_query())?;
        let total = self.count_books_query(&query.to_db_query())?;
        let ids = records.iter().map(|record| record.id).collect::<Vec<_>>();
        let format_rows = self.list_book_formats_for_books(&ids)?;
        Ok(LibrarySummaryPage {
            books: records
                .into_iter()
                .map(|record| {
                    let id = record.id;
                    let mut summary = LibraryBookSummary::from(record);
                    summary.formats = format_rows
                        .get(&id)
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .map(|format| LibraryFormat {
                            format: format.format,
                            size_bytes: format.size_bytes,
                        })
                        .collect();
                    summary
                })
                .collect(),
            total,
            offset: query.offset.unwrap_or(0),
            limit: query.limit,
        })
    }

    fn list_facets(&self, kind: LibraryFacetKind) -> CoreResult<Vec<LibraryFacetValue>> {
        let values = match kind {
            LibraryFacetKind::Authors => self.list_author_categories()?,
            LibraryFacetKind::Tags => self.list_tag_categories()?,
            LibraryFacetKind::Series => self.list_series_categories()?,
            LibraryFacetKind::Publishers => self.list_publisher_categories()?,
            LibraryFacetKind::Ratings => self.list_rating_categories()?,
            LibraryFacetKind::Languages => self.list_language_categories()?,
        };

        Ok(values
            .into_iter()
            .map(|value| LibraryFacetValue {
                id: value.id,
                name: value.name,
                count: value.count,
            })
            .collect())
    }

    fn resolve_content(&self, book_id: i64) -> CoreResult<Option<LibraryContent>> {
        let Some(book) = self.get_book(book_id)? else {
            return Ok(None);
        };
        if !book.format.is_empty() {
            if let Some(content) = self.resolve_content_format(book_id, &book.format)? {
                return Ok(Some(content));
            }
        }
        Ok(Some(LibraryContent {
            book_id: book.id,
            format: book.format,
            path: book.path,
            storage_mode: None,
        }))
    }

    fn list_formats(&self, book_id: i64) -> CoreResult<Vec<LibraryFormat>> {
        let rows = Database::list_book_formats(self, book_id)?;
        if rows.is_empty() {
            let Some(book) = self.get_book(book_id)? else {
                return Ok(Vec::new());
            };
            if book.format.is_empty() {
                return Ok(Vec::new());
            }
            return Ok(vec![LibraryFormat {
                format: book.format.to_ascii_lowercase(),
                size_bytes: None,
            }]);
        }
        Ok(rows
            .into_iter()
            .map(|row| LibraryFormat {
                format: row.format,
                size_bytes: row.size_bytes,
            })
            .collect())
    }

    fn resolve_content_format(
        &self,
        book_id: i64,
        format: &str,
    ) -> CoreResult<Option<LibraryContent>> {
        let Some(book) = self.get_book(book_id)? else {
            return Ok(None);
        };
        if !book.format.eq_ignore_ascii_case(format)
            && self.get_book_format(book_id, format)?.is_none()
        {
            return Ok(None);
        }
        let logical = self.get_book_format(book_id, format)?;
        if let Some(logical) = logical {
            let asset = self
                .list_assets_for_book(book_id)?
                .into_iter()
                .filter(|asset| asset.book_format_id == Some(logical.id))
                .min_by_key(|asset| (asset.storage_mode != "copy", asset.id));
            if let Some(asset) = asset {
                return Ok(Some(LibraryContent {
                    book_id,
                    format: logical.format,
                    path: asset.stored_path,
                    storage_mode: Some(asset.storage_mode),
                }));
            }
        }
        if book.format.eq_ignore_ascii_case(format) {
            Ok(Some(LibraryContent {
                book_id,
                format: book.format.to_ascii_lowercase(),
                path: book.path,
                storage_mode: None,
            }))
        } else {
            Ok(None)
        }
    }
}

impl From<BookSummaryRecord> for LibraryBookSummary {
    fn from(record: BookSummaryRecord) -> Self {
        let formats = if record.format.is_empty() {
            Vec::new()
        } else {
            vec![LibraryFormat {
                format: record.format.to_ascii_lowercase(),
                size_bytes: None,
            }]
        };
        Self {
            id: record.id,
            title: record.title,
            format: record.format,
            path: record.path,
            formats,
            authors: record.authors,
            tags: record.tags,
            series: record.series.map(|series| LibrarySeriesSummary {
                name: series.name,
                index: series.index,
            }),
            rating: record.rating,
            publisher: record.publisher,
            languages: record.languages,
            has_cover: record.has_cover,
            date_added: record.timestamp,
            date_modified: record.last_modified,
            pubdate: record.pubdate,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LibraryBook, LibraryCatalog};
    use caliberate_db::database::Database;
    use tempfile::TempDir;

    #[test]
    fn lists_multiple_books_as_library_books() {
        let (_temp_dir, db) = seeded_database();
        let catalog = LibraryCatalog::new(&db);

        let books = catalog.list_books().expect("list books");

        assert_eq!(
            books,
            vec![
                LibraryBook {
                    id: 1,
                    title: "The Hobbit".to_string(),
                    format: "epub".to_string(),
                    path: "/library/hobbit.epub".to_string(),
                },
                LibraryBook {
                    id: 2,
                    title: "Dune".to_string(),
                    format: "pdf".to_string(),
                    path: "/library/dune.pdf".to_string(),
                },
            ]
        );
    }

    #[test]
    fn gets_existing_book_and_missing_id() {
        let (_temp_dir, db) = seeded_database();
        let catalog = LibraryCatalog::new(&db);

        assert_eq!(
            catalog
                .get_book(2)
                .expect("get existing book")
                .unwrap()
                .title,
            "Dune"
        );
        assert_eq!(catalog.get_book(999).expect("get missing book"), None);
    }

    #[test]
    fn searches_as_library_books() {
        let (_temp_dir, db) = seeded_database();
        let catalog = LibraryCatalog::new(&db);

        let books = catalog.search_books("Hobbit").expect("search books");

        assert_eq!(books.len(), 1);
        assert_eq!(books[0].title, "The Hobbit");
        assert_eq!(books[0].format, "epub");
    }

    #[test]
    fn resolves_copied_asset_before_earlier_reference_asset() {
        let (_temp_dir, db) = seeded_database();
        db.add_asset(
            1,
            "reference",
            "C:/books/hobbit.epub",
            None,
            10,
            10,
            None,
            false,
            "2026-04-03T00:00:00Z",
        )
        .expect("add reference asset");
        db.add_asset(
            1,
            "copy",
            "C:/managed/hobbit.epub",
            None,
            10,
            10,
            None,
            false,
            "2026-04-04T00:00:00Z",
        )
        .expect("add copied asset");

        let content = LibraryCatalog::new(&db)
            .resolve_content(1)
            .expect("resolve content")
            .expect("content missing");

        assert_eq!(content.book_id, 1);
        assert_eq!(content.format, "epub");
        assert_eq!(content.path, "C:/managed/hobbit.epub");
        assert_eq!(content.storage_mode.as_deref(), Some("copy"));
    }

    #[test]
    fn resolves_first_asset_when_no_copy_exists() {
        let (_temp_dir, db) = seeded_database();
        db.add_asset(
            1,
            "reference",
            "C:/books/first.epub",
            None,
            10,
            10,
            None,
            false,
            "2026-04-03T00:00:00Z",
        )
        .expect("add first asset");
        db.add_asset(
            1,
            "compressed",
            "C:/books/second.epub",
            None,
            10,
            10,
            None,
            true,
            "2026-04-04T00:00:00Z",
        )
        .expect("add second asset");

        let content = LibraryCatalog::new(&db)
            .resolve_content(1)
            .expect("resolve content")
            .expect("content missing");

        assert_eq!(content.path, "C:/books/first.epub");
        assert_eq!(content.storage_mode.as_deref(), Some("reference"));
    }

    #[test]
    fn resolves_logical_book_path_when_no_assets_exist() {
        let (_temp_dir, db) = seeded_database();

        let content = LibraryCatalog::new(&db)
            .resolve_content(1)
            .expect("resolve content")
            .expect("content missing");

        assert_eq!(content.book_id, 1);
        assert_eq!(content.format, "epub");
        assert_eq!(content.path, "/library/hobbit.epub");
        assert_eq!(content.storage_mode, None);
    }

    #[test]
    fn returns_none_for_missing_book_content() {
        let (_temp_dir, db) = seeded_database();

        assert_eq!(
            LibraryCatalog::new(&db)
                .resolve_content(999)
                .expect("resolve missing content"),
            None
        );
    }

    #[test]
    fn database_exposes_only_its_canonical_format_and_preserves_content_selection() {
        let (_temp_dir, db) = seeded_database();
        let catalog = LibraryCatalog::new(&db);

        assert_eq!(
            catalog.list_formats(1).unwrap(),
            vec![super::LibraryFormat {
                format: "epub".into(),
                size_bytes: None,
            }]
        );
        let summary = catalog
            .query_summary_page(&crate::query::LibraryQuery::default())
            .unwrap();
        assert_eq!(summary.books[0].formats.len(), 1);
        assert_eq!(summary.books[0].formats[0].format, "epub");
        assert_eq!(summary.books[0].formats[0].size_bytes, None);
        assert_eq!(
            catalog.resolve_content_format(1, "EPUB").unwrap(),
            catalog.resolve_content(1).unwrap()
        );
        assert_eq!(catalog.resolve_content_format(1, "pdf").unwrap(), None);
        assert_eq!(catalog.list_formats(999).unwrap(), Vec::new());
        assert_eq!(catalog.resolve_content_format(999, "epub").unwrap(), None);
    }

    #[test]
    fn managed_summary_and_content_use_canonical_logical_formats() {
        let (_temp_dir, db) = seeded_database();
        let pdf = db.upsert_book_format(1, "PDF", Some(200)).unwrap();
        let mobi = db.upsert_book_format(1, "MOBI", None).unwrap();
        let empty = db
            .add_book("Empty", "", "", "2026-04-03T00:00:00Z")
            .unwrap();
        db.add_asset(
            1,
            "reference",
            "/reference/epub",
            None,
            10,
            10,
            None,
            false,
            "2026-04-03T00:00:00Z",
        )
        .unwrap();
        db.add_asset_for_format(
            1,
            pdf,
            None,
            "reference",
            "/reference/pdf",
            None,
            200,
            200,
            None,
            false,
            "2026-04-03T00:00:00Z",
        )
        .unwrap();
        db.add_asset_for_format(
            1,
            db.get_book_format(1, "epub").unwrap().unwrap().id,
            None,
            "copy",
            "/managed/epub",
            None,
            10,
            10,
            None,
            false,
            "2026-04-04T00:00:00Z",
        )
        .unwrap();
        let catalog = LibraryCatalog::new(&db);
        let summary = catalog
            .query_summary_page(&crate::query::LibraryQuery::default())
            .unwrap();
        assert_eq!(summary.total, 3);
        assert_eq!(summary.books[0].format, "epub");
        assert_eq!(
            summary.books[0]
                .formats
                .iter()
                .map(|format| format.format.as_str())
                .collect::<Vec<_>>(),
            ["epub", "pdf", "mobi"]
        );
        assert_eq!(summary.books[2].id, empty);
        assert!(summary.books[2].formats.is_empty());
        assert_eq!(
            catalog
                .resolve_content_format(1, "PDF")
                .unwrap()
                .unwrap()
                .path,
            "/reference/pdf"
        );
        assert_eq!(
            catalog
                .resolve_content_format(1, "EPUB")
                .unwrap()
                .unwrap()
                .path,
            "/managed/epub"
        );
        let page = catalog
            .query_summary_page(&crate::query::LibraryQuery::default().with_limit(1))
            .unwrap();
        assert_eq!(page.total, 3);
        assert_eq!(page.books.len(), 1);
        assert_eq!(page.books[0].id, 1);
        assert_eq!(pdf, db.get_book_format(1, "pdf").unwrap().unwrap().id);
        assert_eq!(mobi, db.get_book_format(1, "mobi").unwrap().unwrap().id);
    }

    fn seeded_database() -> (TempDir, Database) {
        let temp_dir = tempfile::Builder::new()
            .prefix("caliberate-library-catalog-")
            .tempdir()
            .expect("create temp directory");
        let db =
            Database::open_path(temp_dir.path().join("library.db"), 100).expect("open database");
        db.add_book(
            "The Hobbit",
            "epub",
            "/library/hobbit.epub",
            "2026-04-01T00:00:00Z",
        )
        .expect("add Hobbit");
        db.add_book("Dune", "pdf", "/library/dune.pdf", "2026-04-02T00:00:00Z")
            .expect("add Dune");
        (temp_dir, db)
    }
}

#[cfg(test)]
mod fake_backend_tests {
    use super::{LibraryBackend, LibraryBook, LibraryCatalog, LibraryContent, LibraryFormat};
    use crate::query::{LibraryFacetKind, LibraryFacetValue, LibraryQuery, LibraryQueryPage};
    use crate::summary::{LibraryBookSummary, LibrarySummaryPage};
    use caliberate_core::error::CoreResult;
    use std::cell::RefCell;

    struct FakeBackend {
        last_query: RefCell<Option<LibraryQuery>>,
    }

    impl FakeBackend {
        fn book() -> LibraryBook {
            LibraryBook {
                id: 7,
                title: "Fake Book".to_string(),
                format: "epub".to_string(),
                path: "/fake/book.epub".to_string(),
            }
        }
    }

    impl LibraryBackend for FakeBackend {
        fn list_books(&self) -> CoreResult<Vec<LibraryBook>> {
            Ok(vec![Self::book()])
        }

        fn get_book(&self, id: i64) -> CoreResult<Option<LibraryBook>> {
            Ok((id == 7).then(Self::book))
        }

        fn search_books(&self, _query: &str) -> CoreResult<Vec<LibraryBook>> {
            self.list_books()
        }

        fn query_books(&self, query: &LibraryQuery) -> CoreResult<Vec<LibraryBook>> {
            *self.last_query.borrow_mut() = Some(query.clone());
            self.list_books()
        }

        fn query_page(&self, query: &LibraryQuery) -> CoreResult<LibraryQueryPage> {
            Ok(LibraryQueryPage {
                books: self.query_books(query)?,
                total: 1,
                offset: query.offset.unwrap_or(0),
                limit: query.limit,
            })
        }

        fn query_summary_page(&self, _query: &LibraryQuery) -> CoreResult<LibrarySummaryPage> {
            Ok(LibrarySummaryPage {
                books: vec![LibraryBookSummary {
                    id: 7,
                    title: "Fake Book".to_string(),
                    format: "epub".to_string(),
                    path: "/fake/book.epub".to_string(),
                    formats: vec![LibraryFormat {
                        format: "epub".to_string(),
                        size_bytes: Some(42),
                    }],
                    authors: vec!["Fake Author".to_string()],
                    tags: Vec::new(),
                    series: None,
                    rating: None,
                    publisher: None,
                    languages: Vec::new(),
                    has_cover: false,
                    date_added: None,
                    date_modified: None,
                    pubdate: None,
                }],
                total: 1,
                offset: 0,
                limit: None,
            })
        }

        fn list_facets(&self, _kind: LibraryFacetKind) -> CoreResult<Vec<LibraryFacetValue>> {
            Ok(Vec::new())
        }

        fn resolve_content(&self, book_id: i64) -> CoreResult<Option<LibraryContent>> {
            Ok((book_id == 7).then(|| LibraryContent {
                book_id,
                format: "epub".to_string(),
                path: "/fake/book.epub".to_string(),
                storage_mode: Some("reference".to_string()),
            }))
        }

        fn list_formats(&self, book_id: i64) -> CoreResult<Vec<LibraryFormat>> {
            Ok((book_id == 7)
                .then_some(vec![LibraryFormat {
                    format: "epub".to_string(),
                    size_bytes: Some(42),
                }])
                .unwrap_or_default())
        }

        fn resolve_content_format(
            &self,
            book_id: i64,
            format: &str,
        ) -> CoreResult<Option<LibraryContent>> {
            if book_id == 7 && format.eq_ignore_ascii_case("epub") {
                self.resolve_content(book_id)
            } else {
                Ok(None)
            }
        }
    }

    #[test]
    fn catalog_accepts_backend_without_database_and_delegates_domain_calls() {
        let backend = FakeBackend {
            last_query: RefCell::new(None),
        };
        let catalog = LibraryCatalog::new(&backend);

        assert_eq!(catalog.list_books().unwrap()[0].title, "Fake Book");
        let query = LibraryQuery::new().with_title("delegated");
        assert_eq!(catalog.query_books(&query).unwrap().len(), 1);
        assert_eq!(backend.last_query.borrow().as_ref(), Some(&query));

        assert_eq!(
            catalog.list_formats(7).unwrap(),
            vec![LibraryFormat {
                format: "epub".to_string(),
                size_bytes: Some(42),
            }]
        );
        assert_eq!(
            catalog.resolve_content_format(7, "EPUB").unwrap(),
            catalog.resolve_content(7).unwrap()
        );

        assert_eq!(
            catalog.resolve_content(7).unwrap(),
            Some(LibraryContent {
                book_id: 7,
                format: "epub".to_string(),
                path: "/fake/book.epub".to_string(),
                storage_mode: Some("reference".to_string()),
            })
        );
    }
}
