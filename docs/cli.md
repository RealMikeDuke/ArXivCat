# ArXivCat CLI

ArXivCat is a command-line tool for downloading, extracting, and managing arXiv papers. It downloads LaTeX source code, splits it into readable `body.tex` / `appendix.tex`, generates AI-powered descriptions, and supports interactive chat over papers via DeepSeek.

**Binary**: `arxivcat` (Rust, built via `cargo build --release --bin arxivcat`).

---

## Quick Start

```powershell
# 1. Set your workspace
arxivcat workspace open F:\zrs\paper

# 2. Set your DeepSeek API key (for AI features)
arxivcat token set

# 3. Download a paper (accepts URLs or raw IDs)
arxivcat paper download https://arxiv.org/abs/2501.12948

# 4. Download all pending papers
arxivcat paper download-all

# 5. List papers in workspace
arxivcat paper list

# 6. Preview paper content
arxivcat paper preview 2501.12948 -v body

# 7. Chat with papers
arxivcat chat side 2501.12948
arxivcat chat global
```

---

## Global Flags

These can appear anywhere in the command (before or after the subcommand):

| Flag | Description |
|---|---|
| `-w, --workspace <PATH>` | Override workspace path for this invocation. If omitted, reads from config. If passed but path does not exist, errors immediately. |
| `--json` | Output machine-readable JSON. Supported by: `list`, `download`, `download-all`, `preview`, `info`, `token status`. |
| `-h, --help` | Print help for the current command. |
| `-V, --version` | Print binary version. |

---

## Commands

### `workspace`

Manage the workspace folder where papers are stored.

```
arxivcat workspace open <PATH>    # Set workspace (persisted to config)
arxivcat workspace scan           # Scan workspace for untracked PDFs
```

| Subcommand | Description |
|---|---|
| `open <PATH>` | Persist workspace path to `%APPDATA%\ArxivCat\config.json`. |
| `scan` | Scan workspace root for PDFs, extract arXiv IDs from PDF metadata, create paper folders with `note.txt` + `description.md` stubs. Skips already-tracked papers. |

---

### `paper`

Core paper management: download, list, preview, edit notes.

```
arxivcat paper list                              # List all papers
arxivcat paper download <ID_OR_URL>              # Download & extract one paper
arxivcat paper download-all                      # Batch-process all pending papers
arxivcat paper preview <ID_OR_QUERY> -v <VIEW>   # Print file content
arxivcat paper note <ID_OR_QUERY> [TEXT]         # View/write note
arxivcat paper strip <ID_OR_QUERY>               # Strip LaTeX comments
arxivcat paper info <ID_OR_QUERY>                # Show metadata + file sizes
arxivcat paper open <ID_OR_QUERY>                # Open folder in file manager
arxivcat paper pdf <ID_OR_QUERY>                 # Open PDF
```

#### `list`

Show all papers with status indicators (AI decoupled):
- `[C]` Complete — body.tex present
- `[.]` Incomplete — missing body.tex
The bracket column also shows `desc`/`-` for description.md presence (informational).

**JSON output**: Array of paper objects:
```json
[{"arxiv_id":"2501.12948","title":"DeepSeek-R1...","has_body":true,"description_ready":true,"is_complete":true,...}]
```

#### `download <ID_OR_URL>`

Full pipeline: parse arXiv ID from raw ID or URL → download source tar.gz → extract body.tex / appendix.tex → download PDF. AI description is NOT part of the download pipeline — use `paper describe` explicitly.

**ID input**: Accepts raw IDs (`2501.12948`), versioned IDs (`2501.12948v2`), and URLs (`https://arxiv.org/abs/2501.12948`, `arxiv.org/pdf/2501.12948.pdf`, `www.arxiv.org/abs/2501.12948v3`).

**JSON output**:
```json
{"arxiv_id":"2501.12948","folder":"...","body_length":41953,"appendix_length":159056,"description_ready":true}
```

#### `download-all [--jobs N] [--force]`

Process every pending paper (missing body.tex) concurrently (`--jobs`, default 4, range 1-8). Papers in the 24h retry cooldown are skipped and reported; `--force` bypasses the cooldown. Ctrl-C stops the batch and exits 130.

Exit codes: 0 all succeeded / 8 partial (some failed) / 1 all failed / 130 interrupted.

**JSON output**:
```json
{"status":"done|partial|failed|cancelled","total":3,"success":2,"failed":1,"skipped":0,"failures":[{"id":"2501.12948","code":3,"kind":"http","message":"...","retryable":true}]}
```

**JSON output**: `{"status":"done","success":3,"total":5}` (or `{"status":"complete","count":0}` if nothing to do).

#### `preview <ID_OR_QUERY>`

Print the content of a paper file. Accepts raw ID, partial ID, or full URL.

| `-v` value | File |
|---|---|
| `body` (default) | `body.tex` |
| `appendix` | `appendix.tex` |
| `note` | `note.txt` |
| `description` | `description.md` |

**JSON output**:
```json
{"arxiv_id":"2501.12948","title":"DeepSeek-R1...","view":"body","content":"..."}
```

#### `note <ID_OR_QUERY> [TEXT]`

Three modes:
- **No args, no flags**: Print current `note.txt` content (or `(no note)` if empty).
- **With TEXT**: Overwrite `note.txt` with the given text.
- **`-e, --edit`**: Open `note.txt` in `$EDITOR` (falls back to `notepad` on Windows).

#### `strip <ID_OR_QUERY>`

Strip LaTeX comments (lines starting with `%`) from `body.tex`, collapse 3+ consecutive blank lines to 2, print to stdout. Useful for feeding content into other tools.

#### `info <ID_OR_QUERY>`

Print arXiv ID, title, folder path, status, and file sizes for each present file. Accepts raw ID, partial ID, or URL.

**JSON output**:
```json
{"arxiv_id":"2501.12948","title":"DeepSeek-R1...","folder":"...","has_body":true,"description_ready":true,"is_complete":true,"files":{"body.tex":41953,"appendix.tex":159056,"description.md":41965,"note.txt":582}}
```

#### `open <ID_OR_QUERY>`

Open the paper's folder in the system file manager.

#### `pdf <ID_OR_QUERY>`

Open the paper's PDF in the system viewer. Falls back to `https://arxiv.org/pdf/{id}` if no local PDF exists.

---

### `chat`

Interactive AI chat with papers. Requires a valid DeepSeek API key (`arxivcat token set`).

```
arxivcat chat side <ID_OR_QUERY>    # Chat scoped to one paper
arxivcat chat global                # Chat over all workspace papers
```

#### REPL Commands

Both `side` and `global` support these in-chat commands:

| Command | Description |
|---|---|
| `/model Flash\|Pro` | Switch between DeepSeek models. |
| `/thinking` | Toggle deep reasoning mode. |
| `/context body\|appendix\|description\|note` | Toggle which paper fields are included as context. |
| `/save` | Save current chat session to disk. |
| `/load` | Load a previously saved session. |
| `/history` | List saved sessions. |
| `/clear` | Clear the current conversation. |
| `/quit` | Exit chat. |

#### Session Storage

- Side chat: `{paper_folder}/arxiv_chats/{YYYYMMDD_HHMMSS}.json`
- Global chat: `{workspace}/arxivcat_global_chats/{YYYYMMDD_HHMMSS}.json`

---

### `token`

Manage the DeepSeek API key (stored in `%APPDATA%\ArxivCat\config.json`). Also readable from `DEEPSEEK_API_KEY` environment variable.

```
arxivcat token status      # Show whether token is configured (masked) + validate
arxivcat token set         # Prompt to enter token via stdin
arxivcat token validate    # Test token against api.deepseek.com/models
```

| Subcommand | Description |
|---|---|
| `status` | Print masked token (e.g. `sk-5...7abe`), response time, and validity. JSON: `{"configured":true,"masked":"sk-5...7abe","response_time_ms":151,"valid":true}`. |
| `set` | Prompt for token on stdin, save to config. |
| `validate` | Test cached token against `https://api.deepseek.com/models`. Returns response time and validity status. |

---

## ID Matching

Commands that accept `<ID_OR_QUERY>` support these input formats:

| Input | Matches |
|---|---|
| `2501.12948` | Exact arXiv ID |
| `2501` | Partial prefix match |
| `2501_12948` | Underscore-separated |
| `2501.12948v2` | Versioned ID |
| `https://arxiv.org/abs/2501.12948` | Full abs URL |
| `https://arxiv.org/pdf/2501.12948.pdf` | PDF URL with extension |
| `arxiv.org/abs/2501.12948v3` | Versioned URL |
| `www.arxiv.org/abs/2501.12948` | www-prefixed URL |
| `  https://arxiv.org/abs/2501.12948/  ` | Whitespace + trailing slash |

---

## Workspace Layout

Each paper is stored as a folder under the workspace root:

```
workspace/
├── 2501_12948_DeepSeek-R1_Incentivizing_Reasoning/
│   ├── body.tex              # Main content (LaTeX, comments stripped)
│   ├── appendix.tex          # Appendix content (if any)
│   ├── note.txt              # User notes
│   ├── description.md        # AI-generated description
│   ├── 2501.12948.pdf        # Downloaded PDF
│   ├── .description_ready     # Flag file marking completion
│   └── arxiv_chats/          # Saved chat sessions
├── 1706_03762_Attention_Is_All_You_Need/
├── 2510_25741_Scaling_Latent_Reasoning/
└── arxivcat_global_chats/    # Global chat sessions
```

---

## Configuration

Config file: `%APPDATA%\ArxivCat\config.json`

```json
{
  "deepseek_api_key": "sk-...",
  "chat_model": "Flash",
  "workspace_path": "F:\\zrs\\paper"
}
```

The API key can also be set via the `DEEPSEEK_API_KEY` environment variable (takes precedence over config file).

---

## Build

```powershell
# Release build
cargo build --release --bin arxivcat

# Binary location
.\target\release\arxivcat.exe [OPTIONS] <COMMAND>
```

Requires: Rust toolchain with `stable` channel, a Windows/macOS/Linux system.

---

## JSON Mode

Append `--json` anywhere in the command to get structured output. Supported commands:

| Command | JSON content |
|---|---|
| `paper list` | Array of paper objects |
| `paper download` | Download result with file sizes |
| `paper download-all` | Batch status + counts |
| `paper preview` | Paper metadata + content |
| `paper info` | Full paper object with file sizes |
| `paper describe` | `{arxiv_id, description_ready}` |
| `paper remove` | `{removed, folder}` |
| `paper redownload` | `{redownloaded, folder}` |
| `token status` | Token configured, masked, valid, response time |

---

## Exit Codes & Error Contract

Exit codes are **frozen** (changed only in a major breaking release). Agents
should branch on these.

| exit | category | meaning |
|---|---|---|
| 0 | success | command completed |
| 1 | other | unclassified/internal error |
| 2 | usage | clap parse error, unknown command, `--json` on a command without a JSON contract, unparseable arXiv ID |
| 3 | network | HTTP/network failure (retryable) |
| 4 | config | missing API key, broken config, workspace not configured |
| 5 | data | parse/extraction/not-found/json errors, ambiguous paper query |
| 6 | io | local filesystem/permission errors |
| 7 | chat | DeepSeek upstream error (401/403: do not retry) |
| 8 | partial | download-all partially succeeded (some papers failed) |
| 130 | signal | interrupted by Ctrl-C |

**Error envelope** (`--json` mode): stdout is ALWAYS exactly one JSON document.
On failure it is:

```json
{"error": {"code": 3, "kind": "http", "message": "...", "retryable": true}}
```

`kind` ∈ io | http | parse | extraction | chat | config | not_found | json | other | usage.
`retryable` is true for http and for chat except 401/403.

**Stream discipline**: payload goes to stdout; progress/diagnostics go to
stderr (`\r` progress is TTY-gated); human-readable errors go to stderr.

## See Also

- [final-plan-v2.md](./final-plan-v2.md) — CLI projectization plan (expert-reviewed, 2026-08)
