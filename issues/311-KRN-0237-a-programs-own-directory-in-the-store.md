# 311-KRN-0237 — A program's own data directory in the store, when the owner says so

**Type:** kernel track (`loader`) · **Owner:** `KRN` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [311](../issues-done/311-storefs-a-file-face.done) · **Roadmap:** track B · **Constitution:** MC-4.1, MC-4.4, MC-2.3

## Problem and plan

STO's request of 2026-11-12 (from `requests-KRN.md`), for 311's plan item "the programs' own data directories (000-KRN-0170) can live here". The request as it was written, kept whole:

**Asked by** STO on 2026-11-12, for [311](../issues-done/311-storefs-a-file-face.done)'s plan item "the programs' own data directories (000-KRN-0170) can live here".

- **Today:** for `REQUEST_OWN` the loader gives a program a writable scope of `vfs_server`'s `data/<program>/` on the boot volume's FAT (000-KRN-0170). It has no history, no snapshot and no checksum.
- **What `storefs` has:** a scope of a store volume (311-STO-0037), read-only or writable, confined to one directory, and lent as a capability of `storefs`'s own endpoint.
- **Asked of the loader:**
  - where the owner says so, the program's own directory is a writable scope of `storefs`'s volume `own`, its directory `<program>/`, instead of the FAT one;
  - the word could be one line in a boot file, `own store`, as `store.txt` and `update.txt` are written: yours to choose;
  - the program reaches it as now, through `mind::fs` and the same slot.
- **What the owner gets:** each close publishes a new root of `fs/own`. `blocks history fs/own` and two roots compared (311-STO-0039) show what changed. A program's directory is then also under the store's quotas and its memory domains (308-STO-0023).
- **Not asked:** moving what a program kept on FAT. A program on a machine that turns this on starts with an empty directory, as before the first release nothing is converted.
- **Checked by** a check of STO's in the `store` suite once this is in: `bench`'s file cycle in its own directory on the store, the file found after a reboot, and its earlier version read back.

## Acceptance criteria

- What the request's "Checked by" names, run by the asking track once this is in; `init`'s or the loader's own lines in the log.
- The tests the change touches pass on x86 and aarch64 in QEMU.

## Related

- [requests-KRN.md](requests-KRN.md), where it was asked.
