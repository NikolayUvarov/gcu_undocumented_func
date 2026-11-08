# 800-LNK-0037 — One boot of MIND Core from Windows, and back: `boot_switch.ps1`

**Type:** tools · **Owner:** `LNK` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-12.1

## Problem

`scripts/boot_switch.py` ([000-KRN-0124](000-KRN-0124-switching-between-linux-and-mind-core.md)) starts MIND Core once from Linux, and reads that boot's logs back. The maintainer's DEXP N100 runs Windows 11 from its own disk, and MIND Core from a stick. That machine is tested separately, and it needs the same switch from Windows (the maintainer, 2026-11-08).

## Plan

- **`scripts/boot_switch.ps1`** (Windows PowerShell 5.1 and later) with the same three commands as `boot_switch.py`, and the same refusals, messages and state:
  - `status`;
  - `once [-Disk N] [-Reboot]`;
  - `logs DIR [-Disk N]`.
- **Through the firmware's variables themselves** (`Get/SetFirmwareEnvironmentVariableEx`, `SeSystemEnvironmentPrivilege`), not `bcdedit`. Then the stick's entry is found by `HD(1,MBR,signature,start,size)` from its MBR, as `efibootmgr`'s path is in Python. A missing entry is made as `efibootmgr --create-only` makes it.
- **`BootOrder` is never written.**
- **Windows' writes to the stick's FAT** are said, and `logs` copies around them.
- **`tests/boot_switch_ps1_test.ps1`** checks the logic against a modelled firmware and disks, case for case with `tests/boot_switch_test.py`.

## Acceptance criteria

- [x] The script and its test. The test passes in Windows PowerShell 5.1 on LEG9: 37 checks.
- [x] Without an administrator, the script refuses and writes nothing (`status` on LEG9).
- [ ] `status`, `once -Reboot` and `logs` on the DEXP N100 under Windows 11, as administrator: MIND Core boots once, and the next boot is Windows.
- [x] The test runs as `tests/usb_writer_windows.ps1` does, by hand on Windows (README); it is not in `scripts/host_tests.sh`, as not every machine that runs that suite has PowerShell.

## Related

[000-KRN-0124](000-KRN-0124-switching-between-linux-and-mind-core.md) (`boot_switch.py`), [000-KRN-0211](000-KRN-0211-the-loaders-mark-of-how-far-a-boot-got.md) (the loader's mark), [docs/write-disk.md](../docs/write-disk.md) section 11.
