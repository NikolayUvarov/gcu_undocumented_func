# 550-NET-0098 — A client's join that comes during a scan waits for the scan's end

**Type:** `wlan` · **Owner:** `NET` (the kernel session) · **Priority:** P1 · **Status:** in progress (in `main` since round 183, c2318c10b3, 2026-11-28; DRV's run on the Ryzen left) · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md) · **Constitution:** MC-12.1, MC-12.3

## Problem

DRV's report `KRN/2026-11-26T1626Z-DRV-for-net-at-boot-the-station-joins-channel-8-though-channel-100-is-there.md` on `internal-dev-messages`. It holds for the Ryzen, boot 106 (`main` 84401caf4), under the kept country `00`.

**What happened at boot:**
- The automatic join's scan heard channels 36 to 48. After 17 of its tunes it stopped, before channel 100.
- The station then joined the 2.4 GHz access point on channel 8.
- Every scan after a restart of the radio ran all 37 tunes and joined channel 100. So 550-NET-0090's 5 GHz preference never got its turn at boot.

**Why.**
- The shell's autorun gives `wifi join CoolNet2` at each start, while the automatic join's first scan runs.
- `wlan` stopped that scan for the client's join (`join`: "its scan is stopped, so the station takes the join").
- The join then took the best of what half a scan had heard.

## Plan

- **A client's join that comes while the station scans** (its own, the automatic join's, or another client's) is held: `wire::Call::defer`, at most one.
- **The scan runs to its end.** Then the join runs on the whole list, and the client gets its answer.
- **An automatic try whose scan it was** ends ("TRY n ENDED BY A CLIENT'S JOIN"). The next try waits a first wait, as before.
- **A second join while one is held** gets `busy`.
- **The log says it:** `A CLIENT'S JOIN OF <name> WAITS FOR THE SCAN UNDER WAY TO END`.
- **The cost:** the client waits for the rest of the scan, about 6 s at most for 37 channels.

## Acceptance criteria

- [x] `wlan` builds with no warning.
- [x] The groups that run the `net` suite's Wi-Fi checks pass: "x86: network, TLS and Effector" and "aarch64: network, TLS and Effector". **Met** in round 183's gate (x86 on acc-2, 2175 s; aarch64 on acc-1, 1598 s), QEMU only (MC-12.1).
- [ ] **On the Ryzen** (DRV's run): at boot under `00`, with the autorun's `wifi join`, the station joins the 5 GHz access point when it is above the floor, and the log shows the join held.

## Related

- [550-NET-0090](550-NET-0090-the-station-on-5-ghz.md): 5 GHz first.
- [550-NET-0097](550-NET-0097-another-access-point-after-a-refusal.md): on the same branch.
