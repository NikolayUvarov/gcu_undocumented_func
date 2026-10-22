# 800-LNK-0016 — A service replaced over the link: `mindlink replace`, in developer mode

**Type:** service, tool · **Owner:** `LNK` · **Priority:** P1 · **Status:** in progress (2026-10-22) · **Blocked by:** — (`init`'s side is [800-KRN-0144](800-KRN-0144-a-service-replaced-from-data-link.md), in `main`) · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-3.12, MC-12.1, MC-12.3

## Problem

Driver work on the MacBook Pro (550-DRV-0029) changes one service many times a day. An update through the link (351-KRN-0141) takes about ten minutes and a restart. On LNK's request of 2026-10-21, KRN made `init.wit` 1.6 `replace(name, path)` (800-KRN-0144):
- with `dev`, `link`'s lifecycle client (slot 11, `BADGE_DEVELOPER`) may ask `init` to start a service again from an ELF under `data/link/`, with its plan's grants;
- it is logged `NOT VERIFIED`;
- an empty path, or a restart of the machine, brings the boot image back.

The link has no request for it yet.

## Plan

- **`linkproto`:** REPLACE on the programs channel: `unit` (at most 16 bytes) and `path` (a name under `data/link/`, at most 64 bytes, empty for the boot image). The answer is empty, or `init`'s refusal mapped to the link's codes:
  - `denied`: DENIED;
  - `not-found`: NOT_FOUND;
  - `failed`: IO.
- **`link`:** REPLACE calls `init::replace` on its lifecycle client; it is refused UNSUPPORTED without `dev`.
- **`mindlink replace <unit> <file> [<name>]`:** puts the file (under `replace/` by default), then asks. **`mindlink replace <unit> --back`** brings the boot image back. **`effector485`:** a kind `replace`.

## Acceptance criteria

- [x] **Host tests:** REPLACE through the lossy line (`replace_over_a_lossy_line`); `mindlink`'s tests pass with `replace`.
- [x] **The QEMU `link` suite with `dev`** (2026-10-22, x86, local), which meets 800-KRN-0144's first criterion:
  - `rtc` replaced with a copy of `rtc.elf`, the `NOT VERIFIED` line with its SHA-256, `date` still answered;
  - back with `--back`;
  - `logd` refused.
- [ ] **On the MacBook Pro:** `bcm_wifi` replaced over RS-485 with a build of DRV's, the Wi-Fi joined again, with no restart of the machine.

Evidence for the configurations named only (MC-12.1). A replaced service runs unverified until the next boot, as the log says (MC-3.12, MC-12.3).

## Progress

- **2026-10-22, written; the host tests and the QEMU `link` suite pass** (x86, local, the case above).
- **2026-10-22, the MacBook Pro** (`fast-test` 85fefd4, ABI 16, written with `--slots --link-dev --keep-wifi`), from Windows over RS-485:
  - `mindlink replace rtc` put a copy of `rtc.elf` (104 352 bytes) as `replace/rtc.elf`;
  - `init` logged `rtc REPLACED FROM data/link/replace/rtc.elf: SHA-256 bfaacc73…, NOT VERIFIED`;
  - `mindlink replace rtc --back` returned it to its boot image.
  - `bcm_wifi`'s replacement, the third criterion, follows the release that brings the shell's rule.

## Related

[800-KRN-0144](800-KRN-0144-a-service-replaced-from-data-link.md), [800-LNK-0015](800-LNK-0015-shell-commands-over-the-link.md), [550-DRV-0029](550-DRV-0029-bcm4331-receives-ofdm.md), [docs/link/protocol.md](../docs/link/protocol.md).
