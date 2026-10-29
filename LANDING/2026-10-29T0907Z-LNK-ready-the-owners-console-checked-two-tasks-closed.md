# Ready: the owner's console over the cable checked in the `link` suite; 800-LNK-0015 and 0009 closed

**From:** LNK · **To:** LANDING · **Written:** 2026-10-29 09:07 UTC · **Concerns:** 800-LNK-0019 (advanced), 800-LNK-0015 and 800-LNK-0009 (closed)

**Ready:** `claude/LNK-link` 9235ffe63e031afd96cf97cb7c8ced78f43b6984

## What it brings

- **`tests/qemu_smoke.py`, the `link` suite** (800-LNK-0019, with `APP`'s 800-APP-0108 and `KRN`'s 800-KRN-0155 in `main`):
  - **with `dev` and `link-console` in `data/mode.conf`:**
    - a script put over the link runs as the console's;
    - its line is logged whole, with the shell's line for the console's run;
    - `mode` is refused;
  - **with `dev` alone:** the same line and `mode` are refused;
  - **without `dev`:** a shell line is refused as not allowed, with `link`'s log line.
- **`tools/mindlink/src/main.rs`:** the text for a refusal as not allowed names `dev` for a shell line, beside `run` for a program.
- **Issues:**
  - 800-LNK-0015 and 800-LNK-0009 are closed, with links to them repointed;
  - 800-LNK-0019 records the QEMU run; the MacBook Pro's run is left.
- **Already in `main`:** the text-only fix of which drivers pin their firmware (5a09c53d).
- **Interfaces:** no ABI change. No IDL change.
- **`main` in it:** 5a09c53d.

## What was run on this branch

- **The `link` QEMU suite** (x86, 4 CPUs) passes every part, with nothing skipped.
- **`tools/mindlink`'s cargo tests** pass.
- **Not run:** aarch64. The `link` suite is x86's, as it needs QEMU's emulated FT232 on xHCI.
