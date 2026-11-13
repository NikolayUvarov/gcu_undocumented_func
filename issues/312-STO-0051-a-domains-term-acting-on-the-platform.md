# 312-STO-0051 — A domain's term acting on the platform, once `blockstore` hears a calendar clock

**Type:** test (storage) · **Owner:** `STO` track · **Priority:** P2 · **Status:** open · **Blocked by:** 312-KRN-0239, `KRN`'s `rtc` client for `blockstore` in slot 5, in the landing queue on 2026-11-13 · **Main task:** [312](../issues-done/312-the-minds-memory.done) · **Roadmap:** track B · **Constitution:** MC-4.11, Appendix B.3

Split from [312-STO-0050](../issues-done/312-STO-0050-a-memory-domains-term.done), whose store, interface and host tests are done.

## Problem

- **What is done:** a domain's term and mode are kept on the medium and set by their owner. The store acts on them by the calendar time it hears, forgetting, releasing or reporting a domain past its term. This is host-tested on a simulated clock.
- **What is run in QEMU:** a term set and read, and found again after a reboot. With no calendar clock heard, none acts.
- **What is not:** until 312-KRN-0239 is in `main`, `init` gives `blockstore` no `rtc` client, so on the platform no term has ever acted. The service's part is written: it reads slot 5 when it holds an endpoint, at most once a minute and at once after a set.

## Plan

Once `KRN` gives the client:
- the `store` suite (aarch64, where domains are made) sets the guest's clock (`date set`, the shell's), then three domains with a term a minute ahead, one in each mode;
- after it:
  - the forget domain is forgotten and its clients revoked;
  - the release domain refuses a put and a publish, and a collection frees its records while its key stays;
  - the report domain is told passed and keeps everything;
  - the log names each;
- the release domain's mode set back to report before a collection keeps its records;
- across a reboot (the `boot` suite's store disk on x86, with RDRAND), a term passed while the machine was off acts at the first minute after the mount.

## Acceptance criteria

- The QEMU checks above pass on x86 and aarch64.
- `docs/storage` ("Memory domains") says the term acts on the platform, with its evidence.

## Related

[312-STO-0050](../issues-done/312-STO-0050-a-memory-domains-term.done), [requests-KRN.md](requests-KRN.md), [111](111-network-time.md) (`timed`, who sets the clock).
