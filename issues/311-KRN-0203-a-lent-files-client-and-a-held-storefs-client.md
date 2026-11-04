# 311-KRN-0203 — A lent files client and a held `storefs` client: store volumes go to `storefs`

**Type:** `libmind` · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress (made; landing) · **Blocked by:** — · **Main task:** [311](311-storefs-a-file-face.md) (APP's request for [311-APP-0121](311-APP-0121-the-shell-lends-a-storefs-client.md)) · **Constitution:** MC-3.1

## Problem

`mind::fs::use_endpoint(SLOT_FILE)` lends a program the shell's `vfs_server` client. `volumes::route` (311-KRN-0190) then sent every volume to it, so a program such as `fm` found no store volume even when it held a client of `storefs` (APP, `KRN/2026-11-03T1659Z-APP-storefs-for-programs-…`, part 2).

## Plan

**`route`:**
- `vfs_server`'s four volumes go to the lent client, or to `SLOT_VFS` when none was lent;
- any other volume goes to `SLOT_STOREFS` when the program holds it, else to the lent client, else to nobody.

**What it grants.** Nothing new: it only routes to a client the program already holds. The shell's lending of a `storefs` client in `SLOT_STOREFS` is APP's half.

**Part 1 of APP's request** (a plain `storefs` client for every program from the loader) is not taken. KRN proposed a read-only scope the shell lends instead (`APP/2026-11-04T0340Z-KRN-storefs-for-programs-part-2-is-mine-part-1-another-way.md`), and the maintainer decides.

## Acceptance criteria

- **`tests/fs_routing_host.rs`:** a lent client takes `vfs_server`'s volumes, and a held `storefs` client takes the others; without one, the lent client does, as before.

## Related

311-KRN-0190, [311-APP-0121](311-APP-0121-the-shell-lends-a-storefs-client.md).
