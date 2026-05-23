# ArXivCat

[中文说明](README_zh.md)

ArXivCat is a small desktop tool for working with arXiv LaTeX source packages.
It downloads the source, expands LaTeX `\input` / `\include`, and exports cleaner paper text into `body.tex` and `appendix.tex`.

The project is meant for a simple workflow: paste an arXiv URL or ID, inspect the extracted text, make small edits, and optionally use the built-in DeepSeek chat panel to ask quick questions about the current paper content.

![ArXivCat screenshot](assets/screenshot.png)

## Features

- download source packages from an arXiv URL, PDF URL, or raw arXiv ID
- extract and cache arXiv source locally (unlimited cache, no size limit)
- recursively expand nested LaTeX `\input` and `\include`
- detect the main TeX file automatically
- export:
  - `body.tex`
  - `appendix.tex` when available
- **paper history sidebar**: left panel lists all previously downloaded papers, click to reload instantly
- resizable three-column layout (paper list / preview / chat)
- preview and lightly edit extracted text in a Tkinter GUI
- use a lightweight DeepSeek chat panel on the right side with streaming output

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

1. Paste an arXiv URL or ID.
2. Click `Run`.
3. Review the extracted `body` or `appendix` view.
4. Optionally use:
   - `Copy`
   - `Overwrite`
   - `Open Folder`
   - `Strip Comments`
5. Use the right-side chat panel for quick summaries or explanations.

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

- cache: `%APPDATA%/ArxivCat/downloads/`
- extracted output: `%APPDATA%/ArxivCat/outputs/`

If a cache directory becomes unreadable, ArXivCat may re-download the source or write to a `*_freshN` directory.

## Packaging

Windows packaging currently uses `build.ps1` together with PyInstaller and the `arxivcat` conda environment.

## For maintainers

If you plan to maintain or extend the project, read `tech_memo.md` first.
