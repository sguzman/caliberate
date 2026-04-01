//! Content server and OPDS endpoints.

pub mod auth;
pub mod http;
pub mod opds;

use caliberate_core::config::ControlPlane;
use caliberate_core::error::CoreResult;

pub async fn run(config: &ControlPlane) -> CoreResult<()> {
    http::run(config).await
}
