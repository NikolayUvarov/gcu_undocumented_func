# 000-APP-0062 — `fm` opens in data/, and F7 makes a directory where the user may write

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-11.5

**Recorded by:** the maintainer, 2026-10-11: "fm does not create directories." Asked what to change, the maintainer chose to keep the policy, with no change to who may write where.

## Problem

F7 in `fm` makes directories in `data/`, on `ram:` and on `log:`, from the shell and from `wm`. QEMU showed that, with nested and Cyrillic names too. It fails everywhere else on the boot disk. That is the threat model's policy (`docs/profile/threat-model.md`): the user's files client writes on `ram:`, on `log:` and in `data/` of the boot disk.

But `fm` opens at the boot disk's root, from `wm`'s menu and from the shell alike. So the first F7 a user tries ends with `Cannot make A:/x: denied (only ram:, log: and data/ are writable)`, and `fm` seems unable to make directories.

## Plan

- **Where `fm` opens.** `fm` with no argument opens its left panel in `A:/data` when that exists, otherwise at the root. `fm /` opens the root.
- **F7 on the boot disk outside `data/`.** The dialog says, before anything is typed, that only `data/` may be written there. A name typed relative to the panel is made in `A:/data/`, and the panel goes there with the cursor on it. A path that names the root (`A:/x`, `/x`) is still refused with the reason.
- **The tests.** The QEMU tests that rely on the root start `fm /`.

## Acceptance criteria

- `tests/fm_host.rs`:
  - `outside_data` and `mkdir_target` for the root, `EFI`, `data`, `ram:`, `log:` and paths with a volume or a leading `/`;
  - F7 at the root: the dialog's line, `data/system` made, the panel in `data/` on it, the notice; `A:/system` refused.
- The QEMU `disk` check: `fm` alone opens in `A:/data`; F7 at the root makes `data/плюс` and goes there.
- `docs/tools` (EN and RU) say where `fm` opens and where F7 makes a directory.

## Related

- 000-APP-0063: the editor's green line, reported at the same time.
- 068: `fm`'s writes.
- `docs/profile/threat-model.md`: who may write where.
