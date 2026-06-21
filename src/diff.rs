//! The semantic diff engine: recursively compare two `serde_json::Value` trees
//! into a flat, path-addressed change list.
//!
//! Paths use dotpick's dotpath vocabulary (`user.plan`, `items[2].qty`,
//! `["quoted key"]`) so the two tools speak the same language. Objects compare
//! by key, scalars by value, and arrays by index - unless `array_key` is set,
//! in which case arrays of objects match by that field (order-independent),
//! and locators read `items[id=42]`.

use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;

/// The kind of change at a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Op {
    Added,
    Removed,
    Changed,
}

/// One difference between the two inputs.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Change {
    pub op: Op,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new: Option<Value>,
}

impl Change {
    fn added(path: String, new: Value) -> Self {
        Self {
            op: Op::Added,
            path,
            old: None,
            new: Some(new),
        }
    }
    fn removed(path: String, old: Value) -> Self {
        Self {
            op: Op::Removed,
            path,
            old: Some(old),
            new: None,
        }
    }
    fn changed(path: String, old: Value, new: Value) -> Self {
        Self {
            op: Op::Changed,
            path,
            old: Some(old),
            new: Some(new),
        }
    }
}

/// How the diff treats arrays.
#[derive(Debug, Clone, Default)]
pub struct DiffOptions {
    /// Match arrays of objects by this key field instead of by index.
    pub array_key: Option<String>,
}

/// Compute the ordered list of changes turning `left` into `right`.
pub fn diff(left: &Value, right: &Value, opts: &DiffOptions) -> Vec<Change> {
    let mut out = Vec::new();
    walk("", left, right, opts, &mut out);
    out
}

fn walk(path: &str, a: &Value, b: &Value, opts: &DiffOptions, out: &mut Vec<Change>) {
    if a == b {
        return;
    }
    match (a, b) {
        (Value::Object(ma), Value::Object(mb)) => {
            let mut seen = HashSet::new();
            for (k, av) in ma {
                seen.insert(k.as_str());
                match mb.get(k) {
                    Some(bv) => walk(&render_key(path, k), av, bv, opts, out),
                    None => out.push(Change::removed(render_key(path, k), av.clone())),
                }
            }
            for (k, bv) in mb {
                if !seen.contains(k.as_str()) {
                    out.push(Change::added(render_key(path, k), bv.clone()));
                }
            }
        }
        (Value::Array(va), Value::Array(vb)) => {
            if let Some(key) = &opts.array_key
                && let (Some(am), Some(bm)) = (index_by_key(va, key), index_by_key(vb, key))
            {
                diff_keyed(path, key, &am, &bm, opts, out);
            } else {
                diff_indexed(path, va, vb, opts, out);
            }
        }
        _ => out.push(Change::changed(emit_path(path), a.clone(), b.clone())),
    }
}

fn diff_indexed(path: &str, va: &[Value], vb: &[Value], opts: &DiffOptions, out: &mut Vec<Change>) {
    for i in 0..va.len().max(vb.len()) {
        match (va.get(i), vb.get(i)) {
            (Some(a), Some(b)) => walk(&render_index(path, i), a, b, opts, out),
            (Some(a), None) => out.push(Change::removed(render_index(path, i), a.clone())),
            (None, Some(b)) => out.push(Change::added(render_index(path, i), b.clone())),
            (None, None) => unreachable!("index within max length"),
        }
    }
}

/// Index an array of objects by a key field's scalar value, preserving order.
/// Returns `None` (so the caller falls back to index diffing) unless every
/// element is an object carrying a unique value for `key`.
fn index_by_key<'a>(arr: &'a [Value], key: &str) -> Option<Vec<(String, &'a Value)>> {
    let mut seen = HashSet::new();
    let mut pairs = Vec::with_capacity(arr.len());
    for el in arr {
        let kv = el.as_object()?.get(key)?;
        let id = scalar_locator(kv)?;
        if !seen.insert(id.clone()) {
            return None; // ambiguous: duplicate key value
        }
        pairs.push((id, el));
    }
    Some(pairs)
}

fn diff_keyed(
    path: &str,
    key: &str,
    am: &[(String, &Value)],
    bm: &[(String, &Value)],
    opts: &DiffOptions,
    out: &mut Vec<Change>,
) {
    let a_ids: HashSet<&str> = am.iter().map(|(k, _)| k.as_str()).collect();
    let b_index: std::collections::HashMap<&str, &Value> =
        bm.iter().map(|(k, v)| (k.as_str(), *v)).collect();

    for (id, av) in am {
        let loc = format!("{path}[{key}={id}]");
        match b_index.get(id.as_str()) {
            Some(bv) => walk(&loc, av, bv, opts, out),
            None => out.push(Change::removed(loc, (*av).clone())),
        }
    }
    for (id, bv) in bm {
        if !a_ids.contains(id.as_str()) {
            out.push(Change::added(format!("{path}[{key}={id}]"), (*bv).clone()));
        }
    }
}

// --- path rendering (dotpick-compatible) ---

fn emit_path(path: &str) -> String {
    if path.is_empty() {
        ".".to_string()
    } else {
        path.to_string()
    }
}

fn is_bareword(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn render_key(path: &str, key: &str) -> String {
    if is_bareword(key) {
        if path.is_empty() {
            key.to_string()
        } else {
            format!("{path}.{key}")
        }
    } else {
        format!("{path}[\"{}\"]", key.replace('"', "\\\""))
    }
}

fn render_index(path: &str, i: usize) -> String {
    format!("{path}[{i}]")
}

/// A scalar rendered as a compact locator: strings bare (`abc`), others as JSON.
/// `None` for non-scalars, which disqualifies them as an `--array-key`.
fn scalar_locator(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn diff_default(a: Value, b: Value) -> Vec<Change> {
        diff(&a, &b, &DiffOptions::default())
    }

    #[test]
    fn identical_values_produce_no_changes() {
        assert!(
            diff_default(json!({"a": 1, "b": [1, 2]}), json!({"a": 1, "b": [1, 2]})).is_empty()
        );
    }

    #[test]
    fn changed_scalar_reports_old_and_new_at_dotpath() {
        let c = diff_default(
            json!({"user": {"plan": "free"}}),
            json!({"user": {"plan": "pro"}}),
        );
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].op, Op::Changed);
        assert_eq!(c[0].path, "user.plan");
        assert_eq!(c[0].old, Some(json!("free")));
        assert_eq!(c[0].new, Some(json!("pro")));
    }

    #[test]
    fn added_and_removed_keys() {
        let c = diff_default(json!({"keep": 1, "gone": 2}), json!({"keep": 1, "new": 3}));
        let by_path: Vec<_> = c.iter().map(|x| (x.op, x.path.as_str())).collect();
        assert!(by_path.contains(&(Op::Removed, "gone")));
        assert!(by_path.contains(&(Op::Added, "new")));
        assert_eq!(c.len(), 2);
    }

    #[test]
    fn type_change_is_a_single_change_not_a_recursion() {
        let c = diff_default(json!({"x": {"deep": 1}}), json!({"x": [1, 2]}));
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].op, Op::Changed);
        assert_eq!(c[0].path, "x");
    }

    #[test]
    fn arrays_diff_by_index_by_default() {
        let c = diff_default(json!({"items": [1, 2, 3]}), json!({"items": [1, 9, 3, 4]}));
        let paths: Vec<_> = c.iter().map(|x| (x.op, x.path.as_str())).collect();
        assert!(paths.contains(&(Op::Changed, "items[1]")));
        assert!(paths.contains(&(Op::Added, "items[3]")));
        assert_eq!(c.len(), 2);
    }

    #[test]
    fn array_key_matches_objects_order_independently() {
        let a = json!({"items": [{"id": 1, "qty": 1}, {"id": 2, "qty": 5}]});
        let b = json!({"items": [{"id": 2, "qty": 5}, {"id": 1, "qty": 3}]});
        let opts = DiffOptions {
            array_key: Some("id".into()),
        };
        let c = diff(&a, &b, &opts);
        // Only id=1's qty changed; reorder alone is not a change.
        assert_eq!(c.len(), 1, "got {c:?}");
        assert_eq!(c[0].op, Op::Changed);
        assert_eq!(c[0].path, "items[id=1].qty");
        assert_eq!(c[0].new, Some(json!(3)));
    }

    #[test]
    fn array_key_reports_added_and_removed_elements() {
        let a = json!([{"id": 1}, {"id": 2}]);
        let b = json!([{"id": 2}, {"id": 3}]);
        let opts = DiffOptions {
            array_key: Some("id".into()),
        };
        let c = diff(&a, &b, &opts);
        let paths: Vec<_> = c.iter().map(|x| (x.op, x.path.as_str())).collect();
        assert!(paths.contains(&(Op::Removed, "[id=1]")));
        assert!(paths.contains(&(Op::Added, "[id=3]")));
        assert_eq!(c.len(), 2);
    }

    #[test]
    fn array_key_falls_back_to_index_when_key_missing() {
        // Second element lacks the key, so the whole array diffs by index.
        let a = json!([{"id": 1}, {"name": "x"}]);
        let b = json!([{"id": 1}, {"name": "y"}]);
        let opts = DiffOptions {
            array_key: Some("id".into()),
        };
        let c = diff(&a, &b, &opts);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].path, "[1].name");
    }

    #[test]
    fn quoted_keys_for_non_barewords() {
        let c = diff_default(json!({"a.b": 1}), json!({"a.b": 2}));
        assert_eq!(c[0].path, "[\"a.b\"]");
    }

    #[test]
    fn root_scalar_change_uses_dot() {
        let c = diff_default(json!(1), json!(2));
        assert_eq!(c[0].path, ".");
    }
}
