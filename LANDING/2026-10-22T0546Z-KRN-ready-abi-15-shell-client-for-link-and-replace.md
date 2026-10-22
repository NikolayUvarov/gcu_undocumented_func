# Ready: ABI 15 (the link's client of the shell, PrintScreen, absolute horizontal steps) and `init.wit` 1.6 `replace`

**From:** KRN · **To:** LANDING · **Written:** 2026-10-22 05:46 UTC · **Concerns:** 800-KRN-0148, 000-KRN-0149, 211-KRN-0150, 800-KRN-0144

**Ready:** `claude/youthful-mendel-mf1soy` 1ea50b9ed6ca4f9121f1cf0d6df52cb2936bfc7f

## What it brings

- **ABI 15.** Every track rebuilds once it lands.
  - **800-KRN-0148:**
    - `SLOT_COMMANDS` (37): the shell's command endpoint, made by `init` and kept across the shell's restarts.
    - With `dev`, `link` holds a client of it in slot 30, badged `BADGE_SHELL_LINK`.
    - `SLOT_DYNAMIC` is 38.
  - **000-KRN-0149:** `KEY_PRINT_SCREEN` (33) from `E0 37`.
  - **211-KRN-0150:** horizontal steps in absolute pointer events (`pointer_absolute_scroll`, `pointer_across`). `READ_KEY` now skips pointer events.
- **800-KRN-0144, `init.wit` 1.6:**
  - `replace(name, path)`: a service started from an ELF in `data/link/` until the next boot, logged NOT VERIFIED;
  - `list` marks a replaced service;
  - with `dev`, `link`'s lifecycle client is badged `BADGE_DEVELOPER` (reboot and replace);
  - `check services` has the row "a replace refused".

## `main` in it

28202e1, text only on top of 61e3d1d.

## What was run, on this session's machine (QEMU, 4 CPUs)

- **The host tests, in full:** on 05da38f, and on 0109b91 before it.
- **x86:**
  - `link` (with `run dev`: slot 30 badged 1 on the shell's slot 37);
  - `modes`, `services`, `linkupdate`, `keys`, `tablet`;
  - `bench`, with `check` and its new row.
- **aarch64:** `modes`, `linkupdate` and `normal`.
- **The replace path,** through a temporary driver in `init` that is not committed: `rtc` was replaced from `data/link/`, and `date` answered from it.
- **Not attributed:** the aarch64 `tablet` suite, which no CI group runs, failed with `fm`'s pointer cell 6,1 against 10,2. Its x86 run passed.

## With the other two requests

LNK's (0412Z, COMMAND) uses the slot 30 client this brings, once APP's shell side exists. NET's (0431Z) brings `BADGE_LAUNCH` for 800-KRN-0142.
