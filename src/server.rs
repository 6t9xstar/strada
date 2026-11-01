use crate::request::DEFAULT_BODY_LIMIT;
use crate::{Body, Request, Router};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::{TokioIo, TokioTimer};
use hyper_util::server::graceful::GracefulShutdown;
use std::convert::Infallible;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, ToSocketAddrs};

/// Connection-level limits. Defaults are safe for internet-facing use.
#[derive(Debug, Clone)]
pub struct Limits {
    /// Max request body size for `Request::bytes`/`json` (default 2 MiB).
    pub body_limit: usize,
    /// Time a client has to send the full request head (default 10 s). Stops slowloris.
    pub header_read_timeout: Duration,
    /// How long shutdown waits for in-flight connections before dropping them (default 30 s).
    pub shutdown_grace: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            body_limit: DEFAULT_BODY_LIMIT,
            header_read_timeout: Duration::from_secs(10),
            shutdown_grace: Duration::from_secs(30),
        }
    }
}

/// HTTP/1.1 server. Spawns one Tokio task per connection.
pub struct Server<A> {
    addr: A,
    limits: Limits,
}

impl<A: ToSocketAddrs> Server<A> {
    pub fn bind(addr: A) -> Self {
        Server {
            addr,
            limits: Limits::default(),
        }
    }

    pub fn body_limit(mut self, bytes: usize) -> Self {
        self.limits.body_limit = bytes;
        self
    }

    pub fn header_read_timeout(mut self, d: Duration) -> Self {
        self.limits.header_read_timeout = d;
        self
    }

    pub fn shutdown_grace(mut self, d: Duration) -> Self {
        self.limits.shutdown_grace = d;
        self
    }

    /// Serve until the process is killed.
    pub async fn serve(self, router: Router) -> std::io::Result<()> {
        self.serve_with_shutdown(router, std::future::pending())
            .await
    }

    /// Serve until `signal` resolves, then stop accepting and drain in-flight
    /// connections (up to `shutdown_grace`). Typical signal: `tokio::signal::ctrl_c()`.
    pub async fn serve_with_shutdown(
        self,
        router: Router,
        signal: impl Future<Output = ()>,
    ) -> std::io::Result<()> {
        let listener = TcpListener::bind(self.addr).await?;
        serve_listener_with(listener, router, signal, self.limits).await
    }
}

/// Serve on an existing listener with default [`Limits`] until `shutdown` resolves.
pub async fn serve_listener(
    listener: TcpListener,
    router: Router,
    shutdown: impl Future<Output = ()>,
) -> std::io::Result<()> {
    serve_listener_with(listener, router, shutdown, Limits::default()).await
}

/// Serve on an existing listener with explicit [`Limits`].
pub async fn serve_listener_with(
    listener: TcpListener,
    router: Router,
    shutdown: impl Future<Output = ()>,
    limits: Limits,
) -> std::io::Result<()> {
    let router = Arc::new(router);
    let graceful = GracefulShutdown::new();
    let body_limit = limits.body_limit;
    let mut builder = http1::Builder::new();
    builder
        .keep_alive(true)
        .timer(TokioTimer::new())
        .header_read_timeout(limits.header_read_timeout);
    tokio::pin!(shutdown);
    loop {
        let stream = tokio::select! {
            res = listener.accept() => match res {
                Ok((s, _peer)) => s,
                Err(_) => {
                    // Usually EMFILE/ENFILE: back off instead of spinning on the error.
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    continue;
                }
            },
            _ = &mut shutdown => break,
        };
        let _ = stream.set_nodelay(true);
        let router = router.clone();
        let svc = service_fn(move |req: hyper::Request<hyper::body::Incoming>| {
            let router = router.clone();
            async move {
                let req = Request::new(req.map(Body::Incoming)).with_body_limit(body_limit);
                Ok::<_, Infallible>(router.handle(req).await)
            }
        });
        let conn = graceful.watch(builder.serve_connection(TokioIo::new(stream), svc));
        tokio::spawn(async move {
            let _ = conn.await;
        });
    }
    drop(listener);
    // Idle keep-alive connections close now; active requests finish, bounded by the grace period.
    let _ = tokio::time::timeout(limits.shutdown_grace, graceful.shutdown()).await;
    Ok(())
}
