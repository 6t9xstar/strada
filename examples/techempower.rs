//! TechEmpower plaintext + JSON endpoints. `PORT` env var, default 8080.
use serde::Serialize;
use strada::{Json, Request, Router, Server};

#[derive(Serialize)]
struct Message {
    message: &'static str,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".into());
    let app = Router::new()
        .get("/plaintext", |_req: Request| async { "Hello, World!" })
        .get("/json", |_req: Request| async {
            Json(Message { message: "Hello, World!" })
        });
    Server::bind(format!("127.0.0.1:{port}")).serve(app).await
}
