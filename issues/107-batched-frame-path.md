# 107 — Batched frame path between the stack and the card drivers

**Type:** performance · **Owner:** network track · **Priority:** P3 · **Status:** open · **Blocked by:** — (106 done) · **Roadmap:** tracks A and D · **Constitution:** Appendix B.2 (performance by measurement), MC-2.6, MC-2.11, Appendix B.6

## Problem

The benchmark of issue 106 ([docs/profile/network.md](../docs/profile/network.md)) shows that the stack's cost is per frame, not per byte. About 275 µs of `netstack` time and about 210 µs of driver time go into each frame. Each frame is one MIND IDL buffer call: a memory capability made, mapped, copied and revoked. Checksums are about 0.3 % of the work.

## Plan

- Measure first where a frame's time goes: the IPC call, the capability and mapping work, the copies, the stack's poll loop (it also waits up to 10 ms between polls when no client calls).
- A frame path with several frames per call. For example, a region shared between `netstack` and one driver, with descriptor rings and a doorbell over the endpoint, set up once through `idl/net.wit` (a minor version), with the existing calls kept for the shell's diagnostics.
- The receiver checks every descriptor against the region's bounds (MC-2.11). The stack never trusts the driver's lengths beyond the frame size.
- Then decide segmentation offload again: larger frames only pay off once the per-frame cost is down.

## Acceptance criteria

- The `netbench` suite shows a measured gain in CPU time per MiB, recorded in `docs/profile/network.md`. The `net` and `tls` suites pass. A driver restart (with quiesce) still works with the new path.

## Related

[101](../issues-done/101-network-stack.done), [104](../issues-done/104-virtio-modern-msix.done), [106](../issues-done/106-network-offloads.done).
