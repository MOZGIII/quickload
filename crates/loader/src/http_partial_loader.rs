//! Load the chunks via HTTP `Content-Range` requests.

use std::sync::Arc;

use quickload_chunker::Offset;

/// The loader that uses HTTP with a partial request.
#[derive(Debug)]
pub struct Loader<Connect> {
    /// The HTTP client.
    pub client: Arc<hyper::Client<Connect>>,
    /// The URI to request.
    pub uri: hyper::Uri,
    /// The range to request.
    pub byte_range: (Offset, Offset),
}

/// The handshake error.
#[derive(Debug, thiserror::Error)]
pub enum HandshakeError {
    /// The request has failed to build.
    #[error("request build error: {0}")]
    RequestBuild(hyper::http::Error),
    /// The request execution has failed.
    #[error("request execution error: {0}")]
    Request(hyper::Error),
    /// The server has returned bad status.
    #[error("HTTP response has unexpected status {0}")]
    BadStatus(hyper::StatusCode),
    /// The server response did not contain the `Content-Range`.
    #[error("no Content-Range header in the HTTP response")]
    NoContentRange,
}

impl<Connect> Loader<Connect>
where
    Connect: hyper::client::connect::Connect + Clone + Send + Sync + 'static,
{
    /// Execute the HTTP request, check the response status and headers and
    /// return the response body.
    pub async fn handshake(self) -> Result<hyper::Body, HandshakeError> {
        let Self {
            client,
            uri,
            byte_range,
        } = self;
        let (first_byte_offset, last_byte_offset) = byte_range;

        let range_header = format!("bytes={first_byte_offset}-{last_byte_offset}");

        let req = hyper::http::Request::builder()
            .uri(uri)
            .header(hyper::header::RANGE, range_header)
            .body(hyper::Body::empty())
            .map_err(HandshakeError::RequestBuild)?;

        let res = client.request(req).await.map_err(HandshakeError::Request)?;

        let status = res.status();
        if status != hyper::http::StatusCode::PARTIAL_CONTENT {
            return Err(HandshakeError::BadStatus(status));
        }

        let content_range = res
            .headers()
            .get(hyper::http::header::CONTENT_RANGE)
            .ok_or(HandshakeError::NoContentRange)?;
        tracing::info!(message = "loading data", content_range = %content_range.to_str().unwrap());

        let body = res.into_body();

        Ok(body)
    }
}
