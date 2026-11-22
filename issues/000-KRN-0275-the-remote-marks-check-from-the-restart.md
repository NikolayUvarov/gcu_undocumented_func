# 000-KRN-0275 — The remote mark's check samples the screen from the restart on, not after a slow dmesg read

**Type:** tests (`tests/qemu_smoke.py`, `gateway_stub_check`) · **Owner:** `KRN` (the kernel session) · **Priority:** P1 · **Status:** in progress (made, 2026-11-22) · **Blocked by:** — · **Main task:** — · **Constitution:** MC-12.1, MC-12.2

## Problem

Round 171's gate failed `x86: services, storage, audio` on x32, in the `modes` suite's `gateway_stub_check` (800-KRN-0156):

`AssertionError: (['e0c020', '808080'], '1e1e2e')`

The Effector agent's mark was seen yellow and grey, never red.

**The restarted stub lit red at the right time.** Its lines in the run's log:
- `[   44.165] … REBOOT NOT ASKED`;
- `[   44.196] … REMOTE MARK CHANGING OK`;
- red then lasted until `READING` at about 50.2 s.

**The check began to sample too late.** It waited for `REBOOT NOT ASKED` by reading `dmesg -s effector_gw` once a second, and began sampling only after that wait. Each of those reads returns about 11 KB through the console. On a loaded machine (x32 ran x32-2's lane beside it) the read took longer than the red's 6 s.

So the check missed a state the screen showed: a race in the check, not in the compositor or the stub.

## Plan

1. **Sample from the restart.** The pixel sampling starts as soon as `svc restart effector_gw` returns, before the stub's lines are waited for. Between dmesg reads it uses QEMU's screendump alone, which goes through the monitor rather than the console.
2. **Read dmesg less often.** The loop reads dmesg at most every 3 s, to learn that `REMOTE MARK STOPPED` came after `SCOPE START 2`. The first instance's `STOPPED` (before the restart) does not end it.
3. **Check the stub's two lines afterwards.** `SCOPE START 2 …` and `REBOOT NOT ASKED` are still required, after the loop. The colours and the compositor's `REMOTE MARK Effector: …` lines are required as before.

## Acceptance criteria

- [x] `x86: services, storage, audio` passes on x32 beside a busy lane, and the red is seen in each run.
- [ ] The gate's groups pass.

**Not covered:** a machine slow enough that one screendump takes longer than 6 s.

## Progress (2026-11-22)

**Made** on `claude/000-KRN-0275-the-remote-marks-check-from-the-restart`, with round 171's `main` merged in (eb592ec468).

**Runs through Effector, at eb592ec468:**
- `x86: services, storage, audio` on x32 and x32-2 at once, two runs each, one after the other: 4 of 4 passed (1009 to 1011 s each);
- that is the load under which round 171's run failed;
- the check passes only when red, yellow and grey are all seen, so the red was seen in each run.

**Left:** the gate's groups.

## Related

[800-KRN-0156](800-KRN-0156-the-remote-marks-in-the-compositor.md) (the marks), [352-KRN-0268](../issues-done/352-KRN-0268-the-live-volume-in-memory.done) (the stub's logging rate, round 164).
