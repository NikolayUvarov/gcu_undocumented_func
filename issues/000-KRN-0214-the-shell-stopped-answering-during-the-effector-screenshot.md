# 000-KRN-0214 — The shell stopped answering during the Effector suite's screenshot, on x86

**Type:** a suite to find, then the service at fault · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** the landing gate · **Constitution:** MC-12.2

## Problem

**Seen twice, on x86 only, both on x32-128:**
- **Round 86's gate** (candidate 6253e7a7, lane `x32`): `x86: network, TLS and Effector` failed after 957 s. Run again alone in the same lane, it passed in 1083 s, so the gate was green.
- **`main` 46b69344,** lane `KRN-2`, beside 000-KRN-0210's repeats: the same group failed once (994 s). Its logs were lost to the next run in that lane, and the same group passed in the next part.

The `aarch64` group of the same suites passed in both rounds.

**What round 86's record shows.** `land.py` kept the logs on x32-128 in `/home/gater/mind-core/KRN/logs/land-6253e7a7/x32-03/`.
- The failure is in `effector_actions_check`, at its first step:
  - the fixture queued `screenshot` (cmd-6);
  - `until` read `dmesg -s effector` once a second for `ACK cmd-6 completed WITH screenshot.png`.
- `effector`'s log never named cmd-6. Its last lines were cmd-5's (`FAILED cmd-5 collect_file: denied`, `ACK cmd-5 failed ACCEPTED`, at 58.3 s).
- **The shell stopped answering:**
  - the console shows 40 reads, each `MIND> dmesg -s effector`, then `STARTED PID=… NAME=dmesg FOREGROUND` and the log;
  - the 41st read's command was echoed, but no `STARTED` line came, nor a prompt, until the suite's wait ran out;
  - the console log holds no `PANIC`, `EXCEPTION` or restart of a service.

**What is not known:** which task stopped, and on what. The console log has only what the shell printed and the kernel's boot lines. Nothing records the tasks' states at the time.

This is not a cause (MC-12.2). Candidates only:
- the compositor, busy with the gateway's capture while the shell asks it for the screen of a program it starts;
- `loader` or `vfs_server`, while the gateway reads or `effector` writes;
- memory, while `effector` encodes the PNG.

## Plan

1. **Record the state when the suite's wait runs out.** In the Effector suite, a wait that fails records, before the VM closes:
   - each CPU's registers through QEMU's monitor (`info registers -a`);
   - a `screendump`;
   - what a second reader of the kernel's task table can show, if there is one that needs no shell.
2. **Reproduce:** `x86: network, TLS and Effector` on x32-128, 10 runs in a row, with the other lane busy, as for 000-KRN-0210.
3. **Find the task and the wait** from that record, and fix it in the service at fault, or pass it to that service's track.

## Acceptance criteria

- The suite's failed wait leaves a record that names the state of every CPU, and the test that makes it fails without it.
- The cause is named, with the record that shows it; or 10 runs in a row pass on x32-128 beside a busy lane, and that is recorded.

## Related

[000-KRN-0210](../issues-done/000-KRN-0210-the-updater-group-ran-past-its-limit-once.done) (a gate's hang, and the logs `land.py` keeps); [750-KRN-0153](../issues-done/750-KRN-0153-the-gateways-screen-capture.done) (the capture), 750-EFF-0007.
