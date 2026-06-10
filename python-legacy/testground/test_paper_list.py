"""Headless test: list papers and load them without GUI."""
import sys
from pathlib import Path
from arxivcat.presenter import Presenter, get_cache_dir
from arxivcat.core import extract_body_from_dir


class FakeUI:
    """Minimal stub satisfying UIProtocol for headless testing."""
    def __init__(self):
        self.logs = []
    def add_log(self, msg): self.logs.append(msg); print(msg)
    def set_mini_status(self, msg, level="info"): print(f"  [status] {msg} ({level})")
    def set_preview(self, content, label): print(f"  [preview] {label}: {len(content)} chars")
    def set_buttons_enabled(self, enabled): pass
    def set_run_busy(self, busy): pass
    def set_paper_actions_busy(self, busy): pass
    def show_toast(self, msg, duration_ms=2000): pass
    def get_url_input(self): return ""
    def get_view_mode(self): return "body"
    def get_preview_text(self): return ""
    def clear_log(self): self.logs.clear()
    def set_url_input(self, url): print(f"  [url] {url}")
    def set_paper_list(self, papers): pass
    def set_title(self, title): pass
    def build_paper_description(self, paper_dir, arxiv_id, title): pass
    def set_download_all_state(self, interrupt_mode): pass
    def run(self): pass


def main():
    ui = FakeUI()
    p = Presenter(ui)

    print("=" * 60)
    print("STEP 1: List papers")
    print("=" * 60)
    papers = p.get_paper_list()
    if not papers:
        print("No papers found!")
        return

    for i, paper in enumerate(papers):
        print(f"  [{i}] {paper['arxiv_id']}  |  {paper['title']}  |  folder={paper['folder_name']}")

    print()
    print("=" * 60)
    print("STEP 2: Load each paper")
    print("=" * 60)
    for paper in papers:
        print(f"\n--- Loading: {paper['arxiv_id']} (folder={paper['folder_name']}) ---")
        ui.logs.clear()
        p.load_paper(paper["folder_name"])
        print()


if __name__ == "__main__":
    main()
