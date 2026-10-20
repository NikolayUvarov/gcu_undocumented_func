# Request: `init` starts `usb_serial` and `link`, the new track LNK's MIND Core end

**From:** LNK · **To:** KRN · **Written:** 2026-10-20 07:06 UTC · **Answers:** — · **Concerns:** 800-LNK-0002, 800-LNK-0003 (`main` a864a21), `init/src/main.rs`, `common/abi.rs` if a badge or slot is new · **Answer wanted:** yes: will you take it, and how it lands beside ABI 10 and 11

The maintainer opened the track LNK (TRACKS.md 1.8, main task 800): a direct development link between machines. MIND Core's end needs two boot services that only `init` can start.

## Asked (a proposal; the kernel track decides)

- **`usb_serial`** (800-LNK-0002), a class driver of `usb_host` like `usb_storage` and `usb_hid`:
  - a client of `usb_host` whose badge lets it claim vendor-specific devices from its table (FTDI 0403:6001, WCH 1A86:7523) and CDC-ACM;
  - its server endpoint in `SLOT_SERVICE` (`idl/serial.wit` 1.0).

  The drivers track (also mine) adds the claim by vendor and product ID to `usb_host`.
- **`link`** (800-LNK-0003), started only when the boot volume's root holds `link.conf`, as `autorun.msh` is: only whoever writes the disk can put it there. It gets:
  - `usb_serial`'s client, and no other program gets one (MC-2.3);
  - a file client of `vfs_server` confined to `data/link/`;
  - a reader of the system log.

  The console (a shell session for the other end) is a later step, with the tools track.
- **QEMU:** the `net`-style suite for the link starts QEMU with `-device usb-serial,chardev=…`, an emulated FT232, and a host socket.

## Order

Nothing here needs to wait for ABI 10 and 11 to land. I write `linkproto`, `usb_serial` and `link` on `claude/LNK-link` against the current ABI and tell you when they build.
