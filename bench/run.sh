#!/usr/bin/env bash
# Reproducible TechEmpower-style plaintext/JSON benchmark: Strada vs Axum vs Actix Web.
#
# Requires: cargo, oha (`cargo install oha --locked`), node (for the summary table).
# Usage:    bench/run.sh                 # all frameworks, 64/256/512 connections, 15s each
#           DURATION=30s CONNS="256" FRAMEWORKS="strada axum" bench/run.sh
#
# Raw oha JSON for every run lands in bench/results/<stamp>/, plus env.txt and summary.md.
set -euo pipefail

cd "$(dirname "$0")/.."
DURATION=${DURATION:-15s}
WARMUP=${WARMUP:-3s}
CONNS=${CONNS:-"64 256 512"}
FRAMEWORKS=${FRAMEWORKS:-"strada axum actix"}
TESTS=${TESTS:-"plaintext json"}
PORT=${PORT:-18080}
STAMP=${STAMP:-$(date -u +%Y%m%dT%H%M%SZ)}
OUT="bench/results/$STAMP"
mkdir -p "$OUT"

command -v oha >/dev/null || { echo "oha not found: cargo install oha --locked" >&2; exit 1; }

echo "Building release binaries..."
cargo build --release --example techempower -p strada
cargo build --release -p axum-te -p actix-te

EXE=""
case "$(uname -s)" in MINGW*|MSYS*|CYGWIN*) EXE=".exe" ;; esac
bin_for() {
  case "$1" in
    strada) echo "target/release/examples/techempower$EXE" ;;
    axum)   echo "target/release/axum-te$EXE" ;;
    actix)  echo "target/release/actix-te$EXE" ;;
  esac
}

{
  echo "date_utc: $STAMP"
  echo "os: $(uname -srm)"
  echo "rustc: $(rustc --version)"
  echo "oha: $(oha --version)"
  echo "duration: $DURATION  warmup: $WARMUP  connections: $CONNS"
  if command -v lscpu >/dev/null; then lscpu | grep -E 'Model name|^CPU\(s\)'
  elif command -v powershell >/dev/null; then
    powershell -NoProfile -Command "(Get-CimInstance Win32_Processor | Select-Object -First 1 | ForEach-Object { 'cpu: ' + \$_.Name + ' (' + \$_.NumberOfCores + ' cores / ' + \$_.NumberOfLogicalProcessors + ' threads)' })"
  fi
  echo "client and server on the same machine, loopback (127.0.0.1)"
  grep -A2 -E '^name = "(hyper|axum|actix-web|tokio)"$' Cargo.lock | grep -E 'name|version' | paste - - || true
} > "$OUT/env.txt"
cat "$OUT/env.txt"

PID=""
cleanup() { [ -n "$PID" ] && kill "$PID" 2>/dev/null || true; }
trap cleanup EXIT

wait_up() {
  for _ in $(seq 1 100); do
    curl -fs "http://127.0.0.1:$PORT/plaintext" >/dev/null 2>&1 && return 0
    sleep 0.1
  done
  echo "server did not start" >&2; return 1
}

for fw in $FRAMEWORKS; do
  PORT=$PORT "$(bin_for "$fw")" & PID=$!
  wait_up
  for t in $TESTS; do
    url="http://127.0.0.1:$PORT/$t"
    oha -z "$WARMUP" -c 256 --no-tui --output-format quiet "$url" || true
    for c in $CONNS; do
      echo ">> $fw $t c=$c ($DURATION)"
      oha -z "$DURATION" -c "$c" --no-tui --output-format json -o "$OUT/$fw-$t-c$c.json" "$url"
      sleep 1
    done
  done
  kill "$PID"; wait "$PID" 2>/dev/null || true; PID=""
  sleep 1
done

node bench/summarize.mjs "$OUT" | tee "$OUT/summary.md"
