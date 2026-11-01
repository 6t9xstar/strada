use crate::{IntoResponse, Params, Response};
use bytes::Bytes;
use http::request::Parts;
use http::{HeaderMap, Method, StatusCode, Uri};
use http_body_util::{BodyExt, Limited};
use serde::de::DeserializeOwned;

/// Default maximum request body size: 2 MiB. Change it with [`crate::Server::body_limit`].
pub const DEFAULT_BODY_LIMIT: usize = 2 * 1024 * 1024;

/// Request body: either a streaming hyper body or bytes already in memory.
pub enum Body {
    Incoming(hyper::body::Incoming),
    Full(Bytes),
}

impl From<Bytes> for Body {
    fn from(b: Bytes) -> Self {
        Body::Full(b)
    }
}

impl From<&'static str> for Body {
    fn from(s: &'static str) -> Self {
        Body::Full(Bytes::from_static(s.as_bytes()))
    }
}

/// Why reading a request body failed.
#[derive(Debug)]
pub enum BodyError {
    /// The body exceeded the configured limit. Maps to `413 Payload Too Large`.
    TooLarge { limit: usize },
    /// The connection failed or the body was malformed. Maps to `400 Bad Request`.
    Read(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for BodyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BodyError::TooLarge { limit } => write!(f, "request body exceeds {limit} bytes"),
            BodyError::Read(e) => write!(f, "failed to read request body: {e}"),
        }
    }
}

impl std::error::Error for BodyError {}

impl IntoResponse for BodyError {
    fn into_response(self) -> Response {
        match self {
            BodyError::TooLarge { .. } => StatusCode::PAYLOAD_TOO_LARGE.into_response(),
            BodyError::Read(_) => StatusCode::BAD_REQUEST.into_response(),
        }
    }
}

/// An incoming HTTP request plus matched path params.
pub struct Request {
    parts: Parts,
    body: Option<Body>,
    body_limit: usize,
    pub(crate) params: Params,
}

impl Request {
    pub fn new(req: http::Request<Body>) -> Self {
        let (parts, body) = req.into_parts();
        Request {
            parts,
            body: Some(body),
            body_limit: DEFAULT_BODY_LIMIT,
            params: Params::default(),
        }
    }

    /// Override the body size limit for this request.
    pub fn with_body_limit(mut self, limit: usize) -> Self {
        self.body_limit = limit;
        self
    }

    pub fn method(&self) -> &Method {
        &self.parts.method
    }
    pub fn uri(&self) -> &Uri {
        &self.parts.uri
    }
    pub fn headers(&self) -> &HeaderMap {
        &self.parts.headers
    }
    pub fn params(&self) -> &Params {
        &self.params
    }
    /// A named path param, e.g. `id` for `/users/:id`.
    pub fn param(&self, name: &str) -> Option<&str> {
        self.params.get(name)
    }

    /// Read the whole body, up to the body limit. Returns an empty buffer if already consumed.
    pub async fn bytes(&mut self) -> Result<Bytes, BodyError> {
        let limit = self.body_limit;
        match self.body.take() {
            None => Ok(Bytes::new()),
            Some(Body::Full(b)) if b.len() > limit => Err(BodyError::TooLarge { limit }),
            Some(Body::Full(b)) => Ok(b),
            Some(Body::Incoming(b)) => {
                // Reject early when the client declares an oversized body.
                let declared = self
                    .parts
                    .headers
                    .get(http::header::CONTENT_LENGTH)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok());
                if declared.is_some_and(|n| n > limit as u64) {
                    return Err(BodyError::TooLarge { limit });
                }
                match Limited::new(b, limit).collect().await {
                    Ok(c) => Ok(c.to_bytes()),
                    Err(e) if e.is::<http_body_util::LengthLimitError>() => {
                        Err(BodyError::TooLarge { limit })
                    }
                    Err(e) => Err(BodyError::Read(e)),
                }
            }
        }
    }

    /// Read and deserialize a JSON body. Errors are ready-made responses:
    /// 413 (too large), 400 (read failure) or 422 (invalid JSON).
    pub async fn json<T: DeserializeOwned>(&mut self) -> Result<T, Response> {
        let bytes = self.bytes().await.map_err(IntoResponse::into_response)?;
        serde_json::from_slice(&bytes)
            .map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()).into_response())
    }
}
