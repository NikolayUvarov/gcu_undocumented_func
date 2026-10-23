# 750-KRN-0154 — The gateway's reboot, by the owner's policy

**Type:** `init` · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-3.4, MC-3.7, MC-6.7

Split from [750-KRN-0119](../issues-done/750-KRN-0119-boot-images-and-grants-for-effector.done), whose plan gave `effector_gw` an `init` client badged `BADGE_REBOOT`. It serves [750-EFF-0010](750-EFF-0010-reboot.md).

## Problem

- **Who may reboot today.** `init` reboots only for `updater`'s client (`BADGE_REBOOT`) and `link`'s developer client (`BADGE_DEVELOPER`), and refuses every other with MC-3.8.
- **What the gateway holds.** Its `init` client in slot 11 is badged `BADGE_NAMED` (750-KRN-0121), so its `reboot` is refused always.
- **What EFF needs.** A reboot the owner's policy allows (`reboot on` in `data/effector/policy`), and nothing when it does not.

## Plan

1. **No new badge and no new slot.** `init` serves `reboot` from the `BADGE_NAMED` client when `Policy::decide(Ask::Reboot)` allows it, reading `data/effector/policy` at each request, as it does for `run`, `stop` and `restart`.
2. **Refused otherwise,** with the reason in the log and MC-3.4: no policy file, no `reboot on`, or a malformed policy.
3. **The same path as the shell's reboot:** `init` stops the services in order and asks the kernel to restart. In a trial boot, a reboot is a reboot like any other (351-KRN-0014).
4. **`docs/profile/bootstrap.md`** says the gateway may reboot only by the owner's policy.

## Acceptance criteria

- [ ] In the `modes` suite (x86 and aarch64), the gateway's stand-in under `reboot on` reboots the machine, and the next boot comes up in the same mode.
- [ ] Without `reboot on`, its `reboot` is refused with the reason, and the machine runs on.
- [ ] `stop-task` stays refused for the gateway.

## Progress (2026-10-23)

Made on `claude/youthful-mendel-mf1soy`:
- **`init`** (`named_reboot`) serves `reboot` from the `BADGE_NAMED` client when the owner's policy allows it, read at the request.
  - It logs `[INIT] REBOOT BY PID n (BADGE 6): THE OWNER'S POLICY ALLOWS IT`.
  - Otherwise it refuses with `[INIT] REBOOT REFUSED: … MAY NOT RESTART THE MACHINE: NOT IN THE OWNER'S POLICY (MC-3.4)`, or `NO OWNER'S POLICY`.
- **`tests/gateway_stub`** asks a reboot when the suite leaves `reboot-now` in its delivery root, and removes it first.
- **The `modes` suite:**
  - without `reboot on`, the reboot is refused with that line, and the machine runs on;
  - with it, QEMU restarts the machine once, and the next boot comes up in the development mode; the stub's second start finds the first one's files and asks nothing.
- **`docs/profile/bootstrap.md`** says so.
- **Run:** x86, in this session's container (QEMU TCG); aarch64 is running.

## Related

[750-KRN-0119](../issues-done/750-KRN-0119-boot-images-and-grants-for-effector.done), [750-KRN-0121](../issues-done/750-KRN-0121-a-lifecycle-client-for-named-services.done), [750-EFF-0010](750-EFF-0010-reboot.md), `idl/init.wit` (`reboot`), `libmind/src/effector/policy.rs` (`Ask::Reboot`).
