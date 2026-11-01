use crate::Params;
use bytes::Bytes;
use http::request::Parts;
use http::{HeaderMap, Method, Uri};
use http_body_util::BodyExt;
use serde::de::DeserializeOwned;

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

/// An incoming HTTP request plus matched path params.
pub struct Request {
    parts: Parts,
    body: Option<Body>,
    pub(crate) params: Params,
}

impl Request {
    pub fn new(req: http::Request<Body>) -> Self {
        let (parts, body) = req.into_parts();
        Request { parts, body: Some(body), params: Params::default() }
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

    /// Read the whole body. Returns an empty buffer if already consumed.
    pub async fn bytes(&mut self) -> Result<Bytes, hyper::Error> {
        match self.body.take() {
            None => Ok(Bytes::new()),
            Some(Body::Full(b)) => Ok(b),
            Some(Body::Incoming(b)) => Ok(b.collect().await?.to_bytes()),
        }
    }

    /// Read and deserialize a JSON body.
    pub async fn json<T: DeserializeOwned>(&mut self) -> Result<T, crate::Response> {
        use crate::IntoResponse;
        let bytes = self
            .bytes()
            .await
            .map_err(|_| http::StatusCode::BAD_REQUEST.into_response())?;
        serde_json::from_slice(&bytes)
            .map_err(|e| (http::StatusCode::UNPROCESSABLE_ENTITY, e.to_string()).into_response())
    }
}
