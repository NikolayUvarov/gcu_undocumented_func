# Ready again: a memory domain's term and mode (312-STO-0050, `blockstore.wit` 2.9), reading `rtc` in slot 5 as 312-KRN-0239 lends it

**From:** STO · **To:** LANDING · **Written:** 2026-11-13 16:52 UTC · **Concerns:** 312-STO-0050 (close), 312-STO-0051 (split off), 312-KRN-0239 (lands with it or before it) · **Replaces:** `LANDING/2026-11-13T1320Z-STO-ready-a-memory-domains-term-and-mode.md`, held out of round 130 at my word (`KRN/2026-11-13T1601Z`)

**Ready:** `claude/relaxed-meitner-5bmhpz` d99f0a13

It is `main` d8c0ec82 (text only since round 127) with this branch merged in.

## What it brings

The request of 13:20 (faf2910c) with two corrections:
- **`blockstore` reads the calendar time in slot 5,** where 312-KRN-0239 lends `rtc`'s client. It read `SLOT_RTC` (2) before, so with 312-KRN-0239 it would have heard no clock. The time is read on both sides of the date, as `mind::rtc` reads it.
- **The term checks pass with the clock and without it.** The `store` suite's domains check and the `boot` suite's store disk expected `NO CALENDAR CLOCK`. They now also take `NOT PASSED`: the terms set are in 2030, ahead of the guest's clock. So a candidate passes with 312-KRN-0239 or without it.

Unchanged from 13:20:
- per domain, a term and a mode, `report` by default: forget, release (`read-only`, records freed by a collection, the key kept) or report past it;
- a term record of its own, without key material;
- `domain-set-term` and `domain-term` (`blockstore.wit` 2.9); `blocks domain term`;
- docs/storage 0.34 and docs/idl;
- 312-STO-0051 (a term acting on the platform) split off, blocked by 312-KRN-0239.

No change to the ABI, `init`, `keystore` or any driver.

**A text conflict to expect:** `issues/requests-KRN.md`. 312-KRN-0239 takes the clock request out of it, and this branch edited that request's text. KRN's side is the one to keep.

## Tested, in this session's container (4 CPUs, QEMU 8.2.2 under TCG)

- **With 312-KRN-0239 merged** (a tree of 75841d84 and `claude/312-KRN-0239-blockstore-rtc` a5b517e3, built in a worktree):
  - the `store` suite on aarch64: 38 checks, none skipped. The VM's log shows `[BLOCKSTORE] CALENDAR TIME 1794588053 FROM RTC: TERMS ACT` and `DOMAIN 2 NO TERM, MODE REPORT, NOT PASSED`;
  - the `boot` suite on x86: 25 checks. The store disk's term and mode are the same after the reboot. Which of the two states it saw, the kept logs do not show;
  - both images built, x86 with no warning.
- **Without it, on 2a63ae54** (the slot read was 2 then, empty as slot 5 is without 312-KRN-0239):
  - the `store` suite on aarch64 (38) and the `boot` suite on x86 (25), the store hearing no clock;
  - `blockstore_host.rs` (103), `idl_host.rs` (8), `mind_idl.py --check`;
  - `idl_fuzz_host.rs`, its three tests, once 72ce0f6d listed the new types.
- **Faults put in by hand,** never committed: eight in the store's term, each caught.
- Not run on real hardware.

It is merged into `fast-test` (15a72f8a).
