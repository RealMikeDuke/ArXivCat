# Changelog — arxivcat-cli

## [0.11.13] — 2026-08-17

- `paper tag list` / `paper tag add <id> <tag>` / `paper tag remove <id> <tag>`:
  tag = directory at the workspace root symlinking into `raw/`; new tag dirs
  auto-created; multi-tag supported; tag names validated. JSON envelope for
  add/remove: `{tag, arxiv_id, link}` / `{tag, arxiv_id, removed}`.
- `paper download` now places papers under `{workspace}/raw/{id}` (was
  `{workspace}/{id}`); the raw dir is created on demand. Legacy root papers
  remain readable.
## [Unreleased]

- `paper tag list` / `paper tag add <id> <tag>` / `paper tag remove <id> <tag>`:
  tag = directory at the workspace root symlinking into `raw/`; new tag dirs
  auto-created; multi-tag supported; tag names validated. JSON envelope for
  add/remove: `{tag, arxiv_id, link}` / `{tag, arxiv_id, removed}`.
- `paper download` now places papers under `{workspace}/raw/{id}` (was
  `{workspace}/{id}`); the raw dir is created on demand. Legacy root papers
  remain readable.

## [0.11.12] — 2026-08-16

- `paper download` / `download-all` generate the brief automatically after
  extraction (best-effort, opt out with `--no-describe`).
- `paper deep-summarize <id> [--force]` generates the deep recap in the
  foreground; busy contract: another worker holds the lock → `status:busy`
  JSON + exit 7 (refuses, never double-charges; `--force` cleanup happens
  only after the lock and brief gates pass).
- `download` / `download-all` gained `--no-deep` (deep recap default ON).
  `--no-describe` now also gates deep: with no brief, deep is skipped
  entirely (zero LLM calls — generate_deep's internal rebuild is unlocked).
- `download-all` is a PROCESS SCHEDULER: one independent `internal
  download-worker` process per pending paper (from the first download step),
  `--jobs` concurrent, line-delimited JSON events on each worker's stdout
  pipe (downloading/downloaded/brief_done/deep_spawned/done/failed), live
  progress, aggregated success/failures/skipped, exit 0/8/1/130. Ctrl-C
  kills the download workers; detached deep-workers (own process group)
  survive.
- Deep/brief concurrency: kernel `flock` on permanent `.deep.lock` /
  `.brief.lock` (auto-released on crash/kill; no stale reclaim needed).
  Worker self-locks; every entry re-checks readiness under the lock.
  Automatic generation is best-effort at-least-once (a failed request may
  already be billed server-side; batch retries can rarely repeat round-1
  cost).
- `internal deep-worker` / `internal download-worker` hidden commands
  (detached worker infrastructure).

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

- Jury-burst round 2 fix: `cmd_download` title lookup now strips the version
  before `.get()` (map keys are base ids) — versioned inputs like
  `2501.12948v2` no longer silently miss. Versioned-direction assertion
  added to the wiremock test.

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

- Publish metadata: `license = "MIT"`, `repository`; core dep pinned
  `0.11.1` (crates.io publish order: core first).
- `cargo fmt --all` applied (no behavior change).

## [0.11.0] — 2026-08-12

### Added
- `paper describe <id>` — explicit AI-description command (only entry point).
- `paper remove <id>` / `paper redownload <id>` (metadata preserved).
- `paper download-all --jobs N` (default 4, clamp 1-8) + `--force`.
- `token status --json` (configured/masked/response_time_ms/valid).

### Changed
- **Exit-code contract frozen**: 0 ok / 1 other / 2 usage / 3 network /
  4 config / 5 data / 6 io / 7 chat / 8 partial / 130 SIGINT.
- `--json` stdout is always exactly one JSON document; on failure it is the
  error envelope `{"error":{code,kind,message,retryable}}`; human text and
  progress go to stderr.
- `Cli::try_parse`: usage errors exit 2; help/version exit 0.
- All error exits route through `die`/`die_err`; `paper not found` is exit 5,
  missing workspace exit 4, chat rejects `--json` (exit 2).
- `download-all` aggregation payload `{status,total,success,failed,skipped,
  failures[]}`; real Ctrl-C exits 130; 24h per-paper cooldown + `--force`.
- Ambiguity-safe paper lookup (multiple prefix matches → explicit error).
- `paper list` shows `[C]`/`[.]` + description column (AI decoupled).

## [0.9.1] — 2026-07-29
- Bug fixes, URL robustness, CLI docs (legacy record).

## [0.9.0] — 2026-07
- CLI milestone (legacy record).
