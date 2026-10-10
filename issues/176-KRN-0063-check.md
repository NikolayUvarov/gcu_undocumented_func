# 176-KRN-0063 — `check`: a self-test of what is done, component by component

**Type:** kernel and assurance · **Owner:** kernel session · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Roadmap:** Assurance, main task [176](176-test-and-performance-utilities.md) · **Constitution:** MC-12.1, MC-12.2

## Problem

On a real machine nothing says which parts of the system work there, so each run is judged by reading logs.

## Plan

`check [group …]`, a console program. Every check ends in one of three ways:

- ✓ **passed**;
- ✗ **failed**, with the reason;
- ○ **skipped**, with the reason: no such device, the launcher did not lend the client, nothing to test.

The groups:

| Group | What it checks |
|---|---|
| `kernel` | the clocks advance; a call and reply through an endpoint; a page allocated, written and freed; a capability minted and dropped; a program started and its exit seen |
| `services` | `init`'s boot plan: every boot service that should run runs; the ones not started name their reason (no device) |
| `files` | a file written, read back and removed on `ram:` and in `data/`; the boot volume's manifest read; `log:` mounted |
| `clock` | the RTC's date within 2000–2099 and moving with the monotonic clock |
| `devices` | the PCI functions and USB devices the system sees, with their drivers: storage, input, audio, camera, network, Wi-Fi |
| `network` | an address from DHCP, the gateway answering, a name resolved (when the policy grants `check` a flow) |
| `security` | the device key present; TLS ready; the boot's launch record verified |

At the end a summary: how many passed, failed and were skipped. The exit status is 1 if any failed. The log `log:checkNNNN.txt` has each check's details.

## Acceptance criteria

1. On QEMU (x86, aarch64) with the suites' devices, every check passes or is skipped with a stated reason.
2. A check made to fail (for example a missing file) fails with its reason.
3. The QEMU suite checks the summary and the log.
4. On the MacBook Pro the maintainer's run is recorded.

## Related

[176-KRN-0062](176-KRN-0062-kbench.md), [176-KRN-0064](176-KRN-0064-bench.md), the hardware report (174-KRN-0038).
