use crate::{IntoResponse, Request, Response};
use http::StatusCode;
use std::future::Future;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

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
        // A panic while *creating* the future is caught too.
        match catch_unwind(AssertUnwindSafe(|| self(req))) {
            Ok(fut) => Box::pin(CatchPanic(Box::pin(
                async move { fut.await.into_response() },
            ))),
            Err(_) => Box::pin(async { panic_response() }),
        }
    }
}

pub(crate) type BoxHandler = Arc<dyn Handler>;

fn panic_response() -> Response {
    StatusCode::INTERNAL_SERVER_ERROR.into_response()
}

/// Turns a panic inside a handler future into a `500` instead of dropping the connection.
/// (Has no effect if the binary is built with `panic = "abort"`.)
struct CatchPanic(BoxFuture);

impl Future for CatchPanic {
    type Output = Response;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Response> {
        match catch_unwind(AssertUnwindSafe(|| self.0.as_mut().poll(cx))) {
            Ok(p) => p,
            Err(_) => Poll::Ready(panic_response()),
        }
    }
}
