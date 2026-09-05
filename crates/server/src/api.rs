//! Versioned source-neutral HTTP/JSON library API.

use crate::{ServerState, content};
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use caliberate_library::catalog::{LibraryBook, LibraryFormat};
use caliberate_library::query::{
    LibraryFacetKind, LibraryMetadataFilterField, LibraryMetadataFilterMode, LibraryQuery,
    LibrarySortField,
};
use caliberate_library::summary::LibraryBookSummary;
use serde::{Deserialize, Serialize};
use tracing::warn;

const DEFAULT_LIMIT: usize = 100;
const MAX_LIMIT: usize = 500;

#[derive(Debug, Serialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Debug, Serialize)]
struct ErrorDetail {
    code: &'static str,
    message: &'static str,
}

fn error(status: StatusCode, code: &'static str, message: &'static str) -> Response {
    (
        status,
        Json(ErrorBody {
            error: ErrorDetail { code, message },
        }),
    )
        .into_response()
}

#[derive(Debug, Serialize)]
struct Page<T> {
    items: Vec<T>,
    total: usize,
    offset: usize,
    limit: usize,
}

#[derive(Debug, Serialize)]
struct BookItem {
    id: i64,
    title: String,
    primary_format: String,
    format_count: usize,
    formats: Vec<SummaryFormatItem>,
    authors: Vec<String>,
    tags: Vec<String>,
    series: Option<SeriesItem>,
    rating: Option<i64>,
    publisher: Option<String>,
    languages: Vec<String>,
    has_cover: bool,
    date_added: Option<String>,
    date_modified: Option<String>,
    pubdate: Option<String>,
}

#[derive(Debug, Serialize)]
struct SummaryFormatItem {
    format: String,
    size_bytes: Option<u64>,
}

#[derive(Debug, Serialize)]
struct SeriesItem {
    name: String,
    index: f64,
}

#[derive(Debug, Serialize)]
struct SearchItem {
    id: i64,
    title: String,
    primary_format: String,
}

#[derive(Debug, Serialize)]
struct FormatItem {
    format: String,
    size_bytes: Option<u64>,
    content_href: String,
}

#[derive(Debug, Serialize)]
struct BookDetail {
    id: i64,
    title: String,
    primary_format: String,
    formats: Vec<FormatItem>,
    self_href: String,
    content_href: String,
}

#[derive(Debug, Serialize)]
struct FormatsResponse {
    book_id: i64,
    formats: Vec<FormatItem>,
}

#[derive(Debug, Serialize)]
struct FacetsResponse {
    kind: String,
    values: Vec<FacetItem>,
}

#[derive(Debug, Serialize)]
struct FacetItem {
    id: i64,
    name: String,
    count: i64,
}

#[derive(Debug, Deserialize, Default)]
pub struct BrowseParams {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub sort: Option<String>,
    pub direction: Option<String>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub tag: Option<String>,
    pub series: Option<String>,
    pub publisher: Option<String>,
    pub language: Option<String>,
    pub identifier: Option<String>,
    pub format: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct QueryRequest {
    #[serde(flatten)]
    pub browse: BrowseParams,
    pub metadata_filters: Option<Vec<MetadataFilterRequest>>,
}

#[derive(Debug, Deserialize)]
pub struct MetadataFilterRequest {
    pub field: String,
    pub mode: String,
    pub value: String,
}

pub async fn list_books(
    State(state): State<ServerState>,
    params: Result<Query<BrowseParams>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let Query(params) = match params {
        Ok(params) => params,
        Err(_) => {
            return error(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "query parameters are invalid",
            );
        }
    };
    let query = match build_query(&params, None) {
        Ok(query) => query,
        Err(response) => return response,
    };
    match state.with_catalog(|catalog| catalog.query_summary_page(&query)) {
        Ok(page) => Json(Page {
            items: page.books.iter().map(summary_item).collect(),
            total: page.total,
            offset: page.offset,
            limit: page.limit.unwrap_or(DEFAULT_LIMIT),
        })
        .into_response(),
        Err(err) => internal_error(&err, "query books"),
    }
}

pub async fn query_books(
    State(state): State<ServerState>,
    payload: Result<Json<QueryRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Json(request) = match payload {
        Ok(payload) => payload,
        Err(_) => {
            return error(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "request body is invalid JSON",
            );
        }
    };
    let query = match build_query(&request.browse, request.metadata_filters.as_deref()) {
        Ok(query) => query,
        Err(response) => return response,
    };
    match state.with_catalog(|catalog| catalog.query_summary_page(&query)) {
        Ok(page) => Json(Page {
            items: page.books.iter().map(summary_item).collect(),
            total: page.total,
            offset: page.offset,
            limit: page.limit.unwrap_or(DEFAULT_LIMIT),
        })
        .into_response(),
        Err(err) => internal_error(&err, "query books"),
    }
}

pub async fn search(
    State(state): State<ServerState>,
    params: Result<Query<SearchParams>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let Query(params) = match params {
        Ok(params) => params,
        Err(_) => {
            return error(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "query parameters are invalid",
            );
        }
    };
    let Some(term) = params.q.filter(|q| !q.trim().is_empty()) else {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "q is required and must not be empty",
        );
    };
    let (limit, offset) = match limits(params.limit, params.offset) {
        Ok(value) => value,
        Err(response) => return response,
    };
    match state.with_catalog(|catalog| catalog.search_books(&term)) {
        Ok(books) => {
            let total = books.len();
            let items = books
                .into_iter()
                .skip(offset)
                .take(limit)
                .map(search_item)
                .collect();
            Json(Page {
                items,
                total,
                offset,
                limit,
            })
            .into_response()
        }
        Err(err) => internal_error(&err, "search books"),
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct SearchParams {
    pub q: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

pub async fn book_detail(State(state): State<ServerState>, Path(id): Path<i64>) -> Response {
    match state.with_catalog(|catalog| {
        let Some(book) = catalog.get_book(id)? else {
            return Ok(None);
        };
        Ok(Some((book, catalog.list_formats(id)?)))
    }) {
        Ok(Some((book, formats))) => Json(detail(&state, book, formats)).into_response(),
        Ok(None) => error(StatusCode::NOT_FOUND, "not_found", "book was not found"),
        Err(err) => internal_error(&err, "read book detail"),
    }
}

pub async fn book_formats(State(state): State<ServerState>, Path(id): Path<i64>) -> Response {
    match state.with_catalog(|catalog| {
        let exists = catalog.get_book(id)?.is_some();
        Ok(if exists {
            Some(catalog.list_formats(id)?)
        } else {
            None
        })
    }) {
        Ok(Some(formats)) => Json(FormatsResponse {
            book_id: id,
            formats: format_items(&state, id, formats),
        })
        .into_response(),
        Ok(None) => error(StatusCode::NOT_FOUND, "not_found", "book was not found"),
        Err(err) => internal_error(&err, "list book formats"),
    }
}

pub async fn primary_content(State(state): State<ServerState>, Path(id): Path<i64>) -> Response {
    resolve_and_stream(state, id, None).await
}

pub async fn format_content(
    State(state): State<ServerState>,
    Path((id, format)): Path<(i64, String)>,
) -> Response {
    resolve_and_stream(state, id, Some(format)).await
}

async fn resolve_and_stream(state: ServerState, id: i64, format: Option<String>) -> Response {
    let result = state.with_catalog(|catalog| match format.as_deref() {
        Some(format) => catalog.resolve_content_format(id, format),
        None => catalog.resolve_content(id),
    });
    match result {
        Ok(Some(content)) => content::stream_content(&state, content).await,
        Ok(None) => error(StatusCode::NOT_FOUND, "not_found", "content was not found"),
        Err(err) => internal_error(&err, "resolve book content"),
    }
}

pub async fn facets(State(state): State<ServerState>, Path(kind): Path<String>) -> Response {
    let Some(facet_kind) = parse_facet(&kind) else {
        return error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "unknown facet kind",
        );
    };
    match state.with_catalog(|catalog| catalog.list_facets(facet_kind)) {
        Ok(values) => Json(FacetsResponse {
            kind,
            values: values
                .into_iter()
                .map(|value| FacetItem {
                    id: value.id,
                    name: value.name,
                    count: value.count,
                })
                .collect(),
        })
        .into_response(),
        Err(err) => internal_error(&err, "list facets"),
    }
}

fn build_query(
    params: &BrowseParams,
    filters: Option<&[MetadataFilterRequest]>,
) -> Result<LibraryQuery, Response> {
    let (limit, offset) = limits(params.limit, params.offset)?;
    let sort = parse_sort(params.sort.as_deref().unwrap_or("id"))?;
    let descending = match params.direction.as_deref().unwrap_or("asc") {
        "asc" => false,
        "desc" => true,
        _ => {
            return Err(error(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "direction must be asc or desc",
            ));
        }
    };
    let mut query = LibraryQuery::new()
        .with_limit(limit)
        .with_offset(offset)
        .with_sort(sort);
    query.descending = descending;
    query.title = params.title.clone();
    query.author = params.author.clone();
    query.tag = params.tag.clone();
    query.series = params.series.clone();
    query.publisher = params.publisher.clone();
    query.language = params.language.clone();
    query.identifier = params.identifier.clone();
    query.format = params.format.clone();
    if let Some(filters) = filters {
        for filter in filters {
            let field = parse_filter_field(&filter.field)?;
            let mode = match filter.mode.as_str() {
                "include" => LibraryMetadataFilterMode::Include,
                "exclude" => LibraryMetadataFilterMode::Exclude,
                _ => {
                    return Err(error(
                        StatusCode::BAD_REQUEST,
                        "invalid_request",
                        "unknown metadata filter mode",
                    ));
                }
            };
            query = query.with_metadata_filter(field, mode, &filter.value);
        }
    }
    Ok(query)
}

fn limits(limit: Option<usize>, offset: Option<usize>) -> Result<(usize, usize), Response> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT);
    if limit == 0 || limit > MAX_LIMIT {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "limit must be between 1 and 500",
        ));
    }
    Ok((limit, offset.unwrap_or(0)))
}

fn parse_sort(value: &str) -> Result<LibrarySortField, Response> {
    let field = match value {
        "id" => LibrarySortField::Id,
        "title" => LibrarySortField::Title,
        "authors" => LibrarySortField::Authors,
        "series" => LibrarySortField::Series,
        "tags" => LibrarySortField::Tags,
        "format" => LibrarySortField::Format,
        "rating" => LibrarySortField::Rating,
        "publisher" => LibrarySortField::Publisher,
        "languages" => LibrarySortField::Languages,
        "date_added" => LibrarySortField::DateAdded,
        "date_modified" => LibrarySortField::DateModified,
        "pubdate" => LibrarySortField::PubDate,
        _ => {
            return Err(error(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "unknown sort value",
            ));
        }
    };
    Ok(field)
}

fn parse_filter_field(value: &str) -> Result<LibraryMetadataFilterField, Response> {
    match value {
        "authors" => Ok(LibraryMetadataFilterField::Authors),
        "tags" => Ok(LibraryMetadataFilterField::Tags),
        "series" => Ok(LibraryMetadataFilterField::Series),
        "publishers" => Ok(LibraryMetadataFilterField::Publishers),
        "ratings" => Ok(LibraryMetadataFilterField::Ratings),
        "languages" => Ok(LibraryMetadataFilterField::Languages),
        _ => Err(error(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "unknown metadata filter field",
        )),
    }
}

fn parse_facet(value: &str) -> Option<LibraryFacetKind> {
    match value {
        "authors" => Some(LibraryFacetKind::Authors),
        "tags" => Some(LibraryFacetKind::Tags),
        "series" => Some(LibraryFacetKind::Series),
        "publishers" => Some(LibraryFacetKind::Publishers),
        "ratings" => Some(LibraryFacetKind::Ratings),
        "languages" => Some(LibraryFacetKind::Languages),
        _ => None,
    }
}

fn summary_item(book: &LibraryBookSummary) -> BookItem {
    BookItem {
        id: book.id,
        title: book.title.clone(),
        primary_format: book.format.clone(),
        format_count: book.formats.len(),
        formats: book
            .formats
            .iter()
            .map(|format| SummaryFormatItem {
                format: format.format.clone(),
                size_bytes: format.size_bytes,
            })
            .collect(),
        authors: book.authors.clone(),
        tags: book.tags.clone(),
        series: book.series.as_ref().map(|s| SeriesItem {
            name: s.name.clone(),
            index: s.index,
        }),
        rating: book.rating,
        publisher: book.publisher.clone(),
        languages: book.languages.clone(),
        has_cover: book.has_cover,
        date_added: book.date_added.clone(),
        date_modified: book.date_modified.clone(),
        pubdate: book.pubdate.clone(),
    }
}

fn search_item(book: LibraryBook) -> SearchItem {
    SearchItem {
        id: book.id,
        title: book.title,
        primary_format: book.format,
    }
}

fn format_items(state: &ServerState, id: i64, formats: Vec<LibraryFormat>) -> Vec<FormatItem> {
    formats
        .into_iter()
        .map(|format| FormatItem {
            content_href: href(
                state,
                &format!("/api/v1/books/{id}/content/{}", format.format),
            ),
            format: format.format,
            size_bytes: format.size_bytes,
        })
        .collect()
}

fn detail(state: &ServerState, book: LibraryBook, formats: Vec<LibraryFormat>) -> BookDetail {
    BookDetail {
        id: book.id,
        title: book.title,
        primary_format: book.format,
        formats: format_items(state, book.id, formats),
        self_href: href(state, &format!("/api/v1/books/{}", book.id)),
        content_href: href(state, &format!("/api/v1/books/{}/content", book.id)),
    }
}

fn href(state: &ServerState, path: &str) -> String {
    format!("{}{}", state.config.server.url_prefix, path)
}

fn internal_error(err: &caliberate_core::error::CoreError, context: &str) -> Response {
    warn!(component = "server", error = %err, "{context}");
    error(
        StatusCode::INTERNAL_SERVER_ERROR,
        "internal_error",
        "the server could not complete the request",
    )
}
