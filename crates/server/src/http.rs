//! HTTP server wiring.

use crate::opds;
use axum::{Router, routing::get};
use caliberate_core::config::ControlPlane;
use caliberate_core::error::{CoreError, CoreResult};
use std::net::SocketAddr;
use tracing::info;

pub async fn run(config: &ControlPlane) -> CoreResult<()> {
    let base = Router::new()
        .route("/health", get(health))
        .route("/opds", get(opds::opds_feed));

    let app = if config.server.url_prefix.is_empty() {
        base
    } else {
        Router::new().nest(&config.server.url_prefix, base)
    };

    let addr = format!("{}:{}", config.server.host, config.server.port)
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

async fn health() -> &'static str {
    "ok"
}
