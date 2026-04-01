//! Content server and OPDS endpoints.

pub mod auth;
pub mod http;
pub mod opds;

use caliberate_core::config::ControlPlane;
use caliberate_core::error::CoreResult;

#[derive(Clone)]
pub struct ServerState {
    pub config: ControlPlane,
}

pub async fn run(config: &ControlPlane) -> CoreResult<()> {
    let state = ServerState {
        config: config.clone(),
    };
    http::run(state).await
}
