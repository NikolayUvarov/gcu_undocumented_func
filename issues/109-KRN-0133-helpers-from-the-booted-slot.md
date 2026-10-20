# 109-KRN-0133 — The loader starts its helpers from the booted slot's copy

**Type:** `loader` · **Owner:** kernel session · **Priority:** P2 · **Status:** made, in `fast-test` (2026-10-20); the gate is left · **Blocked by:** — · **Main task:** 109 (parsers) · **Constitution:** MC-3.11, MC-11.11, Appendix B.6

## Problem

The update track recorded this on 2026-10-20 ([requests-KRN.md](requests-KRN.md), "The loader starts no helper on a disk with boot slots"), for [109-UPD-0017](109-UPD-0017-the-updaters-checks-in-their-own-parser.md) and [109-NET-0010](109-NET-0010-a-parser-per-session.md).
- **The failure.** On a disk with boot slots (351-UPD-0016), the boot services are only in `MIND/A/` or `MIND/B/`. `loader`'s `spawn_helper` opened `parse.elf` at the volume's root, so it answered `not-found`.
- **The cost.** The updater's checks, and every `download`, fell back to the shared parser (`[UPDATER] PARSER: THE SHARED SERVICE (Loader(NotFound))`).

## Plan

- **The booted slot's copy.** A helper is a boot service. When `BootInfo.boot_slot` names slot A or B, `loader` opens it in `MIND/<slot>/`; on a volume without boot records, at the root as before.
- **Why that copy.** It is the one the bootloader verified against the manifest at this boot.
  - No client can write it: `vfs_server`'s update zone is the other slot.
  - The root's copy, which a slot disk does not verify, is not used.
- **Not chosen:** starting the helper from the boot image `init` holds. It needs no file read, but a memory capability of a boot image would have to go from `init` to `loader`. That is a larger change for the same guarantee.

## Acceptance criteria

- [ ] On the `update` suite's slot disk the updater logs no `PARSER: THE SHARED SERVICE`, booted from slot A and, after the update, from slot B. The suite checks both.
- [ ] The update track puts back its check: the server holds the channel's request, one `parse` beside the shared one holds only its endpoint (`stat caps`), and it is gone with the check.

## Progress (2026-10-20)

`spawn_helper` puts `MIND/A/` or `MIND/B/` before the helper's file name from `BootInfo.boot_slot`. The `update` suite asserts the updater's own parser on both slots. Built; in `fast-test`; the suites run after the merge.

## Related

[109-KRN-0117](109-KRN-0117-a-loader-client-that-only-starts-helpers.md), [109-UPD-0017](109-UPD-0017-the-updaters-checks-in-their-own-parser.md), [109-NET-0010](109-NET-0010-a-parser-per-session.md), [docs/update/slots.md](../docs/update/slots.md).
