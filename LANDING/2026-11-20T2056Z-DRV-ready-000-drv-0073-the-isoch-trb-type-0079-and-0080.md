# DRV landing request: the Isoch TRB's type (000-DRV-0073), a further try resets its port (000-DRV-0079), the USB audio device set up last plays (000-DRV-0080)

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-20 20:56 UTC · **Concerns:** 000-DRV-0073, 000-DRV-0079, 000-DRV-0080

**Ready:** `claude/DRV-ryzen-peripherals` ed5e76b87

## What it carries

On `main` 03c88c514; in `fast-test` 4eb1e7af2 (the code; the last commit here is text).

- **000-DRV-0073, `usb_host`: the Isoch TRB is type 5.** It was written as 17, the Reset Device command's.
  - QEMU takes a TD by its endpoint's type, so its suite passed while no byte moved.
  - The Ryzen's AMD xHCI stopped with a host controller error at the first such TD.
  - Everything on the DEXP's one Intel xHCI stopped at its first sound.

  The `usbaudio` suite now reads `usb-audio`'s wav and asserts the tone reached it. The AMD gate stays in until the Ryzen's run.
- **000-DRV-0079, `usb_host`, closed here:**
  - a further try at a root port's device resets the port even when it is enabled, with 50 ms after it;
  - a failed step's own completion code stays for the report;
  - the reset is logged.
- **000-DRV-0080, `audio_gw`:** of several USB audio devices the one set up last plays, and the one before it comes back when it goes. The DEXP has USB audio of its own, so a speakerphone plugged in beside it was never used. The `usbaudio` suite plugs a second device in, plays into it, and unplugs it.
- **Text:** the three issues and their rows.
- **No ABI or IDL change.**

## Tested

- **On LEG9, the tree 05170b9ba** (ed5e76b87 adds issue text only):
  - `build (x86)`, `host tests`, `build (x86 test programs)` and `x86: keys, shell, tools` pass.
  - **`x86: services, storage, audio` is red at its `ahci` suite, as on `main` itself.** Its suites `services`, `audio`, `tts`, `listen`, `hda`, `usbaudio`, `link` and `modes` pass. `ahci` fails on LEG9 on `main` 03c88c514 too, in 2 clean runs of 2 and 4 more with prints. The cause is in `KRN/2026-11-20T2046Z-DRV-the-ahci-suite-fails-on-leg9-on-main-vvfats-slack-bytes.md`: vvfat's bytes past a file's end change between reads, and `vfs_server`'s medium comparison takes that for another medium. Round 161 passed the group on its machines.
  - aarch64 is left to the gate.
- **On the DEXP** (Intel xHCI; releases of its ABI 34 tree with these commits, put through the cable, each trial confirmed):
  - 202611201916: a 3 s tone to its own USB audio 0573:1573: `STREAM ENDED: 3972 PACKETS SENT, 0 MISSED, 0 TIMES DRY, 0 WITH ERRORS`, and the machine went on. Before the fix it stopped at that tone twice.
  - 202611202012: the speakerphone in root port 2 fails each of three tries with `COMPLETION 4` at ADDRESS DEVICE, the second and third after `RESET FOR ANOTHER TRY`.
- **Not tested:**
  - the Ryzen's AMD xHCI with the fix. Release 202611201957 is on it and confirmed; its run waits for the speakerphone;
  - 000-DRV-0080 on the DEXP with two devices: the speakerphone is not set up in its root port.
