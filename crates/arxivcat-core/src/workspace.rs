use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::error::{ArxivError, Result};
use crate::extract::{arxiv::extract_arxiv_id_from_pdf, source::download_pdf};

const WORKSPACE_INTERNAL_DIRS: &[&str] = &["arxivcat_global_chats"];

/// Name of the subdirectory holding paper entities (canonical layout since
/// 0.11.13). Tag directories live at the workspace root and symlink into
/// `raw/` — the raw dir is skipped by tag listing and vice versa.
pub const RAW_DIR: &str = "raw";

/// True if `folder_name` is reserved (dotdirs, internal dirs, the raw dir).
pub fn is_reserved_dir(folder_name: &str) -> bool {
    folder_name.starts_with('.') || WORKSPACE_INTERNAL_DIRS.contains(&folder_name)
}

/// Validate a tag name: no path separators, no reserved names, and it must
/// not look like a paper folder (`2501_12948` style — digits+underscore),
/// otherwise a tag dir could be mistaken for a legacy paper.
pub fn validate_tag_name(tag: &str) -> Result<()> {
    if tag.is_empty() {
        return Err(crate::error::ArxivError::Other(
            "tag name must not be empty".into(),
        ));
    }
    if tag.contains('/') || tag.contains('\\') || tag.contains(std::path::MAIN_SEPARATOR) {
        return Err(crate::error::ArxivError::Other(format!(
            "tag name must not contain path separators: {tag:?}"
        )));
    }
    if is_reserved_dir(tag) || tag == RAW_DIR {
        return Err(crate::error::ArxivError::Other(format!(
            "tag name is reserved: {tag:?}"
        )));
    }
    let parts: Vec<&str> = tag.split('_').collect();
    if parts.len() >= 2 && parts[0].chars().all(|c| c.is_ascii_digit()) {
        return Err(crate::error::ArxivError::Other(format!(
            "tag name looks like a paper id (digits+underscore), pick another: {tag:?}"
        )));
    }
    Ok(())
}

/// List existing tags: directories at the workspace root that are not
/// reserved, not the raw dir, and not paper directories.
pub fn list_tags(ws: &std::path::Path) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(ws) {
        for entry in entries.flatten() {
            if let Ok(ft) = entry.file_type() {
                if ft.is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if is_reserved_dir(&name) || name == RAW_DIR {
                        continue;
                    }
                    if Paper::from_folder(&entry.path()).is_some() {
                        continue; // a paper dir, not a tag
                    }
                    tags.push(name);
                }
            }
        }
    }
    tags.sort();
    tags
}

/// Relative symlink target from a tag dir at the workspace root to a paper
/// folder: canonical raw/ layout -> `../raw/{folder}`, legacy root layout
/// -> `../{folder}`.
fn tag_link_target(ws: &std::path::Path, paper: &Paper) -> std::path::PathBuf {
    let raw = ws.join(RAW_DIR);
    if paper.folder.starts_with(&raw) {
        std::path::PathBuf::from("..")
            .join(RAW_DIR)
            .join(&paper.folder_name)
    } else {
        std::path::PathBuf::from("..").join(&paper.folder_name)
    }
}

/// Add a paper to a tag: create the tag dir if needed and symlink
/// `{ws}/{tag}/{folder}` -> relative target. Idempotent.
pub fn tag_paper(ws: &std::path::Path, paper: &Paper, tag: &str) -> Result<std::path::PathBuf> {
    validate_tag_name(tag)?;
    let tag_dir = ws.join(tag);
    std::fs::create_dir_all(&tag_dir)?;
    let link = tag_dir.join(&paper.folder_name);
    if std::fs::symlink_metadata(&link).is_ok() {
        return Ok(link); // already tagged
    }
    let target = tag_link_target(ws, paper);
    symlink_dir(&target, &link)?;
    Ok(link)
}

/// Remove a paper from a tag (removes the symlink). Idempotent.
pub fn untag_paper(ws: &std::path::Path, paper: &Paper, tag: &str) -> Result<()> {
    let link = ws.join(tag).join(&paper.folder_name);
    if std::fs::symlink_metadata(&link).is_ok() {
        std::fs::remove_file(&link)?;
    }
    Ok(())
}

#[cfg(unix)]
fn symlink_dir(target: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(not(unix))]
fn symlink_dir(target: &std::path::Path, link: &std::path::Path) -> std::io::Result<()> {
    // Windows: directory junction (no admin rights needed) when available;
    // fall back to a plain directory copy is NOT done here — symlink support
    // on Windows is best-effort (see docs/planning/0.12-content-pdf.md U-series).
    #[cfg(windows)]
    {
        use std::os::windows::fs::symlink_dir as win_symlink_dir;
        return win_symlink_dir(target, link);
    }
    #[cfg(not(windows))]
    {
        let _ = (target, link);
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "symlinks unsupported on this platform",
        ))
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Paper {
    pub arxiv_id: String,
    pub title: String,
    pub folder_name: String,
    pub folder: PathBuf,
    pub has_body: bool,
    pub description_ready: bool,
    pub deep_ready: bool,
    pub is_complete: bool,
}

impl Paper {
    pub fn from_folder(folder: &Path) -> Option<Self> {
        let folder_name = folder.file_name()?.to_string_lossy().to_string();

        if folder_name.starts_with('.') || WORKSPACE_INTERNAL_DIRS.contains(&folder_name.as_str()) {
            return None;
        }

        // Canonical: paper.json manifest is the single source of truth.
        if let Ok(Some(m)) = crate::manifest::PaperManifest::load(folder) {
            let has_body = m
                .files
                .body
                .as_deref()
                .map(|f| folder.join(f).is_file())
                .unwrap_or(false);
            let description_ready = m.description_ready && has_complete_description(folder);
            let deep_ready = m.deep_ready && has_complete_deep(folder);
            return Some(Paper {
                arxiv_id: m.arxiv_id,
                title: m.title,
                folder_name,
                folder: folder.to_path_buf(),
                has_body,
                description_ready,
                deep_ready,
                is_complete: has_body,
            });
        }

        // Legacy read-only fallback: parse {id1}_{id2}_..._{title} (or ID-only
        // {id1}_{id2} with empty title). P1.2 makes the ID-only form canonical.
        // First segment MUST be digits — otherwise a user dir like `my_notes`
        // would become a ghost paper `my.notes` and get downloaded into (C).
        let parts: Vec<&str> = folder_name.split('_').collect();
        if parts.len() < 2 || !parts[0].chars().all(|c| c.is_ascii_digit()) {
            return None;
        }

        let arxiv_id = format!("{}.{}", parts[0], parts[1]);
        let title = parts[2..].join(" ");

        let has_body = folder.join("body.tex").exists();
        let description_ready = has_complete_description(folder);
        let is_complete = has_body;

        Some(Paper {
            arxiv_id,
            title,
            folder_name,
            folder: folder.to_path_buf(),
            has_body,
            description_ready,
            deep_ready: has_complete_deep(folder),
            is_complete,
        })
    }
}

fn has_complete_description(paper_dir: &Path) -> bool {
    let flag = paper_dir.join(".description_ready");
    for name in ["brief_summary.md", "description.md"] {
        if let Ok(meta) = std::fs::metadata(paper_dir.join(name)) {
            if meta.len() > 0 && flag.exists() {
                return true;
            }
        }
    }
    false
}

fn has_complete_deep(paper_dir: &Path) -> bool {
    if let Ok(meta) = std::fs::metadata(paper_dir.join("deep_summary.md")) {
        meta.len() > 0 && paper_dir.join(".deep_ready").exists()
    } else {
        false
    }
}

#[derive(Debug)]
pub struct Workspace {
    pub path: PathBuf,
    pub papers: Vec<Paper>,
}

impl Workspace {
    pub fn open(path: &Path) -> Result<Self> {
        // Read-only open: never create directories here. Commands that need
        // to write (download, chat, note, ...) create dirs explicitly. This
        // lets `paper list` work on a read-only workspace (P0.9).
        let path = path.to_path_buf();
        if !path.exists() {
            return Err(ArxivError::NotFound(format!(
                "workspace not found: {}",
                path.display()
            )));
        }
        let papers = Self::list_papers(&path);
        Ok(Workspace { path, papers })
    }

    pub fn list_papers(path: &Path) -> Vec<Paper> {
        let mut papers: Vec<Paper> = Vec::new();
        // De-duplicate by folder name: the same paper could exist as a legacy
        // root entry AND under raw/ during a transition.
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

        // New layout: papers live under workspace/raw/ (canonical since 0.11.13).
        let raw_dir = path.join(RAW_DIR);
        if raw_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&raw_dir) {
                for entry in entries.flatten() {
                    if let Ok(file_type) = entry.file_type() {
                        if file_type.is_dir() {
                            if let Some(paper) = Paper::from_folder(&entry.path()) {
                                if seen.insert(paper.folder_name.clone()) {
                                    papers.push(paper);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Legacy compatibility: papers at the workspace root (pre-0.11.13
        // layout). Tag directories (real dirs whose name is not a paper id)
        // and symlink entries are rejected by Paper::from_folder, so they
        // never appear here.
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_dir() {
                        if let Some(paper) = Paper::from_folder(&entry.path()) {
                            if seen.insert(paper.folder_name.clone()) {
                                papers.push(paper);
                            }
                        }
                    }
                }
            }
        }

        papers.sort_by(|a, b| b.arxiv_id.cmp(&a.arxiv_id));
        papers
    }

    pub fn refresh(&mut self) {
        self.papers = Self::list_papers(&self.path);
    }

    pub fn load_paper(&self, folder_name: &str) -> Option<&Paper> {
        self.papers.iter().find(|p| p.folder_name == folder_name)
    }

    /// All papers matching an ID/query. Exact base-ID match (version-stripped,
    /// P1.2) wins; otherwise prefix matches are returned and callers must
    /// treat >1 result as an ambiguity error, never silently pick one.
    pub fn find_papers_by_id(&self, arxiv_id: &str) -> Vec<&Paper> {
        let base = crate::manifest::strip_version(arxiv_id).to_lowercase();
        let normalized = arxiv_id.replace(['.', '-'], "_").to_lowercase();

        let exact: Vec<&Paper> = self
            .papers
            .iter()
            .filter(|p| crate::manifest::strip_version(&p.arxiv_id).to_lowercase() == base)
            .collect();
        if !exact.is_empty() {
            return exact;
        }

        self.papers
            .iter()
            .filter(|p| {
                let fid = p
                    .folder_name
                    .split('_')
                    .take(2)
                    .collect::<Vec<_>>()
                    .join("_")
                    .to_lowercase();
                fid.starts_with(&normalized) || normalized.starts_with(&fid)
            })
            .collect()
    }

    /// Returns the FIRST paper matching `arxiv_id`.
    ///
    /// ⚠️ When the workspace contains both a legacy `{id}_{title}` folder and
    /// the canonical `{id}` folder, this silently picks one — prefer
    /// `find_papers_by_id` plus explicit disambiguation in new callers. Kept
    /// as-is for API stability (0.11.x); revisit with a `Result` in a future
    /// breaking release.
    pub fn find_paper_by_id(&self, arxiv_id: &str) -> Option<&Paper> {
        self.find_papers_by_id(arxiv_id).into_iter().next()
    }

    pub fn pending_papers(&self) -> Vec<&Paper> {
        self.papers.iter().filter(|p| !p.is_complete).collect()
    }
}

pub async fn scan_workspace_pdfs(
    cfg: &crate::net::HttpConfig,
    workspace: &mut Workspace,
) -> Result<usize> {
    let v_suffix_re = regex::Regex::new(r"v\d+$").unwrap();
    let mut existing_ids: HashSet<String> = workspace
        .papers
        .iter()
        .map(|p| v_suffix_re.replace(&p.arxiv_id, "").to_string())
        .collect();

    // Collect new PDF IDs first so titles can be fetched in ONE export-API
    // batch (P1.3) instead of one abs-page fetch per paper.
    let mut new_ids: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&workspace.path) {
        for entry in entries.flatten() {
            if !entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                continue;
            }
            let path = entry.path();
            if path.extension().map(|e| e == "pdf").unwrap_or(false) {
                if let Ok(Some(id)) = extract_arxiv_id_from_pdf(&path) {
                    let base_id = v_suffix_re.replace(&id, "").to_string();
                    if existing_ids.contains(&base_id) || new_ids.contains(&base_id) {
                        continue;
                    }
                    new_ids.push(base_id);
                }
            }
        }
    }

    let titles = crate::extract::arxiv::fetch_titles_batch(cfg, &new_ids).await;

    let mut count = 0;
    for base_id in &new_ids {
        let title = titles.get(base_id).cloned().unwrap_or_default();

        // P1.2: canonical folder name is the base ID (no title).
        let folder_name = base_id.replace('.', "_");
        let folder = workspace.path.join(&folder_name);
        std::fs::create_dir_all(&folder)?;

        let _note = std::fs::File::create(folder.join("note.txt"));
        let _desc = std::fs::File::create(folder.join("description.md"));
        std::fs::create_dir_all(folder.join("arxiv_chats"))?;

        // Lazy migration (P1.1): write the manifest immediately so the
        // folder is canonical on next open.
        let _ = crate::manifest::refresh_manifest(&folder, base_id, &title);

        existing_ids.insert(base_id.clone());
        count += 1;
    }

    workspace.refresh();
    Ok(count)
}

pub async fn process_pending_paper(
    cfg: &crate::net::HttpConfig,
    paper: &Paper,
    downloads_dir: &Path,
    workspace_path: &Path,
    cancel_flag: &std::sync::atomic::AtomicBool,
    on_event: Option<&(dyn Fn(&str) + Sync)>,
) -> Result<bool> {
    let ev = |name: &str| {
        if let Some(cb) = on_event {
            cb(name);
        }
    };

    if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
        return Ok(false);
    }

    let arxiv_id = &paper.arxiv_id;
    let out_dir = workspace_path.join(&paper.folder_name);

    if paper.has_body {
        return Ok(true);
    }

    ev("downloading");

    let (paper_dir_opt, _) =
        crate::extract::source::download_source(cfg, arxiv_id, downloads_dir).await?;

    let paper_dir = match paper_dir_opt {
        Some(d) => d,
        None => return Ok(false),
    };

    if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
        return Ok(false);
    }

    crate::extract::tex::extract_body_from_dir(&paper_dir, &out_dir)?;
    ev("downloaded");

    if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
        return Ok(false);
    }

    let _ = download_pdf(cfg, arxiv_id, &out_dir).await;

    ensure_paper_meta_files(&out_dir)?;

    // Manifest is the single source of truth (P1.1): refresh it now so the
    // folder is migrated and inventory/cooldown state is durable.
    crate::manifest::refresh_manifest(&out_dir, arxiv_id, &paper.title)?;
    crate::manifest::clear_cooldown(&out_dir)?;

    Ok(out_dir.join("body.tex").exists())
}

pub fn ensure_paper_meta_files(paper_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(paper_dir)?;
    crate::manifest::lazy_migrate_brief(paper_dir);
    let note = paper_dir.join("note.txt");
    if !note.exists() {
        std::fs::write(&note, "")?;
    }
    let brief = paper_dir.join("brief_summary.md");
    if !brief.exists() {
        std::fs::write(&brief, "")?;
    }
    std::fs::create_dir_all(paper_dir.join("arxiv_chats"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paper_from_folder_complete() {
        let dir = tempfile::tempdir().unwrap();
        let paper_dir = dir.path().join("2501_12948_Test_Paper");
        std::fs::create_dir(&paper_dir).unwrap();
        std::fs::write(paper_dir.join("body.tex"), "content").unwrap();
        std::fs::write(paper_dir.join("description.md"), "desc").unwrap();
        std::fs::write(paper_dir.join(".description_ready"), "ok\n").unwrap();

        let paper = Paper::from_folder(&paper_dir).unwrap();
        assert_eq!(paper.arxiv_id, "2501.12948");
        assert!(paper.is_complete);
    }

    #[test]
    fn test_paper_from_folder_pending() {
        // New semantics (AI decoupled): is_complete == has_body.
        // A paper with body.tex but no description is complete, and
        // description_ready tracks the optional AI state independently.
        let dir = tempfile::tempdir().unwrap();
        let paper_dir = dir.path().join("2412_04445_Moto");
        std::fs::create_dir(&paper_dir).unwrap();
        std::fs::write(paper_dir.join("body.tex"), "content").unwrap();

        let paper = Paper::from_folder(&paper_dir).unwrap();
        assert_eq!(paper.arxiv_id, "2412.04445");
        assert!(paper.is_complete);
        assert!(!paper.description_ready);
    }

    #[test]
    fn test_paper_skips_internal_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let internal = dir.path().join("arxivcat_global_chats");
        std::fs::create_dir(&internal).unwrap();

        assert!(Paper::from_folder(&internal).is_none());
    }

    #[test]
    fn test_paper_skips_hidden() {
        let dir = tempfile::tempdir().unwrap();
        let hidden = dir.path().join(".hidden_dir");
        std::fs::create_dir(&hidden).unwrap();

        assert!(Paper::from_folder(&hidden).is_none());
    }

    #[test]
    fn test_workspace_list_papers() {
        let dir = tempfile::tempdir().unwrap();
        let p1 = dir.path().join("2501_12948_Test");
        let p2 = dir.path().join("2412_04445_Moto");
        std::fs::create_dir(&p1).unwrap();
        std::fs::create_dir(&p2).unwrap();
        std::fs::write(p1.join("body.tex"), "x").unwrap();
        std::fs::write(p2.join("body.tex"), "x").unwrap();

        let papers = Workspace::list_papers(dir.path());
        assert_eq!(papers.len(), 2);
        assert_eq!(papers[0].arxiv_id, "2501.12948");
        assert_eq!(papers[1].arxiv_id, "2412.04445");
    }
}

#[cfg(test)]
mod tag_tests {
    use super::*;

    static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

    fn ws() -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("arxivcat_ws_{}_{}", std::process::id(), n));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("raw")).unwrap();
        dir
    }

    fn paper(ws: &std::path::Path, folder: &str) -> Paper {
        let dir = ws.join("raw").join(folder);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("body.tex"), "x").unwrap();
        Paper {
            arxiv_id: folder.replace('_', "."),
            title: String::new(),
            folder_name: folder.to_string(),
            folder: dir,
            has_body: true,
            description_ready: false,
            deep_ready: false,
            is_complete: true,
        }
    }

    #[test]
    fn list_papers_scans_raw_and_legacy_root() {
        let dir = ws();
        let p = paper(&dir, "2501_11111");
        // legacy root layout
        let legacy = dir.join("2501_22222");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join("body.tex"), "y").unwrap();
        let papers = Workspace::list_papers(&dir);
        let ids: Vec<&str> = papers.iter().map(|p| p.arxiv_id.as_str()).collect();
        assert!(ids.contains(&"2501.11111"), "raw paper found: {ids:?}");
        assert!(ids.contains(&"2501.22222"), "legacy paper found: {ids:?}");
        let _ = p;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tag_dir_is_not_a_paper() {
        let dir = ws();
        paper(&dir, "2501_11111");
        std::fs::create_dir_all(dir.join("3d-vision")).unwrap();
        let papers = Workspace::list_papers(&dir);
        assert_eq!(papers.len(), 1, "tag dir must not appear as paper");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tag_add_creates_dir_and_relative_symlink() {
        let dir = ws();
        let p = paper(&dir, "2501_11111");
        let link = tag_paper(&dir, &p, "3d-vision").unwrap();
        assert!(link.exists() || std::fs::symlink_metadata(&link).is_ok());
        let target = std::fs::read_link(&link).unwrap();
        assert_eq!(target, std::path::PathBuf::from("../raw/2501_11111"));
        // idempotent
        let again = tag_paper(&dir, &p, "3d-vision").unwrap();
        assert_eq!(again, link);
        let tags = list_tags(&dir);
        assert_eq!(tags, vec!["3d-vision".to_string()]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tag_add_legacy_root_uses_relative_root_target() {
        let dir = ws();
        let folder = dir.join("2501_22222");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("body.tex"), "y").unwrap();
        let p = Paper {
            arxiv_id: "2501.22222".into(),
            title: String::new(),
            folder_name: "2501_22222".into(),
            folder: folder.clone(),
            has_body: true,
            description_ready: false,
            deep_ready: false,
            is_complete: true,
        };
        let link = tag_paper(&dir, &p, "notes").unwrap();
        let target = std::fs::read_link(&link).unwrap();
        assert_eq!(target, std::path::PathBuf::from("../2501_22222"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tag_remove_deletes_symlink() {
        let dir = ws();
        let p = paper(&dir, "2501_11111");
        tag_paper(&dir, &p, "3d-vision").unwrap();
        untag_paper(&dir, &p, "3d-vision").unwrap();
        assert!(std::fs::symlink_metadata(dir.join("3d-vision/2501_11111")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_tag_rejects_bad_names() {
        assert!(validate_tag_name("3d-vision").is_ok());
        assert!(validate_tag_name("llm").is_ok());
        assert!(
            validate_tag_name("2501_99999").is_err(),
            "digits+underscore looks like paper"
        );
        assert!(validate_tag_name("raw").is_err());
        assert!(validate_tag_name("a/b").is_err());
        assert!(validate_tag_name("arxivcat_global_chats").is_err());
        assert!(validate_tag_name(".hidden").is_err());
        assert!(validate_tag_name("").is_err());
    }
}
