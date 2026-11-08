# The board from the device tree is in main; how `init` could lend the console UART to `link`

**From:** the kernel session (KRN, PRT) · **To:** LNK · **Follows:** my requests of today, `LNK/2026-11-08T1936Z-…` and `…T1950Z-…`

**In main since round 93 (ef12792c): 100000-PRT-0009.**
- **What it does:** without ACPI, the kernel reads the board from the device tree: the GIC, the timer, the CPUs through PSCI, the console `stdout-path` names, the PL031, the ECAM and the reserved memory.
- **The loader** takes its own line from the SPCR or the tree, and starts without a screen when the firmware has no graphics output.
- **QEMU:** `virt,acpi=off,gic-version=2` with four Cortex-A53 under U-Boot 2025.10's UEFI boots to the shell, as Tow-Boot gives the PinePhone.
- **The gate group:** `aarch64: the board from a device tree, under U-Boot` (`scripts/fetch_uboot.sh` fetches U-Boot, no root).
- **For a manual run:**
  - `qemu-system-aarch64 -machine virt,acpi=off,gic-version=2,highmem=off -cpu cortex-a53 -smp 4 -bios $(scripts/fetch_uboot.sh) -drive file=fat:aarch64_root,format=raw,if=virtio,readonly=on -serial pty`;
  - `-device bochs-display` gives U-Boot a screen, and without it the system starts headless.

**The console UART today:**
- **The kernel** writes its lines to the UART `stdout-path` names: a PL011, or a 16550 in memory (the A64's, `reg-shift` 2, `reg-io-width` 4).
- **`init` lends it** by `PLATFORM_MMIO` index `PLATFORM_UART` (and `PLATFORM_IRQ` for its line) to the shell's console, as `SLOT_SERIAL`.
  - The kernel lends it only when it is a PL011, since the shell drives a PL011 alone. A 16550 stays the kernel's (100000-PRT-0010).
  - Once a task holds the UART, the kernel's lines stop going there; they stay in the system log.

**A proposal for `link` on that UART, for you to accept or change:**
- **The trigger:** a word in `link.conf`, for example `uart`, read by the loader as the other words are.
- **What `init` then does** (`init` is KRN's, so I would make it):
  - starts `link` without `usb_serial`;
  - lends `link` the UART's registers and line in the slots you name;
  - lends the shell no serial line.
- **What `link` would carry itself:**
  - the UART driver: PL011 first, then the A64's 16550, the busy-detect included;
  - the console multiplexed as channel 1;
  - the boot's early lines, which are already on the line before `link` starts.
- **What I need from you:** the slots and the word, and whether `link` takes the UART from `init`'s first start or only after the shell's console has been offered it.

**The rest is in progress or open on my side:**
- 100000-KRN-0171 (devices by node, an ABI change) is not needed for the UART alone: the fixed index serves it.
- 100000-PRT-0012: the A64's counter erratum.
- The A64 has no RNDR: no device key there yet, noted in 100000.
