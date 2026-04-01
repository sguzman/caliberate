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

#[tokio::test]
async fn opds_download_returns_file() {
    let library_dir = tempdir().expect("library dir");
    let db_dir = tempdir().expect("db dir");
    let db_path = db_dir.path().join("server.db");
    let book_path = library_dir.path().join("book.epub");
    std::fs::write(&book_path, b"book data").expect("write book");

    let config_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../config/control-plane.toml");
    let mut config = ControlPlane::load_from_path(&config_path).expect("load config");
    config.db.sqlite_path = db_path;
    config.paths.library_dir = library_dir.path().to_path_buf();

    let db = Database::open_with_fts(&config.db, &config.fts).expect("open db");
    let book_id = db
        .add_book(
            "Test Book",
            "epub",
            book_path.to_str().unwrap(),
            "2024-01-01T00:00:00Z",
        )
        .expect("add book");

    let state = ServerState { config };
    let app = http::router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/opds/books/{book_id}/download"))
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
    assert_eq!(&body[..], b"book data");
}

#[tokio::test]
async fn opds_download_blocks_external_reference() {
    let library_dir = tempdir().expect("library dir");
    let db_dir = tempdir().expect("db dir");
    let db_path = db_dir.path().join("server.db");
    let external_dir = tempdir().expect("external dir");
    let book_path = external_dir.path().join("book.epub");
    std::fs::write(&book_path, b"book data").expect("write book");

    let config_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../config/control-plane.toml");
    let mut config = ControlPlane::load_from_path(&config_path).expect("load config");
    config.db.sqlite_path = db_path;
    config.paths.library_dir = library_dir.path().to_path_buf();
    config.server.download_allow_external = false;

    let db = Database::open_with_fts(&config.db, &config.fts).expect("open db");
    let book_id = db
        .add_book(
            "Test Book",
            "epub",
            book_path.to_str().unwrap(),
            "2024-01-01T00:00:00Z",
        )
        .expect("add book");

    let state = ServerState { config };
    let app = http::router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/opds/books/{book_id}/download"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
