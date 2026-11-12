# 311-APP-0135 — `fm` opens a store volume, served by `storefs`

**Type:** tools (`fm`) · **Owner:** tools track (`APP`) · **Priority:** P3 · **Status:** open (done in the branch, 2026-11-12; its QEMU check to pass) · **Blocked by:** — · **Main task:** [311](../issues-done/311-storefs-a-file-face.done) · **Roadmap:** track G · **Constitution:** MC-4.1, MC-11.5

**Asked by:** STO, for 311's plan item "`fm` and the shell over `storefs`", in [requests-APP.md](requests-APP.md), 2026-11-12. Numbered on 2026-11-12.

## Problem

`storefs` serves `vfs.wit` 2.6 over the store's names `fs/<volume>`, and `mind::fs` reaches it for a volume `vfs_server` does not have, when the program holds `storefs`'s client. The shell lends it (311-APP-0121), but `fm`'s panels are a fixed list (the boot disk, `ram:`, `log:`, `models:`, `store:`), so the store's files reach the owner through the shell's commands alone.

## Plan

- **A panel for each store volume:** the names under `fs/` (`blockstore.wit`'s `names("fs/")`) listed among the roots, or one typed; opened as a disk's, through the `storefs` client the shell lends.
- **Copying between a disk and a store volume,** each file published as its close does.

## Acceptance criteria

- **Host tests:** the roots with store volumes, a panel over one.
- **QEMU (`fm` in the `tools` or `wm` suite):** a store volume a test fills through the shell, listed, opened and a file copied to and from it.

## Progress (2026-11-12)

- **The volume menu** (Alt+F1/F2, `fm/src/fm.rs`): after the disks, each store volume the `Disk` lists (`store_volumes`), at most 16; one named as a disk's volume is the disk's. Its line is `notes: store volume` and the size `storefs` gives. The status line names the line under the cursor as a panel's path: `DIALOG=VOLUME:/notes:`.
- **The store volumes** (`fm/src/main.rs`): the store's names under `fs/`, one page (`blockstore.wit`'s `names("fs/")`), each `fs/<volume>` with a volume's name (`fm/src/panel.rs`'s `store_volume`), where fm holds `storefs`'s client: the shell's copy, lent with the user's files (311-APP-0121), so fm writes there as it writes `data/`. A volume `vfs_server` has is left out.
- **A panel over one** is a disk's, through `mind::fs`: listing, F3, the editor, F5, F6, F7 and F8, each file published as its close does (storefs).
- **Tested:** `tests/fm_host.rs`'s `the_volume_menu_lists_the_store_volumes_and_opens_one`: the names, the menu with a store volume and one named as a disk's, Enter, a copy each way. **Checked by** the `shell` suite's `fm_store_volume_check`, in `storefs_check`: `notes:` listed, opened, a file copied from it to `ram:` and from `ram:` to it.

## Related

[311](../issues-done/311-storefs-a-file-face.done), [311-APP-0121](../issues-done/311-APP-0121-the-shell-lends-a-storefs-client.done), 300-APP-0101 (`store:` in `fm`).
