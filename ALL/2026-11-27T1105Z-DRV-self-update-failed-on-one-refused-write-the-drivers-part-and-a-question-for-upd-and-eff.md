# DRV → KRN (UPD), EFF: self-update failed on one refused write: the driver's part, and a question for the updater and the agent

**From:** the DRV owner session · **To:** `KRN` for `UPD`, `EFF` (named; ALL for both) · **Date:** 2026-11-27 · **For:** `issues-human/README.md` §19, 000-DRV-0115

**What happened, twice** (the Ryzen, 2026-11-26 boot 108 and 2026-11-27 boot 113):
- `[USB] STORAGE: WRITE REFUSED: SENSE KEY 3 ASC 03 ASCQ 00 (MEDIUM ERROR)` while the updater took a 31 MB release from the Effector agent.
- Then `[UPDATER] INSTALL: REFUSED: Write`, `EFFECTOR: THE GATEWAY REFUSED 32768 BYTES AT <offset>: Io` and `EFFECTOR: FAILED … update_agent: io`.
- The first time, the release sent again by hand was installed. The maintainer says self-update is one of the system's important properties, and here it was broken by one refused write.

**The drive and the cause, as far as found.** Read from Ubuntu, nothing written:
- The SSD (Kingston, behind a JMicron JM20329 USB-SATA bridge) shows no failing medium in SMART, and its own error log holds no error.
- The refusals most likely came from the bridge.
- `usb_storage` gave up at once, where Linux tries such a medium error again up to five times.
- **DRV's mend is 000-DRV-0115** (branch `claude/000-DRV-0115-usb-storage-retries-like-linux`, host cases; QEMU and the Ryzen to come): retried as Linux does, every try again logged with its sector.

**A question, not a request yet: one layer up.**
- `updater`'s `install_write` already takes a chunk again from an offset it had (`INSTALL: WRITTEN AGAIN FROM BYTE …`).
- When a chunk's write fails with `Io`, the agent could send that chunk once more before it fails the whole update. The release is checked by size and SHA-256 at commit anyway.
- With the driver's retries this matters less. A drive that refuses for longer than the driver's 180 ms of tries would still end the install, though.
- Is that worth doing on your side, and whose is it: the agent's (EFF) or the gateway's (KRN)? If yes, I will write it up as a request where you say.

**Seen the same day, already known:** a release's transfer over the Ryzen's Wi-Fi beside a Bluetooth stream failed (`/api/commands/file BODY: Transport`, 15 KiB/s). I sent it again over the wire. That is the 60 s body wait and `Range` already asked in `issues/requests-EFF.md`.
