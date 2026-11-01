//! Radix-tree router.
//!
//! Static segments are stored with shared-prefix compression. `:name` matches one
//! path segment, `*name` matches the remainder of the path (must be last).
//! Lookup prefers static matches, then params, then catch-alls, and backtracks
//! when a more specific branch fails to match.

use std::sync::Arc;

/// Captured path parameters, in route order.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Params {
    inner: Vec<(Arc<str>, String)>,
}

impl Params {
    pub fn get(&self, name: &str) -> Option<&str> {
        self.inner
            .iter()
            .find(|(k, _)| &**k == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.inner.iter().map(|(k, v)| (&**k, v.as_str()))
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InsertError {
    /// The exact route is already registered.
    Duplicate(String),
    /// Two routes use different names for a param at the same position.
    ParamConflict { existing: String, new: String },
    /// Malformed route (empty param name, catch-all not last, ...).
    Invalid(String),
}

impl std::fmt::Display for InsertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InsertError::Duplicate(p) => write!(f, "duplicate route: {p}"),
            InsertError::ParamConflict { existing, new } => {
                write!(f, "param name conflict: `{existing}` vs `{new}`")
            }
            InsertError::Invalid(m) => write!(f, "invalid route: {m}"),
        }
    }
}

impl std::error::Error for InsertError {}

enum Piece<'a> {
    Static(&'a str),
    Param(&'a str),
    CatchAll(&'a str),
}

fn parse(path: &str) -> Result<Vec<Piece<'_>>, InsertError> {
    if !path.starts_with('/') {
        return Err(InsertError::Invalid(format!("`{path}` must start with '/'")));
    }
    let mut pieces = Vec::new();
    let bytes = path.as_bytes();
    let mut i = 0;
    let mut start = 0;
    while i < bytes.len() {
        match bytes[i] {
            b':' | b'*' => {
                if start < i {
                    pieces.push(Piece::Static(&path[start..i]));
                }
                let kind = bytes[i];
                let name_start = i + 1;
                let mut j = name_start;
                while j < bytes.len() && bytes[j] != b'/' {
                    j += 1;
                }
                let name = &path[name_start..j];
                if name.is_empty() {
                    return Err(InsertError::Invalid(format!("empty param name in `{path}`")));
                }
                if name.contains([':', '*']) {
                    return Err(InsertError::Invalid(format!("bad param name in `{path}`")));
                }
                if kind == b'*' {
                    if j != bytes.len() {
                        return Err(InsertError::Invalid(format!(
                            "catch-all must be the last segment in `{path}`"
                        )));
                    }
                    pieces.push(Piece::CatchAll(name));
                } else {
                    pieces.push(Piece::Param(name));
                }
                i = j;
                start = j;
            }
            _ => i += 1,
        }
    }
    if start < bytes.len() {
        pieces.push(Piece::Static(&path[start..]));
    }
    Ok(pieces)
}

struct ParamChild<T> {
    name: Arc<str>,
    node: Node<T>,
}

struct Node<T> {
    prefix: Box<str>,
    /// First byte of each static child's prefix, parallel to `children`.
    indices: Vec<u8>,
    children: Vec<Node<T>>,
    param: Option<Box<ParamChild<T>>>,
    catch_all: Option<(Arc<str>, T)>,
    value: Option<T>,
}

impl<T> Node<T> {
    fn new(prefix: &str) -> Self {
        Node {
            prefix: prefix.into(),
            indices: Vec::new(),
            children: Vec::new(),
            param: None,
            catch_all: None,
            value: None,
        }
    }

    fn insert(&mut self, pieces: &[Piece<'_>], full: &str, value: T) -> Result<(), InsertError> {
        let Some((first, rest)) = pieces.split_first() else {
            if self.value.is_some() {
                return Err(InsertError::Duplicate(full.to_string()));
            }
            self.value = Some(value);
            return Ok(());
        };
        match *first {
            Piece::Static(s) => self.insert_static(s, rest, full, value),
            Piece::Param(name) => {
                let child = self.param.get_or_insert_with(|| {
                    Box::new(ParamChild { name: name.into(), node: Node::new("") })
                });
                if &*child.name != name {
                    return Err(InsertError::ParamConflict {
                        existing: child.name.to_string(),
                        new: name.to_string(),
                    });
                }
                child.node.insert(rest, full, value)
            }
            Piece::CatchAll(name) => {
                if let Some((existing, _)) = &self.catch_all {
                    if &**existing != name {
                        return Err(InsertError::ParamConflict {
                            existing: existing.to_string(),
                            new: name.to_string(),
                        });
                    }
                    return Err(InsertError::Duplicate(full.to_string()));
                }
                self.catch_all = Some((name.into(), value));
                Ok(())
            }
        }
    }

    fn insert_static(
        &mut self,
        s: &str,
        rest: &[Piece<'_>],
        full: &str,
        value: T,
    ) -> Result<(), InsertError> {
        let first = s.as_bytes()[0];
        let Some(pos) = self.indices.iter().position(|&b| b == first) else {
            let mut child = Node::new(s);
            child.insert(rest, full, value)?;
            self.indices.push(first);
            self.children.push(child);
            return Ok(());
        };
        let child = &mut self.children[pos];
        let lcp = child
            .prefix
            .bytes()
            .zip(s.bytes())
            .take_while(|(a, b)| a == b)
            .count();
        if lcp < child.prefix.len() {
            // Split: child keeps its tail, a new node holds the shared head.
            let tail = Node {
                prefix: child.prefix[lcp..].into(),
                indices: std::mem::take(&mut child.indices),
                children: std::mem::take(&mut child.children),
                param: child.param.take(),
                catch_all: child.catch_all.take(),
                value: child.value.take(),
            };
            child.prefix = s[..lcp].into();
            child.indices = vec![tail.prefix.as_bytes()[0]];
            child.children = vec![tail];
        }
        if lcp == s.len() {
            child.insert(rest, full, value)
        } else {
            child.insert_static(&s[lcp..], rest, full, value)
        }
    }

    fn at<'n>(&'n self, path: &str, params: &mut Vec<(Arc<str>, String)>) -> Option<&'n T> {
        if path.is_empty() {
            if let Some(v) = &self.value {
                return Some(v);
            }
        }
        if let Some(&b) = path.as_bytes().first() {
            for (i, &idx) in self.indices.iter().enumerate() {
                if idx != b {
                    continue;
                }
                let child = &self.children[i];
                if let Some(rest) = path.strip_prefix(&*child.prefix) {
                    if let Some(v) = child.at(rest, params) {
                        return Some(v);
                    }
                }
                break;
            }
        }
        if let Some(p) = &self.param {
            let end = path.find('/').unwrap_or(path.len());
            if end > 0 {
                params.push((p.name.clone(), path[..end].to_string()));
                if let Some(v) = p.node.at(&path[end..], params) {
                    return Some(v);
                }
                params.pop();
            }
        }
        if let Some((name, v)) = &self.catch_all {
            if !path.is_empty() {
                params.push((name.clone(), path.to_string()));
                return Some(v);
            }
        }
        None
    }
}

/// A radix tree mapping route patterns to values.
pub struct Tree<T> {
    root: Node<T>,
}

impl<T> Default for Tree<T> {
    fn default() -> Self {
        Tree { root: Node::new("") }
    }
}

impl<T> Tree<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, path: &str, value: T) -> Result<(), InsertError> {
        let pieces = parse(path)?;
        self.root.insert(&pieces, path, value)
    }

    /// Mutable access to the value at an exact pattern, inserting the default if absent.
    pub fn entry_or_default(&mut self, path: &str) -> Result<&mut T, InsertError>
    where
        T: Default,
    {
        if self.get_pattern_mut(path)?.is_none() {
            self.insert(path, T::default())?;
        }
        Ok(self.get_pattern_mut(path)?.expect("just inserted"))
    }

    fn get_pattern_mut(&mut self, path: &str) -> Result<Option<&mut T>, InsertError> {
        let pieces = parse(path)?;
        let mut node = &mut self.root;
        let mut i = 0;
        while i < pieces.len() {
            match pieces[i] {
                Piece::Static(mut s) => loop {
                    let Some(pos) = node.indices.iter().position(|&b| b == s.as_bytes()[0])
                    else {
                        return Ok(None);
                    };
                    let child = &mut node.children[pos];
                    let Some(rest) = s.strip_prefix(&*child.prefix) else {
                        return Ok(None);
                    };
                    node = child;
                    if rest.is_empty() {
                        break;
                    }
                    s = rest;
                },
                Piece::Param(name) => match &mut node.param {
                    Some(p) if &*p.name == name => node = &mut p.node,
                    _ => return Ok(None),
                },
                Piece::CatchAll(name) => {
                    return Ok(match &mut node.catch_all {
                        Some((n, v)) if &**n == name => Some(v),
                        _ => None,
                    })
                }
            }
            i += 1;
        }
        Ok(node.value.as_mut())
    }

    /// Match a request path, returning the value and captured params.
    pub fn at(&self, path: &str) -> Option<(&T, Params)> {
        let mut params = Vec::new();
        self.root.at(path, &mut params).map(|v| (v, Params { inner: params }))
    }
}
