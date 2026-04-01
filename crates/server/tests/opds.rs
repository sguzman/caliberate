use axum::body::Body;
use axum::http::{Request, StatusCode};
use caliberate_core::config::ControlPlane;
use caliberate_db::database::Database;
use caliberate_server::{ServerState, http};
use http_body_util::BodyExt;
use tempfile::tempdir;
use tower::ServiceExt;

#[tokio::test]
async fn opds_books_returns_feed() {
    let db_dir = tempdir().expect("db dir");
    let db_path = db_dir.path().join("server.db");
    let config_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../config/control-plane.toml");
    let mut config = ControlPlane::load_from_path(&config_path).expect("load config");
    config.db.sqlite_path = db_path;

    let db = Database::open_with_fts(&config.db, &config.fts).expect("open db");
    let _id = db
        .add_book(
            "Test Book",
            "epub",
            "/tmp/test.epub",
            "2024-01-01T00:00:00Z",
        )
        .expect("add book");

    let state = ServerState { config };
    let app = http::router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/opds/books")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let body = String::from_utf8_lossy(&body);
    assert!(body.contains("Test Book"));
}
