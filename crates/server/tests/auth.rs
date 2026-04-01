use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use caliberate_core::config::ControlPlane;
use caliberate_server::{ServerState, http};
use tower::ServiceExt;

fn build_state(enable_auth: bool, api_keys: Vec<String>) -> ServerState {
    let config_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../config/control-plane.toml");
    let mut config = ControlPlane::load_from_path(&config_path).expect("load config");
    config.server.enable_auth = enable_auth;
    config.server.api_keys = api_keys;

    ServerState { config }
}

#[tokio::test]
async fn health_is_public_when_auth_disabled() {
    let state = build_state(false, Vec::new());
    let app = http::router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn auth_rejects_missing_key() {
    let state = build_state(true, vec!["secret".to_string()]);
    let app = http::router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn auth_accepts_bearer_token() {
    let state = build_state(true, vec!["secret".to_string()]);
    let app = http::router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .header(header::AUTHORIZATION, "Bearer secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request");

    assert_eq!(response.status(), StatusCode::OK);
}
