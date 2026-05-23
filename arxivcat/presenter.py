"""Presenter: all business logic. Zero dependency on any UI framework."""
from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import threading
import webbrowser
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from arxivcat.ui.base import UIProtocol

from arxivcat.core import (
    extract_arxiv_id,
    extract_arxiv_id_from_pdf,
    download_pdf,
    download_source,
    extract_body_from_dir,
    fetch_title_from_arxiv,
    find_main_tex,
    sanitize_filename,
)

VERSION = "v0.6.0"
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


def save_workspace_path(path: str) -> None:
    """Save last workspace path to config."""
    token_path = get_token_path()
    token_path.parent.mkdir(parents=True, exist_ok=True)
    config = {}
    if token_path.exists():
        try:
            with open(token_path, "r") as f:
                config = json.load(f)
        except Exception:
            pass
    config["workspace_path"] = path
    with open(token_path, "w") as f:
        json.dump(config, f)


def load_workspace_path() -> str | None:
    """Load last workspace path from config."""
    token_path = get_token_path()
    if token_path.exists():
        try:
            with open(token_path, "r") as f:
                config = json.load(f)
                return config.get("workspace_path")
        except Exception:
            pass
    return None


class Presenter:
    def __init__(self, ui: "UIProtocol"):
        self.ui = ui
        self.output_dir: Path | None = None
        self.workspace_path: Path | None = None
        self._init_cache()

    # ── init ──────────────────────────────────────────────────

    def _init_cache(self):
        """Initialize download cache directory."""
        base = get_cache_dir()
        downloads = base / "downloads"
        downloads.mkdir(parents=True, exist_ok=True)

    # ── workspace ─────────────────────────────────────────────

    def open_workspace(self, path: str):
        """Set workspace folder and refresh UI."""
        self.workspace_path = Path(path)
        self.workspace_path.mkdir(parents=True, exist_ok=True)
        save_workspace_path(str(self.workspace_path))
        papers = self.get_paper_list()
        self.ui.set_paper_list(papers)
        self.ui.set_title(f"ArxivCat — {self.workspace_path.name}")

    # ── paper list ────────────────────────────────────────────

    def get_paper_list(self) -> list[dict]:
        """Get list of papers in the current workspace folder."""
        if not self.workspace_path or not self.workspace_path.exists():
            return []

        papers = []
        for folder in sorted(self.workspace_path.iterdir(), key=lambda f: f.name):
            if not folder.is_dir() or folder.name.startswith('.'):
                continue

            name = folder.name
            parts = name.split('_')
            if len(parts) < 2:
                continue
            arxiv_id = f"{parts[0]}.{parts[1]}"
            title = ' '.join(parts[2:])
            title = re.sub(r'\s*fresh\d+$', '', title).strip()

            papers.append({
                "arxiv_id": arxiv_id,
                "title": title,
                "folder_name": name,
                "has_body": (folder / "body.tex").exists(),
            })

        return sorted(papers, key=lambda x: x["arxiv_id"], reverse=True)

    def load_paper(self, folder_name: str):
        """Load a paper from the workspace folder."""
        if not self.workspace_path:
            return

        paper_dir = self.workspace_path / folder_name
        if not paper_dir.exists():
            self.ui.add_log(f"[ERROR] Paper folder not found: {folder_name}")
            return

        parts = folder_name.split('_')
        arxiv_id = f"{parts[0]}.{parts[1]}" if len(parts) >= 2 else folder_name
        self.ui.add_log(f"[INFO] Loading paper: {arxiv_id}")
        self.ui.set_url_input(arxiv_id)

        body_path = paper_dir / "body.tex"
        if body_path.exists():
            self.output_dir = paper_dir
            self._load_preview()
            self.ui.set_mini_status("loaded", "ok")
            self.ui.set_buttons_enabled(True)
        else:
            self.output_dir = None
            self.ui.set_preview(
                f"Paper {arxiv_id} is pending download.\n\n"
                f"Click \"Download All\" or paste the ID and click Run.",
                ""
            )
            self.ui.set_buttons_enabled(False)
            self.ui.set_mini_status("pending", "info")

    # ── public actions ────────────────────────────────────────

    def scan_workspace_pdfs(self):
        """Scan workspace PDFs, create empty folders for new papers."""
        if not self.workspace_path:
            return
        self.ui.set_run_busy(True)
        self.ui.set_buttons_enabled(False)
        self.ui.clear_log()
        self.ui.set_mini_status("scanning PDFs...", "info")
        threading.Thread(target=self._scan_pdfs_work, daemon=True).start()

    def download_all_pending(self):
        """Download and extract all papers without body.tex."""
        if not self.workspace_path:
            return
        self.ui.set_run_busy(True)
        self.ui.set_buttons_enabled(False)
        self.ui.clear_log()
        self.ui.set_mini_status("preparing...", "info")
        threading.Thread(target=self._download_all_work, daemon=True).start()

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

    def open_pdf_in_browser(self):
        arxiv_id = extract_arxiv_id(self.ui.get_url_input())
        if arxiv_id:
            webbrowser.open(f"https://arxiv.org/pdf/{arxiv_id}")

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

    def _scan_pdfs_work(self):
        pdfs = list(self.workspace_path.glob("*.pdf"))
        if not pdfs:
            self.ui.add_log("[INFO] No PDF files found in workspace")
            self._done()
            return

        total = len(pdfs)
        self.ui.add_log(f"[INFO] Found {total} PDF files, scanning...")

        # Collect existing base IDs to deduplicate
        existing_base = {
            re.sub(r'v\d+$', '', p["arxiv_id"])
            for p in self.get_paper_list()
        }

        created = 0
        for i, pdf in enumerate(pdfs, 1):
            self.ui.set_mini_status(f"scanning {i}/{total}", "info")
            self.ui.add_log(f"[INFO] [{i}/{total}] {pdf.name}")

            arxiv_id = extract_arxiv_id_from_pdf(str(pdf), log=self._emit_log)
            if not arxiv_id:
                self.ui.add_log(f"[WARN]   no arXiv ID found")
                continue

            base_id = re.sub(r'v\d+$', '', arxiv_id)
            if base_id in existing_base:
                self.ui.add_log(f"[INFO]   {arxiv_id} already in workspace")
                continue

            # Fetch title and create empty folder
            self.ui.add_log(f"[INFO]   {arxiv_id} — fetching title...")
            title = fetch_title_from_arxiv(arxiv_id, log=self._emit_log) or "unknown"
            folder_name = f"{arxiv_id.replace('.', '_')}_{sanitize_filename(title)}"
            folder = self.workspace_path / folder_name
            folder.mkdir(parents=True, exist_ok=True)
            existing_base.add(base_id)
            created += 1
            self.ui.add_log(f"[OK]   created: {folder_name}")

        papers = self.get_paper_list()
        self.ui.set_paper_list(papers)
        self.ui.add_log(f"\n[OK] Scan done. {created} new paper folders created.")
        self.ui.set_mini_status(f"scan done (+{created})", "ok")
        self._done()

    def _download_all_work(self):
        papers = self.get_paper_list()
        pending = [p for p in papers if not p["has_body"]]

        if not pending:
            self.ui.add_log("[INFO] All papers already downloaded")
            self.ui.set_mini_status("nothing to download", "ok")
            self._done()
            return

        total = len(pending)
        self.ui.add_log(f"[INFO] {total} papers to download\n")

        base = get_cache_dir()
        downloads_dir = base / "downloads"
        downloads_dir.mkdir(parents=True, exist_ok=True)
        success = 0

        for i, paper in enumerate(pending, 1):
            arxiv_id = paper["arxiv_id"]
            ws_folder = paper["folder_name"]
            self.ui.set_mini_status(f"{i}/{total}  {arxiv_id}", "info")
            self.ui.add_log(f"[INFO] [{i}/{total}] {arxiv_id}")

            paper_dir, _ = download_source(
                arxiv_id, downloads_dir, log=self._emit_log
            )
            if paper_dir:
                result = extract_body_from_dir(
                    paper_dir, self.workspace_path, ws_folder, log=self._emit_log
                )
                if result:
                    success += 1
                    out_dir = self.workspace_path / ws_folder
                    download_pdf(arxiv_id, out_dir, log=self._emit_log)
                    # Incremental refresh so interrupted runs show progress
                    self.ui.set_paper_list(self.get_paper_list())
            else:
                self.ui.add_log(f"[ERROR]   download failed for {arxiv_id}")
        self.ui.add_log(f"\n[OK] Download complete. {success}/{total} papers ready.")
        self.ui.set_mini_status(f"done {success}/{total}", "ok")
        self._done()

    def _process(self, url: str):
        if not self.workspace_path:
            self.ui.add_log("[ERROR] No workspace folder selected")
            self._done()
            return

        base = get_cache_dir()
        downloads_dir = base / "downloads"
        downloads_dir.mkdir(parents=True, exist_ok=True)

        arxiv_id = extract_arxiv_id(url)
        if not arxiv_id:
            self.ui.add_log("[ERROR] 无法识别 arXiv ID")
            self.ui.set_mini_status("ID error", "error")
            self._done()
            return

        self.ui.add_log(f"[INFO] 处理论文: {arxiv_id}")

        paper_dir, folder_name = download_source(arxiv_id, downloads_dir, log=self._emit_log)
        if paper_dir:
            result = extract_body_from_dir(
                paper_dir, self.workspace_path, folder_name, log=self._emit_log
            )
            if result:
                self.output_dir = self.workspace_path / folder_name
                download_pdf(arxiv_id, self.output_dir, log=self._emit_log)
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
