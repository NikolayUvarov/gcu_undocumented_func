# 000-KRN-0202 — x32-128 in the gate's pools

**Type:** scripts · **Owner:** kernel session · **Priority:** P1 · **Status:** in progress (made; its own round is the test) · **Blocked by:** — · **Roadmap:** the landing queue (AGENTS.md, section 4) · **Constitution:** MC-12.1

## Problem

**Why x32-128 now.** The maintainer put x32-128 into the gate's pools (2026-11-03). It has been ready since:
- its new CPU passed the burn-in;
- PL1 = PL2 = 200 W, with no throttling;
- [000-KRN-0200](../issues-done/000-KRN-0200-setup-for-the-ubuntu-22-04-gate-machines.done) set it up, and its host part passes.

**What the pools lack.** Since un-rizen went offline, the gates ran on three lanes, and the host tests ran nowhere on the machines.

**What `land.py` lacks.** It puts every lane's clone under `/home/un`. The agent on x32-128 runs as `gater`.

## Plan

1. **`MACHINES`:** `x32`, with its agent, no `run_as` and its home `/home/gater`. A lane's clone and its logs sit under the machine's home.
2. **Two lanes, `x32` and `x32-2`,** in `pool` and `pool-no-i9`, each in its own clone (32 threads). They take the host, x86 and aarch64 parts.
3. **`docs/effector.md`:** the machine's row and the pool's lanes.

## Acceptance criteria

- **A gate whose pool holds both x32 lanes passes,** with the host tests among their groups.

## Related

[000-KRN-0200](../issues-done/000-KRN-0200-setup-for-the-ubuntu-22-04-gate-machines.done), [docs/effector.md](../docs/effector.md), sections 1 and 4.
