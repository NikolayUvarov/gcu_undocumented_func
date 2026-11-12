# 311-APP-0135 — `fm` opens a store volume, served by `storefs`

**Type:** tools (`fm`) · **Owner:** tools track (`APP`) · **Priority:** P3 · **Status:** open · **Blocked by:** — · **Main task:** [311](../issues-done/311-storefs-a-file-face.done) · **Roadmap:** track G · **Constitution:** MC-4.1, MC-11.5

**Asked by:** STO, for 311's plan item "`fm` and the shell over `storefs`", in [requests-APP.md](requests-APP.md), 2026-11-12. Numbered on 2026-11-12.

## Problem

`storefs` serves `vfs.wit` 2.6 over the store's names `fs/<volume>`, and `mind::fs` reaches it for a volume `vfs_server` does not have, when the program holds `storefs`'s client. The shell lends it (311-APP-0121), but `fm`'s panels are a fixed list (the boot disk, `ram:`, `log:`, `models:`, `store:`), so the store's files reach the owner through the shell's commands alone.

## Plan

- **A panel for each store volume:** the names under `fs/` (`blockstore.wit`'s `names("fs/")`) listed among the roots, or one typed; opened as a disk's, through the `storefs` client the shell lends.
- **Copying between a disk and a store volume,** each file published as its close does.

## Acceptance criteria

- **Host tests:** the roots with store volumes, a panel over one.
- **QEMU (`fm` in the `tools` or `wm` suite):** a store volume a test fills through the shell, listed, opened and a file copied to and from it.

## Related

[311](../issues-done/311-storefs-a-file-face.done), [311-APP-0121](../issues-done/311-APP-0121-the-shell-lends-a-storefs-client.done), 300-APP-0101 (`store:` in `fm`).
