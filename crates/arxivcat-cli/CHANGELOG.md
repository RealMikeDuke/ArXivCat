# Changelog — arxivcat-cli

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
