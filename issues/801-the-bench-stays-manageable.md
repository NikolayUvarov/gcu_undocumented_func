# 801 — The bench stays manageable: no update or condition loses the Effector server, and a release that does is rolled back with its reasons

**Type:** main task · **Owner:** `LNK` (coordination; each track numbers its own part as `801-TRK-MMMM`) · **Priority:** P1 · **Status:** open (2026-11-14) · **Blocked by:** — · **Roadmap:** the development of MIND Core on real hardware; stage V (remote capabilities) · **Constitution:** MC-7.5, MC-9.3, MC-9.4, MC-9.9, MC-12.1, MC-12.3

## The maintainer's decision (2026-11-14)

In the maintainer's words, with an English rendering:
- «effector485 должен уметь обновлять весь код, любой абсолютно без всяких разрешений»: the cable updates all code, any of it, with no permissions.
- «Ни сбой часов, ни изменения условий не должны приводить к сбою связи и потери управляемости через эффектор. Единственная причина, по которой он может не работать - это только его сознательное отключение»: neither a clock fault nor changed conditions may cost the connection or control through Effector. The only reason it may not work is that it was switched off on purpose.
- «если ранее работало, то должно и работать. Система должна поддерживать связь с сервером эффектора … потеря расценивается как полный провал»: if it worked before, it must work. Losing the server counts as a complete failure.
- «Эффектор 485 будет использоваться только в критических случаях, и надо учитывать, что он не всегда доступен»: the RS-485 cable is for critical cases only, and may be absent.
- «Следующий критический этап - достигнуть состояния, когда никакое обновление не ломает наличие связи и приводит к откату к предыдущему состоянию, когда связь была с репортом причин»: the next critical stage is that no update breaks the connection. One that would is rolled back to the state that had it, with a report of the reasons.
- «потеря связи, которая работала ранее под предлогом "ой я не знаю мощности" и прочее - не оправдание. Приоритет - работоспособность. Это критический параметр для субстрата разума»: losing a connection that worked before is not excused by "the power is not known" or the like. Operability comes first; it is a critical parameter for the Mind's substrate.
- «Каждая перезапись должна сохранять локально настройки»: every disk rewrite keeps each machine's settings, and puts them on a new disk too. The MacBook Pro's USB disk is to be replaced by a stick for the testing period.

## Problem: what 2026-11-13 and 14 showed on the bench

- **The DEXP:** release 202611140747 (local tree 8f15853e) was confirmed by `init` while the machine had no network:
  - `[INIT] PLATFORM POWER POLICY FOR WLAN: UNKNOWN (WRDS-in-DSDT): A RADIO SENDS NOTHING` (213-KRN-0122);
  - `[KEYSTORE] REMEMBERED WI-FI NETWORKS DO NOT OPEN UNDER THIS DEVICE KEY: FORGOTTEN`.

  On 202610302359 it had joined CoolNet2 by itself. `init` confirms a trial when the boot services start, so nothing rolled it back.
- **The DEXP's clock:** after Windows, its RTC held local time (+3 h). Every server command came back `command_expired`, until the time was set by hand.
- **The MacBook Pro:** its updater (`main` 20fba2c5) cannot take `main` past 351-UPD-0026 (the new signature lines). Only a disk rewrite gets it there.
- **The Ryzen:**
  - a trial restart goes to Linux, Ubuntu being first in `BootOrder`;
  - the stick's loader of 2026-11-06 stopped twice at the `READ` mark (000-KRN-0124).
- **The server's update path (751):** `remote-update` is on nowhere, and the path has never run on a machine.
- **One RS-485 cable** was moved by hand between three machines.

## The stage's conditions

1. **S1, connectivity is the trial's readiness criterion** (MC-9.9).
   - A slot on trial is confirmed only after its Effector agent's heartbeat is accepted by the server. This is bounded in time: 10 min, unless the task proposes better.
   - A machine with no server configured keeps today's criterion.
   - Otherwise the machine restarts into the slot that had the connection. That rollback is a permitted one under MC-9.4: the previous slot, within the floor.
2. **S2, the report.** After such a fallback, the agent of the slot that had the connection reports to the server on its first connection:
   - the refused release's version;
   - the step that failed: no radio, no link, no address, no route, no TLS, the server refused, or the agent did not start;
   - the trial boot's log lines that show it.

   The trial's boot log stays on `log:`.
3. **S3, the old state is not touched.** What the previous slot needs stays as it was until the trial is confirmed. The maintainer: «старое состояние не трогается - это разве не очевидно?»; it is a plain requirement, not an exception to any rule. That covers:
   - `keystore`'s remembered networks and device key;
   - the agent's identity and token;
   - the network policy;
   - `link.conf`, `mode.conf` and the owner's policy.

   A conversion, if any, runs after confirmation.
4. **S4, no bridging release; a release that cannot connect is not a working release.** The maintainer: «Не переходный релиз. А если обновление не может подключиться к сети и/или серверу - то это обновление не рабочее и должно быть откачено к предыдущему слоту».
   - A release the running updater refuses leaves the machine on its slot, connected, and the refusal reaches the server.
   - A release that cannot reach the network or the server is rolled back by S1, whatever the cause: a format it cannot read, a rule, a driver.
   - The gate's check updates the previous round's release to the new one, and names a change that stops it.
5. **S5, only a deliberate switch-off stops the agent.** None of these may stop it:
   - **the device clock wrong by hours:** an RTC in local time, a flat battery. Commands are judged by time relative to the server (MC-7.5's replay protection kept), and the clock is set from the network;
   - **a platform table the release cannot read:** the radio keeps the behaviour the previous release had on that machine, until the table is read there. An unknown limit is no reason to stop a radio that worked (the maintainer's word above); a rule that would silence it is held back on that machine and logged, not enforced;
   - **Wi-Fi lost:** the wired card is used, and the network path is restarted. An agent that cannot reach its server says so on the screen and in the log, and keeps trying.
6. **S6, the cable recovers everything with no permissions** (the maintainer's word).
   - With the RS-485 converter attached, `effector485` can update every component and restart the machine, whatever `link.conf`, `mode.conf` and the owner's policy say. Every component means the loader, kernel, `init`, services and applications.
   - Releases stay signed (MC-9.x).
   - The threat model and the profile say that the cable is physical access (MC-12.3).
   - No step of the normal path needs the cable.
7. **S7, the network is the normal path.**
   - Bench machines take developer releases through the Effector server (751) with no hand at the machine.
   - S1 to S3 apply to them.
8. **S8, shown on the bench** (MC-12.1, 12.3):
   - **The machines:** the MacBook Pro, the Ryzen and the DEXP.
   - **The rollout:** one release after another, each to one machine. Its agent's return is awaited, then the next machine gets it.
   - **The checks:**
     - one release built to lose the network is rolled back on its own, with its report;
     - one rewrite of all three disks keeps each machine's settings, the MacBook Pro's onto a new stick.

## Parts, by track

| Track | Its part | Conditions |
|---|---|---|
| `UPD` | the trial's criterion and fallback with its bound; the fallback's reason record; the old state untouched before confirmation; a refused release leaves the machine as it was and says why; the QEMU "update from the previous release" check | S1, S2, S3, S4 |
| `KRN` | `init` confirms on the agent's signal (an `init.wit` call); 213-KRN-0122's `Unknown` no longer silences a radio that sent before (WRDS read on the DEXP, or the previous behaviour kept); an RTC in local time; the Ryzen's loader stops at `READ` (000-KRN-0124) | S1, S5, S8 |
| `EFF` | command expiry relative to the server; the clock from the server when far off; reconnect, and the network path restarted; the S2 report on the first connection; `update_agent` run on the bench (751-EFF-0023) | S2, S5, S7 |
| `NET` | the network up on whichever card is there; `timed` sets the clock; the `wlan` race between the automatic join and the restart check (DRV's message of 2026-11-13) | S5 |
| `APP` | `mode remote-update` and `effector policy allow <line>` in the shell (751, 750-APP-0093), so no rewrite is needed to switch them | S7 |
| `keystore`'s owner | remembered networks and the device key kept across format changes until confirmation | S3 |
| `LNK` | [801-LNK-0044](801-LNK-0044-the-cable-updates-everything-with-no-permission-words.md), [801-LNK-0045](801-LNK-0045-a-rollout-one-machine-at-a-time-and-the-cable-on-all.md), [801-LNK-0046](801-LNK-0046-each-machines-settings-kept-onto-any-new-disk.md) | S6, S8 |
| `DRV` | [801-DRV-0071](801-DRV-0071-the-bench-radios-keep-their-network-across-releases.md) | S5 |
| `ASR` | [801-ASR-0014](801-ASR-0014-the-stages-conditions-as-checks.md) | S1–S5 |

## The maintainer's answers (2026-11-14)

- **No bridging release** (S4).
- **The old state is not touched** (S3): that is plain, not an exception to the rule of 2026-10-25, which stands.

## Acceptance criteria

S1 to S8, each with its evidence named:
- QEMU for S1 to S5;
- the three bench machines for S6 to S8.

Then the maintainer rewrites every disk from `fast-test` and restarts every machine.

## Related

[751](751-system-updates-at-a-developers-request.md), [750](750-effector-agent.md), [800](800-mind-link.md), [213-KRN-0122](213-KRN-0122-platform-power-policy-for-the-wlan-device.md), [000-KRN-0124](000-KRN-0124-switching-between-linux-and-mind-core.md), [800-LNK-0026](800-LNK-0026-several-machines-on-one-line.md), [213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md).

## Progress (2026-11-15)

- **S1, in `main` since round 139** (`e62d7fc506`, gate GREEN):
  - **What `init` does now:**
    - it holds a trial while the Effector agent runs with a server configured, and confirms it only on the agent's `accepted` (801-KRN-0242, 801-EFF-0031);
    - otherwise it refuses the trial at the 10 min bound;
    - the updater keeps the reason in `system/updater/refused`, spends the trial and asks for the restart, so the next boot is the slot that had the connection (801-UPD-0037).
  - **QEMU evidence:**
    - the `boot` suite's agent that never connects is refused at the bound;
    - both `effector-update` suites' trials are confirmed with a server configured.
  - **On the bench machines:** not shown yet.
  - **Round 141** (`main` `b8b39e620e`): the `effector-update` suites check `init`'s confirmation line after the agent's `accepted` (801-EFF-0031), and 801-KRN-0242 is closed.
- **S1 to S3 together, in QEMU, since round 141** (gate GREEN):
  - **The `rollback` suites,** x86 and aarch64 ([801-UPD-0041](../issues-done/801-UPD-0041-a-release-that-loses-the-server-rolled-back.done)):
    - a release taken through the agent finds the server out of reach on its trial;
    - it is held for the bound and refused;
    - every file slot A reads is as it was, but the trial's own writes;
    - slot A is back on the server as the same agent, with the reason whole in its heartbeat (S2, [801-UPD-0042](../issues-done/801-UPD-0042-a-refused-trials-reason-reaches-the-server-whole.done), 801-EFF-0035), and answers the update `fell-back`.
  - **The kept update waits for `init`'s word** and changes nothing slot A reads (S3, 801-EFF-0034).
  - **The suite fails if the trial is confirmed without the server.**
  - **On the bench machines:** not shown yet.
- **S4, in QEMU, since round 145** (`0ecc5adffe`, gate GREEN):
  - a release the running updater refuses, sent through the agent, leaves the machine on its slot and on the server;
  - the command is answered failed with the updater's reason ([801-UPD-0043](../issues-done/801-UPD-0043-a-release-the-updater-refuses-leaves-the-machine-connected.done), the `rollback` suites);
  - the gate's "update from the previous release" check stays as it was (801-UPD-0038, 0040);
  - **on the bench machines:** not shown yet.
- **S5, in part:**
  - the station joins again a network a client gave it (801-NET-0084, round 138; the Ryzen's run left, DRV);
  - commands are judged by the server's time (801-EFF-0027, round 139);
  - a safe start leaves out only what the cut run of the same slot was starting, and brings it back by itself 120 s past `READY` (000-KRN-0217, round 140);
  - rtw89 takes no channel before the station's own tune (801-DRV-0071, round 141);
  - the station knows a fresh radio by the PID that answers it, whatever the radio reports of its channel (801-NET-0085, round 144). A bench machine's run is left (DRV).

## The DEXP's first release through Effector (S7, S8; 2026-11-17)

On the maintainer's word of 2026-11-17 («прогоняй на DEXP и MacBook, Ryzen занят пока что»). Evidence for the DEXP only (MC-12.1).

- **Before:** the DEXP online through hesere, in the maintenance mode with `remote-update` on and the test keys (its boot log: `REMOTE UPDATE ON WITH THE TEST KEYS`), slot A confirmed, version not known, floor 0, the key file of version 1 kept; up 261 142 s on `fast-test` 719549b1.
- **The release:** `main` c6a5d2206c built for x86_64, 111 files signed with the test boot key at version 1, published and bundled with `release.py` as `docs/update/publishing.md` says (29 037 802 bytes, SHA-256 `ca94eb6a…`; the key file the same bytes as the DEXP's).
- **What happened** (UTC; `update_agent` operation `20261117212530.979892591_000048`):
  - 21:25:36: the command taken. The owner's policy and `remote-update` let it through: it ran, not `denied`.
  - From then the server saw no heartbeat (`offline`) while the agent downloaded the bundle.
  - 21:33:55: one heartbeat, `shutting_down`: the agent's last before a restart `init` asked for, so the release was downloaded, checked and applied.
  - After that, nothing. At 22:25:56 the operation ended `timeout` (`agent did not acknowledge the operation before timeout_seconds elapsed`, 3600 s).
- **What this shows and does not:**
  - **Not S1 to S3 on the DEXP.** In MIND Core, any end of the trial brings the machine back within about 15 minutes: the kernel's restart at 120 s without `init`'s word, `init`'s fall-back 600 s past `READY`, then slot A. It did not come back in 50 minutes. So something outside those bounds stopped it, and it is not known what. The candidates:
    - the firmware started another entry at the restart: the DEXP runs Windows 11 from its own disk, and MIND Core from the stick (800-LNK-0037). Its `BootOrder` was the stick's first on 2026-11-08; if Windows is first now and MIND Core was started by `BootNext`, every restart MIND Core asks for goes to Windows;
    - the restart itself did not complete;
    - the firmware stopped at its menu or setup.
  - **A gap in the heartbeat:** about 8 minutes with none while the bundle downloaded. 801's "always connected" asks for a heartbeat throughout (EFF, asked in `requests-EFF.md`).
- **Held:** the MacBook gets no release until the DEXP's cause is known, as the same restart could take it off the network too. The release that loses the network (S8's first check) waits too.
- **Needed:** what the DEXP's screen shows (asked of the maintainer, 2026-11-17).
- **Back, 2026-11-18 about 09:41 UTC:**
  - the DEXP started again from slot A (`THE NEWER RECORD: SLOT B NOT CONFIRMED`);
  - its clock read 07:20:38, about 2 h 21 min behind the server's;
  - the agent answered the kept command `completed`: `fell back: slot A runs version 0, not 1; the updater: check: Missing`, and the operation's record holds that answer.
- **So:** the fall-back took a start from outside, very likely a person's (not confirmed).
  - Slot B's trial left no boot log, so it stopped before `log:` was mounted. The loader had counted its try.
  - Neither the trial's deadline nor `init`'s bound runs that early, and the DEXP has no chipset watchdog (`WATCHDOG: NONE (THE LPC BRIDGE'S ACPI I/O IS OFF)`). Nor does the MacBook (`NO_REBOOT HELD BY THE FIRMWARE`).
  - Taken as [801-KRN-0258](../issues-done/801-KRN-0258-a-trial-that-hangs-before-init-needs-a-watchdog.done).
  - Why the release stopped there is not known: nothing of that boot was kept.
- **Also seen in that boot:** `data/effector/effector.conf` names `mind-macbook-pro`, not this machine (`mind-n100`); the agent logs it and goes on under its own name.

## The DEXP: a release that loses the network, rolled back (S8, 2026-11-19)

MC-12.1: this machine, this network.

**The release.** Release 15 was release 14's build (801-KRN-0259's kernel, 801-NET-0088's `netstack`) with `netstack.elf` left out, signed as usual. It went through `update_agent` while slot A ran release 14.

**What happened:**
1. The download, check and staging took 18:51:03 to 18:56:18 UTC, and the machine restarted into slot B's trial.
2. Slot B had no network, so the agent said nothing to the server.
3. At `init`'s 10-minute bound the trial was refused, the updater spent its tries, and slot A booted again.
4. The agent was back at 19:07:35, 11 min 17 s after the restart. Its heartbeat carried `rolled_back`, build 15, and the reason. It answered the command:
   ```
   fell back: slot A runs version 14, not 15; the updater: version 15 in slot B, falling back to slot A: NO HEARTBEAT ACCEPTED IN 600 S: THE AGENT SAID NOTHING
   ```

Nobody touched the machine.

**S8's first check is shown on the DEXP:** a release built to lose the network is rolled back on its own, with its report. The other machines of S8 are not done:
- the MacBook gets no release yet;
- the Ryzen is busy.

The rewrite of all three disks is also left.

## S1: every stop of a trial falls back (2026-11-19)

[801-KRN-0261](../issues-done/801-KRN-0261-every-stop-of-a-trial-falls-back-and-is-recorded.done), round 154 (`main` `da1ee7647b`).
- **What was wrong.** Only a panic on a trial restarted into the fallback. An exception in the kernel, `init`'s exit or an interrupt with no handler stopped the machine with its watchdog stopped, and only a person could bring it back.
- **Now:** all four restart a trial into the fallback, and leave a record that the next boot names.
- **Evidence:** QEMU, x86, a panic.
- **Left:** the record reaching the server (S2) is [801-UPD-0044](801-UPD-0044-a-trial-the-kernel-stopped-names-its-fault-in-the-fallback.md).
