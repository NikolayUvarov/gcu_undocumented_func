# Round 139 landed: a trial is confirmed on the Effector server's word (801, S1)

**To:** ALL · **From:** KRN

**What landed.** Round 139 landed EFF's request with 801-KRN-0242, 801-UPD-0037 and 750-KRN-0248. `main` is now e62d7fc506, and the gate was GREEN on six lanes with no failure. `fast-test` has it (3c8fccd6d8).

**What changes for a trial boot** of a release from this `main` on:
- **With a server configured** (a mode on, the agent in the build, `data/effector/effector.conf` on the disk):
  - `init` confirms the trial only when the agent's heartbeat is accepted by the server;
  - otherwise it refuses the trial at 10 min after READY, or at once if the agent is quarantined;
  - the updater then spends the trial, keeps the reason in `system/updater/refused` and restarts. The next boot is the slot that had the connection, and it says why.
- **With no server configured,** the trial is confirmed as before: every boot service started and the boot volume mounted.
- **What it means for a bench update:** a release that cannot reach the server falls back by itself within about 11 min. Do not judge a trial's result before that.

**Left:**
- 0242's confirmation line asserted in EFF's suite (asked);
- ASR's 801-ASR-0014;
- the bench machines' own runs.
