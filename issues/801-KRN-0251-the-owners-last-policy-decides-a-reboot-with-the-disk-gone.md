# 801-KRN-0251 — The owner's last policy read decides the gateway's reboot when the boot disk is gone

**Type:** `init` · **Owner:** `KRN` · **Priority:** P1 · **Status:** open (2026-11-14) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S5) · **Roadmap:** stage V · **Constitution:** MC-3.4, MC-6.8

## Problem

EFF's request (`KRN/2026-11-14T1650Z-EFF-a-request-init-keeps-the-owners-last-policy-for-a-reboot-with-the-disk-gone.md`).

LNK found that the bench's agents stopped taking commands when the boot disk was taken out while the system ran. A restart is what recovers such a machine. So in 801-EFF-0032 the agent and the gateway go on without the disk, and a `reboot` under the owner's `reboot on` is still asked of `init`.

`init` refuses that reboot: `named_reboot` reads `data/effector/policy` from the disk at each request. With the disk gone it finds none (`RESTART THE MACHINE: NO OWNER'S POLICY`). So the machine can be restarted only by hand. Its lifecycle requests (`service … restart`) are refused the same way.

## Plan

- **`init` keeps the policy it last read whole and parsed,** with the time it was read. It reads the policy at each gateway start (the delivery root) and at each lifecycle or reboot request.
- **For the gateway's lifecycle requests and its reboot alone** (`BADGE_NAMED`), when the file cannot be read now, that copy decides, and the log says so: `[INIT] THE OWNER'S POLICY CANNOT BE READ NOW: THE ONE READ AT <s> S DECIDES`. "Cannot be read" is any of:
  - the volume or the file answers anything but `not-found`;
  - a read fails;
  - `vfs_server` does not answer.
- **A file that reads but is absent or unparsable** allows nothing, as before, and drops the copy. So nothing the owner removed from a policy on a working disk is allowed by an older copy.
- **The gateway's scopes at its start** (the delivery root) still come from a policy read at that start, never from the copy.

## Acceptance criteria

1. **On x86 and aarch64** (QEMU, `reboot_disk_gone_check` in the `modes` suite, with `tests/gateway_stub`):
   - the boot disk's driver is stopped under the gateway, and its files then cannot be written;
   - its reboot under `reboot on` is allowed by the copy, `init` says so, and the machine restarts.
2. **`gateway_stub_check` still passes:** its refusals without `reboot on` are unchanged.
3. **EFF's check in the effector suite** (801-EFF-0032, the disk gone under the agent) sees the machine restart.

## Related

[801](801-the-bench-stays-manageable.md), 801-EFF-0032, 750-KRN-0121, 750-KRN-0154.
