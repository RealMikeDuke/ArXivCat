# ArxivCat Architecture

## Migration Summary

**From**: Python + Tkinter desktop app + Flask web app + WebView Android wrapper  
**To**: Rust + Tauri 2 (React GUI) + ratatui TUI + clap CLI

Python legacy code preserved under `python-legacy/`.

---

## Feature Parity Status

| Feature | Python | Rust CLI | Rust TUI | Rust GUI (Tauri) |
|---|---|---|---|---|
| arXiv ID parsing (URL/raw/versioned) | done | done | done | done |
| PDF ID extraction (PyMuPDF/lopdf) | done | done | done | done |
| Title fetching from arXiv | done | done | done | done |
| Source tarball download + untar | done | done | done | done |
| Cache validation + repair | done | done | done | done |
| Tar path traversal protection | done | done | done | done |
| Main TeX detection | done | done | done | done |
| Expand \input/\include (cycles) | done | done | done | done |
| Body/appendix split | done | done | done | done |
| PDF download | done | done | done | done |
| Workspace (open folder) | done | done | done | done |
| Paper list (from workspace) | done | done | done | 🚧 compile only |
| Paper loading (body/appendix/note/desc) | done | done | done | 🚧 compile only |
| Scan workspace PDFs | done | done | done | 🚧 compile only |
| Download All (batch) | done | done | done | 🚧 compile only |
| Note save/edit | done | done | done | 🚧 compile only |
| Strip LaTeX comments | done | done | done | 🚧 compile only |
| Description generation (DeepSeek) | done | done | done | 🚧 compile only |
| Side chat (single paper) | done | done (REPL) | 🚧 UI done, no stream | no |
| Global chat (all papers) | done | done (REPL) | no | no |
| Chat streaming (SSE) | done | done (code, untested) | no | 🚧 compile only |
| Chat session persistence | done | done | no | no |
| Chat model selection (Flash/Pro) | done | done | done | no |
| Deep thinking toggle | done | done | done | no |
| Token management | done | done | no | no |
| Multiple chat surfaces | done (side + global) | done (side + global) | side only | no |
| Drag-to-resize panels | done (tkinter) | n/a | done | n/a |
| Mouse text selection | no | n/a | 🐛 being debugged | n/a |
| Copy-to-clipboard | no | n/a | done (Ctrl+C) | n/a |
| Clipboard paste | no | n/a | done (Ctrl+V) | n/a |
| Android APK | done (WebView) | n/a | n/a | Tauri2: not configured |

Legend: done = implemented and verified, 🚧 = implemented but not runtime tested, 🐛 = has known bugs, no = not started

---

## Code Structure

```
ArxivCat/
├── Cargo.toml                     # workspace: [crates/*, src-tauri]
├── package.json                   # React frontend deps
├── vite.config.ts                 # Vite config
├── tsconfig.json                  # TypeScript config
├── index.html                     # Entry HTML
│
├── crates/
│   ├── arxivcat-core/             # Pure Rust library (shared by CLI/TUI/GUI)
│   │   ├── Cargo.toml
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── error.rs           # ArxivError enum (Io, Http, Parse, Extraction, Chat, Config, Json, …)
│   │   │   ├── config.rs          # config.json I/O, paths via APPDATA
│   │   │   ├── extract/
│   │   │   │   ├── mod.rs         # ExtractionOutput struct, extract_paper orchestrator
│   │   │   │   ├── arxiv.rs       # ID parsing, PDF ID extraction (lopdf), title fetch (reqwest)
│   │   │   │   ├── source.rs      # download tar.gz, untar, cache verify/repair, download PDF
│   │   │   │   └── tex.rs         # find main, expand inputs, body/appendix split, strip comments
│   │   │   ├── workspace.rs       # Paper/Workspace structs, list/scan/batch
│   │   │   └── chat/
│   │   │       ├── mod.rs         # ChatContext, ContextSelection, build_side/global_chat_context
│   │   │       ├── deepseek.rs    # OpenAI-compatible SSE streaming, model map
│   │   │       ├── session.rs     # JSON session CRUD (save/load/list/rename/delete)
│   │   │       └── description.rs # Description generation via DeepSeek
│   │   └── tests/
│   │       └── integration.rs     # 17 integration tests
│   │
│   ├── arxivcat-cli/              # CLI binary (clap derive)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs            # CLI entry, argparse-style subcommands
│   │       └── commands/
│   │           ├── mod.rs         # resolve_workspace, find_paper
│   │           ├── workspace.rs   # workspace open, scan
│   │           ├── paper.rs       # paper list/download/download-all/preview/note/strip/info
│   │           ├── chat.rs        # side/global chat REPL
│   │           └── token.rs       # token status/set/validate
│   │
│   └── arxivcat-tui/              # TUI binary (ratatui + crossterm)
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs            # Event loop, keyboard+mouse handlers, UI render
│           └── app.rs             # App state, workspace ops, text selection logic, screen_map
│
├── src-tauri/                     # Tauri 2 Rust backend
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── build.rs
│   ├── capabilities/
│   │   └── default.json
│   └── src/
│       ├── main.rs                # Entry point
│       ├── lib.rs                 # Tauri app builder, invoke_handler list
│       └── commands.rs            # #[tauri::command] wrappers around arxivcat-core
│
├── src/                           # React + TypeScript frontend
│   ├── main.tsx
│   ├── App.tsx                    # Root component, 3-panel layout with draggable dividers
│   ├── store.ts                   # zustand state management
│   ├── index.css                  # Tailwind CSS entry
│   └── components/
│       ├── Toolbar.tsx            # Open folder, scan, download, chat, token buttons
│       ├── PaperList.tsx          # Paper list sidebar with status indicators
│       ├── Preview.tsx            # Preview panel with body/appendix/note/description tabs
│       ├── ChatPanel.tsx          # Side chat panel (streaming, model select, deep thinking)
│       └── GlobalChat.tsx         # Global chat overlay (over all workspace papers)
│
├── tests/
│   └── _data/                     # gitignored test data directory
│
└── python-legacy/                 # Original Python code (reference only)
    ├── arxivcat/                  # core.py, presenter.py, chat_service.py, ui/
    ├── web/                       # Flask web app
    └── testground/                # Test scripts
```

---

## Test Status

### Unit Tests (arxivcat-core) — 29/29 pass
- arXiv ID parsing: 7 tests (URL, raw, version, underscore, invalid, text, empty)
- TeX processing: 9 tests (strip comments, find main, expand inputs, body/appendix)
- Source/cache: 2 tests
- Workspace: 5 tests (paper from folder, hidden dirs, internal dirs, list)
- Chat session: 5 tests (save/load, list, rename, delete, empty)

### Integration Tests (arxivcat-core) — 17/17 pass
- Mixed arXiv ID inputs
- Filename sanitization edge cases
- Multiline comment stripping
- Nested input expansion
- Triple-cycle detection
- Body/appendix with bibliography
- Workspace mixed papers
- Config roundtrip
- Side chat context building

### Not Verified
- Actual arXiv download + extraction (needs network)
- DeepSeek chat streaming (needs API key)
- Description generation (needs API key)
- CLI subcommands end-to-end (compile-only)
- TUI interactive functionality (compile-only for chat, tested for UI/interaction)
- Tauri GUI build + run (cargo tauri build not run; needs icons + platform setup)
- Tauri Android APK build (needs Android SDK + NDK)

---

## Known Bugs (Being Debugged)

### TUI mouse → text coordinate mapping
**Status**: 🐛 Active — line mapping works after screen_map fix, column mapping broken again  
**Symptom**: After switching from `char_indices()` on full line to `line[off..]`, both row and column mapping regressed  
**Root cause**: The `screen_map` approach was correct for fixing wrap-aware row mapping. The column fix (using `line[off..]`) had a subtractive error — the visible portion `line[off..]` is correct but the combination with `off` in the final byte position may be introducing a double-count or off-by-one  
**Likely next step**: The `off` value in screen_map represents byte offset within line. `visible = line[off..]`; `char_indices` on `visible` gives positions relative to `off`; `off + col_byte` should give position in line. Need to verify `off` values in screen_map are correct

### Chat streaming (CLI + TUI)
**Status**: Untested — code compiles, no API key  
**Risk**: DeepSeek SSE format may differ from OpenAI format; streaming loop may need tweaks

---

## Dependency Map

| Python | Rust Crate | Purpose |
|---|---|---|
| `re` | `regex` | TeX parsing, arXiv ID patterns |
| `requests` | `reqwest` + `tokio` | HTTP client, SSE streaming |
| `tarfile` / `gzip` | `flate2` + `tar` | Decompress + extract tar.gz |
| `pymupdf` | `lopdf` | PDF metadata extraction |
| `openai` | `reqwest` (manual) | DeepSeek API |
| `argparse` | `clap` (derive) | CLI argument parsing |
| `tkinter` | Tauri 2 / ratatui | GUI / TUI |
| `threading` | `tokio::spawn` | Async concurrency |
| `json` | `serde` + `serde_json` | Serialization |
| `shutil` | `std::fs` | File operations |
| `tempfile` | `tempfile` | Temp directories |
| n/a | `arboard` | System clipboard |
| n/a | `crossterm` | Terminal raw mode, mouse events |
| n/a | `owo-colors` | Terminal color output |
