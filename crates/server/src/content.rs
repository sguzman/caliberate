//! Shared authorization and streaming policy for library content.

use crate::ServerState;
use axum::body::Body;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use caliberate_library::catalog::LibraryContent;
use std::path::{Path, PathBuf};
use tokio_util::io::ReaderStream;

pub async fn stream_content(state: &ServerState, content: LibraryContent) -> Response {
    if !state.config.server.download_enabled {
        return StatusCode::FORBIDDEN.into_response();
    }
    let path = match authorized_content_path(state, &content) {
        Ok(path) => path,
        Err(status) => return status.into_response(),
    };
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
    let mut response = Body::from_stream(ReaderStream::new(file)).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(content_type_for_format(&content.format)),
    );
    response.headers_mut().insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&metadata.len().to_string())
            .unwrap_or_else(|_| HeaderValue::from_static("0")),
    );
    response
}

pub fn content_type_for_format(format: &str) -> &'static str {
    match format {
        "epub" => "application/epub+zip",
        "pdf" => "application/pdf",
        "mobi" => "application/x-mobipocket-ebook",
        "azw" | "azw3" => "application/vnd.amazon.ebook",
        _ => "application/octet-stream",
    }
}

fn authorized_content_path(
    state: &ServerState,
    content: &LibraryContent,
) -> Result<PathBuf, StatusCode> {
    if let Some(root) = state.attached_calibre_root() {
        let path = Path::new(&content.path);
        if !path.starts_with(root) {
            return Err(StatusCode::FORBIDDEN);
        }
        let canonical = std::fs::canonicalize(path).map_err(|_| StatusCode::NOT_FOUND)?;
        if !canonical.starts_with(root) {
            return Err(StatusCode::FORBIDDEN);
        }
        return Ok(canonical);
    }
    if state.config.server.download_allow_external {
        return Ok(PathBuf::from(&content.path));
    }
    if content.storage_mode.as_deref() == Some("reference") {
        return Err(StatusCode::FORBIDDEN);
    }
    let path = Path::new(&content.path);
    if !path.starts_with(&state.config.paths.library_dir) {
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::content_type_for_format;

    #[test]
    fn maps_content_types_from_normalized_formats() {
        assert_eq!(content_type_for_format("epub"), "application/epub+zip");
        assert_eq!(content_type_for_format("pdf"), "application/pdf");
    }
}
