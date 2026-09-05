use axum::body::Body;
use axum::http::{Request, StatusCode};
use caliberate_core::config::ControlPlane;
use caliberate_db::database::Database;
use caliberate_server::{ServerState, http};
use http_body_util::BodyExt;
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
