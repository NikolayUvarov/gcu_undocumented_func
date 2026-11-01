# Ready: 213-KRN-0099, init grants the second xHCI controller to `usb_host`

**From:** KRN · **To:** LANDING · **Written:** 2026-11-01 16:05 UTC · **Concerns:** 213-KRN-0099 (advanced; its Ryzen criterion waits for a release on the maintainer's machine), with DRV's 213-DRV-0048

**Ready:** `claude/youthful-mendel-mf1soy` 101b4f47

## What it brings

On `main` c331d1a8, with DRV's request 4756d380 merged in as it is:
- **`init`:** grants `usb_host` each further xHCI controller in PCI order, in `mind::usb::XHCI`'s slots: BAR0, which turns its bus mastering on, and its own 512 KiB DMA region;
- **a driver's restart:** the second controller is stopped and resumed with the first;
- **the `usb` suite:** DRV's keyboard on a second `qemu-xhci` is required, no longer a SKIP;
- **docs:** README's `usb_host` row and `docs/profile/bootstrap.md`.

My first try, with instances (371de749), is reverted in 8f2c3f97, so the branch's net change is DRV's commit and the grant.

**Order in the round:** DRV's request first. This one holds its commit, so it merges after it with nothing more.

## Tested, in this session's container (QEMU without KVM, 4 CPUs; no Effector key here since the container restarted)

- **The `usb` suite:** PASS on x86 and aarch64, with the second-xHCI check run: the keyboard on the second controller types, also after `usb_hid` and `usb_host` restart.
- **The rest of "x86: keys, shell, tools":** `keys`, `shell`, `tools`, `windows`, `tablet`, `ehci`, `nnd` PASS.
- **`bench`:** FAILED twice here, on `sleep 10 ms` (a median of 86.6 ms against a bound of 40 ms). That is this container's TCG timing: the suite's machine has no xHCI, so neither `usb_host` nor the grant runs in it. It needs the gate's machines.
- **All of `scripts/host_tests.sh`:** PASS. The container needed openssh-server for the SSH client's tests and the system Python for the manifest test's `cryptography`.
- **The x86 and aarch64 builds:** without warnings in `init`.

The full gate needs the Effector machines, once the key is back.
