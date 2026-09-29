//! Human-facing library pages for browsers and Voice Dream's web content source.

use crate::{ServerState, content};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use caliberate_library::query::{LibraryQuery, LibrarySortField};
use serde::Deserialize;
use std::fmt::Write as _;
use tracing::warn;

const PAGE_SIZE: usize = 50;

#[derive(Debug, Deserialize, Default)]
pub struct LibraryPageQuery {
    pub q: Option<String>,
    pub offset: Option<usize>,
    pub sort: Option<String>,
}

pub async fn library_page(
    State(state): State<ServerState>,
    Query(params): Query<LibraryPageQuery>,
) -> Response {
    let offset = params.offset.unwrap_or(0);
    let search = params
        .q
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);

    let page = match state.with_catalog(|catalog| {
        if let Some(term) = search.as_deref() {
            let books = catalog.search_books(term)?;
            let total = books.len();
            let items = books
                .into_iter()
                .skip(offset)
                .take(PAGE_SIZE)
                .map(|book| {
                    let formats = catalog.list_formats(book.id)?;
                    Ok(WebBook {
                        id: book.id,
                        title: book.title,
                        authors: Vec::new(),
                        formats: if formats.is_empty() {
                            vec![book.format]
                        } else {
                            formats.into_iter().map(|format| format.format).collect()
                        },
                    })
                })
                .collect::<caliberate_core::error::CoreResult<Vec<_>>>()?;
            Ok(WebPage { items, total })
        } else {
            let mut query = LibraryQuery::new()
                .with_limit(PAGE_SIZE)
                .with_offset(offset)
                .with_sort(match params.sort.as_deref() {
                    Some("recent") => LibrarySortField::DateAdded,
                    _ => LibrarySortField::Title,
                });
            if matches!(params.sort.as_deref(), Some("recent")) {
                query.descending = true;
            }
            let result = catalog.query_summary_page(&query)?;
            Ok(WebPage {
                total: result.total,
                items: result
                    .books
                    .into_iter()
                    .map(|book| WebBook {
                        id: book.id,
                        title: book.title,
                        authors: book.authors,
                        formats: if book.formats.is_empty() {
                            vec![book.format]
                        } else {
                            book.formats.into_iter().map(|format| format.format).collect()
                        },
                    })
                    .collect(),
            })
        }
    }) {
        Ok(page) => page,
        Err(err) => {
            warn!(component = "server", error = %err, "failed to render library page");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let body = render_library_page(&page, &params, offset);
    Html(body).into_response()
}

pub async fn named_download(
    State(state): State<ServerState>,
    Path((id, format, _filename)): Path<(i64, String, String)>,
) -> Response {
    let result = state.with_catalog(|catalog| {
        let book = catalog.get_book(id)?;
        let content = catalog.resolve_content_format(id, &format)?;
        Ok((book, content))
    });

    let (book, content_item) = match result {
        Ok((Some(book), Some(content_item))) => (book, content_item),
        Ok(_) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            warn!(component = "server", error = %err, "failed to resolve web download");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let mut response = content::stream_content(&state, content_item).await;
    if response.status().is_success() {
        let filename = download_filename(&book.title, &format);
        if let Ok(value) = HeaderValue::from_str(&format!(
            "attachment; filename=\"{}\"",
            filename
        )) {
            response.headers_mut().insert(header::CONTENT_DISPOSITION, value);
        }
    }
    response
}

struct WebPage {
    items: Vec<WebBook>,
    total: usize,
}

struct WebBook {
    id: i64,
    title: String,
    authors: Vec<String>,
    formats: Vec<String>,
}

fn render_library_page(page: &WebPage, params: &LibraryPageQuery, offset: usize) -> String {
    let mut body = String::new();
    body.push_str("<!doctype html><html><head><meta charset=\"utf-8\">");
    body.push_str("<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">");
    body.push_str("<title>Caliberate Library</title>");
    body.push_str("<style>");
    body.push_str("body{font-family:-apple-system,BlinkMacSystemFont,sans-serif;max-width:760px;margin:0 auto;padding:18px;line-height:1.4}");
    body.push_str("h1{margin:0 0 12px}form{display:flex;gap:8px;margin:12px 0}input{font-size:18px;padding:10px;flex:1}button,a.button{font-size:17px;padding:10px 12px}");
    body.push_str(".book{padding:14px 0;border-bottom:1px solid #ddd}.title{font-size:19px;font-weight:650}.meta{color:#555;margin:3px 0 8px}.downloads a{display:inline-block;margin:4px 8px 4px 0}.nav{display:flex;gap:12px;margin:18px 0}");
    body.push_str("</style></head><body>");
    body.push_str("<h1>Caliberate Library</h1>");
    let _ = writeln!(body, "<p>{} books available.</p>", page.total);
    body.push_str("<form method=\"get\" action=\"/library\">");
    body.push_str("<input name=\"q\" type=\"search\" placeholder=\"Search books\" value=\"");
    if let Some(q) = params.q.as_deref() {
        body.push_str(&html_escape(q));
    }
    body.push_str("\"><button type=\"submit\">Search</button></form>");
    body.push_str("<p><a href=\"/library\">All books</a> · <a href=\"/library?sort=recent\">Recently added</a></p>");

    if page.items.is_empty() {
        body.push_str("<p>No books found.</p>");
    }

    for book in &page.items {
        body.push_str("<div class=\"book\">");
        let _ = writeln!(body, "<div class=\"title\">{}</div>", html_escape(&book.title));
        if !book.authors.is_empty() {
            let _ = writeln!(
                body,
                "<div class=\"meta\">{}</div>",
                html_escape(&book.authors.join(", "))
            );
        }
        body.push_str("<div class=\"downloads\">");
        for format_name in &book.formats {
            if format_name.trim().is_empty() {
                continue;
            }
            let filename = download_filename(&book.title, format_name);
            let _ = write!(
                body,
                "<a href=\"/library/download/{}/{}/{}\">Download {}</a>",
                book.id,
                urlencoding::encode(format_name),
                urlencoding::encode(&filename),
                html_escape(&format_name.to_ascii_uppercase())
            );
        }
        body.push_str("</div></div>");
    }

    let mut nav = String::new();
    if offset > 0 {
        let previous = offset.saturating_sub(PAGE_SIZE);
        let _ = write!(
            nav,
            "<a class=\"button\" href=\"{}\">Previous</a>",
            page_url(params, previous)
        );
    }
    if offset.saturating_add(PAGE_SIZE) < page.total {
        let _ = write!(
            nav,
            "<a class=\"button\" href=\"{}\">Next</a>",
            page_url(params, offset + PAGE_SIZE)
        );
    }
    if !nav.is_empty() {
        body.push_str("<div class=\"nav\">");
        body.push_str(&nav);
        body.push_str("</div>");
    }

    body.push_str("</body></html>");
    body
}

fn page_url(params: &LibraryPageQuery, offset: usize) -> String {
    let mut parts = vec![format!("offset={offset}")];
    if let Some(q) = params.q.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        parts.push(format!("q={}", urlencoding::encode(q)));
    }
    if let Some(sort) = params.sort.as_deref() {
        parts.push(format!("sort={}", urlencoding::encode(sort)));
    }
    format!("/library?{}", parts.join("&"))
}

fn download_filename(title: &str, format_name: &str) -> String {
    let mut stem = title
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, ' ' | '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    stem = stem.trim().trim_matches('.').to_string();
    if stem.is_empty() {
        stem = "book".to_string();
    }
    if stem.len() > 120 {
        stem.truncate(120);
    }
    let extension = format_name
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    format!("{stem}.{}", if extension.is_empty() { "bin" } else { &extension })
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::download_filename;

    #[test]
    fn download_names_have_real_extensions() {
        assert_eq!(
            download_filename("A Wizard of Earthsea", "EPUB"),
            "A Wizard of Earthsea.epub"
        );
        assert_eq!(download_filename("Bad / Name", "pdf"), "Bad _ Name.pdf");
    }
}
