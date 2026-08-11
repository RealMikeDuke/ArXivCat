use regex::Regex;

use crate::error::{ArxivError, Result};

pub fn extract_arxiv_id(input: &str) -> Option<String> {
    let re = Regex::new(r"(\d+[._]\d+(?:v\d+)?)").ok()?;
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

pub async fn fetch_title_from_arxiv(arxiv_id: &str) -> Result<Option<String>> {
    let url = format!("https://arxiv.org/abs/{arxiv_id}");
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await?;

    let html = response.text().await?;

    let re = Regex::new(r#"<meta property="og:title" content="([^"]+)""#)
        .map_err(|e| ArxivError::Other(e.to_string()))?;

    Ok(re.captures(&html).and_then(|c| c.get(1)).map(|m| {
        m.as_str().to_string()
    }))
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
        assert_eq!(result.chars().all(|c| c == '论'), true);
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
