# 105 — Several network cards

**Type:** service policy + driver · **Owner:** network track · **Priority:** P3 · **Status:** open · **Blocked by:** 101 · **Roadmap:** tracks A and D · **Constitution:** MC-1.4, MC-6.4, Appendix B.6

## Problem

`init` starts one `virtio_net` for the first card it finds; a boot service is one name with one instance. A machine with two networks (or MIND Core acting as a gateway) needs one driver instance per card and a stack with several interfaces.

## Plan

- **init:** a driver service may have several instances, one per matching device (`DEVICE_FIND` with n-th match), each with its own device capabilities, DMA region, endpoint and restart budget; `svc` and `STAT` name instances (`virtio_net#0`, `virtio_net#1`).
- **Stack (issue 101):** several interfaces with their own addresses, routes and DHCP; the stack is the only client of every card driver.
- **Policy:** which card a flow may use is part of the policy broker's grants (issue 102).

## Acceptance criteria

- In QEMU with two VirtIO cards on two user-mode networks: both get addresses, traffic goes out through the right card per route, killing one driver leaves the other card working, and the restarted instance gets its own device back.

## Related

[101](../issues-done/101-network-stack.done), [102](102-network-policy-broker.md), [104](../issues-done/104-virtio-modern-msix.done).
