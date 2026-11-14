# 801-KRN-0250 — A network grant from a policy line added while the system runs, with no restart of the machine

**Type:** `init` · **Owner:** `KRN` · **Priority:** P1 · **Status:** open (2026-11-14) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S5) · **Roadmap:** stage V · **Constitution:** MC-3.13, MC-11.6

## Problem

LNK's request (`KRN/2026-11-14T1603Z-LNK-801-s5-init-asks-the-agents-grant-only-once-a-boot.md`).

**The DEXP, on `fast-test` 719549b1:** a disk was written without the agent's policy line. The writer's half is LNK's, fixed in `fast-test` 3efb8fae.
- `netpolicy` refused `effector` (`NO POLICY`), and `init` logged `effector: NO NETWORK GRANT`.
- At 367 s the line was added (`POLICY CHANGED BY PID 38: ADDED effector …`).
- Even so, the agent stayed without a network until the machine was restarted. It went on ending itself every 31 s "so that `init` starts it again".

**Why:** `init` asks the broker for a unit's flow only when it makes the unit's plan, at its first start. Every restart reuses that plan (`Init::start`).
- So a line added later never reaches `effector` or `updater`, which start without a flow.
- Nor does it reach `timed`, `httpd` and `sshd`, which `init` does not start at all without one.

Under 801's S5 a machine must not stay out of reach until a restart because its policy line came late.

## Plan

- **`effector` and `updater`:**
  - at each later start of either, when its kept plan holds no grant in `SLOT_NETWORK`, `init` reads the policy in force;
  - if a line is now the program's, `init` asks the broker as at the first start;
  - a grant it gets is added to the plan and kept for every restart after, as one given at the first.
- **`timed`, `httpd` and `sshd`:**
  - when one was not started for want of a line, `init` looks every 30 s whether a line names it, and starts it once one does;
  - a start that then fails is not tried again until `svc start`.
- **Reading the policy:**
  - `init` reads it through `lines` (`netpolicy.wit` 1.1), which the broker does not log, and matches a line as the broker does: a rule line names the program first, `listen` and `browse` lines second;
  - so the restarts before the line come asks nothing and log no refusal, and the broker logs one refusal at the first start, not one a restart.
- **Not here:**
  - a spent grant, one whose term or volume ran out, is not renewed at a restart: that would multiply the line's budget (MC-3.13);
  - a running updater without a grant gets its grant only at its next start.

## Acceptance criteria

1. **On x86 and aarch64** (QEMU, `late_grant_check` in the `modes` suite):
   - the agent, started with no line, logs no grant;
   - a line added with `netpolicy add` gives it a grant at its next start, kept for the start after, with no grant asked for again;
   - `timed` starts within 30 s of its line;
   - the broker logs one refusal for `effector`.
2. **The `modes` suite** passes whole on both architectures. So do the network groups, because `init`'s loop now wakes every 30 s while a flow-only unit waits.
3. **The docs say so:** `docs/effector/README.md` (the grant), `docs/network/policy.md` (`timed`), and `docs/update/server.md` in EN and RU (`httpd`).

## Related

[801](801-the-bench-stays-manageable.md), 801-LNK-0044, 750-KRN-0119, 111-KRN-0206, 110-NET-0070, 112-NET-0072.
