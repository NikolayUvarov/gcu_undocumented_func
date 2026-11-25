# DRV landing request: what a deferred transfer's client is answered, host-tested, and `usbload`'s tally check (000-DRV-0101)

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-25 23:49 UTC · **Concerns:** 000-DRV-0101 (advanced: its first two criteria are met; it stays open for its run on the Ryzen)

**Ready:** `claude/000-DRV-0101-usb-host-answers-bulk-from-its-loop` 703c3add8

This is the second request the first one named (`LANDING/2026-11-22T2030Z-DRV-…`, landed as bf9942b64). No ABI change, no IDL change. `main` (d4453ef47) is merged in.

## What it carries

- **613434c46:** the rule for what a deferred bulk transfer's client is answered moves into `usb_host/src/pending.rs` (`pending::answer`), and the loop and a client's new attach act on it: the bytes moved; a halt (the endpoint reset, the client answered invalid); its device or controller gone (not-found, which outweighs a completion found with it); no completion within 30 s (stopped, timed out); its client's new instance (stopped, not-found). A case for each in `tests/usb_pending_host.rs`.
- **49bd2fede:** QEMU's `usbload` waits for the minute's tally line and checks it: every deferred transfer answered, none lost.
- The issue's text: the Ryzen's boots 97 to 100 with the first landing's code, and a section for whoever continues.

## Tested

- **On acc-1 through Effector, on da1be6e73** (this head less two merges of `main`'s text-only commits; no file outside `issues/` and `issues-done/` differs), 2026-11-25 22:15 to 22:55 UTC:
  - `build (x86)`: PASS; `build (x86 test programs)`: PASS;
  - `x86: services, storage, audio` (with `usbaudio`, `usbload`, `link`): PASS, 1143 s;
  - `x86: keys, shell, tools` (with `usb`, `ehci`, `cameras`): PASS, 600 s.
- **On x32-128 through Effector, the same commit:** `host tests`, the whole of `scripts/host_tests.sh`: PASS, 2668 s.
- **Earlier, on LEG9 (2026-11-22),** on 05ac6a253 with the same two code commits: the full host tests (263 groups) and 17 QEMU suites, none failed (the issue's Progress).
- **Not run:** the aarch64 groups.
- **Not shown on hardware:** a real halt, a timeout or a client's death while a transfer is deferred. The Ryzen's boots 97 to 100 ran the first landing's code, not these two commits.

## With DRV's other request of this hour

`claude/000-DRV-0081-a-close-tells-the-device-first` also changes `usb_host/src/main.rs` and `tests/qemu_smoke.py`. A trial merge of the two heads is clean (`git merge-tree`). Each was tested with `main` alone, not the two together: the candidate's gate is the first run of both in one tree.

Not in `fast-test`.
