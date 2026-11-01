use serde::{Deserialize, Serialize};
use strada::{serve_listener, Json, Request, Router, StatusCode};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

#[derive(Serialize, Deserialize)]
struct Msg {
    message: String,
}

fn app() -> Router {
    Router::new()
        .get("/plaintext", |_r: Request| async { "Hello, World!" })
        .get("/json", |_r: Request| async {
            Json(Msg { message: "Hello, World!".into() })
        })
        .get("/users/:id", |r: Request| async move {
            format!("user={}", r.param("id").unwrap())
        })
        .post("/echo", |mut r: Request| async move { r.json::<Msg>().await.map(Json) })
}

async fn start() -> std::net::SocketAddr {
    let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    tokio::spawn(serve_listener(l, app(), std::future::pending()));
    addr
}

async fn raw(addr: std::net::SocketAddr, req: &str) -> String {
    let mut s = TcpStream::connect(addr).await.unwrap();
    s.write_all(req.as_bytes()).await.unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).await.unwrap();
    out
}

fn req(method: &str, path: &str) -> String {
    format!("{method} {path} HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
}

#[tokio::test]
async fn plaintext() {
    let res = raw(start().await, &req("GET", "/plaintext")).await;
    let lower = res.to_lowercase();
    assert!(res.starts_with("HTTP/1.1 200 OK"), "{res}");
    assert!(lower.contains("content-type: text/plain"));
    assert!(lower.contains("date: "));
    assert!(res.ends_with("\r\n\r\nHello, World!"));
}

#[tokio::test]
async fn json() {
    let res = raw(start().await, &req("GET", "/json")).await;
    assert!(res.to_lowercase().contains("content-type: application/json"));
    assert!(res.ends_with(r#"{"message":"Hello, World!"}"#), "{res}");
}

#[tokio::test]
async fn path_params() {
    let res = raw(start().await, &req("GET", "/users/42")).await;
    assert!(res.ends_with("user=42"), "{res}");
}

#[tokio::test]
async fn not_found_and_method_not_allowed() {
    let addr = start().await;
    assert!(raw(addr, &req("GET", "/nope")).await.starts_with("HTTP/1.1 404"));
    let res = raw(addr, &req("DELETE", "/plaintext")).await;
    assert!(res.starts_with("HTTP/1.1 405"), "{res}");
    assert!(res.to_lowercase().contains("allow: get"));
}

#[tokio::test]
async fn json_body_roundtrip_and_error() {
    let addr = start().await;
    let body = r#"{"message":"hi"}"#;
    let ok = format!(
        "POST /echo HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    assert!(raw(addr, &ok).await.ends_with(body));
    let bad = "POST /echo HTTP/1.1\r\nHost: x\r\nContent-Length: 3\r\nConnection: close\r\n\r\nbad";
    assert!(raw(addr, bad).await.starts_with("HTTP/1.1 422"));
}

#[tokio::test]
async fn keep_alive_serves_pipelined_requests() {
    let addr = start().await;
    let one = "GET /plaintext HTTP/1.1\r\nHost: x\r\n\r\n";
    let res = raw(addr, &format!("{one}{one}{}", req("GET", "/json"))).await;
    assert_eq!(res.matches("HTTP/1.1 200 OK").count(), 3);
}

#[tokio::test]
async fn head_falls_back_to_get() {
    let res = raw(start().await, &req("HEAD", "/plaintext")).await;
    assert!(res.starts_with("HTTP/1.1 200"));
    assert!(!res.contains("Hello"));
}

#[test]
#[should_panic(expected = "duplicate route")]
fn duplicate_method_route_panics() {
    let _ = Router::new()
        .get("/a", |_r: Request| async { StatusCode::OK })
        .get("/a", |_r: Request| async { StatusCode::OK });
}
