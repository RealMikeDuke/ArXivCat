//! HTTP retry/backoff contract tests (P1.6) — driven with wiremock so they
//! never touch the real arXiv.

use arxivcat_core::net::HttpConfig;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// HttpConfig pointed at the mock server with a tiny backoff for test speed.
async fn test_cfg(server: &MockServer) -> HttpConfig {
    let mut cfg = HttpConfig::new().unwrap();
    cfg.arxiv_base = server.uri();
    cfg.backoff_base_ms = 1;
    cfg.retry_after_cap_ms = 50;
    cfg.max_retries = 3;
    cfg
}

#[tokio::test]
async fn retries_429_then_succeeds() {
    let server = MockServer::start().await;
    // First request: 429 (rate limited); then success.
    Mock::given(method("GET"))
        .and(path("/src/2501.12948"))
        .respond_with(ResponseTemplate::new(429).set_body_string("slow down"))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/src/2501.12948"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"tar-bytes"))
        .mount(&server)
        .await;

    let cfg = test_cfg(&server).await;
    let resp = cfg
        .get_with_retry(&format!("{}/src/2501.12948", server.uri()))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "429 must be retried with backoff");
    let body = resp.bytes().await.unwrap();
    assert_eq!(&body[..], b"tar-bytes");
}

#[tokio::test]
async fn retries_5xx_then_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/abs/2501.12948"))
        .respond_with(ResponseTemplate::new(500))
        .up_to_n_times(2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/abs/2501.12948"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("<meta property=\"og:title\" content=\"Mock Title\" />"),
        )
        .mount(&server)
        .await;

    let cfg = test_cfg(&server).await;
    let title = arxivcat_core::extract::arxiv::fetch_title_from_arxiv(&cfg, "2501.12948")
        .await
        .unwrap();
    assert_eq!(title.as_deref(), Some("Mock Title"));
}

#[tokio::test]
async fn gives_up_after_max_retries_on_persistent_429() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/src/9999.00000"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&server)
        .await;

    let cfg = test_cfg(&server).await;
    let err = cfg
        .get_with_retry(&format!("{}/src/9999.00000", server.uri()))
        .await
        .expect_err("persistent 429 must surface as an error after retry exhaustion");
    match err {
        arxivcat_core::error::ArxivError::HttpStatus(429) => {}
        other => panic!("expected HttpStatus(429), got {other:?}"),
    }
}

#[tokio::test]
async fn respects_retry_after_header() {
    let server = MockServer::start().await;
    let mut tmpl = ResponseTemplate::new(429);
    tmpl = tmpl.append_header("retry-after", "0"); // 0s: exercise the path quickly
    Mock::given(method("GET"))
        .and(path("/src/2501.12948"))
        .respond_with(tmpl)
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/src/2501.12948"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let cfg = test_cfg(&server).await;
    let resp = cfg
        .get_with_retry(&format!("{}/src/2501.12948", server.uri()))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn busy_lock_is_waited_not_failed() {
    // P2-3 (jury-ask A): a transient lock collision must WAIT for the
    // holder instead of failing immediately (which would arm a 24h
    // cooldown for a paper that is simply being downloaded elsewhere).
    let server = MockServer::start().await;
    let tar_bytes = {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        let buf = std::io::Cursor::new(Vec::new());
        let mut enc = GzEncoder::new(buf, Compression::fast());
        {
            let mut builder = tar::Builder::new(&mut enc);
            let mut header = tar::Header::new_gnu();
            header.set_path("main.tex").unwrap();
            header.set_size(20);
            header.set_cksum();
            builder
                .append(&header, &b"\\documentclass{article}"[..])
                .unwrap();
            builder.finish().unwrap();
        }
        enc.finish().unwrap().into_inner()
    };
    Mock::given(method("GET"))
        .and(path("/src/2501.20001"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(tar_bytes))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/pdf/2501.20001"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let cfg = test_cfg(&server).await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".locks")).unwrap();
    // A fresh (non-stale) lock held by "another process".
    let lock_path = dir.path().join(".locks").join("2501_20001.lock");
    std::fs::write(&lock_path, "0").unwrap();

    // Release the lock after 1s, as a real winner would.
    let lock_path2 = lock_path.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let _ = std::fs::remove_file(&lock_path2);
    });

    let out = arxivcat_core::extract::source::download_source(&cfg, "2501.20001", dir.path())
        .await
        .expect("must succeed after waiting for the busy lock");
    assert!(out.0.is_some(), "paper must be downloaded");
    assert!(dir.path().join("2501_20001").join("main.tex").exists());
}

#[tokio::test]
async fn versioned_download_writes_base_id_pdf() {
    // 2501.12948v2 must land as 2501.12948.pdf so the manifest scan
    // ({base_id}.pdf) and the on-disk file agree (expert review C).
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/pdf/2501.12948v2"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"%PDF-1.4 fake"))
        .mount(&server)
        .await;

    let cfg = test_cfg(&server).await;
    let dir = tempfile::tempdir().unwrap();
    let out = arxivcat_core::extract::source::download_pdf(&cfg, "2501.12948v2", dir.path())
        .await
        .unwrap();
    let path = out.expect("pdf downloaded");
    assert_eq!(
        path.file_name().unwrap().to_str().unwrap(),
        "2501.12948.pdf",
        "versioned input must write the base-id filename"
    );
    assert!(dir.path().join("2501.12948.pdf").exists());
    assert!(!dir.path().join("2501.12948v2.pdf").exists());
}

#[tokio::test]
async fn batch_titles_normalize_versioned_ids() {
    // The export API always returns VERSIONED <id>s; keys must be base ids
    // so bare "2501.12948" lookups hit (jury-review MAJOR #1 regression).
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/query"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <entry>
    <id>http://arxiv.org/abs/2501.12948v2</id>
    <title>DeepSeek-R1 Incentivizing Reasoning Capabilities in LLMs</title>
  </entry>
</feed>"#,
        ))
        .mount(&server)
        .await;

    let cfg = test_cfg(&server).await;
    let map =
        arxivcat_core::extract::arxiv::fetch_titles_batch(&cfg, &["2501.12948".to_string()]).await;
    assert_eq!(
        map.get("2501.12948").map(|s| s.as_str()),
        Some("DeepSeek-R1 Incentivizing Reasoning Capabilities in LLMs"),
        "versioned export id must resolve under the bare base id"
    );
    // And a versioned lookup (what cmd_download does for 2501.12948v2 input)
    // must hit the SAME normalized key after stripping (jury-burst R2).
    let v = arxivcat_core::manifest::strip_version("2501.12948v2");
    assert_eq!(
        map.get(&v).map(|s| s.as_str()),
        Some("DeepSeek-R1 Incentivizing Reasoning Capabilities in LLMs"),
        "versioned caller key must hit after strip_version"
    );
}

#[tokio::test]
async fn title_failure_never_blocks_download_folder() {
    // A 500 on the abs page must not prevent download_source from returning
    // an ID-only folder decision (P0.7/P1.3 contract).
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/abs/2501.12948"))
        .respond_with(ResponseTemplate::new(500))
        .up_to_n_times(10)
        .mount(&server)
        .await;
    // Source tarball: a real gzipped tar with one file.
    let tar_bytes = {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        let buf = std::io::Cursor::new(Vec::new());
        let mut enc = GzEncoder::new(buf, Compression::fast());
        {
            let mut builder = tar::Builder::new(&mut enc);
            let mut header = tar::Header::new_gnu();
            header.set_path("main.tex").unwrap();
            header.set_size(20);
            header.set_cksum();
            builder
                .append(&header, &b"\\documentclass{article}"[..])
                .unwrap();
            builder.finish().unwrap();
        }
        enc.finish().unwrap().into_inner()
    };
    Mock::given(method("GET"))
        .and(path("/src/2501.12948"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(tar_bytes))
        .mount(&server)
        .await;

    let cfg = test_cfg(&server).await;
    let downloads = tempfile::tempdir().unwrap();
    let (dir_opt, folder_name) =
        arxivcat_core::extract::source::download_source(&cfg, "2501.12948", downloads.path())
            .await
            .unwrap();
    assert!(dir_opt.is_some());
    assert_eq!(
        folder_name.as_deref(),
        Some("2501_12948"),
        "title failure -> ID-only folder (no 'unknown')"
    );
    let main = dir_opt.unwrap().join("main.tex");
    assert!(main.exists());
}
