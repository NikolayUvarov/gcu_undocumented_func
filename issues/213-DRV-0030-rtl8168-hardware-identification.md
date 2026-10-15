# 213-DRV-0030 — Identify the Ryzen's RTL8168 MAC and PHY before implementing its driver

**Type:** hardware evidence · **Owner:** ASTRA auditor-fixer, for `DRV` · **Priority:** P1 · **Status:** in progress (collector prepared; hardware evidence pending) · **Blocked by:** a new Ryzen hardware observation (MIND report or Linux collector) · **Main task:** [213](213-amd-ryzen-pc.md), [550](550-network-on-real-hardware.md) · **Roadmap:** tracks H, A · **Constitution:** MC-12.1, MC-12.3

## Problem

The maintainer's `hw0002.txt`–`hw0005.txt` from build e6730d7a identify PCI function `02:00.0`, vendor/device `10ec:8168`, revision `0c`, subsystem `10ec:0123`. The reported BAR2 is MMIO `0xFCE00000`, size `0x1000`; BAR0 is I/O `0xF000`, size `0x100`; BAR4 holds an MSI-X table. MSI is present; the legacy IRQ line is `255`. MIND has no driver for this NIC. These addresses describe that firmware run, not fixed addresses a driver may assume.

PCI revision alone does not establish the MAC/PHY initialization sequence. Linux's r8169 driver identifies the MAC from the XID in TxConfig, then selects variant-specific handling. The supplied PCI inventory contains no TxConfig dump. A guessed generic RTL8169 reset/PHY sequence is not adequate evidence for this RTL8168.

## Plan

1. Prepare `scripts/hardware/rtl8168_probe.py`, a Linux collector which verifies `10ec:8168`, records sysfs PCI identity/BARs/driver/interface/link state, and requests the bound driver's read-only `ethtool` information and register dump. It never changes link settings, resets the NIC, binds a driver, maps BARs or writes device registers.
2. Decode TxConfig at offset `0x40` only from a complete, 256-byte raw dump belonging to Linux `r8169`, on the supported little-endian host. Compute `XID = (TxConfig >> 20) & 0xfcf`. Reject all-ones, an unknown dump layout, truncation or driver identity mismatch instead of guessing. Keep the raw dump and its hash for review.
3. On a Linux boot of this Ryzen, run locally or through Effector:

   ```bash
   python3 scripts/hardware/rtl8168_probe.py --pci 0000:02:00.0 --output /tmp/rtl8168-ryzen
   ```

   If register access is denied, run the same collector with the machine's authorized administrative privileges and a new output directory. The tool reports missing dependencies and permission failures; it does not escalate itself. BDF may differ under Linux: omit `--pci` to select the sole matching device.
4. Review the resulting identity, XID, PHY/link evidence, driver/firmware versions and any missing observations. Preserve machine/build provenance. A link that is down is an observation, not permission to change it.
5. Choose and document the supported silicon variant in [213-DRV-0031](213-DRV-0031-rtl8168-wired-network.md), with sources and any firmware requirements. Register dumps remain private run artifacts, not committed fixtures.

Alternatively, a new MIND boot containing [213-KRN-0113](213-KRN-0113-realtek-boot-identification.md) records a guarded direct TxConfig/XID observation in `hwNNNN.txt`. This supplies MAC identity without Linux; it does not supply the Linux collector's PHY, firmware and link evidence. Keep the missing observations explicit and select the variant from the actual value, not PCI revision alone.

## Acceptance criteria

- The collector refuses other PCI vendor/device IDs before invoking hardware tools.
- Its parser rejects invalid/truncated register dumps, validates driver identity and byte order, and has regression tests with independent known register/XID pairs.
- A report from the actual Ryzen supplies an unambiguous MAC XID and PCI identity, or explicitly names the observation still missing. Merely preparing or host-testing the collector does not close this task.
- The driver task names the observed variant and its required initialization/firmware without presenting a proposed sequence as tested support.

## Related

[213-DRV-0031](213-DRV-0031-rtl8168-wired-network.md), [requests-DRV.md](requests-DRV.md), [550](550-network-on-real-hardware.md). Primary reference: [Linux v6.12 r8169 register layout, register dump and MAC identification](https://raw.githubusercontent.com/torvalds/linux/v6.12/drivers/net/ethernet/realtek/r8169_main.c). No Linux driver implementation is copied into the collector.

## Progress (2026-10-15)

The collector and seven tests in `tests/rtl8168_probe_test.py` are prepared and pass. Tests exercise known TxConfig/XID byte sequences, offset/endianness, malformed dumps, wrong device refusal before querying hardware, ambiguity, existing-output protection, private artifact permissions, missing ethtool and permission failure reporting. The collection path changes no fixture sysfs files and issues only the three documented information queries. This is host evidence for the collector, not a run on the Ryzen; the hardware identification criterion remains open.
