//! End-to-end tests of the compiled binary: the clispec contract, diffing real
//! files across formats, the exit-code contract, and the error envelope.

use std::path::PathBuf;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_dotdiff");

struct Output {
    code: i32,
    stdout: String,
    stderr: String,
}

fn run(args: &[&str]) -> Output {
    let out = Command::new(BIN).args(args).output().expect("spawn binary");
    Output {
        code: out.status.code().unwrap(),
        stdout: String::from_utf8(out.stdout).unwrap(),
        stderr: String::from_utf8(out.stderr).unwrap(),
    }
}

/// Write a uniquely named temp file and return its path.
fn tmp(name: &str, contents: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("dotdiff-{}-{}", std::process::id(), name));
    std::fs::write(&path, contents).unwrap();
    path
}

fn error_envelope(stderr: &str) -> serde_json::Value {
    let last = stderr.lines().last().expect("stderr has an error line");
    serde_json::from_str::<serde_json::Value>(last).expect("error envelope is JSON")["error"]
        .clone()
}

#[test]
fn schema_is_clispec_v0_2() {
    let out = run(&["schema"]);
    assert_eq!(out.code, 0);
    let v: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(v["clispec"], "0.2");
    // The diff/grep exit-1 contract is declared as an outcome, not an error.
    assert_eq!(v["outcomes"][0]["code"], 1);
    assert_eq!(v["outcomes"][0]["name"], "differences_found");
}

#[test]
fn help_mentions_schema() {
    let out = run(&["--help"]);
    assert_eq!(out.code, 0);
    assert!(out.stdout.contains("schema"));
}

#[test]
fn differences_exit_1_with_change_list() {
    let a = tmp("a1.json", r#"{"plan": "free", "seats": 1}"#);
    let b = tmp("b1.json", r#"{"plan": "pro", "seats": 1}"#);
    let out = run(&[a.to_str().unwrap(), b.to_str().unwrap()]);
    assert_eq!(out.code, 1, "differences -> exit 1; stderr: {}", out.stderr);
    let v: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(v["identical"], false);
    let change = &v["changes"][0];
    assert_eq!(change["op"], "changed");
    assert_eq!(change["path"], "plan");
    assert_eq!(change["old"], "free");
    assert_eq!(change["new"], "pro");
}

#[test]
fn identical_inputs_exit_0() {
    let a = tmp("a2.json", r#"{"a": 1, "b": [1, 2]}"#);
    let b = tmp("b2.json", r#"{"a": 1, "b": [1, 2]}"#);
    let out = run(&[a.to_str().unwrap(), b.to_str().unwrap()]);
    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
    let v: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(v["identical"], true);
}

#[test]
fn cross_format_same_data_is_identical() {
    let a = tmp("a3.yaml", "a: 1\nb:\n  - 2\n  - 3\n");
    let b = tmp("b3.json", r#"{"a": 1, "b": [2, 3]}"#);
    let out = run(&[a.to_str().unwrap(), b.to_str().unwrap()]);
    assert_eq!(out.code, 0, "stderr: {}", out.stderr);
}

#[test]
fn array_key_matches_objects_order_independently() {
    let a = tmp(
        "a4.json",
        r#"{"items": [{"id": 1, "q": 1}, {"id": 2, "q": 5}]}"#,
    );
    let b = tmp(
        "b4.json",
        r#"{"items": [{"id": 2, "q": 5}, {"id": 1, "q": 9}]}"#,
    );
    let out = run(&[
        a.to_str().unwrap(),
        b.to_str().unwrap(),
        "--array-key",
        "id",
    ]);
    assert_eq!(out.code, 1, "stderr: {}", out.stderr);
    let v: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
    let changes = v["changes"].as_array().unwrap();
    assert_eq!(
        changes.len(),
        1,
        "reorder alone is not a change: {changes:?}"
    );
    assert_eq!(changes[0]["path"], "items[id=1].q");
}

#[test]
fn text_output_uses_sigils() {
    let a = tmp("a5.json", r#"{"x": 1}"#);
    let b = tmp("b5.json", r#"{"x": 2}"#);
    let out = run(&[a.to_str().unwrap(), b.to_str().unwrap(), "-o", "text"]);
    assert_eq!(out.code, 1);
    assert_eq!(out.stdout.trim(), "~ x  1 -> 2");
}

#[test]
fn stdin_as_left_input() {
    use std::process::Stdio;
    let b = tmp("b6.json", r#"{"x": 2}"#);
    let mut child = Command::new(BIN)
        .args(["-", b.to_str().unwrap(), "--format", "json", "-o", "text"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(br#"{"x": 1}"#)
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.code().unwrap(), 1);
    assert_eq!(String::from_utf8(out.stdout).unwrap().trim(), "~ x  1 -> 2");
}

#[test]
fn missing_inputs_exit_3() {
    let out = run(&[]);
    assert_eq!(out.code, 3);
    assert_eq!(error_envelope(&out.stderr)["kind"], "usage");
}

#[test]
fn unparseable_input_exits_2() {
    let a = tmp("a7.json", "{ not valid json");
    let b = tmp("b7.json", r#"{"x": 1}"#);
    let out = run(&[a.to_str().unwrap(), b.to_str().unwrap()]);
    assert_eq!(out.code, 2);
    assert_eq!(error_envelope(&out.stderr)["kind"], "parse");
}

#[test]
fn missing_file_exits_2_io() {
    let b = tmp("b8.json", r#"{"x": 1}"#);
    let out = run(&["/no/such/dotdiff/file.json", b.to_str().unwrap()]);
    assert_eq!(out.code, 2);
    assert_eq!(error_envelope(&out.stderr)["kind"], "io");
}
