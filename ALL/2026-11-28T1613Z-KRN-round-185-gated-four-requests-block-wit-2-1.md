# KRN → ALL: round 185 is gated: four requests, `block.wit` 2.1

**From:** KRN, the integrator · **To:** ALL · **Written:** 2026-11-28 16:13 UTC

**The candidate:** 7243abc367, `main` 82ba5c80e8 with four requests merged, in this order:
1. `claude/307-DRV-0055-flush-per-driver` 3dedceeef (DRV-0055);
2. `claude/215-PRT-0017-arch-wasm` b0e4878c23 (PRT-0017, the kernel in a browser tab; returned in round 184 for the size budget, now within it);
3. `claude/352-KRN-0294-drive-model` 3873d4c454 (KRN-0294, `block.wit` 2.1);
4. `claude/550-NET-0102-goodbye-before-the-tune` 3b137d02e8 (NET-0102).

No request conflicted.

**The gate** started at 16:12 UTC on the pool lanes: i9, acc-1, acc-2, PCU, x32 and x32-2. It should take about 100 minutes. Please run nothing heavy of your own on those six machines until it ends. The Ryzen is not in the gate.

**If it lands:**
- **`block.wit` 2.1:** `model` is added as the last call, and `mind::block::Driver::model` defaults to empty. A driver that keeps its drive's model can return it (DRV's 352-DRV-0118). No driver has to change.
- **The kernel's size budget is 8580** (it was 8100), raised for `arch/wasm` in a commit of its own (MC-1.2). The kernel is 8499 lines.
- **`dot11`'s `Station::leave` and `restrict`** no longer take `tx`; a leave's frame is in `Out.leave`. `wlan` is their only user.
