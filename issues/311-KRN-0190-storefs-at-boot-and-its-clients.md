# 311-KRN-0190 — `storefs` at boot, a client of it beside `vfs_server`'s, and `mind::fs` routing by volume

**Type:** `init` (start and grants), `common/abi.rs` (a slot, ABI), `libmind` `fs` · **Owner:** `KRN` · **Priority:** P2 · **Status:** made (2026-10-30); closes once landed, the shell's part aside · **Blocked by:** — · **Main task:** [311](311-storefs-a-file-face.md) (STO's), for 311-STO-0032 · **Constitution:** MC-3.1, MC-3.4, MC-6.1, MC-12.4

**Asked by:** the storage track (`STO`) in `issues/requests-KRN.md`, 2026-10-30, "`storefs`: started by `init`, reached by a files client, and a `conflict` error in `vfs.wit`", and in its message to KRN of 10:53. The kernel track chose the way in, (b), in its answer of 11:15.

## Problem

`storefs` will serve `vfs.wit` over the store's directory trees `fs/<volume>`. Nothing starts it, and no program holds a client of it.

STO offered two ways in: (a) `vfs_server` forwards the volumes it does not hold, or (b) `mind::fs` picks the server by volume. Forwarding would cost three things:
- **Waiting.** `vfs_server` serves one request at a time. Every file client, and the system journal and the updater's zone with them, would wait on `storefs` and, through it, on `blockstore` (MC-6.1: the failure boundary).
- **Rights.** `vfs_server` would hold each client's rights for both servers (MC-3.1, 3.4).
- **Buffers.** Each lent buffer would cross twice.

## Plan

- **`init`:**
  - starts `storefs` after `blockstore`, with a client of the store that may get, put and publish (`blockstore`'s badge for it), and its service endpoint;
  - waits for it as for the other services;
  - leaves it out in the safe start, as `blockstore`.
- **A slot for a client of `storefs`** in a program's space, beside `SLOT_VFS`, in `common/abi.rs`. ABI 26 and handoff 26, if the boot images' list changes with it.
  - `init` lends one to the shell.
  - `storefs` badges its own clients, as `blockstore` does (308-KRN-0184).
- **`mind::fs` routes by volume:**
  - `vfs_server`'s volumes (`""`, `ram`, `log`, `models`) are asked first;
  - a root `vfs_server` answers `not-found` is then asked of `storefs`, if the program holds a client in that slot;
  - each root is opened once, so that is one extra call per volume.

  `storefs` refuses those four names for a volume of its own. If STO names a fixed prefix instead, `mind::fs` routes by it.
- **The shell's part is APP's,** filed in `requests-APP.md` with the slot's number: it lends its `storefs` client to the programs it starts, and a scoped one for `REQUEST_FILE` on a store volume.
- **Docs:** `docs/api` for `mind::fs`'s routing, and the boot services list.

## Acceptance criteria

- `storefs` runs at boot on x86 and aarch64 in QEMU.
- The shell's `ls` and `cat` reach a volume of it, through the same `mind::fs` calls as a FAT volume.
- `vfs_server`'s volumes answer as before: the `files and block writes` group (x86) and `files and the block store` (aarch64) pass unchanged.
- A program without a `storefs` client gets `not-found` for a store volume, and nothing is asked of `storefs`.
- Host tests: `mind::fs`'s routing, and the four reserved names.

## Progress

- **2026-10-30, made** over STO's `storefs/` (311-STO-0032, round 58):
  - **ABI 27, handoff 28** (`common/abi.rs`):
    - `storefs.elf` is a boot image right after `keystore`; `02_build.sh` builds it again;
    - `SLOT_STOREFS` 42 is a launch slot, and `SLOT_DYNAMIC` moves to 43.
  - **`init`:**
    - starts `storefs` after `keystore` and `blockstore`, with its endpoint (with `CAP_BADGE`) and, while the store runs, the store's client badged get, put and publish in `SLOT_BLOCKSTORE`;
    - gives the shell a client badged `BADGE_USER` in `SLOT_STOREFS`.
    - `data/services.txt` may turn it off, and turning `blockstore` off turns it off. **Unlike the plan, a safe start leaves it on,** as it leaves `blockstore` on.
  - **`mind::fs`** (`libmind/src/fs.rs`, `libmind/src/fs/volumes.rs`):
    - asks `vfs_server` for `""`, `ram`, `log` and `models` (`VFS_VOLUMES`), and `storefs` for every other volume through `SLOT_STOREFS`;
    - without that client a store volume is `NotFound`, and nothing is asked;
    - a client a launcher lent (`use_endpoint`) takes every volume, as before;
    - each `Dir` and `File` keeps its server's slot; a move between two servers is `Invalid`;
    - a client holds the four roots of `vfs_server` and two of `storefs` at most.
  - **Docs:** `docs/api/README.md` (ABI 27, `SLOT_STOREFS`, the routing), `README.md` and `docs/profile/bootstrap.md` (`storefs`'s row, the shell's client).
  - **Tests:**
    - host: `tests/fs_routing_host.rs` (the routing, a lent client, and `storefs`'s refused names against `VFS_VOLUMES`); `tests/runtime.rs` (the slots, ABI 27); `tests/boot_abi_host.rs` (handoff 28, 47 images, `storefs` after `keystore`);
    - QEMU: `storefs_check` in the `shell` suite: `storefs` ready with the store's client; `mkdir`, `write`, `cat`, `ls` and `rm` on `notes:`; `find notes:` from a program without a client says `NOTFOUND`.
- **Tested locally (x86, QEMU TCG), over STO's c89c70c2 before it landed:**
  - the x86 build;
  - host tests `runtime`, `boot_abi`, `fs_routing`, `vfs_zone`, `idl`;
  - the `shell` suite whole, with `storefs_check`.
- **Left:**
  - the machines' run on x86 and aarch64 once STO's `storefs/` is in `main`;
  - the shell's part: its client lent to programs. APP waits for a client of `storefs` that does not write (`requests-APP.md`), so programs get none for now.
  - `storefs` answering `conflict` (311-STO-0032).

## Related

[311](311-storefs-a-file-face.md), 311-STO-0032, [311-KRN-0189](../issues-done/311-KRN-0189-a-conflict-error-in-vfs-wit.done), [308-KRN-0184](../issues-done/308-KRN-0184-blockstore-badges-its-own-clients.done).
