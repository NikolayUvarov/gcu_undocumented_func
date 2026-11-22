# DRV landing request: `usb_host` answers a long bulk transfer from its loop (000-DRV-0101)

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-22 20:30 UTC · **Concerns:** 000-DRV-0101 (advanced; it stays open: three failure cases are host-tested on its task branch and come with a second request)

**Ready:** `claude/000-DRV-0101-landing` bf9942b64

**After** `LANDING/2026-11-22T1858Z-DRV-ready-again-000-drv-0094-steps-2-and-3-and-000-lnk-0050-two-text-commits-more.md` (dd815c122): this branch grew from that line's code (43686b6a2) and holds it. A trial merge with today's `main` (7346ab3d5) is clean. No ABI change, no IDL change.

## What it carries

- **The change (05bc8e92c):** a bulk transfer on xHCI is started and looked for up to 2 ms, so a fast one is answered at once as before; a longer one is put into a table of 8 and answered from `usb_host`'s loop, which looks every millisecond and serves its other clients meanwhile. Its bytes go in a pool page of its own up to 4 KiB, else in the shared buffer, one such transfer a controller at a time. It is answered not-found if its device or controller goes, stopped after 30 s, and stopped when its client attaches anew. EHCI's bulk transfers are waited on as before.
- **A tally (3ad6e4d76):** one line at the boot's first transfer answered from the loop, and one line a minute in any minute that had some: deferred, answered, not found, timed out, stopped, waited on, the longest.
- **A test build and a QEMU suite, `usbload`,** added to the gate's services, storage and audio group in `scripts/ci_local.sh` and `.github/workflows/ci.yml`: the image boots from a USB stick with a `usb_host` built to defer every bulk transfer; a 4 MiB file is read off the stick three times while QEMU's usb-audio plays and the log is read over QEMU's serial converter.
- The issue's text (bf9942b64): the runs below and a section for whoever continues.

## Tested

- **On LEG9, on bf9942b64 itself** (18:51 to 20:19 UTC):
  - `02_build.sh`: success; the fixtures: built;
  - the full `scripts/host_tests.sh` with `MIND_SSHD`: exit 0, 263 groups, none failed;
  - QEMU `usb`, `ehci`, `iso`, `cameras`, `link`, `usbaudio` (with the audio stub's check), `vfs` (all eight), `block` (ATA, AHCI, USB) and `usbload`: 17 passed, none failed, none skipped.
- The aarch64 groups were not run.
- **On the Ryzen, as the boot image** (releases 202611221738 to 202611221915, boots 97 to 100, 17:43 UTC on; that machine and its devices, MC-12.1):
  - the stick, the serial converter and both cameras found in every boot; `A BULK TRANSFER ANSWERED FROM THE LOOP: SLOT 3 ENDPOINT 81, 65536 BYTES, 3 MS` (a read of the boot stick) in boot 97;
  - the tally in every minute that had any: deferred equal to answered, 0 not found, 0 timed out, 0 stopped, 0 waited on; up to 3453 in a minute, the longest 17 ms;
  - eight timed reads of the log over the cable in one minute (about 600 KB through the converter): 2914 deferred, all answered;
  - three runs of both cameras streaming (60 s and twice 180 s) with cable reads or a network test in their middle: no bulk ring empty for a wait; camera 1 lost no frame, camera 0 one and ten of about a thousand and three thousand, none by the camera's error bit;
  - four bench releases were installed through it (the stick written and read by the updater), each confirmed.
- **Not shown:**
  - an isochronous stream under it: the speakerphone has not been on the Ryzen since 11:16 UTC;
  - a real halt, a timeout or a client's death while a transfer is deferred (host cases for these are on the task branch, 613434c46, with a second request to come);
  - another controller or machine (MC-12.9).
- **Left, in the issue:** control transfers, commands and port resets still hold the loop for a few milliseconds each; 000-DRV-0081 (the order of a stream's close) is a task of its own.

It is in `fast-test` (4f8ace647).
