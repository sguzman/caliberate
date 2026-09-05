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
        Ok(LibrarySummaryPage {
            books: records.into_iter().map(LibraryBookSummary::from).collect(),
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

        let assets = self.list_assets_for_book(book_id)?;
        if let Some(asset) = assets
            .iter()
            .find(|asset| asset.storage_mode == "copy")
            .or_else(|| assets.first())
        {
            return Ok(Some(LibraryContent {
                book_id: book.id,
                format: book.format,
                path: asset.stored_path.clone(),
                storage_mode: Some(asset.storage_mode.clone()),
            }));
        }

        Ok(Some(LibraryContent {
            book_id: book.id,
            format: book.format,
            path: book.path,
            storage_mode: None,
        }))
    }
}

impl From<BookSummaryRecord> for LibraryBookSummary {
    fn from(record: BookSummaryRecord) -> Self {
        Self {
            id: record.id,
            title: record.title,
            format: record.format,
            path: record.path,
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
    use super::{LibraryBackend, LibraryBook, LibraryCatalog, LibraryContent};
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
