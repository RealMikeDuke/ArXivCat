//! Exit-code contract matrix (expert-review follow-up): lock the frozen
//! codes 2/3/5/8 against the real binary.
//!
//! Frozen table: 0 ok | 1 other | 2 usage | 3 network | 4 config | 5 data |
//! 6 io | 7 chat | 8 partial | 130 SIGINT.

use std::process::Command;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_arxivcat"))
}

fn tmp_ws(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("arxivcat_exit_{}_{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn invalid_arxiv_id_exits_2() {
    // "not-a-real-id" fails extract_arxiv_id -> usage error -> exit 2
    let out = bin()
        .args([
            "-w",
            tmp_ws("badid").to_str().unwrap(),
            "paper",
            "download",
            "not-a-real-id",
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "unparseable arXiv ID is usage");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["error"]["code"], 2);
    assert_eq!(v["error"]["kind"], "usage");
}

#[test]
fn paper_not_found_exits_5() {
    let ws = tmp_ws("nf");
    // Create the folder first — writing into a non-existent parent would
    // silently fail and the test would pass for the wrong reason.
    std::fs::create_dir_all(ws.join("2501_12948")).unwrap();
    std::fs::write(ws.join("2501_12948").join("body.tex"), "x").unwrap();
    let out = bin()
        .args([
            "-w",
            ws.to_str().unwrap(),
            "paper",
            "info",
            "9999.99999",
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(5), "not found is data error");
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["error"]["kind"], "not_found");
    assert_eq!(v["error"]["retryable"], false);
}

#[test]
fn ambiguous_query_exits_5_with_kind_ambiguous() {
    // Two papers sharing a prefix -> ambiguity error (not silent pick).
    let ws = tmp_ws("ambig");
    std::fs::create_dir_all(ws.join("2501_12948_A")).unwrap();
    std::fs::create_dir_all(ws.join("2501_12948_B")).unwrap();
    std::fs::write(ws.join("2501_12948_A").join("body.tex"), "x").unwrap();
    std::fs::write(ws.join("2501_12948_B").join("body.tex"), "x").unwrap();

    let out = bin()
        .args([
            "-w",
            ws.to_str().unwrap(),
            "paper",
            "info",
            "2501.1294",
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(5));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["error"]["kind"], "ambiguous");
}

#[test]
fn jobs_out_of_range_exits_2() {
    let out = bin()
        .args(["paper", "download-all", "--jobs", "99"])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(2),
        "--jobs out of 1..=8 is usage error"
    );
}

// ─── wiremock end-to-end: exit 3 (network) and 8 (partial) ───

fn gz_tar(entries: &[(&str, &[u8])]) -> Vec<u8> {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    let buf = std::io::Cursor::new(Vec::new());
    let mut enc = GzEncoder::new(buf, Compression::fast());
    {
        let mut builder = tar::Builder::new(&mut enc);
        for (name, content) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_path(name).unwrap();
            header.set_size(content.len() as u64);
            header.set_cksum();
            builder.append(&header, &content[..]).unwrap();
        }
        builder.finish().unwrap();
    }
    enc.finish().unwrap().into_inner()
}

#[tokio::test]
async fn download_network_error_exits_3() {
    let server = MockServer::start().await;
    // Persistent 429 -> retry exhaustion -> HttpStatus -> exit 3 / kind http
    Mock::given(method("GET"))
        .and(path("/src/9999.00001"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/abs/9999.00001"))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>ignored</html>"))
        .mount(&server)
        .await;

    let ws = tmp_ws("net");
    let appdata = std::env::temp_dir().join(format!("arxivcat_appdata_net_{}", std::process::id()));
    let out = bin()
        .env("APPDATA", &appdata)
        .env("ARXIVCAT_ARXIV_BASE_URL", server.uri())
        .args([
            "-w",
            ws.to_str().unwrap(),
            "paper",
            "download",
            "9999.00001",
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(3),
        "429 retry-exhaustion must be exit 3 (frozen contract)"
    );
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["error"]["kind"], "http");
    assert_eq!(v["error"]["retryable"], true);
}

#[tokio::test]
async fn download_all_partial_exits_8() {
    let server = MockServer::start().await;
    // One paper succeeds (valid tar), one fails (404 -> NotFound exit 5
    // internally), so the batch is PARTIAL -> exit 8.
    let tar = gz_tar(&[(
        "main.tex",
        b"\\documentclass{article}\n\\begin{document}\nHello world\n\\end{document}",
    )]);
    Mock::given(method("GET"))
        .and(path("/src/2501.10001"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(tar.clone()))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/src/9999.00002"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/pdf/2501.10001"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/pdf/9999.00002"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let ws = tmp_ws("partial");
    for d in ["2501_10001", "9999_00002"] {
        std::fs::create_dir_all(ws.join(d)).unwrap();
    }
    let appdata =
        std::env::temp_dir().join(format!("arxivcat_appdata_partial_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&appdata);

    let out = bin()
        .env("APPDATA", &appdata)
        .env("ARXIVCAT_ARXIV_BASE_URL", server.uri())
        .args([
            "-w",
            ws.to_str().unwrap(),
            "paper",
            "download-all",
            "--json",
        ])
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    eprintln!("STDERR: {}", String::from_utf8_lossy(&out.stderr));
    eprintln!("PAYLOAD: {v}");
    assert_eq!(v["total"], 2);
    assert_eq!(v["success"], 1);
    assert_eq!(v["failed"], 1);
    assert_eq!(
        out.status.code(),
        Some(8),
        "partial download-all must exit 8, payload {v}"
    );
}

// ---------------------------------------------------------------------------
// Jury-burst regression tests: the lock/gating holes found across R6-R12
// must stay closed. These fail if anyone re-opens them.
// ---------------------------------------------------------------------------

/// deep-summarize must refuse (exit 7, status:busy) while a worker holds
/// the deep lock — otherwise a foreground command would double-charge
/// (jury-burst R6: no-lock entry was a MAJOR).
#[cfg(unix)]
#[test]
fn deep_summarize_busy_exits_7() {
    use std::os::unix::io::AsRawFd;
    let ws = tmp_ws("busy");
    std::fs::create_dir_all(ws.join("2501_00001")).unwrap();

    // Hold .deep.lock like a running worker would (flock is kernel-scoped:
    // a second flock from another process fails).
    let lock_file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(ws.join("2501_00001/.deep.lock"))
        .unwrap();
    let rc = unsafe { libc::flock(lock_file.as_raw_fd(), libc::LOCK_EX) };
    assert_eq!(rc, 0, "test must hold the lock");

    let out = bin()
        .args([
            "-w",
            ws.to_str().unwrap(),
            "paper",
            "deep-summarize",
            "2501.00001",
            "--json",
        ])
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(out.status.code(), Some(7), "busy must exit 7, got {v}");
    assert_eq!(v["status"], "busy", "busy JSON contract, got {v}");
}

/// --force must NOT delete the current summary when it is refused (busy):
/// cleaning before acquiring would destroy the old artifact (jury-burst
/// R7/R10: cleanup moved AFTER the gate).
#[cfg(unix)]
#[test]
fn deep_summarize_force_busy_keeps_artifacts() {
    use std::os::unix::io::AsRawFd;
    let ws = tmp_ws("forcebusy");
    std::fs::create_dir_all(ws.join("2501_00002")).unwrap();
    std::fs::write(ws.join("2501_00002/deep_summary.md"), "old summary").unwrap();
    std::fs::write(ws.join("2501_00002/.deep_ready"), "ok\n").unwrap();

    let lock_file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(ws.join("2501_00002/.deep.lock"))
        .unwrap();
    let rc = unsafe { libc::flock(lock_file.as_raw_fd(), libc::LOCK_EX) };
    assert_eq!(rc, 0);

    let out = bin()
        .args([
            "-w",
            ws.to_str().unwrap(),
            "paper",
            "deep-summarize",
            "2501.00002",
            "--force",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(7), "busy must exit 7");
    assert_eq!(
        std::fs::read_to_string(ws.join("2501_00002/deep_summary.md")).unwrap(),
        "old summary",
        "--force refusal must not delete the current summary"
    );
    assert!(
        ws.join("2501_00002/.deep_ready").exists(),
        "--force refusal must not delete the ready marker"
    );
}

/// --no-describe must mean ZERO DeepSeek calls, even with deep default ON:
/// deep is only possible when a brief already exists, and generate_deep's
/// internal brief rebuild is unlocked (jury-burst R9/R10/R11: hardcoded
/// brief_ok=true was a MAJOR; batch path split semantics was a MAJOR).
#[tokio::test]
async fn no_describe_makes_zero_deepseek_calls() {
    let server = MockServer::start().await;
    let tar = gz_tar(&[(
        "main.tex",
        b"\\documentclass{article}\n\\begin{document}\nHello world\n\\end{document}",
    )]);
    Mock::given(method("GET"))
        .and(path("/src/2501.00003"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(tar))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/pdf/2501.00003"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let ds = MockServer::start().await;
    // Any chat/completions call would trip this expectation on drop.
    let _gate = Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&ds)
        .await;

    let ws = tmp_ws("nodescribe");
    let out = bin()
        .env("ARXIVCAT_ARXIV_BASE_URL", server.uri())
        .env("ARXIVCAT_DEEPSEEK_BASE_URL", ds.uri())
        .args([
            "-w",
            ws.to_str().unwrap(),
            "paper",
            "download",
            "2501.00003",
            "--no-describe",
            "--json",
        ])
        .output()
        .unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(out.status.code(), Some(0), "download must succeed, got {v}");
    let folder = ws.join("2501_00003");
    assert!(
        !folder.join(".deep_ready").exists(),
        "--no-describe with missing brief must skip deep"
    );
    assert!(
        !folder.join(".description_ready").exists(),
        "--no-describe must never mark a brief ready"
    );
    // expect(0) on the chat mock verifies zero API calls on drop.
}
