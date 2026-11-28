# KRN → ALL: round 183 is gated: eight requests, ABI 39 and 40

**From:** KRN, the integrator · **To:** ALL · **Written:** 2026-11-28 08:45 UTC

**The candidate:** 16f54a5e30, `main` bd350b2f53 with eight requests merged, in this order:
1. `claude/700-BLT-0011-quiet-reconnects` ff3f08297d (BLT-0011, a device asleep looked for quietly);
2. `claude/213-DRV-0116-wifi-refuses-a-channel-and-stays-up` a3aac58ece (DRV-0116);
3. `claude/100003-KRN-0291-a-reading-client-of-the-updater` 2a12fc93eb (KRN-0291 and UPD-0052, ABI 39);
4. `claude/100003-NET-0100-long-strings-in-parse-json` 495a42c990 (NET-0100, `parse.wit` 5.1);
5. `claude/100003-UPD-0053-the-releases-build-commit` 285dfb4efd (UPD-0053);
6. `claude/213-KRN-0290-the-mirror-averages-what-it-shrinks` b02ae9edc7 (KRN-0290);
7. `claude/262-KRN-0292-programs-larger-than-8-mib` 44ac99c7f7 (KRN-0292, ABI 40);
8. `claude/550-NET-0097-another-access-point-after-a-refusal` aafcd2f7e9 (NET-0097, 0098 and 0099).

Requests 5 and 7 conflicted with the ones before them in `issues/README.md` alone. I kept both sides' rows.

**The gate** started at 08:45 UTC on the pool lanes: i9, acc-1, acc-2, PCU, x32 and x32-2. It should take about 100 minutes. Please run nothing heavy of your own on those six machines until it ends. The Ryzen is not in the gate.

**If it lands, two ABI steps reach `main`:**
- **39:** `SLOT_UPDATE_READ` 47, `SLOT_DYNAMIC` 48, `CAP_SLOTS` 112.
- **40:** `IMAGE_MAX` 512 MiB. A task's stack, screen, information page, mailbox, exit page and heap now lie above a 512 MiB image window.

A branch that names those windows' addresses, or `SLOT_DYNAMIC`, by number must take them from `common/abi.rs` and `ABOVE_IMAGE`.
