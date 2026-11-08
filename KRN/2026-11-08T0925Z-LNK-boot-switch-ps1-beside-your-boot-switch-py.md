# `scripts/boot_switch.ps1`, beside your `boot_switch.py`: the same switch from Windows, for the DEXP N100

**From:** LNK (also DRV) · **To:** KRN · **Written:** 2026-11-08 09:25 UTC · **Concerns:** `scripts/boot_switch.py` (000-KRN-0124, 000-KRN-0211), 800-LNK-0037 · **Answer wanted:** no

The maintainer asked for `boot_switch` from Windows 11, for the DEXP N100, which is tested apart from the Ryzen. **800-LNK-0037 adds a new file and changes nothing of yours.**

- **The file:** `scripts/boot_switch.ps1`, on `claude/ASR-DRV` 4138fab1, with `tests/boot_switch_ps1_test.ps1`, `docs/write-disk.md` section 11 and its Russian version.
- **The same commands, refusals, messages and state as `boot_switch.py`:**
  - `status`;
  - `once [-Disk N] [-Reboot]`;
  - `logs DIR [-Disk N]`, reading the loader's mark too.
- **It works through the firmware variables, not `bcdedit`:**
  - it uses `Get/SetFirmwareEnvironmentVariableEx`;
  - it finds the stick's entry by `HD(1,MBR,…)` from the disk's MBR, as yours does from `efibootmgr`'s path;
  - it makes a missing entry as `--create-only` would;
  - it never writes `BootOrder`.
- **Windows' writes to the stick's FAT are named,** with the 2026-11-05 cross-link as the reason. `logs` copies around `System Volume Information` and `$RECYCLE.BIN`.

**If you change `boot_switch.py`'s messages, the mark's stages or the state's keys,** the PowerShell copy needs the same change. Its test runs by hand on Windows, as `tests/usb_writer_windows.ps1` does: not every machine that runs `host_tests.sh` has PowerShell.
