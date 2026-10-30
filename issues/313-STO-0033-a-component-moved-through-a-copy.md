# 313-STO-0033 — A component moved to another store through a copy, active in one place only

**Type:** storage (`libmind` `checkpoint`, `tally`, `blocks`) · **Owner:** `STO` track · **Priority:** P2 · **Status:** open · **Blocked by:** — (uses [309-STO-0031](309-STO-0031-a-copy-of-named-roots-in-a-file.md)'s copies, on this branch) · **Main task:** [313](313-persistent-actors.md) · **Constitution:** MC-6.10, MC-6.11, MC-6.12, Appendix B.4

Split from [313](313-persistent-actors.md) (2026-10-30), its third step: "a move to another machine through a copy of its store, and a rebind".

## Problem

A component's checkpoint lives in one store. A copy of its names ([309-STO-0031](309-STO-0031-a-copy-of-named-roots-in-a-file.md)) can be carried to another store, but two things stand in the way:
- **It is not restored there.** An import publishes the names at that store's versions. `restore` then refuses a manifest whose epoch is not its names' version (`Inconsistent`).
- **Nothing ends the component at the source.** An instance there could go on taking requests after the copy was made. Two places would then both be active, which fencing (MC-6.12) exists to prevent.

## Plan

- **A move out** at the source:
  - **releases the component:** one commit by compare-and-swap from the epoch restored, which fences every instance of that epoch;
  - **writes its last checkpoint to a copy,** from the names' history if need be.

  A release refused (another instance saved since) moves nothing.
- **After a release, the source refuses the component:**
  - a new instance there is refused at its restore, by a reason that names the release, not taken as a fresh start;
  - an older instance is fenced at its save.
- **A move in:** an import of the copy into another store, then a restore there.
  - The restore takes the names' version in that store as its epoch.
  - It keeps the checks that the manifest and the state's names agree, and that a state name never moved alone.
  - The instance then continues the same sequence, clients table, inbox and outbox. Its rights come from its grants there (MC-6.11).
- **A cut anywhere in a move** leaves the component either still active at the source, or released there with its last checkpoint still in the source's history, to be written to a copy again. Never active in two places.
- **`tally` as the pilot:**
  - `tally as <name> move <file>` releases and writes the copy;
  - the copy is brought in with `blocks import`;
  - the next `tally as <name> …` there restores it and goes on.

  `tally`'s contract changes only if the release needs it, and then its version and docs/storage/checkpoints.md say so.
- **docs/storage/checkpoints.md:** moves, releases, and what "active in one place" rests on. It rests on the release's compare-and-swap in the source's store; nothing checks two stores against each other.

## Acceptance criteria

- Host tests (`tests/checkpoint_host.rs`, two stores on simulated media):
  - a move stopped after each number of either store's calls in turn: the component never active in both, and its sequence, clients and outbox carried whole when it lands;
  - a released component refused at the source by its restore, and an instance restored before the release fenced at its save;
  - a moved-in checkpoint restored and saved again in the other store, and a manifest whose state name moved alone still refused.
- QEMU `store` suite (x86, aarch64), on the copies' reboot:
  - `tally as a` counts, and is moved to a file on the boot volume;
  - at the source it is refused afterwards;
  - after the reboot leaves the store blank, the copy is imported and `tally as a` goes on from its count, its repeated operations still answered from the record.
- docs/storage/checkpoints.md and the profile.

## Related

[313](313-persistent-actors.md), [313-STO-0026](../issues-done/313-STO-0026-operation-ids-and-queues-in-a-checkpoint.done), [313-STO-0030](313-STO-0030-messages-between-components.md), [309-STO-0031](309-STO-0031-a-copy-of-named-roots-in-a-file.md), [306](../issues-done/306-checkpoints-and-rebinding.done).
