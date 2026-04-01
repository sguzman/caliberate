//! Authentication and authorization.

use crate::ServerState;
use axum::body::Body;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::Next;
use axum::response::Response;
use tracing::warn;

pub async fn auth_middleware(
    State(state): State<ServerState>,
    mut req: axum::http::Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    if !state.config.server.enable_auth {
        return Ok(next.run(req).await);
    }

    if state.config.server.api_keys.is_empty() {
        warn!(component = "server", "auth enabled but api_keys is empty");
        return Err(StatusCode::UNAUTHORIZED);
    }

    let headers = req.headers_mut();
    if authorize_request(headers, &state.config.server.api_keys) {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

fn authorize_request(headers: &HeaderMap, api_keys: &[String]) -> bool {
    if let Some(value) = headers.get(header::AUTHORIZATION) {
        if let Ok(value) = value.to_str() {
            if let Some(token) = value.strip_prefix("Bearer ") {
                return api_keys.iter().any(|key| key == token);
            }
        }
    }
    if let Some(value) = headers.get("x-api-key") {
        if let Ok(value) = value.to_str() {
            return api_keys.iter().any(|key| key == value);
        }
    }
    false
}
