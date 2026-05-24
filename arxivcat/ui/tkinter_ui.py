"""Tkinter UI backend – dark Catppuccin theme, mirrors the Flet layout."""
from __future__ import annotations

import ctypes
import os
import sys
import threading
from pathlib import Path
import tkinter as tk
import tkinter.font as tkfont
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
FONT_FAMILY = "Maple Mono NF CN"


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
             font=(FONT_FAMILY, 10)).pack(pady=(15, 10))
    
    token_var = tk.StringVar()
    entry = tk.Entry(dialog, textvariable=token_var, bg=PANEL, fg=TEXT,
                     font=(FONT_FAMILY, 9), width=50, show="*")
    entry.pack(pady=5, padx=20)
    entry.focus()
    
    status_var = tk.StringVar(value="")
    status_label = tk.Label(dialog, textvariable=status_var, bg=BG, fg=MUTED,
                           font=(FONT_FAMILY, 8))
    status_label.pack(pady=5)
    
    # Log section
    log_frame = tk.Frame(dialog, bg=BG)
    log_frame.pack(fill="both", expand=True, padx=20, pady=10)
    
    log_text = tk.Text(log_frame, bg=BTN, fg=TEXT, font=(FONT_FAMILY, 8),
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
              font=(FONT_FAMILY, 9), padx=20).pack(side="left", padx=5)
    tk.Button(btn_frame, text="OK", command=on_ok, bg=SUCCESS, fg=BG,
              font=(FONT_FAMILY, 9), padx=20).pack(side="left", padx=5)
    tk.Button(btn_frame, text="Cancel", command=on_cancel, bg=BTN, fg=MUTED,
              font=(FONT_FAMILY, 9), padx=20).pack(side="left", padx=5)
    
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
            font=(FONT_FAMILY, 9),
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
            font=(FONT_FAMILY, 10, "bold"),
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
        self._note_save_after = None
        self._loading_preview = False
        self._current_view = "body"
        self._text_zoom = 0
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
            self._loading_preview = True
            self._preview.config(state="normal")
            self._preview.delete("1.0", "end")
            self._preview.insert("1.0", content)
            if label:
                self._view_label_var.set(label)
            self._update_word_count()
            self._loading_preview = False
        self._root.after(0, _do)

    def set_buttons_enabled(self, enabled: bool) -> None:
        self._root.after(0, lambda: [
            b.set_enabled(enabled)
            for b in (self._copy_btn, self._open_btn, self._pdf_btn, self._strip_btn)
        ])

    def set_run_busy(self, busy: bool) -> None:
        self._root.after(0, lambda: self._run_btn.set_busy(busy))

    def set_paper_actions_busy(self, busy: bool) -> None:
        state = "disabled" if busy else "normal"
        cursor = "" if busy else "hand2"

        def _do():
            for b in (
                self._workspace_btn,
                self._scan_btn,
                self._refresh_btn,
                self._smart_search_btn,
            ):
                b.config(state=state, cursor=cursor)
            self._download_all_btn.config(state="normal", cursor="hand2")

        self._root.after(0, _do)

    def set_download_all_state(self, interrupt_mode: bool) -> None:
        def _do():
            if interrupt_mode:
                self._download_all_btn.config(
                    text="Interrupt",
                    bg=ERROR,
                    fg=BG,
                    activebackground=ERROR,
                    activeforeground=BG,
                )
            else:
                self._download_all_btn.config(
                    text="Download All",
                    bg=ACCENT,
                    fg=BG,
                    activebackground=RUN_HOV,
                    activeforeground=BG,
                )
        self._root.after(0, _do)

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

    def _format_zoom_percent(self) -> int:
        return int(round((self._preview_font.cget("size") / 10) * 100))

    def _apply_text_zoom(self):
        preview_size = max(8, min(24, 10 + self._text_zoom))
        text_size = max(8, min(22, 9 + self._text_zoom))
        speaker_size = max(9, min(24, 10 + self._text_zoom))
        self._preview_font.configure(size=preview_size)
        self._chat_font.configure(size=text_size)
        self._chat_input_font.configure(size=text_size)
        self._paper_list_font.configure(size=text_size)
        self._log_font.configure(size=text_size)
        self._chat_speaker_font.configure(size=speaker_size)

    def _change_text_zoom(self, delta: int):
        new_zoom = max(-1, min(14, self._text_zoom + delta))
        if new_zoom == self._text_zoom:
            return "break"
        self._text_zoom = new_zoom
        self._apply_text_zoom()
        self.show_toast(f"Zoom: {self._format_zoom_percent()}%")
        return "break"

    def _bind_zoom_shortcuts(self):
        for sequence in (
            "<Control-minus>",
            "<Control-KP_Subtract>",
        ):
            self._root.bind_all(sequence, lambda e: self._change_text_zoom(-1))
        for sequence in (
            "<Control-equal>",
            "<Control-plus>",
            "<Control-KP_Add>",
        ):
            self._root.bind_all(sequence, lambda e: self._change_text_zoom(1))

    def _on_paper_click(self, event):
        """Handle click on paper list item."""
        selection = self._paper_list.curselection()
        if selection:
            index = selection[0]
            if index < len(self._paper_data):
                self._save_note_now()
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
                           font=(FONT_FAMILY, 9), justify="left",
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
        self._save_note_now(force=self._current_view == "note")
        self._current_view = self.get_view_mode()
        self._presenter.switch_view()

    def _on_copy(self):
        self._root.clipboard_clear()
        self._root.clipboard_append(self.get_preview_text())
        if self.get_view_mode() == "note":
            filename = "note.txt"
        elif self.get_view_mode() == "description":
            filename = "description.md"
        else:
            filename = f"{self.get_view_mode()}.tex"
        self.show_toast(f"Copied {filename}!")

    def _on_preview_key_release(self, _):
        self._update_word_count()
        if self.get_view_mode() != "note" or self._loading_preview:
            return
        if self._note_save_after:
            self._root.after_cancel(self._note_save_after)
        self._note_save_after = self._root.after(800, self._save_note_now)

    def _save_note_now(self, force: bool = False):
        if self._note_save_after:
            self._root.after_cancel(self._note_save_after)
            self._note_save_after = None
        if (force or self._current_view == "note") and not self._loading_preview:
            self._presenter.save_note(self.get_preview_text())
            self._mini_var.set("note saved")
            self._mini_lbl.config(fg=SUCCESS)

    def _create_chat_panel(
        self,
        parent,
        *,
        title: str,
        subtitle: str,
        panel_bg: str,
        output_bg: str,
        placeholder: str,
        on_send: Callable,
        on_stop: Callable,
        on_reset: Callable,
        header_builder: Callable[[tk.Widget], None] | None = None,
    ) -> dict:
        tk.Label(parent, text=title, bg=panel_bg, fg=ACCENT,
                 font=(FONT_FAMILY, 13, "bold")).pack(anchor="w")
        subtitle_label = tk.Label(parent, text=subtitle, bg=panel_bg, fg=MUTED,
                                  font=(FONT_FAMILY, 8))
        subtitle_label.pack(anchor="w", pady=(2, 6))

        if header_builder is not None:
            header_builder(parent)

        bottom = tk.Frame(parent, bg=panel_bg)
        bottom.pack(fill="x", side="bottom")

        status_var = tk.StringVar(value="idle")
        status_label = tk.Label(
            bottom,
            textvariable=status_var,
            bg=panel_bg,
            fg=MUTED,
            font=(FONT_FAMILY, 8),
        )
        status_label.pack(anchor="w", pady=(0, 2))

        metrics_var = tk.StringVar(value="")
        metrics_label = tk.Label(
            bottom,
            textvariable=metrics_var,
            bg=panel_bg,
            fg=ACCENT,
            font=(FONT_FAMILY, 8),
        )
        metrics_label.pack(anchor="w", pady=(0, 4))

        input_widget = tk.Text(
            bottom,
            height=5,
            bg=BG,
            fg=MUTED,
            insertbackground=ACCENT,
            font=self._chat_input_font,
            relief="flat",
            wrap="word",
            padx=8,
            pady=8,
        )
        input_widget.pack(fill="x")

        btn_row = tk.Frame(bottom, bg=panel_bg)
        btn_row.pack(fill="x", pady=(8, 0))

        send_btn = _FlatButton(btn_row, "Send", on_send)
        stop_btn = _FlatButton(btn_row, "Stop", on_stop)
        reset_btn = _FlatButton(btn_row, "Reset", on_reset)
        send_btn.pack(side="left", padx=(0, 6))
        stop_btn.pack(side="left", padx=(0, 6))
        reset_btn.pack(side="left")

        tk.Frame(parent, bg=panel_bg, height=8).pack(fill="x", side="bottom")

        output_wrap = tk.Frame(parent, bg=output_bg)
        output_wrap.pack(fill="both", expand=True)

        output_scroll = ttk.Scrollbar(
            output_wrap,
            orient="vertical",
            style="AC.Vertical.TScrollbar",
        )
        output_scroll.pack(side="right", fill="y")

        output_widget = tk.Text(
            output_wrap,
            bg=output_bg,
            fg=TEXT,
            insertbackground=ACCENT,
            font=self._chat_font,
            relief="flat",
            wrap="char",
            state="disabled",
            yscrollcommand=output_scroll.set,
            padx=8,
            pady=8,
        )
        output_widget.pack(fill="both", expand=True)
        output_scroll.config(command=output_widget.yview)

        panel = {
            "subtitle_label": subtitle_label,
            "status_var": status_var,
            "status_label": status_label,
            "metrics_var": metrics_var,
            "metrics_label": metrics_label,
            "input": input_widget,
            "output": output_widget,
            "send_btn": send_btn,
            "stop_btn": stop_btn,
            "reset_btn": reset_btn,
            "placeholder": placeholder,
            "busy": False,
        }
        self._clear_panel_input(panel)
        input_widget.bind("<FocusIn>", lambda e: self._panel_focus_in(panel, e))
        input_widget.bind("<FocusOut>", lambda e: self._panel_focus_out(panel, e))
        input_widget.bind("<Control-Return>", lambda e: (on_send(), "break"))
        send_btn.set_enabled(True)
        stop_btn.set_enabled(False)
        reset_btn.set_enabled(True)
        return panel

    def _set_panel_status(self, panel: dict, msg: str, color: str = MUTED):
        self._root.after(0, lambda: (
            panel["status_var"].set(msg),
            panel["status_label"].config(fg=color),
        ))

    def _set_panel_busy(self, panel: dict, busy: bool):
        def _do():
            panel["busy"] = busy
            panel["send_btn"].set_enabled(not busy)
            panel["reset_btn"].set_enabled(not busy)
            panel["stop_btn"].set_enabled(busy)
            panel["input"].config(state="disabled" if busy else "normal")
        self._root.after(0, _do)

    def _append_panel_message(self, panel: dict, speaker: str, content: str, color: str, append: bool = False):
        self._append_message_to_widget(panel["output"], speaker, content, color, append)

    def _get_panel_input(self, panel: dict) -> str:
        value = panel["input"].get("1.0", "end-1c").strip()
        return "" if value == panel["placeholder"] else value

    def _clear_panel_input(self, panel: dict):
        panel["input"].delete("1.0", "end")
        panel["input"].insert("1.0", panel["placeholder"])
        panel["input"].config(fg=MUTED)

    def _panel_focus_in(self, panel: dict, _):
        if self._get_panel_input(panel) == "" and panel["input"].get("1.0", "end-1c").strip() == panel["placeholder"]:
            panel["input"].delete("1.0", "end")
            panel["input"].config(fg=TEXT)

    def _panel_focus_out(self, panel: dict, _):
        if not panel["input"].get("1.0", "end-1c").strip():
            self._clear_panel_input(panel)

    def _reset_panel_output(self, panel: dict):
        panel["output"].config(state="normal")
        panel["output"].delete("1.0", "end")
        panel["output"].config(state="disabled")

    def _set_chat_status(self, msg: str, color: str = MUTED):
        self._set_panel_status(self._chat_panel, msg, color)

    def _set_chat_busy(self, busy: bool):
        self._chat_busy = busy
        self._set_panel_busy(self._chat_panel, busy)

    def _append_chat_message(self, speaker: str, content: str, color: str, append: bool = False):
        self._append_panel_message(self._chat_panel, speaker, content, color, append)

    def _append_message_to_widget(self, widget: tk.Text, speaker: str, content: str, color: str, append: bool = False):
        def _do():
            widget.tag_config(f"{speaker}_tag", foreground=color, font=self._chat_speaker_font)
            widget.tag_config(f"{speaker}_body", foreground=TEXT, font=self._chat_font)
            widget.config(state="normal")
            if not append:
                widget.insert("end", f"{speaker}: ", (f"{speaker}_tag",))
                if content:
                    widget.insert("end", content + "\n\n", (f"{speaker}_body",))
                else:
                    widget.insert("end", "\n\n", (f"{speaker}_body",))
            else:
                widget.insert("end", content, (f"{speaker}_body",))
            widget.see("end")
            widget.config(state="disabled")
        self._root.after(0, _do)

    def _run_streaming_chat(
        self,
        *,
        messages: list[dict],
        output_widget: tk.Text,
        set_status: Callable[[str, str], None],
        on_complete: Callable[[str], None] | None = None,
        model: str | None = None,
        include_thinking: bool = False,
        speaker_name: str = "deepseek",
        speaker_color: str = SUCCESS,
    ):
        def _work():
            output_buffer = ""
            try:
                client = self._ensure_chat_client()
                extra_params = {}
                if include_thinking and self._deep_thinking_enabled.get():
                    extra_params["extra_body"] = {"thinking": {"type": "enabled"}}
                    extra_params["reasoning_effort"] = "high"

                response = client.chat.completions.create(
                    model=model or self.CHAT_MODELS[self._chat_model],
                    messages=messages,
                    stream=True,
                    **extra_params,
                )

                first_chunk = True
                for chunk in response:
                    if self._chat_cancelled:
                        break
                    if chunk.choices[0].delta.content:
                        content = chunk.choices[0].delta.content
                        output_buffer += content
                        if first_chunk:
                            self._append_message_to_widget(output_widget, speaker_name, content, speaker_color, append=False)
                            first_chunk = False
                        else:
                            self._append_message_to_widget(output_widget, speaker_name, content, speaker_color, append=True)

                if not self._chat_cancelled and output_buffer.strip():
                    self._append_message_to_widget(output_widget, speaker_name, "\n\n", speaker_color, append=True)
                if self._chat_cancelled:
                    set_status("cancelled", MUTED)
                else:
                    set_status(model or self.CHAT_MODELS[self._chat_model], SUCCESS)
                    if on_complete:
                        on_complete(output_buffer.strip())
            except Exception as exc:
                self._append_message_to_widget(output_widget, "system", str(exc), ERROR, append=False)
                set_status("chat error", ERROR)
            finally:
                self._chat_cancelled = False

        threading.Thread(target=_work, daemon=True).start()

    def _get_chat_input(self) -> str:
        return self._get_panel_input(self._chat_panel)

    def _clear_chat_input(self):
        self._clear_panel_input(self._chat_panel)

    def _chat_focus_in(self, _):
        self._panel_focus_in(self._chat_panel, _)

    def _chat_focus_out(self, _):
        self._panel_focus_out(self._chat_panel, _)

    def _ensure_chat_client(self):
        if self._chat_client is None:
            api_key = load_cached_token()
            if not api_key:
                raise ValueError("Missing DeepSeek API token. Please restart the app and enter your token.")
            self._chat_client = OpenAI(api_key=api_key, base_url="https://api.deepseek.com")
        return self._chat_client

    def build_paper_description(self, paper_dir: str, arxiv_id: str, title: str) -> None:
        paper_path = Path(paper_dir)
        description_path = paper_path / "description.md"
        flag_path = paper_path / ".description_ready"
        body_path = paper_path / "body.tex"
        appendix_path = paper_path / "appendix.tex"
        body = body_path.read_text(encoding="utf-8", errors="ignore") if body_path.exists() else ""
        appendix = appendix_path.read_text(encoding="utf-8", errors="ignore") if appendix_path.exists() else ""
        context = body[:14000]
        if appendix.strip():
            context += "\n\n[Appendix]\n" + appendix[:4000]
        if not context.strip():
            raise ValueError("paper text is empty")
        self._root.after(0, self.add_log, f"[INFO] Building description.md for {arxiv_id}...")
        client = self._ensure_chat_client()
        messages = [
            {
                "role": "system",
                "content": "You write structured markdown briefs for arXiv papers. The brief will later be used for semantic paper search inside a local workspace. Be detailed but compact, faithful to the provided paper text, and emphasize searchable technical concepts. Output markdown only. Use these sections exactly: # Overview, ## Problem, ## Method, ## Key Contributions, ## Technical Details, ## Search Tags, ## Good Match Queries."
            },
            {
                "role": "user",
                "content": f"arXiv ID: {arxiv_id}\nTitle: {title}\n\nPaper text snippet:\n{context}"
            }
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
        self._root.after(0, self.add_log, f"[OK] description.md saved for {arxiv_id}")

    def _collect_workspace_descriptions(self) -> list[dict]:
        if not getattr(self, "_presenter", None) or not self._presenter.workspace_path:
            return []
        entries = []
        papers = self._presenter.get_paper_list()
        for index, paper in enumerate(papers, 1):
            paper_dir = self._presenter.workspace_path / paper["folder_name"]
            description_path = paper_dir / "description.md"
            description = ""
            if description_path.exists():
                description = description_path.read_text(encoding="utf-8", errors="ignore").strip()
            entries.append({
                "index": index,
                "arxiv_id": paper["arxiv_id"],
                "title": paper["title"],
                "folder_name": paper["folder_name"],
                "description": description,
            })
        return entries

    def _build_description_context(self, entries: list[dict]) -> str:
        blocks = []
        for entry in entries:
            description = entry["description"] or "(description unavailable)"
            block = (
                f"[{entry['index']}] {entry['arxiv_id']} | {entry['title']}\n"
                f"folder: {entry['folder_name']}\n"
                f"description:\n{description}"
            )
            blocks.append(block)
        return "\n\n---\n\n".join(blocks)

    def _on_global_chat(self):
        entries = self._collect_workspace_descriptions()
        if not entries:
            self.show_toast("No papers in workspace")
            return

        dialog = tk.Toplevel(self._root)
        dialog.title("Global Chat")
        dialog.geometry("880x700")
        dialog.configure(bg=PANEL)
        dialog.transient(self._root)

        wrap = tk.Frame(dialog, bg=PANEL, padx=12, pady=12)
        wrap.pack(fill="both", expand=True)

        global_history: list[tuple[str, str]] = []
        global_panel: dict = {}

        def _do_global_chat():
            if global_panel.get("busy"):
                return
            query = self._get_panel_input(global_panel)
            if not query:
                self._set_panel_status(global_panel, "empty prompt", ERROR)
                return
            context = self._build_description_context(entries)
            global_history.append(("you", query))
            self._append_panel_message(global_panel, "you", query, ACCENT)
            global_panel["input"].delete("1.0", "end")
            global_panel["input"].config(fg=TEXT)
            self._set_panel_busy(global_panel, True)
            self._set_panel_status(global_panel, "thinking...", MUTED)

            messages = [
                {
                    "role": "system",
                    "content": "You are a global workspace chat assistant inside an arXiv paper extraction tool. You can only use the provided numbered paper descriptions from the current workspace. Maintain short conversation continuity. If the user wants papers, recommend only from the given list. Include bracketed numbers, arXiv ids, exact titles, and concise reasons when pointing to papers. If the user asks a general question unrelated to the provided workspace, answer briefly and say it is outside the workspace scope. Output markdown only."
                },
                {
                    "role": "user",
                    "content": f"Workspace paper descriptions:\n{context}"
                }
            ]
            for speaker, message in global_history[-12:]:
                role = "user" if speaker == "you" else "assistant"
                messages.append({"role": role, "content": message})

            def _on_complete(answer: str):
                if answer:
                    global_history.append(("deepseek", answer))
                self._root.after(0, lambda: self._set_panel_busy(global_panel, False))
                self._root.after(0, lambda: self._clear_panel_input(global_panel))

            def _set_status_proxy(msg: str, color: str):
                self._set_panel_status(global_panel, msg, color)
                if msg in ("cancelled", "chat error"):
                    self._root.after(0, lambda: self._set_panel_busy(global_panel, False))
                    self._root.after(0, lambda: self._clear_panel_input(global_panel))

            self._run_streaming_chat(
                messages=messages,
                output_widget=global_panel["output"],
                set_status=_set_status_proxy,
                on_complete=_on_complete,
                model=self.CHAT_MODELS[self._chat_model],
                include_thinking=True,
            )

        def _on_global_reset():
            if global_panel.get("busy"):
                return
            global_history.clear()
            self._reset_panel_output(global_panel)
            self._clear_panel_input(global_panel)
            self._set_panel_status(global_panel, "reset", MUTED)

        def _build_global_header(parent):
            controls_row = tk.Frame(parent, bg=PANEL)
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
                font=(FONT_FAMILY, 9),
            ).pack(side="left", padx=(0, 10))

            global_model_var = tk.StringVar(value=self._chat_model)

            def _on_global_model_change(value):
                self._on_model_change(value)
                global_panel["subtitle_label"].config(text=self.CHAT_MODELS[value])

            model_dropdown = tk.OptionMenu(
                controls_row,
                global_model_var,
                *self.CHAT_MODELS.keys(),
                command=_on_global_model_change,
            )
            _style_option_menu(model_dropdown)
            model_dropdown.pack(side="left", padx=(0, 10))

            tk.Button(
                controls_row,
                text="Close",
                command=dialog.destroy,
                bg=BTN,
                fg=TEXT,
                activebackground=BTN_HOV,
                activeforeground=TEXT,
                font=(FONT_FAMILY, 9),
                relief="flat",
                padx=10,
                pady=2,
            ).pack(side="left")

        global_panel = self._create_chat_panel(
            wrap,
            title="global chat",
            subtitle=self.CHAT_MODELS[self._chat_model],
            panel_bg=PANEL,
            output_bg=BTN,
            placeholder="Ask about all papers in the current workspace...",
            on_send=_do_global_chat,
            on_stop=self._on_chat_stop,
            on_reset=_on_global_reset,
            header_builder=_build_global_header,
        )
        global_panel["metrics_var"].set(f"ctx: {len(entries)} papers")

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

        context = preview_text[:12000] if preview_text else "(no preview loaded)"
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
        for speaker, message in self._chat_history[-12:]:
            role = "user" if speaker == "you" else "assistant"
            messages.append({"role": role, "content": message})

        def _on_complete(answer: str):
            if answer:
                self._chat_history.append(("deepseek", answer))
            self._root.after(0, self._set_chat_busy, False)
            self._root.after(0, self._clear_chat_input)

        def _set_status_proxy(msg: str, color: str):
            self._set_chat_status(msg, color)
            if msg in ("cancelled", "chat error"):
                self._root.after(0, self._set_chat_busy, False)
                self._root.after(0, self._clear_chat_input)

        self._run_streaming_chat(
            messages=messages,
            output_widget=self._chat_output,
            set_status=_set_status_proxy,
            on_complete=_on_complete,
            include_thinking=True,
        )

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
        self._preview_font = tkfont.Font(root=root, family=FONT_FAMILY, size=10)
        self._chat_font = tkfont.Font(root=root, family=FONT_FAMILY, size=9)
        self._chat_input_font = tkfont.Font(root=root, family=FONT_FAMILY, size=9)
        self._paper_list_font = tkfont.Font(root=root, family=FONT_FAMILY, size=9)
        self._log_font = tkfont.Font(root=root, family=FONT_FAMILY, size=9)
        self._chat_speaker_font = tkfont.Font(root=root, family=FONT_FAMILY, size=10, weight="bold")

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
                 font=(FONT_FAMILY, 13, "bold")).pack(side="left")
        self._workspace_btn = tk.Button(
            paper_header, text="Open Folder",
            command=self._on_open_folder,
            bg=BTN, fg=TEXT, activebackground=BTN_HOV, activeforeground=TEXT,
            font=(FONT_FAMILY, 8), relief="flat", padx=6, pady=2,
            cursor="hand2",
        )
        self._workspace_btn.pack(side="right")
        self._scan_btn = tk.Button(
            paper_header, text="Scan PDFs",
            command=lambda: self._presenter.scan_workspace_pdfs(),
            bg=BTN, fg=TEXT, activebackground=BTN_HOV, activeforeground=TEXT,
            font=(FONT_FAMILY, 8), relief="flat", padx=6, pady=2,
            cursor="hand2",
        )
        self._scan_btn.pack(side="right", padx=(0, 4))

        paper_btn_row = tk.Frame(paper_list_col, bg=PANEL)
        paper_btn_row.pack(fill="x", pady=(0, 6))
        self._refresh_btn = tk.Button(
            paper_btn_row, text="Refresh",
            command=lambda: self._presenter.refresh_paper_list(),
            bg=BTN, fg=TEXT, activebackground=BTN_HOV, activeforeground=TEXT,
            font=(FONT_FAMILY, 9), relief="flat", padx=10, pady=3,
            cursor="hand2",
        )
        self._refresh_btn.pack(fill="x", pady=(0, 4))
        self._download_all_btn = tk.Button(
            paper_btn_row, text="Download All",
            command=lambda: self._presenter.download_all_pending(),
            bg=ACCENT, fg=BG, activebackground=RUN_HOV, activeforeground=BG,
            font=(FONT_FAMILY, 9, "bold"), relief="flat", padx=10, pady=3,
            cursor="hand2",
        )
        self._download_all_btn.pack(fill="x")
        self._smart_search_btn = tk.Button(
            paper_btn_row, text="Global Chat",
            command=self._on_global_chat,
            bg=BTN, fg=TEXT, activebackground=BTN_HOV, activeforeground=TEXT,
            font=(FONT_FAMILY, 9), relief="flat", padx=10, pady=3,
            cursor="hand2",
        )
        self._smart_search_btn.pack(fill="x", pady=(4, 0))

        paper_list_scroll = ttk.Scrollbar(
            paper_list_col,
            orient="vertical",
            style="AC.Vertical.TScrollbar",
        )
        paper_list_scroll.pack(side="right", fill="y")

        self._paper_list = tk.Listbox(
            paper_list_col,
            bg=PANEL,
            fg=TEXT,
            selectbackground=ACCENT,
            selectforeground=BG,
            font=self._paper_list_font,
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
                 font=(FONT_FAMILY, 16, "bold")).pack(side="left")
        tk.Label(title_row, text=f"  {AUTHOR}  {VERSION}",
                 bg=BG, fg=MUTED, font=(FONT_FAMILY, 9)).pack(side="left", pady=(4, 0))

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
            font=(FONT_FAMILY, 11),
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
            "AC.Vertical.TScrollbar",
            gripcount=0,
            background=BTN_HOV,
            darkcolor=BTN_HOV,
            lightcolor=BTN_HOV,
            troughcolor=PANEL,
            bordercolor=PANEL,
            arrowcolor=BTN_HOV,
            relief="flat",
            borderwidth=0,
            arrowsize=12,
            width=12,
        )
        style.map(
            "AC.Vertical.TScrollbar",
            background=[("active", MUTED), ("pressed", ACCENT)],
            darkcolor=[("active", MUTED), ("pressed", ACCENT)],
            lightcolor=[("active", MUTED), ("pressed", ACCENT)],
            arrowcolor=[("active", MUTED), ("pressed", ACCENT)],
            troughcolor=[("active", PANEL)],
        )
        def _style_option_menu(menu: tk.OptionMenu):
            menu.config(
                bg=PANEL,
                fg=TEXT,
                font=(FONT_FAMILY, 9),
                activebackground=ACCENT,
                activeforeground=BG,
                relief="solid",
                bd=1,
                highlightthickness=0,
            )
            menu["menu"].config(
                bg=BG,
                fg=TEXT,
                font=(FONT_FAMILY, 9),
                activebackground=ACCENT,
                activeforeground=BG,
            )

        view_dropdown = tk.OptionMenu(
            ctrl_row,
            self._view_var,
            "body",
            "appendix",
            "note",
            "description",
        )
        _style_option_menu(view_dropdown)
        view_dropdown.pack(side="left")
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
            font=(FONT_FAMILY, 9),
            command=self._toggle_log,
        ).pack(side="left", padx=(10, 0))

        self._mini_var = tk.StringVar()
        self._mini_lbl = tk.Label(
            ctrl_row,
            textvariable=self._mini_var,
            bg=BG,
            fg=MUTED,
            font=(FONT_FAMILY, 10),
        )
        self._mini_lbl.pack(side="right")

        tk.Frame(main_col, bg=BG, height=4).pack(fill="x")

        log_wrap = tk.Frame(main_col, bg=PANEL, pady=4, padx=6)
        self._log_frame = log_wrap

        self._log_text = tk.Text(
            log_wrap,
            bg=PANEL,
            fg=TEXT,
            font=self._log_font,
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
        self._open_btn = _FlatButton(btn_row, "Open Folder", lambda: self._presenter.open_folder())
        self._pdf_btn = _FlatButton(btn_row, "Open PDF", lambda: self._presenter.open_pdf_in_browser())
        self._strip_btn = _FlatButton(btn_row, "Strip Comments", lambda: self._presenter.strip_comments())

        for b in (self._copy_btn, self._open_btn, self._pdf_btn, self._strip_btn):
            b.pack(side="left", padx=(0, 6))

        self._toast_var = tk.StringVar()
        tk.Label(btn_row, textvariable=self._toast_var,
                 bg=BG, fg=MUTED, font=(FONT_FAMILY, 9)).pack(side="right")

        tk.Frame(main_col, bg=BG, height=2).pack(fill="x", side="top")
        self._preview_header = tk.Frame(main_col, bg=BG)
        self._preview_header.pack(fill="x", side="top")
        self._view_label_var = tk.StringVar(value="body.tex")
        tk.Label(self._preview_header, textvariable=self._view_label_var,
                 bg=BG, fg=MUTED, font=(FONT_FAMILY, 9)).pack(side="left")
        self._wc_var = tk.StringVar()
        tk.Label(self._preview_header, textvariable=self._wc_var,
                 bg=BG, fg=MUTED, font=(FONT_FAMILY, 9)).pack(side="right")

        preview_wrap = tk.Frame(main_col, bg=PANEL)
        preview_wrap.pack(fill="both", expand=True)

        preview_scroll = ttk.Scrollbar(
            preview_wrap,
            orient="vertical",
            style="AC.Vertical.TScrollbar",
        )
        preview_scroll.pack(side="right", fill="y")

        self._preview = tk.Text(
            preview_wrap,
            bg=PANEL,
            fg=TEXT,
            insertbackground=ACCENT,
            font=self._preview_font,
            relief="flat",
            wrap="word",
            yscrollcommand=preview_scroll.set,
            padx=8,
            pady=6,
            undo=True,
        )
        self._preview.pack(fill="both", expand=True)
        preview_scroll.config(command=self._preview.yview)
        self._preview.bind("<KeyRelease>", self._on_preview_key_release)

        self._deep_thinking_enabled = tk.BooleanVar(value=self._deep_thinking)

        def _build_side_chat_header(parent):
            controls_row = tk.Frame(parent, bg=PANEL)
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
                font=(FONT_FAMILY, 9),
            ).pack(side="left", padx=(0, 10))
            self._chat_model_var = tk.StringVar(value=self._chat_model)
            model_dropdown = tk.OptionMenu(
                controls_row,
                self._chat_model_var,
                *self.CHAT_MODELS.keys(),
                command=self._on_model_change
            )
            _style_option_menu(model_dropdown)
            model_dropdown.pack(side="left", padx=(0, 10))
            tk.Button(
                controls_row,
                text="Update Token",
                command=self._on_update_token,
                bg=BTN,
                fg=TEXT,
                activebackground=BTN_HOV,
                activeforeground=TEXT,
                font=(FONT_FAMILY, 9),
                relief="flat",
                padx=10,
                pady=2,
            ).pack(side="left")

        self._chat_panel = self._create_chat_panel(
            chat_col,
            title="chat",
            subtitle=self.CHAT_MODELS[self._chat_model],
            panel_bg=PANEL,
            output_bg=BTN,
            placeholder=self._chat_placeholder,
            on_send=self._on_chat_send,
            on_stop=self._on_chat_stop,
            on_reset=self._on_chat_reset,
            header_builder=_build_side_chat_header,
        )
        self._chat_model_label = self._chat_panel["subtitle_label"]
        self._chat_status_var = self._chat_panel["status_var"]
        self._chat_status_lbl = self._chat_panel["status_label"]
        self._chat_metrics_var = self._chat_panel["metrics_var"]
        self._chat_metrics_lbl = self._chat_panel["metrics_label"]
        self._chat_input = self._chat_panel["input"]
        self._chat_output = self._chat_panel["output"]
        self._chat_send_btn = self._chat_panel["send_btn"]
        self._chat_stop_btn = self._chat_panel["stop_btn"]
        self._chat_reset_btn = self._chat_panel["reset_btn"]
        self._bind_zoom_shortcuts()
        self._apply_text_zoom()
