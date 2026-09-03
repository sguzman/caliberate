use caliberate_db::query::{BookQuery, BookSortField};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LibraryQuery {
    pub title: Option<String>,
    pub author: Option<String>,
    pub tag: Option<String>,
    pub series: Option<String>,
    pub publisher: Option<String>,
    pub language: Option<String>,
    pub identifier: Option<String>,
    pub format: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub sort: LibrarySortField,
    pub descending: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibrarySortField {
    Id,
    Title,
    Format,
}

impl Default for LibrarySortField {
    fn default() -> Self {
        Self::Id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryQueryPage {
    pub books: Vec<crate::catalog::LibraryBook>,
    pub total: usize,
    pub offset: usize,
    pub limit: Option<usize>,
}

impl LibraryQuery {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn with_title(mut self, value: &str) -> Self {
        self.title = Some(value.to_string());
        self
    }
    pub fn with_author(mut self, value: &str) -> Self {
        self.author = Some(value.to_string());
        self
    }
    pub fn with_tag(mut self, value: &str) -> Self {
        self.tag = Some(value.to_string());
        self
    }
    pub fn with_series(mut self, value: &str) -> Self {
        self.series = Some(value.to_string());
        self
    }
    pub fn with_publisher(mut self, value: &str) -> Self {
        self.publisher = Some(value.to_string());
        self
    }
    pub fn with_language(mut self, value: &str) -> Self {
        self.language = Some(value.to_string());
        self
    }
    pub fn with_identifier(mut self, value: &str) -> Self {
        self.identifier = Some(value.to_string());
        self
    }
    pub fn with_format(mut self, value: &str) -> Self {
        self.format = Some(value.to_string());
        self
    }
    pub fn with_limit(mut self, value: usize) -> Self {
        self.limit = Some(value);
        self
    }

    pub(crate) fn to_db_query(&self) -> BookQuery {
        BookQuery {
            title: self.title.clone(),
            author: self.author.clone(),
            tag: self.tag.clone(),
            series: self.series.clone(),
            publisher: self.publisher.clone(),
            language: self.language.clone(),
            identifier: self.identifier.clone(),
            format: self.format.clone(),
            limit: self.limit,
            offset: self.offset,
            sort: match self.sort {
                LibrarySortField::Id => BookSortField::Id,
                LibrarySortField::Title => BookSortField::Title,
                LibrarySortField::Format => BookSortField::Format,
            },
            descending: self.descending,
        }
    }

    pub fn with_offset(mut self, value: usize) -> Self {
        self.offset = Some(value);
        self
    }

    pub fn with_sort(mut self, field: LibrarySortField) -> Self {
        self.sort = field;
        self
    }

    pub fn descending(mut self) -> Self {
        self.descending = true;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryFacetKind {
    Authors,
    Tags,
    Series,
    Publishers,
    Ratings,
    Languages,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryFacetValue {
    pub id: i64,
    pub name: String,
    pub count: i64,
}

#[cfg(test)]
mod tests {
    use super::{LibraryFacetKind, LibraryQuery, LibrarySortField};
    use crate::catalog::LibraryCatalog;
    use caliberate_db::database::Database;
    use tempfile::TempDir;

    #[test]
    fn query_filters_by_title_format_author_tag_and_limit() {
        let (_temp_dir, mut db) = seeded_database();
        let author = vec!["Ursula Le Guin".to_string()];
        let tag = vec!["classic".to_string()];
        db.add_book_authors(1, &author).expect("add author");
        db.add_book_tags(1, &tag).expect("add tag");
        db.add_book_authors(2, &author).expect("add author");
        db.add_book_tags(2, &tag).expect("add tag");
        let catalog = LibraryCatalog::new(&db);

        assert_eq!(
            catalog
                .query_books(&LibraryQuery::new().with_title("Earthsea"))
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            catalog
                .query_books(&LibraryQuery::new().with_format("epub"))
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            catalog
                .query_books(&LibraryQuery::new().with_author("Le Guin"))
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            catalog
                .query_books(&LibraryQuery::new().with_tag("classic"))
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            catalog
                .query_books(&LibraryQuery::new().with_limit(1))
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn facets_return_library_values_with_names_and_counts() {
        let (_temp_dir, mut db) = seeded_database();
        let author = vec!["Ursula Le Guin".to_string()];
        let tag = vec!["classic".to_string()];
        db.add_book_authors(1, &author).expect("add author");
        db.add_book_authors(2, &author).expect("add author");
        db.add_book_tags(1, &tag).expect("add tag");
        db.add_book_tags(2, &tag).expect("add tag");
        let catalog = LibraryCatalog::new(&db);

        let authors = catalog
            .list_facets(LibraryFacetKind::Authors)
            .expect("list authors");
        let tags = catalog
            .list_facets(LibraryFacetKind::Tags)
            .expect("list tags");
        assert_eq!(
            authors
                .iter()
                .find(|value| value.name == "Ursula Le Guin")
                .map(|value| value.count),
            Some(2)
        );
        assert_eq!(
            tags.iter()
                .find(|value| value.name == "classic")
                .map(|value| value.count),
            Some(2)
        );
    }

    #[test]
    fn query_page_maps_sort_pagination_and_total() {
        let (_temp_dir, db) = seeded_database();
        let catalog = LibraryCatalog::new(&db);
        let query = LibraryQuery::new()
            .with_sort(LibrarySortField::Title)
            .descending()
            .with_limit(1)
            .with_offset(1);

        let page = catalog.query_page(&query).expect("query page");

        assert_eq!(page.books.len(), 1);
        assert_eq!(page.books[0].title, "A Wizard of Earthsea");
        assert_eq!(page.total, 2);
        assert_eq!(page.offset, 1);
        assert_eq!(page.limit, Some(1));
    }

    #[test]
    fn query_summary_page_maps_domain_summary_and_dates() {
        let (_temp_dir, mut db) = seeded_database();
        db.set_book_series(1, "Earthsea", 1.5).expect("set series");
        db.set_book_publisher(1, "Ace").expect("set publisher");
        db.set_book_rating(1, 7).expect("set rating");
        db.set_book_languages(1, &["en".to_string()])
            .expect("set language");
        db.update_book_timestamp(1, "2026-04-02T00:00:00Z")
            .expect("set timestamp");
        db.update_book_last_modified(1, "2026-04-03T00:00:00Z")
            .expect("set modified");
        db.update_book_pubdate(1, "2026-01-01")
            .expect("set pubdate");
        db.update_book_has_cover(1, true).expect("set cover");

        let page = LibraryCatalog::new(&db)
            .query_summary_page(&LibraryQuery::new().with_title("Earthsea"))
            .expect("summary page");
        let book = &page.books[0];
        assert_eq!(book.title, "A Wizard of Earthsea");
        assert_eq!(
            book.series
                .as_ref()
                .map(|series| (&series.name, series.index)),
            Some((&"Earthsea".to_string(), 1.5))
        );
        assert_eq!(book.publisher.as_deref(), Some("Ace"));
        assert_eq!(book.rating, Some(7));
        assert_eq!(book.languages, ["en"]);
        assert!(book.has_cover);
        assert_eq!(book.date_added.as_deref(), Some("2026-04-02T00:00:00Z"));
        assert_eq!(book.date_modified.as_deref(), Some("2026-04-03T00:00:00Z"));
        assert_eq!(book.pubdate.as_deref(), Some("2026-01-01"));
        assert_eq!(page.total, 1);
    }

    fn seeded_database() -> (TempDir, Database) {
        let temp_dir = tempfile::Builder::new()
            .prefix("caliberate-library-query-")
            .tempdir()
            .expect("create temp directory");
        let db =
            Database::open_path(temp_dir.path().join("library.db"), 100).expect("open database");
        db.add_book(
            "A Wizard of Earthsea",
            "epub",
            "/library/earthsea.epub",
            "2026-04-01T00:00:00Z",
        )
        .expect("add Earthsea");
        db.add_book(
            "The Left Hand of Darkness",
            "epub",
            "/library/left-hand.epub",
            "2026-04-02T00:00:00Z",
        )
        .expect("add Left Hand");
        (temp_dir, db)
    }
}
