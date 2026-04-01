//! OPDS feed endpoints.

use crate::ServerState;
use axum::extract::{Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use caliberate_db::database::Database;
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
    let db = match Database::open_with_fts(&state.config.db, &state.config.fts) {
        Ok(db) => db,
        Err(err) => {
            warn!(component = "server", error = %err, "failed to open database");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    match db.list_books() {
        Ok(books) => {
            let entries = books
                .into_iter()
                .map(|book| FeedEntry {
                    id: format!("urn:caliberate:book:{}", book.id),
                    title: book.title,
                    link: Link {
                        href: format!("{}/opds/books/{}", opds_base(&state), book.id),
                        rel: "self",
                        r#type: "application/atom+xml",
                        title: None,
                    },
                })
                .collect::<Vec<_>>();
            respond_feed(
                "Caliberate Catalog",
                "urn:caliberate:opds:books",
                &[],
                &entries,
            )
        }
        Err(err) => {
            warn!(component = "server", error = %err, "failed to list books");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn opds_search(
    State(state): State<ServerState>,
    Query(query): Query<SearchQuery>,
) -> Response {
    let Some(term) = query.q else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let db = match Database::open_with_fts(&state.config.db, &state.config.fts) {
        Ok(db) => db,
        Err(err) => {
            warn!(component = "server", error = %err, "failed to open database");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    match db.search_books(&term) {
        Ok(books) => {
            let entries = books
                .into_iter()
                .map(|book| FeedEntry {
                    id: format!("urn:caliberate:book:{}", book.id),
                    title: book.title,
                    link: Link {
                        href: format!("{}/opds/books/{}", opds_base(&state), book.id),
                        rel: "self",
                        r#type: "application/atom+xml",
                        title: None,
                    },
                })
                .collect::<Vec<_>>();
            respond_feed(
                "Caliberate Search",
                "urn:caliberate:opds:search",
                &[],
                &entries,
            )
        }
        Err(err) => {
            warn!(component = "server", error = %err, "failed to search books");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
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
    link: Link<'static>,
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
        append_link(&mut body, &entry.link);
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
