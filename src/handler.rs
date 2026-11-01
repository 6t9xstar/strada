use crate::{IntoResponse, Request, Response};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub type BoxFuture = Pin<Box<dyn Future<Output = Response> + Send>>;

/// An async request handler. Implemented for any `Fn(Request) -> impl Future<Output = impl IntoResponse>`.
pub trait Handler: Send + Sync + 'static {
    fn call(&self, req: Request) -> BoxFuture;
}

impl<F, Fut, R> Handler for F
where
    F: Fn(Request) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = R> + Send + 'static,
    R: IntoResponse,
{
    fn call(&self, req: Request) -> BoxFuture {
        let fut = self(req);
        Box::pin(async move { fut.await.into_response() })
    }
}

pub(crate) type BoxHandler = Arc<dyn Handler>;
