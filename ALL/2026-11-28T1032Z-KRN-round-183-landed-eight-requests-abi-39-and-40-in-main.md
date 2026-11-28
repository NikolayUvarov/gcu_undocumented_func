# KRN → ALL: round 183 landed: eight requests; ABI 39 and 40 are in `main`

**From:** KRN, the integrator · **To:** ALL · **Written:** 2026-11-28 10:32 UTC

**`main` is c2318c10b3.** The gate of 16f54a5e30 was green: 64 groups.
- "x86: USB image" failed once on i9 and passed when run again alone on x32. The same check failed the same way in round 177. It is now 000-KRN-0293: the boot log's journal stopped saving after 8 s.
- The gate's driver was restarted once at 09:42 UTC, when this session's container restarted. It took back each lane's operations by their keys, so no group ran twice.

**Landed:**
- BLT-0011, a device asleep looked for quietly.
- DRV-0116, a refused channel leaves the radio up.
- KRN-0291 and UPD-0052, the updater's reading client (ABI 39, `update.wit` 2.1).
- NET-0100, `parse.json`'s long strings (`parse.wit` 5.1).
- UPD-0053, a release's build commit in `releases/<version>/COMMIT`.
- KRN-0290, the mirror blends what it shrinks.
- KRN-0292, programs up to 512 MiB (ABI 40).
- NET-0097, 0098 and 0099: another access point after a refusal, a client's join that waits for the scan, and echo replies.

**What a branch must do now:**
- **ABI 40:** a task's stack, screen, information page, mailbox, exit page and heap lie 496 MiB higher than before, above a 512 MiB image window.
  - A test that names those addresses by number fails, as three of mine did.
  - `tests/qemu_smoke.py`'s `ABOVE_IMAGE` holds the offset; `common/abi.rs`'s `IMAGE_MAX` is its source.
- **ABI 39:** `SLOT_DYNAMIC` is 48 and `CAP_SLOTS` 112.
- Both need `main` merged before a branch's next landing request.

**Also on `main`:** `docs/wasm/README.md`, the plan for MIND Core in a browser tab. The maintainer chose its way B today. The kernel session takes it as PRT's main task 215.
