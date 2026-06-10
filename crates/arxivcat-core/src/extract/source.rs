use std::path::{Path, PathBuf};

use crate::error::{ArxivError, Result};
use crate::extract::arxiv::{fetch_title_from_arxiv, sanitize_filename};

pub async fn download_source(
    arxiv_id: &str,
    downloads_dir: &Path,
) -> Result<(Option<PathBuf>, Option<String>)> {
    std::fs::create_dir_all(downloads_dir)?;

    let title = fetch_title_from_arxiv(arxiv_id).await?.unwrap_or_else(|| "unknown".to_string());
    let folder_name = format!(
        "{}_{}",
        arxiv_id.replace('.', "_"),
        sanitize_filename(&title)
    );

    let paper_dir = downloads_dir.join(&folder_name);

    if paper_dir.exists() {
        if validate_cache(&paper_dir)? {
            return Ok((Some(paper_dir), Some(folder_name)));
        }
        repair_permissions(&paper_dir)?;
        if validate_cache(&paper_dir)? {
            return Ok((Some(paper_dir), Some(folder_name)));
        }
        match std::fs::remove_dir_all(&paper_dir) {
            Ok(_) => {}
            Err(_) => {
                return Ok((None, Some(fresh_folder_name(downloads_dir, &folder_name)?)));
            }
        }
    }

    let tar_url = format!("https://arxiv.org/src/{arxiv_id}");
    let client = reqwest::Client::new();
    let response = client
        .get(&tar_url)
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| ArxivError::Http(e))?;

    if !response.status().is_success() {
        return Err(ArxivError::Other(format!(
            "failed to download source: HTTP {}",
            response.status()
        )));
    }

    let tar_path = downloads_dir.join(format!("{arxiv_id}.tar.gz"));
    let bytes = response
        .bytes()
        .await
        .map_err(|e| ArxivError::Http(e))?;
    std::fs::write(&tar_path, &bytes)?;

    let temp_dir = tempfile::Builder::new()
        .prefix(&format!("{arxiv_id}_"))
        .tempdir_in(downloads_dir)?;

    extract_tar(&tar_path, temp_dir.path())?;

    let _ = std::fs::remove_file(&tar_path);

    match move_to_target(temp_dir.path(), &paper_dir) {
        Ok(()) => Ok((Some(paper_dir), Some(folder_name))),
        Err(_) => {
            let fresh_name = fresh_folder_name(downloads_dir, &folder_name)?;
            let fresh_dir = downloads_dir.join(&fresh_name);
            move_to_target(temp_dir.path(), &fresh_dir)?;
            Ok((Some(fresh_dir), Some(fresh_name)))
        }
    }
}

pub async fn download_pdf(arxiv_id: &str, output_dir: &Path) -> Result<Option<PathBuf>> {
    let pdf_path = output_dir.join(format!("{arxiv_id}.pdf"));
    if pdf_path.exists() {
        return Ok(Some(pdf_path));
    }

    let url = format!("https://arxiv.org/pdf/{arxiv_id}");
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| ArxivError::Http(e))?;

    if !response.status().is_success() {
        return Ok(None);
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| ArxivError::Http(e))?;

    std::fs::create_dir_all(output_dir)?;
    std::fs::write(&pdf_path, &bytes)?;

    Ok(Some(pdf_path))
}

fn validate_cache(paper_dir: &Path) -> Result<bool> {
    let main_tex = find_main_tex_in_dir(paper_dir);
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

fn find_main_tex_in_dir(dir: &Path) -> Option<PathBuf> {
    let main_candidate = dir.join("main.tex");
    if main_candidate.exists() {
        return Some(main_candidate);
    }
    let glob_pattern = format!("{}/*.tex", dir.display());
    if let Ok(entries) = glob::glob(&glob_pattern) {
        for entry in entries.flatten() {
            if let Ok(content) = std::fs::read_to_string(&entry) {
                if content.contains("\\documentclass") {
                    return Some(entry);
                }
            }
        }
    }
    None
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
        if std::fs::read_to_string(&entry).is_err() {
            return false;
        }
    }
    true
}

fn repair_permissions(dir: &Path) -> Result<()> {
    let pattern = format!("{}/**/*", dir.display());
    if let Ok(entries) = glob::glob(&pattern) {
        for entry in entries.flatten() {
            let perms = match std::fs::metadata(&entry) {
                Ok(m) => m.permissions(),
                Err(_) => continue,
            };
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = perms.clone();
                if entry.is_dir() {
                    perms.set_mode(0o755);
                } else {
                    perms.set_mode(0o644);
                }
                let _ = std::fs::set_permissions(&entry, perms);
            }
            #[cfg(not(unix))]
            {
                let _ = std::fs::set_permissions(&entry, perms);
            }
        }
    }
    Ok(())
}

fn is_safe_tar_member<R: std::io::Read>(member: &tar::Entry<'_, R>, target_dir: &Path) -> bool {
    let path = member.path();
    if path.is_err() {
        return false;
    }
    let path = path.unwrap();
    let member_path = target_dir.join(&*path);

    match member_path.canonicalize() {
        Ok(resolved) => {
            let target = match target_dir.canonicalize() {
                Ok(t) => t,
                Err(_) => return false,
            };
            resolved == target || resolved.starts_with(&target)
        }
        Err(_) => false,
    }
}

fn extract_tar(tar_path: &Path, target_dir: &Path) -> Result<()> {
    let file =
        std::fs::File::open(tar_path).map_err(|e| ArxivError::Io(e))?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);

    let entries = archive.entries().map_err(|e| ArxivError::Io(e))?;
    for entry in entries {
        let mut entry = entry.map_err(|e| ArxivError::Io(e))?;
        if !is_safe_tar_member(&entry, target_dir) {
            continue;
        }
        entry
            .unpack_in(target_dir)
            .map_err(|e| ArxivError::Io(e))?;
    }

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

fn fresh_folder_name(base_dir: &Path, original: &str) -> Result<String> {
    for n in 1..100 {
        let candidate = format!("{original}_fresh{n}");
        if !base_dir.join(&candidate).exists() {
            return Ok(candidate);
        }
    }
    Err(ArxivError::Other("cannot find fresh folder name".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_main_tex_prefers_main_dot_tex() {
        let dir = tempfile::tempdir().unwrap();
        let fake_main = dir.path().join("main.tex");
        std::fs::write(&fake_main, "content").unwrap();
        assert_eq!(find_main_tex_in_dir(dir.path()), Some(fake_main));
    }

    #[test]
    fn test_fresh_folder_name_increments() {
        let dir = tempfile::tempdir().unwrap();
        let base = "test_folder";
        std::fs::create_dir(dir.path().join("test_folder_fresh1")).unwrap();
        let name = fresh_folder_name(dir.path(), base).unwrap();
        assert_eq!(name, "test_folder_fresh2");
    }
}
