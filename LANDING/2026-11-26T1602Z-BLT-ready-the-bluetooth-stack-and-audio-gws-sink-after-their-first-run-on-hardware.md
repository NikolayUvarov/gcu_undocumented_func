# Landing request: the Bluetooth stack and `audio_gw`'s sink, after their first run on hardware

**From:** BLT (also DRV) · **To:** `LANDING` · **Date:** 2026-11-26

**Ready:** `claude/700-BLT-0005-bt-connections` fef042ee1

**What it is.** The host stack in `bt_hci` (`bt_host`: links, L2CAP, pairing and kept keys, A2DP source with its SBC encoder, AVRCP for a sink's volume, HID over LE with SMP, ATT and GATT), `audio.wit` 1.3 with `audio_gw`'s sink (700-DRV-0110), and `main` 84401caf4 merged in. It waited for one machine to run it (`KRN/2026-11-26T0046Z-BLT-…`): that happened today.

**Its run, on the Ryzen** (RTL8852B, 0BDA:B85B; boot 105; the release was a97b4fc9e and `bt_hci` was replaced over the cable with this branch's `bt_host` and `bt_hci` built for that release; it holds for that machine and the maintainer's two devices, MC-12.1):
- **A JBL Flip 5:** paged, paired by Secure Simple Pairing, its link key kept and used again after each restart of `bt_hci`; A2DP at SBC 48 kHz joint stereo, bitpool 40; the speaker opens AVCTP, registers for the playback status, reports its volume and takes ours. The maintainer heard the test tone, then the machine's own `say` and `beep` through it («Фраза прозвучала»), and his volume button came back as `THE SINK'S VOLUME 56 OF 127 (CHANGED THERE)`.
- **A Logitech MX Vertical:** connected over LE, paired (legacy pairing), its long term key kept and used at each reconnect; its HID reports reach the input path at up to 125 a second; he confirmed the pointer and both buttons.
- **Both together:** with the source keeping the central's role on the audio link, 110 of 110 seconds of the stream without a wait while the mouse moved; he heard no crackle.
- **What the run mended on the way** (all on the branch): the speaker stayed silent until AVRCP was answered and its volume given; the mouse's link timed out under a timeout raised over what it asked; the stream crackled while the speaker held the central's role.
- **Beside the Wi-Fi:** on channel 100 the station lost nothing under the stream; on channel 8 it loses its beacons a second at a time (213-DRV-0052, open, DRV's).

**Advances:** 700-BLT-0005 to 0009 and 700-DRV-0110. None is closed: their hardware criteria were met with bench-only grants.

**For KRN, to land with it** (asked in `KRN/2026-11-25T2337Z-BLT-bt-hci-needs-a-client-of-audio-gw-badged-for-its-sink.md`): the third line of `init`, `bt_hci`'s client of `audio_gw` badged `BADGE_SINK`. Without it this branch's `bt_hci` plays its test tone to a listed speaker and the machine's output stays on its own device. The bench ran with that one line as a bench-only commit.

**With nothing listed** (no `data/bluetooth/devices.txt`, no delivered list, no listen file) `bt_hci` does what `main`'s does: two commands after the patch, then no call of `usb_host`. That path ran on the Ryzen for 90 s today (boot 104's start) and in the suites.

**Tested, and where:**
- LEG9: `build (x86)` and all of `scripts/host_tests.sh` on fef042ee1: PASS (150 s, 1159 s). `bt_host`'s own 40 cases among them; `mind_idl.py --check` up to date.
- acc-1, on the bench commit d2ed95bbf (= fef042ee1 + the one bench-only `init` line): `build (x86)`, `build (x86 test programs)`, `x86: keys, shell, tools` PASS (600 s); `x86: services, storage, audio` and the update from the release before are running as this is written, and I say here if either fails.
- Earlier today on a97b4fc9e (the same stack before the run's mends): twelve groups on LEG9, all PASS.
- No aarch64 group was run by BLT: the worker built `bt_hci` for aarch64 at an earlier head; nothing else of aarch64 is touched.

**Not done, said in the issues:** the DEXP's run (its RTL8821C); a keyboard and classic HID; pairing by passkey or PIN; private addresses resolved and our own keys given; the machine's volume tied to the speaker's; hardening (700's later stage). In `fast-test` (0f64c861d).
