use axum::body::Body;
use axum::http::{Request, StatusCode};
use caliberate_core::config::ControlPlane;
use caliberate_db::database::Database;
use caliberate_library::calibre::CalibreLibraryBackend;
use caliberate_server::{ServerState, http};
use http_body_util::BodyExt;
use rusqlite::Connection;
use serde_json::Value;
use std::fs;
use tempfile::TempDir;
use tower::ServiceExt;

fn state() -> (TempDir, ServerState) {
    let dir = tempfile::tempdir().unwrap();
    let config_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../config/control-plane.toml");
    let mut config = ControlPlane::load_from_path(config_path).unwrap();
    config.db.sqlite_path = dir.path().join("library.db");
    config.paths.library_dir = dir.path().join("library");
    config.server.download_enabled = true;
    config.server.enable_auth = false;
    fs::create_dir_all(&config.paths.library_dir).unwrap();
    let mut db = Database::open_with_fts(&config.db, &config.fts).unwrap();
    db.add_book(
        "Alpha",
        "epub",
        &config
            .paths
            .library_dir
            .join("alpha.epub")
            .to_string_lossy(),
        "2026-01-01",
    )
    .unwrap();
    db.add_book(
        "Beta",
        "pdf",
        &config.paths.library_dir.join("beta.pdf").to_string_lossy(),
        "2026-01-02",
    )
    .unwrap();
    db.add_book_authors(1, &["Author One".to_string()]).unwrap();
    db.add_book_tags(1, &["fiction".to_string()]).unwrap();
    fs::write(config.paths.library_dir.join("alpha.epub"), b"epub bytes").unwrap();
    fs::write(config.paths.library_dir.join("beta.pdf"), b"pdf bytes").unwrap();
    (dir, ServerState::new(config))
}

fn attached_state() -> (TempDir, ServerState) {
    let dir = tempfile::tempdir().unwrap();
    let db = Connection::open(dir.path().join("metadata.db")).unwrap();
    db.execute_batch(
        "CREATE TABLE books(id INTEGER PRIMARY KEY,title TEXT,timestamp TEXT,pubdate TEXT,series_index REAL,author_sort TEXT,path TEXT,uuid TEXT,has_cover INTEGER,last_modified TEXT);
         CREATE TABLE data(id INTEGER PRIMARY KEY,book INTEGER,format TEXT,uncompressed_size INTEGER,name TEXT);
         CREATE TABLE authors(id INTEGER PRIMARY KEY,name TEXT);
         CREATE TABLE books_authors_link(id INTEGER PRIMARY KEY,book INTEGER,author INTEGER);
         CREATE TABLE tags(id INTEGER PRIMARY KEY,name TEXT);
         CREATE TABLE books_tags_link(id INTEGER PRIMARY KEY,book INTEGER,tag INTEGER);
         CREATE TABLE series(id INTEGER PRIMARY KEY,name TEXT);
         CREATE TABLE books_series_link(id INTEGER PRIMARY KEY,book INTEGER,series INTEGER);
         CREATE TABLE publishers(id INTEGER PRIMARY KEY,name TEXT);
         CREATE TABLE books_publishers_link(id INTEGER PRIMARY KEY,book INTEGER,publisher INTEGER);
         CREATE TABLE ratings(id INTEGER PRIMARY KEY,rating INTEGER);
         CREATE TABLE books_ratings_link(id INTEGER PRIMARY KEY,book INTEGER,rating INTEGER);
         CREATE TABLE languages(id INTEGER PRIMARY KEY,lang_code TEXT);
         CREATE TABLE books_languages_link(id INTEGER PRIMARY KEY,book INTEGER,lang_code INTEGER,item_order INTEGER);
         CREATE TABLE identifiers(id INTEGER PRIMARY KEY,book INTEGER,type TEXT,val TEXT);
         INSERT INTO books VALUES(1,'Attached Two Formats','2026-01-01','2025-01-01',1.0,'Attached Author','Attached Author/Attached Two Formats (1)','attached-1',0,NULL);
         INSERT INTO books VALUES(2,'Attached Metadata Only','2026-01-02',NULL,1.0,'','','attached-2',0,NULL);
         INSERT INTO data VALUES(10,1,'PDF',10,'Attached Two Formats - Attached Author');
         INSERT INTO data VALUES(11,1,'EPUB',11,'Attached Two Formats - Attached Author');
         INSERT INTO data VALUES(12,1,'MOBI',12,'Attached Two Formats - Attached Author');
         INSERT INTO authors VALUES(1,'Attached Author');
         INSERT INTO books_authors_link VALUES(1,1,1);
         INSERT INTO tags VALUES(1,'attached');
         INSERT INTO books_tags_link VALUES(1,1,1);",
    )
    .unwrap();
    let book_dir = dir.path().join("Attached Author/Attached Two Formats (1)");
    fs::create_dir_all(&book_dir).unwrap();
    fs::write(
        book_dir.join("Attached Two Formats - Attached Author.pdf"),
        b"pdf bytes",
    )
    .unwrap();
    fs::write(
        book_dir.join("Attached Two Formats - Attached Author.epub"),
        b"epub bytes",
    )
    .unwrap();
    fs::write(
        book_dir.join("Attached Two Formats - Attached Author.mobi"),
        b"mobi bytes",
    )
    .unwrap();
    let backend = CalibreLibraryBackend::open(dir.path()).unwrap();
    let config_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../config/control-plane.toml");
    let mut config = ControlPlane::load_from_path(config_path).unwrap();
    config.db.sqlite_path = dir.path().join("must-not-open.db");
    config.server.download_enabled = true;
    config.server.download_allow_external = false;
    config.server.enable_auth = false;
    (dir, ServerState::with_attached_calibre(config, backend))
}

async fn json_response(
    app: axum::Router,
    method: &str,
    uri: &str,
    body: Body,
) -> (StatusCode, Value) {
    let response = app
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .body(body)
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}

async fn raw_response(app: axum::Router, uri: &str) -> (StatusCode, String, Vec<u8>) {
    let response = app
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let body = response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    (status, content_type, body)
}

#[tokio::test]
async fn books_query_detail_formats_and_facets_are_json_without_paths() {
    let (_dir, state) = state();
    let app = http::router(state);
    let (status, body) = json_response(app.clone(), "GET", "/api/v1/books", Body::empty()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["limit"], 100);
    assert_eq!(body["total"], 2);
    assert!(body.to_string().find("library\\").is_none());

    let query = r#"{"sort":"title","direction":"desc","limit":1,"offset":0,"metadata_filters":[{"field":"tags","mode":"include","value":"fiction"},{"field":"authors","mode":"include","value":"author"}]}"#;
    let (status, body) = json_response(
        app.clone(),
        "POST",
        "/api/v1/books/query",
        Body::from(query),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"].as_array().unwrap().len(), 1);

    let (status, body) = json_response(app.clone(), "GET", "/api/v1/books/1", Body::empty()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["primary_format"], "epub");
    assert_eq!(body["formats"][0]["format"], "epub");
    assert!(
        body["formats"][0]["content_href"]
            .as_str()
            .unwrap()
            .ends_with("/content/epub")
    );

    let (status, body) =
        json_response(app.clone(), "GET", "/api/v1/facets/authors", Body::empty()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["kind"], "authors");
    let (status, _) = json_response(app, "GET", "/api/v1/facets/nope", Body::empty()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn api_rejects_invalid_paging_and_missing_books_with_json_errors() {
    let (_dir, state) = state();
    let app = http::router(state);
    for uri in [
        "/api/v1/books?limit=0",
        "/api/v1/books?limit=501",
        "/api/v1/books?sort=nope",
        "/api/v1/books?direction=nope",
    ] {
        let (status, body) = json_response(app.clone(), "GET", uri, Body::empty()).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"]["code"], "invalid_request");
    }
    let (status, content_type, _) = raw_response(app.clone(), "/api/v1/books?limit=0").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(content_type.starts_with("application/json"));
    let (status, body) =
        json_response(app.clone(), "GET", "/api/v1/books/999", Body::empty()).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");
    let (status, body) = json_response(app, "GET", "/api/v1/search?q=", Body::empty()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_request");
}

#[tokio::test]
async fn content_streams_internal_managed_files_and_honors_prefix() {
    let (_dir, mut state) = state();
    state.config.server.url_prefix = "/proxy".into();
    let app = http::router(state);
    let response = app
        .oneshot(
            Request::get("/proxy/api/v1/books/1/content")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "application/epub+zip");
    assert_eq!(
        response.into_body().collect().await.unwrap().to_bytes(),
        "epub bytes"
    );
}

#[tokio::test]
async fn search_browse_facets_prefix_and_json_content_types_are_covered() {
    let (_dir, mut state) = state();
    state.config.server.url_prefix = "/proxy".into();
    let app = http::router(state);
    let (status, body) = json_response(
        app.clone(),
        "GET",
        "/proxy/api/v1/search?q=a&limit=1&offset=1",
        Body::empty(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 2);
    assert_eq!(body["items"].as_array().unwrap().len(), 1);
    let (status, body) = json_response(
        app.clone(),
        "GET",
        "/proxy/api/v1/books?sort=title&limit=1&offset=1",
        Body::empty(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["items"][0]["title"], "Beta");
    let (status, body) = json_response(
        app.clone(),
        "GET",
        "/proxy/api/v1/facets/authors",
        Body::empty(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["kind"], "authors");
    let (status, body) = json_response(
        app.clone(),
        "GET",
        "/proxy/api/v1/facets/tags",
        Body::empty(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["kind"], "tags");
    let (status, body) =
        json_response(app.clone(), "GET", "/proxy/api/v1/books/1", Body::empty()).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body["formats"][0]["content_href"]
            .as_str()
            .unwrap()
            .starts_with("/proxy/api/v1/")
    );
    let response = app
        .oneshot(
            Request::get("/proxy/api/v1/books")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        response.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("application/json")
    );
}

#[tokio::test]
async fn configured_external_reference_is_forbidden_and_auth_protects_api() {
    let (_dir, mut auth_state) = state();
    auth_state.config.server.download_allow_external = false;
    auth_state.config.server.enable_auth = true;
    auth_state.config.server.api_keys = vec!["secret".into()];
    let app = http::router(auth_state.clone());
    let (status, _, _) = raw_response(app.clone(), "/api/v1/books").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let response = app
        .oneshot(
            Request::get("/api/v1/books")
                .header("authorization", "Bearer secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let (_dir, configured_state) = state();
    let db =
        Database::open_with_fts(&configured_state.config.db, &configured_state.config.fts).unwrap();
    db.add_asset(
        1,
        "reference",
        "C:/outside/reference.epub",
        None,
        10,
        10,
        None,
        false,
        "2026-01-01",
    )
    .unwrap();
    let app = http::router(configured_state);
    let (status, _, _) = raw_response(app, "/api/v1/books/1/content").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn attached_json_api_uses_source_formats_and_preserves_metadata_bytes() {
    let (dir, state) = attached_state();
    let before = fs::read(dir.path().join("metadata.db")).unwrap();
    let app = http::router(state);
    let (status, body) = json_response(app.clone(), "GET", "/api/v1/books", Body::empty()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["items"][0]["title"], "Attached Two Formats");
    assert_eq!(body["items"][0]["primary_format"], "pdf");
    assert_eq!(body["items"][0]["format_count"], 3);
    assert_eq!(body["items"][0]["formats"][0]["format"], "pdf");
    assert_eq!(body["items"][0]["formats"][1]["format"], "epub");
    assert_eq!(body["items"][0]["formats"][2]["format"], "mobi");
    assert_eq!(
        body["items"][0]["format_count"],
        body["items"][0]["formats"].as_array().unwrap().len()
    );
    assert!(body["items"][0]["formats"][0].get("content_href").is_none());
    assert_eq!(body["items"][1]["format_count"], 0);
    assert_eq!(body["items"][1]["formats"].as_array().unwrap().len(), 0);
    assert!(!body.to_string().contains("metadata.db"));
    assert!(!body.to_string().contains("Attached Author/Attached"));

    let query = r#"{"title":"Attached","sort":"title","direction":"asc","metadata_filters":[{"field":"tags","mode":"include","value":"attached"}]}"#;
    let (status, body) = json_response(
        app.clone(),
        "POST",
        "/api/v1/books/query",
        Body::from(query),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 1);
    assert_eq!(body["items"][0]["primary_format"], "pdf");
    assert_eq!(body["items"][0]["format_count"], 3);
    assert_eq!(
        body["items"][0]["format_count"],
        body["items"][0]["formats"].as_array().unwrap().len()
    );
    let (status, body) = json_response(app.clone(), "GET", "/api/v1/books/1", Body::empty()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["primary_format"], "pdf");
    assert_eq!(body["formats"][0]["format"], "pdf");
    assert_eq!(body["formats"][1]["format"], "epub");
    assert_eq!(body["formats"][2]["format"], "mobi");
    assert!(!body.to_string().contains("storage_mode"));
    let (status, body) =
        json_response(app.clone(), "GET", "/api/v1/books/1/formats", Body::empty()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["formats"][0]["format"], "pdf");
    assert_eq!(body["formats"][1]["format"], "epub");
    assert_eq!(body["formats"][2]["format"], "mobi");

    let (status, content_type, bytes) =
        raw_response(app.clone(), "/api/v1/books/1/content/EPUB").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(content_type, "application/epub+zip");
    assert_eq!(bytes, b"epub bytes");
    let (status, _, bytes) = raw_response(app.clone(), "/api/v1/books/1/content/pdf").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bytes, b"pdf bytes");
    let (status, _, bytes) = raw_response(app.clone(), "/api/v1/books/1/content").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bytes, b"pdf bytes");
    let (status, _, bytes) = raw_response(app.clone(), "/api/v1/books/1/content/mobi").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bytes, b"mobi bytes");
    let (status, _, _) = raw_response(app, "/api/v1/books/1/content/azw3").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(fs::read(dir.path().join("metadata.db")).unwrap(), before);
    assert!(!dir.path().join("must-not-open.db").exists());
}

fn compressed_state(download_max_bytes: u64) -> (TempDir, ServerState, Vec<u8>, Vec<u8>) {
    let (dir, mut state) = state();
    let logical = vec![b'x'; 4096];
    let stored = zstd::stream::encode_all(logical.as_slice(), 3).unwrap();
    let compressed_path = state.config.paths.library_dir.join("alpha.epub.zst");
    fs::write(&compressed_path, &stored).unwrap();
    let db = Database::open_with_fts(&state.config.db, &state.config.fts).unwrap();
    db.add_asset(
        1,
        "reference",
        "C:/outside/reference.epub",
        None,
        logical.len() as u64,
        logical.len() as u64,
        None,
        false,
        "2026-01-01",
    )
    .unwrap();
    db.add_asset(
        1,
        "copy",
        &compressed_path.to_string_lossy(),
        None,
        logical.len() as u64,
        stored.len() as u64,
        None,
        true,
        "2026-01-02",
    )
    .unwrap();
    state.config.server.download_max_bytes = download_max_bytes;
    (dir, state, logical, stored)
}

#[tokio::test]
async fn compressed_managed_content_is_streamed_as_logical_bytes() {
    let (_dir, state, logical, stored) = compressed_state(8192);
    for uri in [
        "/api/v1/books/1/content",
        "/api/v1/books/1/content/epub",
        "/opds/books/1/download",
    ] {
        let response = http::router(state.clone())
            .oneshot(Request::get(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "application/epub+zip");
        assert_eq!(
            response.headers()["content-length"],
            logical.len().to_string()
        );
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(bytes.as_ref(), logical.as_slice());
        assert_ne!(bytes.as_ref(), stored.as_slice());
    }
}

#[tokio::test]
async fn compressed_content_uses_logical_download_limit() {
    let (_dir, state, logical, stored) = compressed_state(1024);
    assert!(stored.len() < 1024);
    assert!(logical.len() > 1024);
    let (status, _, _) =
        raw_response(http::router(state.clone()), "/api/v1/books/1/content/epub").await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);

    let mut allowed = state;
    allowed.config.server.download_max_bytes = logical.len() as u64;
    let (status, _, bytes) =
        raw_response(http::router(allowed), "/api/v1/books/1/content/epub").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bytes, logical);
}

#[tokio::test]
async fn corrupt_preferred_zstd_content_terminates_with_body_error() {
    let (_dir, state, _logical, _stored) = compressed_state(8192);
    let compressed_path = state.config.paths.library_dir.join("alpha.epub.zst");
    fs::write(&compressed_path, b"not zstd").unwrap();
    let response = http::router(state)
        .oneshot(
            Request::get("/api/v1/books/1/content/epub")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.into_body().collect().await.is_err());
}
