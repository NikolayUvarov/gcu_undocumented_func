# 038 — Scheduling budgets (roadmap C7)

**Type:** kernel · **Owner:** kernel track (coordinates `kernel/src/scheduler.rs`, `common/abi.rs`) · **Priority:** P1 · **Status:** open · **Roadmap:** step 3, C7 · **Constitution:** MC-5.1–5.5

## Problem

Scheduling is round-robin per CPU with a 10 ms tick: a busy task takes its CPU share regardless of importance, and nothing reserves time for `init` to recover the system under overload.

## Plan

- A scheduling context per task: budget and period (default: unlimited), charged in TSC time at each switch; a task that spends its budget waits for the next period.
- Priority bands: supervisor/drivers above applications; `init` sets contexts for services (policy) and has a reserve that application load cannot consume.
- `SCHED_SET(pid, budget_us, period_us)` for the lifecycle owner.

## Acceptance criteria

- `busy` suite variant: a CPU-bound application with a 20 % budget uses about 20 % (±5 %) of its CPU; the shell and `init` stay responsive while all CPUs are saturated.

## Related

[ROADMAP](../ROADMAP.md) C7.
