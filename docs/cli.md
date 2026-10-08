# ArXivCat CLI

ArXivCat is a command-line tool for downloading, extracting, and managing arXiv papers. It downloads LaTeX source code, splits it into readable `body.tex` / `appendix.tex`, generates AI-powered descriptions, and supports interactive chat through selectable OpenAI-compatible API providers.

**Binary**: `arxivcat` (Rust, built via `cargo build --release --bin arxivcat`).

---

## Quick Start

```powershell
# 1. Set your workspace
arxivcat workspace open F:\zrs\paper

# 2. Set the default DeepSeek API key (profile 1, for AI features)
arxivcat token set

# Or configure a custom OpenAI-compatible API (profile 2)
arxivcat token set --profile 2
arxivcat token use 2

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

**Workspace layout** (since 0.11.13): papers live under `{workspace}/raw/`
(the canonical download target); legacy papers at the workspace root are
still read. Tag directories (see `paper tag`) are directories at the
workspace root holding symlinks into `raw/` — they are never mistaken for
papers.

#### `download <ID_OR_URL> [--no-describe] [--no-deep]`

After extraction, the brief summary (`brief_summary.md`, round 1) and the
deep recap (`deep_summary.md`, round 2) are generated automatically
(DeepSeek Flash, independent of the interactive chat model preference;
with the custom xfast provider this is `deepseek/deepseek-v4.1-flash`,
while the official DeepSeek provider uses `deepseek-v4-flash`) unless
`--no-describe` / `--no-deep` is given. Generation is
best-effort: missing API key or any failure is warned on stderr and never
affects the download result/exit code. Single downloads wait for both.

**自动生成语义（best-effort at-least-once）**：batch 路径生成失败会重试
（deep-worker 锁下重试）。由于一次失败请求可能已在服务端计费，瞬时故障
时极端情况下 round-1 可能重复计费（罕见）。`--no-describe` 表示绝不生成
brief——此时 deep 只在 brief 已存在时运行。

Full pipeline: parse arXiv ID from raw ID or URL → download source tar.gz → extract body.tex / appendix.tex → download PDF → brief → deep recap.

**ID input**: Accepts raw IDs (`2501.12948`), versioned IDs (`2501.12948v2`), and URLs (`https://arxiv.org/abs/2501.12948`, `arxiv.org/pdf/2501.12948.pdf`, `www.arxiv.org/abs/2501.12948v3`).

**JSON output**:
```json
{"arxiv_id":"2501.12948","folder":"...","body_length":41953,"appendix_length":159056,"description_ready":true,"deep_ready":true}
```

#### `download-all [--jobs N] [--force] [--no-describe] [--no-deep]`

Briefs are generated serially in-process for each successful download.
Deep recaps default ON: each successful download spawns a DETACHED worker
(`arxivcat internal deep-worker <paper_dir>`, own process group, stdio →
`<paper>/.deep.log`) that finishes asynchronously — the batch never waits
for it. `--no-deep` disables. Same best-effort semantics as `download`.


Process every pending paper (missing body.tex) concurrently (`--jobs`, default 4, range 1-8). Papers in the 24h retry cooldown are skipped and reported; `--force` bypasses the cooldown. Ctrl-C stops the batch and exits 130.

Exit codes: 0 all succeeded / 8 partial (some failed) / 1 all failed / 130 interrupted.

**JSON output**:
```json
{"status":"done|partial|failed|cancelled","total":3,"success":2,"failed":1,"skipped":0,"failures":[{"id":"2501.12948","code":3,"kind":"http","message":"...","retryable":true}]}
```


#### `preview <ID_OR_QUERY>`

Print the content of a paper file. Accepts raw ID, partial ID, or full URL.

| `-v` value | File |
|---|---|
| `body` (default) | `body.tex` |
| `appendix` | `appendix.tex` |
| `note` | `note.txt` |
| `description` \| `brief` | `brief_summary.md` |
| `deep` | `deep_summary.md` |

**JSON output**:
```json
{"arxiv_id":"2501.12948","title":"DeepSeek-R1...","view":"body","content":"..."}
```

#### `note <ID_OR_QUERY> [TEXT]`

> Note: `note` accepts free-form trailing text, so a global `--json` must be
> placed BEFORE the subcommand (`arxivcat --json paper note ...`) — after it
> the flag is consumed as note text. `note` has no JSON contract regardless.

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

Interactive AI chat with papers. Requires a token for the active API provider (`arxivcat token set`).

```
arxivcat chat side <ID_OR_QUERY>    # Chat scoped to one paper
arxivcat chat global                # Chat over all workspace papers
```

#### REPL Commands

Both `side` and `global` support these in-chat commands:

| Command | Description |
|---|---|
| `/model Flash\|Pro` | Switch between DeepSeek models (profile 1 only). |
| `/thinking` | Toggle deep reasoning mode (profile 1 only). |
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

Manage the active API provider and its token (stored in `%APPDATA%\ArxivCat\config.json`). Profile 1 is DeepSeek. Profile 2 is a custom OpenAI-compatible API: its base URL, model, and token are requested locally by the CLI and are not embedded in this repository. `DEEPSEEK_API_KEY` remains an environment override for profile 1.

```
arxivcat token list                  # List provider IDs, endpoint, model, and active state
arxivcat token set --profile 2       # Enter a custom base URL, model, and token
arxivcat token use 2                 # Select the saved custom API
arxivcat token set                   # Prompt for the active provider token via stdin
arxivcat token set --profile 1       # Set DeepSeek token without switching providers
arxivcat token status                # Show active provider and masked token, then validate
arxivcat token validate              # Test active token against its /models endpoint
```

| Subcommand | Description |
|---|---|
| `list` | List DeepSeek and the saved custom API. Custom endpoint/model are shown only after local configuration. `--json` is supported. |
| `use <1\|2>` | Persist the active provider. `--json` is supported. |
| `status` | Print active provider, masked token (e.g. `sk-5...7abe`), response time, and validity. JSON adds `profile`, `provider`, and `model` to the existing fields. |
| `set [--profile <1\|2>]` | Profile 1 prompts for a DeepSeek token. Profile 2 prompts for a base URL, model, and token, then saves them atomically. Token input is not echoed. Does not switch provider. |
| `validate` | Test the active token against that provider's `/models` endpoint. Returns response time and validity status. |

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
├── 2501_12948/               # ID-only folder name (P1.2)
│   ├── paper.json           # Manifest: single source of truth
│   ├── body.tex              # Main content (LaTeX, comments stripped)
│   ├── appendix.tex          # Appendix content (if any)
│   ├── note.txt              # User notes
│   ├── description.md        # AI-generated description
│   ├── 2501.12948.pdf        # Downloaded PDF
│   └── arxiv_chats/          # Saved chat sessions
├── 2412_04445/
└── arxivcat_global_chats/    # Global chat sessions
```

---

## Configuration

Config file: `%APPDATA%\ArxivCat\config.json`

```json
{
  "deepseek_api_key": "sk-...",
  "api_profile": 2,
  "custom_api": {
    "base_url": "https://api.example.com/v1",
    "model": "your-model-id",
    "api_key": "sk-..."
  },
  "chat_model": "Flash",
  "workspace_path": "F:\\zrs\\paper"
}
```

`deepseek_api_key` is used by profile 1. `custom_api` stores profile 2's local endpoint, model, and token. The `api_profile` field selects the active provider. `DEEPSEEK_API_KEY` takes precedence over the saved token when profile 1 is active.

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
| `paper tag list` | List all tags (directories of symlinks) |
| `paper tag add <id> <tag>` | Tag a paper (creates the tag dir if new, symlinks into `raw/`) |
| `paper tag remove <id> <tag>` | Untag a paper (removes the symlink) |
| `paper tag set <id> <tag1,tag2>` | Reclassify: set the FULL tag list (removes unlisted tags) |
| `paper tag clear <id>` | Remove all tags from a paper |

Tags are recorded in the paper manifest (`categories`) AND materialized as
symlinks in `{workspace}/{tag}/`; `set`/`clear` remove stale symlinks.

**Export / Import** (move a library between machines):
```
arxivcat workspace export <out.tar.gz>   # papers (raw/ + legacy) + manifest categories
arxivcat workspace import <in.tar.gz>    # copy new papers, rebuild tag symlinks
```
- Export archives every paper under a uniform `raw/{folder}` layout;
  classification travels inside each manifest (`categories`), so no
  separate tag packing is needed.
- Import dedupes by folder name (existing papers are skipped), then rebuilds
  tag directories + relative symlinks (`../raw/{folder}`) from each
  manifest's categories. Idempotent.
- Symlinks are native on Linux/macOS; Windows junction support is future
  work.
| `paper preview` | Paper metadata + content |
| `paper info` | Full paper object with file sizes |
| `paper describe` | `{arxiv_id, description_ready}` |
| `paper deep-summarize` | `{arxiv_id, deep_ready}`；busy 时 `{arxiv_id, status:"busy", message}` + exit 7 |
| `paper remove` | `{removed, folder}` |
| `paper redownload` | `{redownloaded, folder}` |
| `token status` | Token configured, masked, valid, response time |
| `workspace scan` | `{scanned: N}` |

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

`kind` ∈ io | http | parse | extraction | chat | config | not_found | json | other | usage | ambiguous (exit 5).
`retryable` is true for http and for chat except 401/403.

**Stream discipline**: payload goes to stdout; progress/diagnostics go to
stderr (`\r` progress is TTY-gated); human-readable errors go to stderr.

## See Also

- [gui-revival.md](./gui-revival.md) — how to bring back the GUI from legacy-gui
- [maintenance-decisions.md](./maintenance-decisions.md) — repair_permissions assessment, error kinds, publish order
- [final-plan-v2.md](./final-plan-v2.md) — projectization plan archive (executed)
