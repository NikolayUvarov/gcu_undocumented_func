# 213-KRN-0113 — Record guarded Realtek chip identity in the boot hardware report

**Type:** kernel / hardware evidence · **Owner:** ASTRA auditor-fixer (the maintainer's assignment, 2026-10-15) · **Priority:** P1 · **Status:** in progress · **Blocked by:** — · **Roadmap:** H / [213](213-amd-ryzen-pc.md), D / [550](550-network-on-real-hardware.md) · **Constitution:** MC-3.3, MC-12.1, MC-12.3

## Problem

The Ryzen's PCI inventory identifies RTL8852BE `10ec:b852` and RTL8168 `10ec:8168`, but lacks the digital chip cut and MAC XID needed to select reviewed initialization paths. PCI revision is insufficient. The devices have no MIND driver yet. A Linux Wi-Fi debug read can require an indirect transaction; the next MIND boot can instead collect the two documented direct identity registers without powering the radio or enabling DMA.

## Plan

1. Restrict the probe to the exact IDs on x86-64 and to ungranted, non-mastering, accessible functions. Validate ordinary PCI headers and the bounded capability list; require D0 when a power-management capability exists.
2. Validate BAR2's current configuration, memory type, nonzero/aligned base, size and checked range. Accept only an already present, identity-translated, uncached register mapping below 4 GiB; do not create mappings or change cache attributes.
3. If memory decode is off, temporarily set only the PCI Command MEM bit, verify it, and restore the original command on every result path. Write zero to the neighboring Status halfword so write-one-to-clear flags are preserved. Never enable bus mastering, alter BARs/PM/interrupts, or write MMIO.
4. Read one aligned 32-bit register: RTL8852BE BAR2+`0xf0` (SYS_CFG1, cut bits 15:12), RTL8168 BAR2+`0x40` (TxConfig, XID mask `0xfcf` after shift 20). Report raw values and decode; report why a probe is skipped or unavailable. No reset, RF, PHY, EFUSE, OCP or polling.
5. Add host regressions for the production guard/restore flow and actual identity-map walk. Run focused kernel build and the coordinated kernel gate; obtain the new hardware report separately.

## Acceptance criteria

- Only the two specified identity registers can be read, after all ownership, PCI, power, BAR and mapping guards pass.
- Normal and failure paths restore PCI command, preserve status flags, leave bus mastering off and perform no device-register writes. A device refusing command restoration is reported explicitly, with no successful identity claim.
- Unsupported architecture, malformed/cyclic configuration, inaccessible values and unsuitable mappings produce explicit reasons without MMIO access or unbounded waits.
- Host regressions and the required kernel gate pass; actual Ryzen values are recorded as a later hardware observation, not inferred from fixtures.

## Related

[213-DRV-0030](213-DRV-0030-rtl8168-hardware-identification.md), [213-DRV-0031](213-DRV-0031-rtl8168-wired-network.md), [213-KRN-0112](213-KRN-0112-firmware-ram-cache-types.md), [hwdocs/rtl8168.md](../hwdocs/rtl8168.md), [hwdocs/rtl8852be.md](../hwdocs/rtl8852be.md).

Primary sources: [Linux v6.12 r8169 TxConfig/XID identification](https://raw.githubusercontent.com/torvalds/linux/v6.12/drivers/net/ethernet/realtek/r8169_main.c), [rtw89 register definitions](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/reg.h), [rtw89 chip-version read before MAC power-on](https://github.com/torvalds/linux/blob/05f7e89ab9731565d8a62e3b5d1ec206485eeb0b/drivers/net/wireless/realtek/rtw89/core.c), and [PCI Command, Status and PM register definitions](https://raw.githubusercontent.com/torvalds/linux/v6.12/include/uapi/linux/pci_regs.h). The existing BSD-attributed `rtw89_wifi` constants and independent `rtl8168` decoder are reused; no upstream driver sequence is copied.

## Progress (2026-10-15)

`kernel/src/realtek_probe.rs` implements the bounded guard/restore flow used by `report.rs`; `pci.rs::command` writes only a 16-bit command value with zero Status bits. The x86 MMU helper validates the actual existing large/split UC identity leaf without remapping anything. The report preserves raw observed registers and prints explicit skip/failure reasons, including a device refusing command restoration. The scheduler lock already serializes this report with grants and PCI configuration changes. Other architectures explicitly skip these probes.

Eight guard-flow host tests and one actual page-table-walk regression pass. `cargo build --offline --release --target-dir /tmp/mind-realtek-kernel-target` in `kernel/` passes for `x86_64-unknown-none`. The full kernel gate is coordinated separately; actual Ryzen register values have not been collected. The task does not initialize either NIC.
