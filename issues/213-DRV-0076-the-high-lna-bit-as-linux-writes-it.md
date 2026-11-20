# 213-DRV-0076 — `rtw89_wifi`: the WLAN scoreboard's high-LNA bit written as Linux writes it, not chasing Bluetooth's

**Type:** driver (`rtw89_wifi`) · **Owner:** `DRV` · **Priority:** P1 · **Status:** in progress (2026-11-20) · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md), [700](700-bluetooth.md) · **Constitution:** MC-6.1, MC-12.1

## Problem

On the Ryzen, once `bt_hci` patched the RTL8852B (700-BLT-0004), the Bluetooth scoreboard changed hundreds of times a second. Bits 31 and 13 (BT_HILNA) went `4000250B` ↔ `C000050B`. A diagnostic line on each change made logd refuse over 300 records a second.

**The cause is ours** (213-DRV-0052's coexistence, `btc_context.rs`):
- **The rule.** The WLAN scoreboard's high-LNA bit was cleared whenever Bluetooth's bit 13 was set, and set again for a linked station with BTG when it was clear.
- **The loop.** Bluetooth takes up the WLAN's bit. So each publish flipped Bluetooth's bit, and the next reconcile flipped ours. Every publish also writes the scoreboard with a 1 ms wait.
- **Why it never showed before.** The unpatched ROM never reported BTG, so the rule was never reached.

**Linux** (`_set_bt_rx_agc`, `coex.c` at the pinned `05f7e89a`) computes what is wanted: a link and the BTG receive. It writes the bit only when that differs from Bluetooth's own bit 13, so the two settle.

## Plan

`btc_context::hi_lna(wanted, bluetooth)` returns the bit to write, or nothing, as Linux decides. The reconcile uses it.

## Acceptance criteria

- [x] **Host test** (`tests/rtw89_connection_host.rs`, `the_high_lna_bit_is_written_only_while_bluetooths_differs`): the four cases, and a Bluetooth that takes up the bit, after which nothing more is written.
- [x] **On the Ryzen,** with the patched RTL8852B: one scoreboard change in 110 s after this `rtw89_wifi` went in, where there had been hundreds a second. Bluetooth then keeps its high LNA (`4000250B`).
- [ ] **On the Ryzen:** the Wi-Fi through inquiries every 80 s for half an hour, compared with the hour before the fix:
  - before the fix: three data-mode stops in 48 inquiries, one of them a coexistence reconcile's register fault (`Coexistence(Session(Register(Fault { PowerState, 0x38 })))`).

## Related

[213-DRV-0052](213-DRV-0052-coexistence-after-association.md) (the coexistence), [700-BLT-0004](700-BLT-0004-bt-hci-patches-the-rtl8852b.md) (the patch that brought the rule into play).
