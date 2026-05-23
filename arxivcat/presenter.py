"""Presenter: all business logic. Zero dependency on any UI framework."""
from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import threading
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from arxivcat.ui.base import UIProtocol

from arxivcat.core import (
    extract_arxiv_id,
    download_source,
    extract_body_from_dir,
)

VERSION = "v0.4.2"
AUTHOR = "by MikeDuke"


def get_cache_dir() -> Path:
    """Get the cache directory for ArxivCat."""
    return Path(os.environ.get("APPDATA", Path.home())) / "ArxivCat"


def get_token_path() -> Path:
    """Get the path to the token cache file."""
    return get_cache_dir() / "config.json"


def load_cached_token() -> str | None:
    """Load DeepSeek API token from cache."""
    token_path = get_token_path()
    if token_path.exists():
        try:
            with open(token_path, "r") as f:
                config = json.load(f)
                return config.get("deepseek_api_key")
        except Exception:
            return None
    return None


def save_token(token: str) -> None:
    """Save DeepSeek API token to cache."""
    token_path = get_token_path()
    token_path.parent.mkdir(parents=True, exist_ok=True)
    config = {"deepseek_api_key": token}
    with open(token_path, "w") as f:
        json.dump(config, f)


class Presenter:
    def __init__(self, ui: "UIProtocol"):
        self.ui = ui
        self.output_dir: Path | None = None
        self._init_cache()

    # ── init ──────────────────────────────────────────────────

    def _init_cache(self):
        """Clean downloads cache if > 50 MB on startup."""
        base = get_cache_dir()
        downloads = base / "downloads"
        if downloads.exists():
            size = sum(f.stat().st_size for f in downloads.rglob("*") if f.is_file())
            if size > 50 * 1024 * 1024:
                shutil.rmtree(downloads)
                downloads.mkdir(parents=True, exist_ok=True)

    # ── public actions ────────────────────────────────────────

    def run_fetch(self):
        """Called when user clicks Run. Spawns background thread."""
        url = self.ui.get_url_input().strip()
        if not url:
            return
        self.ui.set_run_busy(True)
        self.ui.set_buttons_enabled(False)
        self.ui.clear_log()
        self.ui.set_preview("", "")
        self.ui.set_mini_status("", "info")
        self.output_dir = None
        threading.Thread(target=self._process, args=(url,), daemon=True).start()

    def overwrite_file(self):
        if not self.output_dir:
            return
        view = self.ui.get_view_mode()
        path = self.output_dir / f"{view}.tex"
        path.write_text(self.ui.get_preview_text(), encoding="utf-8")
        self.ui.show_toast(f"Saved {view}.tex")

    def open_folder(self):
        if self.output_dir and self.output_dir.exists():
            subprocess.Popen(f'explorer "{self.output_dir}"')

    def strip_comments(self):
        content = self.ui.get_preview_text()
        stripped = re.sub(r'(?<!\\)%.*', '', content)
        stripped = re.sub(r'\n{3,}', '\n\n', stripped).strip()
        self.ui.set_preview(stripped, self.ui.get_view_mode() + ".tex")
        self.ui.show_toast("Comments stripped")

    def switch_view(self):
        """Called when dropdown changes. Reload preview from disk."""
        self._load_preview()

    # ── internal ──────────────────────────────────────────────

    def _emit_log(self, msg: str):
        self.ui.add_log(msg)
        if "Downloading" in msg and "%" in msg:
            self.ui.set_mini_status("downloading...", "info")
        elif "Download complete" in msg:
            self.ui.set_mini_status("downloaded", "ok")
        elif "Extracting" in msg:
            self.ui.set_mini_status("extracting...", "info")
        elif "Expanding" in msg:
            self.ui.set_mini_status("expanding...", "info")
        elif "Parsing body" in msg:
            self.ui.set_mini_status("parsing...", "info")
        elif "Already cached" in msg:
            self.ui.set_mini_status("cached", "info")
        elif "[OK]" in msg and "saved" in msg:
            self.ui.set_mini_status("done", "ok")

    def _load_preview(self):
        if not self.output_dir:
            return
        view = self.ui.get_view_mode()
        path = self.output_dir / f"{view}.tex"
        content = path.read_text(encoding="utf-8") if path.exists() else "(file not found)"
        self.ui.set_preview(content, f"{view}.tex")

    def _process(self, url: str):
        base = Path(os.environ.get("APPDATA", Path.home())) / "ArxivCat"
        downloads_dir = base / "downloads"
        outputs_dir = base / "outputs"
        downloads_dir.mkdir(parents=True, exist_ok=True)
        outputs_dir.mkdir(parents=True, exist_ok=True)

        arxiv_id = extract_arxiv_id(url)
        if not arxiv_id:
            self.ui.add_log("[ERROR] 无法识别 arXiv ID")
            self.ui.set_mini_status("ID error", "error")
            self._done()
            return

        self.ui.add_log(f"[INFO] 处理论文: {arxiv_id}")

        paper_dir, folder_name = download_source(arxiv_id, downloads_dir, log=self._emit_log)
        if paper_dir:
            result = extract_body_from_dir(paper_dir, outputs_dir, folder_name, log=self._emit_log)
            if result:
                self.output_dir = outputs_dir / folder_name

        if self.output_dir:
            self._load_preview()
            self.ui.set_mini_status("done", "ok")
        self._done()

    def _done(self):
        self.ui.set_run_busy(False)
        if self.output_dir:
            self.ui.set_buttons_enabled(True)
