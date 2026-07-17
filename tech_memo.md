# ArXivCat Technical Memo

This document is for future maintainers.

It is intentionally more detailed than the README and focuses on how the project is structured, how the current packaging works, and what conventions have emerged so far.

## 1. Project purpose

ArXivCat is a focused utility for extracting readable paper text from arXiv LaTeX source packages.

The core workflow is:

1. parse an arXiv ID from user input
2. download the source tarball
3. unpack the source into a cache directory
4. locate the main TeX file
5. recursively expand `\input` / `\include`
6. split output into `body.tex` and `appendix.tex`
7. write the extracted files into the selected workspace
8. generate a `description.md` brief for the paper
9. preview and lightly edit the result in the GUI

The current desktop workflow is workspace-oriented: the user opens a folder, each paper becomes a subfolder, and extracted `body.tex`, optional `appendix.tex`, `note.txt`, `description.md`, the downloaded PDF, and a description-ready flag can live together there.

Lightweight chat panels exist as reading assistance, not as full retrieval or agent systems. The desktop app currently uses DeepSeek through the OpenAI-compatible SDK. The Web app still uses Gemini.

## 2. Current top-level files

- `main.py`: GUI entry point
- `cli.py`: command-line entry point (argparse subcommands: `workspace`, `paper`, `chat`, `token`)
- `test_cli.py`: CLI test suite
- `arxivcat/core.py`: download and extraction logic
- `arxivcat/presenter.py`: UI-agnostic application logic (Presenter pattern)
- `arxivcat/chat_service.py`: chat logic (DeepSeek API, session persistence, description generation)
- `arxivcat/ui/tkinter_ui.py`: current GUI implementation (~2600 lines)
- `arxivcat/ui/cli_ui.py`: CLI UI implementation (implements UIProtocol for colored terminal output)
- `arxivcat/ui/base.py`: UI protocol (abstract interface between UI and Presenter)
- `build.ps1`: Windows packaging script
- `pyi_rth_tk_env.py`: runtime hook for packaged Tk/Tcl environment setup
- `requirements.txt`: desktop dependencies
- `requirements-web.txt`: Web version dependencies
- `web/app.py`: Flask backend for the Web version
- `web/templates/` and `web/static/`: Web UI files

## 3. Architecture summary

### 3.1 Presenter pattern

The project currently uses a simple presenter-style separation.

- `Presenter` owns application logic and workflow state.
- `TkApp` owns widgets and UI rendering.
- `UIProtocol` defines the interface between them.

The goal here is not heavy abstraction. It is just enough separation so extraction logic is not deeply mixed into Tk widget code.

### 3.2 Extraction logic

Most of the paper-processing logic lives in `arxivcat/core.py`. Key functions and their responsibilities:

| Function | Responsibility |
|---|---|
| `extract_arxiv_id(input_str)` | Parse arXiv ID from URL, raw ID, or versioned ID |
| `extract_arxiv_id_from_pdf(pdf_path)` | Extract arXiv ID from PDF metadata using PyMuPDF |
| `fetch_title_from_arxiv(arxiv_id)` | Scrape paper title from arXiv abstract page |
| `download_source(arxiv_id, downloads_dir)` | Download source tarball from `arxiv.org/src/`, verify cache integrity |
| `download_pdf(arxiv_id, output_dir)` | Download PDF from `arxiv.org/pdf/` |
| `find_main_tex(paper_dir)` | Locate the `.tex` file containing `\documentclass` |
| `expand_inputs(tex_content, base_dir)` | Recursively expand `\input`/`\include` directives, with cycle detection |
| `extract_body_and_appendix(tex_content)` | Heuristically split expanded text into `(body, appendix, error)` |
| `extract_body_from_dir(paper_dir, ...)` | Top-level orchestrator: find main → expand → validate → split → write |

**arXiv ID parsing** (`extract_arxiv_id`): Uses regex `(\d+[._]\d+(?:v\d+)?)`. After matching, `_` in the captured group is replaced with `.`. Returns `None` if no match.

**PDF ID extraction** (`extract_arxiv_id_from_pdf`): Three strategies tried in order:
1. PDF metadata (`subject`, `keywords`, `title`, `author` fields) — bare pattern `\d{4}\.\d{4,5}(?:v\d+)?`
2. First page text — tries `arXiv:\s*(\d{4}\.\d{4,5}(?:v\d+)?)` first, then bare pattern
3. First 3 pages — tries `arXiv:` prefix pattern on each page
Returns `None` if PyMuPDF unavailable, PDF unreadable, or no match.

**Title fetching** (`fetch_title_from_arxiv`): GET `arxiv.org/abs/{id}`, extracts `<meta property="og:title" content="..."` with 15s timeout.

**Main TeX detection** (`find_main_tex`):
1. If `main.tex` exists at root level, return it immediately.
2. Otherwise iterate top-level `*.tex` files (non-recursive), return the first one containing `\documentclass` in its content.
3. Return `None` if nothing found.

**Input expansion** (`expand_inputs`): Recursively resolves `\input{...}` and `\include{...}` via regex `\\(?:input|include)\s*\{([^}]+)\}`. Resolution tries: `base_dir/name`, `root_dir/name`, and appending `.tex` if missing. Cycle prevention: a `_seen` set tracks all expanded file paths; re-encountered files are left as-is in output. After expansion, `extract_body_from_dir` re-validates that no unresolved `\input{...}` or `\include{...}` remain — aborts if any are found.

**Cache validation** (in `download_source`): When cache exists, checks 4 conditions:
1. `find_main_tex()` returns a file
2. `_can_walk_dir()` — `rglob("*")` succeeds
3. `_can_read_tex_files()` — all `*.tex` files readable as UTF-8
4. `_all_inputs_readable()` — all input/include references resolvable and readable
Failing cache gets `_repair_permissions()` (chmod 777/666 on all files), re-checked. If still bad, `shutil.rmtree` + re-download. Windows file locks during deletion fall back to `_fresh1`, `_fresh2`, etc. suffixes on the folder name.

Body/appendix split uses these heuristic markers in order:
1. Body starts at `\begin{abstract}`, first `\section`, or `\begin{document}` (whichever comes first)
2. Split point is the earliest of: `\appendix`, `\begin{appendix}`, or `\bibliography{...}`/`\bibliographystyle{...}` — whichever appears after the body start. If none found, falls back to the last section matching "Conclusion"/"Summary", or `\end{document}`
3. Appendix text has `\bibliography{...}` and `\clearpage` cleaned out. Appendices shorter than 50 chars are treated as non-existent

**Tar safety**: `_is_safe_tar_member()` resolves the target dir and each member's extraction path; returns `True` only if the resolved member path is the target dir or a descendant (prevents `..` path traversal). Suspicious members are skipped with a warning.

All extraction logic is heuristic by design — practical on common arXiv papers, not mathematically complete for all LaTeX projects.

### 3.3 GUI logic

The current GUI is Tkinter-based.

Notable UI features:

- arXiv input field
- run button
- workspace folder picker
- paper list panel
- PDF scanning button
- batch download button
- body / appendix / note / description switcher
- preview panel
- copy / open folder / open PDF / strip comments actions
- optional log panel
- right-side DeepSeek chat panel
- workspace-level Global Chat dialog over all paper descriptions
- small inline status and word / character count

The chat panel currently keeps short-lived in-memory history only.

### 3.4 Workspace logic

The desktop app treats a user-selected folder as the workspace. All workspace state and orchestration lives in the `Presenter` class.

**Presenter key state:**
- `workspace_path`: selected workspace folder
- `output_dir`: currently loaded paper folder
- `_task_busy`: mutex-guarded flag preventing concurrent operations
- `_download_all_cancelled`: cancellation flag for batch download

**Concurrency model:**
- Single-operation flows (`run_fetch`, `scan_workspace_pdfs`) spawn a `threading.Thread` daemon
- Batch `Download All` uses `ThreadPoolExecutor(max_workers=25)` with `as_completed()` for progress
- Cancellation: an atomic `_download_all_cancelled` flag is checked at multiple points inside each worker; remaining queued futures are `.cancel()`'d. In-flight workers complete their current step then return early
- All worker-to-UI communication goes through `UIProtocol` methods

**`_emit_log()` status mapping:**
Calls from `core.py` log functions are forwarded to `ui.add_log()`. Additionally, specific substrings trigger `ui.set_mini_status()` updates:
- `"Downloading"` + `"%"` → `"downloading..."`
- `"Download complete"` → `"downloaded"`
- `"Extracting"` → `"extracting..."`
- `"Expanding"` → `"expanding..."`
- `"Parsing body"` → `"parsing..."`
- `"Already cached"` → `"cached"`
- `"[OK]"` + `"saved"` → `"done"`

**Paper list construction** (`get_paper_list()`): Iterates workspace subdirectories, skipping hidden dirs (`.`-prefixed) and internal dirs (`arxivcat_global_chats`). Parses folder name `{id_part1}_{id_part2}_{title...}` to extract arXiv ID (with `.` separator) and title. Checks paper status:
- `has_body = (body.tex exists)`
- `description_ready = (description.md non-empty AND .description_ready exists)`
- `is_complete = has_body AND description_ready`

**`_process_pending_paper()`** (single paper unit for batch download): Two paths:
- Paper has body but missing description → only rebuilds description
- Paper needs full download → `download_source` → `extract_body_from_dir` → `download_pdf` → meta files → build description
Checks cancellation flag before each major step.

**Workspace initialization** (`open_workspace`):
- Saves path to `%APPDATA%/ArxivCat/config.json` (key: `workspace_path`)
- Creates workspace dir and `arxivcat_global_chats/` subdirectory
- Refreshes paper list and updates window title

Workspace behavior:

- the last workspace path is saved in `%APPDATA%/ArxivCat/config.json`
- `TkApp._init_workspace()` restores the saved path if it still exists
- otherwise the app prompts the user to choose a folder
- each paper is represented by a subfolder named from the arXiv ID and sanitized title
- folders without `body.tex` are treated as pending downloads
- folders with `body.tex` but without a complete description are also treated as incomplete
- the left paper list is derived from the current workspace folder, not from the download cache

`Scan PDFs` only scans PDF files directly under the workspace root. For each PDF, it tries to extract an arXiv ID and then creates a pending paper folder if that base ID is not already present.

`Download All` processes incomplete paper folders with a `ThreadPoolExecutor(max_workers=25)`. Each worker calls `_process_pending_paper()`: download source → extract → download PDF → write meta files → build description. The UI refreshes the paper list incrementally. Interrupt is supported via a cancel flag checked before each submission and propagated to queued futures.

Two special directories under the workspace are excluded from paper recognition:
- `arxivcat_global_chats/` — stores Global Chat session JSONs
- `arxiv_chats/` — stores per-paper side chat session JSONs (one per paper folder)

The current desktop flow treats a paper as fully ready only when both of these are true:

- `body.tex` exists
- `description.md` is non-empty and `.description_ready` exists

This extra flag is intentional. It helps detect partially written descriptions after interruption or app shutdown.

### 3.5 Web version

The Web version is separate from the desktop workspace flow.

Current behavior in `web/app.py`:

- Flask backend with CORS enabled
- `/api/extract` downloads source and extracts text for a submitted arXiv ID or URL
- `/api/strip-comments` removes LaTeX comments from submitted text
- `/api/chat` sends paper context and chat history to Gemini
- source cache is `%APPDATA%/ArxivCat/downloads/`
- Web extraction output is `%APPDATA%/ArxivCat/outputs/`

### 3.6 CLI version

`cli.py` provides a full command-line alternative with argparse subcommands:

| Group | Subcommand | Purpose |
|---|---|---|
| `workspace` | `open` | Set and cache workspace path |
| `workspace` | `scan` | Scan workspace for PDFs, create paper folders |
| `paper` | `list` / `download` / `download-all` | Manage papers |
| `paper` | `preview` / `note` / `strip` / `open` / `pdf` / `info` | View and edit paper content |
| `chat` | `side` | Interactive chat scoped to one paper |
| `chat` | `global` | Interactive chat over all workspace descriptions |
| `token` | `status` / `set` / `validate` | Manage DeepSeek API token |

The CLI reuses `Presenter` and `ChatService` directly, with `CliUI` (in `arxivcat/ui/cli_ui.py`) implementing `UIProtocol` for colored terminal output.

**CLI chat loops** (`cli.py`):
- Side chat (`chat side <id>`): Interactive REPL with commands `/quit`, `/model <Flash|Pro>`, `/thinking` (toggle), `/context [field]` (toggle body/appendix/description/note), `/save`, `/load`, `/history`, `/clear`, `/help`. Context is built from the paper folder's `body.tex`/`appendix.tex`/`description.md`/`note.txt` based on toggled fields. Sessions auto-saved under `<paper_dir>/arxiv_chats/`.
- Global chat (`chat global`): Same REPL, scoped to all workspace papers. Builds context from `description.md` by default. Sessions saved under `<workspace>/arxivcat_global_chats/`.
- Both use `ChatService.stream_chat()` with `include_thinking=True`, exposing the cancel via SIGINT. System prompts differ: side chat emphasizes paper-level Q&A with arXiv ID attribution; global chat treats papers as numbered entries.

### 3.7 UIProtocol reference

`UIProtocol` (in `arxivcat/ui/base.py`) is a `typing.Protocol` decorated with `@runtime_checkable`. Every UI backend must implement this interface. The `Presenter` calls these methods to update any UI without knowing which implementation is active:

| Method | Purpose |
|---|---|
| `add_log(msg)` | Append a log line. `[OK]`/`[ERROR]`/`[INFO]` prefix controls color |
| `set_mini_status(msg, level)` | Update small inline status; levels: `info`, `ok`, `error` |
| `set_preview(content, label)` | Replace main preview text area |
| `set_buttons_enabled(enabled)` | Enable/disable action buttons (copy, open, etc.) |
| `set_run_busy(busy)` | Toggle Run button between ready/busy states |
| `set_paper_actions_busy(busy)` | Disable paper list actions during background work |
| `show_toast(msg, duration_ms)` | Transient status message that auto-clears |
| `get_url_input()` | Return current arXiv URL/ID from input field |
| `get_view_mode()` | Return dropdown: `"body"`, `"appendix"`, `"note"`, `"description"` |
| `get_preview_text()` | Return current preview area text (for strip comments) |
| `clear_log()` | Clear all log entries |
| `set_url_input(url)` | Set arXiv URL/ID input field value |
| `set_paper_list(papers)` | Update left panel paper list (list of dicts with: `arxiv_id`, `title`, `folder_name`, `has_body`, `description_ready`, `is_complete`) |
| `set_title(title)` | Set window title |
| `build_paper_description(paper_dir, arxiv_id, title)` | Trigger description generation (delegates to `ChatService`) |
| `set_download_all_state(interrupt_mode)` | Update Download All button between idle/interrupt states |
| `run()` | Start UI event loop (blocking call) |

Tkinter UI: implements all methods, using `root.after()` for thread-safe marshalling. CLI UI: implements most as terminal output; `build_paper_description`, `get_preview_text`, `set_buttons_enabled`, etc. are no-ops or unsupported in CLI context.

## 4. Chat implementation notes

The chat panels are intentionally lightweight.

### 4.1 Desktop chat

Desktop chat logic lives in `arxivcat/chat_service.py` (the `ChatService` class), separate from the UI layer.

Current behavior:

- uses the OpenAI SDK with `base_url="https://api.deepseek.com"`
- API key is a DeepSeek token saved as `deepseek_api_key` in `%APPDATA%/ArxivCat/config.json`
- model choices are:
  - `Flash` → `deepseek-v4-flash`
  - `Pro` → `deepseek-v4-pro`
- model preference is saved as `chat_model` in the same config file
- sends the current preview text as context
- includes the recent in-memory chat history, currently the last 12 entries
- `stream_chat()` handles streaming output, calling `on_token` / `on_status` / `on_complete` callbacks
- supports a stop button by setting an internal cancel flag checked in the stream loop
- optionally sends DeepSeek thinking parameters when deep thinking is enabled
- shows rough performance metrics such as TTFT, tokens/sec, and estimated token counts
- `Reset` clears the in-memory history

**Session persistence:** Chat sessions are saved automatically as JSON files. Side chat sessions go under `<paper_dir>/arxiv_chats/`, Global Chat sessions under `<workspace>/arxivcat_global_chats/`. Each session is named `YYYYMMDD_HHMMSS.json` (with numeric suffix on conflict). Sessions store: `title`, `kind`, `model`, `deep_thinking`, `messages`, `context_selection`, `context_snapshot`, `view_name`, `updated_at`. The `ChatService` (via module-level helpers `save_chat_session`, `load_chat_session`, `list_chat_sessions`, `rename_chat_session`, `delete_chat_session`) manages the full CRUD cycle.

The desktop GUI now has two chat surfaces built from the same chat-panel abstraction:

- side chat: scoped to the currently loaded paper preview
- Global Chat: scoped to all numbered `description.md` files in the workspace

Global Chat uses the same visual structure and the same `Flash` / `Pro` model controls plus deep thinking toggle. Its difference is the context source, not the widget structure.

**Context building:** Two module-level functions construct chat context:

- `build_side_chat_context(paper_dir, selection)` — reads `body.tex`, `appendix.tex`, `description.md`, `note.txt` from the paper folder based on which fields are `True` in the `selection` dict `{"body": bool, "appendix": bool, "description": bool, "note": bool}`
- `build_description_context(entries, selection)` — for Global Chat; iterates all workspace papers, prepends arXiv ID and title, wraps each paper's enabled fields in numbered `"Paper [1]\n---\n..."` blocks

**Selection delta tracking:** Chat panels remember which context sections were sent in the last API call (`last_sent`). `compute_selection_delta(current, last_sent)` returns newly-enabled fields. When the user toggles a context checkbox mid-conversation, newly added content is injected as a system message before the next user message, so the model sees it without losing conversation continuity. Same pattern applies for global chat via `compute_global_selection_delta()`.

**Streaming (`ChatService.stream_chat`):**
- Runs in a daemon thread so the UI stays responsive
- Open streaming: `client.chat.completions.create(stream=True)`
- Checks `_cancelled` flag at the top of each chunk iteration
- Collapses multiple consecutive newlines into a single `\n` in each token for cleaner display
- On cancellation: calls `on_status("cancelled")`, no completion callback
- On success: calls `on_status(model)`, then `on_complete(full_text.strip())`
- Deep thinking: sends `extra_body={"thinking": {"type": "enabled"}}` and `reasoning_effort="high"`
- Metrics: reports TTFT (ms), tokens/sec, prompt_tokens, completion_tokens via `on_status`

### 4.2 Web chat

The Web chat lives in `web/app.py`.

Current behavior:

- uses `google-genai`
- reads `GEMINI_API_KEY` from the environment
- model: `gemini-2.0-flash-lite`
- receives context and history from the frontend
- includes the last 10 history messages
- returns a normal JSON response rather than streaming

### 4.3 Description generation

Description generation lives in `arxivcat/chat_service.py` (`ChatService.build_description()`) and is invoked from `Presenter`.

Current behavior:

- uses a fresh DeepSeek chat completion, not reusing side-chat history
- forces `deepseek-v4-flash`, `max_tokens=1400`
- reads `body.tex` and optional `appendix.tex`
- system prompt instructs the model to produce structured markdown with these sections:
  - `# Overview`
  - `## Problem`
  - `## Method`
  - `## Key Contributions`
  - `## Technical Details`
  - `## Search Tags`
  - `## Good Match Queries`
- user prompt includes the arXiv ID, title, and the extracted text snippet
- writes structured markdown into `description.md`
- writes `.description_ready` only after the description content is fully written
- is triggered during single-paper download and batch `Download All`

This keeps `description.md` separate from ad hoc notes and makes workspace-level Global Chat possible without building a full retrieval stack.

## 5. Build and packaging notes

### 5.1 Why packaging needed extra care

Tkinter packaging on Windows can fail even when source execution works.

The specific issue seen in this project was a Tcl/Tk version mismatch during PyInstaller output, producing runtime errors like:

- missing `init.tcl`
- Tcl version conflicts such as `8.6.12` vs `8.6.15`

This happened because a build could accidentally mix:

- the wrong Python environment
- stale PyInstaller cache / spec files
- mismatched Tcl/Tk resources

### 5.2 Current packaging approach

`build.ps1` now does the following:

- explicitly uses `D:\anaconda3\envs\arxivcat\python.exe`
- removes previous `build/`, `dist/`, and generated `.spec`
- runs a clean PyInstaller build
- explicitly bundles:
  - `Library/lib/tcl8.6`
  - `Library/lib/tk8.6`
  - `Library/bin/tcl86t.dll`
  - `Library/bin/tk86t.dll`
- includes project and dependency data needed by the packaged app, including `arxivcat`, `requests`, `google`, `openai`, `fitz`, and `pymupdf`
- uses `pyi_rth_tk_env.py` to set:
  - `TCL_LIBRARY`
  - `TK_LIBRARY`
  at runtime inside the packaged app
- writes `dist/ArxivCat.exe`
- compresses it as `dist/ArxivCat-v0.7.0-win64.zip`

This setup exists for a reason. Future maintainers should be careful before “simplifying” it.

### 5.3 Packaging assumptions

The current packaging script assumes:

- Windows
- conda environment name and location matching the existing setup
- PyInstaller-based one-file build
- the release version in `build.ps1` is kept in sync with the intended release
- desktop packaging only; the Web version is run from source with Flask

If the environment path changes, `build.ps1` should be updated accordingly.

## 6. Runtime paths and config

Shared base directory:

- `%APPDATA%/ArxivCat/` on Windows when `APPDATA` is available
- otherwise `Path.home()/ArxivCat`

Desktop:

- config: `%APPDATA%/ArxivCat/config.json`
- source cache: `%APPDATA%/ArxivCat/downloads/`
- workspace: user-selected folder

**Per-paper folder structure** (under workspace):
```
<arXivID>_<SanitizedTitle>/
  body.tex             # Extracted main text (always present for a complete paper)
  appendix.tex          # Optional appendix text (>50 chars)
  description.md        # AI-generated structured summary
  .description_ready    # Flag file (content: "ok\n"), marks complete description
  note.txt              # User's own notes
  <arXivID>.pdf         # Downloaded PDF (optional)
  arxiv_chats/          # Side chat session JSONs (YYYYMMDD_HHMMSS.json)
```
Plus at workspace root: `arxivcat_global_chats/` for Global Chat sessions.

**Config keys** in `config.json`:
- `deepseek_api_key` — DeepSeek API token
- `chat_model` — `"Flash"` or `"Pro"`
- `workspace_path` — last opened workspace folder path

**Config I/O helpers** (in `presenter.py`): `load_cached_token()`, `save_token()`, `save_model_preference()`, `load_model_preference()`, `save_workspace_path()`, `load_workspace_path()` — read/write individual keys, creating the file and parent directories on first write. No special encoding, plain `json.dump`/`json.load`.

Web:

- source cache: `%APPDATA%/ArxivCat/downloads/`
- output: `%APPDATA%/ArxivCat/outputs/`
- Gemini key is read from `GEMINI_API_KEY`, not from `config.json`

## 7. Dependency notes

Desktop `requirements.txt` currently includes:

- `requests`
- `google-genai`
- `pymupdf`

The desktop code also imports `openai` for DeepSeek chat. If setting up a fresh environment, make sure `openai` is installed even if the requirements file has not been updated yet.

Web `requirements-web.txt` currently includes:

- `flask`
- `flask-cors`
- `requests`
- `google-genai`

## 8. README policy

The README should stay practical.

The project style so far suggests:

- do not oversell the tool
- explain what it does and what it does not do
- keep examples short but real
- include the screenshot with a relative path so GitHub rendering works

The screenshot should remain referenced like this:

```md
![ArXivCat screenshot](assets/screenshot.png)
```

Using a local absolute path would break rendering on GitHub.

## 9. Commit comment style in this repo

Recent commit history suggests a simple, low-ceremony style.

Examples include:

- `some updates`
- `modify readme`
- `0.3.0`
- `fix tkinter build`
- `0.2.1, switched to tkinter and fixed extraction`

Very short summary of the style:

- keep it concise
- usually lowercase
- say what changed plainly
- version bumps can be just the version number
- no need for conventional commit prefixes unless the owner explicitly wants that later

A practical rule of thumb for this repo:

- feature or doc tweak: a short natural phrase
- version release: just the version number is acceptable
- bug fix: a short direct description

## 10. Things to watch out for

### 10.1 Cache handling

The cache logic tries to be resilient, including repair / fallback behavior when a cache directory is unreadable or locked.

That is useful, but it also means file and directory behavior on Windows matters a lot.

### 10.2 Heuristic extraction

The body/appendix split is heuristic. Before changing it, test against multiple real papers.

### 10.3 Chat scope creep

The chat panel is easy to bloat. It should remain simple unless there is a strong reason to add more complexity.

In particular, avoid adding a full retrieval pipeline unless the use case is clear.

### 10.4 Workspace assumptions

The desktop paper list is inferred from folder names under the workspace.

Be careful when changing folder naming rules, arXiv ID parsing, or duplicate handling. Existing workspace folders may already follow the current `id_title` naming convention.

### 10.5 Desktop and Web divergence

The desktop and Web versions share extraction logic but not the full app workflow.

Desktop uses a persistent workspace and DeepSeek chat. Web uses the `%APPDATA%/ArxivCat/outputs/` directory and Gemini chat. If adding a feature, decide explicitly whether it belongs to one frontend or both.

## 11. Suggested maintenance habits

- test both GUI and CLI after touching extraction logic
- test PDF scanning after changing arXiv ID parsing or PyMuPDF-related code
- test `Download All` after changing workspace, output directory behavior, or description completeness rules
- test packaged Windows builds after touching Tk or build config
- keep release artifacts aligned with the version shown in the GUI
- avoid mixing unrelated changes into the same commit
- when in doubt, prefer direct and readable code over abstraction

## 12. Open questions for future work

Reasonable future improvements could include:

- better context selection for chat
- making desktop dependencies and `requirements.txt` fully consistent
- easier configuration of chat models
- more robust TeX main-file detection
- more robust arXiv ID extraction from unusual PDFs
- improved extraction heuristics for unusual paper layouts
- clearer feature parity decisions between desktop and Web
- a small smoke test for packaged app startup

But none of these are mandatory right now. The current project is intentionally simple and that simplicity is worth preserving.
