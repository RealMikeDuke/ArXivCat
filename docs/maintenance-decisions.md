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
  then cli**. The cli dependency pins `arxivcat-core = { version = "0.11.0" }`.
- Both manifests now carry `license = "MIT"` and `repository`.

Actual publishing needs a crates.io token and is a user decision.
