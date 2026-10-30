# 313-STO-0033 — A component moved to another store through a copy, active in one place only

**Type:** storage (`libmind` `checkpoint`, `tally`, `blocks`) · **Owner:** `STO` track · **Priority:** P2 · **Status:** open · **Blocked by:** — (uses [309-STO-0031](../issues-done/309-STO-0031-a-copy-of-named-roots-in-a-file.done)'s copies, on this branch) · **Main task:** [313](313-persistent-actors.md) · **Constitution:** MC-6.10, MC-6.11, MC-6.12, Appendix B.4

Split from [313](313-persistent-actors.md) (2026-10-30), its third step: "a move to another machine through a copy of its store, and a rebind".

## Problem

A component's checkpoint lives in one store. A copy of its names ([309-STO-0031](../issues-done/309-STO-0031-a-copy-of-named-roots-in-a-file.done)) can be carried to another store, but two things stand in the way:
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

## Decisions while building (2026-10-30)

- **A release is a record, not a removal.** One commit by compare-and-swap from the epoch restored moves both names to one raw block: `MIND-RLS`, the format version (2), the last checkpoint's epoch, and the CIDs of its manifest and state. A removed name keeps only its removal in its history, so after a removal the last checkpoint would be retained by nothing, and a move cut after it could not be written again. With a record, the names' history keeps the last checkpoint as the version before the release, and nothing at the source saves after it.
- **The manifest's format stays 2.** The release record is a block of its own. A restore looks for its magic before it decodes a manifest: both names at one record is `Error::Released(<the release's epoch>)`, never fresh, and a record at the manifest's name alone is `Inconsistent`. A build without the record would refuse it as not a manifest (`Format`).
- **`mind::checkpoint` gains:**
  - `release(store, names, from)`: `Fenced` if the names are not at `from`; `Invalid` with no checkpoint there; `Released` if both names are at one block already (a checkpoint's manifest names its state, so they never are). It returns the record and the release's epoch;
  - `released(store, names)`: the record again, for a second move;
  - `Release`, with its one encoding.

  The `Store` trait is unchanged: a release is a commit of both names to the record's root.
- **A manifest of another epoch is restored only at its names' first version** (the session's review, 2026-10-30). A manifest moved in records its epoch in the store it came from, and an import into a store that never held its names publishes them at version 1. So restore takes a manifest whose epoch is not its names' version only when they are at 1, and its epoch is then 1. Anywhere else it is still `Inconsistent`, so an older manifest and state published again by hand at a later version of existing names are refused (a host test). The first build dropped the epoch check wholesale, and lost that. The other checks hold invariant 4 as before: both names at one version (a state name moved alone), and the manifest's state root that of the state's name. A move into a store where the names already exist (removed: they come in at a later version) is refused, and stays under "Not provided".
- **The copy is written after the release, never before.** Written before, a copy of a component whose release was then refused could be imported while the source was still active. It is a copy of both names at the version before the release, each at its root: `car::write`, a copy of the roots given, which `car::export` now uses for the current ones. The names node records the source's version.
- **A second move writes the same copy:** the restore is refused (`Released`), `released` gives the record, and the names, versions and roots are the same, so are the bytes. The host and QEMU tests compare them.
- **`tally [as <name>] move <file>`** (a component without `as` moves too) checks the files authority before the release. It prints `RELEASED <name> EPOCH <e>: …`, then `MOVED <name> EPOCH <e-1> TO <file>: …; SHA-256 <hex>`; a second move `RELEASED <name> EPOCH <e> BEFORE: …`. A copy not written whole is removed. A component with no checkpoint is not moved. Every other run of a released component prints `tally: RELEASED: … (MC-6.12)`: MC-6.12 asks that a migration prevent the old and the new owner from both accepting commands.
- **`tally`'s contract stays 3:** its manifest and state are unchanged, and the release record is `mind::checkpoint`'s, the same for every contract (docs/storage/checkpoints.md says why).
- **The move in needs nothing new:** `blocks import` publishes both names at version 1 of a store that never held them, and the next `tally as <name>` restores them.
- **A move back** into the store a component left: the import finds both names at the record, another root, and leaves them (a conflict), so the component stays released there. Host-tested, and said under "Not provided".
- **The QEMU check, `tally_move_check`,** runs after `store_copy_check`, with a reboot of its own done the same way (`reboot_blank_store`, now shared by both). The stalled instance holds 15 s. The suite writes the release record itself from its format and compares its CID with both names' root.
- **The host tests** bring `worktree` and `car` into `tests/checkpoint_host.rs`, so a move runs through the store's logic on both sides. A move is stopped after every number of calls of either store, until a stop it does not reach.

## Progress (2026-10-30)

- **Built** in a worktree by an agent of the storage session, for the session to review and merge. On this session's container (x86, TCG, 4 CPUs):
  - `tests/checkpoint_host.rs` (19 tests, 4 of them new) passes, also built with `-O -C debug-assertions=on`; `car`, `blockstore` pass; `scripts/host_tests.sh` whole exits 0;
  - the QEMU `store` suite (x86) passes, `tally_move_check` among it.
- **The tests can fail** (experiments on a copy of `checkpoint.rs`):
  - a release record restored as a fresh start fails two host tests;
  - a release that commits nothing, so the source goes on saving, fails three;
  - a manifest of another epoch taken at any version fails two host tests, the refusal of a republished one among them; taken at none, as before this task, it fails two, the move stopped anywhere among them.
- **Not yet:** the aarch64 run and the profile.

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

[313](313-persistent-actors.md), [313-STO-0026](../issues-done/313-STO-0026-operation-ids-and-queues-in-a-checkpoint.done), [313-STO-0030](../issues-done/313-STO-0030-messages-between-components.done), [309-STO-0031](../issues-done/309-STO-0031-a-copy-of-named-roots-in-a-file.done), [306](../issues-done/306-checkpoints-and-rebinding.done).
