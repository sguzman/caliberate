//! OPDS feed endpoints.

use crate::ServerState;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use caliberate_db::database::Database;
use serde::Deserialize;
use std::fmt::Write as _;
use tokio_util::io::ReaderStream;
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
        Err(err) => {
            warn!(component = "server", error = %err, "failed to list books");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn opds_book_entry(State(state): State<ServerState>, Path(id): Path<i64>) -> Response {
    let db = match Database::open_with_fts(&state.config.db, &state.config.fts) {
        Ok(db) => db,
        Err(err) => {
            warn!(component = "server", error = %err, "failed to open database");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let Some(book) = (match db.get_book(id) {
        Ok(book) => book,
        Err(err) => {
            warn!(component = "server", error = %err, "failed to fetch book");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    }) else {
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
                r#type: content_type_for_format(&book.format),
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
    if !state.config.server.download_enabled {
        return StatusCode::FORBIDDEN.into_response();
    }

    let db = match Database::open_with_fts(&state.config.db, &state.config.fts) {
        Ok(db) => db,
        Err(err) => {
            warn!(component = "server", error = %err, "failed to open database");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let Some(book) = (match db.get_book(id) {
        Ok(book) => book,
        Err(err) => {
            warn!(component = "server", error = %err, "failed to fetch book");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    }) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let assets = match db.list_assets_for_book(id) {
        Ok(assets) => assets,
        Err(err) => {
            warn!(component = "server", error = %err, "failed to list assets");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let (path, storage_mode) = if let Some(asset) = assets
        .iter()
        .find(|asset| asset.storage_mode == "copy")
        .or_else(|| assets.first())
    {
        (asset.stored_path.clone(), Some(asset.storage_mode.clone()))
    } else {
        (book.path.clone(), None)
    };

    if !is_path_allowed(&state, &path, storage_mode.as_deref()) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let metadata = match tokio::fs::metadata(&path).await {
        Ok(metadata) => metadata,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    if metadata.len() > state.config.server.download_max_bytes {
        return StatusCode::PAYLOAD_TOO_LARGE.into_response();
    }

    let file = match tokio::fs::File::open(&path).await {
        Ok(file) => file,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);
    let mut response = body.into_response();
    let content_type = content_type_for_format(&book.format);
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    response.headers_mut().insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&metadata.len().to_string())
            .unwrap_or_else(|_| HeaderValue::from_static("0")),
    );
    response
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

fn content_type_for_format(format: &str) -> &'static str {
    match format {
        "epub" => "application/epub+zip",
        "pdf" => "application/pdf",
        "mobi" => "application/x-mobipocket-ebook",
        "azw" | "azw3" => "application/vnd.amazon.ebook",
        _ => "application/octet-stream",
    }
}

fn is_path_allowed(state: &ServerState, path: &str, storage_mode: Option<&str>) -> bool {
    if state.config.server.download_allow_external {
        return true;
    }
    if let Some(mode) = storage_mode {
        if mode == "reference" {
            return false;
        }
    }
    let library_dir = &state.config.paths.library_dir;
    let path = std::path::Path::new(path);
    path.starts_with(library_dir)
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\"', "&quot;")
        .replace('\'', "&apos;")
}
