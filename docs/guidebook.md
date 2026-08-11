# ArXivCat Guidebook

> **STATUS: GUI-era architecture doc.** The GUI this document describes lives
> on the `legacy-gui` branch (tag `v0.9.1-gui`) and is *not* part of the
> CLI-only `main` build. This file is kept on main so it survives future
> `git merge legacy-gui` (and because main's copy is newer, git keeps it).
> CLI/agent documentation: [docs/cli.md](./cli.md).

# ArXivCat Developer Guidebook

This document is for future maintainers and contributors. It explains the full architecture, key design decisions, and code paths of the ArXivCat project.

**参见：[conventions.md](./conventions.md)** — 预设（BTN/TOAST）、组件约定、命名规则。
**注意：`docs/archive/` 为历史快照，勿改。当前状态以本文为准。**

---

## 1. Project Overview

ArXivCat extracts readable LaTeX source code from arXiv papers and provides a workspace-oriented desktop GUI for browsing, editing, and chatting about them.

**Core workflow**: arXiv ID/URL → download source tarball → untar → find main TeX file → recursively expand `\input`/`\include` → split into `body.tex` and `appendix.tex` → optionally generate AI description → store in workspace folder.

**Tech stack**: Rust (core library + CLI + Tauri backend) + React/TypeScript (frontend) + Zustand (state management) + Tailwind CSS v4 (styling).

---

## 2. Directory Structure

```
ArXivCat/
├── Cargo.toml                  # Workspace root: crates/* + src-tauri
├── package.json                # Frontend deps (React, Vite, Tauri CLI)
├── vite.config.ts              # Vite config (HMR, Tailwind plugin)
├── tsconfig.json               # TypeScript strict config
├── index.html                  # Entry HTML (Vite injects scripts)
│
├── crates/
│   ├── arxivcat-core/          # Pure Rust library, no GUI/CLI deps
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs          # Module declarations
│   │       ├── error.rs        # ArxivError enum + ErrorLevel
│   │       ├── config.rs       # %APPDATA%/ArxivCat/config.json I/O
│   │       ├── extract/
│   │       │   ├── mod.rs      # ExtractionOutput struct, extract_paper orchestrator
│   │       │   ├── arxiv.rs    # arXiv ID parsing, PDF ID extraction, title fetch
│   │       │   ├── source.rs   # Download tar.gz, untar, cache validation/repair
│   │       │   └── tex.rs      # Find main TeX, expand \input/\include, body/appendix split
│   │       ├── workspace.rs     # Paper/Workspace structs, scan, batch download
│   │       └── chat/
│   │           ├── mod.rs      # ChatContext, ContextSelection, context builders
│   │           ├── deepseek.rs # DeepSeek API SSE streaming
│   │           ├── session.rs  # Chat session CRUD (JSON files)
│   │           └── description.rs # AI description generation
│   │
│   ├── arxivcat-cli/           # CLI binary (clap)
│   │   └── src/
│   │       ├── main.rs         # Argparse subcommands
│   │       └── commands/       # workspace, paper, chat, token handlers
│   │
│   └── arxivcat-tui/           # TUI binary (ratatui) — NOT PRESENT, only spec
│
├── src-tauri/                  # Tauri 2 backend + frontend shell
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── capabilities/default.json
│   └── src/
│       ├── main.rs             # Entry point
│       ├── lib.rs              # App builder, invoke_handler registration
│       └── commands.rs         # All #[tauri::command] functions
│
├── src/                        # React frontend
│   ├── main.tsx                # React entry
│   ├── App.tsx                 # Root layout: 3-panel + log bar
│   ├── store.ts                # Zustand store (state + actions)
│   ├── index.css               # Tailwind v4 entry + scrollbar/keyframe styles
│   ├── hooks/
│   │   └── useChatSessions.ts  # Chat session lifecycle + streaming
│   └── components/
│       ├── ChatControls.tsx    # Model + reasoning effort selector (reused)
│       ├── ChatMessages.tsx    # Shared message list + streaming + input area
│       ├── ChatPanel.tsx       # Side chat panel (thin wrapper around shared components)
│       ├── ChatSessionBar.tsx  # Session list + CRUD, compact popup mode
│       ├── Dialog.tsx          # Reusable floating window (resize + drag + enter/exit animation)
│       ├── GlobalChat.tsx      # Global chat overlay dialog
│       ├── PaperList.tsx       # Paper list sidebar
│       ├── Preview.tsx         # Tabbed preview + editing + PDF viewer
│       ├── Ripple.tsx          # Button wrapper
│       ├── SegmentedControl.tsx # Pill button group (model selector)
│       ├── Select.tsx          # Custom dropdown menu
│       ├── Toast.tsx           # Global toast notification
│       ├── ToggleChips.tsx     # Pill toggle chips (context selection)
│       ├── Toolbar.tsx         # Toolbar: workspace path, scan, download, chat toggles
│       └── Tooltip.tsx         # Hover tooltip
│
├── python-legacy/              # Original Python code (archived, not in use)
├── docs/                       # Developer documentation
└── performance_profiling/      # Chrome DevTools Performance traces
```

---

## 3. Core Data Flow

### 3.1 Paper Download & Extraction

```
User enters arXiv URL/ID → Toolbar.tsx
  → store.downloadPaper(rawInput)
    → invoke("download_paper", { rawInput, workspacePath })
      → commands.rs: download_paper()
        1. extract_arxiv_id(rawInput) → "2501.12948"
        2. fetch_title_from_arxiv(id) → "Paper Title"
        3. sanitize_filename(title) → "Paper_Title"
        4. create workspace/{id}_{title}/
        5. download_source(id, cache_dir) → tar.gz → untar
        6. find_main_tex(paper_dir) → which .tex has \documentclass
        7. expand_inputs(content, base_dir) → recursive \input/\include resolution
        8. extract_body_and_appendix(expanded) → heuristic split
        9. write body.tex, appendix.tex
        10. download_pdf(id, out_dir)
        11. build_description(...) → DeepSeek API → description.md
        12. return PaperDto
    → store refreshes paper list
    → selectPaper(paper) → preview loads
```

### 3.2 Tab Switching

```
User clicks tab button → tabClick(key)
  1. setActiveTab(key)           # INSTANT: local state, button color changes
  2. setTimeout(() => ..., 0)    # DEFERRED: expensive content switch
     a. saveDraft if editing
     b. setEditing(false)
     c. switchView(key) → zustand store update
        → Preview re-renders with new currentView
        → conditional rendering shows the right pre/textarea/embed
```

This split (visual feedback first, content later) is the key performance optimization
that makes tab switching feel instant. See §8 for details.

---

## 4. Rust Backend: arxivcat-core

### 4.1 arXiv ID Parsing (`extract/arxiv.rs`)

```rust
extract_arxiv_id(input: &str) -> Option<String>
```

Regex: `(\d+[._]\d+(?:v\d+)?)`. Accepts URLs, bare IDs, versions. Returns normalized
ID with `.` separator (not `_`).

```rust
extract_arxiv_id_from_pdf(pdf_path: &Path) -> Result<Option<String>>
```

Three strategies in order:
1. PDF metadata fields (subject, keywords, title, author)
2. First 3 pages text — tries `arXiv:XXXX.XXXXX` prefix pattern first, then bare
3. Fails gracefully (returns None, not error)

```rust
fetch_title_from_arxiv(arxiv_id: &str) -> Result<Option<String>>
```

GET `https://arxiv.org/abs/{id}`, extract `<meta property="og:title" content="...">`.
15s timeout. Returns None if page unreadable.

```rust
sanitize_filename(name: &str) -> String
```

Replaces illegal Windows filename chars (`<>:"/\|?*`), control characters,
collapses whitespace, trims trailing `.`/space/`-`/underscore, truncates to 80 chars.

### 4.2 Source Download & Cache (`extract/source.rs`)

```rust
download_source(arxiv_id: &str, downloads_dir: &Path)
  -> Result<(Option<PathBuf>, Option<String>)>
```

Cache at `{downloads_dir}/{id}_{title}/`:
1. If cached dir exists → validate cache → if valid, return path
2. If invalid → repair permissions → re-validate → if still bad, delete and re-download
3. Download from `https://arxiv.org/src/{id}`
4. Extract tar.gz with path traversal protection (`is_safe_tar_member`)
5. On name conflict → `_fresh1`, `_fresh2`, etc.

Cache validation checks: main TeX file exists, directory readable, all .tex files readable.

### 4.3 TeX Extraction (`extract/tex.rs`)

```rust
find_main_tex(paper_dir: &Path) -> Option<PathBuf>
```

1. If `main.tex` exists → return it immediately
2. Otherwise scan top-level `*.tex` files for `\documentclass`

```rust
expand_inputs(tex_content, base_dir, seen, root_dir) -> Result<String>
```

Recursive regex `\\(?:input|include)\s*\{([^}]+)\}`:
- Tries `base_dir/{name}`, `root_dir/{name}`, with `.tex` appended if missing
- Cycle detection via `_seen: HashSet<PathBuf>`
- Returns original `\input{...}` text if file not found (post-expansion validation catches this)

```rust
extract_body_and_appendix(tex_content) -> Result<(String, Option<String>)>
```

Heuristic body/appendix split:
- Body starts at earliest of: `\begin{abstract}`, first `\section`, `\begin{document}`
- Split point at earliest of: `\appendix`, `\begin{appendix}`, `\bibliography{...}` (after body start)
- If no split point → falls back to last Conclusion/Summary section
- Appendices shorter than 50 chars → None (no appendix.tex written)

This is **heuristic by design**. It works on common arXiv paper structures but is not
a complete LaTeX parser.

### 4.4 Workspace (`workspace.rs`)

```rust
Paper { arxiv_id, title, folder_name, folder, has_body, description_ready, is_complete }
```

Paper parsing from folder name: `{id_part1}_{id_part2}_{title}`.
Skips hidden dirs (`.`-prefix) and internal dirs (`arxivcat_global_chats`).

```rust
Paper::from_folder(folder: &Path) -> Option<Paper>
```

Key heuristic: folder name must have at least 2 underscore-separated parts (`{id1}_{id2}`).

### 4.5 Chat (`chat/deepseek.rs`)

OpenAI-compatible SSE streaming to `https://api.deepseek.com/chat/completions`:

```rust
stream_chat(messages, model, deep_thinking, callbacks, cancel_flag) -> Result<()>
```

- Models: Flash (`deepseek-v4-flash`), Pro (`deepseek-v4-pro`)
- Deep thinking: sends `extra_body: { thinking: { type: "enabled" } }`, `reasoning_effort: "high"`
- Callbacks: `on_token(text, bool)`, `on_status(text)`, `on_complete(text)`
- Cancel: checks `AtomicBool` between chunks
- Metrics: TTFT, tokens/sec, token count reported via `on_status`

### 4.6 Chat Session Persistence (`chat/session.rs`)

Sessions stored as JSON files:
- Side chat: `{workspace}/{paper_folder}/arxiv_chats/{YYYYMMDD_HHMMSS}.json`
- Global chat: `{workspace}/arxivcat_global_chats/{YYYYMMDD_HHMMSS}.json`

Each session stores: title, kind, model, deep_thinking, messages[],
context_selection, context_snapshot, view_name, updated_at.

Full CRUD: save, load, list (sorted by modified date), rename, delete.

---

## 5. Rust CLI: arxivcat-cli

Binary name: `arxivcat`. Built via `cargo build --release --bin arxivcat`. Source at `crates/arxivcat-cli/`.

### Global Flags

| Flag | Description |
|---|---|
| `-w, --workspace <PATH>` | Override workspace path for this invocation. If omitted, uses path from config (`APPDATA/ArxivCat/config.json`). If explicitly passed but path does not exist, errors immediately. |
| `--json` | Output machine-readable JSON. Can be placed anywhere in the command (before or after subcommand). Supported by: `list`, `download`, `download-all`, `preview`, `info`, `token status`. |

### 5.1 workspace

```
arxivcat -w /path/to/ws workspace open /path/to/ws    # Set workspace
arxivcat workspace scan                               # Scan for untracked PDFs
```

| Subcommand | Description |
|---|---|
| `open <PATH>` | Persist workspace path to config. |
| `scan` | Scan workspace root for PDFs, extract arXiv IDs from PDF metadata, create paper folders with `note.txt` + `description.md` stubs. Skips already-tracked papers. |

### 5.2 paper

| Subcommand | Description |
|---|---|
| `list` | List all papers with status: `[C]` complete, `[P]` pending (has body, no description), `[.]` incomplete. JSON: array of paper objects with `arxiv_id`, `title`, `has_body`, `description_ready`, `is_complete`. |
| `download <ID_OR_URL>` | Full pipeline: parse arXiv ID from raw ID or URL → fetch title → download source tar.gz → extract `body.tex`/`appendix.tex` → download PDF → generate AI description. JSON: `{arxiv_id, folder, body_length, appendix_length, description_ready}`. |
| `download-all` | Batch-process all `[P]` papers sequentially. JSON: `{status, success, total}`. |
| `preview <ID_OR_QUERY> -v <VIEW>` | Print file content. `-v` values: `body` (default), `appendix`, `note`, `description`. ID can be bare ID, partial match, or full arXiv URL. JSON: `{arxiv_id, title, view, content}`. |
| `note <ID_OR_QUERY> [TEXT]` | Without args: print `note.txt`. With TEXT: overwrite note. `-e` flag: open in `$EDITOR` (falls back to `notepad` on Windows). |
| `strip <ID_OR_QUERY>` | Strip LaTeX comments from `body.tex`, collapse 3+ blank lines → 2, print to stdout. |
| `open <ID_OR_QUERY>` | Open paper folder in system file manager. |
| `pdf <ID_OR_QUERY>` | Open local PDF, or fallback to `https://arxiv.org/pdf/{id}`. |
| `info <ID_OR_QUERY>` | Show arXiv ID, title, folder path, file sizes. JSON: full paper object with `files` map. |

**ID matching**: Most `paper` subcommands accept both raw arXiv IDs (`2501.12948`), partial IDs (`2501` matches `2501.12948`), and full URLs (`https://arxiv.org/abs/2501.12948`, `arxiv.org/pdf/2501.12948.pdf`, `www.arxiv.org/abs/2501.12948v2`, with or without surrounding whitespace). The `download` subcommand additionally strips the ID from URL input before proceeding.

### 5.3 chat

| Subcommand | Description |
|---|---|
| `side <ID_OR_QUERY>` | Interactive REPL chat scoped to one paper. Commands: `/model Flash\|Pro`, `/thinking` (toggle), `/context body\|appendix\|description\|note` (toggle), `/save`, `/load`, `/history`, `/clear`, `/quit`. |
| `global` | Interactive chat over all papers' descriptions from the workspace. Same REPL commands as `side`. |

Sessions persist as JSON files in `{paper_folder}/arxiv_chats/` (side) or `{workspace}/arxivcat_global_chats/` (global). Requires DeepSeek API key.

### 5.4 token

| Subcommand | Description |
|---|---|
| `status` | Show whether token is configured (masked, e.g. `sk-5...7abe`) and validate it. JSON: `{configured, masked, valid}`. |
| `set` | Prompt to enter DeepSeek API token (from stdin). Saved to config. |
| `validate` | Test cached token against `https://api.deepseek.com/models`. |

### Examples

```
arxivcat -w F:\zrs\paper paper list
arxivcat -w F:\zrs\paper --json paper info 2501.12948
arxivcat paper list --json                        # --json works after subcommand too
arxivcat paper download arxiv.org/abs/2501.12948  # accepts URLs
arxivcat paper preview 2501.12948 -v appendix     # view appendix
arxivcat paper note 1706.03762 "my thoughts"      # write note
arxivcat paper strip 2501.12948                   # strip LaTeX comments
arxivcat -w /bad/path paper list                  # errors immediately if -w path invalid
```

**完整 CLI 使用手册见 [cli.md](./cli.md)** — 面向外部用户和 AI agent 的独立参考文档。

---

## 6. Rust Backend: src-tauri (Tauri Commands)

`commands.rs` has 30+ `#[tauri::command]` functions. Key groups:

### 5.1 Paper Management
| Command | Purpose |
|---------|---------|
| `download_paper` | Full pipeline: parse ID → fetch title → download source → extract → PDF → description |
| `extract_paper` | Extract to cache only (legacy, used by old frontend code) |
| `get_paper_list` | List papers in workspace folder |
| `load_paper` | Read body.tex, appendix.tex, note.txt, description.md |
| `save_note` | Write note.txt |
| `save_description` | Write description.md + .description_ready |
| `scan_pdfs` | Scan workspace for PDFs, extract arXiv IDs, create paper folders |
| `download_all` | Batch process pending papers (parallel via tokio::spawn) |

### 5.2 Chat
| Command | Purpose |
|---------|---------|
| `start_chat` | Start SSE streaming, return session_id, emit events |
| `cancel_chat` | Set AtomicBool flag to cancel stream |
| `get_chat_sessions` | List sessions in a directory |
| `save_chat_session_data` | Save session → returns assigned file path |
| `rename_chat_session_data` | Rename session title |
| `delete_chat_session_data` | Delete session file |

### 5.3 Utility
| Command | Purpose |
|---------|---------|
| `open_workspace` | Open folder + save to config |
| `open_paper_folder` | Open file explorer at paper dir |
| `open_paper_pdf` | Open PDF in system viewer or arXiv fallback |
| `read_pdf_base64` | Read PDF file → base64 → frontend blob URL |
| `get_token_status` | Check if DeepSeek API key configured |
| `set_token` | Save DeepSeek API key |
| `validate_token` | Test DeepSeek API key against /models endpoint |

### 5.4 Event Architecture

Streaming uses Tauri's `Emitter` pattern:
- `chat:token` — delta content from DeepSeek SSE
- `chat:status` — TTFT, tok/s, token counts
- `chat:done` — complete response text
- `chat:error` — error message
- `download:progress` — per-paper download progress
- `download:done` — batch download complete

---

## 7. React Frontend

### 6.1 State Management (`store.ts`)

All app state lives in a single Zustand store. Key state groups:

```
workspacePath       Current workspace folder path
papers[]            Paper list
currentPaper        Selected paper
previewContent      { body, appendix, note, description } strings
currentView         "body" | "appendix" | "note" | "description" | "pdf"
drafts              { key: content } for unsaved edits (localStorage backed)
chat               { sessionId, streaming, status, bufferTokens }
download           { inProgress, current, total }
sideChatOpen       Side chat visibility
globalChatOpen     Global chat overlay visibility
logOpen            Log panel visibility
chatModel          "Flash" | "Pro"
deepThinking       Deep thinking toggle
logMessages[]      Rolling log buffer (max 100)
```

Components use `useStore(selector)` (individual selectors) or `useStore(useShallow(...))`
to subscribe only to the fields they need. This is critical for performance — see §8.

### 6.2 Component Tree

```
App
├── Toolbar          # Open folder, scan, download, Papers toggle, chat toggles, Log, Token
├── [left divider]   # (hidden when Papers closed)
├── PaperList        # Scrollable paper list with status indicators + Tooltip
├── [center area]
│   ├── Preview      # Tabbed view: Body/Appendix/Note/Description/PDF
│   └── ChatPanel    # Side chat (when open)
├── [right divider]
├── ChatPanel        # (when sideChatOpen, draggable up to 60% width)
├── GlobalChat       # Dialog overlay (resizable + movable)
├── Toast            # Global notification (green bar + progress)
└── Dialog           # Log overlay (shared Dialog component)
```

### 6.3 Preview Component (the most complex)

The Preview component handles four views plus edit mode:

```
Preview (memo-wrapped)
├── Tab buttons (Body | Appendix | Note | Description | PDF)
├── Content container
│   ├── Body      → <pre> with ¶ markers, dangerouslySetInnerHTML
│   ├── Appendix  → <pre> with ¶ markers, dangerouslySetInnerHTML
│   ├── Note      → <pre> (view) | <textarea> (edit) + Save/Cancel
│   ├── Description → <pre> (view) | <textarea> (edit) + Save/Cancel
│   └── PDF       → <embed> with blob URL (loaded via IPC)
└── Toast overlay (center screen, auto-dismiss)
```

Key state: `activeTab` (local, instant feedback) vs `currentView` (store, content switch).
See §8.3 for the tab switching optimization.

### 6.4 The PreView Component

```tsx
const PreView = memo(({ html }: { html: string }) => (
  <pre className="..." dangerouslySetInnerHTML={{ __html: html }} />
));
```

A memoized wrapper around `<pre>` with `dangerouslySetInnerHTML`. The `html` string
is pre-computed with `¶` markers. `React.memo` ensures it only re-renders when the
HTML string actually changes.

### 6.5 RenderMarkers Function

```ts
function renderMarkers(text: string) {
  // Escape HTML, then insert ¶ before each \n
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/\n/g, '<span class="text-[#6c7086]/50 select-none">¶</span>\n');
}
```

Only marks newlines with a subtle pilcrow. Spaces are NOT marked (removed after
discovery that space marking caused selection/copy issues).

---

## 8. Performance Architecture

### 7.1 Layer 1: Store Subscription Isolation

Every component subscribes only to the store fields it needs. This prevents
unrelated store updates from triggering re-renders:

```tsx
// ❌ BAD: subscribes to everything
const { workspacePath, papers, ... } = useStore();

// ✅ GOOD: only subscribes to what's used
const workspacePath = useStore((s) => s.workspacePath);
const papers = useStore((s) => s.papers);

// For multiple fields with shallow comparison:
const { a, b, c } = useStore(useShallow((s) => ({ a: s.a, b: s.b, c: s.c })));
```

This was the first fix applied — before it, every button click caused the entire
app tree to re-render. After it, components only re-render when their specific
fields change.

### 7.2 Layer 2: React.memo Guard

```tsx
const Preview = memo(function Preview() { ... });
```

The Preview component (which contains large DOM subtrees) is wrapped in `React.memo`.
This prevents it from re-rendering when its parent (App) re-renders due to unrelated
state changes.

Components that are NOT wrapped will re-render whenever their parent re-renders.
This is acceptable for small components (Toolbar buttons, PaperList items) but not
for Preview with its 10k+ DOM nodes.

### 7.3 Layer 3: Tab Switching Optimization (activeTab + setTimeout)

Tab switching is fundamentally different from button clicks — it MUST re-render
Preview because `currentView` changes. The optimization splits the work:

```
Click → setActiveTab(newTab)  ← INSTANT (local useState, ~1ms)
       → setTimeout(0)        ← DEFERRED to next frame
           → switchView(newTab)  ← EXPENSIVE (zustand → React → layout)
```

The tab button checks `activeTab` (local state) instead of `currentView` (store).
This means the button color changes immediately, while the content switch happens
asynchronously. The user sees instant visual feedback, then the content appears.

### 7.4 Layer 4: Conditional Rendering (One Pre at a Time)

The content area renders only ONE view at a time, not all four:
```tsx
{currentView === "body" && <PreView html={renderedBody} />}
{currentView === "appendix" && <PreView html={renderedAppendix} />}
// ... etc
```

An earlier attempt rendered all four views simultaneously with `visibility: hidden`
to avoid DOM recreation. This backfired — having 100k+ DOM nodes in the layout tree
caused 130ms layout times on EVERY button click, not just tab switches.

**Key lesson**: `visibility: hidden` elements still participate in layout. Removing
them from the DOM (`display: none` via conditional rendering) keeps the layout tree
~4x smaller.

### 7.5 Layer 5: Ripple Deferred to requestAnimationFrame

The ripple effect in `Ripple.tsx` used to run synchronously during the click handler,
calling `getBoundingClientRect()` (forces layout) and `appendChild()` (DOM mutation).
This blocked React's state update. Fixed by deferring to `requestAnimationFrame`.

### 7.6 Performance Profile Summary

| Optimization | Before | After | Wins |
|---|---|---|---|
| Store subscription isolation | 300ms on any click | ~5ms | Every button |
| React.memo on Preview | Cascade re-renders | Only when deps change | Every non-tab click |
| activeTab + setTimeout | 150ms perceived lag | Instant button, 50ms content | Tab switching |
| Conditional rendering (1 pre) | 130ms layout (100k DOM) | 50ms layout (25k DOM) | Every re-render |
| Deferred ripple | Layout thrashing on click | No blocking | Every button click |

---

## 9. Chat System (Component Architecture)

### 8.1 Overview

Chat surfaces are built from four shared components + one shared hook:

```
useChatSessions (shared hook)
├── sessions / messages / streaming / lockedFields
├── sendMessage / cancelChat / newSession / switchSession
├── generateTitle (auto / manual)
└── lockedFields (per-paper Vec<String> persisted in session JSON)

ChatControls (shared)
├── SegmentedControl (Flash / Pro)
└── Select (off/low/medium/high/max reasoning effort + gradient)

ChatSessionBar (shared)
├── Compact: button → popup floating panel
├── Session CRUD: switch / rename / delete / regenerate title
└── Click-outside to close

ToggleChips (shared)
├── Pill buttons: Body / Appendix / Description / Note
├── Active = blue, inactive = dark
└── Locked = blue + non-interactive + opacity

ChatMessages (shared)
├── Message list with markdown + LaTeX rendering
├── Streaming buffer with auto-scroll
├── Status bar
└── Input area with Send / Stop
```

### 8.2 ChatSurface Usage

Both `ChatPanel` and `GlobalChat` become thin wrappers:

```
ChatPanel (~90 lines)          GlobalChat (~170 lines)
├── Inline flex layout          ├── Dialog overlay
├── Header: title + session     ├── Header: title + papers + session
│   + ChatControls + Ctx        │   + ChatControls + Ctx + Close
├── ChatSessionBar              ├── (no ChatSessionBar row—
├── ToggleChips (collapsible)   │    sessions button in header)
└── ChatMessages                ├── ToggleChips (per-paper,
                                │    collapsible, with All buttons)
                                └── ChatMessages
```

### 8.3 Context Locking (v0.9.0)

When a message is sent, the currently active context fields are locked:

- **Per-paper** (Global Chat): only papers that had a field active get that field locked
- **Persisted**: locked fields stored in session JSON as `{"folder": ["body", "note"]}`
- **Restored**: on session load, locked fields are restored and chips become non-toggleable
- **Immutable**: once locked, a chip cannot be unchecked (preserves KV cache for prefix)

### 8.4 Auto Title Generation (v0.9.0)

After the first assistant response and every 5 responses after, a non-streaming DeepSeek Flash call generates a short title:

1. Fork the conversation messages (user + assistant turns)
2. Append `"Generate a short title for this conversation..."` as a user message
3. Call with `thinking: disabled`, `max_tokens: 20`
4. Update session title in state + persist via `rename_chat_session_data`
5. Manual regenerate via `↻` button in session popup

Titles starting with `"Chat "` or `"Global Chat "` are considered default (not yet generated).

### 8.2 Hook Interface

```ts
function useChatSessions(sessionDir: string | null, model: string, deepThinking: boolean)
  → {
      sessions,      // ChatSession[] — from disk
      activeIdx,     // currently selected session index
      messages,      // messages of active session
      streaming,     // currently streaming?
      status,        // "thinking...", "cancelled", metrics string
      localBuffer,   // streaming accumulator
      // Actions:
      newSession, switchSession, renameSession, deleteSession,
      sendMessage, cancelChat,
    }
```

### 8.3 Auto-Save

After streaming completes (streaming → false, messages have new assistant message),
the hook automatically calls `saveCurrent()`, which invokes `save_chat_session_data`
on the Rust backend. The backend returns the assigned file path, which is stored
in the local sessions state for subsequent saves.

### 8.4 Session-Chat Message Flow

```
sendMessage(content, context)
  → setMessages(userMsg)        // Optimistic UI
  → invoke("start_chat")        // Returns session_id
  → listen for chat:token       // Accumulate in localBuffer (with dedup)
  → listen for chat:done        // Append assistant message
  → auto-save → saveCurrent()   // Persist to disk
```

---

## 10. Draft System

### 9.1 Purpose

The draft system allows users to edit Note and Description without immediately
writing to disk. Only clicking "Save" persists changes to `note.txt` or
`description.md`.

### 9.2 Storage

Drafts are stored in two places:
1. **Zustand store** (`drafts: Record<string, string>`) — live state
2. **localStorage** (key: `ac_draft_{folderName}_{view}`) — survives app restart

On app startup, drafts are loaded from localStorage and set as the initial state.
If a draft exists for the current paper + view, Preview automatically enters edit
mode with the draft content.

### 9.3 Key Behavior

| Action | What happens |
|---|---|
| Type in Note/Description | 500ms debounce → `saveDraft(key, content)` → zustand + localStorage |
| Switch tabs | `saveCurrent()` called, then switch |
| Click Save | `saveNote()` or `saveDescription()` → backend write → `clearDraft()` |
| Click Cancel | `clearDraft()` → discard draft |
| Restart app | Draft loaded from localStorage → auto-enter edit mode |

---

## 11. Styling & Theme

### 10.1 Color Palette

Based on Catppuccin Mocha:

| Token | Hex | Usage |
|---|---|---|
| `bg-[#1e1e2e]` | Base | Main background |
| `bg-[#11111b]` | Mantle | Text area backgrounds |
| `bg-[#181825]` | Crust | Toolbar |
| `bg-[#313244]` | Surface0 | Scrollbar, inactive buttons |
| `bg-[#45475a]` | Surface1 | Hover state, secondary buttons |
| `bg-[#585b70]` | Surface2 | Active hover |
| `bg-[#89b4fa]` | Blue | Active tab, accent buttons |
| `bg-[#a6e3a1]` | Green | Save button, success |
| `bg-[#f38ba8]` | Red | Stop/Cancel |
| `bg-[#f9e2af]` | Yellow | Status/warning |
| `text-[#cdd6f4]` | Text | Primary text |
| `text-[#a6adc8]` | Subtext0 | Secondary text |
| `text-[#6c7086]` | Overlay0 | Muted text, decorative marks |

### 10.2 Scrollbar

Custom thin scrollbar in `index.css`:
```css
*::-webkit-scrollbar { width: 6px; height: 6px; }
*::-webkit-scrollbar-track { background: transparent; }
*::-webkit-scrollbar-thumb { background: #45475a; border-radius: 3px; }
```

### 10.3 Component Presets

Button and toast colors use preset constants in `src/store.ts` for consistency.
Always use these instead of inline color classes.

#### Button Presets (`BTN`)

```ts
BTN.surface0   // bg-[#313244] hover:bg-[#45475a]    — inactive/secondary buttons
BTN.surface1   // bg-[#45475a] hover:bg-[#585b70]    — default toolbar buttons
BTN.blue       // bg-[#89b4fa] hover:bg-[#b4d0fb]    — accent/active buttons
BTN.green      // bg-[#a6e3a1] hover:bg-[#b8ebc0]    — save/confirm buttons
BTN.red        // bg-[#f38ba8] hover:bg-[#f5a0b9]    — stop/cancel buttons
BTN.ghost      // hover:bg-[#313244]                  — icon-only buttons
```

Usage:
```tsx
<RippleBtn className={`rounded px-3 py-1.5 text-sm ${BTN.surface1}`}>Click</RippleBtn>
```

For toggle buttons that change color by state, use the preset for each branch:
```tsx
className={`rounded px-2 py-0.5 text-xs ${
  active ? BTN.blue : BTN.surface0
}`}
```

#### Toast Presets (`TOAST`)

```ts
TOAST.success  // green  — default
TOAST.info     // blue
TOAST.error    // red
TOAST.warning  // yellow
```

Usage:
```ts
showToast("Saved!")               // green (default)
showToast("Failed", "error")      // red
```

### 10.4 Icons

### 10.5 Dialog (Floating Window)

`src/components/Dialog.tsx` is a reusable floating window used by Global Chat and Log.
It supports drag (via title bar), resize (via bottom-right handle), and enter/exit animation.

**Props:**
| Prop | Default | Description |
|---|---|---|
| `open` | — | Visibility toggle |
| `onClose` | — | Close handler |
| `title` | — | Title content (ReactNode) |
| `children` | — | Body content (fills remaining space) |
| `headerExtra` | — | Extra buttons/controls in title bar |
| `defaultWidth` | 600 | Initial width |
| `defaultHeight` | 400 | Initial height |
| `minWidth` | 400 | Minimum resize width |
| `minHeight` | 300 | Minimum resize height |

**Animation:**
- Enter: `alive → double rAF → visible → scale(0.95→1) + opacity(0→1)` over 0.15s
- Exit: `visible=false → setTimeout(alive=false, 150ms)` over 0.15s

**Performance:**
Resize and drag manipulate DOM directly via `dialogRef` (not React state) during
the operation, and sync final values to state on mouseup. Transition is disabled
during drag/resize to avoid lag. See `Dialog.tsx:67-108`.

---

## 12. Build & Development

### 11.1 Development

```bash
npm install              # Frontend deps
npm run tauri dev        # Dev mode (Vite HMR + cargo watch)
```

The dev server runs Vite on port 1420. Tauri's `beforeDevCommand` starts Vite,
then the Rust backend compiles and launches the window. Frontend changes are
hot-reloaded via Vite HMR. Rust changes trigger automatic rebuild via cargo watch.

### 11.2 Production Build

```bash
npm run tauri build      # tsc → vite build → cargo build --release
```

Output: `target/release/arxivcat-gui.exe`

### 11.3 Required Tools

- Rust 1.97+ (stable)
- Node.js 20+ (LTS)
- Visual Studio Build Tools 2022 (VC++ workload, Windows)
- WebView2 (Windows 10+, included)
- Tauri CLI 2.x (via `npm i -g @tauri-apps/cli`)

### 11.4 Key Dependencies

| Frontend | Purpose |
|---|---|
| `react-markdown` + `remark-math` + `rehype-katex` | Markdown + LaTeX rendering in chat |
| `katex` | Math formula rendering |
| `zustand` | State management |
| `@tauri-apps/api` | IPC invoke + event listen |
| `@tauri-apps/plugin-dialog` | Folder picker |
| `tailwindcss v4` | CSS utility framework |
| `@tailwindcss/typography` | prose classes for markdown |
| `@tailwindcss/vite` | Tailwind v4 Vite plugin |

| Rust | Purpose |
|---|---|
| `reqwest` + `tokio` | HTTP client, DeepSeek API |
| `lopdf` | PDF metadata reading |
| `flate2` + `tar` | tar.gz extraction |
| `regex` | LaTeX parsing, arXiv ID patterns |
| `clap` | CLI argument parsing |
| `serde` + `serde_json` | JSON config, IPC serialization |
| `tauri v2` | Desktop app framework |
| `tauri-plugin-shell` | Open in system browser |
| `tauri-plugin-dialog` | Native file dialogs |
| `base64` | PDF → base64 for IPC transfer |

---

## 13. Known Design Decisions & Trade-offs

### 12.1 LaTeX Extraction is Heuristic

The body/appendix split is NOT a complete LaTeX parser. It uses regex-based
heuristics that work on ~95% of common arXiv papers. Unusual document structures
may produce incorrect splits. This is intentional — a full LaTeX parser would be
orders of magnitude more complex and is unnecessary for the target use case.

### 12.2 Single Zustand Store

All state lives in one store rather than multiple stores. This simplifies the
mental model (one source of truth) but requires discipline in subscription
patterns (`useStore(selector)` everywhere) to avoid performance issues.

### 12.3 `dangerouslySetInnerHTML` for Markers

Using `dangerouslySetInnerHTML` with an HTML string is faster than creating
thousands of React `<span>` elements for the `¶` markers. It bypasses React's
reconciliation entirely. This is safe because the input is escaped HTML.

### 12.4 IPC for PDF Instead of Asset Protocol

The asset protocol (`convertFileSrc`) was originally tried but didn't work reliably
in Tauri 2. The current approach reads the PDF as base64 through IPC and creates
a blob URL. This is slightly slower for large PDFs but works across all platforms
without configuration.

### 12.5 No TUI Implementation

The `crates/arxivcat-tui/` directory referenced in `ARCHITECTURE.md` was never
implemented. Only the spec exists (`tui_spec.md`). The TUI was part of the original
architecture plan but was abandoned in favor of the Tauri GUI.

### 12.6 Python Legacy

The `python-legacy/` directory contains the original Python implementation (Tkinter
+ Flask). This is archived for reference only. The Rust/Tauri version is the
active codebase.

### 12.7 Project History & Python→Rust Migration

#### Origins (Python, v0.1 – v0.7)

ArXivCat started as a Python Tkinter desktop application. The original architecture:

```
main.py (entry) → Presenter (app logic) → core.py (extraction)
                 ↓
              UIProtocol (interface)
                 ↓
         TkApp (Tkinter widgets)  or  CliUI (terminal)
```

This worked but had inherent problems:
- **Tkinter packaging** was fragile — PyInstaller builds broke on Tcl/Tk version
  mismatches between environments.
- **Threading pain** — GUI thread couldn't block, so all extraction ran in
  daemon threads. State synchronization was manual and error-prone.
- **No mobile path** — Tkinter is desktop-only. A separate Flask web app
  (`python-legacy/web/`) was built for mobile, duplicating logic.
- **Global state** scattered across `Presenter` class fields, with ad hoc
  cancellation flags (`_download_all_cancelled`, `_task_busy`).

#### Why Rust + Tauri 2

The rewrite was driven by five concrete pain points:

| Problem | Python | Rust/Tauri |
|---|---|---|
| Concurrency | `threading.Thread` + flag polling | `tokio::spawn` + `AtomicBool` |
| GUI framework | Tkinter (desktop only) + Flask (web) | Tauri 2 (desktop + mobile) |
| Packaging | PyInstaller fragile on Tcl/Tk | Single native exe, no runtime deps |
| Type safety | None at runtime | Full compile-time type checking |
| State management | Mutable class fields | Immutable serde structs |

The extraction logic was preserved but rewritten in idiomatic Rust. The Python
codebase remains in `python-legacy/` as reference, not as an active codebase.

#### Migration Strategy

The rewrite was done crate-first:
1. `arxivcat-core` — Pure Rust port of extraction/core.py, with tests
2. `arxivcat-cli` — clap CLI wrapping core
3. `src-tauri` — Tauri 2 backend calling core, with React frontend

Each crate was built and tested independently. The frontend was rebuilt from
scratch (no migration from Tkinter widgets — they share nothing architecturally).

#### What Was Dropped

- **TUI** (`arxivcat-tui`) — Planned but never implemented. Spec only (`tui_spec.md`).
- **Web version** (Flask + Gemini) — Superseded by Tauri's mobile capability.
- **Google Gemini support** — Desktop now uses DeepSeek exclusively. The Flask web
  version used Gemini because it was free; Tauri desktop has no such constraint.

#### What Was Preserved

- **Extraction heuristics** — The body/appendix split logic, input expansion, and
  cache validation were directly ported from Python with minimal changes.
- **Workspace data format** — Folders named `{id}_{title}` with body.tex, note.txt,
  description.md are compatible between Python and Rust versions.
- **arXiv ID parsing** — Same regex patterns.

### 12.8 Version History

- `v0.2.1` — Python Tkinter, basic extraction
- `v0.3.0` — Python + workspace mode (folder per paper)
- `v0.6.0` — Python + PDF scan, batch download
- `v0.7.1` — Python + description generation, global chat
- `v0.8.0` — Complete Rust/Tauri 2 rewrite (current)

---

## 14. Test Coverage

### Unit Tests — arxivcat-core (44 pass)

| Area | Tests | What's covered |
|---|---|---|
| arXiv ID parsing | 7 | URL, raw, version, underscore, invalid, text, empty |
| TeX processing | 9 | Strip comments, find main, expand inputs (flat/nested/cycle), body/appendix split |
| Source/cache | 5 | find_main_tex prefers main.tex, fresh_folder_name, tar extraction, tar safety |
| Workspace | 5 | Paper from folder (complete/pending), skips hidden/internal dirs, list |
| Chat session | 5 | Save/load, list sorted, rename, delete, empty save is no-op |
| Chat context | 7 | Global context (empty/body/multi-field/skip), selection delta computation |
| Config | 4 | Corrupted JSON, empty object, partial fields, extra fields ignored |
| Filename sanitize | 2 | Illegal chars, length truncation |

### Unit Tests — arxivcat-cli (11 pass)

| Area | Tests | What's covered |
|---|---|---|
| find_paper | 6 | Direct ID, abs/pdf/www/versioned URL, edge cases (nonexistent, whitespace) |
| resolve_workspace | 2 | `-w` flag (valid path, invalid path → error) |
| resolve_view_file | 2 | Valid views (body/appendix/note/description), invalid views (empty/unknown/case) |
| resolve_workspace (config) | — | Not covered; requires config.rs refactor for injectable paths |

### Integration Tests — arxivcat-core (22 pass)

| Area | Tests |
|---|---|
| arXiv ID parsing | Mixed formats, PDF URLs, versioned URLs, www prefix, trailing slash, whitespace |
| Filename sanitization | Empty string → "untitled", illegal chars, underscores |
| TeX processing | Escaped percent comments, multiline comments, missing file preservation |
| Input expansion | Nested includes, deep cycle detection (a→b→c→a) |
| Body/appendix | With bibliography, without appendix/biography |
| Workspace | Empty dir, mixed papers, partial ID lookup |
| Config | Roundtrip (token, model, workspace) |
| Chat | Session save/load empty, side context with/without selection, empty dir |

### Not Verified (need network or API key)
- Actual arXiv download + extraction
- DeepSeek chat streaming
- Description generation
- Tauri GUI full build on CI

---

## 15. Dependency Map (Python ↔ Rust)

| Purpose | Python | Rust |
|---|---|---|
| TeX/ID patterns | `re` | `regex` |
| HTTP client | `requests` | `reqwest` + `tokio` |
| tar.gz extraction | `tarfile` / `gzip` | `flate2` + `tar` |
| PDF metadata | `pymupdf` | `lopdf` |
| DeepSeek API | `openai` SDK | `reqwest` (manual SSE) |
| CLI args | `argparse` | `clap` (derive) |
| GUI | `tkinter` | Tauri 2 |
| Concurrency | `threading` | `tokio::spawn` |
| Serialization | `json` | `serde` + `serde_json` |
| File ops | `shutil` | `std::fs` |
| Temp dirs | `tempfile` | `tempfile` |

---

## 16. Things to Watch Out For

### Cache Handling
The cache logic (`download_source` → validate → repair → fallback) is resilient
but file locking behavior on Windows matters. If the cache dir is locked by another
process, re-download falls back to `_freshN` suffixes. This can accumulate stale
cache dirs.

### Heuristic Extraction
The body/appendix split is regex-based and heuristic. Before changing the split
logic, test against multiple real papers. Unusual LaTeX structures (nested includes,
custom document classes, non-standard sectioning) may produce incorrect splits.

### Chat Scope Creep
The chat panels are intentionally lightweight. Avoid adding a full retrieval
pipeline unless there's a clear use case. The current context-building approach
(sending paper text as system message) is simple and sufficient for the target
workflow.

### Workspace Assumptions
The paper list is inferred from folder names under the workspace. The naming
convention is `{id}_{title}`. Changing folder naming rules, arXiv ID parsing,
or duplicate handling may break existing workspace folders.

### Desktop and Web Divergence (Legacy)
The old Python codebase had separate desktop and web implementations that shared
extraction logic but diverged in workflow (persistent workspace vs temporary
outputs). The Rust rewrite consolidates to a single desktop app. Don't reintroduce
a separate web version unless there's a strong reason.

### README Style
The project uses a concise, practical README style:
- Describe what it does and what it does not do
- Keep examples short but real
- Use relative paths for screenshots (GitHub rendering)
- Don't oversell the tool

---

## 17. Archived Documentation

The following documents are preserved in `docs/archive/` for historical reference.
Most of their content has been incorporated into this guidebook.

| File | What it was | Still relevant? |
|---|---|---|
| `ARCHITECTURE.md` | Original architecture doc with feature parity table | Mostly covered here |
| `tech_memo.md` | Python-era technical memo (546 lines) | Config paths, per-paper structure covered here; packaging/deps outdated |
| `tui_spec.md` | TUI design spec (never implemented) | Only if someone builds the TUI |

---

## 18. Future Work (Open Questions)

- **Android build** — Requires Java/Android SDK setup, responsive layout design
- **PDF.js fallback** — For platforms without native PDF rendering (Android, some Linux)
- **Better LaTeX parsing** — Currently regex-based; could use a proper TeX parser
- **Chat context window management** — Long conversations may exceed token limits
- **Multi-workspace** — Currently single workspace, could support multiple
- **Search** — Full-text search across all papers in workspace
- **Sync** — Cloud sync of workspace between devices
