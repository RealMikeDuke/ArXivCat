"""CLI implementation of UIProtocol. Prints to stdout with ANSI colors."""
from __future__ import annotations

import shutil
import sys
from pathlib import Path
from typing import Any

from arxivcat.ui.base import UIProtocol


# ── ANSI colors (works on Windows Terminal, modern consoles) ───

class C:
    RED = '\033[91m'
    GREEN = '\033[92m'
    YELLOW = '\033[93m'
    BLUE = '\033[94m'
    GRAY = '\033[90m'
    BOLD = '\033[1m'
    R = '\033[0m'


def _terminal_width() -> int:
    try:
        return shutil.get_terminal_size().columns
    except Exception:
        return 80


def _print_table(headers: list[str], rows: list[list[str]], max_col_widths: list[int] | None = None) -> None:
    """Print a simple aligned table."""
    if not rows:
        print(f"{C.GRAY}(empty){C.R}")
        return
    tw = _terminal_width()
    ncols = len(headers)
    if max_col_widths is None:
        max_col_widths = [0] * ncols
    col_widths = [len(h) for h in headers]
    for row in rows:
        for i, cell in enumerate(row):
            col_widths[i] = max(col_widths[i], len(cell))
    for i in range(ncols):
        if max_col_widths[i] > 0:
            col_widths[i] = min(col_widths[i], max_col_widths[i])
    total = sum(col_widths) + 3 * (ncols - 1)
    if total > tw:
        overflow = total - tw
        for i in range(ncols - 1, -1, -1):
            if col_widths[i] > 10:
                cut = min(overflow, col_widths[i] - 10)
                col_widths[i] -= cut
                overflow -= cut
            if overflow <= 0:
                break

    def _fmt(row: list[str]) -> str:
        parts = []
        for i, cell in enumerate(row):
            if len(cell) > col_widths[i]:
                cell = cell[:col_widths[i] - 1] + "…"
            parts.append(cell.ljust(col_widths[i]))
        return " │ ".join(parts)

    sep = "─┼─".join("─" * w for w in col_widths)
    print(f"{C.BOLD}{_fmt(headers)}{C.R}")
    print(f"{C.GRAY}{sep}{C.R}")
    for row in rows:
        print(_fmt(row))


class CliUI:
    """CLI implementation of UIProtocol.

    All output goes to stdout. Thread-safe for background operations
    (Presenter calls UI methods from worker threads).
    """

    def __init__(self, url: str = "", view: str = "body"):
        self._url = url
        self._view = view
        self._papers: list[dict] = []
        self._quiet = False  # suppress non-essential output

    # ── UIProtocol methods ────────────────────────────────────

    def add_log(self, msg: str) -> None:
        if self._quiet:
            return
        if msg.startswith("[OK]"):
            print(f"{C.GREEN}{msg}{C.R}", flush=True)
        elif msg.startswith("[ERROR]") or msg.startswith("[WARN]"):
            print(f"{C.RED}{msg}{C.R}", flush=True)
        elif msg.startswith("[INFO]"):
            print(f"{C.BLUE}{msg}{C.R}", flush=True)
        else:
            print(msg, flush=True)

    def set_mini_status(self, msg: str, level: str = "info") -> None:
        if self._quiet:
            return
        if level == "error":
            print(f"{C.RED}[{msg}]{C.R}", flush=True)
        elif level == "ok":
            print(f"{C.GREEN}[{msg}]{C.R}", flush=True)

    def set_preview(self, content: str, label: str) -> None:
        if self._quiet:
            return
        tw = _terminal_width()
        rule = "─" * min(tw, 60)
        print(f"\n{C.BOLD}{rule}{C.R}")
        print(f"{C.BOLD}  {label}{C.R}")
        print(f"{C.BOLD}{rule}{C.R}\n")
        print(content)
        print(f"\n{C.GRAY}{rule}{C.R}")

    def set_buttons_enabled(self, enabled: bool) -> None:
        pass

    def set_run_busy(self, busy: bool) -> None:
        if not self._quiet and busy:
            print(f"{C.GRAY}Working...{C.R}", flush=True)

    def set_paper_actions_busy(self, busy: bool) -> None:
        pass

    def show_toast(self, msg: str, duration_ms: int = 2000) -> None:
        if not self._quiet:
            print(f"{C.GRAY}{msg}{C.R}", flush=True)

    def get_url_input(self) -> str:
        return self._url

    def get_view_mode(self) -> str:
        return self._view

    def get_preview_text(self) -> str:
        return ""

    def clear_log(self) -> None:
        pass

    def set_url_input(self, url: str) -> None:
        self._url = url

    def set_paper_list(self, papers: list[dict]) -> None:
        self._papers = papers

    def set_title(self, title: str) -> None:
        pass

    def set_download_all_state(self, interrupt_mode: bool) -> None:
        pass

    def run(self) -> None:
        pass

    # ── CLI-specific helpers ──────────────────────────────────

    def print_paper_list(self) -> None:
        """Print the cached paper list as a formatted table."""
        if not self._papers:
            print(f"{C.GRAY}No papers in workspace.{C.R}")
            return
        rows = []
        for i, p in enumerate(self._papers, 1):
            status = ""
            if p.get("pending"):
                status = "P"
            elif not p.get("complete"):
                status = "."
            else:
                status = "C"
            rows.append([str(i), status, p["arxiv_id"], p["title"]])
        _print_table(
            ["#", "", "arXiv ID", "Title"],
            rows,
            max_col_widths=[0, 0, 14, 0],
        )

    def print_paper_info(self, paper: dict) -> None:
        """Print detailed info for a single paper."""
        print(f"{C.BOLD}arXiv ID:{C.R} {paper['arxiv_id']}")
        print(f"{C.BOLD}Title:{C.R}   {paper['title']}")
        print(f"{C.BOLD}Folder:{C.R}  {paper['folder_name']}")
        status = "complete" if paper.get("complete") else ("pending" if paper.get("pending") else "incomplete")
        color = C.GREEN if paper.get("complete") else (C.YELLOW if paper.get("pending") else C.GRAY)
        print(f"{C.BOLD}Status:{C.R}  {color}{status}{C.R}")

    def set_quiet(self, quiet: bool) -> None:
        self._quiet = quiet

    def set_view(self, view: str) -> None:
        self._view = view
