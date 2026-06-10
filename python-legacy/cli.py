"""CLI entry point — full-featured arXiv paper management."""
from __future__ import annotations

import argparse
import signal
import sys
import time
from pathlib import Path

from arxivcat.chat_service import (
    ChatService, CHAT_MODELS,
    save_chat_session, load_chat_session,
    rename_chat_session, delete_chat_session, list_chat_sessions,
    build_side_chat_context, build_description_context,
    collect_workspace_descriptions,
    default_global_context_selection,
    format_side_chat_context_summary, format_global_context_summary,
)
from arxivcat.core import extract_arxiv_id
from arxivcat.presenter import (
    Presenter, load_cached_token, save_token,
    load_workspace_path, save_workspace_path,
)
from arxivcat.ui.cli_ui import CliUI, C


def _get_workspace(args) -> Path | None:
    ws = getattr(args, 'workspace', None)
    if ws:
        return Path(ws)
    cached = load_workspace_path()
    if cached:
        return Path(cached)
    return None


def _resolve_workspace(args) -> Path:
    ws = _get_workspace(args)
    if ws is None:
        print(f"{C.RED}No workspace set. Use 'workspace open <path>' first.{C.R}")
        sys.exit(1)
    if not ws.exists():
        print(f"{C.RED}Workspace not found: {ws}{C.R}")
        sys.exit(1)
    return ws


def _find_paper(workspace: Path, arxiv_id: str) -> dict | None:
    """Find a paper in the workspace by arxiv_id (exact or prefix match)."""
    normalized_query = arxiv_id.replace('.', '_').replace('-', '_').lower()
    for d in sorted(workspace.iterdir()):
        if not d.is_dir():
            continue
        parts = d.name.split('_')
        if len(parts) >= 2:
            fid = f"{parts[0]}_{parts[1]}"
            if fid == normalized_query or fid.startswith(normalized_query) or normalized_query.startswith(fid):
                return {
                    "arxiv_id": fid.replace('_', '.'),
                    "folder_name": d.name,
                    "title": ' '.join(parts[2:]) if len(parts) > 2 else d.name,
                }
    return None


def _ensure_token() -> str:
    token = load_cached_token()
    if not token:
        print(f"{C.RED}No API token found. Run 'token set' first.{C.R}")
        sys.exit(1)
    return token


# ═══════════════════════════════════════════════════════════════
# Chat loops
# ═══════════════════════════════════════════════════════════════

def _chat_header(model: str, deep_thinking: bool, ctx_summary: str) -> None:
    dt = f"{C.GREEN}on{C.R}" if deep_thinking else f"{C.GRAY}off{C.R}"
    print(f"{C.BOLD}Model:{C.R} {CHAT_MODELS[model]}  "
          f"{C.BOLD}Deep Thinking:{C.R} {dt}  "
          f"{C.BOLD}{ctx_summary}{C.R}")
    print(f"{C.GRAY}Commands: /quit /model /thinking /context /save /load /history /clear /help{C.R}")
    print()


def _chat_input(prompt: str = "You: ") -> str | None:
    try:
        return input(f"{C.BOLD}{prompt}{C.R}")
    except (EOFError, KeyboardInterrupt):
        return None


def _run_side_chat(workspace: Path, arxiv_id: str, presenter: Presenter, cli_ui: CliUI) -> None:
    paper = _find_paper(workspace, arxiv_id)
    if paper is None:
        print(f"{C.RED}Paper not found in workspace: {arxiv_id}{C.R}")
        return

    cli_ui.set_quiet(True)
    presenter.load_paper(paper["folder_name"])
    cli_ui.set_quiet(False)

    paper_dir = workspace / paper["folder_name"]
    chat_service = ChatService()
    history: list[tuple[str, str]] = []
    session_path: Path | None = None
    session_title = ""
    context_snapshot = ""
    view_name = "body"
    selection: dict[str, bool] = {"body": True, "appendix": False, "description": False, "note": False}
    last_sent: dict[str, bool] = {}
    model = "Flash"
    deep_thinking = True

    def _ctx_summary() -> str:
        return format_side_chat_context_summary(selection)

    def _save() -> None:
        nonlocal session_path, session_title
        chat_dir = paper_dir / "arxiv_chats"
        chat_dir.mkdir(parents=True, exist_ok=True)
        session_path, session_title = save_chat_session(
            session_dir=chat_dir,
            session_path=session_path,
            session_title=session_title,
            kind="paper",
            history=history,
            model=model,
            deep_thinking=deep_thinking,
            context_selection=selection,
            context_snapshot=context_snapshot,
            view_name=view_name,
        )
        if session_path:
            print(f"{C.GREEN}Saved: {session_title}{C.R}")

    def _send(prompt: str) -> None:
        nonlocal last_sent
        full_ctx = build_side_chat_context(paper_dir, selection)
        last_sent = dict(selection)
        history.append(("you", prompt))

        messages = [
            {"role": "system", "content": (
                "You are a compact in-app chat assistant inside an arXiv paper extraction tool. "
                "Maintain conversation continuity. If the user asks a general question, answer it normally. "
                "If useful, use the paper preview as extra context. "
                "IMPORTANT: When using any content from the paper context (body, appendix, description, or note), "
                "you MUST explicitly include the paper's complete arXiv ID in your response."
            )},
            {"role": "user", "content": f"Paper content:\n{full_ctx}"},
        ]
        for speaker, msg in history[-12:]:
            role = "user" if speaker == "you" else "assistant"
            messages.append({"role": role, "content": msg})

        cancelled = [False]

        def _on_token(content: str, is_first: bool) -> None:
            if is_first:
                print(f"\n{C.BOLD}deepseek:{C.R} ", end='', flush=True)
            print(content, end='', flush=True)

        def _on_status(msg: str) -> None:
            if msg == "cancelled":
                print(f"\n{C.GRAY}[cancelled]{C.R}")
                cancelled[0] = True
            elif msg == "chat error":
                print(f"\n{C.RED}[chat error]{C.R}")
                cancelled[0] = True

        def _on_complete(answer: str) -> None:
            if answer:
                history.append(("deepseek", answer))
                _save()
            print()

        def _on_error(error_msg: str) -> None:
            print(f"\n{C.RED}system: {error_msg}{C.R}")

        original_handler = signal.getsignal(signal.SIGINT)
        def _cancel_handler(signum, frame):
            chat_service.cancel()
        signal.signal(signal.SIGINT, _cancel_handler)

        try:
            chat_service.stream_chat(
                messages=messages,
                on_token=_on_token,
                on_status=_on_status,
                on_complete=_on_complete,
                on_error=_on_error,
                model=CHAT_MODELS[model],
                include_thinking=True,
                deep_thinking=deep_thinking,
            )
            while chat_service.is_busy and not cancelled[0]:
                time.sleep(0.1)
        finally:
            signal.signal(signal.SIGINT, original_handler)

    print(f"\n{C.BOLD}Side Chat — {paper['arxiv_id']}: {paper['title']}{C.R}\n")
    _chat_header(model, deep_thinking, _ctx_summary())

    while True:
        line = _chat_input()
        if line is None:
            break
        line = line.strip()
        if not line:
            continue

        if line.startswith('/'):
            parts = line.split(maxsplit=1)
            cmd = parts[0].lower()
            arg = parts[1] if len(parts) > 1 else ""

            if cmd == '/quit' or cmd == '/exit':
                _save()
                break
            elif cmd == '/model':
                if arg in CHAT_MODELS:
                    model = arg
                    print(f"{C.GREEN}Model: {CHAT_MODELS[model]}{C.R}")
                else:
                    print(f"{C.GRAY}Available: {', '.join(CHAT_MODELS.keys())}{C.R}")
            elif cmd == '/thinking':
                deep_thinking = not deep_thinking
                dt = f"{C.GREEN}on{C.R}" if deep_thinking else f"{C.GRAY}off{C.R}"
                print(f"Deep Thinking: {dt}")
            elif cmd == '/context':
                print(f"\n{C.BOLD}Context selection:{C.R}")
                for field in ("body", "appendix", "description", "note"):
                    status = f"{C.GREEN}✓{C.R}" if selection.get(field) else f"{C.GRAY}✗{C.R}"
                    locked = " 🔒" if last_sent.get(field) else ""
                    print(f"  {status} {field}{locked}")
                print(f"{C.GRAY}Toggle: /context <field>{C.R}")
                if arg in ("body", "appendix", "description", "note"):
                    if not last_sent.get(arg):
                        selection[arg] = not selection.get(arg, False)
                        s = f"{C.GREEN}on{C.R}" if selection[arg] else f"{C.GRAY}off{C.R}"
                        print(f"  {arg}: {s}")
                    else:
                        print(f"  {C.GRAY}{arg} is locked (already sent){C.R}")
            elif cmd == '/save':
                _save()
            elif cmd == '/load':
                chat_dir = paper_dir / "arxiv_chats"
                sessions = list_chat_sessions(chat_dir)
                if not sessions:
                    print(f"{C.GRAY}No saved sessions.{C.R}")
                else:
                    print(f"\n{C.BOLD}Saved sessions:{C.R}")
                    for i, s in enumerate(sessions, 1):
                        print(f"  {i}. {s['title']} ({len(s['history'])} msgs)")
                    choice = _chat_input("Load # (or Enter to cancel): ")
                    if choice and choice.strip().isdigit():
                        idx = int(choice.strip()) - 1
                        if 0 <= idx < len(sessions):
                            s = sessions[idx]
                            history = list(s["history"])
                            session_path = s["path"]
                            session_title = s["title"]
                            if s.get("context_selection"):
                                selection = dict(s["context_selection"])
                            last_sent.clear()
                            if s.get("model") in CHAT_MODELS:
                                model = s["model"]
                            if s.get("deep_thinking") is not None:
                                deep_thinking = s["deep_thinking"]
                            print(f"{C.GREEN}Loaded: {session_title}{C.R}")
            elif cmd == '/history':
                if not history:
                    print(f"{C.GRAY}(empty){C.R}")
                else:
                    for speaker, msg in history:
                        label = f"{C.BOLD}{speaker}:{C.R}"
                        print(f"{label} {msg[:200]}{'...' if len(msg) > 200 else ''}")
            elif cmd == '/clear':
                history.clear()
                session_path = None
                session_title = ""
                last_sent.clear()
                print(f"{C.GREEN}Chat cleared.{C.R}")
            elif cmd == '/help':
                print(textwrap.dedent(f"""
                {C.BOLD}Chat Commands:{C.R}
                  /quit, /exit       Save and exit chat
                  /model <Flash|Pro> Switch model
                  /thinking          Toggle deep thinking
                  /context [field]   Show/toggle context fields
                  /save              Save current session
                  /load              Load a saved session
                  /history           Show chat history
                  /clear             Clear chat (new session)
                  /help              Show this help
                """))
            else:
                print(f"{C.GRAY}Unknown command: {cmd}. Try /help{C.R}")
        else:
            _send(line)
            _chat_header(model, deep_thinking, _ctx_summary())


def _run_global_chat(workspace: Path, presenter: Presenter, cli_ui: CliUI) -> None:
    papers = presenter.get_paper_list()
    if not papers:
        print(f"{C.RED}No papers in workspace.{C.R}")
        return

    entries = collect_workspace_descriptions(workspace, papers)
    if not entries:
        print(f"{C.RED}No papers with content in workspace.{C.R}")
        return

    global_selection = default_global_context_selection(entries)
    chat_service = ChatService()
    history: list[tuple[str, str]] = []
    session_path: Path | None = None
    session_title = ""
    context_snapshot = ""
    last_sent: dict[str, dict[str, bool]] = {}
    model = "Flash"
    deep_thinking = True

    def _ctx_summary() -> str:
        return format_global_context_summary(global_selection)

    def _save() -> None:
        nonlocal session_path, session_title
        chat_dir = workspace / "arxiv_global_chats"
        chat_dir.mkdir(parents=True, exist_ok=True)
        session_path, session_title = save_chat_session(
            session_dir=chat_dir,
            session_path=session_path,
            session_title=session_title,
            kind="global",
            history=history,
            model=model,
            deep_thinking=deep_thinking,
            context_selection=global_selection,
            context_snapshot=context_snapshot,
        )
        if session_path:
            print(f"{C.GREEN}Saved: {session_title}{C.R}")

    def _send(prompt: str) -> None:
        nonlocal last_sent
        full_ctx = build_description_context(entries, global_selection)
        last_sent = {
            folder: dict(fields)
            for folder, fields in global_selection.items()
        }
        history.append(("you", prompt))

        messages = [
            {"role": "system", "content": (
                "You are a global workspace chat assistant inside an arXiv paper extraction tool. "
                "You can only use the provided numbered paper context from the current workspace. "
                "Maintain short conversation continuity. If the user wants papers, recommend only from "
                "the given list. Include bracketed numbers, arXiv ids, exact titles, and concise reasons "
                "when pointing to papers. If the user asks a general question unrelated to the provided "
                "workspace, answer briefly and say it is outside the workspace scope. "
                "IMPORTANT: When using any content from a paper's context (body, appendix, description, "
                "or note), you MUST explicitly include the paper's complete arXiv ID in your response. "
                "Output markdown only."
            )},
            {"role": "user", "content": f"Workspace paper context:\n{full_ctx}"},
        ]
        for speaker, msg in history[-12:]:
            role = "user" if speaker == "you" else "assistant"
            messages.append({"role": role, "content": msg})

        cancelled = [False]

        def _on_token(content: str, is_first: bool) -> None:
            if is_first:
                print(f"\n{C.BOLD}deepseek:{C.R} ", end='', flush=True)
            print(content, end='', flush=True)

        def _on_status(msg: str) -> None:
            if msg == "cancelled":
                print(f"\n{C.GRAY}[cancelled]{C.R}")
                cancelled[0] = True
            elif msg == "chat error":
                print(f"\n{C.RED}[chat error]{C.R}")
                cancelled[0] = True

        def _on_complete(answer: str) -> None:
            if answer:
                history.append(("deepseek", answer))
                _save()
            print()

        def _on_error(error_msg: str) -> None:
            print(f"\n{C.RED}system: {error_msg}{C.R}")

        original_handler = signal.getsignal(signal.SIGINT)
        def _cancel_handler(signum, frame):
            chat_service.cancel()
        signal.signal(signal.SIGINT, _cancel_handler)

        try:
            chat_service.stream_chat(
                messages=messages,
                on_token=_on_token,
                on_status=_on_status,
                on_complete=_on_complete,
                on_error=_on_error,
                model=CHAT_MODELS[model],
                include_thinking=True,
                deep_thinking=deep_thinking,
            )
            while chat_service.is_busy and not cancelled[0]:
                time.sleep(0.1)
        finally:
            signal.signal(signal.SIGINT, original_handler)

    print(f"\n{C.BOLD}Global Chat — {len(entries)} papers in workspace{C.R}\n")
    _chat_header(model, deep_thinking, _ctx_summary())

    while True:
        line = _chat_input()
        if line is None:
            break
        line = line.strip()
        if not line:
            continue

        if line.startswith('/'):
            parts = line.split(maxsplit=1)
            cmd = parts[0].lower()
            arg = parts[1] if len(parts) > 1 else ""

            if cmd == '/quit' or cmd == '/exit':
                _save()
                break
            elif cmd == '/model':
                if arg in CHAT_MODELS:
                    model = arg
                    print(f"{C.GREEN}Model: {CHAT_MODELS[model]}{C.R}")
                else:
                    print(f"{C.GRAY}Available: {', '.join(CHAT_MODELS.keys())}{C.R}")
            elif cmd == '/thinking':
                deep_thinking = not deep_thinking
                dt = f"{C.GREEN}on{C.R}" if deep_thinking else f"{C.GRAY}off{C.R}"
                print(f"Deep Thinking: {dt}")
            elif cmd == '/context':
                print(f"\n{C.BOLD}Global context: {_ctx_summary()}{C.R}")
                print(f"{C.GRAY}Use /context-all to toggle all, or edit via GUI.{C.R}")
            elif cmd == '/save':
                _save()
            elif cmd == '/load':
                chat_dir = workspace / "arxiv_global_chats"
                sessions = list_chat_sessions(chat_dir)
                if not sessions:
                    print(f"{C.GRAY}No saved sessions.{C.R}")
                else:
                    print(f"\n{C.BOLD}Saved sessions:{C.R}")
                    for i, s in enumerate(sessions, 1):
                        print(f"  {i}. {s['title']} ({len(s['history'])} msgs)")
                    choice = _chat_input("Load # (or Enter to cancel): ")
                    if choice and choice.strip().isdigit():
                        idx = int(choice.strip()) - 1
                        if 0 <= idx < len(sessions):
                            s = sessions[idx]
                            history = list(s["history"])
                            session_path = s["path"]
                            session_title = s["title"]
                            if s.get("context_selection"):
                                global_selection.clear()
                                global_selection.update(s["context_selection"])
                            last_sent.clear()
                            if s.get("model") in CHAT_MODELS:
                                model = s["model"]
                            if s.get("deep_thinking") is not None:
                                deep_thinking = s["deep_thinking"]
                            print(f"{C.GREEN}Loaded: {session_title}{C.R}")
            elif cmd == '/history':
                if not history:
                    print(f"{C.GRAY}(empty){C.R}")
                else:
                    for speaker, msg in history:
                        label = f"{C.BOLD}{speaker}:{C.R}"
                        print(f"{label} {msg[:200]}{'...' if len(msg) > 200 else ''}")
            elif cmd == '/clear':
                history.clear()
                session_path = None
                session_title = ""
                last_sent.clear()
                print(f"{C.GREEN}Chat cleared.{C.R}")
            elif cmd == '/help':
                print(textwrap.dedent(f"""
                {C.BOLD}Chat Commands:{C.R}
                  /quit, /exit       Save and exit chat
                  /model <Flash|Pro> Switch model
                  /thinking          Toggle deep thinking
                  /context           Show context summary
                  /save              Save current session
                  /load              Load a saved session
                  /history           Show chat history
                  /clear             Clear chat (new session)
                  /help              Show this help
                """))
            else:
                print(f"{C.GRAY}Unknown command: {cmd}. Try /help{C.R}")
        else:
            _send(line)
            _chat_header(model, deep_thinking, _ctx_summary())


# ═══════════════════════════════════════════════════════════════
# Command handlers
# ═══════════════════════════════════════════════════════════════

def _cmd_workspace_open(args) -> None:
    path = Path(args.path).resolve()
    if not path.exists():
        print(f"{C.RED}Path not found: {path}{C.R}")
        sys.exit(1)
    if not path.is_dir():
        print(f"{C.RED}Not a directory: {path}{C.R}")
        sys.exit(1)
    save_workspace_path(str(path))
    print(f"{C.GREEN}Workspace: {path}{C.R}")


def _cmd_workspace_scan(args) -> None:
    ws = _resolve_workspace(args)
    cli_ui = CliUI()
    presenter = Presenter(cli_ui)
    presenter.open_workspace(str(ws))
    presenter.scan_workspace_pdfs()
    time.sleep(0.5)
    papers = presenter.get_paper_list()
    cli_ui._papers = papers
    cli_ui.print_paper_list()


def _cmd_paper_list(args) -> None:
    ws = _resolve_workspace(args)
    cli_ui = CliUI()
    presenter = Presenter(cli_ui)
    presenter.open_workspace(str(ws))
    papers = presenter.get_paper_list()
    cli_ui._papers = papers
    cli_ui.print_paper_list()


def _cmd_paper_download(args) -> None:
    ws = _resolve_workspace(args)
    arxiv_id = extract_arxiv_id(args.id_or_url)
    if not arxiv_id:
        print(f"{C.RED}Could not parse arXiv ID from: {args.id_or_url}{C.R}")
        sys.exit(1)
    cli_ui = CliUI(url=args.id_or_url)
    presenter = Presenter(cli_ui)
    presenter.open_workspace(str(ws))
    print(f"{C.BLUE}[INFO] Downloading {arxiv_id}...{C.R}")
    presenter.run_fetch()
    time.sleep(0.5)
    paper = _find_paper(ws, arxiv_id)
    if paper:
        cli_ui.print_paper_info(paper)
    else:
        print(f"{C.RED}Download may have failed.{C.R}")


def _cmd_paper_download_all(args) -> None:
    ws = _resolve_workspace(args)
    cli_ui = CliUI()
    presenter = Presenter(cli_ui)
    presenter.open_workspace(str(ws))
    presenter.download_all_pending()
    time.sleep(1)
    papers = presenter.get_paper_list()
    cli_ui._papers = papers
    cli_ui.print_paper_list()


def _cmd_paper_preview(args) -> None:
    ws = _resolve_workspace(args)
    arxiv_id = extract_arxiv_id(args.id_or_url)
    if not arxiv_id:
        print(f"{C.RED}Could not parse arXiv ID from: {args.id_or_url}{C.R}")
        sys.exit(1)
    paper = _find_paper(ws, arxiv_id)
    if paper is None:
        print(f"{C.RED}Paper not found: {args.id_or_url}{C.R}")
        sys.exit(1)
    view = args.view or "body"
    cli_ui = CliUI(view=view)
    presenter = Presenter(cli_ui)
    presenter.open_workspace(str(ws))
    presenter.load_paper(paper["folder_name"])


def _cmd_paper_note(args) -> None:
    ws = _resolve_workspace(args)
    arxiv_id = extract_arxiv_id(args.id_or_url)
    if not arxiv_id:
        print(f"{C.RED}Could not parse arXiv ID from: {args.id_or_url}{C.R}")
        sys.exit(1)
    paper = _find_paper(ws, arxiv_id)
    if paper is None:
        print(f"{C.RED}Paper not found: {args.id_or_url}{C.R}")
        sys.exit(1)
    paper_dir = ws / paper["folder_name"]
    note_path = paper_dir / "note.txt"

    if args.text:
        note_path.write_text(' '.join(args.text), encoding="utf-8")
        print(f"{C.GREEN}Note saved.{C.R}")
    elif args.edit:
        import tempfile, subprocess
        content = note_path.read_text(encoding="utf-8") if note_path.exists() else ""
        with tempfile.NamedTemporaryFile(mode='w', suffix='.txt', delete=False, encoding='utf-8') as f:
            f.write(content)
            tmp_path = f.name
        editor = os.environ.get('EDITOR', 'notepad')
        subprocess.call([editor, tmp_path])
        new_content = Path(tmp_path).read_text(encoding="utf-8")
        note_path.write_text(new_content, encoding="utf-8")
        Path(tmp_path).unlink(missing_ok=True)
        print(f"{C.GREEN}Note saved.{C.R}")
    else:
        if note_path.exists():
            print(note_path.read_text(encoding="utf-8"))
        else:
            print(f"{C.GRAY}(no note){C.R}")


def _cmd_paper_strip(args) -> None:
    ws = _resolve_workspace(args)
    arxiv_id = extract_arxiv_id(args.id_or_url)
    if not arxiv_id:
        print(f"{C.RED}Could not parse arXiv ID from: {args.id_or_url}{C.R}")
        sys.exit(1)
    paper = _find_paper(ws, arxiv_id)
    if paper is None:
        print(f"{C.RED}Paper not found: {args.id_or_url}{C.R}")
        sys.exit(1)
    cli_ui = CliUI()
    presenter = Presenter(cli_ui)
    presenter.open_workspace(str(ws))
    presenter.load_paper(paper["folder_name"])
    presenter.strip_comments()


def _cmd_paper_open(args) -> None:
    ws = _resolve_workspace(args)
    arxiv_id = extract_arxiv_id(args.id_or_url)
    if not arxiv_id:
        print(f"{C.RED}Could not parse arXiv ID from: {args.id_or_url}{C.R}")
        sys.exit(1)
    paper = _find_paper(ws, arxiv_id)
    if paper is None:
        print(f"{C.RED}Paper not found: {args.id_or_url}{C.R}")
        sys.exit(1)
    cli_ui = CliUI()
    presenter = Presenter(cli_ui)
    presenter.open_workspace(str(ws))
    presenter.load_paper(paper["folder_name"])
    presenter.open_folder()


def _cmd_paper_pdf(args) -> None:
    ws = _resolve_workspace(args)
    arxiv_id = extract_arxiv_id(args.id_or_url)
    if not arxiv_id:
        print(f"{C.RED}Could not parse arXiv ID from: {args.id_or_url}{C.R}")
        sys.exit(1)
    paper = _find_paper(ws, arxiv_id)
    if paper is None:
        print(f"{C.RED}Paper not found: {args.id_or_url}{C.R}")
        sys.exit(1)
    cli_ui = CliUI()
    presenter = Presenter(cli_ui)
    presenter.open_workspace(str(ws))
    presenter.load_paper(paper["folder_name"])
    presenter.open_pdf_in_browser()


def _cmd_paper_info(args) -> None:
    ws = _resolve_workspace(args)
    arxiv_id = extract_arxiv_id(args.id_or_url)
    if not arxiv_id:
        print(f"{C.RED}Could not parse arXiv ID from: {args.id_or_url}{C.R}")
        sys.exit(1)
    paper = _find_paper(ws, arxiv_id)
    if paper is None:
        print(f"{C.RED}Paper not found: {args.id_or_url}{C.R}")
        sys.exit(1)
    cli_ui = CliUI()
    cli_ui.print_paper_info(paper)
    paper_dir = ws / paper["folder_name"]
    for label, fname in [("body", "body.tex"), ("appendix", "appendix.tex"),
                          ("description", "description.md"), ("note", "note.txt")]:
        path = paper_dir / fname
        status = f"{C.GREEN}[OK]{C.R}" if path.exists() else f"{C.GRAY}[  ]{C.R}"
        size = f" ({path.stat().st_size} bytes)" if path.exists() else ""
        print(f"  {status} {label}{size}")


def _cmd_chat_side(args) -> None:
    ws = _resolve_workspace(args)
    _ensure_token()
    arxiv_id = extract_arxiv_id(args.id_or_url)
    if not arxiv_id:
        print(f"{C.RED}Could not parse arXiv ID: {args.id_or_url}{C.R}")
        sys.exit(1)
    cli_ui = CliUI()
    presenter = Presenter(cli_ui)
    presenter.open_workspace(str(ws))
    _run_side_chat(ws, arxiv_id, presenter, cli_ui)


def _cmd_chat_global(args) -> None:
    ws = _resolve_workspace(args)
    _ensure_token()
    cli_ui = CliUI()
    presenter = Presenter(cli_ui)
    presenter.open_workspace(str(ws))
    _run_global_chat(ws, presenter, cli_ui)


def _cmd_token_status(args) -> None:
    token = load_cached_token()
    if token:
        masked = token[:8] + "..." + token[-4:]
        print(f"{C.GREEN}Token cached: {masked}{C.R}")
    else:
        print(f"{C.YELLOW}No token cached.{C.R}")
        print(f"Run 'token set' to configure.")


def _cmd_token_set(args) -> None:
    print(f"{C.BOLD}Enter your DeepSeek API token:{C.R}")
    print(f"{C.GRAY}(Get one at https://platform.deepseek.com/api_keys){C.R}")
    try:
        token = input("Token: ").strip()
    except (EOFError, KeyboardInterrupt):
        print()
        return
    if not token:
        print(f"{C.YELLOW}No token entered.{C.R}")
        return
    save_token(token)
    print(f"{C.GREEN}Token saved.{C.R}")


def _cmd_token_validate(args) -> None:
    token = load_cached_token()
    if not token:
        print(f"{C.YELLOW}No token cached. Run 'token set' first.{C.R}")
        sys.exit(1)
    from arxivcat.ui.tkinter_ui import _validate_token_with_details
    print(f"{C.BLUE}Validating token...{C.R}")
    ok, msg = _validate_token_with_details(token)
    if ok:
        print(f"{C.GREEN}Token is valid.{C.R}")
    else:
        print(f"{C.RED}Token validation failed: {msg}{C.R}")


# ═══════════════════════════════════════════════════════════════
# Main
# ═══════════════════════════════════════════════════════════════

def main():
    # Ensure stdout supports Unicode (paper content may contain special chars)
    if hasattr(sys.stdout, 'reconfigure'):
        sys.stdout.reconfigure(encoding='utf-8', errors='replace')
    if hasattr(sys.stderr, 'reconfigure'):
        sys.stderr.reconfigure(encoding='utf-8', errors='replace')

    parser = argparse.ArgumentParser(
        description="ArxivCat: download, extract, and chat with arXiv papers",
    )
    parser.add_argument(
        "--workspace", "-w",
        help="Workspace path (overrides cached)",
    )
    sub = parser.add_subparsers(dest="command", help="Available commands")

    # workspace
    wsp = sub.add_parser("workspace", help="Workspace management")
    wsp.add_argument("--workspace", "-w", help="Workspace path (overrides cached)")
    wsp_sub = wsp.add_subparsers(dest="subcommand")
    wsp_open = wsp_sub.add_parser("open", help="Open a workspace folder")
    wsp_open.add_argument("path", help="Path to workspace folder")
    wsp_scan = wsp_sub.add_parser("scan", help="Scan workspace for PDFs with arXiv IDs")

    # paper
    pp = sub.add_parser("paper", help="Paper operations")
    pp.add_argument("--workspace", "-w", help="Workspace path (overrides cached)")
    pp_sub = pp.add_subparsers(dest="subcommand")
    pp_list = pp_sub.add_parser("list", help="List papers in workspace")
    pp_dl = pp_sub.add_parser("download", help="Download a paper")
    pp_dl.add_argument("id_or_url", help="arXiv ID or URL")
    pp_dla = pp_sub.add_parser("download-all", help="Download all pending papers")
    pp_pv = pp_sub.add_parser("preview", help="Preview a paper")
    pp_pv.add_argument("id_or_url", help="arXiv ID or URL")
    pp_pv.add_argument("--view", "-v", choices=["body", "appendix", "note", "description"],
                        default="body", help="Which view to show")
    pp_note = pp_sub.add_parser("note", help="View or edit paper note")
    pp_note.add_argument("id_or_url", help="arXiv ID or URL")
    pp_note.add_argument("text", nargs="*", help="Text to write to note (if provided, overwrites)")
    pp_note.add_argument("--edit", "-e", action="store_true", help="Open note in editor")
    pp_strip = pp_sub.add_parser("strip", help="Strip comments from body.tex")
    pp_strip.add_argument("id_or_url", help="arXiv ID or URL")
    pp_open = pp_sub.add_parser("open", help="Open paper folder in file manager")
    pp_open.add_argument("id_or_url", help="arXiv ID or URL")
    pp_pdf = pp_sub.add_parser("pdf", help="Open PDF in browser")
    pp_pdf.add_argument("id_or_url", help="arXiv ID or URL")
    pp_info = pp_sub.add_parser("info", help="Show paper details")
    pp_info.add_argument("id_or_url", help="arXiv ID or URL")

    # chat
    ch = sub.add_parser("chat", help="Chat with papers")
    ch.add_argument("--workspace", "-w", help="Workspace path (overrides cached)")
    ch_sub = ch.add_subparsers(dest="subcommand")
    ch_side = ch_sub.add_parser("side", help="Side chat (scoped to one paper)")
    ch_side.add_argument("id_or_url", help="arXiv ID or URL")
    ch_global = ch_sub.add_parser("global", help="Global chat (all papers in workspace)")

    # token
    tk = sub.add_parser("token", help="API token management")
    tk.add_argument("--workspace", "-w", help="Workspace path (overrides cached)")
    tk_sub = tk.add_subparsers(dest="subcommand")
    tk_status = tk_sub.add_parser("status", help="Show token status")
    tk_set = tk_sub.add_parser("set", help="Set API token")
    tk_val = tk_sub.add_parser("validate", help="Validate cached token")

    args = parser.parse_args()

    if args.command is None:
        parser.print_help()
        return

    cmd_map = {
        ("workspace", "open"): _cmd_workspace_open,
        ("workspace", "scan"): _cmd_workspace_scan,
        ("paper", "list"): _cmd_paper_list,
        ("paper", "download"): _cmd_paper_download,
        ("paper", "download-all"): _cmd_paper_download_all,
        ("paper", "preview"): _cmd_paper_preview,
        ("paper", "note"): _cmd_paper_note,
        ("paper", "strip"): _cmd_paper_strip,
        ("paper", "open"): _cmd_paper_open,
        ("paper", "pdf"): _cmd_paper_pdf,
        ("paper", "info"): _cmd_paper_info,
        ("chat", "side"): _cmd_chat_side,
        ("chat", "global"): _cmd_chat_global,
        ("token", "status"): _cmd_token_status,
        ("token", "set"): _cmd_token_set,
        ("token", "validate"): _cmd_token_validate,
    }

    handler = cmd_map.get((args.command, getattr(args, 'subcommand', None)))
    if handler:
        handler(args)
    else:
        print(f"{C.RED}Unknown command: {args.command} {getattr(args, 'subcommand', '')}{C.R}")
        parser.print_help()


if __name__ == "__main__":
    main()
