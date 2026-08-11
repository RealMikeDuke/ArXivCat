use regex::Regex;

use crate::error::{ArxivError, Result};

pub fn extract_arxiv_id(input: &str) -> Option<String> {
    // New-style arXiv IDs only: YYMM.NNNNN (4+4/5 digits). Tightened from the
    // loose \d+[._]\d+ which mis-matched DOIs (10.48550) and dates. Old-style
    // IDs (hep-th/9901001) are intentionally unsupported (documented).
    let re = Regex::new(r"(\d{4}[._]\d{4,5}(?:v\d+)?)").ok()?;
    re.captures(input)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().replace('_', "."))
}

pub fn extract_arxiv_id_from_pdf(pdf_path: &std::path::Path) -> Result<Option<String>> {
    let bytes = std::fs::read(pdf_path).map_err(ArxivError::Io)?;

    let doc = lopdf::Document::load_mem(&bytes)
        .map_err(|e| ArxivError::Parse(format!("failed to parse PDF: {e}")))?;

    let bare_pattern = Regex::new(r"(\d{4}\.\d{4,5}(?:v\d+)?)")
        .map_err(|e| ArxivError::Other(e.to_string()))?;
    let arxiv_pattern =
        Regex::new(r"arXiv[:\s]*(\d{4}\.\d{4,5}(?:v\d+)?)")
            .map_err(|e| ArxivError::Other(e.to_string()))?;

    if let Ok(info_dict) = doc.trailer.get(b"Info") {
        if let Ok(info) = info_dict.as_dict() {
            for field in &["subject", "keywords", "title", "author"] {
                if let Ok(value) = info.get(field.as_bytes()) {
                    let text = object_to_string(value);
                    if let Some(id) = bare_pattern.captures(&text).and_then(|c| c.get(1)) {
                        return Ok(Some(id.as_str().to_string()));
                    }
                }
            }
        }
    }

    let pages = doc.get_pages();
    for (i, page_id) in pages.iter().enumerate() {
        if i >= 3 {
            break;
        }
        if let Ok(text) = doc.extract_text(&[*page_id.0]) {
            if let Some(id) = arxiv_pattern.captures(&text).and_then(|c| c.get(1)) {
                return Ok(Some(id.as_str().to_string()));
            }
            if let Some(id) = bare_pattern.captures(&text).and_then(|c| c.get(1)) {
                return Ok(Some(id.as_str().to_string()));
            }
        }
    }

    Ok(None)
}

fn object_to_string(value: &lopdf::Object) -> String {
    match value {
        lopdf::Object::String(s, _) => String::from_utf8_lossy(s).to_string(),
        lopdf::Object::Name(n) => String::from_utf8_lossy(n).to_string(),
        other => format!("{other:?}"),
    }
}

pub async fn fetch_title_from_arxiv(cfg: &crate::net::HttpConfig, arxiv_id: &str) -> Result<Option<String>> {
    let url = cfg.arxiv_abs_url(arxiv_id);
    let response = cfg.get_with_retry(&url).await?;

    let html = response.text().await?;

    let re = Regex::new(r#"<meta property="og:title" content="([^"]+)" "#)
        .map_err(|e| ArxivError::Other(e.to_string()))?;

    Ok(re.captures(&html).and_then(|c| c.get(1)).map(|m| {
        m.as_str().to_string()
    }))
}

/// Batch-fetch titles via the export API (`/api/query?id_list=`, Atom).
/// Best-effort: any failure returns an empty map (callers fall back to
/// empty titles); the download pipeline never blocks on titles (P0.7).
pub async fn fetch_titles_batch(
    cfg: &crate::net::HttpConfig,
    ids: &[String],
) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    if ids.is_empty() {
        return out;
    }
    // arXiv export API: 1 request per 3s rate limit; chunk conservatively.
    for chunk in ids.chunks(50) {
        let id_list = chunk.join(",");
        let url = format!("{}/api/query?id_list={}", cfg.arxiv_base, id_list);
        let response = match cfg.get_with_retry(&url).await {
            Ok(r) => r,
            Err(_) => continue,
        };
        let text = match response.text().await {
            Ok(t) => t,
            Err(_) => continue,
        };
        for (id, title) in parse_atom_entries(&text) {
            out.insert(id, title);
        }
        // Respect the 3s rate limit between export API calls.
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
    out
}

/// Parse `<entry><id>...abs/2501.12948</id><title>...</title></entry>` blocks.
/// Exposed for unit tests.
pub fn parse_atom_entries(xml: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let entry_re = Regex::new(r"(?s)<entry>.*?</entry>").unwrap();
    let id_re = Regex::new(r"(?s)<id>.*?/abs/(\d{4}\.\d{4,5}(?:v\d+)?)</id>").unwrap();
    let title_re = Regex::new(r"(?s)<title>(.*?)</title>").unwrap();
    for entry in entry_re.find_iter(xml) {
        let block = entry.as_str();
        let id = id_re.captures(block).and_then(|c| c.get(1)).map(|m| m.as_str().to_string());
        let title = title_re.captures(block).and_then(|c| c.get(1)).map(|m| {
            // Collapse whitespace runs (Atom titles often span indented lines).
            Regex::new(r"\s+")
                .unwrap()
                .replace_all(m.as_str().trim(), " ")
                .to_string()
        });
        if let (Some(id), Some(title)) = (id, title) {
            out.push((id, title));
        }
    }
    out
}

pub fn sanitize_filename(name: &str) -> String {
    let filtered: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_ascii_control() => '_',
            other => other,
        })
        .collect();

    let collapsed = Regex::new(r"[\s_]+")
        .unwrap()
        .replace_all(&filtered, "_")
        .to_string();

    let trimmed = collapsed
        .trim_matches(|c: char| c == '_' || c == '.' || c == ' ' || c == '-')
        .to_string();

    if trimmed.is_empty() {
        return "untitled".to_string();
    }

    if trimmed.chars().count() > 80 {
        trimmed.chars().take(80).collect()
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_arxiv_id_url() {
        assert_eq!(
            extract_arxiv_id("https://arxiv.org/abs/2501.12948"),
            Some("2501.12948".to_string())
        );
    }

    #[test]
    fn test_extract_arxiv_id_with_version() {
        assert_eq!(
            extract_arxiv_id("2501.12948v2"),
            Some("2501.12948v2".to_string())
        );
    }

    #[test]
    fn test_extract_arxiv_id_with_underscore() {
        assert_eq!(
            extract_arxiv_id("2501_12948v2"),
            Some("2501.12948v2".to_string())
        );
    }

    #[test]
    fn test_extract_arxiv_id_raw() {
        assert_eq!(extract_arxiv_id("2501.12948"), Some("2501.12948".to_string()));
    }

    #[test]
    fn test_extract_arxiv_id_in_text() {
        assert_eq!(
            extract_arxiv_id("see paper 2501.12948 for details"),
            Some("2501.12948".to_string())
        );
    }

    #[test]
    fn test_extract_arxiv_id_invalid_returns_none() {
        assert_eq!(extract_arxiv_id("hello world"), None);
    }

    #[test]
    fn test_extract_arxiv_id_rejects_doi_prefix() {
        // Regression (P0.7): loose regex matched the DOI part ("10.48550").
        // The tightened regex must still find the embedded arXiv ID.
        assert_eq!(
            extract_arxiv_id("https://doi.org/10.48550/arXiv.2501.12948"),
            Some("2501.12948".to_string())
        );
        // A bare DOI-looking string with a short prefix must NOT match.
        assert_eq!(extract_arxiv_id("10.48550/arXiv"), None);
    }

    #[test]
    fn test_extract_arxiv_id_rejects_old_style() {
        // Old-style IDs (hep-th/9901001) are documented as unsupported.
        assert_eq!(extract_arxiv_id("hep-th/9901001"), None);
    }

    #[test]
    fn test_parse_atom_entries_multiple() {
        let xml = r#"<?xml version="1.0"?>
        <feed>
          <entry>
            <id>http://arxiv.org/abs/2501.12948v2</id>
            <title>First   Paper</title>
          </entry>
          <entry>
            <id>http://arxiv.org/abs/2412.04445</id>
            <title>Second Paper</title>
          </entry>
        </feed>"#;
        let entries = parse_atom_entries(xml);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0], ("2501.12948v2".to_string(), "First Paper".to_string()));
        assert_eq!(entries[1], ("2412.04445".to_string(), "Second Paper".to_string()));
    }

    #[test]
    fn test_parse_atom_entries_empty() {
        assert!(parse_atom_entries("<feed></feed>").is_empty());
    }

    #[test]
    fn test_sanitize_filename_simple() {
        assert_eq!(sanitize_filename("Hello World"), "Hello_World");
    }

    #[test]
    fn test_sanitize_filename_illegal_chars() {
        assert_eq!(
            sanitize_filename("test:file<name>.txt"),
            "test_file_name_.txt"
        );
    }

    #[test]
    fn test_sanitize_filename_long() {
        let long = "a".repeat(100);
        let result = sanitize_filename(&long);
        assert!(result.len() <= 80);
    }

    #[test]
    fn test_sanitize_filename_multibyte_no_panic() {
        // Regression: byte-slice truncation [..80] panicked on multibyte titles.
        // 100 CJK chars = 300 bytes; must truncate at char boundary, no panic.
        let long = "论".repeat(100);
        let result = sanitize_filename(&long);
        assert_eq!(result.chars().count(), 80);
        assert!(result.chars().all(|c| c == '论'));
    }

    #[test]
    fn test_sanitize_filename_boundary_cjk() {
        // 79 ASCII bytes + 1 CJK char (3 bytes) = 82 bytes; truncation must
        // not split the CJK char.
        let mixed = format!("{}论", "a".repeat(79));
        let result = sanitize_filename(&mixed);
        assert_eq!(result.chars().count(), 80);
        assert!(result.ends_with('论'));
    }
}
