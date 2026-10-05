# 203 — aarch64 SMP, reset and power off through PSCI

**Type:** kernel · **Owner:** porting track · **Priority:** P2 · **Status:** open · **Blocked by:** — (201 done) · **Roadmap:** track H · **Constitution:** MC-5.x (budgets on every CPU), MC-6.x

## Plan

- **Secondary CPUs:** started with PSCI `CPU_ON` (HVC or SMC, as ACPI or the device tree says). Per-CPU GIC redistributors and timers.
- **TLB:** shootdown with broadcast TLBI instead of IPIs where the architecture allows it.
- **`REBOOT`:** with PSCI `SYSTEM_RESET`; power off with `SYSTEM_OFF`.

## Acceptance criteria

- The `smp` and `busy` suites pass with `-smp 4`.
- `reboot` restarts the machine.

## Related

[201](../issues-done/201-aarch64-boot.done), [152](../issues-done/152-reboot-system-call.done).
