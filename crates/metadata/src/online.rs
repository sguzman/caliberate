//! Online metadata and cover providers.

use caliberate_core::error::{CoreError, CoreResult};
use reqwest::blocking::Client;
use reqwest::header::USER_AGENT;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::io;
use std::time::Duration;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub timeout_ms: u64,
    pub user_agent: String,
    pub openlibrary_enabled: bool,
    pub openlibrary_base_url: String,
    pub googlebooks_enabled: bool,
    pub googlebooks_base_url: String,
    pub googlebooks_api_key: String,
    pub googlebooks_max_results: usize,
    pub cover_max_bytes: usize,
}

impl ProviderConfig {
    pub fn build_client(&self) -> CoreResult<Client> {
        Client::builder()
            .timeout(Duration::from_millis(self.timeout_ms))
            .build()
            .map_err(|err| {
                CoreError::Io(
                    "build metadata provider client".to_string(),
                    io::Error::other(err.to_string()),
                )
            })
    }
}

#[derive(Debug, Clone)]
pub struct MetadataQuery {
    pub title: String,
    pub authors: Vec<String>,
    pub isbn: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DownloadedMetadata {
    pub provider: String,
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub tags: Vec<String>,
    pub publisher: Option<String>,
    pub pubdate: Option<String>,
    pub language: Option<String>,
    pub identifiers: Vec<(String, String)>,
    pub description: Option<String>,
    pub cover_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CoverDownload {
    pub bytes: Vec<u8>,
    pub content_type: Option<String>,
}

pub fn fetch_metadata(
    config: &ProviderConfig,
    query: &MetadataQuery,
    source: &str,
) -> CoreResult<DownloadedMetadata> {
    let client = config.build_client()?;
    match source {
        "openlibrary" => fetch_openlibrary(&client, config, query),
        "googlebooks" => fetch_googlebooks(&client, config, query),
        "amazon" | "isbndb" => Err(CoreError::ConfigValidate(format!(
            "provider '{source}' is not configured for API fetch"
        ))),
        _ => Err(CoreError::ConfigValidate(format!(
            "unsupported metadata source: {source}"
        ))),
    }
}

pub fn fetch_cover(config: &ProviderConfig, cover_url: &str) -> CoreResult<CoverDownload> {
    let client = config.build_client()?;
    let response = client
        .get(cover_url)
        .header(USER_AGENT, config.user_agent.as_str())
        .send()
        .and_then(|res| res.error_for_status())
        .map_err(|err| {
            CoreError::Io(
                "fetch metadata cover".to_string(),
                io::Error::other(err.to_string()),
            )
        })?;

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string());

    let bytes = response.bytes().map_err(|err| {
        CoreError::Io(
            "read metadata cover body".to_string(),
            io::Error::other(err.to_string()),
        )
    })?;

    if bytes.len() > config.cover_max_bytes {
        return Err(CoreError::ConfigValidate(format!(
            "downloaded cover exceeds max bytes ({} > {})",
            bytes.len(),
            config.cover_max_bytes
        )));
    }

    info!(
        component = "metadata_online",
        bytes = bytes.len(),
        "downloaded cover bytes"
    );

    Ok(CoverDownload {
        bytes: bytes.to_vec(),
        content_type,
    })
}

fn fetch_openlibrary(
    client: &Client,
    config: &ProviderConfig,
    query: &MetadataQuery,
) -> CoreResult<DownloadedMetadata> {
    if !config.openlibrary_enabled {
        return Err(CoreError::ConfigValidate(
            "openlibrary provider disabled by configuration".to_string(),
        ));
    }

    let mut request = client
        .get(format!(
            "{}/search.json",
            config.openlibrary_base_url.trim_end_matches('/')
        ))
        .header(USER_AGENT, config.user_agent.as_str())
        .query(&[("limit", "1")]);

    if let Some(isbn) = &query.isbn {
        request = request.query(&[("isbn", isbn)]);
    } else if !query.title.trim().is_empty() {
        request = request.query(&[("title", query.title.trim())]);
    }

    let response = request
        .send()
        .and_then(|res| res.error_for_status())
        .map_err(|err| {
            CoreError::Io(
                "fetch openlibrary metadata".to_string(),
                io::Error::other(err.to_string()),
            )
        })?;

    let payload: OpenLibrarySearchResponse = response.json().map_err(|err| {
        CoreError::ConfigValidate(format!("parse openlibrary metadata response: {err}"))
    })?;

    let Some(doc) = payload.docs.into_iter().next() else {
        return Err(CoreError::ConfigValidate(
            "no metadata candidates found from openlibrary".to_string(),
        ));
    };

    let mut identifiers = Vec::new();
    if let Some(isbn) = doc.isbn.as_ref().and_then(|list| list.first()) {
        identifiers.push(("isbn".to_string(), isbn.clone()));
    }
    if let Some(key) = &doc.key {
        identifiers.push(("openlibrary".to_string(), key.clone()));
    }

    let cover_url = doc.cover_i.map(|cover_id| {
        format!("https://covers.openlibrary.org/b/id/{cover_id}-L.jpg?default=false")
    });

    info!(
        component = "metadata_online",
        source = "openlibrary",
        "metadata fetched"
    );

    Ok(DownloadedMetadata {
        provider: "openlibrary".to_string(),
        title: doc.title,
        authors: doc.author_name.unwrap_or_default(),
        tags: doc.subject.unwrap_or_default(),
        publisher: doc
            .publisher
            .and_then(|items| items.into_iter().next())
            .filter(|value| !value.trim().is_empty()),
        pubdate: doc
            .first_publish_year
            .map(|year| format!("{year:04}-01-01")),
        language: doc
            .language
            .and_then(|items| items.into_iter().next())
            .filter(|value| !value.trim().is_empty()),
        identifiers,
        description: None,
        cover_url,
    })
}

fn fetch_googlebooks(
    client: &Client,
    config: &ProviderConfig,
    query: &MetadataQuery,
) -> CoreResult<DownloadedMetadata> {
    if !config.googlebooks_enabled {
        return Err(CoreError::ConfigValidate(
            "googlebooks provider disabled by configuration".to_string(),
        ));
    }

    let mut q = String::new();
    if let Some(isbn) = &query.isbn {
        q.push_str("isbn:");
        q.push_str(isbn);
    } else {
        if !query.title.trim().is_empty() {
            q.push_str("intitle:");
            q.push_str(query.title.trim());
        }
        if let Some(author) = query.authors.first() {
            if !q.is_empty() {
                q.push(' ');
            }
            q.push_str("inauthor:");
            q.push_str(author.trim());
        }
    }

    if q.trim().is_empty() {
        return Err(CoreError::ConfigValidate(
            "googlebooks query cannot be empty".to_string(),
        ));
    }

    let max_results = config.googlebooks_max_results.to_string();
    let mut request = client
        .get(format!(
            "{}/volumes",
            config.googlebooks_base_url.trim_end_matches('/')
        ))
        .header(USER_AGENT, config.user_agent.as_str())
        .query(&[("q", q.as_str()), ("maxResults", max_results.as_str())]);

    if !config.googlebooks_api_key.trim().is_empty() {
        request = request.query(&[("key", config.googlebooks_api_key.as_str())]);
    }

    let response = request
        .send()
        .and_then(|res| res.error_for_status())
        .map_err(|err| {
            CoreError::Io(
                "fetch googlebooks metadata".to_string(),
                io::Error::other(err.to_string()),
            )
        })?;

    let payload: GoogleBooksResponse = response.json().map_err(|err| {
        CoreError::ConfigValidate(format!("parse googlebooks metadata response: {err}"))
    })?;

    let Some(item) = payload.items.into_iter().next() else {
        return Err(CoreError::ConfigValidate(
            "no metadata candidates found from googlebooks".to_string(),
        ));
    };

    let volume = item.volume_info;

    let mut identifiers = Vec::new();
    for id in volume.industry_identifiers.unwrap_or_default() {
        if !id.id_type.trim().is_empty() && !id.identifier.trim().is_empty() {
            identifiers.push((id.id_type.to_lowercase(), id.identifier));
        }
    }

    let tags = dedupe_strings(volume.categories.unwrap_or_default());

    let cover_url = volume
        .image_links
        .and_then(|links| links.thumbnail.or(links.small_thumbnail));

    info!(
        component = "metadata_online",
        source = "googlebooks",
        "metadata fetched"
    );

    Ok(DownloadedMetadata {
        provider: "googlebooks".to_string(),
        title: volume.title,
        authors: volume.authors.unwrap_or_default(),
        tags,
        publisher: volume.publisher,
        pubdate: volume.published_date,
        language: volume.language,
        identifiers,
        description: volume.description,
        cover_url,
    })
}

fn dedupe_strings(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut deduped = Vec::new();
    for value in values {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            continue;
        }
        let key = trimmed.to_lowercase();
        if seen.insert(key) {
            deduped.push(trimmed.to_string());
        }
    }
    deduped
}

#[derive(Debug, Deserialize)]
struct OpenLibrarySearchResponse {
    #[serde(default)]
    docs: Vec<OpenLibraryDoc>,
}

#[derive(Debug, Deserialize)]
struct OpenLibraryDoc {
    title: Option<String>,
    #[serde(default)]
    author_name: Option<Vec<String>>,
    #[serde(default)]
    subject: Option<Vec<String>>,
    #[serde(default)]
    publisher: Option<Vec<String>>,
    #[serde(default)]
    first_publish_year: Option<u32>,
    #[serde(default)]
    language: Option<Vec<String>>,
    #[serde(default)]
    isbn: Option<Vec<String>>,
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    cover_i: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct GoogleBooksResponse {
    #[serde(default)]
    items: Vec<GoogleBooksItem>,
}

#[derive(Debug, Deserialize)]
struct GoogleBooksItem {
    #[serde(default)]
    volume_info: GoogleBooksVolumeInfo,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleBooksVolumeInfo {
    title: Option<String>,
    authors: Option<Vec<String>>,
    categories: Option<Vec<String>>,
    publisher: Option<String>,
    published_date: Option<String>,
    language: Option<String>,
    description: Option<String>,
    industry_identifiers: Option<Vec<GoogleBooksIdentifier>>,
    image_links: Option<GoogleBooksImageLinks>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleBooksIdentifier {
    #[serde(default)]
    id_type: String,
    #[serde(default)]
    identifier: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleBooksImageLinks {
    thumbnail: Option<String>,
    small_thumbnail: Option<String>,
}

pub fn source_enabled(config: &ProviderConfig, source: &str) -> bool {
    match source {
        "openlibrary" => config.openlibrary_enabled,
        "googlebooks" => config.googlebooks_enabled,
        "amazon" | "isbndb" => false,
        _ => false,
    }
}

pub fn first_available_source(config: &ProviderConfig) -> Option<&'static str> {
    for source in ["openlibrary", "googlebooks"] {
        if source_enabled(config, source) {
            return Some(source);
        }
    }
    warn!(
        component = "metadata_online",
        "no online metadata provider enabled"
    );
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedupe_tags_ignores_case_and_empty() {
        let input = vec![
            "Sci-Fi".to_string(),
            "sci-fi".to_string(),
            "".to_string(),
            "Space".to_string(),
        ];
        let output = dedupe_strings(input);
        assert_eq!(output, vec!["Sci-Fi".to_string(), "Space".to_string()]);
    }

    #[test]
    fn source_selection_falls_back_to_google() {
        let config = ProviderConfig {
            timeout_ms: 7000,
            user_agent: "ua".to_string(),
            openlibrary_enabled: false,
            openlibrary_base_url: "https://openlibrary.org".to_string(),
            googlebooks_enabled: true,
            googlebooks_base_url: "https://www.googleapis.com/books/v1".to_string(),
            googlebooks_api_key: String::new(),
            googlebooks_max_results: 5,
            cover_max_bytes: 1024,
        };
        assert_eq!(first_available_source(&config), Some("googlebooks"));
    }
}
