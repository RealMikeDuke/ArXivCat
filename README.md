# ArXivCat

Command-line tool for downloading, extracting, and managing arXiv papers.

Input an arXiv ID or URL → download the LaTeX source → expand `\input`/`\include` →
export clean `body.tex` / `appendix.tex` into a workspace folder. Built for
scripts and AI agents: machine-readable `--json` output, stable exit codes,
no GUI.

## Install

```bash
cargo build --release --bin arxivcat
# binary: target/release/arxivcat
```

Requires: Rust toolchain (stable). Linux is the primary tested platform
(CI); macOS/Windows are best-effort, not CI-covered.

## Quick Start

```bash
# Set your workspace
arxivcat workspace open ~/paper

# Download a paper (URL or raw ID)
arxivcat paper download https://arxiv.org/abs/2501.12948

# List papers in workspace
arxivcat paper list

# Preview extracted body
arxivcat paper preview 2501.12948 -v body

# Machine-readable output (supported by list/download/preview/info/token status)
arxivcat paper list --json
```

## Commands

- `workspace open|scan` — manage the workspace folder
- `paper list|download|download-all [--jobs N] [--force]|preview|note|strip|info|describe|deep-summarize|open|pdf|remove|redownload` — manage papers
- `paper tag list|add|remove|set|clear` — classify papers (tag = symlink dir into `raw/`)
- `workspace export <out.tar.gz>` / `workspace import <in.tar.gz>` — move a library between machines
- `token status|set|validate` — manage the DeepSeek API key (optional, for AI features)

## Documentation

- [docs/meta.md](docs/meta.md) — project & documentation navigation (start here)
- [docs/architecture.md](docs/architecture.md) — technical architecture for internal developers: process model, flock locking, AI pipeline, code map, pitfalls
- [docs/cli.md](docs/cli.md) — full CLI manual: commands, JSON schemas, **exit-code contract**, ID matching
- [docs/maintenance-decisions.md](docs/maintenance-decisions.md) — maintainer decisions (error kinds, publish order)
- [docs/gui-revival.md](docs/gui-revival.md) — GUI revival manual (legacy-gui branch)

## License

MIT
