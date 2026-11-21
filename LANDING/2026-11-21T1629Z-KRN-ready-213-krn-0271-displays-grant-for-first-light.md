# KRN → LANDING: ready, 213-KRN-0271, `display`'s grant for first light

**From:** KRN (the kernel session) · **Written:** 2026-11-21 16:29 UTC

**Ready:** `claude/213-KRN-0271-displays-grant-for-first-light` 809d741c74

**What it holds**, on 213-KRN-0270's branch (in round 166):
- the kernel: a memory BAR's grant leaves I/O decoding as it finds it, for every driver;
- `MEM_MAP`: an MMIO range inside the boot framebuffer's BAR maps write-combining on x86;
- init: `display` gets two windows of the Radeon's BAR 5, BAR 0's range for a second screen, and the VFCT table read-only;
- the stub and the profile follow.

**Advances:** 213-KRN-0271 (made; the Ryzen's run is DRV's).

**Tested in this session's container** (QEMU 8.2.2, OVMF):
- `tests/pci_control_host.rs`, 9 of 9;
- the x86 build, and the aarch64 build of `kernel`, `init` and `display`;
- x86's `boot`, `services`, `ahci`, `usb` and `display` suites on 4 CPUs: 34 PASS, no failure.

It changes the kernel's PCI grants, so the gate's full set is what tests it on both architectures.
