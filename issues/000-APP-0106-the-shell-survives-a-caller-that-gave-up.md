# 000-APP-0106 — The shell survives a caller that gave up, and gives `wm` its screen back after a restart

**Type:** tools (`shell`, `wm`) · **Owner:** tools track (`APP`) · **Priority:** P1 (the maintainer: "`wm` must not fall in such cases", 2026-10-22) · **Status:** open: the shell's part done on the tools branch; ASR's `giveup` suite left · **Blocked by:** — for the shell's part; the rule for every server is in `main` (000-KRN-0152, e93c47af); ASR's suite 500-ASR-0012 is not in `main` yet · **Main task:** — · **Roadmap:** the development of MIND Core on real hardware · **Constitution:** MC-3.11, MC-12.1, MC-12.3

**Recorded by:** the tools track, 2026-10-22, from LNK's report (`APP/2026-10-22T1601Z-…`: the fault answering `net-counters`), EFF's report (`APP/2026-10-22T1639Z-…`: the fault after `netpolicy add` in a script) and ASR's request (`APP/2026-10-22T1606Z-…`).

## Problem

- **The fault.** On the MacBook Pro the shell faulted in `wire::reply_buffer` writing `net-counters`' answer: `VECTOR=14 ERROR=0x6`, a write to a page no longer present.
- **Why.**
  - `wm` asks for the counters every second and gives up after 250 ms (`NET_SHELL_MS`).
  - A call it gives up on is revoked by `wire::call_buffer`. The kernel then clears the shell's copy of the lent buffer and unmaps it (`scheduler.rs` `revoke`).
  - A shell already inside the call, slow while a card changes or the policy is written, then wrote into nothing.
- **The screen.** `init` restarted the shell. `wm` kept running, but the new shell kept the screen, and the maintainer lost `wm`.

## Plan

1. **The shell checks before it writes.** Before it answers `run`, `boot-notice` or `net-counters` into the caller's buffer, it checks that it still holds the lent capability.
   - If the capability is gone, the call is dropped unanswered.
   - The shell logs it: the first time, then at each power of two.
2. **A restarted shell finds the `wm` that runs.** With exactly one program called `wm` at its start, it gives that `wm` the screen (`fg`). At a boot none runs yet.
   - It does not make that `wm` its manager. The clock, the zone, the radio and the counters stay for the `wm` the shell itself starts, because a name proves nothing.
3. **The start script once a boot** (EFF's second report, `APP/2026-10-22T1803Z-…`: a restarted shell ran `autorun.msh` again, and its Wi-Fi join left the machine without a default route for 16 s).
   - The shell asks `init` how often it started the shell this boot (`init.wit` `starts`).
   - From the second start on it runs no `autorun.msh` and says so. It also gives `wm` the screen only then.
4. **Left to `KRN`'s rule.**
   - The moment between the check and the write.
   - Every other server.
   - Whether `wm` keeps its buffer until the answer (ASR's item 2). `wm` lends it through the generated `net_counters`, and keeping it needs `wire`'s help.

## Acceptance criteria

- **The `wm` suite:**
  - the shell is killed while `wm` runs;
  - `init` starts it again, and the new shell logs that `wm` has the screen again;
  - `wm`'s close-all key works;
  - the second start ran no start script.
- **The caller that gives up:** ASR's `giveup` suite (500-ASR-0012) checks the server goes on, once `KRN`'s rule lands.
- **Docs:** `docs/tools/README.md` and `README_RU.md`.

The suite shows the behaviour on QEMU only (MC-12.1). On the MacBook Pro it is shown when the maintainer's next run has the fault's conditions.

## Progress (2026-10-23)

- **The shell's part** is on the tools branch: 4b69a22e, 4846ce00 and f69d1a16; the suite's fix is 5c556d68.
  - `caller_waits` checks the lent capability before `run`, `boot-notice` and `net-counters` answer.
  - A restarted shell gives the one `wm` that runs the screen (`give_back_wm`).
  - From `init.wit`'s `starts`, the second start runs no `autorun.msh`.
- **The `wm` suite's step** passed on 11b6ad01, with `main` 2f85e06a and ABI 17, in the local "x86: keys, shell, tools" group:
  - the shell is killed while `wm` runs;
  - the new shell logs `WM (PID n) RAN BEFORE THIS SHELL STARTED: THE SCREEN IS ITS AGAIN`;
  - `wm`'s close all closes its one window;
  - `dmesg -s shell` shows `START 2 OF THIS BOOT: AUTORUN.MSH RAN AT THE FIRST, NOT AGAIN`.
- **The rule for every server** is in `main` with 000-KRN-0152 (ABI 17, e93c47af): a revoked lease leaves its borrower a zeroed page. The shell's check stays, so a call the caller gave up on gets no answer written.
- **Left:**
  - ASR's `giveup` suite (500-ASR-0012) in `main`, run on this code;
  - the MacBook Pro's run under the fault's conditions (MC-12.1).

## Related

- 500-ASR-0012 (the `giveup` suite); [000-APP-0096](000-APP-0096-wms-net-line-from-every-card.md) (`net-counters`).
