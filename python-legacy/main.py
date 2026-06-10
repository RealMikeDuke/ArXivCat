"""GUI entry point."""

from arxivcat.ui.tkinter_ui import TkApp


def main():
    app = TkApp()
    app.run()


if __name__ == "__main__":
    main()
