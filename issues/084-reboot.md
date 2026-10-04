# 084 — `reboot`

**Type:** tool · **Owner:** tools track · **Priority:** P2 · **Status:** open · **Blocked by:** [152](152-reboot-system-call.md) · **Roadmap:** track G, T3

## Problem

`stop` halts the machine; there is no reboot ([docs/tools](../docs/tools/README.md) §2.3), and the kernel has no call for it (152).

## Plan

- `reboot` (shell command): flushes every volume (`vfs.wit` `flush` on the roots), asks `init` to stop the services in reverse start order (lifecycle requests, so drivers quiesce their devices), then calls `REBOOT` (152) with the process-control privilege. `reboot -f` skips the service stop.
- QEMU `services` suite: after `reboot` the VM boots again (the harness sees a second `[INIT] READY`), and a file written to `data/` before the reboot is intact.

## Acceptance criteria

- The test above passes; README and `docs/tools` describe `reboot`.

## Related

[152](152-reboot-system-call.md), [070](../issues-done/070-svc-lifecycle.done).
