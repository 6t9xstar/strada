# Contributing to Strada

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo bench --bench router          # Criterion microbenchmarks
bench/run.sh                        # HTTP benchmark vs Axum/Actix (needs `cargo install oha`)
```

- Every behaviour change needs a test (`tests/`).
- Performance claims need before/after numbers from the same machine. Never claim a win the numbers don't show.
- Update `CHANGELOG.md` under `Unreleased`.
- MSRV is Rust 1.85 (hyper-util uses edition 2024); CI enforces it.
