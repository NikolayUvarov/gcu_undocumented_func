# 258 — A WYSIWYG document editor

**Type:** main task, tools · **Owner:** tools track (`APP`) · **Priority:** P3 · **Status:** open · **Blocked by:** scalable fonts and text shaping (shared with [256](256-graphical-web-browser-current-standards-only.md), stage 1) for the graphical view · **Roadmap:** track G (user services) · **Constitution:** MC-3.1, MC-4.1, MC-11.3, MC-11.5, MC-11.11, MC-12.1, MC-12.3

**Recorded by:** the tools track, 2026-10-23, at the maintainer's request: the editor in two variants, a text one ([257](257-full-text-editor.md)) and a WYSIWYG one. This is the WYSIWYG one.

## Problem

- **No program on MIND Core edits a formatted document:** headings, emphasis, lists, tables, images, page layout and print.
- **`edit` edits plain text.** Today a document is written as Markdown and read as text.
- **A WYSIWYG editor shows the document as it will look.**
  - In a `wm` window, the text in its fonts, sizes and styles, and the pages with their margins.
  - In the console, a text view that keeps the structure (headings, emphasis as colour, lists, tables) for machines without a graphical screen.

## Plan

**The program `write`** (its name is the maintainer's to change).

**1. The document model and its file.**

- **A tree of blocks:**
  - paragraphs, headings (six levels), lists (nested, numbered and not, with checkboxes);
  - block quotes, code blocks, horizontal rules;
  - tables (cells that hold blocks, merged cells), images with captions, page breaks, footnotes.
- **Inline runs:** bold, italic, underline, strike, code, links, a colour and a highlight, sub- and superscript.
- **Named styles** (Title, Heading 1–6, Body, Quote, Code) that blocks refer to, so that a document's look changes in one place.
- **Its own file:** a text format that diffs well and that `edit` can open.
  - The choice is made on a design page: CommonMark with a stated set of extensions (GFM tables, footnotes, attributes for styles) or a small structured format of its own.
  - The format carries its version (MC-4.1).
- **Import and export** through converters, each a separate program in a parser session (MC-11.3, MC-11.11):
  - **Markdown:** both ways.
  - **HTML:** export, and import of the subset the model holds.
  - **OpenDocument Text (`.odt`, ISO/IEC 26300):** both ways, the subset the model holds; what is lost on import is listed to the user, never dropped silently (MC-11.5).
  - **Office Open XML (`.docx`, ISO/IEC 29500):** the same, after `.odt`.
  - **PDF:** export only, with the fonts embedded.

**2. The text view (console and `wm`'s text windows).**

- On `mind::tui`: headings in colour and capitals, emphasis by attributes (bold, underline, inverse), lists and tables drawn with box characters, images as `[image: caption]`.
- **Editing** as in `edit`: the same keys for movement, selection, the clipboard, undo, search.
- **Formatting** by the menu and by keys (Ctrl+B, Ctrl+I, Ctrl+U, Ctrl+1…6 for headings, Ctrl+L for lists).

**3. The graphical view (a `wm` pixel window).**

- The page laid out with its margins, fonts and sizes.
  - Line breaking (UAX #14) and hyphenation for Russian and English.
  - Scalable fonts and shaping, shared with 256's text stack.
- **Views:** a zoom and a page view, a continuous view.
- **Rulers** for indents and tab stops; the cursor and the selection in pixels; the mouse.
- **Images** placed inline or floated, resized by their handles.
- **Printing** to PDF; to a printer when the system has one (a separate task).

**4. Review.**

- Comments in the margin.
- Tracked changes, accepted or rejected.
- A word and character count; spell checking (shared with 257).

## Acceptance criteria

- **Host tests:**
  - the model's operations and undo;
  - the file format's round trip;
  - the converters on a stated corpus (sample `.odt` and `.docx` files from their standards' test suites or LibreOffice's, with what was kept and what was listed as lost).
- **QEMU suites:** a document made in the text view, saved, reopened, exported to Markdown, HTML and PDF; in the graphical view, a page compared with its expected picture.
- **Docs:** `docs/tools/README.md` and `README_RU.md`.

## Related

[257](257-full-text-editor.md) (the text editor: keys, search, spell checking), [256](256-graphical-web-browser-current-standards-only.md) (fonts and shaping), [259](259-spreadsheet.md) (tables, and a sheet embedded in a document).
