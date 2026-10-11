# 600 — An Apple Silicon Mac as a host: the aarch64 system in a virtual machine with HVF

**Type:** porting (main task) · **Owner:** `APL` track (open) · **Priority:** P3 (was P2; the maintainer's goal is bare metal, 2026-10-09) · **Status:** parked (2026-10-09: the `APL` track waits for a Mac whose boot chain may be changed; [TRACKS.md](../TRACKS.md)) · **Blocked by:** a person with an Apple Silicon Mac ([issues-human](../issues-human/README.md#4-a-mac-with-apple-silicon-to-test-on)) · **Roadmap:** track H · **Constitution:** MC-12.1, MC-12.3, MC-12.9

Opened on 2026-10-08 with the track `APL`, at the maintainer's request: a guide to running on Apple Silicon, and a track for what is not finished. The guide is [docs/apple-silicon.md](../docs/apple-silicon.md) ([Russian](../docs/apple-silicon_RU.md)); running natively is main task [210](210-apple-silicon-native.md).

## Problem

On an Apple Silicon Mac, `03_run_qemu_aarch64.sh` chooses Apple's hypervisor (`-accel hvf -cpu host`) and the firmware Homebrew's QEMU ships (commit f28467a, tools track). None of it has run on a Mac: CI and every local run are on Linux, under TCG. On macOS, as the scripts are written (expected, not yet run):

- the aarch64 build stops: `scripts/build_aarch64.sh` needs Bash 4 (`mapfile`) and GNU sed, and macOS has Bash 3.2 and BSD sed;
- `04_make_usb_image_aarch64.sh` stops the same way, through `02_build.sh`;
- the run script opens no window, and under Bash 3.2 stops when `MIND_NET=none`;
- the suites look for Debian's firmware, cannot choose an accelerator, and read `/proc` for the aarch64 idle check.

And the configuration itself, HVF with `-cpu host`, is untested: whether the guest has RNDR (the key service and TLS need it), whether every device access of the kernel and the drivers is one QEMU can emulate under HVF, whether QEMU gives the ITS there. The profile's evidence is for TCG on Linux and does not carry over (MC-12.1, MC-12.9).

## Plan

- [600-APL-0009](600-APL-0009-aarch64-build-on-macos.md) — the aarch64 build with the Bash and sed macOS ships.
- [600-APL-0010](600-APL-0010-run-script-on-macos.md) — `03_run_qemu_aarch64.sh` on macOS: the screen in a window, Bash 3.2, a choice of accelerator.
- [600-APL-0011](600-APL-0011-first-run-on-a-mac.md) — the first run on a Mac, by hand, recorded in the guide and the profile.
- [600-APL-0012](600-APL-0012-aarch64-suites-on-a-mac.md) — the aarch64 suites on a Mac under TCG and HVF; evidence for the HVF configuration.

## Acceptance criteria

On a named Apple Silicon Mac (model, chip, macOS and QEMU versions):

- the build, the USB image and `03_run_qemu_aarch64.sh` work as the guide says, without its workarounds;
- the aarch64 suites pass under HVF, or each failure is filed with its owning track;
- the aarch64 profile states this configuration (HVF, `-cpu host`) with its own evidence and what it does not cover;
- the guide's "not yet tested on a Mac" marks are replaced by what was run, in both languages.

## Related

[210](210-apple-silicon-native.md), [docs/apple-silicon.md](../docs/apple-silicon.md), [docs/profile/aarch64/](../docs/profile/aarch64/README.md), [204](../issues-done/204-aarch64-profile-and-ci.done) (the aarch64 build and CI).
