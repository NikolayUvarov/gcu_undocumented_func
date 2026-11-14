# 801-APP-0140 — `effector policy show | allow <line> | deny <line>` in the shell

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P1 · **Status:** in progress (the commands done and checked in QEMU, 2026-11-14; the gateway's reload at once waits for the owner client) · **Blocked by:** — for the file; the gateway reading it at once waits for the shell's owner client of `effector_gw` (`BADGE_OWNER`), asked of `KRN` · **Main task:** [801](801-the-bench-stays-manageable.md) (S7), the policy part of [750-APP-0093](750-APP-0093-the-shells-effector-command.md) · **Roadmap:** the development of MIND Core on real hardware; stage V · **Constitution:** MC-3.8, MC-3.11, MC-7.5, MC-10.7

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

## Progress (2026-11-14)

- **The code** (007ffffa, on the tools branch; `fast-test` dd18dde9):
  - `shell/src/policyedit.rs`: the command line; a rule's identity (its key, and for `service`, `package-key`, `collect` and `log` its name or root); the edit, `check`ed whole by the caller's parser; the text;
  - `shell/src/policyctl.rs`: the file read (one byte past 4096, so a larger one is refused as the gateway refuses it); the write as `policy.new`, read back, the old file renamed to `policy.old`, the new one renamed into place, the old one removed; `recover` at the shell's start, which finishes or undoes an edit a restart cut;
  - `allow` asked at the local console, `deny` at once; refused to every line typed elsewhere.
- **Tested** in this container, x86, 4 CPUs:
  - `tests/shell_host.rs`: `effector_policy_edits_one_rule_and_never_writes_a_refused_policy`, which uses the gateway's parser (41 tests);
  - the `modes` suite's `effector_policy_edits`:
    - `show`;
    - `allow update apply` answered N unchanged, Y written;
    - `allow reboot on`, then `deny reboot`;
    - `allow update everything` refused, not written;
    - no `policy.new` or `policy.old` left;
  - `effector_policy_recovery_check`: a disk with no policy, a whole `policy.new` and a `policy.old`. At its start the shell gave the new one its name and removed the old one.
- **Docs:** `docs/effector/policy.md`, `docs/tools/README.md` and `README_RU.md`, the shell's help line.
- **Left:** the gateway reading the new policy at once, through the shell's owner client of `effector_gw` (`requests-KRN.md`). Until then it reads the file at its next start.

## Related

[801](801-the-bench-stays-manageable.md), [750-APP-0093](750-APP-0093-the-shells-effector-command.md), [750-EFF-0003](../issues-done/750-EFF-0003-interface-and-policy.done).
