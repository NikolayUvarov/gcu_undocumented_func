# 351-KRN-0195 — The firmware's own network boot stays off: named in the threat model, refused by `efivar`

**Type:** docs, `efivar` · **Owner:** `KRN` · **Priority:** P3 · **Status:** open (made; the aarch64 run and the machines' firmware left) · **Blocked by:** — · **Main task:** [351](351-self-update.md) · **Roadmap:** track C · **Constitution:** MC-9.1, MC-12.3

**Asked by:** the maintainer on 2026-10-30, after a public advisory on EDK II's PXE driver (CVE-2026-97631). The priority is low: nothing in MIND Core boots over the network, and our machines can turn it off in their firmware today.

## Problem

- **What runs before our loader.** A UEFI firmware's network boot (PXE, HTTP boot) is the firmware's own code. It runs before our bootloader, outside every guarantee we state.
  - A flaw there runs code before the loader checks its signed manifest (350-UPD-0003), and before Secure Boot with our own keys (351-UPD-0012) is of any use.
  - The device key's seal is not bound to the boot state (`docs/profile/threat-model.md`), so code that ran first may also open the seal.
- **What MIND Core does not do.** It never boots over the network:
  - the bootloader opens only its own device's file system;
  - no service sends a PXE or DHCP boot request;
  - `init` and the updater create no `Boot####` entry.
- **What is not said anywhere.** Neither the profile nor the disk-writing guide says that the firmware's network boot should be off on a machine that boots MIND Core.
- **What `efivar` allows.** `efivar bootorder` and `efivar bootnext` take any existing entry. A network entry the firmware made (QEMU's OVMF makes `UEFI PXEv4` ones for a network card with a boot ROM) can be put first by a typing slip.

## Plan

- **The threat model** (`docs/profile/threat-model.md` and `docs/profile/aarch64/threat-model.md`): the firmware line in "Out of scope" names the firmware's network boot.
  - It runs before the loader, outside our guarantees.
  - It should be off on every machine that boots MIND Core.
  - Binding the device key's seal to the boot state is a task of its own, not this one.
- **The disk-writing guide** (`docs/write-disk.md`, `docs/write-disk_RU.md`): a step to turn off network boot (PXE, the UEFI network stack, HTTP boot) in the firmware's setup.
- **`efivar`:**
  - `efivar boot` marks an entry whose device path holds a network node (MAC, IPv4, IPv6 or URI messaging node) `(network)`;
  - `bootorder` and `bootnext` refuse such an entry and name it, so the order never puts one first by our hand.
- **The classifier** of a device path lives in `libmind/src/firmware.rs`, with host tests.
- **The machines of Effector:** the maintainer turns network boot off in their firmware. That is a task for a person in `issues-human/`, not code.

## Acceptance criteria

- [x] The threat model (both profiles) names the firmware's network boot as out of scope, to be turned off.
- [x] The disk-writing guide, in English and Russian, has the step.
- [x] `efivar boot` marks network entries. `efivar bootorder` and `efivar bootnext` refuse them with a message naming the entry.
- [x] Host tests of the device-path classifier: a disk path, a MAC + IPv4 path, an IPv6 path, a URI path and a malformed path.
- [x] The `efivar` suite on x86 (OVMF with a network card that has a boot ROM) lists a `(network)` entry and sees `bootnext` refuse it.
- [x] A task in `issues-human/`: network boot turned off in the firmware of the maintainer's machines.

## Progress (2026-10-31)

- **The classifier,** `libmind/src/firmware/boot_path.rs` (a child of `firmware.rs`, so the host tests build it alone, as `fs/volumes.rs` is):
  - `path_kind` reads the device path node by node to its end node. A MAC (`0x0B`), IPv4 (`0x0C`), IPv6 (`0x0D`) or URI (`0x18`) messaging node in any instance makes it `Network`. A node shorter than its header, one past the path's end, or no end node makes it `Malformed`;
  - `option_path` and `option_kind` find the path in a `Boot####` variable after its description.
- **`efivar`:**
  - `efivar boot` marks such an entry `(network)`, after `(inactive)` where both apply;
  - `bootnext` and `bootorder` refuse a network entry, and one whose path cannot be read, before writing: `efivar: REFUSED: Boot0002 (UEFI PXEv4 (MAC:525400123456)) boots from the network, before and outside MIND Core's guarantees; turn network boot off in the firmware's setup (351-KRN-0195)`. An entry that is not set is the firmware's to skip, as before.
- **Host tests,** `tests/boot_path_host.rs`, 8: a disk path, MAC + IPv4, MAC alone, IPv6, URI (behind IPv4 and alone), a network instance after a disk instance, malformed paths (empty, no end node, the end node cut, a node of length 2, 0 or past the end), and a `Boot####` read past its description or cut short.
- **The `efivar` check** (x86's `boot` suite, aarch64's `efivar` suite) finds the `(network)` entry and sees both commands refuse it, with `BootNext` and `BootOrder` unchanged. On x86 it requires one; on aarch64 it checks one when AAVMF lists one.
  - **Run locally,** x86, QEMU TCG with OVMF: OVMF's `Boot0002 UEFI PXEv4 (MAC:525400123456)` for QEMU's default card was marked and refused; the rest of the check passed as before.
- **Docs:** both threat models; the disk-writing guide's firmware settings, in English and Russian; the x86 evidence row; a task for the maintainer in `issues-human/` (section 13).

**Left:**
- the `efivar` suite on aarch64, to see whether AAVMF lists a network entry;
- the maintainer's machines' firmware (`issues-human/`, section 13).

## Related

- 351-KRN-0027, 351-KRN-0028: `efivar`, the firmware's variables.
- 350-UPD-0003, 351-UPD-0012: the signed manifest; Secure Boot with our own keys.
- 351-NET-0005, 351-NET-0006: the device key and its seal.
- 000-KRN-0124: `scripts/boot_switch.py` sets `BootNext` from Linux to the MIND Core disk's own entry only.
