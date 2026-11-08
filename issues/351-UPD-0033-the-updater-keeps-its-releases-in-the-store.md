# 351-UPD-0033 — The updater keeps the running and the last-known-good release in the block store

**Type:** `updater` · **Owner:** `STO`, by agreement with the update track's owner (the kernel session, 2026-11-08) · **Priority:** P3 · **Status:** open · **Blocked by:** [351-KRN-0218](351-KRN-0218-the-updaters-store-client.md) (`init`'s store client) · **Main task:** [351](351-self-update.md) · **Constitution:** MC-9.3, MC-12.1

**Numbered by the update track's owner** at STO's offer (`requests-UPD.md`, 2026-11-08): STO makes it with the track's code and counter (AGENTS.md, section 2), in `updater/` and its tests, on its own branch. The plan is STO's, taken as it is; the update track's owner reviews the landing request.

## Problem

- **MC-9.3:** the recovery images and the objects they need are kept from ordinary cleanup. Today the last-known-good release lives only in the slot that booted, so a damaged slot can only be fetched again from a source.
- **The block store** keeps what an owner pins until that owner ends the pin, and its collection never frees it. It holds files and directories as objects (`mind::dag`, `mind::tree`) and reads them back with every block checked against its CID.

## Plan

1. **Put each release in the store** (`init`'s client is requested of KRN in `requests-KRN.md`):
   - when a fetch or an install has staged a release and its files have passed their checks, each file goes in as an object (`dag::Builder`), and the directory as a `mind::tree` root (`mind::worktree`);
   - the root is pinned, and named `release/<version>` for people. Names are anyone's to change, so the updater never trusts one.
2. **Keep two, after each confirmed boot** (351-KRN-0014): the pins of the running and of the last-known-good release stay; older ones end. The updater finds its releases by its own pins (`pins`) and the manifest each holds.
3. **Rebuild a slot from the store** as from any other source (`source.rs`, beside `Https`, `Directory` and `Bundle`): the manifest is checked against the boot key and every file by size and SHA-256, so the store's copy can stage nothing the signatures do not allow. A minor version of `update.wit` asks for it, and `status` says which releases the store keeps.
4. **Without the store,** or with a store too small, the updater runs as now and logs why it kept nothing.

## Acceptance criteria

- **QEMU, the `update` suite, x86 and aarch64,** with a store disk:
  - after a confirmed update and a collection (`blocks collect`), the store holds both pinned releases whole;
  - a slot damaged from the host is rebuilt from the store, and boots;
  - the shell's store client cannot end the updater's pins.

## Related

[351-KRN-0218](351-KRN-0218-the-updaters-store-client.md), [351-STO-0006](351-STO-0006-releases-pinned-in-the-store.md), 351-UPD-0007 (the updater's sources, STO's).
