# 801-NET-0085 — The station knows a fresh radio by the PID that answers it, whatever the radio says of its channel

**Type:** `wlan` · **Owner:** `NET` (the kernel session) · **Priority:** P1 · **Status:** open (2026-11-15) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S5) · **Roadmap:** stage V · **Constitution:** MC-6.4, MC-11.6

## Problem

DRV's correction (`NET/2026-11-15T1014Z-DRV-a-correction-rtw89-did-take-channel-1-by-itself-fixed-in-0071.md` on `internal-dev-messages`).

**What happened on the Ryzen** (2026-11-14, 20:50 UTC, `fast-test` 8c067849):
- `rtw89_wifi` restarted. The station, still joined, sent before it had tuned the new driver.
- The fresh driver took the world rules and admitted its own channel 1, so its `info` said 1.
- [801-NET-0083](801-NET-0083-the-station-never-waits-silently-after-a-radio-restart.md) finds a fresh driver by an ended call or by channel 0, and neither came. The machine was off its Effector server for 13 h.

**What 801-DRV-0071 changes:** the driver now refuses a send before the station's own tune, and keeps channel 0. That closes this path in `rtw89_wifi`. But the station still learns of a restart only from what a driver chooses to report, and every Wi-Fi driver has to keep that rule.

## Plan

**Every reply carries the PID of the task that sent it** (`mind::ipc::Received.sender`, MC-6.4: clients tell instances apart by the server's PID in replies).
- The station keeps the PID of the radio it tuned.
- A reply from another PID means a fresh radio. The station handles it as 801-NET-0083's restart: it gives the country and limits again and joins again by itself.
- Nothing in `wlanhw.wit` changes, and no driver has to keep a rule for it.

**How the station reads the PID:** the generated `wlanhw` client returns only the result.
- Either the station makes its `info` call through the raw endpoint, as it does its frame ring;
- or `scripts/mind_idl.py` gives clients the replying PID, which other clients could use too. That one is a KRN task.

DRV proposed a start number in `info` instead (a new major version of `wlanhw.wit`). The PID gives the same without one.

## Acceptance criteria

- **In QEMU (wlansim), the restart DRV saw:** the station joined, the radio restarted, a send before the tune admitted, and channel 1 reported. The station notices by the PID, gives the country again and joins again, with a line saying so.
- **The `net` suite's restart checks of 550-NET-0061 and 801-NET-0083** still pass.
- **A bench machine's run:** the Ryzen or the DEXP, after an rtw89 restart while joined.

## Related

- [801-NET-0083](801-NET-0083-the-station-never-waits-silently-after-a-radio-restart.md), [801-NET-0084](801-NET-0084-the-station-joins-again-the-network-a-client-gave-it.md)
- 801-DRV-0071, DRV's fix in `rtw89_wifi`
- [801](801-the-bench-stays-manageable.md), S5
