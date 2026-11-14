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
3. **S3, no destruction before confirmation.** What the previous slot needs stays as it was until the trial is confirmed. That covers:
   - `keystore`'s remembered networks and device key;
   - the agent's identity and token;
   - the network policy;
   - `link.conf`, `mode.conf` and the owner's policy.

   A conversion, if any, runs after confirmation.
4. **S4, each release is installable by the one before it** on every bench machine.
   - The running slot's updater accepts the next channel, manifest and keys. A change of signature or key format reaches the machines through a bridging release first.
   - A QEMU check on every landing round updates the previous round's release to the new one.
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
| `UPD` | the trial's criterion and fallback with its bound; the fallback's reason record; nothing destroyed before confirmation; installable by the previous updater, a bridging release, the QEMU "update from the previous release" check | S1, S2, S3, S4 |
| `KRN` | `init` confirms on the agent's signal (an `init.wit` call); 213-KRN-0122's `Unknown` no longer silences a radio that sent before (WRDS read on the DEXP, or the previous behaviour kept); an RTC in local time; the Ryzen's loader stops at `READ` (000-KRN-0124) | S1, S5, S8 |
| `EFF` | command expiry relative to the server; the clock from the server when far off; reconnect, and the network path restarted; the S2 report on the first connection; `update_agent` run on the bench (751-EFF-0023) | S2, S5, S7 |
| `NET` | the network up on whichever card is there; `timed` sets the clock; the `wlan` race between the automatic join and the restart check (DRV's message of 2026-11-13) | S5 |
| `APP` | `mode remote-update` and `effector policy allow <line>` in the shell (751, 750-APP-0093), so no rewrite is needed to switch them | S7 |
| `keystore`'s owner | remembered networks and the device key kept across format changes until confirmation | S3 |
| `LNK` | [801-LNK-0044](801-LNK-0044-the-cable-updates-everything-with-no-permission-words.md), [801-LNK-0045](801-LNK-0045-a-rollout-one-machine-at-a-time-and-the-cable-on-all.md), [801-LNK-0046](801-LNK-0046-each-machines-settings-kept-onto-any-new-disk.md) | S6, S8 |
| `DRV` | [801-DRV-0071](801-DRV-0071-the-bench-radios-keep-their-network-across-releases.md) | S5 |
| `ASR` | [801-ASR-0014](801-ASR-0014-the-stages-conditions-as-checks.md) | S1–S5 |

## A decision asked of the maintainer

S3 and S4 narrow the rule of 2026-10-25 ("no code that exists only so that an older version … keeps working … a machine that has it is written again by hand") for the connectivity path alone. A bridging release is allowed, and old state is kept until confirmation. Elsewhere the rule stands.

## Acceptance criteria

S1 to S8, each with its evidence named:
- QEMU for S1 to S5;
- the three bench machines for S6 to S8.

Then the maintainer rewrites every disk from `fast-test` and restarts every machine.

## Related

[751](751-system-updates-at-a-developers-request.md), [750](750-effector-agent.md), [800](800-mind-link.md), [213-KRN-0122](213-KRN-0122-platform-power-policy-for-the-wlan-device.md), [000-KRN-0124](000-KRN-0124-switching-between-linux-and-mind-core.md), [800-LNK-0026](800-LNK-0026-several-machines-on-one-line.md), [213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md).
