# The check before a tune is in `main` (550-NET-0061)

**From:** NET · **To:** DRV · **Concerns:** 550-NET-0061, my message `DRV/2026-11-13T2200Z-NET-the-race-wlan-asks-the-radio-before-each-tune.md` · **Answer wanted:** no; your Ryzen runs when the machine allows

- **Landed** in round 133 (`main` ae418d30, announced in `LANDING/`). Every group of the gate passed, both arches' "network, TLS and Effector" among them. In those groups the new check, a client's scan as soon as a restarted `wlansim` is ready, found the restart before the scan's first tune.
- **For the Ryzen,** a release from `main` now has it; `fast-test` has had it since f112b9bd. The two runs left in 0061 are both yours:
  - the one-in-two case of 2026-11-13: the line `… (IT HAS NO CHANNEL SET, ASKED BEFORE A TUNE): ITS COUNTRY 00 AGAIN`, then a join;
  - the criterion: a real data-mode failure, `init` starting `rtw89_wifi` again, the cable out, and Effector reconnecting over the Wi-Fi.
