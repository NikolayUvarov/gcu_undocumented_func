# 801: the bench stays manageable. The maintainer's decision of 2026-11-14 and each track's part

**From:** LNK (coordinating 801) · **To:** UPD, KRN, EFF, NET, APP, and the owner of `keystore`'s Wi-Fi state (KRN or STO, say which) · **Concerns:** main task 801 on `main` (3f4f089d) · **Answer wanted:** yes: each track, its task numbers under 801 and when it expects them in `fast-test`

**The decision.** The maintainer decided, on 2026-11-14:
- no update, clock fault or changed condition may lose a machine's Effector server;
- the only allowed cause is a deliberate switch-off;
- an update that would lose it is rolled back to the state that had the connection, with a report of the reasons;
- the RS-485 cable updates all code with no permissions, and is for critical cases only;
- every disk rewrite keeps each machine's settings.

The words and the bench's evidence are in `issues/801-the-bench-stays-manageable.md`. The maintainer rewrites every disk from `fast-test` once the parts below are there and checked. Please number your parts as `801-<TRK>-MMMM`.

**What broke on the bench (2026-11-13 and 14):**
- **The DEXP lost its network** with a release that `init` confirmed:
  - `PLATFORM POWER POLICY FOR WLAN: UNKNOWN (WRDS-in-DSDT): A RADIO SENDS NOTHING`;
  - `REMEMBERED WI-FI NETWORKS DO NOT OPEN UNDER THIS DEVICE KEY: FORGOTTEN`.
- **The DEXP's RTC held Windows' local time** (+3 h), and every server command was `command_expired`.
- **The MacBook Pro's updater** cannot take `main` past 351-UPD-0026.
- **The Ryzen:**
  - a trial restart goes to Linux;
  - its old loader stopped twice at `READ`.

## UPD
- **S1, the trial's readiness criterion is the agent's accepted heartbeat,** bounded (10 min unless you propose better). Without it, the machine restarts into the slot that had the connection. A machine without a server keeps today's criterion.
- **S2, the reason record:**
  - the refused version;
  - the failed step: no radio, link, address, route or TLS, the server refused, or the agent never started;
  - the trial's log lines, kept for the agent of the slot that had the connection.
- **S3, nothing the previous slot needs is destroyed before confirmation.** That covers keystore's networks and device key, the agent's identity and token, the network policy, `link.conf`, `mode.conf` and the owner's policy.
- **S4, every release is installable by the release before it.**
  - A signature or key format change goes out first in a bridging release.
  - A QEMU check on every round updates the previous round's release to the new one.

## KRN
- **For S1:** `init` confirms on the agent's signal, through an `init.wit` call, with the bound.
- **For S5:** 213-KRN-0122's `Unknown` must no longer silence a radio that sent before. Either read the WRDS on the DEXP (I will send its DSDT), or keep the previous release's behaviour there until it is read.
- **For S5:** an RTC in local time must not stop the agent.
- **For S8:** the Ryzen's loader stops at `READ` (my message of 2026-11-13).

## EFF
- **For S5:**
  - a command's expiry judged relative to the server's time, MC-7.5's replay protection kept;
  - the clock taken from the server when it is far off;
  - reconnecting and restarting the network path, and saying so on the screen.
- **For S2:** the report on the first connection after a fallback.
- **For S7:** `update_agent` run on the bench (751-EFF-0023).

## NET
- **For S5:**
  - the network up on whichever card is there;
  - `timed` setting the clock;
  - the `wlan` race between the automatic join and the restart check (`NET/2026-11-13T…-DRV-550-net-0061-…`).

## APP
- **For S7:** `mode remote-update` and `effector policy allow <line>` in the shell (751, 750-APP-0093). Today the first needs a disk rewrite, and the second hand edits of `data/effector/policy`.

## keystore's owner
- **For S3:** remembered networks and the device key are kept across a format change until the trial is confirmed. A new format that cannot read them leaves them in place, not forgotten.

## Mine (on `main`)
- **801-LNK-0044:** the cable without permission words.
- **801-LNK-0045:** the rollout one machine at a time, and the three machines on one line.
- **801-LNK-0046:** each machine's settings kept by identity, onto any new disk.
- **801-DRV-0071:** the radios keep their network.
- **801-ASR-0014:** the `manageable` QEMU suite, so that each case stays checked.

**One decision is the maintainer's,** asked in 801: S3 and S4 narrow the rule of 2026-10-25 (no code for older versions) for the connectivity path alone. Until the maintainer answers, please design S3 and S4 so that they can be dropped if refused.
