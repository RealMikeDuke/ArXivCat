# ArxivCat TUI — Feature Spec & Layout

## Layout

```
┌─ HEADER ───────────────────────────────────────────────────────────────────────┐
│  ArxivCat TUI — /path/to/workspace  papers:51                                  │
├─ PAPERS ──┬─ PREVIEW ────────────────────────────────┬─ CHAT ─────────────────┤
│ ● paper1  │  Body | Appdx | Note | Desc   [scrollbar] │  <You> msg             │
│ ● paper2  │                                          │  <AI>  reply           │
│ ● paper3  │  line 1 of body text                     │                        │
│   ...     │  line 2 of body text                     │                        │
│           │  line 3 ...                              │                        │
│           │                                          │                        │
│           │  🔓 unlocked | scroll ↑↓/wheel           │                        │
│           │                                          │                        │
├───────────┴──────────────────────────────────────────┴────────────────────────┤
│  > chat input here                                                [cursor ▐]  │
├─ STATUS ───────────────────────────────────────────────────────────────────────┤
│  [NORMAL] ↑↓/scroll/jk:nav 1-4:view c:chat e:edit s:scan d:dl :cmd q:quit     │
└────────────────────────────────────────────────────────────────────────────────┘
```

### Zones

| Zone | Rows | Default width | Resizable |
|------|------|---------------|-----------|
| Header | 2 | full | No |
| Paper List | remaining - 1 | 25% | Yes (left border) |
| Preview | remaining - 1 | 40% | Yes (right border) |
| Chat | remaining - 1 | 35% | No (stretches) |
| Status | 1 | full | No |

Chat panel is optional — when hidden, preview stretches to fill the space.

---

## Input Modes

### Normal Mode

Default mode — navigate papers and view content.

| Key | Action |
|-----|--------|
| `j` / `↓` | Next paper |
| `k` / `↑` | Previous paper |
| `Enter` | Load selected paper, open chat |
| `1` `2` `3` `4` | Switch to Body / Appendix / Note / Description |
| `c` | Toggle chat panel |
| `e` | Enter NoteEdit (Note tab only) |
| `s` | Scan workspace for new PDFs |
| `d` | Download all pending papers |
| `o` | Open paper folder |
| `p` | Open paper PDF on arXiv |
| `:` | Enter Command mode |
| `q` | Quit (blocked when hotkeys locked) |
| `Ctrl+Alt+Q` | Toggle hotkey lock |
| `Ctrl+C` | Copy selected text |
| `←` `→` `↑` `↓` | Move cursor in preview |
| `Home` / `End` | Jump to line start/end in preview |
| `PageUp` / `PageDown` | Scroll preview by one page |

**When cursor is focused in Note preview (hotkeys locked or non-hotkey chars):**

| Key | Action |
|-----|--------|
| Printable chars | Insert at cursor (overwrite selection first if any) |
| `Backspace` / `Delete` | Delete selection, or char before cursor |
| `Enter` | Insert newline |

**Mouse:**

| Action | Result |
|--------|--------|
| Click paper | Select & load it |
| Click tab | Switch view mode |
| Click preview text | Place cursor, start 0-length selection (anchor=focus) |
| Drag in preview | Extend selection |
| Click scrollbar track | Jump scroll to clicked position |
| Drag scrollbar thumb | Continuous scroll |
| Wheel on preview | Scroll ±3 lines |
| Wheel on chat | Scroll messages |
| Click chat input | Focus chat mode |
| Drag left border | Resize paper-list ↔ preview |
| Drag right border | Resize preview ↔ chat |

### Chat Mode

Multi-line chat input for asking questions about the current paper.

| Key | Action |
|-----|--------|
| Printable chars | Type into input |
| `Enter` | Send message |
| `Backspace` / `Delete` | Delete char |
| `←` `→` `Home` `End` | Move cursor |
| `Esc` | Close chat, return to Normal |
| `Ctrl+V` | Paste (multi-line → `[Pasted ~N lines]`) |

### NoteEdit Mode

Full-text editor for the Note tab.

| Key | Action |
|-----|--------|
| Printable chars | Insert (overwrite selection first) |
| `Enter` | Insert newline |
| `Backspace` / `Delete` | Delete selection or char before cursor |
| `Esc` | Save note to disk, return to Normal |
| `Ctrl+V` | Paste |

### Command Mode

Single-line command input (prefix `:`).

| Command | Action |
|---------|--------|
| `o` / `open` | Open paper folder |
| `p` / `pdf` | Open arXiv PDF |
| `scan` | Scan workspace for PDFs |
| `dl` / `download` | Download all pending |

`Esc` cancels, `Enter` executes.

---

## Features

### Paper Management
- Load workspace from configured path
- Paper list with status icons: ● complete, ○ has body, · metadata only
- Select & load paper (Enter or click), auto-opens chat
- Four view tabs: Body, Appendix, Note, Description
- Scan workspace for new PDFs/archives (`s`)
- Batch download all pending papers (`d`)
- Open paper folder or arXiv PDF link from TUI

### Text Viewer (Preview)
- Catppuccin Mocha color scheme
- Title bar shows character + line counts
- Scroll: mouse wheel, arrow keys, PageUp/PageDown, scrollbar
- Scrollbar: track + proportional thumb, click-to-jump, drag-to-scroll
- Cursor: click-to-place, arrow-key movement, visually rendered (inverted colors)
- Text pre-split into visual lines at character-width boundaries; rendered without ratatui Wrap
- Unicode width aware (CJK double-width, emoji)
- Empty-line cursor visible (▌ block char)

### Text Selection
- Drag to select text across visual lines
- Selected text highlighted (background color)
- Click = 0-length selection (anchor=focus at same point, like opencode)
- Ctrl+C copies selected text to system clipboard
- Status bar shows "[copied to clipboard]" feedback on copy
- Typing in focused Note preview deletes selection first, then inserts
- Double-click: select word
- Triple-click: select line
- Copy-on-select: auto-copy on mouse-up (toggleable via config)

### Chat
- Toggle panel (`c`)
- Input area with `> ` prefix and visible cursor
- Mouse click positions cursor in chat input
- Message list: speaker labels (`<You>` / `<AI>`)
- Scroll via mouse wheel, auto-scroll to bottom on new message
- Ctrl+V paste with multi-line folding
- Model name and deep-thinking indicator (🧠) in title
- Chat session persistence across restarts
- SSE streaming response display
- Model selection toggle (Flash / Pro)
- Token usage display
- Global chat mode (over all papers)

### Hotkey Lock
- Ctrl+Alt+Q toggles
- Locked: all printable keys insert directly (in focused Note preview)
- Unlocked: hotkeys function normally
- Lock status shown in preview footer (🔒 / 🔓)
- `q` quit always blocked when locked

### Panel Resizing
- Drag left border: paper list ↔ preview, clamped 15–55%
- Drag right border: preview ↔ chat, clamped 20–50%
- Seam areas highlight on hover

---

## Visual Design

### Color Scheme (Catppuccin Mocha)

| Element | Color | RGB |
|---------|-------|-----|
| Text (normal) | Text | 205, 214, 244 |
| Text (dim) | Subtext1 | 166, 173, 200 |
| Text (muted) | Overlay0 | 108, 112, 134 |
| Selection highlight bg | Surface0 | 69, 71, 90 |
| Cursor fg | Base | 30, 30, 46 |
| Cursor bg | Blue | 137, 180, 250 |
| Border (rest) | Crust | 49, 50, 68 |
| Border (hover) | Overlay0 | 108, 112, 134 |
| Header | Blue | 137, 180, 250 |
| Scrollbar track | Crust | 49, 50, 68 |
| Scrollbar thumb | Overlay0 | 108, 112, 134 |
| Tab active | Blue bg / Base fg | 137, 180, 250 / 30, 30, 46 |
| Tab inactive | Overlay0 fg / Base bg | 108, 112, 134 / 30, 30, 46 |
| Paper selected | Text fg / Surface0 bg | 205, 214, 244 / 69, 71, 90 |
| Paper hover | Blue fg / Crust bg | 137, 180, 250 / 49, 50, 68 |

### Scrollbar

```
    │  ← track (Crust bg)
    │
    █  ← thumb (Overlay0 bg)
    █
    █
    │
    │
```

- Track: 1 column, Crust background
- Thumb: Overlay0 background
- `thumbHeight = max(1, ceil(viewportLines / totalLines × trackHeight))`
- `thumbY = scrollPos / (totalLines - viewportLines) × (trackHeight - thumbHeight)`
- 1-column gap between text and scrollbar (avoids overlap with panel-resize seam)

---

## Architecture

### Crate Separation

```
crates/
├── arxivcat-core/          Business logic
│   ├── config.rs           Config loading
│   ├── workspace.rs        Workspace, Paper, scan, download
│   ├── extract/            PDF/TeX extraction
│   └── chat.rs             DeepSeek chat API
│
└── arxivcat-tui/           Terminal UI
    ├── main.rs             Event loop, layout, rendering, input dispatch
    └── app.rs              App state, view modes, visual-line engine, coordinate mapping
```

Core is pure logic. TUI consumes core via `use arxivcat_core::...`.

### Coordinate System (zellij-inspired)

Each visual line is self-contained:

```rust
struct VisualRow {
    text: String,         // rendered text for this row
    abs_byte_start: usize, // byte position in the original full text
}
```

**Mouse click → byte position:**
```
visual_row = visual_rows[screen_y + scroll]
clicked_char = visual_row.text.chars().nth(screen_x)
byte_offset = clicked_char's byte index within visual_row.text
absolute_byte = visual_row.abs_byte_start + byte_offset
```

**Byte position → cursor display:**
```
binary search visual_rows.abs_byte_start for row containing cursor_byte
column = cursor_byte - visual_row.abs_byte_start → char index in visual_row.text
render cursor at (row, column)
```

No dependency on logical-line structure. The `VisualRow` array is built once from the full text at the current line width, and serves both rendering and hit-testing.
