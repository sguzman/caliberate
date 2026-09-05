//! OPDS feed endpoints.

use crate::{ServerState, content};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use std::fmt::Write as _;
use tracing::warn;

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
}

pub async fn opds_root(State(state): State<ServerState>) -> Response {
    let base = opds_base(&state);
    let mut links = Vec::new();
    links.push(Link {
        href: format!("{base}/opds/books"),
        rel: "subsection",
        r#type: "application/atom+xml",
        title: Some("All books"),
    });
    links.push(Link {
        href: format!("{base}/opds/search?q={{searchTerms}}"),
        rel: "search",
        r#type: "application/atom+xml",
        title: Some("Search"),
    });
    respond_feed("Caliberate OPDS", "urn:caliberate:opds", &links, &[])
}

pub async fn opds_books(State(state): State<ServerState>) -> Response {
    let books = match state.with_catalog(|catalog| catalog.list_books()) {
        Ok(books) => books,
        Err(err) => {
            warn!(component = "server", error = %err, "failed to list books");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    {
        let entries = books
            .into_iter()
            .map(|book| FeedEntry {
                id: format!("urn:caliberate:book:{}", book.id),
                title: book.title,
                links: vec![Link {
                    href: format!("{}/opds/books/{}", opds_base(&state), book.id),
                    rel: "self",
                    r#type: "application/atom+xml",
                    title: None,
                }],
            })
            .collect::<Vec<_>>();
        respond_feed(
            "Caliberate Catalog",
            "urn:caliberate:opds:books",
            &[],
            &entries,
        )
    }
}

pub async fn opds_book_entry(State(state): State<ServerState>, Path(id): Path<i64>) -> Response {
    let book = match state.with_catalog(|catalog| catalog.get_book(id)) {
        Ok(book) => book,
        Err(err) => {
            warn!(component = "server", error = %err, "failed to fetch book");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let Some(book) = book else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let download_href = format!("{}/opds/books/{}/download", opds_base(&state), id);
    let entry = FeedEntry {
        id: format!("urn:caliberate:book:{}", book.id),
        title: book.title,
        links: vec![
            Link {
                href: format!("{}/opds/books/{}", opds_base(&state), book.id),
                rel: "self",
                r#type: "application/atom+xml",
                title: None,
            },
            Link {
                href: download_href,
                rel: "http://opds-spec.org/acquisition",
                r#type: content::content_type_for_format(&book.format),
                title: Some("Download"),
            },
        ],
    };

    respond_feed(
        "Caliberate Book",
        &format!("urn:caliberate:opds:book:{}", id),
        &[],
        std::slice::from_ref(&entry),
    )
}

pub async fn opds_book_download(State(state): State<ServerState>, Path(id): Path<i64>) -> Response {
    let content = match state.with_catalog(|catalog| catalog.resolve_content(id)) {
        Ok(Some(content)) => content,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            warn!(component = "server", error = %err, "failed to resolve book content");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    content::stream_content(&state, content).await
}

pub async fn opds_search(
    State(state): State<ServerState>,
    Query(query): Query<SearchQuery>,
) -> Response {
    let Some(term) = query.q else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let books = match state.with_catalog(|catalog| catalog.search_books(&term)) {
        Ok(books) => books,
        Err(err) => {
            warn!(component = "server", error = %err, "failed to search books");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    {
        let entries = books
            .into_iter()
            .map(|book| FeedEntry {
                id: format!("urn:caliberate:book:{}", book.id),
                title: book.title,
                links: vec![Link {
                    href: format!("{}/opds/books/{}", opds_base(&state), book.id),
                    rel: "self",
                    r#type: "application/atom+xml",
                    title: None,
                }],
            })
            .collect::<Vec<_>>();
        respond_feed(
            "Caliberate Search",
            "urn:caliberate:opds:search",
            &[],
            &entries,
        )
    }
}

struct Link<'a> {
    href: String,
    rel: &'a str,
    r#type: &'a str,
    title: Option<&'a str>,
}

struct FeedEntry {
    id: String,
    title: String,
    links: Vec<Link<'static>>,
}

fn respond_feed(title: &str, id: &str, links: &[Link<'_>], entries: &[FeedEntry]) -> Response {
    let mut body = String::new();
    body.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    body.push_str("<feed xmlns=\"http://www.w3.org/2005/Atom\">\n");
    let _ = writeln!(body, "  <title>{}</title>", xml_escape(title));
    let _ = writeln!(body, "  <id>{}</id>", xml_escape(id));
    for link in links {
        append_link(&mut body, link);
    }
    for entry in entries {
        body.push_str("  <entry>\n");
        let _ = writeln!(body, "    <title>{}</title>", xml_escape(&entry.title));
        let _ = writeln!(body, "    <id>{}</id>", xml_escape(&entry.id));
        for link in &entry.links {
            append_link(&mut body, link);
        }
        body.push_str("  </entry>\n");
    }
    body.push_str("</feed>\n");

    let mut response = body.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/atom+xml"),
    );
    response
}

fn append_link(buf: &mut String, link: &Link<'_>) {
    let _ = write!(
        buf,
        "  <link href=\"{}\" rel=\"{}\" type=\"{}\"",
        xml_escape(&link.href),
        link.rel,
        link.r#type
    );
    if let Some(title) = link.title {
        let _ = write!(buf, " title=\"{}\"", xml_escape(title));
    }
    buf.push_str(" />\n");
}

fn opds_base(state: &ServerState) -> String {
    if state.config.server.url_prefix.is_empty() {
        String::new()
    } else {
        state.config.server.url_prefix.clone()
    }
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use crate::content::content_type_for_format;
    use std::path::Path;

    #[test]
    fn attached_canonical_policy_rejects_outside_paths_and_symlink_targets() {
        let root = Path::new(r"C:\synthetic-calibre");
        assert!(Path::new(r"C:\synthetic-calibre\Author\book.epub").starts_with(root));
        assert!(!Path::new(r"C:\outside\book.epub").starts_with(root));
        assert_eq!(content_type_for_format("epub"), "application/epub+zip");
    }
}
