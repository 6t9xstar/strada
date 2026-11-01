//! Strada: a small, fast HTTP backend framework on Tokio + Hyper 1.x.
//!
//! ```no_run
//! use strada::{Json, Request, Router, Server};
//!
//! #[tokio::main]
//! async fn main() -> std::io::Result<()> {
//!     let app = Router::new()
//!         .get("/", |_req: Request| async { "Hello, World!" })
//!         .get("/users/:id", |req: Request| async move {
//!             format!("user {}", req.param("id").unwrap_or_default())
//!         })
//!         .get("/json", |_req: Request| async {
//!             Json(serde_json::json!({ "message": "Hello, World!" }))
//!         });
//!     Server::bind("127.0.0.1:8080").serve(app).await
//! }
//! ```

mod handler;
mod request;
mod response;
mod router;
mod server;

pub use handler::Handler;
pub use request::{Body, Request};
pub use response::{IntoResponse, Json, Response};
pub use router::{InsertError, Params, Tree};
pub use server::{serve_listener, Server};

pub use bytes::Bytes;
pub use http::{header, Method, StatusCode};

use handler::BoxHandler;
use std::sync::Arc;

#[derive(Default)]
struct MethodMap {
    handlers: Vec<(Method, BoxHandler)>,
}

/// Maps method + path to handlers.
pub struct Router {
    tree: Tree<MethodMap>,
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

impl Router {
    pub fn new() -> Self {
        Router { tree: Tree::new() }
    }

    /// Register a handler. Panics on an invalid or duplicate route, as these are
    /// programming errors caught at startup.
    pub fn route<H: Handler>(mut self, method: Method, path: &str, handler: H) -> Self {
        let map = self
            .tree
            .entry_or_default(path)
            .unwrap_or_else(|e| panic!("{e}"));
        if map.handlers.iter().any(|(m, _)| *m == method) {
            panic!("{}", InsertError::Duplicate(format!("{method} {path}")));
        }
        map.handlers.push((method, Arc::new(handler)));
        self
    }

    pub fn get<H: Handler>(self, path: &str, h: H) -> Self {
        self.route(Method::GET, path, h)
    }
    pub fn post<H: Handler>(self, path: &str, h: H) -> Self {
        self.route(Method::POST, path, h)
    }
    pub fn put<H: Handler>(self, path: &str, h: H) -> Self {
        self.route(Method::PUT, path, h)
    }
    pub fn patch<H: Handler>(self, path: &str, h: H) -> Self {
        self.route(Method::PATCH, path, h)
    }
    pub fn delete<H: Handler>(self, path: &str, h: H) -> Self {
        self.route(Method::DELETE, path, h)
    }

    /// Dispatch a request in-process (no network). Used by the server, tests and benches.
    pub async fn handle(&self, mut req: Request) -> Response {
        let Some((map, params)) = self.tree.at(req.uri().path()) else {
            return StatusCode::NOT_FOUND.into_response();
        };
        let method = req.method().clone();
        let found = map
            .handlers
            .iter()
            .find(|(m, _)| *m == method)
            // HEAD falls back to GET; hyper strips the body.
            .or_else(|| {
                (method == Method::HEAD)
                    .then(|| map.handlers.iter().find(|(m, _)| *m == Method::GET))
                    .flatten()
            });
        match found {
            Some((_, h)) => {
                req.params = params;
                h.call(req).await
            }
            None => {
                let allow = map
                    .handlers
                    .iter()
                    .map(|(m, _)| m.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                let mut res = StatusCode::METHOD_NOT_ALLOWED.into_response();
                if let Ok(v) = header::HeaderValue::from_str(&allow) {
                    res.headers_mut().insert(header::ALLOW, v);
                }
                res
            }
        }
    }
}
