# 312-KRN-0239 — A calendar clock for `blockstore`: an `rtc` client that only reads

**Type:** kernel track (`init`) · **Owner:** `KRN` · **Priority:** P2 · **Status:** in progress · **Blocked by:** — · **Main task:** — ([312-STO-0050](312-STO-0050-a-memory-domains-term.md)) · **Roadmap:** track B · **Constitution:** MC-4.11, Appendix B.3

## Problem and plan

STO's request of 2026-11-13 (from `requests-KRN.md`), for [312-STO-0050](312-STO-0050-a-memory-domains-term.md), a memory domain's term. The request as it was written, kept whole:

**Asked by** STO on 2026-11-13, for [312-STO-0050](312-STO-0050-a-memory-domains-term.md), a memory domain's term, as the maintainer chose it that day.

- **What there is:**
  - `blockstore` keeps the monotonic time of its boot only (`mind::time::monotonic_ns`), for leases;
  - calendar time is `rtc`'s (`rtc.wit` 1.3 `now`, `date`), set by the shell's `date set` or by `timed`.
- **Asked:** `init` gives `blockstore` a client of `rtc` without the setting badge, in a slot `KRN` chooses, so that it reads `now` and `date`. It sets nothing.
- **Why:** a domain's term is a calendar time. Past it, the store reports, releases or forgets the domain, as the domain's owner set.
- **Without it,** as today, the store acts on no term: every domain acts as in report, and the log says why.
- **Then STO** reads it at most once a minute, and checks a term's end across a reboot in QEMU.

## Acceptance criteria

- `blockstore` holds an `rtc` client without the setting badge in slot 5 when `rtc` runs, and starts without it, slot 5 empty, when `rtc` does not.
- STO's check named above, run by STO once this is in.
- The tests the change touches pass on x86 and aarch64 in QEMU; `docs/profile/bootstrap.md` names the slot.

## Progress

**Made (2026-11-13)** on `claude/312-KRN-0239-blockstore-rtc`:
- **`init`:** `blockstore`'s slot 5 (`SLOT_STORE_RTC`) holds a copy of `init`'s own `rtc` client, which carries no setting badge, when `rtc` runs or is prepared. It is lent as the updater's and `keystore`'s are, so `rtc` refuses it `set`.
- **Docs:** `docs/profile/bootstrap.md`, `blockstore`'s row.
- **Tested** (this container): `init` builds for x86 and aarch64 with no warning.
- **Left:** the QEMU groups that boot the store, then the gate; STO's reading of it.

## Related

[requests-KRN.md](requests-KRN.md), [312-STO-0050](312-STO-0050-a-memory-domains-term.md), [309-KRN-0209](../issues-done/309-KRN-0209-a-second-store-disk-for-the-block-stores-copy.done) (slot 4).
