//! Rendering the change list as text (TTY) or JSON (piped).
//!
//! Text uses `~`/`+`/`-` sigils and is empty when the inputs are identical (the
//! `diff` convention). JSON is `{identical, changes:[{op,path,old,new}]}`.

use crate::OutputFormat;
use crate::diff::{Change, Op};
use serde_json::{Value, json};

/// Render the change list. Empty text means identical; JSON always emits the
/// envelope.
pub fn render(changes: &[Change], format: OutputFormat) -> String {
    match format {
        OutputFormat::Json => json!({
            "identical": changes.is_empty(),
            "changes": changes,
        })
        .to_string(),
        OutputFormat::Text => changes
            .iter()
            .map(render_line)
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

fn render_line(c: &Change) -> String {
    match c.op {
        Op::Changed => format!("~ {}  {} -> {}", c.path, val(&c.old), val(&c.new)),
        Op::Removed => format!("- {}  {}", c.path, val(&c.old)),
        Op::Added => format!("+ {}  {}", c.path, val(&c.new)),
    }
}

fn val(v: &Option<Value>) -> String {
    v.as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_default())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn changed() -> Vec<Change> {
        let a = json!({"plan": "free", "trial": "x", "items": [1]});
        let b = json!({"plan": "pro", "seats": 5, "items": [1, 2]});
        crate::diff::diff(&a, &b, &crate::diff::DiffOptions::default())
    }

    #[test]
    fn json_carries_identical_flag_and_changes() {
        let out = render(&[], OutputFormat::Json);
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["identical"], json!(true));
        assert_eq!(v["changes"], json!([]));
    }

    #[test]
    fn json_change_has_op_path_old_new() {
        let out = render(&changed(), OutputFormat::Json);
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["identical"], json!(false));
        let change = v["changes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["path"] == "plan")
            .unwrap();
        assert_eq!(change["op"], "changed");
        assert_eq!(change["old"], "free");
        assert_eq!(change["new"], "pro");
    }

    #[test]
    fn added_change_omits_old_field() {
        let out = render(&changed(), OutputFormat::Json);
        let v: Value = serde_json::from_str(&out).unwrap();
        let added = v["changes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["path"] == "seats")
            .unwrap();
        assert_eq!(added["op"], "added");
        assert!(added.get("old").is_none(), "added has no old value");
        assert_eq!(added["new"], json!(5));
    }

    #[test]
    fn text_uses_sigils_and_is_empty_when_identical() {
        assert_eq!(render(&[], OutputFormat::Text), "");
        let text = render(&changed(), OutputFormat::Text);
        assert!(text.contains("~ plan  \"free\" -> \"pro\""), "got:\n{text}");
        assert!(text.lines().any(|l| l.starts_with("+ seats")));
    }
}
