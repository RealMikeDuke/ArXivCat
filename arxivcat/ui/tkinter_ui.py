"""Tkinter UI backend – dark Catppuccin theme, mirrors the Flet layout."""
from __future__ import annotations

import ctypes
import os
import sys
import threading
import tkinter as tk
from tkinter import ttk
from typing import Callable

from openai import OpenAI

from arxivcat.presenter import Presenter, VERSION, AUTHOR

# ── palette (Catppuccin Mocha) ────────────────────────────────
BG      = "#1e1e2e"
PANEL   = "#2a2a3e"
ACCENT  = "#89b4fa"
TEXT    = "#cdd6f4"
MUTED   = "#6c7086"
SUCCESS = "#a6e3a1"
ERROR   = "#f38ba8"
BTN     = "#313244"
BTN_HOV = "#45475a"
RUN_HOV = "#74c7ec"


def _enable_windows_dpi(root: tk.Tk) -> None:
    if sys.platform != "win32":
        return

    try:
        ctypes.windll.shcore.SetProcessDpiAwareness(1)
    except Exception:
        try:
            ctypes.windll.user32.SetProcessDPIAware()
        except Exception:
            pass

    try:
        dpi = ctypes.windll.user32.GetDpiForWindow(root.winfo_id())
        if dpi > 0:
            root.tk.call("tk", "scaling", dpi / 72.0)
    except Exception:
        pass


class _FlatButton(tk.Label):
    """Clickable label styled like the Flet container buttons."""

    def __init__(self, parent, text: str, command: Callable, **kw):
        super().__init__(
            parent, text=text,
            bg=BTN, fg=MUTED,
            font=("Consolas", 9),
            padx=14, pady=6,
            cursor="hand2", **kw,
        )
        self._command = command
        self._enabled = False
        self.bind("<Enter>", self._on_enter)
        self.bind("<Leave>", self._on_leave)
        self.bind("<Button-1>", self._on_click)

    def set_enabled(self, enabled: bool):
        self._enabled = enabled
        self.config(fg=TEXT if enabled else MUTED)

    def _on_enter(self, _):
        if self._enabled:
            self.config(bg=BTN_HOV)

    def _on_leave(self, _):
        self.config(bg=BTN)

    def _on_click(self, _):
        if self._enabled:
            self._command()


class _RunButton(tk.Label):
    """Accent-coloured Run button."""

    def __init__(self, parent, command: Callable):
        super().__init__(
            parent, text="Run",
            bg=ACCENT, fg=BG,
            font=("Consolas", 10, "bold"),
            padx=20, pady=8,
            cursor="hand2",
        )
        self._command = command
        self._busy = False
        self.bind("<Enter>", self._on_enter)
        self.bind("<Leave>", self._on_leave)
        self.bind("<Button-1>", self._on_click)

    def set_busy(self, busy: bool):
        self._busy = busy
        self.config(bg=MUTED if busy else ACCENT,
                    cursor="watch" if busy else "hand2")

    def _on_enter(self, _):
        if not self._busy:
            self.config(bg=RUN_HOV)

    def _on_leave(self, _):
        self.config(bg=MUTED if self._busy else ACCENT)

    def _on_click(self, _):
        if not self._busy:
            self._command()


class TkApp:
    """Tkinter implementation of UIProtocol."""

    CHAT_MODEL = "deepseek-v4-flash"

    def __init__(self):
        self._toast_after = None
        self._log_visible = False
        self._chat_client = None
        self._chat_after = None
        self._chat_history: list[tuple[str, str]] = []
        self._chat_placeholder = "Ask something about the current paper…"
        self._chat_output_buffer = ""
        self._deep_thinking = True  # Python bool for storage
        self._chat_cancelled = False
        self._chat_metrics = {
            "ttft": None,  # time to first token
            "tokens_per_sec": None,
            "prompt_tokens": None,
            "completion_tokens": None,
        }
        self._build()
        self._presenter = Presenter(self)

    # ── UIProtocol ────────────────────────────────────────────

    def add_log(self, msg: str) -> None:
        color = SUCCESS if msg.startswith("[OK]") else ERROR if msg.startswith("[ERROR]") else TEXT
        self._log_text.config(state="normal")
        self._log_text.insert("end", msg + "\n", (color,))
        self._log_text.see("end")
        self._log_text.config(state="disabled")

    def set_mini_status(self, msg: str, level: str = "info") -> None:
        color = {"ok": SUCCESS, "error": ERROR}.get(level, MUTED)
        self._root.after(0, lambda: (
            self._mini_var.set(msg),
            self._mini_lbl.config(fg=color),
        ))

    def set_preview(self, content: str, label: str) -> None:
        def _do():
            self._preview.config(state="normal")
            self._preview.delete("1.0", "end")
            self._preview.insert("1.0", content)
            if label:
                self._view_label_var.set(label)
            self._update_word_count()
        self._root.after(0, _do)

    def set_buttons_enabled(self, enabled: bool) -> None:
        self._root.after(0, lambda: [
            b.set_enabled(enabled)
            for b in (self._copy_btn, self._overwrite_btn,
                      self._open_btn, self._strip_btn)
        ])

    def set_run_busy(self, busy: bool) -> None:
        self._root.after(0, lambda: self._run_btn.set_busy(busy))

    def show_toast(self, msg: str, duration_ms: int = 2000) -> None:
        def _do():
            self._toast_var.set(msg)
            if self._toast_after:
                self._root.after_cancel(self._toast_after)
            self._toast_after = self._root.after(
                duration_ms, lambda: self._toast_var.set(""))
        self._root.after(0, _do)

    def get_url_input(self) -> str:
        v = self._url_var.get().strip()
        return "" if v == self._placeholder else v

    def get_view_mode(self) -> str:
        return self._view_var.get()

    def get_preview_text(self) -> str:
        return self._preview.get("1.0", "end-1c")

    def clear_log(self) -> None:
        self._root.after(0, lambda: (
            self._log_text.config(state="normal"),
            self._log_text.delete("1.0", "end"),
            self._log_text.config(state="disabled"),
        ))

    def run(self) -> None:
        self._root.mainloop()

    # ── internal helpers ──────────────────────────────────────

    def _update_word_count(self):
        content = self.get_preview_text()
        self._wc_var.set(f"{len(content.split())} words  {len(content)} chars")

    def _toggle_log(self):
        if self._log_visible:
            self._log_frame.pack_forget()
            self._log_visible = False
        else:
            self._log_frame.pack(fill="x", side="top", after=self._ctrl_row, pady=(0, 4))
            self._log_visible = True

    def _on_view_change(self, *_):
        self._presenter.switch_view()

    def _on_copy(self):
        self._root.clipboard_clear()
        self._root.clipboard_append(self.get_preview_text())
        self.show_toast(f"Copied {self.get_view_mode()}.tex!")

    def _set_chat_status(self, msg: str, color: str = MUTED):
        self._root.after(0, lambda: (
            self._chat_status_var.set(msg),
            self._chat_status_lbl.config(fg=color),
        ))

    def _set_chat_busy(self, busy: bool):
        def _do():
            self._chat_busy = busy
            self._chat_send_btn.set_enabled(not busy)
            self._chat_reset_btn.set_enabled(not busy)
            self._chat_stop_btn.set_enabled(busy)
            self._chat_input.config(state="disabled" if busy else "normal")
        self._root.after(0, _do)

    def _append_chat_message(self, speaker: str, content: str, color: str, append: bool = False):
        def _do():
            self._chat_output.tag_config(f"{speaker}_tag", foreground=color, font=("Consolas", 10, "bold"))
            self._chat_output.tag_config(f"{speaker}_body", foreground=TEXT)
            self._chat_output.config(state="normal")
            if not append:
                self._chat_output.insert("end", f"{speaker}: ", (f"{speaker}_tag",))
                if content:
                    self._chat_output.insert("end", content + "\n\n", (f"{speaker}_body",))
                else:
                    self._chat_output.insert("end", "\n\n", (f"{speaker}_body",))
            else:
                self._chat_output.insert("end", content, (f"{speaker}_body",))
            self._chat_output.see("end")
            self._chat_output.config(state="disabled")
        self._root.after(0, _do)

    def _get_chat_input(self) -> str:
        value = self._chat_input.get("1.0", "end-1c").strip()
        return "" if value == self._chat_placeholder else value

    def _clear_chat_input(self):
        self._chat_input.delete("1.0", "end")
        self._chat_input.insert("1.0", self._chat_placeholder)
        self._chat_input.config(fg=MUTED)

    def _chat_focus_in(self, _):
        if self._get_chat_input() == "" and self._chat_input.get("1.0", "end-1c").strip() == self._chat_placeholder:
            self._chat_input.delete("1.0", "end")
            self._chat_input.config(fg=TEXT)

    def _chat_focus_out(self, _):
        if not self._chat_input.get("1.0", "end-1c").strip():
            self._clear_chat_input()

    def _ensure_chat_client(self):
        if self._chat_client is None:
            api_key = os.getenv("DEEPSEEK_API_KEY")
            if not api_key:
                raise ValueError("Missing DEEPSEEK_API_KEY")
            self._chat_client = OpenAI(api_key=api_key, base_url="https://api.deepseek.com")
        return self._chat_client

    def _on_chat_send(self):
        if getattr(self, "_chat_busy", False):
            return
        prompt = self._get_chat_input()
        if not prompt:
            self._set_chat_status("empty prompt", ERROR)
            return

        preview_text = self.get_preview_text().strip()
        view_name = self.get_view_mode()
        self._chat_history.append(("you", prompt))
        self._append_chat_message("you", prompt, ACCENT)
        self._chat_input.delete("1.0", "end")
        self._chat_input.config(fg=TEXT)
        self._set_chat_busy(True)
        self._set_chat_status("thinking...", MUTED)

        def _work():
            try:
                client = self._ensure_chat_client()
                context = preview_text[:12000] if preview_text else "(no preview loaded)"
                
                # Build messages in standard format for cache hit
                messages = [
                    {
                        "role": "system",
                        "content": "You are a compact in-app chat assistant inside an arXiv paper extraction tool. Maintain conversation continuity. If the user asks a general question, answer it normally. If useful, use the paper preview as extra context."
                    },
                    {
                        "role": "user",
                        "content": f"Current view: {view_name}\n\nPaper content snippet:\n{context}"
                    }
                ]
                
                # Add conversation history
                for speaker, message in self._chat_history[-12:]:
                    role = "user" if speaker == "you" else "assistant"
                    messages.append({"role": role, "content": message})
                
                # Prepare extra parameters for deep thinking
                extra_params = {}
                if self._deep_thinking_enabled.get():
                    extra_params["extra_body"] = {"thinking": {"type": "enabled"}}
                    extra_params["reasoning_effort"] = "high"
                
                # Stream the response
                import time
                self._chat_output_buffer = ""
                first_chunk = True
                start_time = time.time()
                first_token_time = None
                token_count = 0
                
                # Estimate input tokens (rough approximation)
                total_chars = sum(len(m["content"]) for m in messages)
                estimated_input_tokens = total_chars // 3
                
                response = client.chat.completions.create(
                    model=self.CHAT_MODEL,
                    messages=messages,
                    stream=True,
                    **extra_params
                )
                
                for chunk in response:
                    if self._chat_cancelled:
                        break
                    if chunk.choices[0].delta.content:
                        content = chunk.choices[0].delta.content
                        self._chat_output_buffer += content
                        token_count += 1
                        
                        if first_token_time is None:
                            first_token_time = time.time()
                            self._chat_metrics["ttft"] = (first_token_time - start_time) * 1000  # ms
                        
                        if first_chunk:
                            self._append_chat_message("deepseek", content, SUCCESS, append=False)
                            first_chunk = False
                        else:
                            self._append_chat_message("deepseek", content, SUCCESS, append=True)
                
                end_time = time.time()
                
                # Calculate tokens per second
                if token_count > 0 and end_time > start_time:
                    self._chat_metrics["tokens_per_sec"] = token_count / (end_time - start_time)
                
                # Set estimated token counts
                self._chat_metrics["prompt_tokens"] = estimated_input_tokens
                self._chat_metrics["completion_tokens"] = token_count
                
                # Add final newlines if not cancelled and got content
                if not self._chat_cancelled and self._chat_output_buffer.strip():
                    self._append_chat_message("deepseek", "\n\n", SUCCESS, append=True)
                
                if self._chat_output_buffer.strip():
                    self._chat_history.append(("deepseek", self._chat_output_buffer.strip()))
                
                if self._chat_cancelled:
                    self._set_chat_status("cancelled", MUTED)
                else:
                    # Update status with model name only, metrics shown separately
                    self._set_chat_status(self.CHAT_MODEL, SUCCESS)
                    self._format_metrics()
            except Exception as exc:
                if not self._chat_cancelled:
                    self._append_chat_message("system", str(exc), ERROR)
                    self._set_chat_status("chat error", ERROR)
            finally:
                self._chat_cancelled = False
                self._set_chat_busy(False)
                self._root.after(0, self._clear_chat_input)

        threading.Thread(target=_work, daemon=True).start()

    def _on_chat_reset(self):
        if getattr(self, "_chat_busy", False):
            return
        self._chat_history.clear()
        self._chat_output.config(state="normal")
        self._chat_output.delete("1.0", "end")
        self._chat_output.config(state="disabled")
        self._clear_chat_input()
        self._set_chat_status("reset", MUTED)

    def _on_chat_stop(self):
        if getattr(self, "_chat_busy", False):
            self._chat_cancelled = True
            self._set_chat_status("stopping...", MUTED)

    def _format_metrics(self) -> str:
        parts = []
        if self._chat_metrics["ttft"]:
            parts.append(f"TTFT: {self._chat_metrics['ttft']:.0f}ms")
        if self._chat_metrics["tokens_per_sec"]:
            parts.append(f"{self._chat_metrics['tokens_per_sec']:.1f} tok/s")
        if self._chat_metrics["prompt_tokens"]:
            parts.append(f"in: {self._chat_metrics['prompt_tokens']}")
        if self._chat_metrics["completion_tokens"]:
            parts.append(f"out: {self._chat_metrics['completion_tokens']}")
        
        metrics_str = " | ".join(parts) if parts else ""
        self._chat_metrics_var.set(metrics_str)
        return metrics_str

    # ── build ─────────────────────────────────────────────────

    def _build(self):
        root = tk.Tk()
        _enable_windows_dpi(root)
        root.title("ArxivCat")
        root.geometry("1180x700")
        root.minsize(900, 560)
        root.configure(bg=BG)
        self._root = root
        self._chat_busy = False

        outer = tk.Frame(root, bg=BG)
        outer.pack(fill="both", expand=True, padx=24, pady=(18, 14))

        split = tk.Frame(outer, bg=BG)
        split.pack(fill="both", expand=True)

        main_col = tk.Frame(split, bg=BG)
        main_col.pack(side="left", fill="both", expand=True)
        self._main_col = main_col

        gutter = tk.Frame(split, bg=BG, width=14)
        gutter.pack(side="left", fill="y")

        chat_col = tk.Frame(split, bg=PANEL, width=320, padx=12, pady=12)
        chat_col.pack(side="right", fill="y")
        chat_col.pack_propagate(False)

        title_row = tk.Frame(main_col, bg=BG)
        title_row.pack(fill="x")
        tk.Label(title_row, text="ArxivCat", bg=BG, fg=ACCENT,
                 font=("Consolas", 16, "bold")).pack(side="left")
        tk.Label(title_row, text=f"  {AUTHOR}  {VERSION}",
                 bg=BG, fg=MUTED, font=("Consolas", 9)).pack(side="left", pady=(4, 0))

        tk.Frame(main_col, bg=BG, height=10).pack(fill="x")

        input_row = tk.Frame(main_col, bg=BG)
        input_row.pack(fill="x")
        input_row.columnconfigure(0, weight=1)

        self._placeholder = "paste an arXiv URL or ID"
        self._url_var = tk.StringVar()
        url_entry = tk.Entry(
            input_row,
            textvariable=self._url_var,
            bg=PANEL, fg=MUTED,
            insertbackground=ACCENT,
            relief="flat",
            font=("Consolas", 11),
            bd=0,
        )
        url_entry.grid(row=0, column=0, sticky="ew", ipady=8, padx=(0, 8))
        url_entry.insert(0, self._placeholder)

        def _focus_in(e):
            if url_entry.get() == self._placeholder:
                url_entry.delete(0, "end")
                url_entry.config(fg=TEXT)

        def _focus_out(e):
            if not url_entry.get():
                url_entry.insert(0, self._placeholder)
                url_entry.config(fg=MUTED)

        url_entry.bind("<FocusIn>", _focus_in)
        url_entry.bind("<FocusOut>", _focus_out)
        url_entry.bind("<Return>", lambda e: self._presenter.run_fetch())

        self._run_btn = _RunButton(input_row, command=lambda: self._presenter.run_fetch())
        self._run_btn.grid(row=0, column=1)

        tk.Frame(main_col, bg=BG, height=6).pack(fill="x")

        ctrl_row = tk.Frame(main_col, bg=BG)
        ctrl_row.pack(fill="x")
        self._ctrl_row = ctrl_row

        self._view_var = tk.StringVar(value="body")
        style = ttk.Style()
        style.theme_use("clam")
        style.configure(
            "AC.TCombobox",
            fieldbackground=PANEL,
            background=PANEL,
            foreground=TEXT,
            selectbackground=PANEL,
            selectforeground=TEXT,
            arrowcolor=MUTED,
            bordercolor=MUTED,
            lightcolor=PANEL,
            darkcolor=PANEL,
        )
        style.map(
            "AC.TCombobox",
            fieldbackground=[("readonly", PANEL)],
            foreground=[("readonly", TEXT)],
        )
        ttk.Combobox(
            ctrl_row,
            textvariable=self._view_var,
            values=["body", "appendix"],
            state="readonly",
            width=10,
            font=("Consolas", 10),
            style="AC.TCombobox",
        ).pack(side="left")
        self._view_var.trace_add("write", self._on_view_change)

        show_log_var = tk.BooleanVar(value=False)
        tk.Checkbutton(
            ctrl_row,
            text="show log",
            variable=show_log_var,
            bg=BG,
            fg=MUTED,
            activebackground=BG,
            activeforeground=TEXT,
            selectcolor=BG,
            font=("Consolas", 9),
            command=self._toggle_log,
        ).pack(side="left", padx=(10, 0))

        self._mini_var = tk.StringVar()
        self._mini_lbl = tk.Label(
            ctrl_row,
            textvariable=self._mini_var,
            bg=BG,
            fg=MUTED,
            font=("Consolas", 10),
        )
        self._mini_lbl.pack(side="right")

        tk.Frame(main_col, bg=BG, height=4).pack(fill="x")

        log_wrap = tk.Frame(main_col, bg=PANEL, pady=4, padx=6)
        self._log_frame = log_wrap

        self._log_text = tk.Text(
            log_wrap,
            bg=PANEL,
            fg=TEXT,
            font=("Consolas", 9),
            height=10,
            relief="flat",
            state="disabled",
            wrap="word",
        )
        self._log_text.pack(fill="both", expand=True)
        self._log_text.tag_config(SUCCESS, foreground=SUCCESS)
        self._log_text.tag_config(ERROR, foreground=ERROR)
        self._log_text.tag_config(TEXT, foreground=TEXT)

        btn_row = tk.Frame(main_col, bg=BG)
        btn_row.pack(fill="x", side="bottom", pady=(8, 0))

        self._copy_btn = _FlatButton(btn_row, "Copy", self._on_copy)
        self._overwrite_btn = _FlatButton(btn_row, "Overwrite", lambda: self._presenter.overwrite_file())
        self._open_btn = _FlatButton(btn_row, "Open Folder", lambda: self._presenter.open_folder())
        self._strip_btn = _FlatButton(btn_row, "Strip Comments", lambda: self._presenter.strip_comments())

        for b in (self._copy_btn, self._overwrite_btn, self._open_btn, self._strip_btn):
            b.pack(side="left", padx=(0, 6))

        self._toast_var = tk.StringVar()
        tk.Label(btn_row, textvariable=self._toast_var,
                 bg=BG, fg=MUTED, font=("Consolas", 9)).pack(side="right")

        tk.Frame(main_col, bg=BG, height=2).pack(fill="x", side="top")
        self._preview_header = tk.Frame(main_col, bg=BG)
        self._preview_header.pack(fill="x", side="top")
        self._view_label_var = tk.StringVar(value="body.tex")
        tk.Label(self._preview_header, textvariable=self._view_label_var,
                 bg=BG, fg=MUTED, font=("Consolas", 9)).pack(side="left")
        self._wc_var = tk.StringVar()
        tk.Label(self._preview_header, textvariable=self._wc_var,
                 bg=BG, fg=MUTED, font=("Consolas", 9)).pack(side="right")

        preview_wrap = tk.Frame(main_col, bg=PANEL)
        preview_wrap.pack(fill="both", expand=True)

        preview_scroll = tk.Scrollbar(preview_wrap, bg=PANEL,
                                      troughcolor=PANEL, activebackground=BTN_HOV)
        preview_scroll.pack(side="right", fill="y")

        self._preview = tk.Text(
            preview_wrap,
            bg=PANEL,
            fg=TEXT,
            insertbackground=ACCENT,
            font=("Consolas", 10),
            relief="flat",
            wrap="word",
            yscrollcommand=preview_scroll.set,
            padx=8,
            pady=6,
            undo=True,
        )
        self._preview.pack(fill="both", expand=True)
        preview_scroll.config(command=self._preview.yview)
        self._preview.bind("<KeyRelease>", lambda e: self._update_word_count())

        tk.Label(chat_col, text="chat", bg=PANEL, fg=ACCENT,
                 font=("Consolas", 13, "bold")).pack(anchor="w")
        tk.Label(chat_col, text=self.CHAT_MODEL, bg=PANEL, fg=MUTED,
                 font=("Consolas", 8)).pack(anchor="w", pady=(2, 6))
        
        self._deep_thinking_enabled = tk.BooleanVar(value=self._deep_thinking)
        tk.Checkbutton(
            chat_col,
            text="Deep Thinking",
            variable=self._deep_thinking_enabled,
            bg=PANEL,
            fg=MUTED,
            activebackground=PANEL,
            activeforeground=TEXT,
            selectcolor=BG,
            font=("Consolas", 9),
        ).pack(anchor="w", pady=(0, 10))

        chat_bottom = tk.Frame(chat_col, bg=PANEL)
        chat_bottom.pack(fill="x", side="bottom")

        self._chat_status_var = tk.StringVar(value="idle")
        self._chat_status_lbl = tk.Label(
            chat_bottom,
            textvariable=self._chat_status_var,
            bg=PANEL,
            fg=MUTED,
            font=("Consolas", 8),
        )
        self._chat_status_lbl.pack(anchor="w", pady=(0, 2))
        
        self._chat_metrics_var = tk.StringVar(value="")
        self._chat_metrics_lbl = tk.Label(
            chat_bottom,
            textvariable=self._chat_metrics_var,
            bg=PANEL,
            fg=ACCENT,
            font=("Consolas", 8),
        )
        self._chat_metrics_lbl.pack(anchor="w", pady=(0, 4))

        self._chat_input = tk.Text(
            chat_bottom,
            height=5,
            bg=BG,
            fg=MUTED,
            insertbackground=ACCENT,
            font=("Consolas", 9),
            relief="flat",
            wrap="word",
            padx=8,
            pady=8,
        )
        self._chat_input.pack(fill="x")
        self._clear_chat_input()
        self._chat_input.bind("<FocusIn>", self._chat_focus_in)
        self._chat_input.bind("<FocusOut>", self._chat_focus_out)
        self._chat_input.bind("<Control-Return>", lambda e: (self._on_chat_send(), "break"))

        chat_btn_row = tk.Frame(chat_bottom, bg=PANEL)
        chat_btn_row.pack(fill="x", pady=(8, 0))

        self._chat_send_btn = _FlatButton(chat_btn_row, "Send", self._on_chat_send)
        self._chat_stop_btn = _FlatButton(chat_btn_row, "Stop", self._on_chat_stop)
        self._chat_reset_btn = _FlatButton(chat_btn_row, "Reset", self._on_chat_reset)
        self._chat_send_btn.pack(side="left", padx=(0, 6))
        self._chat_stop_btn.pack(side="left", padx=(0, 6))
        self._chat_reset_btn.pack(side="left")
        self._chat_send_btn.set_enabled(True)
        self._chat_stop_btn.set_enabled(False)
        self._chat_reset_btn.set_enabled(True)

        tk.Frame(chat_col, bg=PANEL, height=8).pack(fill="x", side="bottom")

        chat_output_wrap = tk.Frame(chat_col, bg=BTN)
        chat_output_wrap.pack(fill="both", expand=True)

        chat_scroll = tk.Scrollbar(chat_output_wrap, bg=BTN,
                                   troughcolor=BTN, activebackground=BTN_HOV)
        chat_scroll.pack(side="right", fill="y")

        self._chat_output = tk.Text(
            chat_output_wrap,
            bg=BTN,
            fg=TEXT,
            insertbackground=ACCENT,
            font=("Consolas", 9),
            relief="flat",
            wrap="char",
            state="disabled",
            yscrollcommand=chat_scroll.set,
            padx=8,
            pady=8,
        )
        self._chat_output.pack(fill="both", expand=True)
        chat_scroll.config(command=self._chat_output.yview)
