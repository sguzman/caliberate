//! Human-facing library pages for browsers and Voice Dream's web content source.

use crate::{ServerState, content};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use caliberate_library::catalog::LibraryFormat;
use caliberate_library::query::{LibraryQuery, LibrarySortField};
use caliberate_library::summary::LibraryBookSummary;
use serde::Deserialize;
use std::collections::HashMap;
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
            // Preserve title, author, series and tag search semantics.
            // Enrich only the visible page, never the entire library.
            let found = catalog.search_books(term)?;
            let total = found.len();
            let visible = found
                .into_iter()
                .skip(offset)
                .take(PAGE_SIZE)
                .collect::<Vec<_>>();
            let ids = visible.iter().map(|book| book.id).collect::<Vec<_>>();
            let mut summaries: HashMap<i64, LibraryBookSummary> = if ids.is_empty() {
                HashMap::new()
            } else {
                catalog
                    .query_summary_page(&LibraryQuery::new().with_ids(ids))?
                    .books
                    .into_iter()
                    .map(|book| (book.id, book))
                    .collect()
            };
            let items = visible
                .iter()
                .filter_map(|book| summaries.remove(&book.id))
                .collect();
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
                items: result.books,
            })
        }
    }) {
        Ok(page) => page,
        Err(err) => {
            warn!(component = "server", error = %err, "failed to render library page");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let body = render_library_page(&page, &params, offset, &state.config.server.url_prefix);
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
        if let Ok(value) = HeaderValue::from_str(&format!("attachment; filename=\"{}\"", filename))
        {
            response
                .headers_mut()
                .insert(header::CONTENT_DISPOSITION, value);
        }
    }
    response
}

struct WebPage {
    items: Vec<LibraryBookSummary>,
    total: usize,
}

fn render_library_page(
    page: &WebPage,
    params: &LibraryPageQuery,
    offset: usize,
    prefix: &str,
) -> String {
    let base = prefix.trim_end_matches('/');
    let mut body = String::new();
    body.push_str("<!doctype html><html><head><meta charset=\"utf-8\">");
    body.push_str("<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">");
    body.push_str("<title>Caliberate Library</title><style>");
    body.push_str(":root{color-scheme:light dark;--bg:#fff;--fg:#18212c;--muted:#536173;--line:#d6dce2;--card:#f7f9fb;--accent:#175dbb}");
    body.push_str("@media(prefers-color-scheme:dark){:root{--bg:#141820;--fg:#ecf0f4;--muted:#abb7c6;--line:#424b58;--card:#202732;--accent:#8db9ff}}");
    body.push_str("html[data-theme=light]{color-scheme:light;--bg:#fff;--fg:#18212c;--muted:#536173;--line:#d6dce2;--card:#f7f9fb;--accent:#175dbb}");
    body.push_str("html[data-theme=dark]{color-scheme:dark;--bg:#141820;--fg:#ecf0f4;--muted:#abb7c6;--line:#424b58;--card:#202732;--accent:#8db9ff}");
    body.push_str("body{font:16px/1.45 -apple-system,BlinkMacSystemFont,sans-serif;max-width:800px;margin:auto;padding:16px;background:var(--bg);color:var(--fg)}");
    body.push_str("a{color:var(--accent)}.top{display:flex;align-items:center;justify-content:space-between;gap:12px;flex-wrap:wrap}h1{margin:0;font-size:25px}h2{margin:0;font-size:19px;overflow-wrap:anywhere}");
    body.push_str("form{display:flex;gap:8px;margin:16px 0}input,button,select{font:inherit;padding:9px;border:1px solid var(--line);border-radius:6px;color:var(--fg);background:var(--card)}input{min-width:0;flex:1}button{cursor:pointer}.muted{color:var(--muted)}");
    body.push_str(".book{display:grid;grid-template-columns:96px minmax(0,1fr);gap:15px;background:var(--card);border:1px solid var(--line);border-radius:10px;padding:13px;margin:13px 0}");
    body.push_str(".cover{width:96px;height:144px;border-radius:4px;overflow:hidden;background:var(--line);display:flex;justify-content:center;align-items:center;text-align:center}.cover img{width:100%;height:100%;object-fit:cover}.no-cover{font-size:12px;padding:8px;color:var(--muted)}");
    body.push_str(".facts{display:flex;flex-wrap:wrap;gap:5px 12px;margin:9px 0;color:var(--muted);font-size:14px}.facts span{overflow-wrap:anywhere}.language{font-weight:650;color:var(--fg)}");
    body.push_str(".downloads{display:flex;flex-wrap:wrap;gap:7px;margin-top:11px}.downloads a{display:inline-block;border:1px solid var(--line);padding:7px 9px;border-radius:6px;text-decoration:none;font-size:14px}");
    body.push_str(".nav{display:flex;gap:16px;margin:20px 0}.theme{font-size:13px}.theme select{font-size:13px;padding:5px}");
    body.push_str("@media(max-width:440px){body{padding:10px}.book{grid-template-columns:76px minmax(0,1fr);gap:10px;padding:10px}.cover{width:76px;height:114px}}");
    body.push_str("</style></head><body>");
    body.push_str("<header class=\"top\"><h1>Caliberate Library</h1><label class=\"theme\">Theme <select id=\"theme\" aria-label=\"Color theme\"><option value=\"system\">System</option><option value=\"light\">Light</option><option value=\"dark\">Dark</option></select></label></header>");
    let _ = writeln!(
        body,
        "<p class=\"muted\">{} books available.</p>",
        page.total
    );
    let _ = write!(
        body,
        "<form method=\"get\" action=\"{}\"><input name=\"q\" type=\"search\" placeholder=\"Search books\" aria-label=\"Search books\" value=\"",
        html_escape(&format!("{base}/library"))
    );
    if let Some(q) = params.q.as_deref() {
        body.push_str(&html_escape(q));
    }
    body.push_str("\"><button type=\"submit\">Search</button></form>");
    let _ = write!(
        body,
        "<p><a href=\"{}\">All books</a> · <a href=\"{}\">Recently added</a></p>",
        html_escape(&format!("{base}/library")),
        html_escape(&format!("{base}/library?sort=recent"))
    );

    if page.items.is_empty() {
        body.push_str("<p>No books found.</p>");
    }

    for book in &page.items {
        body.push_str("<article class=\"book\">");
        body.push_str("<div class=\"cover\">");
        if book.has_cover {
            let _ = write!(
                body,
                "<img loading=\"lazy\" alt=\"Cover of {}\" src=\"{}/api/v1/books/{}/cover\" onerror=\"this.hidden=true;this.nextElementSibling.hidden=false\"><span class=\"no-cover\" hidden>Cover unavailable</span>",
                html_escape(&book.title),
                html_escape(base),
                book.id
            );
        } else {
            body.push_str("<span class=\"no-cover\">No cover</span>");
        }
        body.push_str("</div><div class=\"details\">");
        let _ = write!(body, "<h2>{}</h2>", html_escape(&book.title));
        if !book.authors.is_empty() {
            let _ = write!(
                body,
                "<div class=\"muted\">by {}</div>",
                html_escape(&book.authors.join(", "))
            );
        }
        body.push_str("<div class=\"facts\">");
        if book.languages.is_empty() {
            body.push_str("<span class=\"language\">Language: not recorded</span>");
        } else {
            let labels = book
                .languages
                .iter()
                .map(|code| language_label(code))
                .collect::<Vec<_>>();
            let _ = write!(
                body,
                "<span class=\"language\">Language: {}</span>",
                html_escape(&labels.join(", "))
            );
        }
        if let Some(publisher) = &book.publisher {
            let _ = write!(body, "<span>Publisher: {}</span>", html_escape(publisher));
        }
        if let Some(pubdate) = &book.pubdate {
            let _ = write!(body, "<span>Published: {}</span>", html_escape(pubdate));
        }
        if let Some(series) = &book.series {
            let _ = write!(
                body,
                "<span>Series: {} #{}</span>",
                html_escape(&series.name),
                series.index
            );
        }
        if let Some(rating) = book.rating {
            let _ = write!(body, "<span>Rating: {rating}/10</span>");
        }
        if !book.tags.is_empty() {
            let _ = write!(
                body,
                "<span>Tags: {}</span>",
                html_escape(
                    &book
                        .tags
                        .iter()
                        .take(4)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            );
        }
        body.push_str("</div><div class=\"downloads\">");
        let formats = if book.formats.is_empty() {
            vec![LibraryFormat {
                format: book.format.clone(),
                size_bytes: None,
            }]
        } else {
            book.formats.clone()
        };
        for format in &formats {
            if format.format.trim().is_empty() {
                continue;
            }
            let filename = download_filename(&book.title, &format.format);
            let href = format!(
                "{base}/library/download/{}/{}/{}",
                book.id,
                urlencoding::encode(&format.format),
                urlencoding::encode(&filename)
            );
            let size = format
                .size_bytes
                .map(|bytes| format!(" · {}", human_size(bytes)))
                .unwrap_or_default();
            let _ = write!(
                body,
                "<a href=\"{}\">Download {}{}</a>",
                html_escape(&href),
                html_escape(&format.format.to_ascii_uppercase()),
                html_escape(&size)
            );
        }
        body.push_str("</div></div></article>");
    }

    let mut nav = String::new();
    if offset > 0 {
        let previous = offset.saturating_sub(PAGE_SIZE);
        let _ = write!(
            nav,
            "<a href=\"{}\">Previous</a>",
            html_escape(&page_url(params, previous, base))
        );
    }
    if offset.saturating_add(PAGE_SIZE) < page.total {
        let _ = write!(
            nav,
            "<a href=\"{}\">Next</a>",
            html_escape(&page_url(params, offset + PAGE_SIZE, base))
        );
    }
    if !nav.is_empty() {
        body.push_str("<nav class=\"nav\">");
        body.push_str(&nav);
        body.push_str("</nav>");
    }

    body.push_str("<script>(function(){var e=document.getElementById('theme');try{e.value=localStorage.getItem('caliberate-theme')||'system'}catch(_){}function apply(){if(e.value==='system'){document.documentElement.removeAttribute('data-theme')}else{document.documentElement.setAttribute('data-theme',e.value)}}e.addEventListener('change',function(){apply();try{localStorage.setItem('caliberate-theme',e.value)}catch(_){}});apply()})();</script>");
    body.push_str("</body></html>");
    body
}

fn language_label(code: &str) -> String {
    let code = code.trim();
    let name = match code.to_ascii_lowercase().as_str() {
        "es" | "spa" => "Spanish",
        "hu" | "hun" => "Hungarian",
        "en" | "eng" => "English",
        "fr" | "fra" | "fre" => "French",
        "de" | "deu" | "ger" => "German",
        "pt" | "por" => "Portuguese",
        "it" | "ita" => "Italian",
        "ca" | "cat" => "Catalan",
        "ru" | "rus" => "Russian",
        "ja" | "jpn" => "Japanese",
        "zh" | "zho" | "chi" => "Chinese",
        "nl" | "nld" | "dut" => "Dutch",
        "pl" | "pol" => "Polish",
        "la" | "lat" => "Latin",
        "ar" | "ara" => "Arabic",
        _ => return code.to_string(),
    };
    format!("{name} ({code})")
}

fn human_size(size: u64) -> String {
    if size >= 1_048_576 {
        format!("{:.1} MB", size as f64 / 1_048_576.0)
    } else if size >= 1_024 {
        format!("{:.1} KB", size as f64 / 1_024.0)
    } else {
        format!("{size} B")
    }
}

fn page_url(params: &LibraryPageQuery, offset: usize, base: &str) -> String {
    let mut parts = vec![format!("offset={offset}")];
    if let Some(q) = params.q.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        parts.push(format!("q={}", urlencoding::encode(q)));
    }
    if let Some(sort) = params.sort.as_deref() {
        parts.push(format!("sort={}", urlencoding::encode(sort)));
    }
    format!("{base}/library?{}", parts.join("&"))
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
    format!(
        "{stem}.{}",
        if extension.is_empty() {
            "bin"
        } else {
            &extension
        }
    )
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
    use super::{download_filename, human_size, language_label};

    #[test]
    fn language_codes_are_recognizable_without_guessing_missing_metadata() {
        assert_eq!(language_label("spa"), "Spanish (spa)");
        assert_eq!(language_label("hun"), "Hungarian (hun)");
        assert_eq!(language_label("eng"), "English (eng)");
        assert_eq!(language_label("xyz"), "xyz");
    }

    #[test]
    fn format_sizes_are_learner_readable() {
        assert_eq!(human_size(900), "900 B");
        assert_eq!(human_size(2048), "2.0 KB");
        assert_eq!(human_size(1_048_576), "1.0 MB");
    }

    #[test]
    fn download_names_have_real_extensions() {
        assert_eq!(
            download_filename("A Wizard of Earthsea", "EPUB"),
            "A Wizard of Earthsea.epub"
        );
        assert_eq!(download_filename("Bad / Name", "pdf"), "Bad _ Name.pdf");
    }
}
