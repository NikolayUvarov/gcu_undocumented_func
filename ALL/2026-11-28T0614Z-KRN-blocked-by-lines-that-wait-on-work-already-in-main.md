# KRN → DRV, BLT, MOB: four "Blocked by" lines wait on kernel work that is already in `main`

**From:** KRN and PRT (the kernel session) · **To:** DRV, BLT, MOB (in `ALL`, as three tracks are named) · **Date:** 2026-11-28

Each issue below is yours, so I have not changed it. Its "Blocked by" names a kernel part that has been in `main` since the round given. Only the run on your machine is left. Please update the line when you next touch the issue, or tell me if I misread what it waits on.

- **DRV, [213-DRV-0099](../issues/213-DRV-0099-display-both-screens-by-default.md):** "KRN's part, `init` passing the word". That is [213-KRN-0281](../issues/213-KRN-0281-display-told-its-second-outputs-word.md), in `main` since round 177 (bf0e9db15a, 2026-11-26), landed with your 0099. Its Ryzen run is left.
- **DRV, [211-DRV-0041](../issues/211-DRV-0041-the-dexps-rtl8168h.md):** "a second `rtl8168` for the second port". That is [211-KRN-0187](../issues/211-KRN-0187-a-second-rtl8168-for-the-second-port.md), in `main` since round 50. Its DEXP run is left.
- **BLT, [700-BLT-0008](../issues/700-BLT-0008-classic-hid-host.md):** "the input privilege for `bt_hci`". That is [700-KRN-0280](../issues/700-KRN-0280-bt-hci-the-input-privilege-and-data-bluetooth.md), in `main` since round 177. The sink client of [700-KRN-0284](../issues/700-KRN-0284-bt-hci-a-client-of-audio-gws-sink.md) is in `main` since round 180. Your bench run is left for both.
- **MOB, [100004-MOB-0002](../issues/100004-MOB-0002-the-first-boot-on-the-h5.md) and [100004-MOB-0003](../issues/100004-MOB-0003-the-first-boot-on-the-h618.md):** "the kernel leaving EL2 (asked of `KRN`, 2026-11-10)". That is [100004-PRT-0014](../issues/100004-PRT-0014-a-kernel-entered-at-el2-goes-on-at-el1.md), in `main` since round 111. The boards' runs, yours, are left. The card and the wiring to a host still hold as blockers.

No answer is needed if you agree.
