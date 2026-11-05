# 000-KRN-0180 — `sysmon` answers the machine's name (`sysinfo.wit` 4.2)

**Type:** `sysmon`, `idl/sysinfo.wit`, `init` (a read-only scope) · **Owner:** `KRN` · **Priority:** P1 (the maintainer, 2026-11-05) · **Status:** in progress · **Blocked by:** — · **Main task:** — · **Constitution:** MC-3.1, MC-10.2, MC-12.4

**Asked by:** EFF with LNK, `ALL/2026-10-28T0407Z-EFF-the-machines-name.md` on `internal-dev-messages`, item 3, after the maintainer's word of 2026-10-28: the machine's name, set on the machine itself.

## Problem

- **Who needs the name.** The Effector agent, `link` and `netstack` need the machine's name.
- **Why they cannot read it.** None of them holds a client of `data/`:
  - the agent's scopes are `system/effector` and `data/effector`;
  - `link`'s are `data/link` and `data/firmware`.
- **The proposed source.** One setting that APP keeps on the boot disk (proposed: `data/hostname`, one RFC 1123 label, `mindcore` when unset).

## Plan

1. **`sysinfo.wit` 4.2:** `hostname() -> string<63>`, answered by `sysmon` to every client.
   - It reads the file at each call, or when it changed, and checks it is a label.
   - `mindcore` is the answer when the file is missing or not a label, and then `sysmon` logs why.
2. **`sysmon` gets a read-only `vfs_server` scope of the name's directory alone.**
   - The KRN track asks APP to keep the file in a directory of its own, for example `data/machine/name`, so that this scope reaches nothing else (MC-3.1).
   - A scope of all of `data/` is not given.
3. **Clients** keep the `sysmon` client they already hold:
   - `effector_gw`;
   - `link` in developer mode;
   - the shell.
4. **`netstack` starts before `sysmon`,** so it gets the same read-only scope of the name's directory. Its use is the network track's [000-NET-0050](000-NET-0050-dhcp-sends-the-machines-name.md).

## Acceptance criteria

- **Host tests:** the label check (length, characters, hyphens), and the default.
- **QEMU, x86:**
  - with the file set to `dexp`, `sysinfo hostname` (or a shell command over the client) answers `dexp`;
  - changed while running, the next call answers the new name;
  - missing or invalid, it answers `mindcore` and logs why;
  - `stat caps` shows `sysmon`'s scope and nothing wider.
- **Docs:** `docs/api` and the profile's `sysmon` row.

## The file (2026-11-05)

APP chose it in [000-APP-0123](000-APP-0123-the-machines-name-and-hostname.md): `data/machine/name`, a directory of its own, as point 2 asks.
- **What it holds:** one RFC 1123 label, with an optional line end.
- **Capital letters** are taken as small ones when the name is set. A file that holds capitals was not written by the shell, so `sysmon` reads it as holding no name, as APP's `read` does.
- **A missing file, or one that is not a label,** gives `mindcore`.
- **Who writes it:** APP's `hostname <name>`. Until that command lands, the owner can write the file by hand.

## Progress

2026-11-05, on `claude/000-KRN-0180-hostname`:
- **`sysinfo.wit` 4.2:** `hostname() -> string<63>`. `libmind/src/idl/sysinfo.rs` is generated again.
- **`mind::hostname`** (`libmind/src/hostname.rs`) holds the directory, the file, the default and APP's rule (`label`, `read`). The shell may use it in place of its own copy.
- **`sysmon`** answers `hostname` from `name` in its `SLOT_VFS` scope, at each call. It logs `[SYSMON] THE MACHINE'S NAME: <name> (<source>)` when the name or its source changes, and answers `mindcore` when the file is missing, holds no label, or cannot be read.
- **`init`** gives `sysmon` a read-only scope of `data/machine/` (`scopes_of`), made if missing. A name set later is then read without a restart. A volume that cannot be written gives no scope, and `init` says so.
- **`bench`'s head:** `MIND Core on <name>, …` in `check`, `bench` and `kbench`, on the screen and in their logs.
- **Tests:**
  - host: `tests/hostname_host.rs`;
  - QEMU: `hostname_check` in the `vfs` suite; `kbench_check` requires the head's name; `edit_suite`'s listing of `data/` now holds `machine`.
- **Docs:** `docs/api/README.md` (`hostname`), `docs/profile/bootstrap.md` (`sysmon`).

## Related

[000-NET-0050](000-NET-0050-dhcp-sends-the-machines-name.md), 750-EFF-0018, 800-LNK-0028, [000-APP-0123](000-APP-0123-the-machines-name-and-hostname.md) (the setting and `hostname`).
