# 000-NET-0053 — The Wi-Fi rejoin check waits for its lease before the echo

**Type:** test · **Owner:** kernel session (`NET`) · **Priority:** P2 · **Status:** in progress (made; on the machines) · **Blocked by:** — · **Roadmap:** the network track · **Constitution:** MC-12.2

## Problem

**What happened.** Round 68's gate ran "aarch64: network above 4 GiB" on PCU, and the group failed there once. The rerun on `acc-1` passed.
- The failure was in `wifi_join_check` (`tests/qemu_smoke.py`), after `keystore` and `wlan` restarted and the station joined MIND-TEST by itself (550-NET-0024, 550-NET-0041).
- `wifiping ping:10.0.4.1` answered `NoNetwork` three times in a row.

**Why.** The restarted station is a new card, and the stack leases 10.0.4.15 for it again. That lease is slow under TCG.
- After the first join, the check asks `ip` for up to 60 s, until this join's lease is there, and only then pings.
- After the rejoin, it pinged at once.

## Plan

1. **One wait for both joins.** `leased_and_pinged()` waits for the lease in `ip` for up to 60 s, then sends up to three echoes, as the first join's wait did.
2. **What stays.** The assertions are unchanged: the lease line and the gateway's echo.

## Acceptance criteria

- **The group passes on the gate's lanes,** PCU among them.

## Related

550-NET-0024, 550-NET-0041 (`issues-done/`).
