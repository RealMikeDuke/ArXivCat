# GUI Revival Manual (legacy-gui → main merge)

Status: **reference** — only needed if the GUI is ever revived.
The GUI lives in full on the `legacy-gui` branch (tag `v0.9.1-gui`); main is
the pure-CLI project. Reviving the GUI is a normal branch merge + a bounded
core-API adaptation — NOT a rewrite and NOT a cross-repo reconciliation.

## 1. Merge

```bash
git checkout main
git merge legacy-gui        # or: git merge v0.9.1-gui
```

Text conflicts are expected in exactly three places:
- root `Cargo.toml` — `members` must re-add `src-tauri` (and its deps)
- `Cargo.lock`
- `README.md` (GUI-era README vs CLI-only README)

## 2. Core-API adaptation checklist (bounded)

The CLI refactor changed the core public API. GUI callers (mostly
`src-tauri/src/commands.rs`, ~30 commands mirroring core) must adapt:

| core change (v0.10+) | GUI adaptation |
|---|---|
| `Workspace::open` is read-only | GUI code that relied on it creating `arxivcat_global_chats` must create dirs explicitly (chat save does this) |
| `HttpConfig` injection | Every network call (`download_source`, `download_pdf`, `fetch_title_from_arxiv`, `build_description`, `stream_chat`) now takes `&HttpConfig`; construct once and thread through |
| `is_complete = has_body` | GUI pending/complete badges keyed on `has_body`; description status is informational (`description_ready`) |
| `build_description` removed from pipeline | GUI download flow no longer triggers AI; explicit `describe` action only |
| `ChatSession` fields removed | `locked_fields`, `context_snapshot`, `view_name` no longer exist. For old session JSON: re-add with `#[serde(default)]` or drop — new deserializer ignores unknown keys |
| `compute_selection_delta` removed | Replace with a local diff or drop the feature |
| `ErrorLevel`/`level()` removed | Map `ArxivError` variants to toast/notice by variant (see exit-code table in docs/cli.md) |
| Paper folder names are ID-only | GUI path parsing must read `paper.json` manifest (single source of truth); legacy `{id}_{title}` parsing is read-only fallback |
| Manifest is authoritative | Write-path commands must refresh `paper.json` (download/scan/describe) |

## 3. Six-line serde patch (if old session JSON must keep GUI fields)

Re-add the three GUI fields to `ChatSession` with defaults:

```rust
#[serde(default)]
pub locked_fields: HashMap<String, Vec<String>>,
#[serde(default)]
pub context_snapshot: String,
#[serde(default)]
pub view_name: String,
```

New `ChatSession` already ignores unknown JSON keys, so the GUI fields are
compatible in both directions.

## 4. Post-merge verification

- `cargo build --workspace` (all members incl. src-tauri)
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- Launch Tauri dev, run one download + one chat session against an existing
  v0.11 workspace (exercises manifest + HttpConfig + session compat).

## 5. Rollback

If the GUI is not actually coming back: `git merge --abort` (or reset) —
main remains the pure-CLI project. The branch/tag are permanent archives.
