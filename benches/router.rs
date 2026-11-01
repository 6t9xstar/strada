use criterion::{black_box, criterion_group, criterion_main, Criterion};
use strada::{Body, Json, Request, Router, Tree};

// A GitHub-API-shaped route set, a common router benchmark corpus.
const ROUTES: &[&str] = &[
    "/authorizations",
    "/authorizations/:id",
    "/applications/:client_id/tokens/:access_token",
    "/events",
    "/repos/:owner/:repo/events",
    "/networks/:owner/:repo/events",
    "/orgs/:org/events",
    "/users/:user/received_events",
    "/users/:user/received_events/public",
    "/users/:user/events",
    "/users/:user/events/public",
    "/users/:user/events/orgs/:org",
    "/feeds",
    "/notifications",
    "/repos/:owner/:repo/notifications",
    "/notifications/threads/:id",
    "/notifications/threads/:id/subscription",
    "/repos/:owner/:repo/stargazers",
    "/users/:user/starred",
    "/user/starred",
    "/user/starred/:owner/:repo",
    "/repos/:owner/:repo/subscribers",
    "/users/:user/subscriptions",
    "/user/subscriptions",
    "/repos/:owner/:repo/subscription",
    "/user/subscriptions/:owner/:repo",
    "/users/:user/gists",
    "/gists",
    "/gists/:id",
    "/gists/:id/star",
    "/repos/:owner/:repo/git/blobs/:sha",
    "/repos/:owner/:repo/git/commits/:sha",
    "/repos/:owner/:repo/git/refs",
    "/repos/:owner/:repo/git/tags/:sha",
    "/repos/:owner/:repo/git/trees/:sha",
    "/issues",
    "/user/issues",
    "/orgs/:org/issues",
    "/repos/:owner/:repo/issues",
    "/repos/:owner/:repo/issues/:number",
    "/repos/:owner/:repo/assignees",
    "/repos/:owner/:repo/assignees/:assignee",
    "/repos/:owner/:repo/issues/:number/comments",
    "/repos/:owner/:repo/issues/:number/events",
    "/repos/:owner/:repo/labels",
    "/repos/:owner/:repo/labels/:name",
    "/repos/:owner/:repo/milestones",
    "/repos/:owner/:repo/milestones/:number",
    "/emojis",
    "/gitignore/templates",
    "/gitignore/templates/:name",
    "/meta",
    "/rate_limit",
    "/users/:user/orgs",
    "/user/orgs",
    "/orgs/:org",
    "/orgs/:org/members",
    "/orgs/:org/teams",
    "/teams/:id",
    "/teams/:id/members",
    "/user/teams",
    "/repos/:owner/:repo/pulls",
    "/repos/:owner/:repo/pulls/:number",
    "/repos/:owner/:repo/pulls/:number/commits",
    "/repos/:owner/:repo/pulls/:number/files",
    "/repos/:owner/:repo/pulls/:number/merge",
    "/user/repos",
    "/users/:user/repos",
    "/orgs/:org/repos",
    "/repositories",
    "/repos/:owner/:repo",
    "/repos/:owner/:repo/contributors",
    "/repos/:owner/:repo/languages",
    "/repos/:owner/:repo/branches",
    "/repos/:owner/:repo/branches/:branch",
    "/repos/:owner/:repo/collaborators",
    "/repos/:owner/:repo/commits",
    "/repos/:owner/:repo/commits/:sha",
    "/repos/:owner/:repo/readme",
    "/repos/:owner/:repo/contents/*path",
    "/repos/:owner/:repo/keys",
    "/repos/:owner/:repo/releases",
    "/repos/:owner/:repo/releases/:id",
    "/search/repositories",
    "/search/code",
    "/search/issues",
    "/search/users",
    "/users/:user",
    "/user",
    "/users",
    "/user/emails",
    "/users/:user/followers",
    "/user/followers",
    "/users/:user/following",
    "/user/following",
    "/user/keys",
    "/user/keys/:id",
];

fn concrete(route: &str) -> String {
    route
        .split('/')
        .map(|s| match s.as_bytes().first() {
            Some(b':') => "abc123",
            Some(b'*') => "src/lib.rs",
            _ => s,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn bench_tree(c: &mut Criterion) {
    let mut tree = Tree::new();
    for (i, r) in ROUTES.iter().enumerate() {
        tree.insert(r, i).unwrap();
    }
    let paths: Vec<String> = ROUTES.iter().map(|r| concrete(r)).collect();

    c.bench_function("tree/lookup_all_96_github_routes", |b| {
        b.iter(|| {
            for p in &paths {
                black_box(tree.at(black_box(p)).unwrap());
            }
        })
    });
    c.bench_function("tree/lookup_static", |b| {
        b.iter(|| black_box(tree.at(black_box("/user/subscriptions")).unwrap()))
    });
    c.bench_function("tree/lookup_3_params", |b| {
        b.iter(|| black_box(tree.at(black_box("/repos/rust-lang/rust/pulls/42/files")).unwrap()))
    });
}

#[derive(serde::Serialize)]
struct Message {
    message: &'static str,
}

fn bench_dispatch(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_current_thread().build().unwrap();
    let app = Router::new()
        .get("/plaintext", |_r: Request| async { "Hello, World!" })
        .get("/json", |_r: Request| async { Json(Message { message: "Hello, World!" }) })
        .get("/users/:id", |r: Request| async move {
            format!("user {}", r.param("id").unwrap_or_default())
        });
    let req = |p: &'static str| Request::new(http::Request::get(p).body(Body::from("")).unwrap());
    for path in ["/plaintext", "/json", "/users/42"] {
        c.bench_function(&format!("dispatch{path}"), |b| {
            b.to_async(&rt).iter(|| async { black_box(app.handle(req(path)).await) })
        });
    }
}

criterion_group!(benches, bench_tree, bench_dispatch);
criterion_main!(benches);
