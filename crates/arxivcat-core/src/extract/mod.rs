pub mod arxiv;
pub mod source;
pub mod tex;

use crate::error::Result;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct ExtractionOutput {
    pub body: String,
    pub appendix: Option<String>,
    pub body_path: PathBuf,
    pub appendix_path: Option<PathBuf>,
    pub pdf_path: Option<PathBuf>,
    /// Non-fatal extraction warnings (unexpanded references, encoding, etc.).
    pub warnings: Vec<String>,
}

pub async fn extract_paper(
    cfg: &crate::net::HttpConfig,
    arxiv_id: &str,
    downloads_dir: &Path,
    output_dir: &Path,
) -> Result<ExtractionOutput> {
    let (paper_dir, _folder_name) =
        source::download_source(cfg, arxiv_id, downloads_dir).await?;

    let paper_dir = paper_dir.ok_or_else(|| {
        crate::error::ArxivError::Extraction("source download returned None".into())
    })?;

    tex::extract_body_from_dir(&paper_dir, output_dir)
}
