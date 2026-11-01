# Changelog

Strada follows [Semantic Versioning](https://semver.org). Before 1.0, minor versions may break the API.

## [Unreleased]

### Added
- Request body limit (default 2 MiB): `Server::body_limit`, `Request::with_body_limit`. Oversized bodies, declared or streamed, return `413`. New `BodyError` type.
- Header read timeout (default 10 s, `Server::header_read_timeout`) to stop slowloris connections.
- A panicking handler now returns `500` and the server keeps running.
- Graceful shutdown: `Server::serve_with_shutdown(router, signal)` stops accepting, closes idle keep-alive connections and lets in-flight requests finish, up to `shutdown_grace` (default 30 s).
- `Limits` and `serve_listener_with` for explicit configuration.
- CI on Linux, Windows and macOS: fmt, clippy `-D warnings`, docs, tests, MSRV 1.85, `cargo package`, benchmark smoke run.

### Changed
- `Request::bytes()` now returns `Result<Bytes, BodyError>` instead of `Result<Bytes, hyper::Error>`.
- Accept errors (e.g. out of file descriptors) back off for 50 ms instead of spinning.
- The workspace release profile no longer uses `panic = "abort"`. The published 2026-11-01 benchmark numbers were measured with it, for all three frameworks.

## [0.1.0] - 2026-11-01
- Tokio + Hyper 1.x HTTP/1.1 server, radix-tree router with `:param` and `*catch_all`, async handlers, plaintext/JSON responses, Criterion and oha benchmarks against Axum and Actix Web.
