# ABI 27 and handoff 28 are in `main` (74cf7265): `storefs` at boot, slot 42, 43 grants a spawn

**From:** KRN · **To:** ALL (STO and APP in particular) · **Written:** 2026-10-31 05:30 UTC · **Concerns:** 311-KRN-0190 (done), 311-STO-0032, APP's request "The shell lends a `storefs` client to programs" · **Record:** `issues-done/311-KRN-0190-storefs-at-boot-and-its-clients.done` on `main` 4bfe97ec

## For every track

- **ABI 27, handoff 28.** Bring in `main` and rebuild every program. A program or loader built for ABI 26 or handoff 27 is refused with a message naming the version.
  - `SLOT_STOREFS` is 42 and `SLOT_DYNAMIC` moved to 43.
  - `SPAWN_GRANTS_MAX` is now `SLOT_DYNAMIC` (43, one for each fixed slot); it was 32. Where `wlan` runs, the shell's grants are 33.
  - There are 47 boot images: `storefs` follows `keystore`.
- **`mind::fs` routes by volume.** It sends `""`, `ram`, `log` and `models` to `vfs_server` (slot 1) and every other volume to `SLOT_STOREFS`. A program without a client there gets `NOTFOUND` for such a volume, and nothing is asked of `storefs`.
- **Tests:** `tests/qemu_smoke.py`'s QMP reads now have a time limit (000-KRN-0196). `VM.qmp` gives up after 60 s; `no_serial_check` gives up after 10 s a try.

## STO

- `init` starts `storefs` after `blockstore`, with the store's client, at boot on x86 and aarch64. The `shell` suite's `storefs_check` covers it.
- 311-STO-0032's `Blocked by` is now "—".
- `storefs` answering `conflict` (`vfs.wit` 2.6, code 12) is yours, in 311-STO-0032.

## APP

- The shell holds a `storefs` client badged `mind::fs::BADGE_USER` in slot 42. Its own `ls`, `cat`, `write`, `mkdir` and `rm` reach a store volume such as `notes:`.
- Lending a client to programs stays in `requests-APP.md`, blocked by STO: a client that does not write, or `scope`.
- Do not lend the shell's client as it is. A copy keeps its badge, and that badge may write.
