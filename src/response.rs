use bytes::Bytes;
use http::header::{HeaderValue, CONTENT_TYPE};
use http::StatusCode;
use http_body_util::Full;
use serde::Serialize;

pub type Response = http::Response<Full<Bytes>>;

const TEXT: HeaderValue = HeaderValue::from_static("text/plain; charset=utf-8");
const JSON: HeaderValue = HeaderValue::from_static("application/json");

fn with_type(body: Bytes, ct: HeaderValue) -> Response {
    let mut res = http::Response::new(Full::new(body));
    res.headers_mut().insert(CONTENT_TYPE, ct);
    res
}

/// Anything a handler can return.
pub trait IntoResponse {
    fn into_response(self) -> Response;
}

impl IntoResponse for Response {
    fn into_response(self) -> Response {
        self
    }
}

impl IntoResponse for &'static str {
    fn into_response(self) -> Response {
        with_type(Bytes::from_static(self.as_bytes()), TEXT)
    }
}

impl IntoResponse for String {
    fn into_response(self) -> Response {
        with_type(Bytes::from(self), TEXT)
    }
}

impl IntoResponse for Bytes {
    fn into_response(self) -> Response {
        let mut res = http::Response::new(Full::new(self));
        res.headers_mut().insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );
        res
    }
}

impl IntoResponse for StatusCode {
    fn into_response(self) -> Response {
        let mut res = http::Response::new(Full::new(Bytes::new()));
        *res.status_mut() = self;
        res
    }
}

impl<T: IntoResponse> IntoResponse for (StatusCode, T) {
    fn into_response(self) -> Response {
        let mut res = self.1.into_response();
        *res.status_mut() = self.0;
        res
    }
}

impl<T: IntoResponse, E: IntoResponse> IntoResponse for Result<T, E> {
    fn into_response(self) -> Response {
        match self {
            Ok(v) => v.into_response(),
            Err(e) => e.into_response(),
        }
    }
}

/// Serialize `T` as an `application/json` response.
pub struct Json<T>(pub T);

impl<T: Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        match serde_json::to_vec(&self.0) {
            Ok(v) => with_type(Bytes::from(v), JSON),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
        }
    }
}
