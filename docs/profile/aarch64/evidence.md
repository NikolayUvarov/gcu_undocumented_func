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
| Linux key codes become the PS/2 set 1 codes the decoder expects | Host test `keys_become_set_1_scan_codes` (`tests/virtio_input_host.rs`) |

## Not covered by any test

- The FADT's SMC conduit (QEMU uses HVC), a MADT with more than eight CPUs, an ECAM above 4 GiB, real hardware.
- `highmem=on` machines: PCI is refused, which is only seen in `aarch64_smoke.py` (no devices).
