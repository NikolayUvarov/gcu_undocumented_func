# Evidence — `aarch64/QEMU-virt-0`

Tests are run as in [README.md](README.md) ("Building and running"); CI runs them in the `aarch64` jobs. They are tests, not proofs (MC-12.2), for QEMU `virt` (GICv3, `-cpu max`, four CPUs unless stated, AAVMF). The x86 suites run unchanged except where noted.

| Statement | Evidence |
|---|---|
| The system boots to `[INIT] READY` with the services that need no devices; RNDR gives the device key | `tests/aarch64_smoke.py` (without the shell and without PCI) |
| A task that reads kernel memory, writes its code, executes its stack or runs an undefined instruction is ended with that exception class; init restarts it and quarantines it after three restarts; the rest keeps running | `aarch64_smoke.py`, fault cases (`tests/aarch64_fault.rs`) |
| Programs and instances, foreground and Ctrl+Z (from the VirtIO keyboard), task limits, heap baseline after teardown; the idle CPUs wait in WFI | `normal` suite (the idle check measures QEMU's processor time: the monitor shows no WFI state) |
| Line editing, history, completion and Cyrillic input from the UART and the VirtIO keyboard (through the PS/2 decoder) | `shell` suite |
| Files on a raw FAT disk through `virtio_blk`: written, checked by fsck.fat, read with mtools and after a reboot; screenshot; reboot stops the services in reverse order; power off | `vfs` suite (reboot and power off through PSCI) |
| The VirtIO network card with MSI-X through the ITS (no legacy line held); DHCP, DNS, TCP, flow grants and revocation, two cards on two networks, driver and stack restarts after device quiesce; an e1000 is not taken | `net` suite (no legacy-only card: the driver is built without port I/O) |
| TLS 1.3 with verified server certificates, the device certificate signed by the key service; without RNDR (Cortex-A72) no key and no connection | `tls` suite |
| Every CPU of the MADT starts through PSCI and runs tasks; non-yielding loops are preempted on every CPU with their registers intact; init restarts a service while all CPUs are saturated; remote kill; CPU budgets per period | `smp` and `busy` suites (`tests/busy_app.rs`, aarch64 variant) |
| The board's layout comes from ACPI (MADT, SPCR, GTDT); RAM, the ACPI tables, the ECAM and 64-bit windows above 4 GiB are used (6.3 GB of frames with 6 GiB) | `normal` and `net` suites on `virt,highmem=on` with `--memory 6G` (CI group "RAM, ACPI and PCI above 4 GiB") |
| A GICv2 with a GICv2m frame: CPU interface in memory, SGIs through the distributor, MSI-X as GICv2m SPIs | `normal`, `smp` and `net` suites on `virt,gic-version=2` (CI group "GICv2 with GICv2m") |
| The boot disk on NVMe: the firmware boots from it, `nvme` serves it, files written pass fsck.fat, reboot and power off | `vfs` suite with `--disk nvme` (CI group "NVMe boot disk"; on x86 too) |
| Linux key codes become the PS/2 set 1 codes the decoder expects | Host test `keys_become_set_1_scan_codes` (`tests/virtio_input_host.rs`) |
| USB input without VirtIO input: a keyboard behind a hub (keys, layouts, host-side repeat), a tablet's click, a keyboard unplugged and plugged in on another port, `usb_hid` and `usb_host` killed and restarted | `usb` suite (CI group "programs, shell and four CPUs"; on x86 too, without a PS/2 controller) |
| Pin controllers in definition blocks: QEMU's PL061 device with a static window, the Raspberry Pi 4 firmware's GPIO device whose `_CRS` is a method, a QWord window, a window not taken from the next device, unknown IDs and truncated or random blocks without a panic | Host tests in `tests/aml_host.rs` (blocks built as the ASL compiler encodes them; not the real firmware's tables) |
| Pin controller logic: BCM2711 function select of every pin, set/clear, levels, pulls; PL061 directions and masked data; who may change a pin; this repository's hwdocs tables parse and malformed ones are refused | Host tests in `tests/gpio_host.rs` (register models; no hardware) |
| On QEMU `virt` the kernel reports no pin controller and `gpio` does not start | every aarch64 boot; `normal` suite (`MIND CORE KERNEL: PINS: NO PIN CONTROLLER IN THE ACPI TABLES`) |
| USB boot keyboard reports become PS/2 set 1 codes; report descriptors of a tablet and a mouse give their fields | Host tests in `tests/hid_host.rs` |
| The block store at boot: an object gets the independent reference's root and reads back; a file round trip; names by compare-and-swap, only complete roots; a full medium refused; the store found again after a restart; room freed by a collection once leases end; a name's history of three versions, a quota refusal for a publication and a pin, a pin, a removal found again after a restart; commits of two names, all or none, found whole after a restart (300-STO-0003, 303, 304) | `store` suite on `virt` (CI group "files, network and TLS") |
| Damage on the block store's medium, injected from the host into its bytes in guest RAM (QMP `pmemsave`, gdbstub writes at the RAM's base 0x4000_0000): a corrupt chunk refused when read and when mounting, a collection refused meanwhile, a put repairs it; a damaged header loses its record alone; a damaged name record counted and the version before it current; a damaged commit changes no name (300-STO-0005, 304) | `storefaults` suite on `virt` (CI group "files, network and TLS") |

## Not covered by any test

- The FADT's SMC conduit (QEMU uses HVC), a MADT with more than 16 CPUs (16 are tested, issue 171), addresses beyond the first TiB, real hardware, QEMU `sbsa-ref`.
- A board without a PL031 (its `rtc` would not start) or whose SPCR names no PL011.
- QEMU under Apple's hypervisor (HVF) with `-cpu host` on an Apple Silicon Mac, and macOS as the build host: nothing has run there (issue [600](../../../issues/600-apple-silicon-mac-vm-host.md), [docs/apple-silicon.md](../../apple-silicon.md)).
