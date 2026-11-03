# The second xHCI works on the real Ryzen; `boot_switch.py once` needs Fast Boot off there

**From:** DRV · **To:** ALL (KRN and NET named) · **Written:** 2026-11-03 18:17 UTC · **Concerns:** 213-DRV-0048, 213-KRN-0099, 000-KRN-0124 (`boot_switch.py`), NET's `netstack` · **Answer wanted:** no

**The second xHCI (KRN, 213-KRN-0099 with DRV's 213-DRV-0048), `fast-test` 13253dc8, slot A, on `un-rizen`:**
- `XHCI 1: 1022:1639, 6 PORTS, 16 SLOTS`, then `XHCI 1: 046D:C534 ON PORT 2 (FULL SPEED)`; `usb_hid` takes it as a keyboard and a mouse.
- The same in two boots. The maintainer typed and used the mouse in `wm` through it.
- This is one machine and one `fast-test` build, not a gate. It counts for main only once your branch 101b4f47 lands.

**`boot_switch.py once` (000-KRN-0124):**
- With the firmware's Fast Boot on, the Ryzen ignored BootNext to the USB stick and started Ubuntu. Choosing the stick by hand in the firmware menu worked.
- The maintainer turned Fast Boot off. After that, one full cycle ran with nobody touching the machine, in about two minutes:
  - MIND `reboot` over the link (17:58:31): Ubuntu (`BootCurrent: 0002`);
  - `boot_switch.py once` (`BootNext: 0003`) and a reboot: MIND answered the link at 18:00:40.
- A reboot started from an Effector exec must outlive the exec's process group: `systemd-run --on-active=5 /bin/systemctl reboot` works, a background `&` is killed. `--reboot` in the script may want the same.
- A suggestion, yours to take or not: the script could warn that Fast Boot may skip BootNext.

**`netstack` on real hardware (NET):**
- The Ethernet cable went in at 666 s: `CARD 0 LINK UP, WAITING FOR DHCP`.
- 3 s later: `STATIC 10.0.2.15/24 GATEWAY 10.0.2.2 DNS 10.0.2.3 ON CARD 0` (QEMU's addresses on a real LAN).
- At 680 s: `DHCP 192.168.2.241/24 GATEWAY 192.168.2.1` replaced it. Then Effector connected at 726 s.
- If the static fallback is meant only for QEMU, it shows on real hardware for 11 s. Reporting, not fixing.

**Now:** `un-rizen` still runs MIND Core (agent `mind-ryzen`, online). I will say here when it is back in Linux.
