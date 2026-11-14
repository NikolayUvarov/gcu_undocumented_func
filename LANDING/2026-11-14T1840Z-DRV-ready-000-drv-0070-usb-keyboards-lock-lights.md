# DRV landing request: USB keyboards' lock lights (000-DRV-0070)

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-14 18:40 UTC · **Concerns:** 000-DRV-0070

**Ready:** `claude/000-DRV-0070-keyboard-lock-lights` de34ec6292680b68c790e8e5c590382b2afabd6a

## What it carries

`main` 32272bba66 merged in (de34ec62), with no conflict. One commit of code, 983f8d75, in `fast-test` already:
- **`libmind/src/keys.rs`:** `Ps2::locks()` gives Num, Caps and Scroll Lock from the decoder that chooses the letters' case and the keypad's keys, so the lights cannot disagree with them. Scroll Lock is now kept. Pause and Ctrl+Break touch no lock. With Caps Lock as the layout switch, it locks nothing.
- **`libmind/src/hid.rs`:** a press of Caps, Num or Scroll Lock toggles once and does not repeat. Before, Caps Lock held for a second toggled the lock about fifteen times.
- **`usb_hid/src/main.rs`:** after each round of polls, every keyboard whose last report differs from the locks is sent the boot protocol's one-byte output report by SET_REPORT (0x21, 0x09, 0x0200, its interface; HID 1.11, 7.2.2 and B.1). That covers a change and a keyboard just set up. A refusal is logged once and not sent again, and that keyboard's keys are still read.
- **Tests:** `tests/lock_lights_host.rs` (new), `tests/hid_host.rs`; in `tests/qemu_smoke.py`, the `usb` suite checks the reports at set-up, on Caps Lock and Num Lock with the case and the keypad, and on a keyboard plugged in with both on. The `ehci` suite checks the report over EHCI.
- **Text:** `README.md`, `docs/profile/evidence.md` and `docs/profile/aarch64/evidence.md` (the suites' evidence lines), the issue, its `issues/README.md` row. 45225509 and ceef1a30 are text only.

**With round 136's `main`** (40c761e6), `git merge-tree` finds no conflict. Both sides change `tests/qemu_smoke.py`, which merges by itself. Not tested on that merge.

**Issues:** advances 000-DRV-0070. Closes none: the Ryzen's run (17EF:60A1, and a keyboard plugged in after Caps Lock) is left.

## What was tested, and where

LEG9, WSL2, Ubuntu 24.04, QEMU 8.2.2 (TCG), 4 CPUs, on de34ec62's exact tree:
- **Builds:** `./02_build.sh` (x86) and `scripts/x86_fixtures.sh`.
- **The full `scripts/host_tests.sh`** with `MIND_SSHD`: exit 0, 230 suites, 6251 passed, none failed. `lock_lights_host` and `hid_host` are among them.
- **The group "x86: keys, shell, tools"** (`keys`, `shell`, `tools`, `bench`, `windows`, `tablet`, `usb`, `ehci`, `nnd`): PASS, 468 s. Among its checks:
  - `usb`: `USB keyboard behind a hub, tablet and mouse: keys, layouts, repeat, lock lights, a click, movement, hot plug, driver restarts, a keyboard on a second xHCI`;
  - `ehci`: `a keyboard on EHCI with a tablet on xHCI types and takes its lights, and the other way round`.

**Recorded in the issue, not run again here:**
- the aarch64 `usb` suite (QEMU `virt`, 4 CPUs) passed on this branch before the merge (45225509 records it);
- **the MacBook Pro** (release 202611140614, the same `usb_hid`): `05AC:0263 LOCK LIGHTS: … (REPORT 00)` and `05AC:820A` at set-up. The maintainer saw Caps Lock light its key and turn the letters to capitals, off again on the next press, and a long press acting as a short one.

**Not tested** (MC-12.3): the Ryzen's Lenovo keyboard 17EF:60A1, a keyboard plugged in after Caps Lock on real hardware, a keyboard that refuses SET_REPORT, and aarch64 on the merged tree.
