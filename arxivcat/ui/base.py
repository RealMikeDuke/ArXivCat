"""Abstract UI protocol. All UI backends must implement this interface."""
from typing import Protocol, runtime_checkable


@runtime_checkable
class UIProtocol(Protocol):
    """Interface that every UI backend must satisfy."""

    def add_log(self, msg: str) -> None:
        """Append a log line. Prefix [OK] / [ERROR] / [INFO] determines color."""
        ...

    def set_mini_status(self, msg: str, level: str = "info") -> None:
        """
        Update the small inline status label.
        level: 'info' | 'ok' | 'error'
        """
        ...

    def set_preview(self, content: str, label: str) -> None:
        """Replace the main preview text area content."""
        ...

    def set_buttons_enabled(self, enabled: bool) -> None:
        """Enable / disable the action buttons (Copy, Overwrite, …)."""
        ...

    def set_run_busy(self, busy: bool) -> None:
        """Toggle the Run button between ready and busy states."""
        ...

    def show_toast(self, msg: str, duration_ms: int = 2000) -> None:
        """Show a transient status message that auto-clears."""
        ...

    def get_url_input(self) -> str:
        """Return the current value of the URL / ID input field."""
        ...

    def get_view_mode(self) -> str:
        """Return current dropdown selection: 'body' or 'appendix'."""
        ...

    def get_preview_text(self) -> str:
        """Return the current text in the preview area."""
        ...

    def clear_log(self) -> None:
        """Clear all log entries."""
        ...

    def set_url_input(self, url: str) -> None:
        """Set the value of the URL / ID input field."""
        ...

    def set_paper_list(self, papers: list[dict]) -> None:
        """Update the paper list in the left panel."""
        ...

    def set_title(self, title: str) -> None:
        """Set the window title."""
        ...

    def run(self) -> None:
        """Start the UI event loop (blocking)."""
        ...
