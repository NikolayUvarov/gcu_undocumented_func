# 109-KRN-0133 — The loader starts its helpers from the booted slot's copy

**Type:** `loader` · **Owner:** kernel session · **Priority:** P2 · **Status:** in `main` (8def173, 2026-10-21); slot A shown; slot B waits for the parser sessions' race · **Blocked by:** [requests-NET.md](requests-NET.md) and [requests-UPD.md](requests-UPD.md) ("A parser session ends before its helper does") · **Main task:** 109 (parsers) · **Constitution:** MC-3.11, MC-11.11, Appendix B.6

## Problem

The update track recorded this on 2026-10-20 ([requests-KRN.md](requests-KRN.md), "The loader starts no helper on a disk with boot slots"), for [109-UPD-0017](../issues-done/109-UPD-0017-the-updaters-checks-in-their-own-parser.done) and [109-NET-0010](../issues-done/109-NET-0010-a-parser-per-session.done).
- **The failure.** On a disk with boot slots (351-UPD-0016), the boot services are only in `MIND/A/` or `MIND/B/`. `loader`'s `spawn_helper` opened `parse.elf` at the volume's root, so it answered `not-found`.
- **The cost.** The updater's checks, and every `download`, fell back to the shared parser (`[UPDATER] PARSER: THE SHARED SERVICE (Loader(NotFound))`).

## Plan

- **The booted slot's copy.** A helper is a boot service. When `BootInfo.boot_slot` names slot A or B, `loader` opens it in `MIND/<slot>/`; on a volume without boot records, at the root as before.
- **Why that copy.** It is the one the bootloader verified against the manifest at this boot.
  - No client can write it: `vfs_server`'s update zone is the other slot.
  - The root's copy, which a slot disk does not verify, is not used.
- **Not chosen:** starting the helper from the boot image `init` holds. It needs no file read, but a memory capability of a boot image would have to go from `init` to `loader`. That is a larger change for the same guarantee.

## Acceptance criteria

- [ ] On the `update` suite's slot disk the updater logs no `PARSER: THE SHARED SERVICE`, booted from slot A and, after the update, from slot B. The suite checks slot A; slot B's check comes back with the sessions' fix.
- [ ] The update track puts back its check: the server holds the channel's request, one `parse` beside the shared one holds only its endpoint (`stat caps`), and it is gone with the check.

## Progress (2026-10-20)

`spawn_helper` puts `MIND/A/` or `MIND/B/` before the helper's file name from `BootInfo.boot_slot`. The `update` suite asserted the updater's own parser on both slots; the slot B check is withdrawn below. Built; in `fast-test`.

## Progress (2026-10-20, gate g0134)

- **Slot A works.** In gate g0134 and on the kernel session's machine, the updater gets a parser of its own when booted from slot A.
- **Slot B fails now and then.** Once in three runs (i9) the updater, booted from slot B, logged `PARSER: THE SHARED SERVICE (Endpoint(Other(ERR_LIMIT)))`.
  - At its start it reads the channel kept in each slot (`kept_version`). On slot B both slots hold one, so it opens two parser sessions one after the other.
  - A session's drop (`parse::end_session`) asks the helper to stop and drops the updater's endpoint capability, but does not wait for the helper to end. The helper's copy keeps the endpoint alive, and the endpoint counts against the updater's quota: `init` gives it one.
  - So the second `Endpoint::create` fails when the first helper has not ended yet. Before this task no helper started on a slot disk, so the race could not happen.
- **Recorded for the owners:**
  - NET: a session's drop waits until its helper has ended;
  - UPD: the updater reads both channels in one session.
- **The suite.** Its check of the slot B boot is not part of this task's change until then. The slot A check stays.

## Progress (2026-10-21)

In `main` (8def173, gate g0138): the updater gets a parser of its own when booted from slot A. Slot B waits for the network or update track's fix of the sessions' race.

## Related

[109-KRN-0117](109-KRN-0117-a-loader-client-that-only-starts-helpers.md), [109-UPD-0017](../issues-done/109-UPD-0017-the-updaters-checks-in-their-own-parser.done), [109-NET-0010](../issues-done/109-NET-0010-a-parser-per-session.done), [docs/update/slots.md](../docs/update/slots.md).
