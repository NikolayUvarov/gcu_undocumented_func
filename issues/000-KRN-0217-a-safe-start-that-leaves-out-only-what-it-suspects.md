# 000-KRN-0217 — A safe start that leaves out only what it suspects, and brings it back by itself

**Type:** `init`, the run's mark (firmware variables), the kernel's `BootInfo` (ABI 34) · **Owner:** kernel session · **Priority:** P1 · **Status:** in progress (2026-11-15) · **Blocked by:** — · **Roadmap:** track A; 801 S5 · **Constitution:** MC-6.1, MC-6.3, MC-9.4, MC-12.2, MC-12.3

## Problem

Asked by LNK for the maintainer (`KRN/2026-11-08T1513Z-LNK-a-safe-start-that-leaves-out-only-what-it-suspects.md` on `internal-dev-messages`). It refines 000-KRN-0076's steps 2–3.

**On the DEXP N100** (2026-11-08, build `d83c98c5`):
- **`boot0007.log`:** the run joined CoolNet2 and reached hesere. After 43 s the maintainer cut the power, before a minute past `READY`, so the run's mark stayed set.
- **`boot0008.log`:** a safe start. `bcm_wifi`, `video_gw`, `audio_gw`, `tts`, `rtw89_wifi`, `rtw88_wifi` and `bt_hci` were not started, and `wlan` was left out with them.
- **Its effect:** no network, so neither Effector nor the link could reach the machine; the screen alone said why.
- **Its cause was wrong:** every service had started and the network worked, so nothing pointed at a driver.
- **The watchdog** is `NONE` on this machine (the LPC bridge's ACPI I/O is off), so nothing told a freeze from a cut power.

**Why it happens.** `init` keeps one mark for the whole run (`RUN_MARK_NAME`) and clears it a minute past `READY`; the kernel clears it at every halt, restart and power-off asked of it. A run that ends before that minute otherwise, a cut power among them, is taken for a freeze, and the whole of `SAFE_LEFT_OUT` is left out.

## Plan

1. **A mark for each service of `SAFE_LEFT_OUT`.**
   - Set while it starts; cleared when it reports ready, or when 000-KRN-0075's limit ends its start. The run's own mark stays as it is.
   - Kept in one firmware variable (a bit per service), to bound the writes: one when a start begins and one when it ends.
2. **After a run that did not end as asked:**
   - **only the services whose own mark is still set are left out.** `bcm_wifi`, hung in its stage 2 on 2026-10-11, is that case;
   - **when every service was ready,** as on the DEXP, the boot is a normal one, with one line: the last run ended early, and nothing points at a service.
3. **A freeze is evidence; a cut power alone is not.**
   - **Evidence:** a run that the watchdog restarted (`IT RESTARTED THE LAST RUN`), or that ended in a panic. That leaves out what step 2 names at once.
   - **Without it,** a service is left out only after its start was cut short in two runs in a row: a count kept in the same variable.
4. **Back by itself.**
   - A safe start that left something out starts it again once the machine has run 5 minutes past `READY` without a fault.
   - One service at a time, each with its mark, and a line each, so that a freeze then names that one service at the next boot.
5. **Said where it can be read without a screen:** the safe start and what it left out, in Effector's telemetry and in the link's log. A machine with no network then says so the next time it has one, and a machine reached by the cable says so at once. Those are EFF's and LNK's parts, asked of them once the lines exist.

## Acceptance criteria

The `watchdog` suite's two boots exist already. Three cases in QEMU, with `system_reset` standing for a cut power:
- **a run cut after every service was ready** starts normally;
- **a run cut while a test service was still starting** leaves out only that one, after two such runs without evidence, or at once after the watchdog;
- **a left-out service comes back** after the stable time.

And: a power-button shutdown within a minute of `READY` is followed by no safe start (with 000-KRN-0216).

## Progress (2026-11-15)

Taken up again for 801 S5 at the maintainer's word of 2026-11-15: a machine whose connection is on is never kept off the network by something a module could not read or a suspicion without evidence. On branch `claude/000-KRN-0217-the-safe-start-keeps-the-network`.

**Why now.** The old rule leaves every radio out after any early end, so a bench machine reached only by Wi-Fi drops off the Effector server:
- after a cut power, as on the DEXP;
- after the kernel's restart of a trial at its deadline, which does not clear the mark. The rollback that follows then boots the slot before without its radio.

**What is made:**
- **The run's mark** (`RUN_MARK_NAME`, format version 2) holds the slot whose run set it and a bit for each service of `SAFE_LEFT_OUT`. Each start of one is marked before it runs and stays marked until it settles, `SETTLE_MS` (30 s) past both its start and `READY`.
  - The writes: one at boot, one at each start, one when the starts settle, one when the mark is cleared.
  - Version 1's one byte is read as a mark that names nothing: a normal start.
- **The decision** (`init/src/safe.rs`, pure):
  - no mark: nothing is left out;
  - a mark of the other slot: a normal start, because what slot B's trial was starting is not slot A's to suspect (S3);
  - no start unsettled: a normal start (the DEXP's case);
  - unsettled starts with the watchdog's evidence: those are left out at once;
  - unsettled starts without evidence: they are suspected, and left out only when the run before suspected them too.
- **The evidence:** the kernel sets `BOOT_WATCHDOG_RESTARTED` in the tasks' `BootInfo.boot_flags` when the TCO or FCH watchdog restarted the last run (ABI 34; aarch64's watchdog keeps no such status). The kernel now arms the watchdog just before it starts `init`, not with the tick a moment later: the first run on `x32` (`2b3fb199c2`) showed `init`'s copy was written before the watchdog had read its status.
- **Back by itself:** what a safe start left out starts again from `RETURN_MS` (120 s) past `READY`.
  - One at a time, each once the start before settled, the radios first, and `wlan` after its radio.
  - The 5 min of the plan became 120 s, so that a trial's agent still reaches its server within 801-KRN-0242's 600 s.
  - No fault of another service keeps them out.
- **The mark is cleared** a minute past `READY`, once every start settled and what was left out came back.
- **The lines:**
  - `[INIT] SAFE START: …` keeps the prefix the shell reads (000-APP-0086). Each line fits `logd`'s 200-byte record with all seven names;
  - each normal start after an early end says why it is one;
  - `[INIT] EVERY START OF THE SAFE START'S LIST SETTLED: …`;
  - `[INIT] <service> STARTED AGAIN BY ITSELF <n> S PAST READY, …`.
- **Tests:**
  - `tests/safe_start_host.rs` covers the decision, the format and the return's order.
  - **The `watchdog` suite:** only what the stopped run was starting is left out. One service is started by hand, and the others must come back by themselves at 120 s or later.
  - **The power button's check:** a reset once the list settled is followed by a normal start. Two resets in a row before it settles are followed by a safe start.
  - **The link and modes suites** reach their safe start by two resets.
  - **The `boot` suite's trial check:** a trial of slot B held for the server is cut twice at the prompt, which spends its tries. Slot A then starts normally and names slot B's run.

**The runs before landing** (the code at `8929b024f3`, the tests' last fixes at `1b2c788582`):
- x86: the watchdog, on i9, 253 s;
- x86: boot, display, shell, memory, clock, on x32, 1466 s, and on i9, 1595 s;
- x86: services, storage, audio, on acc2, 1043 s;
- aarch64: the link, 229 s, and aarch64: the updater and the modes, 1167 s, both on acc1;
- host tests, on x32, 2204 s.

**What the runs found and fixed:**
- the kernel started `init` before it read the watchdog's status (`2b3fb199c2`, x32);
- the trial kernel's 15 s deadline restarted slot B before `init` held it, on i9;
- the trial check typed `ps` while the refused trial restarted, on x32.

**Landed in round 140** (`main` `e919c24512`, 2026-11-15). The gate was green on the pool, with one failure on i9 that was not this task's:
- **The failure:** x86: network, TLS and Effector. The Effector agent was killed after `init`'s stop notice, 5 s after its restart. A request the suite's outage had left hanging held its pass past the notice's 2 s.
- **Whose:** the agent's, and EFF took it as 801-EFF-0036. Neither `init`'s notice nor the agent changed in this task.
- **The repeat:** the group alone on x32 passed, in 1671 s.

**Left:**
- a panic as evidence (step 3), not done;
- a bench machine's run;
- step 5 (Effector's telemetry and the link's log), EFF's and LNK's to ask.

## Related

[801](801-the-bench-stays-manageable.md) (S5), [801-KRN-0242](../issues-done/801-KRN-0242-a-trial-confirmed-on-the-effector-servers-word.done), [000-KRN-0076](000-KRN-0076-a-frozen-machine-restarts-and-starts-safely.md), 000-KRN-0075 (`init` waits for every service with a limit), 173-KRN-0036 (a safe start asked by a key), [000-KRN-0216](000-KRN-0216-the-power-button-shuts-down-cleanly.md).
