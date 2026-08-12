# Changelog — arxivcat-cli

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
