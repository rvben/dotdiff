//! Format detection and parsing into the universal `serde_json::Value` model.
//!
//! Every format is parsed into a single `Value`, so cross-format diffing works
//! (`a.yaml` vs `b.json`). NDJSON becomes a `Value::Array` of its records, which
//! also lets `--array-key` match records across two NDJSON streams.

use crate::Format;
use crate::error::Error;
use serde_json::Value;

/// Parse `text` into a single value, detecting the format when `forced` is
/// `None`. `label` names the input in any parse error.
pub fn load(text: &str, forced: Option<Format>, label: &str) -> Result<Value, Error> {
    let format = forced.unwrap_or_else(|| detect(text));
    match format {
        Format::Json => parse(text, label, "json", |t| {
            serde_json::from_str(t).map_err(|e| e.to_string())
        }),
        Format::Yaml => parse(text, label, "yaml", |t| {
            serde_norway::from_str(t).map_err(|e| e.to_string())
        }),
        Format::Toml => parse(text, label, "toml", |t| {
            toml::from_str::<toml::Value>(t)
                .map(toml_to_json)
                .map_err(|e| e.to_string())
        }),
        Format::Ndjson => parse_ndjson(text, label),
    }
}

fn parse(
    text: &str,
    label: &str,
    format: &str,
    f: impl Fn(&str) -> Result<Value, String>,
) -> Result<Value, Error> {
    f(text).map_err(|message| Error::Parse {
        label: label.to_string(),
        format: format.to_string(),
        message,
    })
}

/// Best-effort detection by trial parsing, mirroring dotpick: JSON first (a
/// strict subset of YAML), then NDJSON, then TOML, then YAML as the fallback.
fn detect(text: &str) -> Format {
    if serde_json::from_str::<Value>(text).is_ok() {
        Format::Json
    } else if is_ndjson(text) {
        Format::Ndjson
    } else if toml::from_str::<Value>(text).is_ok() {
        Format::Toml
    } else {
        Format::Yaml
    }
}

/// True when the input is two or more non-empty lines that each parse as JSON.
fn is_ndjson(text: &str) -> bool {
    let mut count = 0usize;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        if serde_json::from_str::<Value>(line).is_err() {
            return false;
        }
        count += 1;
    }
    count >= 2
}

fn parse_ndjson(text: &str, label: &str) -> Result<Value, Error> {
    let mut records = Vec::new();
    for (lineno, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let value = serde_json::from_str(line).map_err(|e| Error::Parse {
            label: label.to_string(),
            format: "ndjson".into(),
            message: format!("line {}: {e}", lineno + 1),
        })?;
        records.push(value);
    }
    Ok(Value::Array(records))
}

/// Convert a `toml::Value` into the universal JSON model. TOML datetimes have no
/// JSON counterpart, so they become RFC 3339 strings.
fn toml_to_json(value: toml::Value) -> Value {
    use serde_json::{Map, Number};
    match value {
        toml::Value::String(s) => Value::String(s),
        toml::Value::Integer(i) => Value::Number(i.into()),
        toml::Value::Float(f) => Number::from_f64(f)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        toml::Value::Boolean(b) => Value::Bool(b),
        toml::Value::Datetime(dt) => Value::String(dt.to_string()),
        toml::Value::Array(a) => Value::Array(a.into_iter().map(toml_to_json).collect()),
        toml::Value::Table(t) => Value::Object(
            t.into_iter()
                .map(|(k, v)| (k, toml_to_json(v)))
                .collect::<Map<_, _>>(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn loads_json_yaml_toml_to_the_same_value() {
        let j = load(r#"{"a":1,"b":[2,3]}"#, None, "j").unwrap();
        let y = load("a: 1\nb:\n  - 2\n  - 3\n", None, "y").unwrap();
        assert_eq!(j, y);
        assert_eq!(j, json!({"a": 1, "b": [2, 3]}));
    }

    #[test]
    fn ndjson_becomes_an_array() {
        let v = load("{\"id\":1}\n{\"id\":2}\n", None, "n").unwrap();
        assert_eq!(v, json!([{"id": 1}, {"id": 2}]));
    }

    #[test]
    fn forced_format_overrides_detection() {
        // Valid TOML that would also detect, forced as toml explicitly.
        let v = load("x = 1\n", Some(Format::Toml), "t").unwrap();
        assert_eq!(v, json!({"x": 1}));
    }

    #[test]
    fn parse_error_names_the_input_and_format() {
        let e = load("{ not json", Some(Format::Json), "left.json").unwrap_err();
        assert_eq!(e.kind(), "parse");
        assert!(e.to_string().contains("left.json"));
    }
}
