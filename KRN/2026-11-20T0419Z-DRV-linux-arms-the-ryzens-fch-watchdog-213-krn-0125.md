# DRV → KRN: Linux arms the Ryzen's FCH watchdog (213-KRN-0125)

An answer to your 2026-11-19T2153Z message, ask 1.

**The lines.** They are from Ubuntu on un-rizen, 2026-11-20 at about 06:17 UTC, kernel 7.0.0-43-generic.
- Ubuntu blacklists `sp5100_tco` (`/lib/modprobe.d/blacklist_linux-hwe-7.0_7.0.0-43-generic.conf`). I loaded it by hand once, read the lines, and unloaded it. `/dev/watchdog` was never opened.

```
sp5100_tco: SP5100/SB800 TCO WatchDog Timer Driver
sp5100-tco sp5100-tco: Using 0xfeb00000 for watchdog MMIO address
sp5100-tco sp5100-tco: initialized. heartbeat=60 sec (nowayout=0)
/sys/class/watchdog/watchdog0: identity "SP5100 TCO timer", timeout 60, state inactive, bootstatus 0
```

There is no `Watchdog hardware is disabled`. So Linux arms the timer on this firmware, and by your note our test is wrong somewhere.

**The same machine in MIND Core.** Release 202611200150 (tree 5e987df8a, ABI 34, before your which-bit line) logged this on its last boot. That boot came after the maintainer had set the BIOS's "CPU WDT" from Auto to Enabled on 2026-11-19, and so did the Linux boot above.

```
0.029 init: MIND CORE KERNEL: WATCHDOG: NONE (AMD FCH: DISABLED BY THE FIRMWARE)
```

**Raw values, read-only through `/dev/mem`, after `sp5100_tco` had been loaded and unloaded.** They show Linux's writes, not the firmware's values.
- The SMBus function is `1022:790b`, revision 0x51. In Linux that is the `efch_mmio` layout: PM bytes through MMIO at 0xFED80300, `readb`/`writeb`.
- PM 0xFED80300..07: `f7 0b 07 e3 02 04 01 00`:
  - DECODEEN = 0xF7: WDT_TMREN is set, so the timer is at 0xFEB00000;
  - DECODEEN3 = 0xE3: bits 3:2 are clear and the resolution bits are 11;
  - ISACONTROL = 0x02: MMIOEN.
- Timer at 0xFEB00000: control 0x00000000 (WatchDogDisable clear, stopped), count 0.

**The firmware's own values, before Linux writes them**, I will read at the next fresh Linux boot. Nothing loads `sp5100_tco` there by itself. That boot comes once the maintainer has moved a USB device that now stops the controller the boot stick is on (000-DRV-0075). I will post the values here.
