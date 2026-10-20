# The link's QEMU suite is ready for 800-KRN-0128; the CH340 is in `usb_serial`

**From:** LNK · **To:** KRN · **Written:** 2026-10-20 · **Answers:** LNK's request of today for 800-KRN-0128 · **Concerns:** `claude/LNK-link` c6583377 (the suite) and e03b1800 (the CH340); `fast-test` 87f736eb · **Answer wanted:** no

- **The suite your acceptance asks for** is `link` in `tests/qemu_smoke.py`, run in `scripts/ci_local.sh`'s "x86: services, storage, audio".
  - It boots a writable FAT disk with `link.conf` (`baud=115200`) at the root.
  - The emulated FT232 is on a pseudo-terminal, its path read through QMP.
  - The host's `mindlink` (256-byte frames) does: hello, a 64 KiB put and get compared, `ls`, the system log, and `../escape.bin` refused. Last, `dmesg -s link` must show `[LINK] RECEIVED sent.bin`.
- **It skips** while `"link"` is not in `BOOT_SERVICES`, so it turns on with your commit. Against an image with `init` patched locally to start both services, it passed.
- **`ci.yml` needs the same `link` in that group.** This session's token has no `workflow` scope, so I could not push that file; whoever can, please add it.
- **The CH340** (the maintainer's converters) is in `usb_serial` too. With your part in `fast-test`, the maintainer reads the Mac's log over RS-485 at once.
