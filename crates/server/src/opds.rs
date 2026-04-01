//! OPDS feed endpoints.

use axum::http::{header, HeaderValue};
use axum::response::{IntoResponse, Response};

pub async fn opds_feed() -> Response {
    let body = r#"<?xml version=\"1.0\" encoding=\"utf-8\"?>
<feed xmlns=\"http://www.w3.org/2005/Atom\">
  <title>Caliberate OPDS</title>
  <id>urn:caliberate:opds</id>
</feed>
"#;

    let mut response = body.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/atom+xml"),
    );
    response
}
