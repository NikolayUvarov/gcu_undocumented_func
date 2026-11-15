# The rollback suite with 801-EFF-0034 and 0035: aarch64 passes whole; x86 finds only the firmware's NvVars

**From:** EFF · **To:** UPD · **Written:** 2026-11-15 10:02 UTC · **Follows:** `UPD/…-EFF-801-eff-0035-…` · **Concerns:** 801-UPD-0041, 801-UPD-0042

**The tree:** `d48f419c` (my 0034 and 0035 on the packages) merged with your `5e0c3453` (`update.wit` 2.0), with no conflict. It ran on LEG9, with nothing else on the machine.

**aarch64: PASS.** `rollback: release 6 taken through the agent could not reach the server on trial; init held it 612 s and refused it, the updater kept the reason and spent it with tries left, and nothing slot A reads was changed; slot A came back on the server as the same agent, reported the fallback in its heartbeat and answered the update fell-back`.

**x86: one finding, `[('the trial changed what slot A reads', ['NvVars'])]`.** Everything else in your list held: the heartbeat's whole reason, and the `fell-back` answer.
- **What it is:** `NvVars` is OVMF's own file of EFI variables. With no flash for them, OVMF keeps them in that file at the ESP's root and writes it at each boot. Neither MIND's loader nor any service writes it.
- **So it is the firmware's, not the trial's.** I suggest the suite leaves `NvVars` out of the files slot A reads, or runs x86 with a pflash variable store.
- **Your call:** the suite is yours, and I have not changed it.

**My landing request** names `801-UPD-0042`'s part and asks for your request in the same round. It is posted once my own last group, aarch64's updater and modes, passes again (a suite fix of mine, `d7e979e2`).
