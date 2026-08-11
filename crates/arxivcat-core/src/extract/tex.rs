use std::path::{Path, PathBuf};

use regex::Regex;

use crate::error::{ArxivError, Result};
use crate::extract::ExtractionOutput;

pub fn find_main_tex(paper_dir: &Path) -> Option<PathBuf> {
    let main_candidate = paper_dir.join("main.tex");
    if main_candidate.exists() {
        return Some(main_candidate);
    }

    let glob_pattern = format!("{}/*.tex", paper_dir.display());
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

pub fn strip_latex_comments(tex_content: &str) -> String {
    let mut result = String::with_capacity(tex_content.len());
    let chars = tex_content.char_indices().peekable();
    let mut skip_to_eol = false;

    for (i, ch) in chars {
        if skip_to_eol {
            if ch == '\n' {
                skip_to_eol = false;
                result.push('\n');
            }
            continue;
        }

        if ch == '%' {
            if i > 0 {
                let byte_before = tex_content.as_bytes().get(i - 1);
                if byte_before == Some(&b'\\') {
                    result.push('%');
                    continue;
                }
            }
            skip_to_eol = true;
            continue;
        }

        result.push(ch);
    }

    result
}

pub fn expand_inputs(
    tex_content: &str,
    base_dir: &Path,
    seen: Option<&mut std::collections::HashSet<PathBuf>>,
    root_dir: Option<&Path>,
) -> Result<String> {
    let root_dir = root_dir.unwrap_or(base_dir);
    let stripped = strip_latex_comments(tex_content);

    let input_re = Regex::new(r"\\(?:input|include)\s*\{([^}]+)\}")
        .map_err(|e| ArxivError::Parse(e.to_string()))?;

    let mut owned_seen;
    let seen = match seen {
        Some(s) => s,
        None => {
            owned_seen = std::collections::HashSet::new();
            &mut owned_seen
        }
    };

    let base_dir_owned = base_dir.to_path_buf();
    let root_dir_owned = root_dir.to_path_buf();

    let result = input_re
        .replace_all(&stripped, |caps: &regex::Captures| {
            let filename = caps.get(1).map(|m| m.as_str().trim()).unwrap_or("");
            if filename.is_empty() {
                return caps[0].to_string();
            }

            let candidates: Vec<PathBuf> = {
                let mut v = vec![
                    base_dir_owned.join(filename),
                    root_dir_owned.join(filename),
                ];
                if !filename.ends_with(".tex") {
                    v.push(base_dir_owned.join(format!("{filename}.tex")));
                    v.push(root_dir_owned.join(format!("{filename}.tex")));
                }
                v
            };

            let resolved = candidates.iter().find(|p| p.exists()).cloned();
            let resolved = match resolved {
                Some(r) => r,
                None => return caps[0].to_string(),
            };

            if seen.contains(&resolved) {
                return caps[0].to_string();
            }

            let content = match std::fs::read_to_string(&resolved) {
                Ok(c) => c,
                Err(_) => return caps[0].to_string(),
            };

            seen.insert(resolved.clone());

            let parent = resolved.parent().unwrap_or(&root_dir_owned);
            match expand_inputs(&content, parent, Some(seen), Some(&root_dir_owned)) {
                Ok(expanded) => expanded,
                Err(_) => caps[0].to_string(),
            }
        })
        .to_string();

    Ok(result)
}

pub fn extract_body_and_appendix(tex_content: &str) -> Result<(String, Option<String>)> {
    let abstract_re = Regex::new(r"\\begin\{abstract\}")
        .map_err(|e| ArxivError::Parse(e.to_string()))?;
    let section_re = Regex::new(r"\\section\s*[\*]?\s*\{")
        .map_err(|e| ArxivError::Parse(e.to_string()))?;
    let doc_begin_re = Regex::new(r"\\begin\{document\}")
        .map_err(|e| ArxivError::Parse(e.to_string()))?;
    let doc_end_re = Regex::new(r"\\end\{document\}")
        .map_err(|e| ArxivError::Parse(e.to_string()))?;
    let conclusion_re = Regex::new(
        r"\\section\*?\s*\{[^}]*?(?:[Cc]onclusion|[Ss]ummary)[^}]*\}",
    )
    .map_err(|e| ArxivError::Parse(e.to_string()))?;
    let appendix_re = Regex::new(r"\\appendix(?:\s|$)|\\begin\{appendix\}")
        .map_err(|e| ArxivError::Parse(e.to_string()))?;
    let bibliography_re = Regex::new(r"\\bibliography(?:style)?\s*\{")
        .map_err(|e| ArxivError::Parse(e.to_string()))?;

    let doc_end = doc_end_re.find(tex_content).map(|m| m.start());

    let abstract_match = abstract_re.find(tex_content);
    let first_section = section_re.find(tex_content);

    let start = match (&abstract_match, &first_section) {
        (Some(a), Some(s)) => a.start().min(s.start()),
        (Some(a), None) => a.start(),
        (None, Some(s)) => s.start(),
        (None, None) => {
            if let Some(doc) = doc_begin_re.find(tex_content) {
                doc.end()
            } else {
                return Err(ArxivError::Extraction(
                    "Could not find abstract, first section, or document start".into(),
                ));
            }
        }
    };

    let appendix_pos = appendix_re.find(tex_content).map(|m| m.start());
    let bib_pos = bibliography_re.find(tex_content).map(|m| m.start());

    let mut candidates: Vec<usize> = Vec::new();
    if let Some(p) = appendix_pos {
        if p > start {
            candidates.push(p);
        }
    }
    if let Some(p) = bib_pos {
        if p > start {
            candidates.push(p);
        }
    }

    let body_end = if !candidates.is_empty() {
        *candidates.iter().min().unwrap()
    } else {
        let conclusion_matches: Vec<_> = conclusion_re.find_iter(tex_content).collect();
        if let Some(last_conc) = conclusion_matches.last() {
            let after_conc = &tex_content[last_conc.end()..];
            let next_section = Regex::new(r"\\section\s*[\*]?\s*\{").unwrap();
            let next_chapter = Regex::new(r"\\chapter\s*[\*]?\s*\{").unwrap();

            let ns = next_section.find(after_conc);
            let nc = next_chapter.find(after_conc);
            let next_boundary = [ns.map(|m| m.start()), nc.map(|m| m.start())]
                .iter()
                .flatten()
                .min()
                .copied();

            match next_boundary {
                Some(offset) => last_conc.end() + offset,
                None => doc_end.unwrap_or(tex_content.len()),
            }
        } else {
            doc_end.unwrap_or(tex_content.len())
        }
    };

    let body = tex_content[start..body_end].trim().to_string();

    let appendix_content = if body_end < tex_content.len() {
        let end = doc_end.unwrap_or(tex_content.len());
        let raw = &tex_content[body_end..end];

        let cleaned = Regex::new(r"\\bibliography(?:style)?\s*\{[^}]*\}")
            .unwrap()
            .replace_all(raw, "")
            .to_string();
        let cleaned = Regex::new(r"\\clearpage")
            .unwrap()
            .replace_all(&cleaned, "")
            .to_string();

        let trimmed = cleaned.trim().to_string();
        if trimmed.len() > 50 {
            Some(trimmed)
        } else {
            None
        }
    } else {
        None
    };

    Ok((body, appendix_content))
}

pub fn extract_body_from_dir(paper_dir: &Path, output_dir: &Path) -> Result<ExtractionOutput> {
    let main_tex = find_main_tex(paper_dir).ok_or_else(|| {
        ArxivError::Extraction(format!(
            "main tex not found in {}",
            paper_dir.display()
        ))
    })?;

    let content = std::fs::read_to_string(&main_tex)
        .map_err(ArxivError::Io)?;

    let expanded = expand_inputs(&content, paper_dir, None, None)?;

    let leftover_re = Regex::new(r"\\(?:input|include)\s*\{").unwrap();
    if leftover_re.is_match(&expanded) {
        return Err(ArxivError::Extraction(
            "unresolved \\input/\\include references remain after expansion".into(),
        ));
    }

    let (body, appendix) = extract_body_and_appendix(&expanded)?;

    std::fs::create_dir_all(output_dir)?;

    let body_path = output_dir.join("body.tex");
    std::fs::write(&body_path, &body)?;

    let appendix_path = if let Some(ref app) = appendix {
        let p = output_dir.join("appendix.tex");
        std::fs::write(&p, app)?;
        Some(p)
    } else {
        None
    };

    Ok(ExtractionOutput {
        body,
        appendix,
        body_path,
        appendix_path,
        pdf_path: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_latex_comments() {
        let input = "hello % this is a comment\nworld % another\nfoo";
        let result = strip_latex_comments(input);
        assert_eq!(result.trim(), "hello \nworld \nfoo");
    }

    #[test]
    fn test_strip_latex_comments_escaped_percent() {
        let input = r"100\% complete % comment";
        let result = strip_latex_comments(input);
        assert!(result.contains("100\\% complete"));
    }

    #[test]
    fn test_find_main_tex_prefers_main_dot_tex() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("main.tex"), "\\documentclass{article}").unwrap();
        std::fs::write(dir.path().join("other.tex"), "\\documentclass{article}").unwrap();
        assert_eq!(
            find_main_tex(dir.path()),
            Some(dir.path().join("main.tex"))
        );
    }

    #[test]
    fn test_find_main_tex_scans_for_documentclass() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("paper.tex"), "\\documentclass{article}\nHello").unwrap();
        assert_eq!(
            find_main_tex(dir.path()),
            Some(dir.path().join("paper.tex"))
        );
    }

    #[test]
    fn test_find_main_tex_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("empty.tex"), "no documentclass here").unwrap();
        assert_eq!(find_main_tex(dir.path()), None);
    }

    #[test]
    fn test_expand_inputs_flat() {
        let dir = tempfile::tempdir().unwrap();
        let main = dir.path().join("main.tex");
        let sub = dir.path().join("intro.tex");

        std::fs::write(&main, "\\documentclass{article}\n\\input{intro}\n\\end{document}").unwrap();
        std::fs::write(&sub, "Introduction text here.").unwrap();

        let result = expand_inputs(
            &std::fs::read_to_string(&main).unwrap(),
            dir.path(),
            None,
            None,
        )
        .unwrap();

        assert!(result.contains("Introduction text here."));
        assert!(!result.contains("\\input{intro}"));
    }

    #[test]
    fn test_expand_inputs_cycle_detection() {
        let dir = tempfile::tempdir().unwrap();
        let main = dir.path().join("main.tex");
        let a = dir.path().join("a.tex");

        std::fs::write(&main, "\\input{a}").unwrap();
        std::fs::write(&a, "\\input{main} content").unwrap();

        let result = expand_inputs(
            &std::fs::read_to_string(&main).unwrap(),
            dir.path(),
            None,
            None,
        )
        .unwrap();

        assert!(result.contains("content"));
        assert!(!result.contains("\\input{main}"));
    }

    #[test]
    fn test_extract_body_and_appendix() {
        let content = r"\documentclass{article}
\begin{document}
\begin{abstract}
This is the abstract.
\end{abstract}
\section{Introduction}
Some introduction text here with enough length.
\section{Conclusion}
We conclude this paper with some remarks.
\appendix
\section{Appendix A}
Extra details and supplementary material that goes beyond the main text of the paper.
\end{document}";

        let (body, appendix) = extract_body_and_appendix(content).unwrap();
        assert!(body.contains("This is the abstract."));
        assert!(body.contains("Introduction"));
        assert!(appendix.is_some());
        if let Some(app) = appendix {
            assert!(app.contains("Extra details"));
        }
    }

    #[test]
    fn test_extract_body_no_appendix() {
        let content = r"\documentclass{article}
\begin{document}
\begin{abstract}
Abstract text.
\end{abstract}
\section{Body}
Body text.
\end{document}";

        let (body, appendix) = extract_body_and_appendix(content).unwrap();
        assert!(body.contains("Abstract text."));
        assert!(appendix.is_none());
    }
}
