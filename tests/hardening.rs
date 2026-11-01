use std::time::Duration;
use strada::{serve_listener_with, Limits, Request, Router};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;

fn app() -> Router {
    Router::new()
        .post("/upload", |mut r: Request| async move {
            r.bytes().await.map(|b| format!("got {}", b.len()))
        })
        .get("/panic", |_r: Request| async {
            if true {
                panic!("handler bug");
            }
            "unreachable"
        })
        .get("/ok", |_r: Request| async { "ok" })
        .get("/slow", |_r: Request| async {
            tokio::time::sleep(Duration::from_millis(300)).await;
            "slow done"
        })
}

async fn start(
    limits: Limits,
) -> (
    std::net::SocketAddr,
    oneshot::Sender<()>,
    tokio::task::JoinHandle<()>,
) {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    let (tx, rx) = oneshot::channel::<()>();
    let task = tokio::spawn(async move {
        serve_listener_with(
            l,
            app(),
            async {
                let _ = rx.await;
            },
            limits,
        )
        .await
        .unwrap();
    });
    (addr, tx, task)
}

async fn raw(addr: std::net::SocketAddr, req: &[u8]) -> String {
    let mut s = TcpStream::connect(addr).await.unwrap();
    s.write_all(req).await.unwrap();
    let mut out = Vec::new();
    let _ = s.read_to_end(&mut out).await;
    String::from_utf8_lossy(&out).into_owned()
}

fn small_limits() -> Limits {
    Limits {
        body_limit: 16,
        ..Limits::default()
    }
}

#[tokio::test]
async fn body_within_limit_is_read() {
    let (addr, _tx, _t) = start(small_limits()).await;
    let res = raw(
        addr,
        b"POST /upload HTTP/1.1\r\nHost: x\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello",
    )
    .await;
    assert!(res.ends_with("got 5"), "{res}");
}

#[tokio::test]
async fn declared_oversized_body_gets_413() {
    let (addr, _tx, _t) = start(small_limits()).await;
    let body = "x".repeat(17);
    let req = format!(
        "POST /upload HTTP/1.1\r\nHost: x\r\nContent-Length: 17\r\nConnection: close\r\n\r\n{body}"
    );
    assert!(raw(addr, req.as_bytes()).await.starts_with("HTTP/1.1 413"));
}

#[tokio::test]
async fn chunked_oversized_body_gets_413() {
    let (addr, _tx, _t) = start(small_limits()).await;
    // No Content-Length: the limit must be enforced while streaming.
    let req = b"POST /upload HTTP/1.1\r\nHost: x\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\na\r\n0123456789\r\na\r\n0123456789\r\n0\r\n\r\n";
    assert!(raw(addr, req).await.starts_with("HTTP/1.1 413"));
}

#[tokio::test]
async fn handler_panic_returns_500_and_server_survives() {
    let (addr, _tx, _t) = start(Limits::default()).await;
    let res = raw(
        addr,
        b"GET /panic HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n",
    )
    .await;
    assert!(res.starts_with("HTTP/1.1 500"), "{res}");
    let res = raw(
        addr,
        b"GET /ok HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n",
    )
    .await;
    assert!(res.ends_with("ok"), "{res}");
}

#[tokio::test]
async fn slow_header_connection_is_closed() {
    let limits = Limits {
        header_read_timeout: Duration::from_millis(200),
        ..Limits::default()
    };
    let (addr, _tx, _t) = start(limits).await;
    let mut s = TcpStream::connect(addr).await.unwrap();
    s.write_all(b"GET /ok HTTP/1.1\r\nHost: x\r\n")
        .await
        .unwrap(); // never finishes the head
    let mut buf = Vec::new();
    let read = tokio::time::timeout(Duration::from_secs(3), s.read_to_end(&mut buf)).await;
    assert!(
        read.is_ok(),
        "server kept a stalled connection open past the header timeout"
    );
    assert!(!String::from_utf8_lossy(&buf).contains("200 OK"));
}

#[tokio::test]
async fn graceful_shutdown_finishes_in_flight_requests() {
    let (addr, tx, task) = start(Limits::default()).await;
    let inflight = tokio::spawn(async move {
        raw(
            addr,
            b"GET /slow HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n",
        )
        .await
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    tx.send(()).unwrap();
    assert!(inflight.await.unwrap().ends_with("slow done"));
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .expect("server did not stop")
        .unwrap();
    assert!(
        TcpStream::connect(addr).await.is_err(),
        "listener still accepting after shutdown"
    );
}
