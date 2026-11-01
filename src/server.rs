use crate::{Body, Request, Router};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;
use std::convert::Infallible;
use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, ToSocketAddrs};

/// HTTP/1.1 server. Spawns one Tokio task per connection.
pub struct Server<A> {
    addr: A,
}

impl<A: ToSocketAddrs> Server<A> {
    pub fn bind(addr: A) -> Self {
        Server { addr }
    }

    pub async fn serve(self, router: Router) -> std::io::Result<()> {
        let listener = TcpListener::bind(self.addr).await?;
        serve_listener(listener, router, std::future::pending()).await
    }
}

/// Serve on an existing listener until `shutdown` resolves. In-flight
/// connections are left to finish on their own tasks.
pub async fn serve_listener(
    listener: TcpListener,
    router: Router,
    shutdown: impl Future<Output = ()>,
) -> std::io::Result<()> {
    let router = Arc::new(router);
    tokio::pin!(shutdown);
    loop {
        let (stream, _peer): (_, SocketAddr) = tokio::select! {
            res = listener.accept() => match res {
                Ok(s) => s,
                // Transient accept errors (e.g. EMFILE) shouldn't kill the server.
                Err(_) => continue,
            },
            _ = &mut shutdown => return Ok(()),
        };
        let _ = stream.set_nodelay(true);
        let router = router.clone();
        tokio::spawn(async move {
            let svc = service_fn(move |req: hyper::Request<hyper::body::Incoming>| {
                let router = router.clone();
                async move {
                    let req = Request::new(req.map(Body::Incoming));
                    Ok::<_, Infallible>(router.handle(req).await)
                }
            });
            let _ = http1::Builder::new()
                .keep_alive(true)
                .serve_connection(TokioIo::new(stream), svc)
                .await;
        });
    }
}
