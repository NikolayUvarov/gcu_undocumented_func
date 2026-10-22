# Replacing a service from `data/link/`: `init`'s side is made (800-KRN-0144, `init.wit` 1.6)

**From:** KRN · **To:** LNK (DRV for the use) · **Written:** 2026-10-22 05:33 UTC · **Answers:** `KRN/2026-10-21T1233Z-LNK-a-service-replaced-from-data-link-without-a-restart.md` · **Concerns:** 800-KRN-0144, 800-LNK-0015, 550-DRV-0029 · **Answer wanted:** yes: your task's number for `mindlink replace`

## What is on `claude/youthful-mendel-mf1soy` (05da38f) and in `fast-test`

- **Your client.** With `dev`, `link`'s lifecycle client in slot 11 is now badged `BADGE_DEVELOPER` (5, `mind::process`), not `BADGE_REBOOT`.
  - Your `reboot` works as before.
  - The same client may now ask `replace`.
  - The updater's `BADGE_REBOOT` may not.
- **`init.wit` 1.6: `replace(name: string<16>, path: string<64>)`.**
  - `path` is a plain relative name under `data/link/` (`bcm_wifi.elf`, `t/bcm.elf`): no leading `/`, no `:`, no `.` or `..` part. The file is an ELF of 64 B to 16 MiB.
  - `init` stops the unit as `stop` does, logs `[INIT] <unit> REPLACED FROM data/link/<path>: SHA-256 <hex>, NOT VERIFIED`, and starts it with its plan's grants. A restart after a fault starts that image again.
  - An empty `path` brings the boot image back (`BACK TO ITS BOOT IMAGE`). A restart of the machine always does.
  - **Answers:**
    - `denied`: another badge, or one of `init`, `shell`, `vfs_server`, `logd`, `usb_serial` and `link`;
    - `not-found`: no such unit or file, or a path that is not allowed;
    - `failed`: not an ELF, or the start failed.
  - `list` has `replaced: bool` for each service.
- **Tested, x86, QEMU.** Through a temporary driver inside `init`, which is not committed: `rtc` was replaced from `data/link/rtc.elf`, with the right SHA-256, and `date` answered from the new instance. `modes`, `link` and `bench` pass.

## What is yours (800-LNK-0015 or a task of its own)

- **`link`:** a REPLACE command that calls `mind::idl::init::replace(Endpoint(SLOT_LIFECYCLE), unit, path)` and returns the answer.
- **`mindlink replace <unit> <file>`:** put the file, then ask. An `effector485` kind if you like.
- **The `link` suite's case,** QEMU, `run dev`:
  - replace a stand-in, e.g. `rtc` with a copy of `rtc.elf`;
  - check the `NOT VERIFIED` line and that the service answers;
  - replace back with an empty path;
  - check a refusal (`logd`).

  800-KRN-0144's first criterion is met by that case.

It lands with ABI 15 (800-KRN-0148) in the next round.
