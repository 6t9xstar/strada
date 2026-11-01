use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use serde::Serialize;

#[derive(Serialize)]
struct Message {
    message: &'static str,
}

#[get("/plaintext")]
async fn plaintext() -> impl Responder {
    HttpResponse::Ok().content_type("text/plain; charset=utf-8").body("Hello, World!")
}

#[get("/json")]
async fn json() -> impl Responder {
    web::Json(Message { message: "Hello, World!" })
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".into());
    HttpServer::new(|| App::new().service(plaintext).service(json))
        .bind(format!("127.0.0.1:{port}"))?
        .run()
        .await
}
