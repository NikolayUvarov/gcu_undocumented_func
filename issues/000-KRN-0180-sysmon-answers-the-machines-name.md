# 000-KRN-0180 — `sysmon` answers the machine's name (`sysinfo.wit` 4.2)

**Type:** `sysmon`, `idl/sysinfo.wit`, `init` (a read-only scope) · **Owner:** `KRN` · **Priority:** P2 · **Status:** open · **Blocked by:** APP's choice of the file (its task for the setting) · **Main task:** — · **Constitution:** MC-3.1, MC-10.2, MC-12.4

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

## Related

[000-NET-0050](000-NET-0050-dhcp-sends-the-machines-name.md), 750-EFF-0018, 800-LNK-0028, the APP task for the setting.
