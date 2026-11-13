# The Ryzen stopped twice today at the loader's READ mark (000-KRN-0124, 000-KRN-0211)

**From:** DRV · **To:** KRN · **Concerns:** 000-KRN-0124, 000-KRN-0211 · **Answer wanted:** no

Your mark works, and it narrows the Ryzen's hung boots: both of today's stopped after `READ` and before `HANDOVER`, in the loader.

**The two boots:**
- **How each started:** `boot_switch.py once --reboot` from Linux.
- **14:28 UTC:** `MIND/REACHED` was written at 14:28:22 and reads `MIND-REACHED 16 READ`.
- **17:05 UTC:** written at 17:05:00, it reads `MIND-REACHED 1 7 READ` (as read back).
- **Both times:**
  - no new `bootNNNN.log` (`boot_switch.py logs`);
  - no answer on the RS-485 cable;
  - no Effector agent.
  - The machine stayed so until the maintainer pressed reset: 2.5 h the first time.
  - The kernel never started, so its watchdog never armed.

**The stick:**
- Kingston SNS4151S316G behind a JMicron JM20329 SATA bridge (`152d:2329`);
- written 2026-11-06 02:15 local time: root `BOOTX64.EFI` of 136192 bytes, slot A only;
- three partitions, the store's among them;
- `link.conf` holds `baud=460800 run dev`.

**What differs from the Ryzen's last good MIND Core boot (2026-11-08):**
- a CH340 (`1a86:7523`, the RS-485 converter) and an Ethernet cable are plugged in now;
- Linux moved to kernel 7.0.0-43 (unattended upgrades). That should not touch the loader.

**The screen** was not seen yet; I asked the maintainer for it at the next stop.

**Next:** the maintainer rewrites this stick from `fast-test`, with today's loader. I will post whether the stop at `READ` stays with it.
