# 900-NRL-0017 — Native runtime boundary and nnd integration

**Type:** implementation, `NRL` · **Owner:** Codex session (`codex/NRL-lab`) · **Priority:** P1 · **Status:** in progress · **Blocked by:** APP request in [requests-APP.md](requests-APP.md#nrl-native-nnd-integration-for-900-t11), for native service integration/evidence · **Roadmap:** G; 900 T11 · **Constitution:** MC-3.13, MC-5.1, MC-5.2, MC-8.1, MC-9.1, MC-11.1, MC-11.11, MC-12.1, MC-12.2, MC-12.3

## Problem

NRL's model runners require host allocation and lack a portable execution/session boundary. Main provides kernel memory/workers but still has neither APP's nnd nor idl/nn.wit. Cross-built numerical libraries alone cannot close T11.

## Plan

1. Add a no_std/no_alloc executor for the existing bounded feed-forward graph, using trusted checked borrowed kernel views and caller-owned buffers.
2. Bind sessions to a service-minted owner and model identity; validate graph/buffer/work limits, charge completed work without refunds, support cooperative cancellation/deadlines and reject invalid state/clock reuse. Clear transient/output buffers on failure.
3. Bridge already verified host model packages to that executor without changing artifact mathematics or service/kernel interfaces. Add explicit host policy/run evidence and both portable builds.
4. Ask APP for the native nnd/IDL/parser/identity/memory/time/cancellation integration and native client-isolation smoke harness. Keep native acceptance open until it is implemented and executed; proceed independently to host T12/T13.

## Acceptance criteria

- [ ] Portable bounded graph execution agrees with existing model outputs for affine/nonlinear/add/tied objects and five gated paths, without runtime allocations or unsafe code.
- [ ] Ownership/model mismatch, resource exhaustion, cancellation/deadline, non-monotonic clock, numeric failure and independent-session behaviour are exercised; charges and output/scratch clearing are explicit.
- [ ] Host package bridge and CLI, versioned internal contract, source/resource evidence and both portable rlibs pass all relevant checks.
- [ ] APP's nnd uses a versioned cross-domain IDL and isolated checked import, binds caller identity/capabilities, supplies native memory/time/worker/cancel operations and does not silently delegate to a host.
- [ ] A standalone artifact executes through nnd on MIND Core; native budgets, cancellation, two-client isolation and failure recovery are checked on a named target. Profile/TCB/evidence state exactly what ran.

## Related

- [900](900-neural-representation-lab.md), T11; [262](262-model-runtime-on-the-machines-full-hardware.md), APP-owned runtime.
- [T10](../issues-done/900-NRL-0016-student-activation-recovery.done); [AGENTS.md](../AGENTS.md), section 5, owned-track changes.
