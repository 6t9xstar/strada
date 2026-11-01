use strada::{InsertError, Tree};

fn tree(routes: &[&str]) -> Tree<String> {
    let mut t = Tree::new();
    for r in routes {
        t.insert(r, r.to_string()).unwrap();
    }
    t
}

type Hit = (String, Vec<(String, String)>);

fn hit(t: &Tree<String>, path: &str) -> Option<Hit> {
    t.at(path).map(|(v, p)| {
        (
            v.clone(),
            p.iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        )
    })
}

fn kv(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn static_routes_with_shared_prefixes() {
    let t = tree(&["/", "/user", "/users", "/user/profile", "/u"]);
    for p in ["/", "/user", "/users", "/user/profile", "/u"] {
        assert_eq!(hit(&t, p).unwrap().0, p);
    }
    assert!(hit(&t, "/us").is_none());
    assert!(hit(&t, "/user/").is_none());
    assert!(hit(&t, "/userss").is_none());
}

#[test]
fn params() {
    let t = tree(&["/users/:id", "/users/:id/posts/:post"]);
    assert_eq!(
        hit(&t, "/users/42").unwrap(),
        ("/users/:id".into(), kv(&[("id", "42")]))
    );
    assert_eq!(
        hit(&t, "/users/7/posts/abc").unwrap(),
        (
            "/users/:id/posts/:post".into(),
            kv(&[("id", "7"), ("post", "abc")])
        )
    );
    assert!(hit(&t, "/users/").is_none());
    assert!(hit(&t, "/users/7/posts").is_none());
}

#[test]
fn static_beats_param_and_backtracks() {
    let t = tree(&[
        "/users/me",
        "/users/:id",
        "/users/me/settings",
        "/users/:id/avatar",
    ]);
    assert_eq!(hit(&t, "/users/me").unwrap().0, "/users/me");
    assert_eq!(hit(&t, "/users/meow").unwrap().0, "/users/:id");
    assert_eq!(
        hit(&t, "/users/me/settings").unwrap().0,
        "/users/me/settings"
    );
    // The static `me` branch has no /avatar, so lookup must backtrack to the param.
    assert_eq!(
        hit(&t, "/users/me/avatar").unwrap(),
        ("/users/:id/avatar".into(), kv(&[("id", "me")]))
    );
}

#[test]
fn catch_all() {
    let t = tree(&["/static/*path", "/static/index.html"]);
    assert_eq!(
        hit(&t, "/static/index.html").unwrap().0,
        "/static/index.html"
    );
    assert_eq!(
        hit(&t, "/static/css/site.css").unwrap(),
        ("/static/*path".into(), kv(&[("path", "css/site.css")]))
    );
    assert!(hit(&t, "/static/").is_none());
}

#[test]
fn param_after_static_text_in_segment() {
    let t = tree(&["/files/v:version/download"]);
    assert_eq!(
        hit(&t, "/files/v1.2/download").unwrap().1,
        kv(&[("version", "1.2")])
    );
}

#[test]
fn insert_errors() {
    let mut t = Tree::new();
    t.insert("/a/:id", 1).unwrap();
    assert!(matches!(
        t.insert("/a/:id", 2),
        Err(InsertError::Duplicate(_))
    ));
    assert!(matches!(
        t.insert("/a/:name", 2),
        Err(InsertError::ParamConflict { .. })
    ));
    assert!(matches!(
        t.insert("no-slash", 2),
        Err(InsertError::Invalid(_))
    ));
    assert!(matches!(t.insert("/a/:", 2), Err(InsertError::Invalid(_))));
    assert!(matches!(
        t.insert("/a/*rest/x", 2),
        Err(InsertError::Invalid(_))
    ));
}

#[test]
fn node_split_preserves_existing_routes() {
    let t = tree(&["/search/repositories", "/search/code", "/se", "/s/:x"]);
    assert_eq!(
        hit(&t, "/search/repositories").unwrap().0,
        "/search/repositories"
    );
    assert_eq!(hit(&t, "/search/code").unwrap().0, "/search/code");
    assert_eq!(hit(&t, "/se").unwrap().0, "/se");
    assert_eq!(hit(&t, "/s/1").unwrap().0, "/s/:x");
}
