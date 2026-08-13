use std::path::{Path, PathBuf};

use crate::error::{ArxivError, Result};
use crate::net::HttpConfig;

/// Cross-process download lock (P1.7): a lock file per base ID under
/// `{downloads_dir}/.locks/`. Released on drop (including error paths).
struct DownloadLock {
    path: PathBuf,
}

impl DownloadLock {
    /// Lock files older than this are considered stale (crashed process) and
    /// reclaimed.
    const STALE_AFTER_SECS: u64 = 10 * 60;

    /// Max time we wait for a busy lock before giving up (jury-ask A).
    const WAIT_BUDGET: std::time::Duration = std::time::Duration::from_secs(30);
    /// Poll interval while waiting for the lock holder to finish.
    const POLL: std::time::Duration = std::time::Duration::from_millis(500);

    async fn acquire(downloads_dir: &Path, id_dir_name: &str) -> Result<Self> {
        let lock_dir = downloads_dir.join(".locks");
        std::fs::create_dir_all(&lock_dir)?;
        let path = lock_dir.join(format!("{id_dir_name}.lock"));

        // Atomic O_EXCL create — check-then-act would let two processes
        // through simultaneously (TOCTOU). Lock content = unix ms.
        let now_ms = || {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0)
        };

        // Bounded wait instead of failing on the first collision: another
        // process downloading the SAME paper finishes in seconds, so an
        // immediate failure would arm a 24h cooldown for a transient "busy"
        // (P2-3, jury-ask A). Only after the budget is exhausted do we give
        // up and let the normal failure path (cooldown + --force) handle it.
        let deadline = std::time::Instant::now() + Self::WAIT_BUDGET;
        loop {
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            match options.open(&path) {
                Ok(mut f) => {
                    use std::io::Write;
                    let _ = f.write_all(now_ms().to_string().as_bytes());
                    return Ok(Self { path });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    // Stale recovery: a crashed process left the lock behind.
                    // Re-check every poll — the holder may drop it any moment.
                    let stale = std::fs::metadata(&path)
                        .and_then(|m| m.modified())
                        .map(|t| {
                            t.elapsed()
                                .map(|d| d.as_secs() > Self::STALE_AFTER_SECS)
                                .unwrap_or(false)
                        })
                        .unwrap_or(false);
                    if stale {
                        let _ = std::fs::remove_file(&path);
                        // Loop immediately: next iteration re-creates or, if
                        // another process grabbed it first, keeps waiting.
                        continue;
                    }
                    if std::time::Instant::now() >= deadline {
                        return Err(ArxivError::Other(format!(
                            "another process is already downloading {id_dir_name} (waited {:?})",
                            Self::WAIT_BUDGET
                        )));
                    }
                    tokio::time::sleep(Self::POLL).await;
                }
                Err(e) => return Err(ArxivError::Io(e)),
            }
        }
    }
}

impl Drop for DownloadLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Find a legacy `{id}_{title}` cache dir for a base ID (P1.2 compat).
fn find_legacy_cache(downloads_dir: &Path, base_id_dir: &str) -> Option<PathBuf> {
    let pattern = format!("{}/{}_*", downloads_dir.display(), base_id_dir);
    let entries = glob::glob(&pattern).ok()?;
    entries.flatten().find(|e| e.is_dir())
}

pub async fn download_source(
    cfg: &HttpConfig,
    arxiv_id: &str,
    downloads_dir: &Path,
) -> Result<(Option<PathBuf>, Option<String>)> {
    std::fs::create_dir_all(downloads_dir)?;

    // P1.2: canonical folder name is the version-stripped base ID, no title.
    let id_dir_name = crate::manifest::strip_version(arxiv_id).replace('.', "_");

    // Cache-first: the ID-only folder must hit without any network request.
    let id_dir = downloads_dir.join(&id_dir_name);
    if id_dir.exists() {
        if validate_cache(&id_dir)? {
            return Ok((Some(id_dir), Some(id_dir_name)));
        }
        force_uniform_permissions(&id_dir)?;
        if validate_cache(&id_dir)? {
            return Ok((Some(id_dir), Some(id_dir_name)));
        }
        match std::fs::remove_dir_all(&id_dir) {
            Ok(_) => {}
            Err(_) => {
                return Err(ArxivError::Other(format!(
                    "cache directory {} exists but cannot be removed; please delete it manually",
                    id_dir.display()
                )));
            }
        }
    }

    // A legacy {id}_{title} cache dir for the same paper may already exist;
    // accept it (read-only compat), new downloads are always ID-only.
    let legacy_name = find_legacy_cache(downloads_dir, &id_dir_name);
    if let Some(legacy_name) = legacy_name {
        if validate_cache(&legacy_name)? {
            let legacy_folder = legacy_name
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            return Ok((Some(legacy_name), Some(legacy_folder)));
        }
        force_uniform_permissions(&legacy_name)?;
        if validate_cache(&legacy_name)? {
            let legacy_folder = legacy_name
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            return Ok((Some(legacy_name), Some(legacy_folder)));
        }
        match std::fs::remove_dir_all(&legacy_name) {
            Ok(_) => {}
            Err(_) => {
                return Err(ArxivError::Other(format!(
                    "cache directory {} exists but cannot be removed; please delete it manually",
                    legacy_name.display()
                )));
            }
        }
    }

    // P1.2: ID-only folders carry no title, so there is nothing to fetch
    // here — the previous best-effort abs-page request was dead work.
    let folder_name = id_dir_name.clone();
    let paper_dir = downloads_dir.join(&folder_name);

    // P1.7: cross-process download lock — one downloader per paper at a time.
    let _lock = DownloadLock::acquire(downloads_dir, &id_dir_name).await?;

    // The winner may have finished while we waited for the lock — re-check
    // the cache so we never re-download a paper another process just got
    // (and never hit the move-to-target race on the same folder).
    if id_dir.exists() && validate_cache(&id_dir)? {
        return Ok((Some(id_dir), Some(folder_name.clone())));
    }

    let tar_url = cfg.arxiv_src_url(arxiv_id);
    let response = cfg.get_with_retry(&tar_url).await?;

    if !response.status().is_success() {
        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(ArxivError::NotFound(format!(
                "paper source not found on arXiv: HTTP {status}"
            )));
        }
        return Err(ArxivError::HttpStatus(status.as_u16()));
    }

    // P1.8: unique temp tar name (pid+ts) so concurrent runs never collide
    // on a fixed `{id}.tar.gz` path.
    let tar_dir = downloads_dir.join(".tmp");
    std::fs::create_dir_all(&tar_dir)?;
    let unique = format!(
        "{}_{}_{}.tar.gz",
        id_dir_name,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    );
    let tar_path = tar_dir.join(unique);
    let bytes = response.bytes().await.map_err(ArxivError::Http)?;
    std::fs::write(&tar_path, &bytes)?;

    let temp_dir = tempfile::Builder::new()
        .prefix(&format!("{arxiv_id}_"))
        .tempdir_in(downloads_dir)?;

    extract_tar(&tar_path, temp_dir.path())?;

    let _ = std::fs::remove_file(&tar_path);

    match move_to_target(temp_dir.path(), &paper_dir) {
        Ok(()) => Ok((Some(paper_dir), Some(folder_name))),
        Err(e) => Err(ArxivError::Other(format!(
            "failed to move extracted source into {}: {e}; check permissions and free disk space",
            paper_dir.display()
        ))),
    }
}

pub async fn download_pdf(
    cfg: &HttpConfig,
    arxiv_id: &str,
    output_dir: &Path,
) -> Result<Option<PathBuf>> {
    // Write with the base id (version stripped) so the manifest's
    // {base_id}.pdf scan and the on-disk file always agree — a versioned
    // input like 2501.12948v2 must record the same pdf as 2501.12948.
    let base_id = crate::manifest::strip_version(arxiv_id);
    let pdf_path = output_dir.join(format!("{base_id}.pdf"));
    if pdf_path.exists() {
        return Ok(Some(pdf_path));
    }

    let url = cfg.arxiv_pdf_url(arxiv_id);
    let response = cfg.get_with_retry(&url).await?;

    if !response.status().is_success() {
        return Ok(None);
    }

    let bytes = response.bytes().await.map_err(ArxivError::Http)?;

    std::fs::create_dir_all(output_dir)?;
    std::fs::write(&pdf_path, &bytes)?;

    Ok(Some(pdf_path))
}

fn validate_cache(paper_dir: &Path) -> Result<bool> {
    let main_tex = crate::extract::tex::find_main_tex(paper_dir);
    if main_tex.is_none() {
        return Ok(false);
    }

    if !can_walk_dir(paper_dir) {
        return Ok(false);
    }

    if !can_read_tex_files(paper_dir) {
        return Ok(false);
    }

    Ok(true)
}

fn can_walk_dir(dir: &Path) -> bool {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for _ in entries {}
        return true;
    }
    false
}

fn can_read_tex_files(dir: &Path) -> bool {
    let pattern = format!("{}/**/*.tex", dir.display());
    let entries = match glob::glob(&pattern) {
        Ok(e) => e,
        Err(_) => return false,
    };

    for entry in entries.flatten() {
        // Lossy read: legacy latin-1 papers must not invalidate the cache.
        let _ = crate::extract::tex::read_to_string_lossy(&entry);
    }
    true
}

/// Normalize permissions on a directory tree (files 0644, dirs 0755).
///
/// Used both right after tar extraction (prevents arXiv tar files carrying
/// 000/0400 modes from breaking later reads) and as the legacy-cache repair
/// path on validate failure. Symlinks are skipped: set_permissions follows
/// symlinks, which would write through them.
pub fn force_uniform_permissions(dir: &Path) -> Result<()> {
    let pattern = format!("{}/**/*", dir.display());
    if let Ok(entries) = glob::glob(&pattern) {
        for entry in entries.flatten() {
            let md = match std::fs::symlink_metadata(&entry) {
                Ok(m) => m,
                Err(_) => continue,
            };
            if md.file_type().is_symlink() {
                continue;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = md.permissions();
                if md.is_dir() {
                    perms.set_mode(0o755);
                } else {
                    perms.set_mode(0o644);
                }
                let _ = std::fs::set_permissions(&entry, perms);
            }
            #[cfg(not(unix))]
            {
                let _ = std::fs::set_permissions(&entry, md.permissions());
            }
        }
    }
    Ok(())
}

fn is_safe_tar_member<R: std::io::Read>(member: &tar::Entry<'_, R>, target_dir: &Path) -> bool {
    // Reject symlink/hardlink members outright: a name-safe path says nothing
    // about the link target (arXiv tarballs are semi-trusted, agents are not).
    let entry_type = member.header().entry_type();
    if entry_type.is_symlink() || entry_type.is_hard_link() {
        return false;
    }

    let path = match member.path() {
        Ok(p) => p,
        Err(_) => return false,
    };

    for component in path.components() {
        use std::path::Component;
        match component {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return false,
            _ => {}
        }
    }

    let target = match target_dir.canonicalize() {
        Ok(t) => t,
        Err(_) => return false,
    };

    let joined = target.join(&*path);
    joined.starts_with(&target)
}

fn extract_tar(tar_path: &Path, target_dir: &Path) -> Result<()> {
    let file = std::fs::File::open(tar_path).map_err(ArxivError::Io)?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);

    let entries = archive.entries().map_err(ArxivError::Io)?;
    for entry in entries {
        let mut entry = entry.map_err(ArxivError::Io)?;
        if !is_safe_tar_member(&entry, target_dir) {
            continue;
        }
        entry.unpack_in(target_dir).map_err(ArxivError::Io)?;
    }

    force_uniform_permissions(target_dir)?;

    Ok(())
}

fn move_to_target(source: &Path, target: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(target)?;

    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let dest = target.join(entry.file_name());
        if dest.exists() {
            if dest.is_dir() {
                std::fs::remove_dir_all(&dest)?;
            } else {
                std::fs::remove_file(&dest)?;
            }
        }
        std::fs::rename(entry.path(), &dest)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_cache_requires_documentclass_main() {
        // main.tex without \documentclass is not trusted (shared find_main_tex
        // semantics); a top-level paper.tex with \documentclass wins.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("main.tex"), "section content").unwrap();
        assert!(!validate_cache(dir.path()).unwrap());

        std::fs::write(dir.path().join("paper.tex"), "\\documentclass{article}").unwrap();
        assert!(validate_cache(dir.path()).unwrap());
    }

    #[test]
    fn extract_tar_rejects_symlink_member() {
        // A name-safe symlink can still point outside the target: reject it.
        let dir = tempfile::tempdir().unwrap();
        let tgz = dir.path().join("evil.tar.gz");
        let file = std::fs::File::create(&tgz).unwrap();
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
        let mut builder = tar::Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        header.set_path("evil_link").unwrap();
        header.set_size(0);
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_mode(0o777);
        header.set_cksum();
        builder
            .append_link(
                &mut header,
                "evil_link",
                "/tmp/arxivcat_outside_target_marker",
            )
            .unwrap();
        builder.into_inner().unwrap().finish().unwrap();

        let target = tempfile::tempdir().unwrap();
        extract_tar(&tgz, target.path()).unwrap();
        assert!(
            !target.path().join("evil_link").exists(),
            "symlink member must not be unpacked"
        );
        assert!(
            !std::path::Path::new("/tmp/arxivcat_outside_target_marker").exists(),
            "symlink target must not be touched"
        );
    }

    #[test]
    fn extract_tar_normalizes_bad_modes() {
        // arXiv tarballs may carry 000/0400 modes; after extraction the tree
        // must be readable (files 0644).
        let dir = tempfile::tempdir().unwrap();
        let tgz = dir.path().join("mode.tar.gz");
        let file = std::fs::File::create(&tgz).unwrap();
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
        let mut builder = tar::Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        header.set_path("locked.tex").unwrap();
        header.set_size(4);
        header.set_mode(0o400); // read-only, no write for owner
        header.set_cksum();
        builder.append(&header, b"text".as_slice()).unwrap();
        builder.into_inner().unwrap().finish().unwrap();

        let target = tempfile::tempdir().unwrap();
        extract_tar(&tgz, target.path()).unwrap();
        let path = target.path().join("locked.tex");
        assert!(path.exists());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "text");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o644, "extracted file mode normalized to 0644");
        }
    }

    fn make_tar_with_file(name: &str, content: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let tgz = dir.path().join("test.tar.gz");
        let file = std::fs::File::create(&tgz).unwrap();
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
        let mut builder = tar::Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        header.set_path(name).unwrap();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append(&header, content).unwrap();
        builder.into_inner().unwrap().finish().unwrap();
        (dir, tgz)
    }

    fn with_first_entry<R>(
        tgz: &Path,
        f: impl FnOnce(&tar::Entry<'_, flate2::read::GzDecoder<std::fs::File>>) -> R,
    ) -> R {
        let file = std::fs::File::open(tgz).unwrap();
        let decoder = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);
        let entry = archive.entries().unwrap().next().unwrap().unwrap();
        f(&entry)
    }

    #[test]
    fn test_is_safe_tar_member_accepts_regular_file() {
        let (_dir, tgz) = make_tar_with_file("main.tex", b"\\documentclass{article}");
        let target = tempfile::tempdir().unwrap();
        let result = with_first_entry(&tgz, |entry| is_safe_tar_member(entry, target.path()));
        assert!(result);
    }

    #[test]
    fn test_is_safe_tar_member_accepts_subdir_file() {
        let (_dir, tgz) = make_tar_with_file("sec/intro.tex", b"intro");
        let target = tempfile::tempdir().unwrap();
        let result = with_first_entry(&tgz, |entry| is_safe_tar_member(entry, target.path()));
        assert!(result);
    }

    #[test]
    fn test_extract_tar_extracts_files() {
        let (_dir, tgz) = make_tar_with_file("main.tex", b"\\documentclass{article}\nHello world");
        let target = tempfile::tempdir().unwrap();
        extract_tar(&tgz, target.path()).unwrap();

        let main = target.path().join("main.tex");
        assert!(main.exists());
        assert_eq!(
            std::fs::read_to_string(&main).unwrap(),
            "\\documentclass{article}\nHello world"
        );
    }

    #[test]
    fn test_extract_tar_extracts_nested_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let tgz = dir.path().join("nested.tar.gz");
        let file = std::fs::File::create(&tgz).unwrap();
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
        let mut builder = tar::Builder::new(encoder);

        let mut header = tar::Header::new_gnu();
        header.set_path("sec/").unwrap();
        header.set_size(0);
        header.set_entry_type(tar::EntryType::Directory);
        header.set_mode(0o755);
        header.set_cksum();
        builder.append(&header, &[][..]).unwrap();

        let mut header = tar::Header::new_gnu();
        header.set_path("sec/intro.tex").unwrap();
        header.set_size(4);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append(&header, "text".as_bytes()).unwrap();

        builder.into_inner().unwrap().finish().unwrap();

        let target = tempfile::tempdir().unwrap();
        extract_tar(&tgz, target.path()).unwrap();

        assert!(target.path().join("sec/intro.tex").exists());
        assert_eq!(
            std::fs::read_to_string(target.path().join("sec/intro.tex")).unwrap(),
            "text"
        );
    }
}
