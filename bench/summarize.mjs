// Turn a directory of oha JSON results into a Markdown table.
// Usage: node bench/summarize.mjs bench/results/<stamp>
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const dir = process.argv[2];
if (!dir) throw new Error("usage: summarize.mjs <results-dir>");

const rows = [];
for (const f of readdirSync(dir)) {
  const m = f.match(/^(\w+)-(\w+)-c(\d+)\.json$/);
  if (!m) continue;
  const r = JSON.parse(readFileSync(join(dir, f), "utf8"));
  const p = r.latencyPercentiles;
  const codes = r.statusCodeDistribution ?? {};
  const ok = codes["200"] ?? 0;
  const total = Object.values(codes).reduce((a, b) => a + b, 0);
  // In-flight requests cancelled when -z expires are not real failures.
  const errors = Object.entries(r.errorDistribution ?? {})
    .filter(([k]) => k !== "aborted due to deadline")
    .reduce((a, [, n]) => a + n, 0);
  rows.push({
    fw: m[1], test: m[2], c: +m[3],
    rps: r.summary.requestsPerSec,
    p50: p.p50 * 1000, p95: p.p95 * 1000, p99: p.p99 * 1000,
    non200: total - ok, errors,
  });
}

const order = { strada: 0, axum: 1, actix: 2 };
rows.sort((a, b) => a.test.localeCompare(b.test) || a.c - b.c || (order[a.fw] ?? 9) - (order[b.fw] ?? 9));

const fmt = (n, d = 2) => n.toLocaleString("en-US", { minimumFractionDigits: d, maximumFractionDigits: d });
console.log("| Test | Conns | Framework | Req/s | p50 (ms) | p95 (ms) | p99 (ms) | Non-200 | Errors |");
console.log("|---|---:|---|---:|---:|---:|---:|---:|---:|");
for (const r of rows) {
  console.log(`| ${r.test} | ${r.c} | ${r.fw} | ${fmt(r.rps, 0)} | ${fmt(r.p50)} | ${fmt(r.p95)} | ${fmt(r.p99)} | ${r.non200} | ${r.errors} |`);
}
