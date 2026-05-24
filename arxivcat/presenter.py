"""Presenter: all business logic. Zero dependency on any UI framework."""
from __future__ import annotations

from concurrent.futures import ThreadPoolExecutor, as_completed
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

VERSION = "v0.7.1"
AUTHOR = "by MikeDuke"
WORKSPACE_INTERNAL_DIRS = {
    "arxivcat_global_chats",
}


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
        self._task_lock = threading.Lock()
        self._task_busy = False
        self._download_all_cancelled = False
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
        if self._is_busy():
            self.ui.set_mini_status("busy", "info")
            return
        self.workspace_path = Path(path)
        self.workspace_path.mkdir(parents=True, exist_ok=True)
        (self.workspace_path / "arxivcat_global_chats").mkdir(parents=True, exist_ok=True)
        save_workspace_path(str(self.workspace_path))
        self.refresh_paper_list()
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
            if folder.name in WORKSPACE_INTERNAL_DIRS:
                continue

            name = folder.name
            parts = name.split('_')
            if len(parts) < 2:
                continue
            arxiv_id = f"{parts[0]}.{parts[1]}"
            title = ' '.join(parts[2:])
            title = re.sub(r'\s*fresh\d+$', '', title).strip()
            has_body = (folder / "body.tex").exists()
            description_ready = self._has_complete_description(folder)

            papers.append({
                "arxiv_id": arxiv_id,
                "title": title,
                "folder_name": name,
                "has_body": has_body,
                "description_ready": description_ready,
                "is_complete": has_body and description_ready,
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
            self.ui.set_buttons_enabled(True)
            if self._has_complete_description(paper_dir):
                self.ui.set_mini_status("loaded", "ok")
            else:
                self.ui.set_mini_status("description pending", "info")
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

    def refresh_paper_list(self):
        """Refresh the paper list from the current workspace folder."""
        if not self.workspace_path:
            return
        papers = self.get_paper_list()
        self.ui.set_paper_list(papers)
        self.ui.set_mini_status(f"{len(papers)} papers", "info")

    def scan_workspace_pdfs(self):
        """Scan workspace PDFs, create empty folders for new papers."""
        if not self.workspace_path:
            return
        if not self._begin_task("already running"):
            return
        self.ui.set_run_busy(True)
        self.ui.set_paper_actions_busy(True)
        self.ui.set_buttons_enabled(False)
        self.ui.clear_log()
        self.ui.set_mini_status("scanning PDFs...", "info")
        threading.Thread(target=self._scan_pdfs_work, daemon=True).start()

    def download_all_pending(self):
        """Download and extract all papers without body.tex."""
        if not self.workspace_path:
            return
        if self._is_busy():
            self.interrupt_download_all()
            return
        if not self._begin_task("already running"):
            return
        self._download_all_cancelled = False
        self.ui.set_run_busy(True)
        self.ui.set_paper_actions_busy(True)
        self.ui.set_download_all_state(True)
        self.ui.set_buttons_enabled(False)
        self.ui.clear_log()
        self.ui.set_mini_status("preparing...", "info")
        threading.Thread(target=self._download_all_work, daemon=True).start()

    def interrupt_download_all(self):
        if not self._is_busy():
            return
        self._download_all_cancelled = True
        self.ui.add_log("[INFO] Interrupt requested. Finishing in-flight tasks and cancelling queued work...")
        self.ui.set_mini_status("interrupting...", "info")

    def run_fetch(self):
        """Called when user clicks Run. Spawns background thread."""
        url = self.ui.get_url_input().strip()
        if not url:
            return
        if not self._begin_task("already running"):
            return
        self.ui.set_run_busy(True)
        self.ui.set_paper_actions_busy(True)
        self.ui.set_buttons_enabled(False)
        self.ui.clear_log()
        self.ui.set_preview("", "")
        self.ui.set_mini_status("", "info")
        self.output_dir = None
        threading.Thread(target=self._process, args=(url,), daemon=True).start()

    def open_folder(self):
        if self.output_dir and self.output_dir.exists():
            subprocess.Popen(f'explorer "{self.output_dir}"')

    def open_pdf_in_browser(self):
        if self.output_dir and self.output_dir.exists():
            for pdf in self.output_dir.glob("*.pdf"):
                os.startfile(str(pdf))
                return
        arxiv_id = extract_arxiv_id(self.ui.get_url_input())
        if arxiv_id:
            webbrowser.open(f"https://arxiv.org/pdf/{arxiv_id}")

    def strip_comments(self):
        content = self.ui.get_preview_text()
        stripped = re.sub(r'(?<!\\)%.*', '', content)
        stripped = re.sub(r'\n{3,}', '\n\n', stripped).strip()
        view = self.ui.get_view_mode()
        if view == "note":
            filename = "note.txt"
        elif view == "description":
            filename = "description.md"
        else:
            filename = f"{view}.tex"
        self.ui.set_preview(stripped, filename)
        self.ui.show_toast("Comments stripped")

    def switch_view(self):
        """Called when dropdown changes. Reload preview from disk."""
        self._load_preview()

    def save_note(self, content: str):
        if not self.output_dir:
            return
        path = self.output_dir / "note.txt"
        path.write_text(content, encoding="utf-8")

    # ── internal ──────────────────────────────────────────────

    def _is_busy(self) -> bool:
        with self._task_lock:
            return self._task_busy

    def _begin_task(self, busy_msg: str) -> bool:
        with self._task_lock:
            if self._task_busy:
                self.ui.set_mini_status(busy_msg, "info")
                return False
            self._task_busy = True
            return True

    def _ensure_note_file(self, paper_dir: Path):
        note_path = paper_dir / "note.txt"
        if not note_path.exists():
            note_path.write_text("", encoding="utf-8")

    def _ensure_description_file(self, paper_dir: Path):
        description_path = paper_dir / "description.md"
        if not description_path.exists():
            description_path.write_text("", encoding="utf-8")

    def _description_flag_path(self, paper_dir: Path) -> Path:
        return paper_dir / ".description_ready"

    def _has_complete_description(self, paper_dir: Path) -> bool:
        description_path = paper_dir / "description.md"
        flag_path = self._description_flag_path(paper_dir)
        return description_path.exists() and description_path.stat().st_size > 0 and flag_path.exists()

    def _ensure_paper_meta_files(self, paper_dir: Path):
        self._ensure_note_file(paper_dir)
        self._ensure_description_file(paper_dir)
        (paper_dir / "arxiv_chats").mkdir(parents=True, exist_ok=True)

    def _build_paper_description(self, paper_dir: Path, arxiv_id: str, title: str):
        self._ensure_description_file(paper_dir)
        try:
            self.ui.build_paper_description(str(paper_dir), arxiv_id, title)
        except Exception as exc:
            self.ui.add_log(f"[WARN] description build failed for {arxiv_id}: {exc}")

    def _should_cancel_download_all(self) -> bool:
        return self._download_all_cancelled

    def _process_pending_paper(self, paper: dict, downloads_dir: Path) -> bool:
        arxiv_id = paper["arxiv_id"]
        ws_folder = paper["folder_name"]
        out_dir = self.workspace_path / ws_folder

        if self._should_cancel_download_all():
            return False

        if paper.get("has_body") and not paper.get("description_ready"):
            self.ui.add_log(f"[INFO] {arxiv_id} missing description only, rebuilding...")
            self._ensure_paper_meta_files(out_dir)
            if self._should_cancel_download_all():
                return False
            self._build_paper_description(out_dir, arxiv_id, paper["title"])
            return self._has_complete_description(out_dir)

        paper_dir, _ = download_source(arxiv_id, downloads_dir, log=self._emit_log)
        if not paper_dir:
            self.ui.add_log(f"[ERROR]   download failed for {arxiv_id}")
            return False

        if self._should_cancel_download_all():
            return False

        result = extract_body_from_dir(
            paper_dir, self.workspace_path, ws_folder, log=self._emit_log
        )
        if not result:
            self.ui.add_log(f"[ERROR]   extract failed for {arxiv_id}")
            return False

        if self._should_cancel_download_all():
            return False

        download_pdf(arxiv_id, out_dir, log=self._emit_log)
        self._ensure_paper_meta_files(out_dir)
        if self._should_cancel_download_all():
            return False
        self._build_paper_description(out_dir, arxiv_id, paper["title"])
        return (out_dir / "body.tex").exists() and self._has_complete_description(out_dir)

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
        if view == "note":
            filename = "note.txt"
        elif view == "description":
            filename = "description.md"
        else:
            filename = f"{view}.tex"
        path = self.output_dir / filename
        if view == "note" and not path.exists():
            path.write_text("", encoding="utf-8")
        content = path.read_text(encoding="utf-8") if path.exists() else "(file not found)"
        self.ui.set_preview(content, filename)

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
            self._ensure_paper_meta_files(folder)
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
        pending = [p for p in papers if not p.get("is_complete", False)]

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

        max_workers = 25
        self.ui.add_log(f"[INFO] Using {max_workers} concurrent workers")

        with ThreadPoolExecutor(max_workers=max_workers) as executor:
            futures = {}
            for i, paper in enumerate(pending, 1):
                if self._should_cancel_download_all():
                    break
                arxiv_id = paper["arxiv_id"]
                self.ui.add_log(f"[INFO] [queued {i}/{total}] {arxiv_id}")
                future = executor.submit(self._process_pending_paper, paper, downloads_dir)
                futures[future] = paper

            completed = 0
            for future in as_completed(futures):
                if self._should_cancel_download_all():
                    for queued_future in futures:
                        queued_future.cancel()
                paper = futures[future]
                completed += 1
                arxiv_id = paper["arxiv_id"]
                self.ui.set_mini_status(f"{completed}/{total}  {arxiv_id}", "info")
                try:
                    if future.result():
                        success += 1
                except Exception as exc:
                    self.ui.add_log(f"[ERROR] {arxiv_id} failed: {exc}")
                self.ui.set_paper_list(self.get_paper_list())
                if self._should_cancel_download_all():
                    break

        if self._should_cancel_download_all():
            self.ui.add_log(f"\n[INFO] Download interrupted. {success}/{total} papers ready before stop.")
            self.ui.set_mini_status(f"interrupted {success}/{total}", "info")
        else:
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
                self._ensure_paper_meta_files(self.output_dir)
                parts = folder_name.split('_')
                title = ' '.join(parts[2:]).strip() if len(parts) > 2 else folder_name
                self._build_paper_description(self.output_dir, arxiv_id, title)
                papers = self.get_paper_list()
                self.ui.set_paper_list(papers)

        if self.output_dir:
            self._load_preview()
            self.ui.set_mini_status("done", "ok")
        self._done()

    def _done(self):
        with self._task_lock:
            self._task_busy = False
        self._download_all_cancelled = False
        self.ui.set_run_busy(False)
        self.ui.set_paper_actions_busy(False)
        self.ui.set_download_all_state(False)
        if self.output_dir:
            self.ui.set_buttons_enabled(True)
