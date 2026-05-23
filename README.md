# ArXivCat

[中文说明](README_zh.md)

ArXivCat is a small desktop tool for working with arXiv LaTeX source packages.
It downloads the source, expands LaTeX `\input` / `\include`, and exports cleaner paper text into `body.tex` and `appendix.tex`.

The project is meant for a simple workflow: paste an arXiv URL or ID, inspect the extracted text, make small edits, and optionally use the built-in DeepSeek chat panel to ask quick questions about the current paper content.

![ArXivCat screenshot](assets/screenshot.png)

## Features

- **Workspace mode**: open any folder as a workspace (like Obsidian); each subfolder is a paper
- Remembers last workspace on launch; "Open Folder" to switch anytime
- **Scan PDFs**: auto-detect arXiv IDs (with version, e.g. `2604.12630v1`) from PDFs in the workspace
- **Download All**: one-click batch download with progress (`3/10`), resumable
- Automatically downloads the PDF alongside LaTeX source
- **Open PDF**: view the arXiv PDF in your browser
- Download source packages from an arXiv URL, PDF URL, or raw arXiv ID
- Extract and cache arXiv source locally (unlimited cache)
- Recursively expand nested LaTeX `\input` and `\include`
- Detect the main TeX file automatically
- Export `body.tex` and `appendix.tex`
- Resizable three-column layout (paper list / preview / chat)
- Preview and lightly edit extracted text in a Tkinter GUI
- Lightweight DeepSeek chat panel with streaming output

## Scope

ArXivCat is intentionally narrow in scope.

- It is not a full LaTeX compiler.
- It does not guarantee perfect parsing for every paper source tree.
- The chat panel is meant for lightweight reading assistance, not full retrieval over arbitrary long papers.

## Installation

Install dependencies:

```bash
pip install -r requirements.txt
```

To use the chat panel, set `DEEPSEEK_API_KEY` in your environment.

## Run from source

GUI:

```bash
python main.py
```

CLI:

```bash
python cli.py --url 2601.11514
python cli.py --url https://arxiv.org/abs/2601.11514
python cli.py --url https://arxiv.org/pdf/2601.11514
```

## GUI workflow

1. On first launch, select a workspace folder.
2. Paste an arXiv URL or ID → click `Run` → paper is downloaded and extracted into the workspace.
3. Or: drop PDFs into the workspace folder → click `Scan PDFs` → then `Download All`.
4. Click any paper in the left panel to load it.
5. Use action buttons: `Copy`, `Overwrite`, `Open Folder`, `Open PDF`, `Strip Comments`.
6. Use the right-side chat panel for quick summaries or explanations.

## Chat panel

The current chat panel uses `deepseek-v4-flash` with streaming output.

Features:
- Streaming responses for real-time feedback
- Deep thinking mode toggle (optional)
- Stop button to cancel long responses
- Performance metrics display (TTFT, tokens/sec, token usage)
- Sends the current preview text as context
- Keeps short in-memory multi-turn history
- Clears chat memory when you click `Reset`
- Works best after a paper has already been loaded into the preview

## Output locations

- workspace: user-selected folder (each paper is a subfolder containing `body.tex`, `appendix.tex`, and the PDF)
- download cache: `%APPDATA%/ArxivCat/downloads/`
- config: `%APPDATA%/ArxivCat/config.json`

If a cache directory becomes unreadable, ArXivCat may re-download the source or write to a `*_freshN` directory.

## Packaging

Windows packaging currently uses `build.ps1` together with PyInstaller and the `arxivcat` conda environment.

## For maintainers

If you plan to maintain or extend the project, read `tech_memo.md` first.
