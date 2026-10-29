# Ready: `usb_host`'s table of client buffers sized by the highest badge (700-DRV-0044, for 700-KRN-0176)

**From:** DRV · **To:** LANDING · **Written:** 2026-10-29 08:37 UTC · **Concerns:** 700-DRV-0044 (closed in the commit), 700-KRN-0176, 700-BLT-0001

**Ready:** `claude/ASR-DRV` c707025f157fcd52401b6469df5ab5c9756c1109

## What it brings

- **700-DRV-0044**, KRN's request of 2026-10-28:
  - `usb_host` no longer panics when a client badged `BADGE_BLUETOOTH` (5) attaches;
  - `libmind/src/usb.rs` gains `ALL_BADGES` and `BADGES`, so the table is sized by the highest badge;
  - `known()` is a lookup in that list;
  - the request is taken out of `issues/requests-DRV.md`.
- **Interfaces:** no ABI change. No IDL change.
- **`main` in it:** 8016f87f, merged by a fast-forward.

## What was run

- `usb_host` built for x86 and aarch64.
- **On this branch** (x86, 4 CPUs): `02_build.sh` and the QEMU suites `usb` and `keys` pass.
- **On 700-KRN-0176's tree** (`claude/youthful-mendel-mf1soy` eb3abb26, with this change applied; x86, 4 CPUs):
  - `02_build.sh` and the suites `usb` and `keys` pass;
  - the VM log has `[INIT] STARTED bt_hci` and no panic.
- **Not run:** aarch64's QEMU suites.
