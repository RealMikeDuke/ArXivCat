use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::error::{ArxivError, Result};
use crate::extract::{arxiv::extract_arxiv_id_from_pdf, source::download_pdf};

const WORKSPACE_INTERNAL_DIRS: &[&str] = &["arxivcat_global_chats"];

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Paper {
    pub arxiv_id: String,
    pub title: String,
    pub folder_name: String,
    pub folder: PathBuf,
    pub has_body: bool,
    pub description_ready: bool,
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
            return Some(Paper {
                arxiv_id: m.arxiv_id,
                title: m.title,
                folder_name,
                folder: folder.to_path_buf(),
                has_body,
                description_ready,
                is_complete: has_body,
            });
        }

        // Legacy read-only fallback: parse {id1}_{id2}_..._{title} (or ID-only
        // {id1}_{id2} with empty title). P1.2 makes the ID-only form canonical.
        let parts: Vec<&str> = folder_name.split('_').collect();
        if parts.len() < 2 {
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
            is_complete,
        })
    }
}

fn has_complete_description(paper_dir: &Path) -> bool {
    let desc = paper_dir.join("description.md");
    let flag = paper_dir.join(".description_ready");
    if let Ok(meta) = std::fs::metadata(&desc) {
        meta.len() > 0 && flag.exists()
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

        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_dir() {
                        if let Some(paper) = Paper::from_folder(&entry.path()) {
                            papers.push(paper);
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
) -> Result<bool> {
    if cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
        return Ok(false);
    }

    let arxiv_id = &paper.arxiv_id;
    let out_dir = workspace_path.join(&paper.folder_name);

    if paper.has_body {
        return Ok(true);
    }

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
    let note = paper_dir.join("note.txt");
    if !note.exists() {
        std::fs::write(&note, "")?;
    }
    let desc = paper_dir.join("description.md");
    if !desc.exists() {
        std::fs::write(&desc, "")?;
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
