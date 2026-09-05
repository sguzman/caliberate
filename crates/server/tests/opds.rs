use axum::body::Body;
use axum::http::{Request, StatusCode};
use caliberate_core::config::ControlPlane;
use caliberate_db::database::Database;
use caliberate_server::{ServerState, http};
use http_body_util::BodyExt;
use rusqlite::Connection;
use std::fs;
use tempfile::tempdir;
use tower::ServiceExt;

fn attached_fixture() -> tempfile::TempDir {
    let dir = tempdir().expect("attached library dir");
    let db = Connection::open(dir.path().join("metadata.db")).expect("metadata db");
    db.execute_batch(
        "CREATE TABLE books(id INTEGER PRIMARY KEY,title TEXT,timestamp TEXT,pubdate TEXT,series_index REAL,author_sort TEXT,path TEXT,uuid TEXT,has_cover INTEGER,last_modified TEXT);
         CREATE TABLE data(id INTEGER PRIMARY KEY,book INTEGER,format TEXT,uncompressed_size INTEGER,name TEXT,UNIQUE(book,format));
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
         INSERT INTO books VALUES(1,'Attached Book','2026-01-01','2025-01-01',1.0,'Attached Author','Attached Author/Attached Book (1)','attached-1',0,NULL);
         INSERT INTO data VALUES(10,1,'EPUB',12,'Attached Book - Attached Author');
         INSERT INTO authors VALUES(1,'Attached Author');
         INSERT INTO books_authors_link VALUES(1,1,1);"
    ).expect("create attached fixture");
    let book_dir = dir.path().join("Attached Author/Attached Book (1)");
    fs::create_dir_all(&book_dir).expect("book directory");
    fs::write(
        book_dir.join("Attached Book - Attached Author.epub"),
        b"attached bytes",
    )
    .expect("book content");
    dir
}

async fn response_text(response: axum::response::Response) -> String {
    String::from_utf8_lossy(
        &response
            .into_body()
            .collect()
            .await
            .expect("response body")
            .to_bytes(),
    )
    .into_owned()
}

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

    let state = ServerState::new(config);
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

    let state = ServerState::new(config);
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

    let state = ServerState::new(config);
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

#[tokio::test]
async fn attached_opds_uses_only_attached_source_and_downloads_native_content() {
    let attached = attached_fixture();
    let config_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../config/control-plane.toml");
    let mut config = ControlPlane::load_from_path(&config_path).expect("load config");
    config.server.download_enabled = true;
    config.server.download_allow_external = false;
    config.db.sqlite_path = attached.path().join("must-not-open.db");
    let backend = caliberate_library::calibre::CalibreLibraryBackend::open(attached.path())
        .expect("open synthetic attached library");
    let metadata_before = fs::read(attached.path().join("metadata.db")).expect("read metadata");
    let state = ServerState::with_attached_calibre(config, backend);
    let app = http::router(state);

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/opds/books")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("list request");
    assert_eq!(list.status(), StatusCode::OK);
    let list_body = response_text(list).await;
    assert!(list_body.contains("Attached Book"));
    assert!(!list_body.contains("configured"));

    let search = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/opds/search?q=Attached")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("search request");
    assert_eq!(search.status(), StatusCode::OK);
    assert!(response_text(search).await.contains("Attached Book"));

    let entry = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/opds/books/1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("entry request");
    assert_eq!(entry.status(), StatusCode::OK);
    assert!(response_text(entry).await.contains("Attached Book"));

    let download = app
        .oneshot(
            Request::builder()
                .uri("/opds/books/1/download")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("download request");
    assert_eq!(download.status(), StatusCode::OK);
    assert_eq!(
        &download.into_body().collect().await.unwrap().to_bytes()[..],
        b"attached bytes"
    );
    let metadata_after = fs::read(attached.path().join("metadata.db")).expect("read metadata");
    assert_eq!(metadata_after, metadata_before);
}

#[test]
fn attached_invalid_root_is_rejected_before_server_construction() {
    let dir = tempdir().expect("temporary directory");
    let error = caliberate_library::calibre::CalibreLibraryBackend::open(dir.path())
        .expect_err("missing metadata must be rejected");
    assert!(error.to_string().contains("metadata.db"));
}
