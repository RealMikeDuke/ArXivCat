#[cfg(test)]
mod integration_tests {
    use arxivcat_core::chat::*;
    use arxivcat_core::config::*;
    use arxivcat_core::extract::arxiv::*;
    use arxivcat_core::extract::tex::*;
    use arxivcat_core::workspace::*;
    use std::path::Path;

    // ─── arXiv ID Parsing Edge Cases ───

    #[test]
    fn arxiv_id_mixed_inputs() {
        assert_eq!(
            extract_arxiv_id("arxiv.org/abs/2501.12948"),
            Some("2501.12948".into())
        );
        assert_eq!(
            extract_arxiv_id("arXiv:2501.12948"),
            Some("2501.12948".into())
        );
        assert_eq!(extract_arxiv_id("2501_12948"), Some("2501.12948".into()));
        assert_eq!(
            extract_arxiv_id("2501.12948v3"),
            Some("2501.12948v3".into())
        );
        assert_eq!(extract_arxiv_id(" 2501.12948 "), Some("2501.12948".into()));
        assert_eq!(
            extract_arxiv_id("http://arxiv.org/abs/2501.12948"),
            Some("2501.12948".into())
        );
        assert_eq!(extract_arxiv_id(""), None);
        assert_eq!(extract_arxiv_id("arxiv"), None);
    }

    // ─── arXiv URL Robustness ───

    #[test]
    fn arxiv_id_url_pdf_format() {
        assert_eq!(
            extract_arxiv_id("arxiv.org/pdf/2501.12948"),
            Some("2501.12948".into())
        );
        assert_eq!(
            extract_arxiv_id("https://arxiv.org/pdf/2501.12948"),
            Some("2501.12948".into())
        );
        assert_eq!(
            extract_arxiv_id("https://arxiv.org/pdf/2501.12948.pdf"),
            Some("2501.12948".into())
        );
    }

    #[test]
    fn arxiv_id_url_versioned() {
        assert_eq!(
            extract_arxiv_id("https://arxiv.org/abs/2501.12948v2"),
            Some("2501.12948v2".into())
        );
        assert_eq!(
            extract_arxiv_id("arxiv.org/pdf/2501.12948v3"),
            Some("2501.12948v3".into())
        );
    }

    #[test]
    fn arxiv_id_url_www_prefix() {
        assert_eq!(
            extract_arxiv_id("www.arxiv.org/abs/2501.12948"),
            Some("2501.12948".into())
        );
        assert_eq!(
            extract_arxiv_id("http://www.arxiv.org/pdf/2501.12948"),
            Some("2501.12948".into())
        );
    }

    #[test]
    fn arxiv_id_url_trailing_slash() {
        assert_eq!(
            extract_arxiv_id("https://arxiv.org/abs/2501.12948/"),
            Some("2501.12948".into())
        );
    }

    #[test]
    fn arxiv_id_url_whitespace_around() {
        assert_eq!(
            extract_arxiv_id("  https://arxiv.org/abs/2501.12948  "),
            Some("2501.12948".into())
        );
        assert_eq!(
            extract_arxiv_id("\thttps://arxiv.org/abs/2501.12948\n"),
            Some("2501.12948".into())
        );
    }

    // ─── Filename Sanitization ───

    #[test]
    fn sanitize_edge_cases() {
        assert_eq!(sanitize_filename(""), "untitled");
        assert_eq!(sanitize_filename("a"), "a");
        assert_eq!(sanitize_filename("hello  world"), "hello_world");
        assert_eq!(sanitize_filename("___test___"), "test");
        assert_eq!(
            sanitize_filename("test:file<name>.txt"),
            "test_file_name_.txt"
        );
    }

    // ─── TeX Processing ───

    #[test]
    fn strip_comments_with_escaped_percent() {
        let input = r"Value is 100\% of total % this is a comment";
        let result = strip_latex_comments(input);
        assert!(result.contains("100\\% of total"));
        assert!(!result.contains("this is a comment"));
    }

    #[test]
    fn strip_comments_multiline() {
        let input = "line1 % comment1\nline2\n% full line comment\nline3 % comment3";
        let result = strip_latex_comments(input);
        assert!(result.contains("line1"));
        assert!(result.contains("line2"));
        assert!(result.contains("line3"));
        assert!(!result.contains("comment1"));
        assert!(!result.contains("full line comment"));
    }

    #[test]
    fn expand_inputs_nested() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("sec")).unwrap();

        let main = dir.path().join("main.tex");
        let intro = dir.path().join("sec").join("intro.tex");
        let bg = dir.path().join("sec").join("bg.tex");

        std::fs::write(
            &main,
            "\\documentclass{article}\n\\input{sec/intro}\n\\end{document}",
        )
        .unwrap();
        std::fs::write(&intro, "Introduction. \\input{bg}").unwrap();
        std::fs::write(&bg, "Background text.").unwrap();

        let result = expand_inputs(
            &std::fs::read_to_string(&main).unwrap(),
            dir.path(),
            None,
            None,
        )
        .unwrap();

        assert!(result.contains("Introduction"));
        assert!(result.contains("Background text"));
        assert!(!result.contains("\\input{sec/intro}"));
    }

    #[test]
    fn expand_inputs_missing_file_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let main = dir.path().join("main.tex");
        std::fs::write(&main, "\\input{does_not_exist}\nHello").unwrap();

        let result = expand_inputs(
            &std::fs::read_to_string(&main).unwrap(),
            dir.path(),
            None,
            None,
        )
        .unwrap();

        assert!(result.contains("\\input{does_not_exist}"));
        assert!(result.contains("Hello"));
    }

    #[test]
    fn body_appendix_with_bibliography() {
        let content = r"\section{Introduction}
Main text.
\section{Results}
Results here.
\bibliographystyle{plain}
\bibliography{refs}
\appendix
\section{Proofs}
Detailed proofs that are definitely long enough to exceed the fifty character threshold for appendix detection.";
        let (body, appendix) = extract_body_and_appendix(content).unwrap();
        assert!(body.contains("Main text"));
        assert!(body.contains("Results here"));
        assert!(appendix.is_some());
        if let Some(app) = appendix {
            assert!(app.contains("Proofs"));
            assert!(!app.contains("\\bibliography"));
        }
    }

    #[test]
    fn body_without_appendix_or_bibliography() {
        let content = r"\section{Introduction}
Text here.
\section{Conclusion}
We conclude.";
        let (body, appendix) = extract_body_and_appendix(content).unwrap();
        assert!(!body.is_empty());
        assert!(appendix.is_none());
    }

    #[test]
    fn expand_inputs_cycle_with_deep_path() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.tex");
        let b = dir.path().join("b.tex");
        let c = dir.path().join("c.tex");

        std::fs::write(&a, "A \\input{b}").unwrap();
        std::fs::write(&b, "B \\input{c}").unwrap();
        std::fs::write(&c, "C \\input{a}").unwrap();

        let result = expand_inputs(
            &std::fs::read_to_string(&a).unwrap(),
            dir.path(),
            None,
            None,
        )
        .unwrap();

        assert!(result.contains("A"));
        assert!(result.contains("B"));
        assert!(result.contains("C"));
        assert!(result.contains("\\input{b}"));
        assert!(!result.contains("\\input{c}"));
    }

    // ─── Workspace ───

    #[test]
    fn workspace_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let ws = Workspace::open(dir.path()).unwrap();
        assert!(ws.papers.is_empty());
    }

    #[test]
    fn workspace_with_mixed_papers() {
        let dir = tempfile::tempdir().unwrap();

        let complete = dir.path().join("2501_12948_Complete");
        let pending = dir.path().join("2412_04445_Pending");
        let hidden = dir.path().join(".hidden_paper");

        std::fs::create_dir(&complete).unwrap();
        std::fs::create_dir(&pending).unwrap();
        std::fs::create_dir(&hidden).unwrap();

        std::fs::write(complete.join("body.tex"), "content").unwrap();
        std::fs::write(complete.join("description.md"), "desc").unwrap();
        std::fs::write(complete.join(".description_ready"), "ok\n").unwrap();

        std::fs::write(pending.join("body.tex"), "content").unwrap();

        let ws = Workspace::open(dir.path()).unwrap();
        assert_eq!(ws.papers.len(), 2);
        // New semantics: is_complete == has_body (AI decoupled).
        // description_ready is a separate informational state.
        assert!(ws.papers[0].is_complete);
        assert!(ws.papers[0].description_ready);
        assert!(ws.papers[1].is_complete);
        assert!(!ws.papers[1].description_ready);
    }

    #[test]
    fn find_paper_by_id_partial_match() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("2501_12948_Test_Paper");
        std::fs::create_dir(&p).unwrap();
        std::fs::write(p.join("body.tex"), "x").unwrap();

        let ws = Workspace::open(dir.path()).unwrap();
        assert!(ws.find_paper_by_id("2501.12948").is_some());
        assert!(ws.find_paper_by_id("2501_12948").is_some());
        assert!(ws.find_paper_by_id("nonexistent").is_none());
    }

    // ─── Config ───

    #[test]
    fn config_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("APPDATA", dir.path().to_string_lossy().to_string());

        save_token("test-token-123").unwrap();
        let token = load_cached_token().unwrap();
        assert_eq!(token, "test-token-123");

        save_model_preference("Pro").unwrap();
        assert_eq!(load_model_preference(), "Pro");

        save_workspace_path(Path::new("/test/ws")).unwrap();
        assert_eq!(load_workspace_path(), Some("/test/ws".into()));
    }

    // ─── Chat Session ───

    #[test]
    fn chat_session_save_load_empty_messages_skips() {
        let dir = tempfile::tempdir().unwrap();
        let mut session = arxivcat_core::chat::session::ChatSession::new("paper", "test");
        // empty messages - save should be a no-op
        let result = arxivcat_core::chat::session::save_session(&mut session, Some(dir.path()));
        assert!(result.is_ok());
        // should not have created a file since messages are empty
        let sessions = arxivcat_core::chat::session::list_sessions(dir.path()).unwrap();
        assert!(sessions.is_empty());
    }

    #[test]
    fn list_sessions_returns_empty_for_nonexistent_dir() {
        let sessions =
            arxivcat_core::chat::session::list_sessions(Path::new("/nonexistent/path")).unwrap();
        assert!(sessions.is_empty());
    }

    // ─── Context Building ───

    #[test]
    fn side_chat_context_with_selection() {
        use arxivcat_core::chat::build_side_chat_context;
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("body.tex"), "body content").unwrap();
        std::fs::write(dir.path().join("note.txt"), "my note").unwrap();

        let selection = ContextSelection {
            body: true,
            appendix: false,
            description: false,
            note: true,
        };

        let ctx = build_side_chat_context(dir.path(), &selection);
        assert!(ctx.contains("body content"));
        assert!(ctx.contains("my note"));
        assert!(!ctx.contains("appendix"));
    }

    // ─── P0.8: lossy reads + unexpanded-reference warnings ───

    #[test]
    fn extract_body_from_dir_non_utf8_lossy() {
        use arxivcat_core::extract::tex::extract_body_from_dir;
        let dir = tempfile::tempdir().unwrap();
        // latin-1 content (0xE9 = é in latin-1, invalid UTF-8 alone)
        let bytes = br"\documentclass{article}\n\begin{document}\nCaf\xe9 test\n\end{document}";
        std::fs::write(dir.path().join("main.tex"), bytes).unwrap();

        let out_dir = tempfile::tempdir().unwrap();
        let out = extract_body_from_dir(dir.path(), out_dir.path()).unwrap();
        assert!(out.body.contains("Caf"));
        assert!(out.warnings.is_empty());
    }

    #[test]
    fn extract_body_from_dir_unexpanded_input_warns_not_fails() {
        use arxivcat_core::extract::tex::extract_body_from_dir;
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("main.tex"),
            "\\documentclass{article}\n\\begin{document}\n\\input{missing}\n\\end{document}",
        )
        .unwrap();

        let out_dir = tempfile::tempdir().unwrap();
        let out = extract_body_from_dir(dir.path(), out_dir.path()).unwrap();
        assert!(
            out.warnings.iter().any(|w| w.contains("unresolved")),
            "expected unresolved-input warning, got {out:?}"
        );
        assert!(out.body.starts_with("% [arxivcat] unexpanded"));
    }

    #[test]
    fn extract_body_from_dir_subfile_warns() {
        use arxivcat_core::extract::tex::extract_body_from_dir;
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("main.tex"),
            "\\documentclass{article}\n\\begin{document}\n\\subfile{chapters/intro}\n\\end{document}",
        )
        .unwrap();

        let out_dir = tempfile::tempdir().unwrap();
        let out = extract_body_from_dir(dir.path(), out_dir.path()).unwrap();
        assert!(
            out.warnings.iter().any(|w| w.contains("subfile")),
            "expected subfile warning, got {out:?}"
        );
    }

    #[test]
    fn old_gui_session_json_still_deserializes() {
        // P0.12: ChatSession no longer has locked_fields / context_snapshot /
        // view_name (GUI-era). serde must ignore those unknown keys when
        // reading session files written by the old GUI version.
        let json = r#"{
            "title": "2501.12948 2026-01-01 12:00",
            "kind": "paper",
            "model": "Flash",
            "reasoning_effort": "low",
            "locked_fields": {"2501_12948_Test": ["body"]},
            "messages": [{"speaker": "user", "content": "hi"}],
            "context_selection": {"body": true, "appendix": false, "description": false, "note": false},
            "context_snapshot": "body:\n...",
            "view_name": "body",
            "updated_at": "2026-01-01T12:00:00"
        }"#;
        let session: arxivcat_core::chat::session::ChatSession =
            serde_json::from_str(json).unwrap();
        assert_eq!(session.messages.len(), 1);
        assert_eq!(session.title, "2501.12948 2026-01-01 12:00");
        assert_eq!(session.kind, "paper");
        assert_eq!(session.model, "Flash");
    }

    #[test]
    fn side_chat_context_empty_returns_placeholder() {
        let dir = tempfile::tempdir().unwrap();
        let selection = ContextSelection::default();
        let ctx = build_side_chat_context(dir.path(), &selection);
        assert!(!ctx.is_empty());
    }
}
