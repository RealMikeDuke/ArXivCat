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
    find_main_tex,
)

VERSION = "v0.5.0"
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


def save_model_preference(model: str) -> None:
    """Save model preference to cache."""
    token_path = get_token_path()
    token_path.parent.mkdir(parents=True, exist_ok=True)
    
    # Load existing config if exists
    config = {}
    if token_path.exists():
        try:
            with open(token_path, "r") as f:
                config = json.load(f)
        except Exception:
            pass
    
    config["chat_model"] = model
    with open(token_path, "w") as f:
        json.dump(config, f)


def load_model_preference() -> str:
    """Load model preference from cache."""
    token_path = get_token_path()
    if token_path.exists():
        try:
            with open(token_path, "r") as f:
                config = json.load(f)
                return config.get("chat_model", "Flash")
        except Exception:
            pass
    return "Flash"


class Presenter:
    def __init__(self, ui: "UIProtocol"):
        self.ui = ui
        self.output_dir: Path | None = None
        self._init_cache()

    # ── init ──────────────────────────────────────────────────

    def _init_cache(self):
        """Initialize cache directories. No size limit (unlimited cache)."""
        base = get_cache_dir()
        downloads = base / "downloads"
        downloads.mkdir(parents=True, exist_ok=True)

    # ── paper list ────────────────────────────────────────────

    def get_paper_list(self) -> list[dict]:
        """Get list of all downloaded papers, deduplicated by arxiv_id.
        For duplicates, prefer a folder whose main .tex is readable."""
        base = get_cache_dir()
        downloads = base / "downloads"
        seen: dict[str, dict] = {}  # arxiv_id -> paper info

        if not downloads.exists():
            return []

        for folder in sorted(downloads.iterdir(), key=lambda f: f.name):
            if not folder.is_dir():
                continue
            name = folder.name
            parts = name.split('_')
            if len(parts) < 2:
                continue
            arxiv_id = f"{parts[0]}.{parts[1]}"
            title = ' '.join(parts[2:])
            title = re.sub(r'\s*fresh\d+$', '', title).strip()

            has_main = find_main_tex(folder) is not None
            prev = seen.get(arxiv_id)

            # Keep this folder if: first seen, or previous was broken and this one works
            if prev is None or (not prev["_ok"] and has_main):
                seen[arxiv_id] = {
                    "arxiv_id": arxiv_id,
                    "title": title,
                    "folder_name": name,
                    "_ok": has_main,
                }

        # Strip internal field and sort newest first
        for v in seen.values():
            v.pop("_ok", None)
        return sorted(seen.values(), key=lambda x: x["arxiv_id"], reverse=True)

    def load_paper(self, folder_name: str):
        """Load a previously downloaded paper."""
        base = get_cache_dir()
        downloads = base / "downloads"
        outputs_dir = base / "outputs"
        paper_dir = downloads / folder_name

        if not paper_dir.exists():
            self.ui.add_log(f"[ERROR] Paper folder not found: {folder_name}")
            return

        parts = folder_name.split('_')
        arxiv_id = f"{parts[0]}.{parts[1]}" if len(parts) >= 2 else folder_name
        self.ui.add_log(f"[INFO] Loading cached paper: {arxiv_id}")

        self.ui.set_url_input(arxiv_id)

        result = extract_body_from_dir(paper_dir, outputs_dir, folder_name, log=self._emit_log)
        if result:
            self.output_dir = outputs_dir / folder_name
            self._load_preview()
            self.ui.set_mini_status("loaded", "ok")
            self.ui.set_buttons_enabled(True)
        else:
            self.output_dir = None
            self.ui.set_preview(
                f"[ERROR] Failed to load paper {arxiv_id}\n\n"
                f"The cached source may be corrupted or have permission issues.\n"
                f"Try clicking Run to re-download.",
                "error"
            )
            self.ui.set_buttons_enabled(False)
            self.ui.set_mini_status("load error", "error")

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
                # Refresh paper list after successful download
                papers = self.get_paper_list()
                self.ui.set_paper_list(papers)

        if self.output_dir:
            self._load_preview()
            self.ui.set_mini_status("done", "ok")
        self._done()

    def _done(self):
        self.ui.set_run_busy(False)
        if self.output_dir:
            self.ui.set_buttons_enabled(True)
