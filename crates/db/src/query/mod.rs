//! Query and search interfaces.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookSortField {
    Id,
    Title,
    Authors,
    Series,
    Tags,
    Format,
    Rating,
    Publisher,
    Languages,
    DateAdded,
    DateModified,
    PubDate,
}

impl Default for BookSortField {
    fn default() -> Self {
        Self::Id
    }
}

#[derive(Debug, Clone)]
pub struct BookQuery {
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
    pub sort: BookSortField,
    pub descending: bool,
}

impl Default for BookQuery {
    fn default() -> Self {
        Self {
            title: None,
            author: None,
            tag: None,
            series: None,
            publisher: None,
            language: None,
            identifier: None,
            format: None,
            limit: None,
            offset: None,
            sort: BookSortField::Id,
            descending: false,
        }
    }
}

impl BookQuery {
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

    pub fn with_offset(mut self, value: usize) -> Self {
        self.offset = Some(value);
        self
    }

    pub fn with_sort(mut self, field: BookSortField) -> Self {
        self.sort = field;
        self
    }

    pub fn descending(mut self) -> Self {
        self.descending = true;
        self
    }
}
