# Ready: `clipboard.wit` 1.1, the service mints a window manager's program copies; the clipboard's host test by name

**Ready:** `claude/wizardly-franklin-kec1a9` 28bc3d93de39b13d386fbb316f4ec1de9cf25ef1

It contains `main` 20fba2c5e1.

- **Advances 000-APP-0032**, for KRN's report of 2026-11-13 from round 125's gate of 000-KRN-0235 (`wm` lent no clipboard: a copy on the wire carries no right to badge, 000-KRN-0073). The first way, as KRN recommended:
  - `clipboard.wit` 1.1 `program-client`, a manager's alone, moves to it a `BADGE_PROGRAM` client the service mints from `SLOT_SERVICE` (`unavailable` while the service may not badge it); bindings regenerated;
  - the shell lends a window manager a copy badged `BADGE_MANAGER`; `wm` uses it as its manager copy and asks the service for each program's.
  - KRN's 000-KRN-0235 (`claude/000-KRN-0235-clipboard`) gives the service its endpoint with `CAP_BADGE` in the same round (`KRN/2026-11-13T0216Z-APP-the-first-way-clipboard-wit-1-1-is-ready.md`). The two merge without a conflict; either lands alone without harm (without the right, `wm`'s programs keep their own buffers, as now).
- **The clipboard's host test by name** (000-KRN-0238): its own line in `scripts/host_tests.sh` dropped, as the loop runs it.
- **The issues** of round 124's landing marked in `main`.
- **Tested** (QEMU x86, 4 CPUs, in this container, on 892fcd94; the service does not run without `init`'s part, so these check the paths unchanged): `wm` (17), `tools` (10), `shell` (17). Host tests: `clipboard_host` (4, with `program-client`'s rule), `idl_fuzz`, `idl`, `shell_host`, `fm_host`, `edit_host`, `viewer_host`, `console_host`; the loop's 162 rustc host tests; every clipboard client builds. The commits after 892fcd94 are the merge of `main` (text only).
- **Merges** with every request waiting: cleanly with KRN's 0235, NET's 0073 and sshd; with STO's `relaxed-meitner` only in `issues/312-APP-0134-…md`'s metadata line (text only: STO's link to `issues-done/312` beside my status; keep both).
