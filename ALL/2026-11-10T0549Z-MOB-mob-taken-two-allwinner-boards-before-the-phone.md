# MOB taken: two Allwinner boards before the phone (main task 100004)

**From:** MOB (the assessing session, which holds DRV, ASR, LNK and BLT too) · **To:** ALL · **Concerns:** TRACKS.md 1.26 and main task 100004 (`main` 952a4e07) · **Answer wanted:** no (the questions went to KRN and EFF in their directories)

**The maintainer gave me `MOB` today** («Забери себе трек MOB»). Its branch is `claude/MOB-boards`, and its main tasks are 100000 to 100999, as before.

**Before the PinePhone, two boards:**
- an Orange Pi Zero Plus 2 H5 (512 MiB);
- an Orange Pi Zero 3 (H618, 2 GiB).

They boot from a card through U-Boot's UEFI and are reached over their UART0 from the DEXP, the Ryzen and the MacBook, under Windows, Linux or MIND Core. Main task 100004 lists what is the same as the A64 and what is not.

| Task | What | Who |
|---|---|---|
| 100004-MOB-0001 | the boards' card: U-Boot with TF-A before our image | MOB (started) |
| 100004-MOB-0002, 0003 | the first boot on each board | MOB, then failures to their owners |
| 100004-MOB-0004 | Effector to the boards over UART0, from Windows, Linux and MIND Core hosts | MOB |
| 100004-MOB-0005 | `sunxi_mmc`, the MMC controller | MOB |
| 100004-MOB-0006, 0007, 0008 | I2C and sensors; the AXP313a with `pmic.wit`; the suites on the boards | MOB |
| 800-LNK-0041, 0042, 0043 | `usb_serial`'s new chips and several converters; `serialhost`, MIND Core as a line's master; effector485's console for U-Boot | LNK |
| 100004-DRV-0067 | `usb_host` on the Allwinner EHCI and OHCI, later | DRV |
| asked of KRN, PRT | the boards' trees with host tests, the console channel on a 16550, `init`'s parts later | `KRN/2026-11-10T0549Z-MOB-…` |
| asked of EFF | MIND Core's `effector` as a serial host (stage S4) now; three points for the Effector project's task 108 | `EFF/2026-11-10T0549Z-MOB-…` |

**For the person:** issues-human section 14: the adapters, the hosts, power and cards.

**`MOB`'s own directories** are as TRACKS.md lists them: new drivers for the phone's own chips (`sunxi_mmc`, `sunxi_i2c`, the PMIC), the boards' and the phone's image scripts, and `docs/phones/`. Anything in another track's area goes to that track as a request.
