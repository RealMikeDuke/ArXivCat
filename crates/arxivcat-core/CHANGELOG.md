# Changelog — arxivcat-core

## [0.11.11] — 2026-08-14

- P2-3 (jury-ask A): `DownloadLock::acquire` now WAITS up to 30s (poll
  500ms, re-checking stale every round) for a busy cross-process lock
  instead of failing on the first collision — a transient "another process
  is downloading this paper" no longer arms a 24h cooldown. After
  acquiring, the cache is re-checked so a paper the winner just finished is
  not re-downloaded. Regression test: busy lock released after 1s → success.
- P2-5 read paths: `load_workspace_path` / `load_cached_token` /
  `load_model_preference` now back up a corrupt config.json (warn +
  `.corrupt-<ts>`) instead of silently swallowing it. Regression test added.

## [0.11.10] — 2026-08-14

- Test-fidelity fix: the versioned-title wiremock test now feeds
  `2501.12948v2` into `fetch_titles_batch` (guards the actual CLI lookup
  path instead of reimplementing it). Changelog corrected to name the
  owning crate.

## [0.11.9] — 2026-08-14

- Wiremock test now feeds a versioned id (`2501.12948v2`) into
  `fetch_titles_batch` and asserts the normalized base-key hit, guarding the
  CLI consumer lookup path. (The CLI-side lookup fix lives in arxivcat-cli.)

## [0.11.8] — 2026-08-14

- Jury-review fixes on v0.11.7:
  - P2-1 corrected: export-API title keys normalized to base id (versioned
    `<id>2501.12948v2</id>` now resolves under bare `2501.12948` — the
    v0.11.7 title backfill was a silent regression for the most common
    input). Regression test added.
  - P3-3 clamp restored (removal contradicted the adjudication record);
    recorded as known-issue.
  - Cargo.lock synced (was dirtied by build).

## [0.11.7] — 2026-08-14

- Jury-decide round (P2/P3 adjudication): title backfill now uses the
  export API (no extra abs-page request / 429 exposure); batch title
  sleep is conditional (no 3s tail wait); corrupted config.json is backed
  up as .corrupt-<ts> before overwrite (no silent data loss); API docs
  pinned for `find_paper_by_id`; dead clamp removed.
- Accepted known-issues (P2-3 lock-contention cooldown, P2-4 non-Linux
  best-effort, P3-4 token echo) recorded in docs/maintenance-decisions.md.

## [0.11.6] — 2026-08-12

- tex.rs slice panic closed (malformed papers can no longer exit 101
  outside the frozen exit-code contract).
- Duplicate legacy + canonical folders no longer deadlock every command —
  exact base-id query prefers the canonical folder.
- `redownload` hits the network again (cache bypass) and refreshes the
  manifest after metadata restore (description_ready stays fresh).
- `describe` context truncated (120k chars/file, same policy as chat).
- Dead code removed (generate_title, ChatMetrics); `/thinking off` now
  explicitly disables thinking in the API request.

## [0.11.5] — 2026-08-12

- PDF filename normalized to base-id ({base_id}.pdf) for versioned inputs —
  manifest `files.pdf` and disk now always agree.
- `paper preview --json` missing file -> exit 5 / kind not_found (was
  exit 0 + embedded error; contract violation).
- Chat REPL: assistant replies are appended to history (multi-turn
  continuity actually works now); callbacks are FnMut.
- `redownload` aborts (exit 6) instead of deleting the folder when the
  metadata backup fails (cross-device rename protection).

## [0.11.4] — 2026-08-12

- Refactor: chat REPL deduplicated (~400 duplicated lines → shared
  `run_repl` parameterized by context builder/session metadata/lock rule);
  side and global loops no longer drift apart.

## [0.11.3] — 2026-08-12

- Zero-preset review round: ghost-paper fix (non-digit folders are never
  parsed as papers), HTTP total timeout (120s), \input expansion depth
  limit (64), chat context truncation (120k chars/file), downloaded_at
  now populated, single-download title backfill, redownload backup
  timestamped + surfaced, token mask char-safe, note-io kind fix,
  editor fallback (vi on unix / notepad on windows).

## [0.11.2] — 2026-08-12

- Expert-review round: 429/5xx retry-exhaustion now surfaces as exit 3 /
  kind http / retryable true (was exit 1/other); 404 source -> exit 5.
- `mark_failure` no longer writes empty-ID manifests for legacy folders.
- `--json` rejected (exit 2) on non-JSON commands; `--jobs` validated 1..=8.
- redownload preserves `.description_ready`; manifest pdf key fixed;
  dead title request removed; tokio narrowed to `time`.
- New contract tests (wiremock end-to-end): exit 2/3/5/8 locked.

## [0.11.1] — 2026-08-12

- Publish metadata: `license = "MIT"`, `repository` (packaging rehearsal clean).
- `cargo fmt --all` applied (no behavior change).

## [0.11.0] — 2026-08-12

### Added
- `manifest` module: `paper.json` single source of truth (schema, arxiv_id w/
  version, base_id, title, downloaded_at, files inventory, description_ready,
  last_error, cooldown_until_ms) with atomic save; `strip_version`,
  `scan_manifest`, `refresh_manifest`, `mark_failure`, `clear_cooldown`,
  `in_cooldown`.
- `net::HttpConfig`: shared reqwest client + custom UA + retry/backoff
  (`get_with_retry`: 429/5xx/timeout retried, Retry-After respected, cap 30s);
  base URLs env-overridable (`ARXIVCAT_ARXIV_BASE_URL`,
  `ARXIVCAT_DEEPSEEK_BASE_URL`).
- `extract::arxiv::fetch_titles_batch` (export API Atom, 50/chunk, 3s rate
  limit) + `parse_atom_entries`.
- Cross-process download lock (`.locks/{base_id}.lock`, RAII) and unique tar
  temp paths (P1.7/P1.8).

### Changed
- `Workspace::open` is read-only (never creates directories).
- `Paper::from_folder` reads `paper.json` first; legacy folder-name parsing is
  read-only fallback.
- Canonical paper folder name is the version-stripped base ID (no title);
  legacy `{id}_{title}` dirs accepted read-only.
- `download_source` is cache-first (ID-only dir hits with zero network),
  title fetch is best-effort; failure never blocks, fallback folder is
  ID-only (no `unknown`).
- `find_main_tex` unified/recursive, requires `\documentclass`.
- `extract_body_from_dir` warns on unresolved `\input`/`\subfile` (marked in
  body.tex) instead of failing; non-UTF8 reads are lossy.
- `force_uniform_permissions` (files 0644 / dirs 0755, skips symlinks) applied
  after tar extraction; symlink/hardlink tar members rejected.
- `is_complete = has_body` (AI decoupled).
- `sanitize_filename` truncates on char boundaries (no multibyte panic).
- Tokio narrowed to `time` for the library.
- Removed GUI-era residue: `ErrorLevel`, `compute_selection_delta`,
  `ChatSession::{locked_fields, context_snapshot, view_name}`,
  `fresh_folder_name` (`_freshN`).

## [0.9.1] — 2026-07-29
- Bug fixes, URL robustness, test coverage (legacy record).

## [0.9.0] — 2026-07
- CLI migration milestone (legacy record).
