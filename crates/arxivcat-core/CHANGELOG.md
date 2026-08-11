# Changelog — arxivcat-core

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
