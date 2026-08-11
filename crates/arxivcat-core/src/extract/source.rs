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
        .map_err(ArxivError::Http)?;

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
        .map_err(ArxivError::Http)?;
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
        .map_err(ArxivError::Http)?;

    if !response.status().is_success() {
        return Ok(None);
    }

    let bytes = response
        .bytes()
        .await
        .map_err(ArxivError::Http)?;

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
    let file =
        std::fs::File::open(tar_path).map_err(ArxivError::Io)?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);

    let entries = archive.entries().map_err(ArxivError::Io)?;
    for entry in entries {
        let mut entry = entry.map_err(ArxivError::Io)?;
        if !is_safe_tar_member(&entry, target_dir) {
            continue;
        }
        entry
            .unpack_in(target_dir)
            .map_err(ArxivError::Io)?;
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
    fn test_validate_cache_requires_documentclass_main() {
        // main.tex without \documentclass is not trusted (shared find_main_tex
        // semantics); a top-level paper.tex with \documentclass wins.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("main.tex"), "section content").unwrap();
        assert!(validate_cache(dir.path()).unwrap() == false);

        std::fs::write(dir.path().join("paper.tex"), "\\documentclass{article}").unwrap();
        assert!(validate_cache(dir.path()).unwrap());
    }

    #[test]
    fn test_fresh_folder_name_increments() {
        let dir = tempfile::tempdir().unwrap();
        let base = "test_folder";
        std::fs::create_dir(dir.path().join("test_folder_fresh1")).unwrap();
        let name = fresh_folder_name(dir.path(), base).unwrap();
        assert_eq!(name, "test_folder_fresh2");
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

    fn with_first_entry<R>(tgz: &Path, f: impl FnOnce(&tar::Entry<'_, flate2::read::GzDecoder<std::fs::File>>) -> R) -> R {
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
        let result = with_first_entry(&tgz, |entry| {
            is_safe_tar_member(entry, target.path())
        });
        assert!(result);
    }

    #[test]
    fn test_is_safe_tar_member_accepts_subdir_file() {
        let (_dir, tgz) = make_tar_with_file("sec/intro.tex", b"intro");
        let target = tempfile::tempdir().unwrap();
        let result = with_first_entry(&tgz, |entry| {
            is_safe_tar_member(entry, target.path())
        });
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
