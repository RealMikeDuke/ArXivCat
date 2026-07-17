"""Chat service: pure chat logic, zero UI dependency. Shared by GUI and CLI."""
from __future__ import annotations

import json
import re
import threading
from datetime import datetime
from pathlib import Path
from typing import Callable

from openai import OpenAI

CHAT_MODELS = {
    "Flash": "deepseek-v4-flash",
    "Pro": "deepseek-v4-pro",
}


# ── Session persistence (stateless) ───────────────────────────

def serialize_chat_history(history: list[tuple[str, str]]) -> list[dict]:
    return [{"speaker": speaker, "content": content} for speaker, content in history]


def deserialize_chat_history(data: list[dict]) -> list[tuple[str, str]]:
    history = []
    for item in data or []:
        speaker = item.get("speaker", "")
        content = item.get("content", "")
        if speaker:
            history.append((speaker, content))
    return history


def new_chat_session_path(session_dir: Path) -> Path:
    base = datetime.now().strftime("%Y%m%d_%H%M%S")
    path = session_dir / f"{base}.json"
    suffix = 1
    while path.exists():
        path = session_dir / f"{base}_{suffix}.json"
        suffix += 1
    return path


def default_chat_session_title(kind: str, arxiv_id: str = "") -> str:
    stamp = datetime.now().strftime("%Y-%m-%d %H:%M")
    if kind == "global":
        return f"Global Chat {stamp}"
    label = arxiv_id or "Paper"
    return f"{label} {stamp}"


def save_chat_session(
    *,
    session_dir: Path | None,
    session_path: Path | None,
    session_title: str,
    kind: str,
    history: list[tuple[str, str]],
    model: str,
    deep_thinking: bool,
    context_selection: dict | None = None,
    context_snapshot: str | None = None,
    view_name: str | None = None,
) -> tuple[Path | None, str]:
    if session_dir is None or not history:
        return session_path, session_title
    path = session_path or new_chat_session_path(session_dir)
    title = session_title or default_chat_session_title(kind)
    payload = {
        "title": title,
        "kind": kind,
        "model": model,
        "deep_thinking": deep_thinking,
        "messages": serialize_chat_history(history),
        "context_selection": context_selection,
        "context_snapshot": context_snapshot,
        "view_name": view_name,
        "updated_at": datetime.now().isoformat(timespec="seconds"),
    }
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")
    return path, title


def load_chat_session(path: Path) -> dict | None:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return None
    return {
        "path": path,
        "title": payload.get("title") or path.stem,
        "kind": payload.get("kind") or "paper",
        "model": payload.get("model") or None,
        "deep_thinking": payload.get("deep_thinking"),
        "history": deserialize_chat_history(payload.get("messages") or []),
        "context_selection": payload.get("context_selection") or None,
        "context_snapshot": payload.get("context_snapshot") or "",
        "view_name": payload.get("view_name") or "body",
        "updated_at": payload.get("updated_at") or "",
    }


def rename_chat_session(path: Path | None, title: str) -> None:
    if path is None:
        return
    try:
        raw = json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return
    raw["title"] = title
    raw["updated_at"] = datetime.now().isoformat(timespec="seconds")
    path.write_text(json.dumps(raw, ensure_ascii=False, indent=2), encoding="utf-8")


def delete_chat_session(path: Path | None) -> bool:
    if path is None or not path.exists():
        return False
    try:
        path.unlink()
        return True
    except Exception:
        return False


def list_chat_sessions(session_dir: Path | None) -> list[dict]:
    if session_dir is None or not session_dir.exists():
        return []
    items = []
    for p in sorted(session_dir.glob("*.json"), key=lambda p: p.stat().st_mtime, reverse=True):
        payload = load_chat_session(p)
        if payload is not None:
            items.append(payload)
    return items


# ── Context building (stateless) ──────────────────────────────

def build_side_chat_context(paper_dir: Path, selection: dict[str, bool]) -> str:
    if not paper_dir.exists():
        return "(no paper loaded)"
    sections = []
    for field, label in (
        ("body", "body"),
        ("appendix", "appendix"),
        ("description", "description"),
        ("note", "note"),
    ):
        if not selection.get(field, False):
            continue
        if field in ("body", "appendix"):
            path = paper_dir / f"{field}.tex"
        elif field == "description":
            path = paper_dir / "description.md"
        else:
            path = paper_dir / "note.txt"
        if path.exists():
            content = path.read_text(encoding="utf-8", errors="ignore").strip()
        else:
            content = f"({label} unavailable)"
        sections.append(f"{label}:\n{content}")
    if not sections:
        return "(no context selected)"
    return "\n\n".join(sections)


def build_description_context(entries: list[dict], selection: dict[str, dict[str, bool]]) -> str:
    blocks = []
    for entry in entries:
        entry_selection = selection.get(entry["folder_name"], {})
        sections = []
        for field, label in (
            ("body", "body"),
            ("appendix", "appendix"),
            ("description", "description"),
            ("note", "note"),
        ):
            if not entry_selection.get(field, False):
                continue
            content = entry.get(field, "").strip() or f"({label} unavailable)"
            content = f"arXiv ID: {entry['arxiv_id']} | Title: {entry['title']}\n\n{content}"
            sections.append(f"{label}:\n{content}")
        if not sections:
            continue
        block = (
            f"Paper [{entry['index']}]\n"
            f"arXiv ID: {entry['arxiv_id']}\n"
            f"Title: {entry['title']}\n"
            f"---\n"
            f"\n\n".join(sections)
        )
        blocks.append(block)
    return "\n\n---\n\n".join(blocks)


def collect_workspace_descriptions(workspace_path: Path, papers: list[dict]) -> list[dict]:
    entries = []
    for index, paper in enumerate(papers, 1):
        paper_dir = workspace_path / paper["folder_name"]

        def _read(filename: str) -> str:
            path = paper_dir / filename
            if path.exists():
                return path.read_text(encoding="utf-8", errors="ignore").strip()
            return ""

        entries.append({
            "index": index,
            "arxiv_id": paper["arxiv_id"],
            "title": paper["title"],
            "folder_name": paper["folder_name"],
            "body": _read("body.tex"),
            "appendix": _read("appendix.tex"),
            "note": _read("note.txt"),
            "description": _read("description.md"),
        })
    return entries


# ── Selection / delta (stateless) ─────────────────────────────

def compute_selection_delta(current: dict[str, bool], last_sent: dict[str, bool]) -> dict[str, bool]:
    delta: dict[str, bool] = {}
    for key, value in current.items():
        if value and not last_sent.get(key, False):
            delta[key] = True
    return delta


def compute_global_selection_delta(
    current: dict[str, dict[str, bool]],
    last_sent: dict[str, dict[str, bool]],
) -> dict[str, dict[str, bool]]:
    delta: dict[str, dict[str, bool]] = {}
    for folder_name, fields in current.items():
        last_fields = last_sent.get(folder_name, {})
        paper_delta: dict[str, bool] = {}
        for field, enabled in fields.items():
            if enabled and not last_fields.get(field, False):
                paper_delta[field] = True
        if paper_delta:
            delta[folder_name] = paper_delta
    return delta


def default_global_context_selection(entries: list[dict]) -> dict[str, dict[str, bool]]:
    return {
        entry["folder_name"]: {
            "body": False,
            "appendix": False,
            "description": True,
            "note": False,
        }
        for entry in entries
    }


# ── Formatting (stateless) ────────────────────────────────────

def format_side_chat_context_summary(selection: dict[str, bool]) -> str:
    parts = [f[0] for f in ("body", "appendix", "description", "note") if selection.get(f)]
    return f"ctx: {','.join(parts)}" if parts else "ctx: none"


def format_global_context_summary(selection: dict[str, dict[str, bool]]) -> str:
    counts = {"body": 0, "appendix": 0, "description": 0, "note": 0}
    for paper_selection in selection.values():
        for field in counts:
            if paper_selection.get(field):
                counts[field] += 1
    return (
        f"ctx: d={counts['description']} | b={counts['body']} | "
        f"a={counts['appendix']} | n={counts['note']}"
    )


def format_metrics(metrics: dict) -> str:
    parts = []
    if metrics.get("ttft"):
        parts.append(f"TTFT: {metrics['ttft']:.0f}ms")
    if metrics.get("tokens_per_sec"):
        parts.append(f"{metrics['tokens_per_sec']:.1f} tok/s")
    if metrics.get("prompt_tokens"):
        parts.append(f"in: {metrics['prompt_tokens']}")
    if metrics.get("completion_tokens"):
        parts.append(f"out: {metrics['completion_tokens']}")
    return " | ".join(parts) if parts else ""


# ── ChatService (stateful: client + streaming control) ────────

class ChatService:
    """Manages DeepSeek API client and streaming chat calls."""

    def __init__(self):
        self._client: OpenAI | None = None
        self._cancelled = False
        self._busy = False

    # ── client ────────────────────────────────────────────────

    def ensure_client(self) -> OpenAI:
        if self._client is None:
            from arxivcat.presenter import load_cached_token  # lazy import to avoid circular dependency
            api_key = load_cached_token()
            if not api_key:
                raise ValueError("Missing DeepSeek API token. Please restart and enter your token.")
            self._client = OpenAI(api_key=api_key, base_url="https://api.deepseek.com")
        return self._client

    # ── busy / cancel ─────────────────────────────────────────

    @property
    def is_busy(self) -> bool:
        return self._busy

    def cancel(self) -> None:
        self._cancelled = True

    # ── streaming chat ────────────────────────────────────────

    def stream_chat(
        self,
        messages: list[dict],
        *,
        on_token: Callable[[str, bool], None],
        on_status: Callable[[str], None],
        on_complete: Callable[[str], None],
        on_error: Callable[[str], None] | None = None,
        model: str | None = None,
        include_thinking: bool = False,
        deep_thinking: bool = True,
    ) -> None:
        """Run a streaming chat request in a background thread.

        on_token(content, is_first) – called for each text chunk.
        on_status(msg)              – called on completion / error / cancel.
        on_complete(full_text)      – called with the full response text.
        on_error(error_msg)         – called on exception with the error string.
        """
        self._cancelled = False
        self._busy = True

        def _work() -> None:
            output_buffer = ""
            try:
                client = self.ensure_client()
                extra_params: dict = {}
                if include_thinking and deep_thinking:
                    extra_params["extra_body"] = {"thinking": {"type": "enabled"}}
                    extra_params["reasoning_effort"] = "high"

                response = client.chat.completions.create(
                    model=model or CHAT_MODELS["Flash"],
                    messages=messages,
                    stream=True,
                    **extra_params,
                )

                first_chunk = True
                for chunk in response:
                    if self._cancelled:
                        break
                    if chunk.choices[0].delta.content:
                        content = chunk.choices[0].delta.content
                        content = re.sub(r'\n{2,}', '\n', content)
                        output_buffer += content
                        on_token(content, first_chunk)
                        first_chunk = False

                if not self._cancelled and output_buffer.strip():
                    on_token("\n", False)
                if self._cancelled:
                    on_status("cancelled")
                else:
                    on_status(model or CHAT_MODELS["Flash"])
                    on_complete(output_buffer.strip())
            except Exception as exc:
                if on_error:
                    on_error(str(exc))
                else:
                    on_token(f"system: {exc}\n", True)
                on_status("chat error")
            finally:
                self._cancelled = False
                self._busy = False

        threading.Thread(target=_work, daemon=True).start()

    # ── description generation ────────────────────────────────

    def build_description(
        self,
        paper_dir: str | Path,
        arxiv_id: str,
        title: str,
        *,
        log_cb: Callable[[str], None] | None = None,
    ) -> None:
        """Generate description.md for a paper using DeepSeek Flash."""
        paper_path = Path(paper_dir)
        description_path = paper_path / "description.md"
        flag_path = paper_path / ".description_ready"
        body_path = paper_path / "body.tex"
        appendix_path = paper_path / "appendix.tex"

        body = body_path.read_text(encoding="utf-8", errors="ignore") if body_path.exists() else ""
        appendix = appendix_path.read_text(encoding="utf-8", errors="ignore") if appendix_path.exists() else ""
        context = body
        if appendix.strip():
            context += "\n\n[Appendix]\n" + appendix
        if not context.strip():
            raise ValueError("paper text is empty")

        if log_cb:
            log_cb(f"[INFO] Building description.md for {arxiv_id}...")

        client = self.ensure_client()
        messages = [
            {
                "role": "system",
                "content": (
                    "You write structured markdown briefs for arXiv papers. "
                    "The brief will later be used for semantic paper search inside a local workspace. "
                    "Be detailed but compact, faithful to the provided paper text, and emphasize "
                    "searchable technical concepts. Output markdown only. Use these sections exactly: "
                    "# Overview, ## Problem, ## Method, ## Key Contributions, "
                    "## Technical Details, ## Search Tags, ## Good Match Queries."
                ),
            },
            {
                "role": "user",
                "content": f"arXiv ID: {arxiv_id}\nTitle: {title}\n\nPaper text snippet:\n{context}",
            },
        ]
        response = client.chat.completions.create(
            model="deepseek-v4-flash",
            messages=messages,
            max_tokens=1400,
        )
        content = (response.choices[0].message.content or "").strip()
        if not content:
            raise ValueError("empty description response")
        if flag_path.exists():
            flag_path.unlink()
        description_path.write_text(content + "\n", encoding="utf-8")
        flag_path.write_text("ok\n", encoding="utf-8")
        if log_cb:
            log_cb(f"[OK] description.md saved for {arxiv_id}")
