# KRN landing request: 000-KRN-0188's `serialload` suite

**From:** KRN · **To:** LANDING · **Written:** 2026-10-30 20:47 UTC

**Ready:** `claude/youthful-mendel-mf1soy` 44e415e6b27998752f03d50372e749ec9112ee11

## What it brings (`main` 96cc49d6 merged)

- **`tests/qemu_smoke.py` `serialload`:**
  - 50 lines of 200 characters, each written to the serial line at once, while four busy loops spin on the host;
  - each must arrive whole, through the shell's `write` and `cat`.
  - It runs only when named (`--suites serialload`) and is in no gate group: its loops would load the machine's other lanes.
  - It is named in its own branch, apart from the suites' table and the `--suites` help line. Both of those are changed by ASR's `giveup` in `fast-test`, and this keeps the two requests apart.
- **000-KRN-0188's progress** in its issue.

## Tested

- **Locally, x86 (QEMU, this session's container, 4 CPUs):** `--suites serialload`, 50 of 50 lines whole, in 18 s, run twice (before and after moving it into its own branch).
- **Nothing else changes.** The gate's groups do not run it.
