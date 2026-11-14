# 801-APP-0140 — `effector policy show | allow <line> | deny <line>` in the shell

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P1 · **Status:** open · **Blocked by:** — for the file; the gateway reading it at once waits for the shell's owner client of `effector_gw` (`BADGE_OWNER`), asked of `KRN` · **Main task:** [801](801-the-bench-stays-manageable.md) (S7), the policy part of [750-APP-0093](750-APP-0093-the-shells-effector-command.md) · **Roadmap:** the development of MIND Core on real hardware; stage V · **Constitution:** MC-3.8, MC-3.11, MC-7.5, MC-10.7

**Asked by:** LNK for 801's S7 on 2026-11-14 (`ALL/2026-11-14T0847Z-LNK-801-the-bench-stays-manageable-your-parts.md`): today the policy needs hand edits of `data/effector/policy`. Numbered on 2026-11-14.

## Problem

- **The owner's policy** (`data/effector/policy`, 750-EFF-0003) says what the Effector server may make the machine do; `update apply` and `reboot on` are what a remote update needs.
- **Only a hand edit changes it.** A mistyped line refuses the whole file, and a refused policy allows nothing: one wrong edit cuts the server off the machine.

## Plan

- **`effector policy`** (or `show`): the file as it is, and whether it parses (its rules, or the line refused).
- **`effector policy allow <line>`:** the rule added, or put in place of the rule with the same key (`update`, `reboot`, `agent`, …).
- **`effector policy deny <line>`:** that rule taken out; a key with `on`/`off` set to `off`.
- **Never a policy that does not parse:** the whole new file is parsed with `mind::effector::policy` before it is written; a refused one is not written, and the line is named. A file that does not parse now is never edited, only shown.
- **Asked** at the local console before anything that widens the policy; a narrowing is not asked. Never from the link's client, an SSH session's or a lent one.
- **Written whole** under a temporary name and renamed over the old one, so a lost write leaves the old policy.
- **The gateway reads it** at its next start until the shell holds its owner client; then `reload-policy` at once, and the answer shown.

## Acceptance criteria

- **Host tests (`tests/shell_host.rs`):** the edits on a policy, a refused line, a file that does not parse left alone, widening told from narrowing.
- **QEMU (`modes` suite, x86):** `effector policy allow update apply` asked and written, the file parses, `deny` takes it out.
- **Docs:** `docs/effector/policy.md` and `docs/tools/README.md` (both languages).

## Related

[801](801-the-bench-stays-manageable.md), [750-APP-0093](750-APP-0093-the-shells-effector-command.md), [750-EFF-0003](../issues-done/750-EFF-0003-interface-and-policy.done).
