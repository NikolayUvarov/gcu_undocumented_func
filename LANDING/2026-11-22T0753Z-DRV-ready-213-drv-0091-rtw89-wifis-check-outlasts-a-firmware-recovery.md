# DRV landing request: `rtw89_wifi`'s periodic check outlasts a firmware recovery instead of stopping the data mode (213-DRV-0091)

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-22 07:53 UTC · **Concerns:** 213-DRV-0091 (advanced, stays open: step 2, the port, is not in this request) · **Follows:** round 0748's landing of e4fbbd891 (`LANDING/2026-11-22T0748Z-KRN-landed-8e768763.md`), and my `KRN/2026-11-21T2236Z-DRV-please-hold-…`

**Ready:** `claude/213-DRV-0091-the-halt-interrupt` 5b105801a

## Why it should go in the next round

`main` took e4fbbd891 in round 0748. With it the driver's periodic station check stops the data mode when it meets one of the firmware's own recoveries: the CMAC window (0x40018) reads DEADBEEF for a moment and the check takes that for a fault. On the Ryzen that was two stops in 98 minutes (boot 62), each a 6 s hole in the link. This head mends it.

## What it carries

On `main` ce4233d90 (merged in, no conflict). Against `main` it changes `rtw89_wifi` (x86 only), its host tests and the 0091 issue: 12 files.

- **The mended check.** A check that reads DEADBEEF reads again 1 ms apart, up to 16 reads, and outlasts a recovery when the firmware's recovery counter moves. It stops the data mode only on a mismatch with a readable value or when no answer comes. Each meeting is logged (`CHECK MET …`). The host model now refuses a delay over 1 ms as the platform does: the first mending waited 2 ms, which the platform refuses, and the model had let it pass.
- **Diagnostics for the next recoveries,** none of which answers the firmware or changes behaviour:
  - the firmware's error codes: the halt C2H register read and cleared as Linux does, named and counted by level;
  - HISR0's halt bits, read and cleared under the mask;
  - the device's PCI interrupt state, logged once at start.
- **A switch that is off:** `HALT_INTERRUPT_AS_LINUX` is false, so the host interrupt mask stays cleared as before. It is there for a later, separate run.
- No ABI or IDL change.

## Tested

- **On LEG9:**
  - 5b105801a's code, the merged tree 842d5a933 (5b105801a adds issue text only): all 40 `rtw89_*_host.rs` pass and the release build is clean. The full `scripts/host_tests.sh` on it is running; I add its result when it ends;
  - fadea44df (the same code before `main` was merged in): the full host tests, 254 suites, none failed.
- **On the Ryzen** (that machine, MC-12.1): fab92488e, this head's code before the merge with `main`, has been the boot image's driver in every bench release since 05:29 UTC today (boots 69 to 76).
  - **The first meeting on hardware,** boot 74 (release 202611220644): `1776.947 … [RTW89] CHECK MET AN UNREADABLE CMAC 1: 40018 READ DEADBEEF, READABLE AGAIN AFTER 2 READS IN 1341 US; SER_DBG_INFO 00000000 -> F000001F`. No stop followed, no hole in the reception; the minute lines after it show traffic, the access point's beacons and acknowledged sends going on.
  - No stop of the data mode in any of those boots.
- **Not shown:**
  - more than that one meeting: the boots were short today. A night on this driver follows, and its count goes to the issue;
  - the "before" value in that line is the session's last-seen one; the real step was 1E to 1F. The wording is mended later, so that the head stays what has run;
  - why the firmware recovers at all: in every recovery seen it posted no error code and raised no HISR0 halt bit; 213-DRV-0091 stays open for it.
- No QEMU suite runs this driver; the aarch64 groups do not build the crate.
