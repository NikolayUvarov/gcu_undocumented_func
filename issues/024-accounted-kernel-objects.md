# 024 — Per-owner quotas for tasks and endpoints (roadmap K4)

**Type:** feature/architecture · **Priority:** P0 · **Status:** open · **Roadmap:** step 2, K4 · **Constitution:** MC-1.7, MC-3.13, MC-5.1

## Problem

Tasks and endpoints come from fixed global tables. The application limit (`MAX_APPS = 8`) is a constant in the ABI enforced by the kernel for every non-service spawn; any task may create endpoints until the global table is exhausted. Nobody owns these resources, and the limit is policy in the kernel.

## Plan

- Every task has a quota of live child tasks and of endpoints it may create, and counts its usage.
- `SPAWN` delegates a quota to the child (child tasks, endpoints) taken from the spawner's remaining quota; it returns to the spawner when the child exits. The new task itself counts against the spawner's child quota.
- The kernel gives `init` the root quota (all task slots but its own, all dynamic endpoints). `init` decides how much each service gets: `loader` gets 8 application tasks — the application limit becomes init's policy; services get a few endpoints.
- `ENDPOINT_CREATE` fails with `ERR_LIMIT` past the quota; `SPAWN` fails with `ERR_LIMIT` past the child quota.
- The `ps` table and the profile document the quotas.

## Acceptance criteria

- No application limit constant is enforced by the kernel; the "TASK LIMIT REACHED" behaviour of the `normal` suite comes from loader's quota.
- A test shows an application cannot create more endpoints than its quota.
- All suites pass.

## Related

[docs/profile/kernel-objects.md](../docs/profile/kernel-objects.md), [ROADMAP](../ROADMAP.md) K4.
