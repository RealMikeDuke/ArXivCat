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

from arxivcat.presenter import Presenter, VERSION, AUTHOR, load_cached_token, save_token, save_model_preference, load_model_preference, load_workspace_path

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


def _show_token_input_dialog(parent: tk.Tk) -> str | None:
    """Show a popup dialog to input DeepSeek API token."""
    dialog = tk.Toplevel(parent)
    dialog.title("DeepSeek API Token")
    dialog.geometry("600x450")
    dialog.configure(bg=BG)
    dialog.transient(parent)
    dialog.grab_set()
    
    # Center the dialog
    dialog.update_idletasks()
    x = parent.winfo_x() + (parent.winfo_width() - dialog.winfo_width()) // 2
    y = parent.winfo_y() + (parent.winfo_height() - dialog.winfo_height()) // 2
    dialog.geometry(f"+{x}+{y}")
    
    tk.Label(dialog, text="Enter DeepSeek API Token:", bg=BG, fg=TEXT,
             font=("Consolas", 10)).pack(pady=(15, 10))
    
    token_var = tk.StringVar()
    entry = tk.Entry(dialog, textvariable=token_var, bg=PANEL, fg=TEXT,
                     font=("Consolas", 9), width=50, show="*")
    entry.pack(pady=5, padx=20)
    entry.focus()
    
    status_var = tk.StringVar(value="")
    status_label = tk.Label(dialog, textvariable=status_var, bg=BG, fg=MUTED,
                           font=("Consolas", 8))
    status_label.pack(pady=5)
    
    # Log section
    log_frame = tk.Frame(dialog, bg=BG)
    log_frame.pack(fill="both", expand=True, padx=20, pady=10)
    
    log_text = tk.Text(log_frame, bg=BTN, fg=TEXT, font=("Consolas", 8),
                      height=8, state="disabled", relief="flat")
    log_text.pack(fill="both", expand=True)
    
    def add_log(msg: str, color: str = TEXT):
        log_text.config(state="normal")
        log_text.insert("end", msg + "\n", (color,))
        log_text.see("end")
        log_text.config(state="disabled")
    
    log_text.tag_config("SUCCESS", foreground=SUCCESS)
    log_text.tag_config("ERROR", foreground=ERROR)
    log_text.tag_config("INFO", foreground=ACCENT)
    
    result = [None]
    
    def on_validate():
        token = token_var.get().strip()
        if not token:
            status_var.set("Token cannot be empty")
            add_log("[ERROR] Token cannot be empty", "ERROR")
            return
        
        add_log(f"[INFO] Validating token...", "INFO")
        status_var.set("Validating...")
        dialog.update()
        
        success, msg = _validate_token_with_details(token)
        if success:
            add_log(f"[OK] {msg}", "SUCCESS")
            status_var.set("Token valid!")
            result[0] = token
        else:
            add_log(f"[ERROR] {msg}", "ERROR")
            status_var.set("Invalid token")
    
    def on_ok():
        if result[0] is None:
            status_var.set("Please validate token first")
            add_log("[ERROR] Please validate token first", "ERROR")
            return
        dialog.destroy()
    
    def on_cancel():
        dialog.destroy()
    
    btn_frame = tk.Frame(dialog, bg=BG)
    btn_frame.pack(pady=15)
    
    tk.Button(btn_frame, text="Validate", command=on_validate, bg=ACCENT, fg=BG,
              font=("Consolas", 9), padx=20).pack(side="left", padx=5)
    tk.Button(btn_frame, text="OK", command=on_ok, bg=SUCCESS, fg=BG,
              font=("Consolas", 9), padx=20).pack(side="left", padx=5)
    tk.Button(btn_frame, text="Cancel", command=on_cancel, bg=BTN, fg=MUTED,
              font=("Consolas", 9), padx=20).pack(side="left", padx=5)
    
    entry.bind("<Return>", lambda e: on_validate())
    dialog.wait_window()
    
    return result[0]


def _validate_token(token: str) -> bool:
    """Validate DeepSeek API token by sending a test message."""
    success, _ = _validate_token_with_details(token)
    return success


def _validate_token_with_details(token: str) -> tuple[bool, str]:
    """Validate DeepSeek API token with detailed error messages."""
    try:
        import time
        start_time = time.time()
        client = OpenAI(api_key=token, base_url="https://api.deepseek.com")
        response = client.chat.completions.create(
            model="deepseek-v4-flash",
            messages=[{"role": "user", "content": "你好！"}],
            max_tokens=10,
        )
        elapsed = (time.time() - start_time) * 1000
        return True, f"Token validated successfully (response time: {elapsed:.0f}ms)"
    except Exception as e:
        error_msg = str(e)
        if "401" in error_msg or "Unauthorized" in error_msg:
            return False, "Authentication failed: Invalid token"
        elif "429" in error_msg:
            return False, "Rate limit exceeded. Please try again later"
        elif "timeout" in error_msg.lower():
            return False, "Request timed out. Check your network connection"
        else:
            return False, f"Validation failed: {error_msg}"


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

    CHAT_MODELS = {
        "Flash": "deepseek-v4-flash",
        "Pro": "deepseek-v4-pro"
    }

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
        self._chat_model = load_model_preference()  # Load from cache
        self._build()
        self._presenter = Presenter(self)
        self._check_and_prompt_token()
        self._init_workspace()

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
                      self._open_btn, self._pdf_btn, self._strip_btn)
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

    def set_url_input(self, url: str) -> None:
        self._root.after(0, lambda: (
            self._url_var.set(url),
            self._url_entry.config(fg=TEXT)
        ))

    def set_paper_list(self, papers: list[dict]) -> None:
        def _do():
            self._paper_list.delete(0, "end")
            self._paper_data = papers
            for i, paper in enumerate(papers):
                prefix = "" if paper.get("has_body", True) else "⏳ "
                display_text = f"{prefix}{paper['arxiv_id']}\n{paper['title']}"
                self._paper_list.insert("end", display_text)
                if not paper.get("has_body", True):
                    self._paper_list.itemconfig(i, fg=MUTED)
        self._root.after(0, _do)

    def set_title(self, title: str) -> None:
        self._root.after(0, lambda: self._root.title(title))

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

    def _on_paper_click(self, event):
        """Handle click on paper list item."""
        selection = self._paper_list.curselection()
        if selection:
            index = selection[0]
            if index < len(self._paper_data):
                paper = self._paper_data[index]
                self._presenter.load_paper(paper["folder_name"])

    def _on_paper_hover(self, event):
        """Show tooltip with full title on hover."""
        index = self._paper_list.nearest(event.y)
        if index < 0 or index >= len(self._paper_data):
            self._on_paper_leave(None)
            return
        paper = self._paper_data[index]
        tip_text = f"{paper['arxiv_id']}\n{paper['title']}"

        if self._paper_tooltip and self._paper_tooltip.winfo_exists():
            self._paper_tooltip_label.config(text=tip_text)
        else:
            tw = tk.Toplevel(self._root)
            tw.wm_overrideredirect(True)
            tw.configure(bg=MUTED)
            lbl = tk.Label(tw, text=tip_text, bg=BTN, fg=TEXT,
                           font=("Consolas", 9), justify="left",
                           padx=8, pady=4, wraplength=360)
            lbl.pack(padx=1, pady=1)
            self._paper_tooltip = tw
            self._paper_tooltip_label = lbl

        x = self._paper_list.winfo_rootx() + self._paper_list.winfo_width() + 4
        y = self._paper_list.winfo_rooty() + event.y
        self._paper_tooltip.wm_geometry(f"+{x}+{y}")

    def _on_paper_leave(self, event):
        """Destroy tooltip when mouse leaves the list."""
        if self._paper_tooltip and self._paper_tooltip.winfo_exists():
            self._paper_tooltip.destroy()
        self._paper_tooltip = None

    def _init_workspace(self):
        """Check for saved workspace; prompt folder picker if none."""
        saved = load_workspace_path()
        if saved and os.path.isdir(saved):
            self._presenter.open_workspace(saved)
        else:
            self._root.after(200, self._prompt_workspace)

    def _prompt_workspace(self):
        """Show folder picker dialog."""
        from tkinter import filedialog
        path = filedialog.askdirectory(
            title="Select Workspace Folder",
            parent=self._root,
        )
        if path:
            self._presenter.open_workspace(path)
        else:
            self.set_preview(
                "No workspace selected.\n\n"
                "Click \"Open Folder\" in the left panel to choose a workspace.",
                ""
            )

    def _on_open_folder(self):
        """Handle Open Folder button click."""
        self._prompt_workspace()

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
            api_key = load_cached_token()
            if not api_key:
                raise ValueError("Missing DeepSeek API token. Please restart the app and enter your token.")
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
                    model=self.CHAT_MODELS[self._chat_model],
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
                    self._set_chat_status(self.CHAT_MODELS[self._chat_model], SUCCESS)
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

    def _check_and_prompt_token(self):
        """Check for cached token and prompt if missing."""
        token = load_cached_token()
        if not token:
            self._root.after(100, self._prompt_for_token)
        else:
            # Validate cached token
            if not _validate_token(token):
                self._root.after(100, self._prompt_for_token)

    def _prompt_for_token(self):
        """Show token input dialog and validate."""
        token = _show_token_input_dialog(self._root)
        if token:
            save_token(token)
            self.add_log("[OK] Token saved to cache")

    def _on_update_token(self):
        """Handle Update Token button click."""
        self._prompt_for_token()

    def _on_model_change(self, value):
        """Handle model selection change."""
        self._chat_model = value
        self._save_model_preference()
        self._chat_model_label.config(text=self.CHAT_MODELS[value])
        self._set_chat_status(f"Model: {self.CHAT_MODELS[value]}", SUCCESS)

    def _save_model_preference(self):
        """Save model preference to cache."""
        save_model_preference(self._chat_model)

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

        split = tk.PanedWindow(outer, orient="horizontal", bg=BG,
                                sashwidth=8, sashrelief="flat",
                                opaqueresize=True)
        split.pack(fill="both", expand=True)

        paper_list_col = tk.Frame(split, bg=PANEL, padx=10, pady=12)

        main_col = tk.Frame(split, bg=BG)
        self._main_col = main_col

        chat_col = tk.Frame(split, bg=PANEL, padx=12, pady=12)

        split.add(paper_list_col, minsize=160, width=240)
        split.add(main_col, minsize=300, stretch="always")
        split.add(chat_col, minsize=240, width=320)

        # Paper list panel content
        paper_header = tk.Frame(paper_list_col, bg=PANEL)
        paper_header.pack(fill="x", pady=(0, 10))
        tk.Label(paper_header, text="Papers", bg=PANEL, fg=ACCENT,
                 font=("Consolas", 13, "bold")).pack(side="left")
        tk.Button(
            paper_header, text="Open Folder",
            command=self._on_open_folder,
            bg=BTN, fg=TEXT, activebackground=BTN_HOV, activeforeground=TEXT,
            font=("Consolas", 8), relief="flat", padx=6, pady=2,
            cursor="hand2",
        ).pack(side="right")
        tk.Button(
            paper_header, text="Scan PDFs",
            command=lambda: self._presenter.scan_workspace_pdfs(),
            bg=BTN, fg=TEXT, activebackground=BTN_HOV, activeforeground=TEXT,
            font=("Consolas", 8), relief="flat", padx=6, pady=2,
            cursor="hand2",
        ).pack(side="right", padx=(0, 4))

        paper_btn_row = tk.Frame(paper_list_col, bg=PANEL)
        paper_btn_row.pack(fill="x", pady=(0, 6))
        tk.Button(
            paper_btn_row, text="Download All",
            command=lambda: self._presenter.download_all_pending(),
            bg=ACCENT, fg=BG, activebackground=RUN_HOV, activeforeground=BG,
            font=("Consolas", 9, "bold"), relief="flat", padx=10, pady=3,
            cursor="hand2",
        ).pack(fill="x")

        paper_list_scroll = tk.Scrollbar(paper_list_col, bg=PANEL,
                                         troughcolor=PANEL, activebackground=BTN_HOV)
        paper_list_scroll.pack(side="right", fill="y")

        self._paper_list = tk.Listbox(
            paper_list_col,
            bg=PANEL,
            fg=TEXT,
            selectbackground=ACCENT,
            selectforeground=BG,
            font=("Consolas", 9),
            relief="flat",
            yscrollcommand=paper_list_scroll.set,
            bd=0,
            highlightthickness=0,
        )
        self._paper_list.pack(fill="both", expand=True)
        paper_list_scroll.config(command=self._paper_list.yview)
        self._paper_list.bind("<Button-1>", self._on_paper_click)
        self._paper_list.bind("<Motion>", self._on_paper_hover)
        self._paper_list.bind("<Leave>", self._on_paper_leave)
        self._paper_data = []  # Store paper metadata
        self._paper_tooltip = None

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
        self._url_entry = url_entry

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
        self._pdf_btn = _FlatButton(btn_row, "Open PDF", lambda: self._presenter.open_pdf_in_browser())
        self._strip_btn = _FlatButton(btn_row, "Strip Comments", lambda: self._presenter.strip_comments())

        for b in (self._copy_btn, self._overwrite_btn, self._open_btn, self._pdf_btn, self._strip_btn):
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
        self._chat_model_label = tk.Label(chat_col, text=self.CHAT_MODELS[self._chat_model], bg=PANEL, fg=MUTED,
                 font=("Consolas", 8))
        self._chat_model_label.pack(anchor="w", pady=(2, 6))
        
        self._deep_thinking_enabled = tk.BooleanVar(value=self._deep_thinking)
        
        # Container for horizontal layout
        controls_row = tk.Frame(chat_col, bg=PANEL)
        controls_row.pack(fill="x", pady=(0, 10))
        
        tk.Checkbutton(
            controls_row,
            text="Deep Thinking",
            variable=self._deep_thinking_enabled,
            bg=PANEL,
            fg=MUTED,
            activebackground=PANEL,
            activeforeground=TEXT,
            selectcolor=BG,
            font=("Consolas", 9),
        ).pack(side="left", padx=(0, 10))
        
        # Model selector
        self._chat_model_var = tk.StringVar(value=self._chat_model)
        model_dropdown = tk.OptionMenu(
            controls_row,
            self._chat_model_var,
            *self.CHAT_MODELS.keys(),
            command=self._on_model_change
        )
        model_dropdown.config(bg=PANEL, fg=TEXT, font=("Consolas", 9), activebackground=ACCENT, activeforeground=BG, relief="solid", bd=1, highlightthickness=0)
        model_dropdown["menu"].config(bg=BG, fg=TEXT, font=("Consolas", 9), activebackground=ACCENT, activeforeground=BG)
        model_dropdown.pack(side="left", padx=(0, 10))
        
        tk.Button(
            controls_row,
            text="Update Token",
            command=self._on_update_token,
            bg=BTN,
            fg=TEXT,
            activebackground=BTN_HOV,
            activeforeground=TEXT,
            font=("Consolas", 9),
            relief="flat",
            padx=10,
            pady=2,
        ).pack(side="left")

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
