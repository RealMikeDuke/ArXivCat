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
- `cli.py`: command-line entry point
- `arxivcat/core.py`: download and extraction logic
- `arxivcat/presenter.py`: UI-agnostic application logic
- `arxivcat/ui/tkinter_ui.py`: current GUI implementation
- `arxivcat/ui/base.py`: UI protocol
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

Most of the paper-processing logic lives in `arxivcat/core.py`.

Important responsibilities include:

- parsing arXiv IDs from different input forms
- extracting arXiv IDs from local PDFs with PyMuPDF
- fetching paper titles from arXiv pages
- downloading arXiv source packages
- downloading PDFs
- handling partially broken cache states
- checking for unsafe tar paths before extraction
- finding the main TeX file
- recursively expanding `\input` and `\include`
- heuristically splitting body vs appendix

This logic is heuristic by design. It is meant to be practical on common arXiv papers, not mathematically complete for all LaTeX projects.

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

The desktop app now treats a user-selected folder as the workspace.

The key state lives in `Presenter`:

- `workspace_path`: selected workspace folder
- `output_dir`: currently loaded paper folder

Workspace behavior:

- the last workspace path is saved in `%APPDATA%/ArxivCat/config.json`
- `TkApp._init_workspace()` restores the saved path if it still exists
- otherwise the app prompts the user to choose a folder
- each paper is represented by a subfolder named from the arXiv ID and sanitized title
- folders without `body.tex` are treated as pending downloads
- folders with `body.tex` but without a complete description are also treated as incomplete
- the left paper list is derived from the current workspace folder, not from the download cache

`Scan PDFs` only scans PDF files directly under the workspace root. For each PDF, it tries to extract an arXiv ID and then creates a pending paper folder if that base ID is not already present.

`Download All` now processes incomplete paper folders with a thread pool. It can either download and extract a missing paper or only rebuild `description.md` if the body already exists. The UI refreshes the paper list incrementally so interrupted runs still leave visible progress.

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

The Web version does not currently use the desktop workspace folder model.

## 4. Chat implementation notes

The chat panels are intentionally lightweight.

### 4.1 Desktop chat

The desktop chat lives in `arxivcat/ui/tkinter_ui.py`.

Current behavior:

- uses the OpenAI SDK with `base_url="https://api.deepseek.com"`
- API key is a DeepSeek token saved as `deepseek_api_key` in `%APPDATA%/ArxivCat/config.json`
- model choices are:
  - `Flash` → `deepseek-v4-flash`
  - `Pro` → `deepseek-v4-pro`
- model preference is saved as `chat_model` in the same config file
- sends the current preview text as context, truncated to about 12000 characters
- includes the recent in-memory chat history, currently the last 12 entries
- supports streaming output
- supports a stop button by cancelling the local stream loop
- optionally sends DeepSeek thinking parameters when deep thinking is enabled
- shows rough performance metrics such as TTFT, tokens/sec, and estimated token counts
- `Reset` clears the in-memory history

The desktop GUI now has two chat surfaces built from the same chat-panel abstraction:

- side chat: scoped to the currently loaded paper preview
- Global Chat: scoped to all numbered `description.md` files in the workspace

Global Chat uses the same visual structure and the same `Flash` / `Pro` model controls plus deep thinking toggle. Its difference is the context source, not the widget structure.

### 4.2 Web chat

The Web chat lives in `web/app.py`.

Current behavior:

- uses `google-genai`
- reads `GEMINI_API_KEY` from the environment
- model: `gemini-2.0-flash-lite`
- receives context and history from the frontend
- truncates context to about 8000 characters
- includes the last 10 history messages
- returns a normal JSON response rather than streaming

Important limitation:

- this is not full-paper retrieval
- it is closer to “chat over current preview text”
- long papers are truncated before sending to the model

So if someone wants better paper QA in the future, the likely next step is chunking or retrieval, not simply making the prompt longer forever.

### 4.3 Description generation

Desktop description generation also lives in `arxivcat/ui/tkinter_ui.py` and is invoked from `Presenter`.

Current behavior:

- uses a fresh DeepSeek chat completion rather than reusing side-chat history
- currently forces `deepseek-v4-flash`
- reads `body.tex` and optional `appendix.tex`
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
- paper output: one subfolder per paper inside the workspace
- per-paper files usually include `body.tex`, optional `appendix.tex`, `note.txt`, `description.md`, `.description_ready`, and the downloaded PDF

Desktop config keys currently include:

- `deepseek_api_key`
- `chat_model`
- `workspace_path`

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
