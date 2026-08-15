//! Two-round brief+deep generation tests (wiremock, no real API calls).
//!
//! Round 1 must write `brief_summary.md` + `.description_ready`; round 2 must
//! continue the SAME conversation (system + user1 + assistant(brief) + user2)
//! — assert the request shape — write `deep_summary.md` with the raw tables
//! appended verbatim, and set `.deep_ready`.

use std::sync::atomic::{AtomicUsize, Ordering};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer};

use arxivcat_core::chat::summary::{generate_brief, generate_deep};
use arxivcat_core::net::HttpConfig;

fn write_paper(paper_dir: &std::path::Path) {
    std::fs::create_dir_all(paper_dir).unwrap();
    std::fs::write(
        paper_dir.join("body.tex"),
        "\\documentclass{article}\n\\begin{document}\nThe result is 5.0ms.\n\\begin{tabular}{lc}\nA & 80.4 \\\\\nB & 0.72 \\\\\n\\end{tabular}\n\\end{document}\n",
    )
    .unwrap();
}

fn test_cfg(server: &MockServer) -> HttpConfig {
    let mut cfg = HttpConfig::new().unwrap();
    cfg.deepseek_base = server.uri();
    cfg
}

#[tokio::test]
async fn brief_then_deep_uses_two_rounds_and_appends_tables() {
    let server = MockServer::start().await;
    let calls = std::sync::Arc::new(AtomicUsize::new(0));

    // Two chat completions: brief (round 1) then deep (round 2).
    let c1 = calls.clone();
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(move |req: &wiremock::Request| {
            use wiremock::ResponseTemplate;
            let n = c1.fetch_add(1, Ordering::SeqCst);
            let body: serde_json::Value = serde_json::from_slice(&req.body).unwrap();
            let msgs = body["messages"].as_array().unwrap();
            if n == 0 {
                // Round 1: system + user only.
                assert_eq!(msgs.len(), 2);
                assert_eq!(msgs[0]["role"], "system");
                assert_eq!(msgs[1]["role"], "user");
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "choices": [{"message": {"role": "assistant", "content": "BRIEF OUTPUT"}}]
                }))
            } else {
                // Round 2: system + user + assistant(brief) + user(deep).
                assert_eq!(msgs.len(), 4);
                assert_eq!(msgs[0]["role"], "system");
                assert_eq!(msgs[1]["role"], "user");
                assert_eq!(msgs[2]["role"], "assistant");
                assert_eq!(msgs[2]["content"], "BRIEF OUTPUT");
                assert_eq!(msgs[3]["role"], "user");
                // Deep instruction is in round 2's user message.
                assert!(msgs[3]["content"]
                    .as_str()
                    .unwrap()
                    .contains("DEEP technical recap"));
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "choices": [{"message": {"role": "assistant", "content": "DEEP OUTPUT"}}]
                }))
            }
        })
        .expect(2)
        .mount(&server)
        .await;

    let cfg = test_cfg(&server);
    let dir = tempfile::tempdir().unwrap();
    write_paper(dir.path());

    // Set a fake token: load_cached_token reads config; HttpConfig::new_for_test
    // is not enough — write config with a token via the config helpers.
    let cfg_dir = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("APPDATA", cfg_dir.path()) };
    arxivcat_core::config::save_token("sk-test").unwrap();

    let brief = generate_brief(&cfg, dir.path(), "2501.99999", "Test Paper")
        .await
        .expect("brief must succeed");
    assert_eq!(brief, "BRIEF OUTPUT");
    assert!(dir.path().join("brief_summary.md").exists());
    assert!(dir.path().join(".description_ready").exists());

    generate_deep(&cfg, dir.path(), "2501.99999", "Test Paper")
        .await
        .expect("deep must succeed");

    let deep = std::fs::read_to_string(dir.path().join("deep_summary.md")).unwrap();
    assert!(deep.contains("DEEP OUTPUT"));
    // Raw table appended verbatim (deterministic copy, never through the LLM).
    assert!(deep.contains("\\begin{tabular}{lc}"));
    assert!(deep.contains("A & 80.4"));
    assert!(dir.path().join(".deep_ready").exists());

    assert_eq!(calls.load(Ordering::SeqCst), 2, "exactly two rounds");
}

#[tokio::test]
async fn deep_rebuilds_missing_brief_first() {
    let server = MockServer::start().await;
    let calls = std::sync::Arc::new(AtomicUsize::new(0));
    let c1 = calls.clone();
    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(move |req: &wiremock::Request| {
            use wiremock::ResponseTemplate;
            let n = c1.fetch_add(1, Ordering::SeqCst);
            let body: serde_json::Value = serde_json::from_slice(&req.body).unwrap();
            let msgs = body["messages"].as_array().unwrap();
            if n == 0 {
                assert_eq!(msgs.len(), 2, "round 1 = brief");
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "choices": [{"message": {"role": "assistant", "content": "B1"}}]
                }))
            } else {
                assert_eq!(msgs.len(), 4, "round 2 continues the conversation");
                assert_eq!(msgs[2]["content"], "B1");
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "choices": [{"message": {"role": "assistant", "content": "D1"}}]
                }))
            }
        })
        .expect(2)
        .mount(&server)
        .await;

    let cfg = test_cfg(&server);
    let dir = tempfile::tempdir().unwrap();
    write_paper(dir.path());
    let cfg_dir = tempfile::tempdir().unwrap();
    unsafe { std::env::set_var("APPDATA", cfg_dir.path()) };
    arxivcat_core::config::save_token("sk-test").unwrap();

    // No brief on disk — deep must produce it first, then deep.
    generate_deep(&cfg, dir.path(), "2501.99999", "Test Paper")
        .await
        .expect("deep must succeed");
    assert!(dir.path().join("brief_summary.md").exists());
    assert!(dir.path().join("deep_summary.md").exists());
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}
