# Maintenance Decisions (P2.4/P2.5)

## repair_permissions — retire assessment (P2.4)

**Status: keep, narrowed. Retire condition: none foreseen before a workspace
migration tool exists.**

`force_uniform_permissions` (files 0644 / dirs 0755, skips symlinks) serves
two paths:
1. right after tar extraction (prevents arXiv tarballs carrying 000/0400
   modes from breaking later reads), and
2. the legacy-cache validate-failure path in `download_source` (fixes
   read-only trees from earlier versions).

Retiring it would require proof that no historical workspace has restrictive
modes AND that arXiv never ships restrictive modes. Neither holds today.
Cost of keeping: ~30 lines + one glob walk per extraction. Recommendation:
keep; revisit only if a future `workspace migrate` rewrites all trees.

## CLI error severity / `kind` enumeration (P2.5)

The frozen exit-code table in [docs/cli.md](./cli.md#exit-codes--error-contract)
is the single severity source for agents. The error envelope `kind` field maps
to `ArxivError` variants:

| kind | source |
|---|---|
| `io` | `ArxivError::Io` |
| `http` | `ArxivError::Http` |
| `parse` | `ArxivError::Parse` |
| `extraction` | `ArxivError::Extraction` |
| `chat` | `ArxivError::Chat` |
| `config` | `ArxivError::Config` |
| `not_found` | `ArxivError::NotFound` |
| `json` | `ArxivError::Json` |
| `other` | `ArxivError::Other` |
| `usage` | clap/parse/validation errors (exit 2) |
| `ambiguous` | paper lookup with multiple matches (exit 5) |

`retryable` semantics: `http` → true; `chat` → false when the message contains
401/403 (retrying burns money); everything else → false. Agents should retry
only `retryable: true` envelopes, with the documented backoff (3 attempts,
500ms × 2^n, Retry-After ≤ 30s).

The GUI-era `ErrorLevel` (Silent/Toast/Notice/Blocking) was removed in v0.10.0
and must not be reintroduced — `kind` + exit code supersede it.

## crates.io publish order (P2.6 dry-run findings)

`cargo package` was rehearsed:
- `arxivcat-core` packages cleanly (21 files, verifies and compiles).
- `arxivcat-cli` package **requires `arxivcat-core` to exist on crates.io**
  (cargo resolves the versioned path dependency against the index even with
  `--no-verify`). This is expected and matches the plan: **publish core first,
  then cli**. The cli dependency pins `arxivcat-core = { version = "0.11.1" }`.
- Both manifests now carry `license = "MIT"` and `repository`.

Actual publishing needs a crates.io token and is a user decision.

## Known issues (jury-decide 2026-08-14 — deferred / accepted)

- P2-3: cross-process lock contention on the SAME paper marks it failed and
  arms the 24h cooldown; `--force` bypasses. Accepted for 0.11.x; revisit
  the failure-accounting semantics in a future release.
- P2-4: Windows/macOS are best-effort, not CI-covered (README updated to say
  so). Full CI matrix deferred.
- P3-2 (GHOST, verified non-issue): `{:<20}` pads the arxiv_id column, NOT
  the title — titles were never truncated. No action needed; recorded so it
  is not re-reported.
- P3-3: `--jobs` is validated 1..=8 by clap AND clamped again in
  `cmd_download_all` — redundant defense, kept intentionally (restored
  after jury-review flagged the removal as contradicting the adjudication).
- P3-4: `token set` echoes the key in plain text (storage is 0600 + masked
  everywhere else). Deferred; `DEEPSEEK_API_KEY` env var is the recommended
  path for sensitive environments.

## Deep/brief lock protocol — kernel flock (jury-burst 2026-08-16)

Five content-based lock protocol iterations (O_EXCL + PID liveness + stale
reclaim + read-back ownership) reached a correctness ceiling: µs-level
reclaim races, ghost locks on crash-in-init, PID-reuse false liveness. A
3:0 jury verdict moved deep/brief generation locks to kernel `flock`
(LOCK_EX|LOCK_NB) on PERMANENT lock files (never unlinked — unlinking
breaks mutual exclusion; redownload's directory wipe recreates the inode,
which is safe). The kernel releases the lock on exit/crash/kill — no
liveness probing, no stale reclaim, no ghost locks.

Consequences recorded here so they are not re-litigated:
- `.deep.lock` / `.brief.lock` are permanent zero-byte files per paper
  (directory clutter by design).
- Worker self-locks AFTER spawn; a concurrent spawner's worker fails
  acquire and exits 0 (single payment).
- Automatic generation is best-effort **at-least-once**: a failed request
  may already be billed server-side, so batch retries can rarely repeat
  round-1 cost. Documented in docs/cli.md.
- Non-Unix platforms keep the content-based fallback (no flock); its
  liveness probing is best-effort and crash leftovers may need manual
  cleanup. Linux is the tested primary.

