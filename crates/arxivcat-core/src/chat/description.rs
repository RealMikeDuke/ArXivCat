use std::path::Path;

use crate::error::Result;

/// Generate the brief (round 1). Kept as `build_description` for caller
/// compatibility; the implementation now lives in `summary::generate_brief`,
/// which writes `brief_summary.md` + `.description_ready` (flag name kept so
/// the manifest contract is unchanged). `log_cb` / `context_override` are
/// retained for signature stability but unused by the new pipeline.
pub async fn build_description(
    cfg: &crate::net::HttpConfig,
    paper_dir: &Path,
    arxiv_id: &str,
    title: &str,
    _log_cb: Option<&(dyn Fn(&str) + Sync)>,
    _context_override: Option<&str>,
) -> Result<()> {
    super::summary::generate_brief(cfg, paper_dir, arxiv_id, title)
        .await
        .map(|_| ())
}
