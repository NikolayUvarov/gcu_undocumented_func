# 351-STO-0043 — A reserve for the updater's releases: the other owners together within three quarters of the medium

**Type:** service (storage) · **Owner:** `STO` track · **Priority:** P2 · **Status:** in progress (2026-11-09) · **Blocked by:** — · **Main task:** [351](351-self-update.md) · **Roadmap:** track B; track C · **Constitution:** MC-9.3, MC-4.11

## Problem

- **The updater keeps its releases in the store,** pinned by its own owner ([351-UPD-0033](../issues-done/351-UPD-0033-the-updater-keeps-its-releases-in-the-store.done)), so that MC-9.3's recovery images are kept from cleanup.
- **The quota was three quarters of the medium for every owner,** and nothing bounded the owners together (303-STO-0002's "not done"). The shell's owner alone could retain three quarters, and others more, until the updater found no room for a release.

## Plan

- **A quarter of a lasting medium is kept for the updater's owner** (`OWNER_UPDATER` in the badge's owner bits, below `FIRST_DOMAIN_CLIENT`).
  - Every other owner together retains at most three quarters, each root counted once across them all.
  - A publication, a commit or a pin that would pass that is refused with `quota`, and nothing is written, as past an owner's own quota.
  - Each owner's own quota stays three quarters, the updater's too, so it may also use room beyond its quarter.
- **On a RAM disk nothing is set aside:** nothing there outlives a reset, and the updater keeps nothing in it.
- **`usage`** gives another owner, as its quota, what the others together leave it.
- **The store says so at mount:** `[BLOCKSTORE] A QUARTER OF THE MEDIUM KEPT FOR THE UPDATER'S RELEASES: …`.
- **The size is a quarter** until whoever grants the clients grants quotas; this task does not make that grant.

## Acceptance criteria

- **Host tests** (`tests/blockstore_host.rs`):
  - with the reserve, a publication, a pin and a commit by an owner within its own quota but past the others' together are refused;
  - a root another owner retains already costs nothing more;
  - the updater pins within the quarter, with its own quota whole;
  - a domain's client is another owner, whatever its low bits;
  - without the reserve, as before.
- **QEMU:**
  - the `store` suite, x86 and aarch64: no reserve on the RAM disk;
  - the `boot` suite's check of a store disk, x86, where `boot` runs: the reserve on a disk.

## Related

[351-UPD-0033](../issues-done/351-UPD-0033-the-updater-keeps-its-releases-in-the-store.done), [303-STO-0002](../issues-done/303-STO-0002-pins-and-quotas.done), [docs/storage](../docs/storage/README.md).
