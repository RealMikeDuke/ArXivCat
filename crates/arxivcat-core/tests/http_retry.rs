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
    let resp = cfg
        .get_with_retry(&format!("{}/src/9999.00000", server.uri()))
        .await
        .unwrap();
    assert_eq!(resp.status(), 429, "must stop retrying and surface the 429");
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
