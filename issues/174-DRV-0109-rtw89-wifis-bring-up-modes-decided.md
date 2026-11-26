# 174-DRV-0109 — `rtw89_wifi`: the bring-up's debug modes decided one by one

**Type:** driver (`rtw89_wifi`) · **Owner:** `DRV` · **Priority:** P3 · **Status:** later: the hardening stage (the maintainer, 2026-11-08: everything runs first; 2026-11-25: hardening is a stage of its own) · **Blocked by:** — · **Main task:** [174](174-full-use-of-pc-hardware.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.3, MC-12.5

Split from [213-DRV-0032](../issues-done/213-DRV-0032-rtl8852be-wifi.done) when it closed.

## Problem

To get the Ryzen's RTL8852BE joined and carrying traffic in November 2026, nine constants in the driver turned a strict stop into a logged continue, or a source-defined value into one that worked on the bench. Each has its strict path beside it and host cases for both where the path is the driver's own. All nine are on in `main`, and nothing says yet which of them is the driver's intended behaviour and which is a crutch.

| Constant | Where | What it does while on |
|---|---|---|
| `KEEP_SENDING` | `service.rs` | data mode only logs a `set-country` it would refuse, and goes on sending |
| `OUTLAST_SER` | `data_mode.rs` | waits up to 60 ms for the baseband and the scheduler after a firmware recovery |
| `RPQ_ALONE` | `tx.rs` | a data queue's frame is final on DMA and the release queue, without the firmware's report |
| `DROP_UNACKED` | `tx.rs` | a frame released as dropped is answered as not acknowledged, not a stop |
| `REPORT_EACH` | `post_mac.rs` | a release report for every frame (Linux batches 30) |
| `SEND_TRIES` | `service.rs` | at most 8 submissions a send |
| `OWN_DEADLINES` | `data_mode.rs` | an operation keeps its own deadline while a frame is pending |
| `KEEP_LATE` | `service.rs` | a send that outruns its time is answered as not acknowledged and its release awaited, not a stop |
| `RESTART_AFTER_FAILURE` | `main.rs` | after a failure the device stops, powers off and quiesces, the driver exits, and `init` starts a fresh one |

## Plan

For each constant, on evidence from the Ryzen and from Linux's source:
1. **Decide:** it is the driver's behaviour (the constant and the strict path go, the behaviour is described where it acts), or it hides a fault (the fault is found, and the strict path comes back).
2. **`KEEP_SENDING` first:** it is the one that lets the radio send against a country's limits, so it is a regulatory matter and not only a robustness one.
3. **Record** each decision here with its run.

Nothing of this is promised by a date (MC-12.3), and none of it is started.

## Acceptance criteria

- [ ] Each of the nine is decided, with its evidence, and the code has no constant left that only relaxes a check.
- [ ] `KEEP_SENDING` is off, or gone, with the Ryzen still joined through a change of country (MC-12.1: that machine and its access point).
- [ ] The host cases cover the behaviour kept, and [docs/profile/network.md](../docs/profile/network.md) says what the driver does on each failure.

## Related

[213-DRV-0032](../issues-done/213-DRV-0032-rtl8852be-wifi.done) (where each was introduced, with the run that needed it), [213-DRV-0090](213-DRV-0090-the-receiver-goes-deaf-on-a-busy-channel.md) and [213-DRV-0091](213-DRV-0091-the-firmware-told-of-the-link.md) (the firmware's recoveries and the late frames), [550-DRV-0088](550-DRV-0088-the-owners-rule-beside-the-catalogue.md) (the other hardening task of the radio).
