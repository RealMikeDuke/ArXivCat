//! CLI contract tests (P0.4): frozen exit-code table + stdout purity.
//!
//! Exit codes (do not renumber):
//!   0 success | 1 other | 2 usage | 3 network | 4 config | 5 data | 6 io | 7 chat | 8 partial | 130 SIGINT
//!
//! --json contract: stdout is ALWAYS a single JSON document (payload or
//! {"error":{code,kind,message,retryable}}); human text goes to stderr.

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_arxivcat"))
}

fn tmp_ws(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("arxivcat_contract_{}_{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn parse_json(s: &str) -> serde_json::Value {
    serde_json::from_str(s.trim())
        .unwrap_or_else(|e| panic!("stdout is not a single JSON document: {e}\nstdout={s:?}"))
}

#[test]
fn usage_error_exits_2() {
    let out = bin().arg("badcommand").output().unwrap();
    assert_eq!(out.status.code(), Some(2), "usage error must exit 2");
    assert!(
        out.stdout.is_empty(),
        "human mode: stdout must be empty on usage error"
    );
}

#[test]
fn usage_error_json_envelope() {
    let out = bin().args(["--json", "badcommand"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let v = parse_json(&String::from_utf8_lossy(&out.stdout));
    assert_eq!(v["error"]["code"], 2);
    assert_eq!(v["error"]["kind"], "usage");
    assert_eq!(v["error"]["retryable"], false);
}

#[test]
fn unknown_subcommand_json_envelope() {
    let out = bin().args(["--json", "paper", "nope"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let v = parse_json(&String::from_utf8_lossy(&out.stdout));
    assert_eq!(v["error"]["code"], 2);
}

#[test]
fn chat_json_rejected_as_usage() {
    let out = bin()
        .args([
            "-w",
            tmp_ws("chat").to_str().unwrap(),
            "chat",
            "side",
            "2501.12948",
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let v = parse_json(&String::from_utf8_lossy(&out.stdout));
    assert_eq!(v["error"]["kind"], "usage");
}

#[test]
fn missing_workspace_exits_config() {
    let out = bin()
        .args(["-w", "/__nonexistent_ws__", "paper", "list", "--json"])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(4),
        "missing workspace is a config error"
    );
    let v = parse_json(&String::from_utf8_lossy(&out.stdout));
    assert_eq!(v["error"]["code"], 4);
    assert_eq!(v["error"]["kind"], "config");
}

#[test]
fn help_exits_zero() {
    let out = bin().arg("--help").output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let out2 = bin().arg("--version").output().unwrap();
    assert_eq!(out2.status.code(), Some(0));
}

#[test]
fn paper_list_json_single_document() {
    let ws = tmp_ws("list");
    // one complete paper folder
    let pdir = ws.join("2501_12948_Test_Paper");
    std::fs::create_dir_all(&pdir).unwrap();
    std::fs::write(pdir.join("body.tex"), "\\documentclass{article}").unwrap();

    let out = bin()
        .args(["-w", ws.to_str().unwrap(), "paper", "list", "--json"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let v = parse_json(&String::from_utf8_lossy(&out.stdout));
    assert!(v.is_array(), "list --json must output an array, got {v}");
    assert_eq!(v[0]["arxiv_id"], "2501.12948");
    assert_eq!(
        v[0]["is_complete"], true,
        "is_complete = has_body (AI decoupled)"
    );
}

#[test]
fn paper_list_no_json_contract_output_ok() {
    let ws = tmp_ws("list2");
    let out = bin()
        .args(["-w", ws.to_str().unwrap(), "paper", "list"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert!(!out.stdout.is_empty());
}

#[test]
fn download_all_json_stdout_single_document() {
    // Empty pending list: JSON must be the only stdout content (no \r progress).
    let ws = tmp_ws("downloadall");
    let pdir = ws.join("2501_12948_Test_Paper");
    std::fs::create_dir_all(&pdir).unwrap();
    std::fs::write(pdir.join("body.tex"), "x").unwrap();

    let out = bin()
        .args([
            "-w",
            ws.to_str().unwrap(),
            "paper",
            "download-all",
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains('\r'),
        "no \\r progress in --json stdout: {stdout:?}"
    );
    let v = parse_json(&stdout);
    assert_eq!(v["status"], "done", "download-all --json emits status=done");
}
