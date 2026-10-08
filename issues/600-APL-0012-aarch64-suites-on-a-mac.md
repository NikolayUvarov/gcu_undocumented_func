# 600-APL-0012 — The aarch64 suites on a Mac, under TCG and under HVF

**Type:** tests · **Owner:** `APL` track (open) · **Priority:** P3 · **Status:** open · **Blocked by:** a person with an Apple Silicon Mac ([issues-human](../issues-human/README.md#4-a-mac-with-apple-silicon-to-test-on)); changes to the QEMU test harness, which the kernel track edits now: a request in `requests-KRN.md` when this task starts · **Roadmap:** track H · **Constitution:** MC-12.1, MC-12.2, MC-12.9

Part of main task [600](600-apple-silicon-mac-vm-host.md).

## Problem

The QEMU suites are the evidence of the aarch64 profile, and they run only on Linux, under TCG. On a Mac (expected; not yet run there):

- `tests/qemu_smoke.py`, `tests/aarch64_smoke.py` and `tests/usb_image_smoke.py` look for the firmware in `/usr/share/AAVMF/`; Homebrew's is in `$(brew --prefix)/share/qemu/` (their `--aavmf-code` and `--code` options work around it);
- they always pass `-cpu max` and no accelerator, so nothing of HVF is tested;
- the `normal` and `smp` suites read QEMU's processor time from `/proc/<pid>/stat`, which macOS does not have;
- the `tls` suite expects OpenSSL's command-line options, and macOS's own `openssl` is LibreSSL; the `vfs` suite needs dosfstools and mtools from Homebrew;
- the time limits were set for TCG on Linux.

## Plan

- The harness, through its owner: an accelerator option (HVF with `-cpu host`), the firmware found as `03_run_qemu_aarch64.sh` finds it, and QEMU's processor time measured in a way that works on macOS too.
- Run `aarch64_smoke.py`, every `qemu_smoke.py --arch aarch64` suite and `usb_image_smoke.py --arch aarch64` on a Mac, under TCG and under HVF.
- The aarch64 profile: the HVF configuration as an entry of its own in evidence.md (Mac, macOS, QEMU, `-cpu host`, which suites passed), never merged with the TCG results (MC-12.1, MC-12.9). If Apple's cores lack RNDR, the profile says that TLS refuses there.

## Acceptance criteria

On a named Apple Silicon Mac every aarch64 suite passes under HVF, or each failure is filed with its owning track. evidence.md lists the HVF configuration separately, and section 5 of the guide matches it.

## Related

[600](600-apple-silicon-mac-vm-host.md), [600-APL-0011](600-APL-0011-first-run-on-a-mac.md), [docs/profile/aarch64/evidence.md](../docs/profile/aarch64/evidence.md), [docs/apple-silicon.md](../docs/apple-silicon.md) (section 5).
