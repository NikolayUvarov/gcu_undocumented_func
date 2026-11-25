# DRV landing request: a stream is closed by telling the device first (000-DRV-0081)

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-25 23:49 UTC · **Concerns:** 000-DRV-0081 (advanced; it stays open for its run with the speakerphone)

**Ready:** `claude/000-DRV-0081-a-close-tells-the-device-first` 48320af5c

Branched from 000-DRV-0101's first landing (bf9942b64). No ABI change, no IDL change. `main` (d4453ef47) is merged in.

## What it carries

- **eab997a0d:** `usb_host`'s `select` to a setting without a stream, while the interface streams, dropped the isochronous endpoints first and sent SET_INTERFACE after it. The device kept its stream open with nothing coming, and the speakerphone counted 33 to 92 frames lacking at every close (the Ryzen, 2026-11-21). Now (`usb_host/src/close.rs`, `Controller::close_streams`):
  - the streams are marked closing, SET_INTERFACE goes while their packets still go out, and the endpoints are dropped after it; a client's release and its new instance close the same way;
  - an error completion after the mark is said on a line of its own, not in the stream's `WITH ERRORS`; a dry ring and a missed service count as always;
  - one line a close says how long the request and the drop took;
  - a select to setting 0 reads no configuration descriptor when the interface's setting-0 endpoints have no isochronous one.
- **Tests with it:** `tests/usb_close_host.rs` (the order, the descriptor rule, what each completion code counts as); QEMU's `usbaudio` checks the close line and 0 `WITH ERRORS` on the stream's last line.
- The issue's text: its first results and the bench run it needs.

## Tested

- **On acc-2 through Effector, on d3963f5c9** (this head less two merges of `main`'s text-only commits; no file outside `issues/` and `issues-done/` differs), 2026-11-25 22:15 to 22:57 UTC:
  - `build (x86)`: PASS; `build (x86 test programs)`: PASS;
  - `x86: services, storage, audio` (with `usbaudio`, `usbload`, `link`, `audio`, `hda`): PASS, 1160 s;
  - `x86: keys, shell, tools` (with `usb`, `ehci`, `cameras`): PASS, 633 s.
- **On x32-128 through Effector, the same commit:** `host tests`, the whole of `scripts/host_tests.sh`: PASS, 2652 s.
- **Not run:** the aarch64 groups.
- **Not shown on hardware:** anything. The issue's last criterion is the speakerphone's own count of missing and extra packets after a five-minute stream; the speakerphone has not been on the Ryzen since 2026-11-22, and the Ryzen is shared with another project until the maintainer says otherwise.

## With DRV's other request of this hour

`claude/000-DRV-0101-usb-host-answers-bulk-from-its-loop` also changes `usb_host/src/main.rs` and `tests/qemu_smoke.py`. A trial merge of the two heads is clean. Each was tested with `main` alone, not the two together.

Not in `fast-test`.
