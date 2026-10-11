# 176-KRN-0063 — `check`: a self-test of what is done, component by component

**Type:** kernel and assurance · **Owner:** kernel session · **Priority:** P1 · **Status:** in progress (made and run in QEMU; the MacBook Pro's run left) · **Blocked by:** — · **Roadmap:** Assurance, main task [176](176-test-and-performance-utilities.md) · **Constitution:** MC-12.1, MC-12.2

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

## Progress

**2026-10-10: made, and run in QEMU (x86 and aarch64, 4 CPUs).** `bench/src/bin/check.rs` has eight groups:

| Group | What it checks now |
|---|---|
| `kernel` | the clocks advance; 64 KiB written and read back; 1 TiB refused (the quota); a send-only copy may not receive; a revoked copy is gone; a copy of `check` starts, answers a call, reads a page lent to it and ends when told |
| `security` | three probes, each a copy of `check` that must be stopped by a fault: a read of the kernel's memory, a write to its own code, a read of address 0; the boot's launch record (images checked against the manifest's key, the test key named); RDRAND or RNDR |
| `clock` | the RTC's date in 2000–2099; the RTC advancing 1–2 s over 1.2 s of the monotonic clock; the monotonic clock's counter; 20 sleeps of 10 ms, none shorter |
| `files` | 64 KiB written, read back and removed on `ram:` and in `data/`; `log:` and this boot's log; the boot volume, `ram:` and `log:` consistent (vfs.wit `check`); every program on the disk accepted by the loader (`inspect`) |
| `services` | every boot service from `init`'s list: running (its PID, and how often it was started), quarantined (failed), or not started or stopped (skipped, with the reason) |
| `devices` | every PCI function with the driver that holds it (bridges counted in one row) |
| `sound` | the sound device; a 150 ms tone; the microphone: samples within 0.6 s and their peak (silence is a skip with its level, no input a failure) |
| `network` | with a flow grant: the address (DHCP or static), the gateway's answer, a name resolved |

- **Running it.** `check` runs every group; `check kernel clock` runs those two. The exit status is 1 if a check failed.
- **The output.** The screen shows a 79-column table, the failed checks again under it, and the summary. The log (`log:checkNNNN.txt`, or `ram:check-NNN.txt`) holds the table, then every check's full detail, the services' holdings and the devices' BARs.
- **Authority.** It asks for the console, the user's files, `sysmon`, lifecycle control (it calls only `list`) and a flow grant. The shipped policy names nothing for `check`, so the network group is skipped with that reason until a line is added (`netpolicy add`).
- **QEMU** (the `bench` suite):
  - x86: 43 passed, 26 skipped, 1 failed;
  - aarch64: 48 passed, 20 skipped, 1 failed;
  - the one failure is "sleeps last as asked": 10 of 20 sleeps of 10 ms were shorter ([000-KRN-0065](000-KRN-0065-a-sleep-is-never-shorter-than-asked.md));
  - the suite requires the kernel, security, clock, file and core service checks to pass, and accepts no other failure.

Left: the MacBook Pro's run (criterion 4), and a policy line for the network group (the network track's file).

## Related

[176-KRN-0062](176-KRN-0062-kbench.md), [176-KRN-0064](176-KRN-0064-bench.md), the hardware report (174-KRN-0038).
