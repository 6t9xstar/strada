# Strada

A small HTTP backend framework for Rust, built on Tokio and Hyper 1.x.

**Status: v0.1.** Strada has a router, async handlers, and plaintext/JSON responses. It does not have middleware, TLS, HTTP/2, extractors, graceful shutdown of in-flight connections, or tracing yet. Don't use it in production.

```rust
use strada::{Json, Request, Router, Server};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let app = Router::new()
        .get("/", |_req: Request| async { "Hello, World!" })
        .get("/users/:id", |req: Request| async move {
            format!("user {}", req.param("id").unwrap_or_default())
        })
        .post("/echo", |mut req: Request| async move {
            req.json::<serde_json::Value>().await.map(Json)
        });
    Server::bind("127.0.0.1:8080").serve(app).await
}
```

## What's in v0.1

- **Server** ([src/server.rs](src/server.rs)): Hyper 1.x HTTP/1.1 with keep-alive and pipelining, `TCP_NODELAY`, and one Tokio task per connection. `serve_listener` takes an existing `TcpListener` and a shutdown future.
- **Router** ([src/router.rs](src/router.rs)): a radix tree with prefix-compressed static segments, `:param` (one segment), and `*catch_all` (the rest of the path). Lookup tries static routes first, then params, then catch-alls, and backtracks when a branch fails, so `/users/me` and `/users/:id/avatar` can coexist. Duplicate routes and conflicting param names are rejected when you register them. Unknown paths return 404. A known path with the wrong method returns 405 with an `Allow` header. `HEAD` falls back to `GET`.
- **Handlers**: any `Fn(Request) -> impl Future<Output = impl IntoResponse>`.
- **Responses**: `&'static str`, `String`, `Bytes`, `StatusCode`, `(StatusCode, T)`, `Result<T, E>`, and `Json<T>` (Serde).
- **Requests**: `method`, `uri`, `headers`, `param(name)`, `bytes().await`, and `json::<T>().await`. Bad JSON returns a 422.

## Tests

```bash
cargo test --workspace
```

There are 7 router tests and 8 tests that run over real TCP: plaintext, JSON, params, 404/405, JSON body round-trip, the 422 error, pipelined keep-alive, and HEAD. There is also 1 doc test.

## Benchmarks

### Microbenchmarks (Criterion)

```bash
cargo bench --bench router
```

| Benchmark | Time (median) |
|---|---:|
| Tree lookup, all 96 GitHub-API routes | 13.49 µs (~140 ns/route) |
| Tree lookup, static route | 40.8 ns |
| Tree lookup, 3 params (`/repos/:owner/:repo/pulls/:number/files`) | 240.1 ns |
| In-process dispatch, `/plaintext` (lookup + handler + response) | 276.5 ns |
| In-process dispatch, `/json` | 364.9 ns |
| In-process dispatch, `/users/:id` | 479.3 ns |

Raw output is in [bench/results/2026-11-01-win-i7-12800H/criterion.txt](bench/results/2026-11-01-win-i7-12800H/criterion.txt). I did not compare the router against `matchit` (Axum's router) or Actix's router. Param lookup allocates a `String` per captured value, so there is room to improve here.

### HTTP: TechEmpower plaintext and JSON vs Axum and Actix Web

```bash
cargo install oha --locked
bench/run.sh
```

The script builds all three servers in release mode (`lto = "fat"`, `codegen-units = 1`). Each server exposes identical `/plaintext` and `/json` endpoints ([examples/techempower.rs](examples/techempower.rs), [bench/axum-te](bench/axum-te/src/main.rs), [bench/actix-te](bench/actix-te/src/main.rs)). For each server the script warms up for 3 s, then runs [oha](https://github.com/hatoo/oha) for 15 s at 64, 256 and 512 connections. It saves raw JSON per run and generates `summary.md`. You can override `DURATION`, `CONNS`, `FRAMEWORKS` and `TESTS` with environment variables.

**Results, 2026-11-01.** Machine: i7-12800H (14 cores / 20 threads), Windows 11, Rust 1.97.1, hyper 1.12.0, axum 0.8.9, actix-web 4.18.1, oha 1.16.0. Client and server ran on the same machine over loopback. These are single runs.

| Test | Conns | Framework | Req/s | p50 (ms) | p95 (ms) | p99 (ms) |
|---|---:|---|---:|---:|---:|---:|
| plaintext | 64 | strada | 95,266 | 0.57 | 1.36 | 1.89 |
| plaintext | 64 | axum | 96,330 | 0.57 | 1.33 | 1.85 |
| plaintext | 64 | actix | **104,935** | 0.44 | 1.48 | 2.85 |
| plaintext | 256 | strada | 99,510 | 2.22 | 5.21 | 7.07 |
| plaintext | 256 | axum | 87,564 | 2.54 | 5.79 | 7.63 |
| plaintext | 256 | actix | **132,225** | 1.44 | 4.53 | 8.55 |
| plaintext | 512 | strada | 92,826 | 4.65 | 11.68 | 16.93 |
| plaintext | 512 | axum | 81,304 | 5.39 | 13.43 | 18.49 |
| plaintext | 512 | actix | **148,144** | 2.79 | 7.19 | 12.87 |
| json | 64 | strada | 96,889 | 0.57 | 1.33 | 1.86 |
| json | 64 | axum | 86,639 | 0.66 | 1.44 | 1.97 |
| json | 64 | actix | **143,342** | 0.31 | 1.11 | 2.22 |
| json | 256 | strada | 102,145 | 2.16 | 5.14 | 6.98 |
| json | 256 | axum | 84,551 | 2.69 | 6.01 | 7.78 |
| json | 256 | actix | **139,763** | 1.40 | 4.22 | 7.45 |
| json | 512 | strada | 91,100 | 4.62 | 12.41 | 18.67 |
| json | 512 | axum | 74,280 | 5.96 | 14.52 | 19.78 |
| json | 512 | actix | **131,571** | 3.07 | 8.42 | 14.85 |

Every response was a 200 and there were no connection errors. The only errors oha reported were requests still in flight when the 15 s deadline hit, and the summary excludes those. Raw per-run oha JSON, the environment and the summary are in [bench/results/2026-11-01-win-i7-12800H/](bench/results/2026-11-01-win-i7-12800H/).

**What the numbers show:**

- **Actix Web is fastest in every configuration.** It had about 1.1× to 1.6× Strada's throughput and the lowest p50 everywhere. Its p99 was higher than Strada's at 64 and 256 connections, and lower at 512.
- **Strada vs Axum:**
  - Both use Hyper, and they were tied at plaintext with 64 connections (within 1.1%, Axum slightly ahead).
  - In the other 5 configurations Strada had 12% to 23% more requests per second than Axum, with lower latency at every percentile.
  - Treat this as provisional. These are single runs on a laptop, and Axum does more per request (Tower service stack, extractors).
- **No configuration shows Strada beating Actix.**

**Limitations of this setup**, which mean these numbers are not comparable to official TechEmpower results:

- The load generator and the servers share 20 hardware threads, so oha competes with the server for CPU. That likely caps all three servers, and it explains why throughput barely grows from 64 to 512 connections.
- The tests run on Windows, where Tokio and Actix both use IOCP-backed polling. Linux with epoll or io_uring will produce different numbers, and possibly a different ranking.
- There is no HTTP pipelining. TechEmpower plaintext uses a pipeline depth of 16, and oha doesn't pipeline.
- I used oha instead of wrk because wrk doesn't build natively on Windows. oha also reports p95, which wrk's `--latency` output doesn't.
- The results are single 15 s runs with no repeats or confidence intervals. To get trustworthy numbers, run the script several times on an idle Linux box, with the client on a separate machine or pinned to separate cores (`taskset`).

## Layout

```
src/            framework (router, server, request/response, handler)
tests/          router + end-to-end TCP tests
benches/        Criterion benchmarks
examples/       techempower.rs (Strada benchmark server)
bench/          axum-te, actix-te comparison servers; run.sh, summarize.mjs; results/
```

## Roadmap

- **0.2:** Tower `Service` integration and middleware, typed extractors, graceful shutdown that drains connections, and `tracing`.
- **0.3:** HTTP/2 and TLS via Rustls.
- **Later:** a Clap-based CLI, then production hardening (timeouts, limits, backpressure) toward 1.0.

## License

MIT OR Apache-2.0
