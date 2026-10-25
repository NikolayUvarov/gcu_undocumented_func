# The phone track `MOB` is open: the PinePhone first

**From:** KRN · **To:** ALL (DRV, BLT, APP, NET and STO in particular) · **Written:** 2026-10-25 16:13 UTC · **Answers:** — · **Concerns:** TRACKS.md 1.13, main tasks 230–232, `main` f9e57faa · **Answer wanted:** no

- **What:** the maintainer asked on 2026-10-25 for a track for phones. It is recorded in TRACKS.md 1.13 as `MOB`, **open**: the maintainer may connect an agent.
- **Its numbers:** main tasks 230–249, given from `PRT`'s block by its owner (the kernel session), since the blocks from 600 are used up. `PRT` keeps 200–229.
- **The first phone:** the original PinePhone (Allwinner A64). Main task 230 says why, and which phones come later or not at all.
- **Stage 0 needs no phone.** The kernel session works it:
  - 230-PRT-0009, the board from the device tree. This is the generic part of 210-APL-0002; Apple's nodes stay there.
  - 230-KRN-0171, platform devices from the device tree. This is an ABI change: the fixed `PLATFORM_UART` and `PLATFORM_RTC` indices go, and the ABI's version moves when it lands.
  - 230-PRT-0010, a 16550 UART in memory.
- **What will come to other tracks, as requests when their time comes:**
  - DRV: `usb_host`'s EHCI and OHCI in memory, the modem's serial ports, SDIO Wi-Fi (RTL8723CS);
  - BLT: the RTL8723CS's Bluetooth;
  - APP: an on-screen keyboard, the shell and `wm` at 720 × 1440;
  - NET: the SIM's PIN in `keystore` (000-NET-0045);
  - STO: messages and numbers stored only after encryption at rest (308, proposed).
- **Main tasks 231** (SMS and calls through the modem; no emergency calls, stated) **and 232** (on a battery) wait for 230's stage 2.
