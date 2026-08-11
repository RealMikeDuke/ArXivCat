//! paper.json manifest — the single source of truth for a paper folder (P1.1).
//!
//! Precedence: manifest (canonical) > legacy folder-name parsing (read-only
//! fallback). Every write-path command (download, scan, describe, note)
//! refreshes the manifest, so legacy folders get migrated lazily.

use std::path::Path;

use crate::error::{ArxivError, Result};

pub const MANIFEST_FILENAME: &str = "paper.json";

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ManifestFiles {
    /// Relative paths from the paper dir, when present.
    pub body: Option<String>,
    pub appendix: Option<String>,
    pub note: Option<String>,
    pub pdf: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PaperManifest {
    pub schema: u32,
    /// Canonical arXiv ID WITH version suffix when known (2501.12948v2).
    pub arxiv_id: String,
    /// ID without version (2501.12948) — this is the canonical folder name.
    pub base_id: String,
    pub title: String,
    /// ISO-8601 download timestamp; empty for legacy folders.
    pub downloaded_at: String,
    pub files: ManifestFiles,
    pub description_ready: bool,
    /// Last failure reason (for the 24h cooldown / --force flow, P1.4).
    #[serde(default)]
    pub last_error: Option<String>,
    /// Unix ms before which a retry is refused (0 = no cooldown).
    #[serde(default)]
    pub cooldown_until_ms: u64,
}

impl PaperManifest {
    pub fn load(paper_dir: &Path) -> Result<Option<Self>> {
        let path = paper_dir.join(MANIFEST_FILENAME);
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path).map_err(ArxivError::Io)?;
        let m: PaperManifest = serde_json::from_str(&content).map_err(|e| {
            ArxivError::Parse(format!("malformed manifest {}: {e}", path.display()))
        })?;
        Ok(Some(m))
    }

    /// Atomic write (temp + rename); never leave a half-written manifest.
    pub fn save(&self, paper_dir: &Path) -> Result<()> {
        let path = paper_dir.join(MANIFEST_FILENAME);
        let content = serde_json::to_string_pretty(self)?;
        let tmp = paper_dir.join(".paper.json.tmp");
        std::fs::write(&tmp, content)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }
}

/// Strip the version suffix from an arXiv ID: `2501.12948v2` -> `2501.12948`.
/// No-op when there is no `v<N>` suffix.
pub fn strip_version(id: &str) -> String {
    let trimmed = id.trim();
    if let Some(i) = trimmed.rfind('v') {
        let suffix = &trimmed[i + 1..];
        if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()) {
            return trimmed[..i].to_string();
        }
    }
    trimmed.to_string()
}

/// Build a manifest for a paper dir by scanning its current files.
/// Preserves cooldown/error state if a previous manifest exists.
pub fn scan_manifest(paper_dir: &Path, arxiv_id: &str, title: &str) -> Result<PaperManifest> {
    let prev = PaperManifest::load(paper_dir)?;

    let file_opt = |name: &str| -> Option<String> {
        if paper_dir.join(name).is_file() {
            Some(name.to_string())
        } else {
            None
        }
    };

    let files = ManifestFiles {
        body: file_opt("body.tex"),
        appendix: file_opt("appendix.tex"),
        note: file_opt("note.txt"),
        pdf: file_opt("paper.pdf"),
        description: file_opt("description.md"),
    };

    let description_ready =
        files.description.is_some() && paper_dir.join(".description_ready").is_file();

    Ok(PaperManifest {
        schema: 1,
        arxiv_id: arxiv_id.to_string(),
        base_id: strip_version(arxiv_id),
        title: title.to_string(),
        downloaded_at: prev
            .as_ref()
            .map(|m| m.downloaded_at.clone())
            .unwrap_or_default(),
        files,
        description_ready,
        last_error: prev.as_ref().and_then(|m| m.last_error.clone()),
        cooldown_until_ms: prev.as_ref().map(|m| m.cooldown_until_ms).unwrap_or(0),
    })
}

/// Refresh the manifest for an existing paper dir (lazy migration + inventory
/// update). Only called from write-path commands.
pub fn refresh_manifest(paper_dir: &Path, arxiv_id: &str, title: &str) -> Result<()> {
    let m = scan_manifest(paper_dir, arxiv_id, title)?;
    m.save(paper_dir)
}

/// `true` if the paper is inside its retry cooldown window (P1.4).
pub fn in_cooldown(manifest: &PaperManifest, now_ms: u64) -> bool {
    manifest.cooldown_until_ms > now_ms
}

/// Mark a failure and arm the 24h cooldown.
pub fn mark_failure(paper_dir: &Path, error: &str) -> Result<()> {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let mut m = match PaperManifest::load(paper_dir)? {
        Some(m) => m,
        None => {
            // Unknown paper dir: create a minimal manifest so the cooldown
            // survives across runs.
            PaperManifest {
                schema: 1,
                arxiv_id: String::new(),
                base_id: String::new(),
                title: String::new(),
                downloaded_at: String::new(),
                files: ManifestFiles::default(),
                description_ready: false,
                last_error: None,
                cooldown_until_ms: 0,
            }
        }
    };
    m.last_error = Some(error.to_string());
    m.cooldown_until_ms = now_ms + 24 * 60 * 60 * 1000;
    m.save(paper_dir)
}

/// Clear cooldown/error state after a successful operation.
pub fn clear_cooldown(paper_dir: &Path) -> Result<()> {
    if let Some(mut m) = PaperManifest::load(paper_dir)? {
        m.last_error = None;
        m.cooldown_until_ms = 0;
        m.save(paper_dir)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_version() {
        assert_eq!(strip_version("2501.12948"), "2501.12948");
        assert_eq!(strip_version("2501.12948v2"), "2501.12948");
        assert_eq!(strip_version("2501.12948v10"), "2501.12948");
        assert_eq!(strip_version("abc"), "abc");
    }

    #[test]
    fn test_manifest_roundtrip_preserves_cooldown() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("body.tex"), "x").unwrap();

        let m = scan_manifest(dir.path(), "2501.12948v2", "Test").unwrap();
        assert_eq!(m.base_id, "2501.12948");
        assert_eq!(m.files.body.as_deref(), Some("body.tex"));
        m.save(dir.path()).unwrap();

        let loaded = PaperManifest::load(dir.path()).unwrap().unwrap();
        assert_eq!(loaded.arxiv_id, "2501.12948v2");
        assert_eq!(loaded.base_id, "2501.12948");
        assert_eq!(loaded.title, "Test");
    }

    #[test]
    fn test_mark_failure_sets_24h_cooldown() {
        let dir = tempfile::tempdir().unwrap();
        mark_failure(dir.path(), "boom").unwrap();
        let m = PaperManifest::load(dir.path()).unwrap().unwrap();
        assert_eq!(m.last_error.as_deref(), Some("boom"));
        assert!(m.cooldown_until_ms > 0);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        assert!(in_cooldown(&m, now), "freshly armed cooldown must hold");
        clear_cooldown(dir.path()).unwrap();
        let m2 = PaperManifest::load(dir.path()).unwrap().unwrap();
        assert!(!in_cooldown(&m2, now));
    }

    #[test]
    fn test_malformed_manifest_is_parse_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("paper.json"), "not json{{").unwrap();
        let res = PaperManifest::load(dir.path());
        assert!(res.is_err());
    }
}
