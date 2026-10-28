# 259 — A spreadsheet: simple, but complete

**Type:** main task, tools · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** track G (user services) · **Constitution:** MC-4.1, MC-5.1, MC-11.3, MC-11.5, MC-11.11, MC-12.1, MC-12.3

**Recorded by:** the tools track, 2026-10-23, at the maintainer's request: spreadsheets, "simple but fully functional".

## Problem

- **No program on MIND Core keeps a table of numbers and formulas.**
- **"Simple but fully functional"** means what a user expects of any spreadsheet:
  - cells with values and formulas that recompute;
  - sheets, sorting and filtering;
  - formats, charts;
  - files that other programs open.
- It does not mean macros in a programming language, pivot tables at first, or collaboration.

## Plan

**The program `calc`** (its name is the maintainer's to change), in the console and in a `wm` window on `mind::tui`; charts in a pixel window.

**1. The sheet and its formulas.**

- **The grid:**
  - columns A…XFD and 1 048 576 rows, as other spreadsheets;
  - stored sparse, so that an empty sheet costs nothing (MC-5.1).
- **Cell values:** numbers (IEEE 754 doubles, displayed rounded to their format), text, booleans, errors (`#DIV/0!`, `#REF!`, `#NAME?`, `#VALUE!`, `#N/A`, `#NUM!`, `#CIRC!`), dates and times as serial numbers with a format.
- **Formulas:**
  - `=` with references: `A1`, `$A$1`, ranges `A1:B10`, other sheets `Sheet2!A1`, named ranges;
  - arithmetic and comparison, `&` for text, percent.
- **Functions,** a stated set first, each host-tested against its definition in OpenFormula (ODF part 4, ISO/IEC 26300):
  - **mathematical:** `SUM` `SUMIF` `SUMIFS` `PRODUCT` `ROUND` `ROUNDUP` `ROUNDDOWN` `INT` `MOD` `ABS` `POWER` `SQRT` `MIN` `MAX` `AVERAGE` `AVERAGEIF` `MEDIAN` `COUNT` `COUNTA` `COUNTIF` `COUNTIFS` `STDEV` `VAR`;
  - **logical:** `IF` `IFS` `AND` `OR` `NOT` `IFERROR` `ISBLANK` `ISNUMBER` `ISTEXT`;
  - **text:** `CONCAT` `TEXTJOIN` `LEFT` `RIGHT` `MID` `LEN` `UPPER` `LOWER` `TRIM` `SUBSTITUTE` `FIND` `SEARCH` `TEXT` `VALUE`;
  - **lookup:** `VLOOKUP` `HLOOKUP` `INDEX` `MATCH` `XLOOKUP` `CHOOSE`;
  - **date and time:** `DATE` `TODAY` `NOW` `YEAR` `MONTH` `DAY` `WEEKDAY` `EDATE` `EOMONTH` `DATEDIF`;
  - **financial:** `PMT` `FV` `PV` `NPV` `IRR`.
- **Recalculation:**
  - a dependency graph recomputes only what a change reaches;
  - a cycle is reported as `#CIRC!` on its cells, never as a hang;
  - `TODAY`/`NOW` are recomputed on F9 and at open.

**2. Editing.**

- **Movement:** by arrows, PgUp/PgDn, Ctrl+arrows to a block's edge, Ctrl+Home, `F5`/go to a cell.
- **Editing a cell:** in place or in the formula line (F2); Enter and Tab move on.
- **Selection:** ranges and whole rows and columns with Shift and the mouse.
- **Copy, cut and paste:**
  - with references adjusted, relative or absolute;
  - paste values only, paste formats only.
- **Fill** down and right (Ctrl+D, Ctrl+R) and a series (1, 2, 3…; dates by day, month).
- **Rows and columns:** insert, delete, hide; column width and row height; freeze panes.
- **Sorting** by one or more columns, ascending and descending, with a header row. **Filtering** by values and conditions.
- **Find and replace,** in values or formulas.
- **Sheets:** several per file, added, renamed, moved, deleted.
- **Undo and redo** for every change, with a stated depth.

**3. Formats.**

- **Number formats:** general, fixed decimals, thousands separators, percent, currency, scientific, dates and times, text, and custom format strings of a stated grammar.
- **Cell styles:** bold, italic, alignment, a colour and a background where the screen has colours, borders drawn with box characters.
- **Conditional formatting:** a colour by a value's rule.
- **Locale:** the decimal separator and the argument separator (`,` or `;`) follow the user's choice, as in other spreadsheets; files store the neutral form.

**4. Files.**

- **Its own file:** OpenDocument Spreadsheet (`.ods`, ISO/IEC 26300), the native format, so that LibreOffice opens it as it is.
- **CSV and TSV:** import and export, with the separator, the quote and the encoding (UTF-8) chosen.
- **Office Open XML (`.xlsx`):** import and export after `.ods`.
- **Converters run in a parser session** (MC-11.11). What a file has and `calc` does not (macros, pivot tables, unknown functions) is kept where it can be or listed to the user, never dropped silently (MC-11.5).
- **The file's version** is carried and checked (MC-4.1).

**5. Charts.** Bar, line, pie and scatter charts of a range, drawn in a pixel window; in the console as text bars. Kept in the file.

**6. For scripts.** `calc --eval <file> <cell>` prints a cell's value; `calc --to csv <file>` converts; `msh` scripts can read a sheet.

## Acceptance criteria

- **Host tests:**
  - every function against OpenFormula's examples;
  - recalculation and cycles;
  - reference adjustment on copy, insert and delete;
  - formats;
  - the `.ods` and CSV round trips;
  - a stated corpus of `.ods` and `.xlsx` files opened, with what was kept.
- **QEMU suites:** a sheet made, filled, sorted, filtered and saved as `.ods`, reopened, exported to CSV; a 100 000-row sheet opened and recalculated within a stated time and memory.
- **Docs:** `docs/tools/README.md` and `README_RU.md`.

## Related

[258](258-wysiwyg-document-editor.md) (a table or a sheet in a document), [257](257-full-text-editor.md) (CSV in the text editor).
