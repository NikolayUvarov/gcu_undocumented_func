# DRV → LANDING: ready, the Ryzen's RTL8852BE joins CoolNet2 (213-DRV-0032), and boot_switch.ps1 (800-LNK-0037)

**Ready:** `claude/ASR-DRV` 97711bdd

**Replaces:** `LANDING/2026-11-06T0638Z-DRV-ready-a-register-poll-reads-once-after-its-window.md`, unanswered. Its commit 4b577dea is in this branch.

## What it lands

- **213-DRV-0032** (advanced; the progress section of 2026-11-06 and 2026-11-08):
  - **c9255671:** a MAC another instance left on is powered off first.
  - **1ba92c22:** the receive-only service admits a channel under the station's country.
  - **274654e5:** a scan's probe request admits nothing; data mode keeps its channel under a new country.
  - **f47ba6f8:** the debug mode `KEEP_SENDING`.
  - **c696adf2:**
    - Linux's station RX filter for a network's filter, so the busy channel no longer fills the packet buffer and triggers the firmware's L0 recoveries;
    - 32 receive passes a step;
    - the debug mode `OUTLAST_SER`;
    - registers at admission and at a stop;
    - `tx::Seen`.
  - **8e3913fd:**
    - the debug mode `RPQ_ALONE`: a data frame is final on DMA and the RPQ, as Linux's PCIe path;
    - path B's TSSI DAC gain table written unread.

  **The result on the Ryzen:** JOINED and DHCP 192.168.2.169/24 over the RTL8852BE.
- **800-LNK-0037** (advanced): `scripts/boot_switch.ps1` and `tests/boot_switch_ps1_test.ps1` (4138fab1), and a fix for a machine without an MBR disk (a9e6f238). The run on the DEXP under Windows is left.
- **Opened:**
  - [177-DRV-0051](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/claude/ASR-DRV/issues/177-DRV-0051-the-dexps-intel-display-engine.md), the DEXP's Intel display engine;
  - [213-DRV-0052](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/claude/ASR-DRV/issues/213-DRV-0052-coexistence-after-association.md), the coexistence after association.
- **Debug modes, on by the maintainer's word** ("everything runs first, hardening later"): `KEEP_SENDING` (`service.rs`), `OUTLAST_SER` (`data_mode.rs`) and `RPQ_ALONE` (`tx.rs`). Each is a constant, with the strict path beside it and covered by its host cases.

**Directories:**
- `rtw89_wifi/`, `tests/rtw89_*`, `hwdocs/rtl8852be-*.md`;
- `scripts/boot_switch.ps1`, `tests/boot_switch_ps1_test.ps1`;
- `docs/write-disk*.md`, `README.md`, `issues/`.

## What was tested, where

- **Host tests on LEG9 (WSL).** `main` da373d83 was merged in (475659f0). Then `./02_build.sh` and the whole `scripts/host_tests.sh`, with `MIND_SSHD` set: 189 suites, none failed. After that, only text commits (62a725f4, 97711bdd).
- **The PowerShell test** (39 checks), run by hand in Windows PowerShell 5.1 on LEG9, as `tests/usb_writer_windows.ps1` is. `status` also ran on the DEXP's Windows 11, as SYSTEM, through Effector.
- **Hardware:**
  - **The setup:** the Ryzen's image is `fast-test` 44a1a4f1, and `rtw89_wifi` was replaced over the link.
  - **The result:** joined CoolNet2 on channel 8 (WPA2) and got DHCP. With `bt_hci`'s inquiries paused, it held 12 minutes. With them, the network was lost at an inquiry: that is 213-DRV-0052.
  - **The scope:** that machine and those builds only (MC-12.1).
- **Not run here:** the QEMU groups. The code change is in `rtw89_wifi`, which has no device in QEMU, plus a PowerShell script.
