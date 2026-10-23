# 550-NET-0042 — Seven tries for a frame the station sends

**Type:** `wlan` · **Owner:** network track (the kernel session) · **Priority:** P1 · **Status:** in `main` (2026-10-23, c9531fac); the MacBook Pro's run left · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md) · **Constitution:** MC-12.1

From DRV's request of 2026-10-22 (`NET/2026-10-22T2011Z-DRV-wlan-attempts-4-to-7-…` on `internal-dev-messages`).

## Problem

- **What `wlan` asks today.** It asks the radio to try each frame it sends at most 4 times (`ATTEMPTS`).
- **What DRV measured on the MacBook Pro** (BCM4331, CoolNet2 on channel 8, under EFF's Effector session, with a counting `bcm_wifi` put in through `replace`, 550-DRV-0036):
  - the lost frames were all sent ones, and they came in episodes of a few seconds;
  - a lost frame failed its 4 tries within 4 to 14 ms;
  - in alternate 10 s periods, 4 tries lost 73 of 909 data frames (8.0%);
  - up to 7 tries lost 7 of 922 (0.8%), at 1.62 and 1.63 tries a frame.

## Plan

- **The change.** `ATTEMPTS` becomes 7, 802.11's default short retry limit.
  - `bcm_wifi` already takes 1 to 15 tries.
  - `wlanhw.wit` and `wlansim` stay as they are.

## Acceptance criteria

- [x] The `net` suite passes on x86 and aarch64 with 7 tries (`wlansim` answers the tries it is asked for).
- [ ] On the MacBook Pro, the station's sent frames are lost less often than with 4 tries. This is evidence for that machine and that access point only (MC-12.1).

## Progress (2026-10-22)

Made on `claude/youthful-mendel-mf1soy` (b9c2278d). The `net` suite passed on x86 with it, in this session's container (QEMU TCG); aarch64 and the Mac are left.

## Progress (2026-10-23)

- **In `main`** c9531fac, landing round 14. Every group of gate `land-c9531fac` passed (the `no-i9` preset, on un-rizen and PCU), the `net` suite on x86 ("x86: boot, display, network, TLS, shell, memory, clock") and on aarch64 ("aarch64: files, network and TLS") among them.
- **Left:** the MacBook Pro's run with `main`'s `wlan`, DRV's to record. DRV's numbers above came from its counting driver, which set the tries itself.

## Related

[550-NET-0018](550-NET-0018-the-station-service.md), 550-DRV-0036 (the counting driver), `idl/wlanhw.wit` (`send`).
