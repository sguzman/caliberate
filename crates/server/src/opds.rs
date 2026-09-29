//! OPDS 1.2 catalog endpoints.

use crate::{ServerState, content};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use caliberate_library::query::{LibraryFacetKind, LibraryQuery, LibrarySortField};
use caliberate_library::summary::LibraryBookSummary;
use serde::Deserialize;
use std::fmt::Write as _;
use tracing::warn;

const DEFAULT_PAGE_SIZE: usize = 50;
const MAX_PAGE_SIZE: usize = 100;
const OPDS_ACQUISITION_TYPE: &str =
    "application/atom+xml;profile=opds-catalog;kind=acquisition";
const OPDS_NAVIGATION_TYPE: &str =
    "application/atom+xml;profile=opds-catalog;kind=navigation";
const OPDS_ENTRY_TYPE: &str = "application/atom+xml;type=entry;profile=opds-catalog";
const OPENSEARCH_TYPE: &str = "application/opensearchdescription+xml";

#[derive(Debug, Deserialize, Default)]
pub struct BrowseQuery {
    pub offset: Option<usize>,
    pub limit: Option<usize>,
    pub author: Option<String>,
    pub tag: Option<String>,
    pub series: Option<String>,
    pub sort: Option<String>,
    pub direction: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct FacetQuery {
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize, Default)]
pub struct SearchQuery {
    pub q: Option<String>,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}

pub async fn opds_root(State(state): State<ServerState>) -> Response {
    let base = opds_base(&state);
    let links = common_feed_links(&base, "/opds", FeedKind::Navigation);
    let entries = vec![
        navigation_entry(
            "urn:caliberate:nav:all-books",
            "All Books",
            format!("{base}/opds/books"),
            OPDS_ACQUISITION_TYPE,
        ),
        navigation_entry(
            "urn:caliberate:nav:recent",
            "Recently Added",
            format!("{base}/opds/books?sort=date_added&direction=desc"),
            OPDS_ACQUISITION_TYPE,
        ),
        navigation_entry(
            "urn:caliberate:nav:authors",
            "Authors",
            format!("{base}/opds/authors"),
            OPDS_NAVIGATION_TYPE,
        ),
        navigation_entry(
            "urn:caliberate:nav:tags",
            "Tags",
            format!("{base}/opds/tags"),
            OPDS_NAVIGATION_TYPE,
        ),
        navigation_entry(
            "urn:caliberate:nav:series",
            "Series",
            format!("{base}/opds/series"),
            OPDS_NAVIGATION_TYPE,
        ),
    ];
    respond_feed(
        "Caliberate OPDS",
        "urn:caliberate:opds",
        FeedKind::Navigation,
        &links,
        &entries,
    )
}

pub async fn opds_books(
    State(state): State<ServerState>,
    Query(params): Query<BrowseQuery>,
) -> Response {
    let (limit, offset) = page_limits(params.limit, params.offset);
    let mut query = LibraryQuery::new().with_limit(limit).with_offset(offset);
    query.author = params.author.clone();
    query.tag = params.tag.clone();
    query.series = params.series.clone();
    query.sort = match params.sort.as_deref() {
        Some("date_added") => LibrarySortField::DateAdded,
        Some("date_modified") => LibrarySortField::DateModified,
        Some("authors") => LibrarySortField::Authors,
        Some("series") => LibrarySortField::Series,
        _ => LibrarySortField::Title,
    };
    query.descending = matches!(params.direction.as_deref(), Some("desc"));

    let page = match state.with_catalog(|catalog| catalog.query_summary_page(&query)) {
        Ok(page) => page,
        Err(err) => {
            warn!(component = "server", error = %err, "failed to list OPDS books");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let base = opds_base(&state);
    let query_suffix = browse_filter_suffix(&params);
    let self_path = paged_path("/opds/books", &query_suffix, offset, limit);
    let mut links = common_feed_links(&base, &self_path, FeedKind::Acquisition);
    append_pagination_links(
        &mut links,
        &base,
        "/opds/books",
        &query_suffix,
        offset,
        limit,
        page.total,
        FeedKind::Acquisition,
    );

    let entries = page
        .books
        .iter()
        .map(|book| acquisition_entry(&base, book))
        .collect::<Vec<_>>();

    respond_feed(
        "Caliberate Books",
        &format!("urn:caliberate:opds:books:{offset}"),
        FeedKind::Acquisition,
        &links,
        &entries,
    )
}

pub async fn opds_facets(
    State(state): State<ServerState>,
    Path(kind): Path<String>,
    Query(params): Query<FacetQuery>,
) -> Response {
    let Some((facet_kind, title, query_key)) = facet_definition(&kind) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let values = match state.with_catalog(|catalog| catalog.list_facets(facet_kind)) {
        Ok(values) => values,
        Err(err) => {
            warn!(component = "server", error = %err, "failed to list OPDS facets");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let total = values.len();
    let (limit, offset) = page_limits(params.limit, params.offset);
    let base = opds_base(&state);
    let root_path = format!("/opds/{kind}");
    let self_path = paged_path(&root_path, "", offset, limit);
    let mut links = common_feed_links(&base, &self_path, FeedKind::Navigation);
    append_pagination_links(
        &mut links,
        &base,
        &root_path,
        "",
        offset,
        limit,
        total,
        FeedKind::Navigation,
    );

    let entries = values
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|value| {
            navigation_entry(
                &format!("urn:caliberate:nav:{kind}:{}", value.id),
                &format!("{} ({})", value.name, value.count),
                format!(
                    "{base}/opds/books?{}={}",
                    query_key,
                    urlencoding::encode(&value.name)
                ),
                OPDS_ACQUISITION_TYPE,
            )
        })
        .collect::<Vec<_>>();

    respond_feed(
        &format!("Caliberate {title}"),
        &format!("urn:caliberate:opds:{kind}:{offset}"),
        FeedKind::Navigation,
        &links,
        &entries,
    )
}

pub async fn opds_book_entry(State(state): State<ServerState>, Path(id): Path<i64>) -> Response {
    let result = state.with_catalog(|catalog| {
        let book = catalog.get_book(id)?;
        let formats = match book.as_ref() {
            Some(_) => catalog.list_formats(id)?,
            None => Vec::new(),
        };
        Ok((book, formats))
    });
    let (book, formats) = match result {
        Ok(result) => result,
        Err(err) => {
            warn!(component = "server", error = %err, "failed to fetch book");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let Some(book) = book else {
        return StatusCode::NOT_FOUND.into_response();
    };

    let base = opds_base(&state);
    let mut links = vec![Link {
        href: format!("{base}/opds/books/{id}"),
        rel: "self",
        r#type: OPDS_ENTRY_TYPE,
        title: None,
    }];
    if formats.is_empty() {
        links.push(Link {
            href: format!("{base}/opds/books/{id}/download"),
            rel: "http://opds-spec.org/acquisition/open-access",
            r#type: content::content_type_for_format(&book.format),
            title: Some(format!("Download {}", book.format.to_ascii_uppercase())),
        });
    } else {
        for format in formats {
            links.push(Link {
                href: format!("{base}/opds/books/{id}/download/{}", format.format),
                rel: "http://opds-spec.org/acquisition/open-access",
                r#type: content::content_type_for_format(&format.format),
                title: Some(format!("Download {}", format.format.to_ascii_uppercase())),
            });
        }
    }
    let entry = FeedEntry {
        id: format!("urn:caliberate:book:{}", book.id),
        title: book.title,
        authors: Vec::new(),
        updated: None,
        links,
    };
    respond_feed(
        "Caliberate Book",
        &format!("urn:caliberate:opds:book:{id}"),
        FeedKind::Acquisition,
        &common_feed_links(
            &base,
            &format!("/opds/books/{id}"),
            FeedKind::Acquisition,
        ),
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

pub async fn opds_book_format_download(
    State(state): State<ServerState>,
    Path((id, format)): Path<(i64, String)>,
) -> Response {
    let content = match state.with_catalog(|catalog| catalog.resolve_content_format(id, &format)) {
        Ok(Some(content)) => content,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(err) => {
            warn!(component = "server", error = %err, "failed to resolve format content");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    content::stream_content(&state, content).await
}

pub async fn opds_search(
    State(state): State<ServerState>,
    Query(query): Query<SearchQuery>,
) -> Response {
    let Some(term) = query.q.filter(|term| !term.trim().is_empty()) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let books = match state.with_catalog(|catalog| catalog.search_books(&term)) {
        Ok(books) => books,
        Err(err) => {
            warn!(component = "server", error = %err, "failed to search books");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let total = books.len();
    let (limit, offset) = page_limits(query.limit, query.offset);
    let base = opds_base(&state);
    let encoded = urlencoding::encode(&term);
    let filter_suffix = format!("q={encoded}");
    let self_path = paged_path("/opds/search", &filter_suffix, offset, limit);
    let mut links = common_feed_links(&base, &self_path, FeedKind::Acquisition);
    append_pagination_links(
        &mut links,
        &base,
        "/opds/search",
        &filter_suffix,
        offset,
        limit,
        total,
        FeedKind::Acquisition,
    );
    let entries = books
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|book| FeedEntry {
            id: format!("urn:caliberate:book:{}", book.id),
            title: book.title,
            authors: Vec::new(),
            updated: None,
            links: vec![
                Link {
                    href: format!("{base}/opds/books/{}", book.id),
                    rel: "alternate",
                    r#type: OPDS_ENTRY_TYPE,
                    title: Some("Details".to_string()),
                },
                Link {
                    href: format!("{base}/opds/books/{}/download", book.id),
                    rel: "http://opds-spec.org/acquisition/open-access",
                    r#type: content::content_type_for_format(&book.format),
                    title: Some(format!("Download {}", book.format.to_ascii_uppercase())),
                },
            ],
        })
        .collect::<Vec<_>>();
    respond_feed(
        &format!("Search: {term}"),
        &format!("urn:caliberate:opds:search:{encoded}:{offset}"),
        FeedKind::Acquisition,
        &links,
        &entries,
    )
}

pub async fn opds_search_description(State(state): State<ServerState>) -> Response {
    let base = opds_base(&state);
    let mut body = String::new();
    body.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    body.push_str(
        "<OpenSearchDescription xmlns=\"http://a9.com/-/spec/opensearch/1.1/\">\n",
    );
    body.push_str("  <ShortName>Caliberate</ShortName>\n");
    body.push_str("  <Description>Search the Caliberate ebook library</Description>\n");
    let _ = writeln!(
        body,
        "  <Url type=\"{}\" template=\"{}/opds/search?q={{searchTerms}}\" />",
        OPDS_ACQUISITION_TYPE,
        xml_escape(&base)
    );
    body.push_str("</OpenSearchDescription>\n");
    let mut response = body.into_response();
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(OPENSEARCH_TYPE));
    response
}

#[derive(Clone, Copy)]
enum FeedKind {
    Navigation,
    Acquisition,
}

impl FeedKind {
    fn media_type(self) -> &'static str {
        match self {
            Self::Navigation => OPDS_NAVIGATION_TYPE,
            Self::Acquisition => OPDS_ACQUISITION_TYPE,
        }
    }
}

struct Link<'a> {
    href: String,
    rel: &'a str,
    r#type: &'a str,
    title: Option<String>,
}

struct FeedEntry {
    id: String,
    title: String,
    authors: Vec<String>,
    updated: Option<String>,
    links: Vec<Link<'static>>,
}

fn navigation_entry(id: &str, title: &str, href: String, media_type: &'static str) -> FeedEntry {
    FeedEntry {
        id: id.to_string(),
        title: title.to_string(),
        authors: Vec::new(),
        updated: None,
        links: vec![Link {
            href,
            rel: "subsection",
            r#type: media_type,
            title: Some(title.to_string()),
        }],
    }
}

fn acquisition_entry(base: &str, book: &LibraryBookSummary) -> FeedEntry {
    let mut links = vec![Link {
        href: format!("{base}/opds/books/{}", book.id),
        rel: "alternate",
        r#type: OPDS_ENTRY_TYPE,
        title: Some("Details".to_string()),
    }];
    if book.formats.is_empty() {
        links.push(Link {
            href: format!("{base}/opds/books/{}/download", book.id),
            rel: "http://opds-spec.org/acquisition/open-access",
            r#type: content::content_type_for_format(&book.format),
            title: Some(format!("Download {}", book.format.to_ascii_uppercase())),
        });
    } else {
        for format in &book.formats {
            links.push(Link {
                href: format!(
                    "{base}/opds/books/{}/download/{}",
                    book.id, format.format
                ),
                rel: "http://opds-spec.org/acquisition/open-access",
                r#type: content::content_type_for_format(&format.format),
                title: Some(format!("Download {}", format.format.to_ascii_uppercase())),
            });
        }
    }
    if book.has_cover {
        links.push(Link {
            href: format!("{base}/api/v1/books/{}/cover", book.id),
            rel: "http://opds-spec.org/image/thumbnail",
            r#type: "image/jpeg",
            title: Some("Cover".to_string()),
        });
    }
    FeedEntry {
        id: format!("urn:caliberate:book:{}", book.id),
        title: book.title.clone(),
        authors: book.authors.clone(),
        updated: book.date_modified.clone().or_else(|| book.date_added.clone()),
        links,
    }
}

fn common_feed_links(base: &str, self_path: &str, kind: FeedKind) -> Vec<Link<'static>> {
    vec![
        Link {
            href: format!("{base}{self_path}"),
            rel: "self",
            r#type: kind.media_type(),
            title: None,
        },
        Link {
            href: format!("{base}/opds"),
            rel: "start",
            r#type: OPDS_NAVIGATION_TYPE,
            title: Some("Caliberate OPDS".to_string()),
        },
        Link {
            href: format!("{base}/opds/search.xml"),
            rel: "search",
            r#type: OPENSEARCH_TYPE,
            title: Some("Search".to_string()),
        },
    ]
}

fn append_pagination_links(
    links: &mut Vec<Link<'static>>,
    base: &str,
    root_path: &str,
    suffix: &str,
    offset: usize,
    limit: usize,
    total: usize,
    kind: FeedKind,
) {
    if offset > 0 {
        let previous = offset.saturating_sub(limit);
        links.push(Link {
            href: format!("{base}{}", paged_path(root_path, suffix, previous, limit)),
            rel: "previous",
            r#type: kind.media_type(),
            title: Some("Previous".to_string()),
        });
        links.push(Link {
            href: format!("{base}{}", paged_path(root_path, suffix, 0, limit)),
            rel: "first",
            r#type: kind.media_type(),
            title: Some("First".to_string()),
        });
    }
    if offset.saturating_add(limit) < total {
        links.push(Link {
            href: format!(
                "{base}{}",
                paged_path(root_path, suffix, offset + limit, limit)
            ),
            rel: "next",
            r#type: kind.media_type(),
            title: Some("Next".to_string()),
        });
        let last_offset = ((total - 1) / limit) * limit;
        links.push(Link {
            href: format!(
                "{base}{}",
                paged_path(root_path, suffix, last_offset, limit)
            ),
            rel: "last",
            r#type: kind.media_type(),
            title: Some("Last".to_string()),
        });
    }
}

fn browse_filter_suffix(params: &BrowseQuery) -> String {
    let mut parts = Vec::new();
    if let Some(value) = &params.author {
        parts.push(format!("author={}", urlencoding::encode(value)));
    }
    if let Some(value) = &params.tag {
        parts.push(format!("tag={}", urlencoding::encode(value)));
    }
    if let Some(value) = &params.series {
        parts.push(format!("series={}", urlencoding::encode(value)));
    }
    if let Some(value) = &params.sort {
        parts.push(format!("sort={}", urlencoding::encode(value)));
    }
    if let Some(value) = &params.direction {
        parts.push(format!("direction={}", urlencoding::encode(value)));
    }
    parts.join("&")
}

fn paged_path(root_path: &str, suffix: &str, offset: usize, limit: usize) -> String {
    if suffix.is_empty() {
        format!("{root_path}?offset={offset}&limit={limit}")
    } else {
        format!("{root_path}?{suffix}&offset={offset}&limit={limit}")
    }
}

fn page_limits(limit: Option<usize>, offset: Option<usize>) -> (usize, usize) {
    let limit = limit.unwrap_or(DEFAULT_PAGE_SIZE).clamp(1, MAX_PAGE_SIZE);
    (limit, offset.unwrap_or(0))
}

fn facet_definition(kind: &str) -> Option<(LibraryFacetKind, &'static str, &'static str)> {
    match kind {
        "authors" => Some((LibraryFacetKind::Authors, "Authors", "author")),
        "tags" => Some((LibraryFacetKind::Tags, "Tags", "tag")),
        "series" => Some((LibraryFacetKind::Series, "Series", "series")),
        _ => None,
    }
}

fn respond_feed(
    title: &str,
    id: &str,
    kind: FeedKind,
    links: &[Link<'_>],
    entries: &[FeedEntry],
) -> Response {
    let mut body = String::new();
    body.push_str("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    body.push_str(
        "<feed xmlns=\"http://www.w3.org/2005/Atom\" xmlns:dc=\"http://purl.org/dc/terms/\" xmlns:opds=\"http://opds-spec.org/2010/catalog\">\n",
    );
    let _ = writeln!(body, "  <title>{}</title>", xml_escape(title));
    let _ = writeln!(body, "  <id>{}</id>", xml_escape(id));
    body.push_str("  <updated>1970-01-01T00:00:00Z</updated>\n");
    for link in links {
        append_link(&mut body, link, 2);
    }
    for entry in entries {
        body.push_str("  <entry>\n");
        let _ = writeln!(body, "    <title>{}</title>", xml_escape(&entry.title));
        let _ = writeln!(body, "    <id>{}</id>", xml_escape(&entry.id));
        let _ = writeln!(
            body,
            "    <updated>{}</updated>",
            xml_escape(
                entry
                    .updated
                    .as_deref()
                    .unwrap_or("1970-01-01T00:00:00Z")
            )
        );
        for author in &entry.authors {
            let _ = writeln!(
                body,
                "    <author><name>{}</name></author>",
                xml_escape(author)
            );
        }
        for link in &entry.links {
            append_link(&mut body, link, 4);
        }
        body.push_str("  </entry>\n");
    }
    body.push_str("</feed>\n");

    let mut response = body.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(kind.media_type()),
    );
    response
}

fn append_link(buf: &mut String, link: &Link<'_>, indent: usize) {
    let padding = " ".repeat(indent);
    let _ = write!(
        buf,
        "{padding}<link href=\"{}\" rel=\"{}\" type=\"{}\"",
        xml_escape(&link.href),
        link.rel,
        link.r#type
    );
    if let Some(title) = &link.title {
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
    use super::{page_limits, paged_path};

    #[test]
    fn opds_pages_are_bounded() {
        assert_eq!(page_limits(None, None), (50, 0));
        assert_eq!(page_limits(Some(1000), Some(20)), (100, 20));
    }

    #[test]
    fn paged_paths_preserve_filters() {
        assert_eq!(
            paged_path("/opds/books", "author=Le%20Guin", 50, 50),
            "/opds/books?author=Le%20Guin&offset=50&limit=50"
        );
    }
}
