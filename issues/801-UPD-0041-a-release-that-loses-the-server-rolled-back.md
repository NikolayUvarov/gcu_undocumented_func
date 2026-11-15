# 801-UPD-0041 — A release that loses the server, rolled back: the slot before back on the server as it was

**Type:** update (tests: the `rollback` QEMU suite) · **Owner:** `UPD` (the kernel session) · **Priority:** P1 · **Status:** open (2026-11-15) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S1, S2, S3) · **Roadmap:** stage V · **Constitution:** MC-9.4, MC-9.9, MC-12.1

## Problem

**No check runs the whole path** of an update that breaks the connection:
- 801-KRN-0242 and 801-UPD-0037 are in `main` (round 139). Their `boot` suite case refuses a trial whose agent never connects, and the next boot is slot A's.
- In that case slot A cannot connect either, because both slots read the same unreadable configuration. So nothing shows that the slot before is back on the server.
- Nothing checks S3 for a refused trial: that every file the slot before reads is as it was.

The maintainer (801): «никакое обновление не ломает наличие связи и приводит к откату к предыдущему состоянию, когда связь была с репортом причин». An update that breaks the connection is rolled back to the state that had it, with a report of the reasons.

## Plan

**A QEMU suite, `rollback`, its own CI group on x86 and aarch64,** with the Effector server's fixture:
1. **Slot A, release 5, is on the server.**
   - The development mode and `remote-update` are on, and the owner's policy says `update apply`.
   - An `update_agent` command brings release 6 (3 tries).
   - The agent streams it to the updater, which stages it in slot B and restarts. QEMU exits (`-no-reboot`), and the command is not answered before that.
2. **Slot B on trial with the server out of reach:** the fixture answers no request until the machine restarts.
   - `init` holds the trial past the kernel's deadline for the whole 600 s bound, then refuses it.
   - The updater keeps the reason in `system/updater/refused` (`NO HEARTBEAT ACCEPTED IN 600 S: …`), spends the trial with its tries left, and restarts.
   - No heartbeat reaches the server.
3. **S3:** every file on the disk is compared with what was there before the trial. The only exceptions are what a refused trial writes for itself:
   - the boot records;
   - the loader's mark;
   - slot B's own files;
   - `system/updater/`.

   Anything else that differs fails the suite, by name.
4. **Slot A again:**
   - the loader says slot B has no tries left;
   - the agent connects as the same agent with the same token;
   - its heartbeat carries `update: rolled_back`, build 6, with the reason (801-EFF-0029);
   - it answers the update command `failed`, `fell-back`, naming the reason.

## Acceptance criteria

- The suite passes on x86 and aarch64 on the maintainer's machines, then in a gate.
- It fails if the trial is confirmed without the server: a held trial that is confirmed is never refused, so the third step's restart never comes.
- The profile's Article 9 names it as the evidence for S1 to S3 in QEMU.

## Related

[801](801-the-bench-stays-manageable.md), [801-KRN-0242](801-KRN-0242-a-trial-confirmed-on-the-effector-servers-word.md), [801-UPD-0037](../issues-done/801-UPD-0037-a-refused-trial-spent-and-its-reason-kept.done), [801-EFF-0029](801-EFF-0029-the-fallbacks-report-on-the-first-connection.md), [801-ASR-0014](801-ASR-0014-the-stages-conditions-as-checks.md), [751-EFF-0023](751-EFF-0023-the-agent-installs-a-developers-release.md).

## Progress (2026-11-15)

**The branch:** `claude/801-UPD-0041-a-release-that-loses-the-server-rolled-back`, suite as of aa5abd549f. **Where it ran:** x86 on x32-128, aarch64 on acc-1, three runs on each arch.

**What passed on both arches** (S1):
- slot A on the fixture took release 6 by `update_agent` and restarted, the command not answered before;
- slot B's trial, with the server out of reach, was held about 610 s and refused (`NO HEARTBEAT ACCEPTED IN 600 S: …`);
- `system/updater/refused` held the reason, the records said B with no tries left, and no heartbeat reached the server;
- slot A booted, connected as the same agent with the same token, logged `A FALLBACK, REPORTED TO THE SERVER`, and its heartbeat carried `update: rolled_back`, build 6.

**What the suite found:**
- **S3, in EFF's agent:** 180 s into the trial (`CONFIRM_WAIT_MS`), B's agent answered the kept command `failed`, `not-confirmed`. It wrote `system/effector/result`, `executed` and `journal`, and removed `system/effector/update`, so after the fallback slot A sent that stale answer instead of `fell-back`.
  - **Request:** `issues/requests-EFF.md`.
  - **EFF's answer:** taken as 801-EFF-0034 (`8c53f1e2` on EFF's branch, not pushed yet).
- **S2:** the heartbeat's report was cut at 64 bytes, `…: NO HEARTBEAT ACCEPT`, without the agent's last step. That is [801-UPD-0042](801-UPD-0042-a-refused-trials-reason-reaches-the-server-whole.md), with EFF's part requested.
- **A log's append:** the trial's gateway added a line to `system/effector_gw/audit`. The suite takes that as a log's append (slot A's lines unchanged) and fails a rewrite of them.

**The suite now** checks the heartbeat and the command's answer against the reason in `system/updater/refused`, whole. It reports every finding at its end, after slot A's checks.

**Left:** EFF's 801-EFF-0034 and its part of 801-UPD-0042; then the suite on both arches, and a gate.
