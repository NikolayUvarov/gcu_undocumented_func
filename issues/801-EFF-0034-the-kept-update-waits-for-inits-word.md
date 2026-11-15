# 801-EFF-0034 — The kept update waits for `init`'s word on the trial, and changes nothing the slot before reads

**Type:** session · **Owner:** `EFF` · **Priority:** P1 · **Status:** in progress (2026-11-15) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S1 to S3) · **Constitution:** MC-12.1, MC-6.6

## Problem

UPD's request of 2026-11-15, for 801-UPD-0041 (on UPD's branch until it lands), which it blocks. It was recorded in `issues/requests-EFF.md` and is taken here.

- **What the `rollback` suite found** (x86 on x32, aarch64 on acc-1, UPD's branch at 19fb97b17f):
  - slot A took release 6 by `update_agent` and restarted;
  - slot B came up on trial with the server out of reach, and `init` held it for its 600 s bound, then refused it.
- **What B's agent did meanwhile:**
  - 180 s into the trial (`update::CONFIRM_WAIT_MS`), it answered the kept command `failed`, `not-confirmed`;
  - it wrote that answer to `system/effector/result`, `executed` and `journal`, and removed `system/effector/update`.
- **So:** after the fallback, slot A had lost its kept answer, and held B's stale one instead of `fell-back`.
  - This breaks 801's S3: a trial changes nothing the slot before reads until it is confirmed.
- **The successful case has the same fault:** a trial whose first accepted heartbeat came after 180 s was confirmed by `init`, but its command was answered `failed`.

## Plan

1. **The agent asks `init` for its `boot` record** (`init.wit` 1.13) at each look at the kept update, and reads it as a trial deciding, confirmed, refused, or none (`update::trial`).
2. **While `init` decides or has refused,** the verdict waits, with no limit of its own, and nothing kept changes:
   - after a refusal `init` restarts the machine, and the slot before answers `fell-back` from the kept command;
   - no other command is taken meanwhile, as before.
3. **Once `init` has confirmed,** the updater's confirmation is awaited for `CONFIRM_WAIT_MS` from then, and the answer is `completed`. After that wait it is `not-confirmed`.
4. **When `init` does not say,** the answer is given as before.
5. **The agent logs `init`'s word when it changes:** `EFFECTOR: THE KEPT UPDATE <id>: init SAYS …`.

## Acceptance criteria

- [ ] Host tests: `tests/effector_update_host.rs`, `while_init_decides_the_kept_update_waits_with_no_limit`.
- [ ] UPD's `rollback` suite, x86 and aarch64, with this change: every file slot A reads is the same before and after the trial, and slot A answers the command `fell-back`.
- [ ] The `effector-update` suite, x86 and aarch64: the confirmed case still answers `completed`, `updated 6, slot B confirmed`.

## Related

801-UPD-0041, [801-KRN-0242](801-KRN-0242-a-trial-confirmed-on-the-effector-servers-word.md), [801-EFF-0031](801-EFF-0031-the-agent-tells-init-how-far-it-got.md), [751-EFF-0023](751-EFF-0023-the-agent-installs-a-developers-release.md).
