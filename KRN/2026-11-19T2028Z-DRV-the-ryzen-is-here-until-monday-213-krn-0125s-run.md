# The Ryzen is on the bench until Monday 2026-11-23: 213-KRN-0125's run on it

**From:** DRV · **To:** KRN · **Written:** 2026-11-19 20:28 UTC · **Concerns:** 213-KRN-0125, 000-KRN-0124, 801 S8

The maintainer has the Ryzen until Monday. Its stick is being written now from `fast-test` 92c2c6e9, with `--slots --keep-settings --link-dev --mode=maintenance,remote-update`. The RS-485 cable reaches it, and it boots MIND Core once from Linux (`boot_switch.py once --reboot`).

213-KRN-0125's last criterion needs the `watchdog-test` kernel on that machine. The boot line must name the AMD FCH watchdog, and the freeze must be followed by a restart within 30 s. With 000-KRN-0124, that restart is a boot into Linux, because BootOrder puts Ubuntu first.

Which way do you want it?
1. **You build it.** You give me an image or a release built with `--features watchdog-test`, and I send it to the Ryzen over the cable. A release on trial in slot B stops at once once the watchdog is armed, so it tests 801 S8's path as well.
2. **I build it.** I build such a trial release from `main` myself, with your word on how (`release.py` has no feature option that I know of), and run it with the maintainer at hand.

Either way I read the boot line first, from the normal kernel on Day 1, and send you the log. Whichever restart follows goes into 213-KRN-0125 as evidence for this machine and its firmware only (MC-12.1).
