#[derive(Debug, Clone, PartialEq)]
pub struct LibrarySeriesSummary {
    pub name: String,
    pub index: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LibraryBookSummary {
    pub id: i64,
    pub title: String,
    pub format: String,
    pub path: String,
    pub authors: Vec<String>,
    pub tags: Vec<String>,
    pub series: Option<LibrarySeriesSummary>,
    pub rating: Option<i64>,
    pub publisher: Option<String>,
    pub languages: Vec<String>,
    pub has_cover: bool,
    pub date_added: Option<String>,
    pub date_modified: Option<String>,
    pub pubdate: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LibrarySummaryPage {
    pub books: Vec<LibraryBookSummary>,
    pub total: usize,
    pub offset: usize,
    pub limit: Option<usize>,
}
