//! HTTP server wiring.

use crate::{ServerState, auth, opds};
use axum::{Router, routing::get};
use caliberate_core::error::{CoreError, CoreResult};
use std::net::SocketAddr;
use tracing::info;

pub async fn run(state: ServerState) -> CoreResult<()> {
    let app = router(state.clone());
    let addr = format!("{}:{}", state.config.server.host, state.config.server.port)
        .parse::<SocketAddr>()
        .map_err(|err| CoreError::ConfigValidate(err.to_string()))?;

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|err| CoreError::Io("bind server".to_string(), err))?;

    info!(component = "server", address = %addr, "server listening");

    axum::serve(listener, app).await.map_err(|err| {
        CoreError::Io(
            "serve http".to_string(),
            std::io::Error::new(std::io::ErrorKind::Other, err),
        )
    })
}

pub fn router(state: ServerState) -> Router {
    let base = Router::new()
        .route("/health", get(health))
        .route("/opds", get(opds::opds_root))
        .route("/opds/books", get(opds::opds_books))
        .route("/opds/search", get(opds::opds_search))
        .with_state(state.clone())
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::auth_middleware,
        ));

    if state.config.server.url_prefix.is_empty() {
        base
    } else {
        Router::new().nest(&state.config.server.url_prefix, base)
    }
}

async fn health() -> &'static str {
    "ok"
}
