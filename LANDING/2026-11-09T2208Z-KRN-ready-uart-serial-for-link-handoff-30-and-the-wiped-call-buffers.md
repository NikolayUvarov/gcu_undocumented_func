# KRN ready: `init` starts `uart_serial` for `link` (handoff 30), with LNK's `uart_serial`; the IDL clients' wiped call buffers again

**Ready:** `claude/youthful-mendel-mf1soy` 7415c9a16354

**From:** the kernel session (KRN, PRT, NET)

**Replaces** `2026-11-09T1937Z-KRN-ready-an-idl-clients-call-buffer-wiped-before-it-is-freed.md`, which round 105 returned.

**Carries LNK's returned request:** `2026-11-09T1915Z-LNK-ready-uart-serial-and-link-updates-race.md`, its commits as they were:
- db50334a `uart_serial`;
- b9fa8183 `link_update.py`.

Round 105 returned it for `wm`'s menu, which this request mends (below). I asked LNK at 2026-11-09 21:0x whether they agree; no answer yet. If LNK objects, I take them out.

**What it lands** (on `main` fe1d3cd6):
- **100000-KRN-0221 (e89223b9 and its progress): handoff 30.**
  - `uart_serial` becomes a boot image after `usb_serial`, so the images after it are renumbered, and `BOOT_HANDOFF_VERSION` is 30. ABI 29 stays.
  - **With `uart` in link.conf, `init`:**
    - starts `uart_serial` with the console UART in slot 2 (COM1's ports; the console's node on aarch64, or the kernel's channel where its page is shared) and its line in slot 4;
    - gives `link` its client in place of `usb_serial`'s;
    - leaves `usb_serial` and the shell's serial line out, and says so.
  - **Without `uart`,** nothing changes.
  - **The menu:** as a boot image, `uart_serial` is a service to the loader, so `wm`'s menu does not offer it. That was round 105's return of LNK's request.
  - `docs/profile/bootstrap.md` names the grants. The `VM` of `tests/qemu_smoke.py` takes `serial=`, and the `link` suite has a check over the console UART on a pseudo-terminal.
- **000-KRN-0226 (a4e5f772, 1cdb9926, 7415c9a1):** every generated client's call buffer is overwritten with zeros before it is freed (`mind::idl::wire::Wiped`).
  - The fix for round 105's return: `Wiped` lives in `wire.rs`, which the host tests' stand-ins compile as it is. DRV's `tests/rtw89_service_ipc.rs` builds again without a change to it.
  - `audio.rs` is regenerated after `main` brought DRV's new `audio.wit`.

**Tested** in the kernel session's container (QEMU 8.2.2, TCG, 4 CPUs):
- **On this tree, merged with `main` fe1d3cd6:**
  - `./02_build.sh` (x86): every program built;
  - `mind_idl.py --check`: up to date;
  - the host tests `boot_abi`, `idl`, `rtw89_service` (306), `services`.
- **`scripts/host_tests.sh`** on the branch before that merge: every Rust test, 192 sets.
  - `manifest_test.py` failed on this container's `cryptography`, as on `main` here.
  - The steps after it, run on: no failure.
- **On the branch before that merge:**
  - the `link` suite on x86 and on aarch64 `virt`, with the new check: 16 KiB each way through `uart_serial`; `[UART] 16550 …` and `PL011 … RECEIVED AT ITS INTERRUPT AND EVERY 2 MS`; `usb_serial` not started; the shell without its serial line;
  - the `wm` suite on x86 (17 checks): `uart_serial` off the menu;
  - the `shell` and `net` suites on x86 (54) and aarch64 (51) with the wiped buffers, in `mem.rs`'s first form.
- **Not run:** the other groups, which every program's rebuild touches.
