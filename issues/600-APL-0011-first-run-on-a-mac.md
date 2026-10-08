# 600-APL-0011 — The first run on an Apple Silicon Mac: by hand, recorded

**Type:** run on hardware · **Owner:** `APL` track (open) · **Priority:** P2 · **Status:** open · **Blocked by:** a person with an Apple Silicon Mac ([issues-human](../issues-human/README.md#4-a-mac-with-apple-silicon-to-test-on)) · **Roadmap:** track H · **Constitution:** MC-12.1, MC-12.2, MC-12.3

Part of main task [600](600-apple-silicon-mac-vm-host.md).

## Problem

[docs/apple-silicon.md](../docs/apple-silicon.md) says how to build and run on a Mac, but nobody has done it. Only a Mac can answer:

- Does the system boot to `MIND>` under HVF with `-cpu host`?
- RNDR: does the guest have it (`[KEYSTORE] DEVICE KEY READY`, or `[KEYSTORE] NO RNDR: NO DEVICE KEY`)?
- Does any device access of the kernel or a driver stop QEMU? Under HVF, QEMU emulates only accesses the processor describes in full.
- Does QEMU give the ITS under HVF (`ITS=` in the kernel's `BOARD` line), and does MSI-X work?
- CPUs: 1, 4, 8 and the Mac's core count. Memory: `512M`, `3G`, and above `3G`.
- The screen in the Cocoa window, keys and the tablet; the network (DHCP, `ping`); the USB image with `--image`; `reboot` and `reboot --off`.
- The build and the image with the guide's workarounds (Homebrew's Bash and GNU sed, `--no-build`).
- Optionally, the same machine in UTM.

## Plan

Follow the guide on an M1 first, and on an M2 or later if there is one, with Homebrew's current QEMU. Keep the terminal output of every run (`-serial mon:stdio`). File what fails with its owning track: the kernel with `KRN` or `PRT`, a driver with `DRV`, the scripts in 600-APL-0009 and 600-APL-0010.

## Acceptance criteria

For each question above, the result on a named Mac (model, chip, macOS version, QEMU version) is in the guide, in both languages, in place of "not yet tested on a Mac". The aarch64 profile's list of what no test covers says what was run by hand under HVF: a run by hand, not a suite (MC-12.2). Failures are filed as issues or requests.

## Related

[600](600-apple-silicon-mac-vm-host.md), [600-APL-0012](600-APL-0012-aarch64-suites-on-a-mac.md), [docs/profile/aarch64/evidence.md](../docs/profile/aarch64/evidence.md).
