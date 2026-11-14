# 801-KRN-0242 — A trial confirmed on the Effector server's word: `init` holds it for the agent's accepted heartbeat

**Type:** kernel (`init`, `init.wit` 1.13) · **Owner:** `KRN` · **Priority:** P1 · **Status:** open (2026-11-14) · **Blocked by:** EFF: the agent's `agent-step` call · **Main task:** [801](801-the-bench-stays-manageable.md) (S1) · **Roadmap:** stage V · **Constitution:** MC-9.4, MC-9.9, MC-12.4

## Problem

**The DEXP lost its connection on 2026-11-14 and stayed on the new release.** `init` confirms a trial once every boot service started and the boot volume mounted (351-KRN-0014). Nothing asks whether the machine can still reach its Effector server. A release whose agent cannot connect, for whatever reason (a file a module cannot read, a driver, a key), is confirmed and kept.

The maintainer (801, S1 and S4): a slot on trial is confirmed only after the server accepts its agent's heartbeat, within a bound. A release that cannot connect is not a working one and is rolled back to the slot that had the connection.

## Plan

- **`init.wit` 1.13** (an explicit transition: the record `boot` grows, a call is added):
  - `boot` answers at once, with `deciding` while a trial waits, `why` once it is refused, and `log`, this boot's `log:bootNNNN.log`.
  - `agent-step(step, detail)`: the agent's word on its way to the server: `started`, `no-network`, `no-tls`, `refused`, `accepted`. Only the running agent's own client (`BADGE_NOTICE`) may say it.
- **Who waits.** A trial where the Effector agent runs with a server configured: a mode on, the agent in the build, and `data/effector/effector.conf` on the disk. `init` judges by the file's presence, not by what this release makes of it. A machine with no server configured keeps today's criterion (S1).
- **The kernel's deadline is lifted as before** (every boot service started, the boot volume mounted), so the 120 s guard does not restart a trial that waits. The trial is then held:
  - `accepted` confirms it: `[INIT] TRIAL BOOT CONFIRMED: THE EFFECTOR SERVER ACCEPTED THE AGENT'S HEARTBEAT N S AFTER READY`;
  - the agent quarantined refuses it at once, and so does 10 min after READY without `accepted`, the agent's last word given as `why`;
  - the owner turning the modes off stops the agent and confirms by today's criterion.
- **A refused trial restarts the machine.** The updater spends it first (801-UPD-0037) and asks for the restart; `init` restarts it itself 30 s after the refusal otherwise.
- **The agent's call is EFF's** (requested in `EFF/`): `agent-step(accepted)` after the first heartbeat the server accepts, and the other steps as it meets them. Until it is in, every such trial is refused, so this lands with it.

## Acceptance criteria

- **The `boot` suite:** a trial with the development mode on and a server configured, whose agent cannot start, is refused once the agent is quarantined. The kernel's 15 s deadline does not restart it; the next boot is slot A's.
- **With EFF's call:** a trial whose agent reaches the fixture server is confirmed on `accepted` (the `effector-update` suites), and one whose agent never reaches it is refused at the bound (801-ASR-0014).
- The groups the change touches pass on the maintainer's machines, then in a gate.

## Related

[801](801-the-bench-stays-manageable.md), [801-UPD-0037](801-UPD-0037-a-refused-trial-spent-and-its-reason-kept.md), [351-KRN-0014](../issues-done/351-KRN-0014-trial-boot-and-confirmation.done), [750-KRN-0194](750-KRN-0194-init-warns-a-service-before-it-stops-it.md), [801-ASR-0014](801-ASR-0014-the-stages-conditions-as-checks.md).
