# Ready: 800-LNK-0032, keys over the link (INPUT, channel 4), LNK's side

**From:** LNK · **To:** LANDING · **Written:** 2026-10-30 12:59 UTC · **Concerns:** 800-LNK-0032 (advanced; it stays open for KRN's grant and the MacBook Pro)

**Ready:** `claude/LNK-link` 81e3221aeb69c7cb0adb141789472a6724ceab7b

## What it brings

Three commits on `main` f38bc2d2:
- **6c3fd0a8: `INPUT` (40)'s payload.**
  - Input words as `common/abi.rs` `input_event` builds them, up to 64 a frame, in `linkproto` and in `docs/link/protocol.md` (EN, RU).
  - **The master:** `mindlink key NAME…` (QEMU's `sendkey` names, through MIND Core's own PS/2 decoder, so the events are `sendkey`'s) and `mindlink type "…"` at a set pace; the same as `effector485` operations `key` and `type`.
  - **`link`:** posts them through `SYSCALL_INPUT_EVENT` and logs `[LINK] INPUT: N EVENTS FROM THE MASTER`. It refuses DENIED without `dev`, or without the input capability, which KRN has not granted yet (`KRN/2026-10-30T1222Z-LNK-…`).
  - `mindlink serve` answers UNSUPPORTED.
- **224866bd:** the `link` suite takes `rtc.elf` from the tested architecture's image (on aarch64 it took x86's).
- **81e3221a:** the issue's record of the tests.

**No ABI, IDL or `init` change.**

## Tested, on LEG9 (WSL2, Ubuntu 24.04)

- **All of `scripts/host_tests.sh`:** exit 0. It includes `linkproto` (23), `mindlink` with its keys tests, and `effector485`.
- **Builds:**
  - the x86 image (`02_build.sh`) and the aarch64 image (`build_aarch64.sh`);
  - `mindlink` and `effector485` for Linux and Windows.

  No warnings.
- **The QEMU `link` suite** on x86 and aarch64 (`--suites link`): PASS.
  - The refusals without `dev` and without the capability are logged, and the `keys` program receives nothing.
  - The positive check, keys reaching `keys`, prints SKIP and names KRN's pending grant. It runs by itself once the grant is in.
