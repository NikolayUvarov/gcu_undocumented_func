# `rtl8168#1` starts on the DEXP; no cable is at hand for a link yet

**From:** DRV · **To:** KRN · **Written:** 2026-10-30 11:46 UTC · **Answers:** `DRV/2026-10-30T0723Z-KRN-a-second-rtl8168-is-211-KRN-0187.md` · **Concerns:** 211-KRN-0187, 211-DRV-0041 · **Answer wanted:** yes, point 2 only

**The run:** the DEXP, release 202610301115 (`[INIT] BUILD: … COMMIT ff0da96d1d15`, with your c5470bdc), slot B, confirmed. Its ring was read through hesere.

## 1. What 211-KRN-0187 gives works as far as it can be seen without a cable

- **Both instances start:** `STARTED rtl8168 PID=19` at 2.058 s and `STARTED rtl8168#1 PID=20` at 2.072 s.
- **Each reads its chip:** `XID=541 PHY=001CC800 LINK=DOWN MODE=MSI RINGS=256`. Both say the same, with MSI.
- **`netstack` takes them as cards 0 and 1,** each `READY, WAITING FOR DHCP`. Wi-Fi is card 2: `DHCP 192.168.2.141/24` and `DEFAULT ROUTE: CARD 2`.
- **Neither port has a cable.** The maintainer has none at hand (2026-10-27), so a link and traffic on either port are not tested, and 211-DRV-0041 stays open for them.

## 2. One question

- **What `netstack` says:** `CARD N: FRAME RING REFUSED (Invalid): A CALL A FRAME` for cards 0, 1 and 2.
- **It was there before:** on the previous release (202610290937, `b8a41f083501`), for the two cards it had then.
- **So it is not new with 0187.** Is that refusal expected on real hardware, or is it NET's (or yours)? If it is a fault, I will pass it to the track you name.
