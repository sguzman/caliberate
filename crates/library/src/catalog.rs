use caliberate_core::error::CoreResult;
use caliberate_db::database::{BookRecord, Database};

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

pub struct LibraryCatalog<'a> {
    db: &'a Database,
}

impl<'a> LibraryCatalog<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub fn list_books(&self) -> CoreResult<Vec<LibraryBook>> {
        self.db
            .list_books()
            .map(|books| books.into_iter().map(LibraryBook::from).collect())
    }

    pub fn get_book(&self, id: i64) -> CoreResult<Option<LibraryBook>> {
        self.db.get_book(id).map(|book| book.map(LibraryBook::from))
    }

    pub fn search_books(&self, query: &str) -> CoreResult<Vec<LibraryBook>> {
        self.db
            .search_books(query)
            .map(|books| books.into_iter().map(LibraryBook::from).collect())
    }

    pub fn resolve_content(&self, book_id: i64) -> CoreResult<Option<LibraryContent>> {
        let Some(book) = self.db.get_book(book_id)? else {
            return Ok(None);
        };

        let assets = self.db.list_assets_for_book(book_id)?;
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
