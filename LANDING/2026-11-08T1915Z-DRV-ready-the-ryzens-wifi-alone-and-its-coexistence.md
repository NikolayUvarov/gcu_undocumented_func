# DRV → LANDING: ready, the Ryzen's Wi-Fi carries its traffic alone (213-DRV-0032), its coexistence after the join (213-DRV-0052), and boot_switch.ps1's logs (800-LNK-0037)

**Ready:** `claude/ASR-DRV` 0e9016d8

**Replaces:** `LANDING/2026-11-08T1642Z-DRV-ready-the-ryzens-rtl8852be-joins-coolnet2.md`, unanswered. Its commit 97711bdd is in this branch.

## What it lands

Everything of the replaced request, and:

- **213-DRV-0032** (advanced; the section "The Wi-Fi alone, the cable out"):
  - **f804245a, the debug modes** that let the Wi-Fi carry the machine's own traffic with the cable out:
    - `OUTLAST_SER`'s wait has its own time;
    - `DROP_UNACKED`;
    - `REPORT_EACH`, one release report a frame;
    - `SEND_TRIES`;
    - `RESTART_AFTER_FAILURE`: after a clean stop the driver exits and `init` restarts it.
    - Also: the stop line no longer reads the registers after the data mode's own shutdown.
  - **4897f691:** `OWN_DEADLINES`.
  - **1b4e3942:** `KEEP_LATE`. A send that outruns its time is answered as not acknowledged, and its release is awaited 1 s more.
  - Each is a constant, the maintainer's "everything runs first, hardening later", with the strict path beside it.
- **213-DRV-0052** (advanced, plan steps 2 and 3's idle case). **fb10b69d:** the data mode's first coexistence reconcile tells the firmware of the connected station, as Linux does at `BTC_ROLE_MSTS_STA_CONN_END`. It sends ROLE v2, the policy `OFF_BWB1`, AFH around the channel, and path B's S1 standby (pinned Linux `05f7e89a`).
- **800-LNK-0037** (advanced):
  - **dba9edb9:** `boot_switch.ps1 logs` failed on the DEXP. The machine's blocks called a variable gone after `New-WindowsSystem` returned; it is a function now, and the test checks for it.
  - **8929e978:** the DEXP's run recorded. `once` refused while the stick is first in the DEXP's `BootOrder`, as it should.

**Directories:**
- `rtw89_wifi/`, `tests/rtw89_*`, `hwdocs/rtl8852be-*.md`;
- `scripts/boot_switch.ps1`, `tests/boot_switch_ps1_test.ps1`;
- `issues/`.

## What was tested, where

- **Host tests on LEG9 (WSL).**
  - `main` 9f54dfca was merged in (0e9016d8).
  - `./02_build.sh` passed, then the whole `scripts/host_tests.sh` with `MIND_SSHD` set: 189 suites, none failed.
  - Every `tests/rtw89_*_host.rs` suite also ran on its own after each change; the service suite has 331 cases.
- **The PowerShell test,** 40 checks, in Windows PowerShell 5.1 on LEG9. It fails on the script before dba9edb9. On the DEXP's Windows 11, through Effector as SYSTEM: `status`, the refusal of `once`, and `logs` after the fix.
- **Hardware.** The Ryzen, `fast-test` 44a1a4f1's image with `rtw89_wifi` replaced over the link, the Ethernet cable out:
  - Effector's TLS to hesere.uz and pings to the gateway and 8.8.8.8 went over the Wi-Fi alone.
  - With 0052's build and `bt_hci`'s rounds every 300 s:
    - whole 10-second inquiries passed with the station joined;
    - the build of 1b4e3942 went through three inquiries and three late frames in its first 12 minutes without a stop.
  - One machine and one AP (MC-12.1).
- **Not run:**
  - **The QEMU suites.** The changes are in `rtw89_wifi`, a driver for a device QEMU does not model, and in a Windows script.
  - **The aarch64 groups,** for the same reason.

## Messages sent with it

On `internal-dev-messages`:
- `NET/2026-11-08T1805Z-DRV-…`: the card that traffic takes, chosen in software.
- `NET/2026-11-08T1840Z-DRV-…`: `wlan` learns a restarted radio again.
- `KRN/2026-11-08T1800Z-DRV-…`: the DEXP's camera waits for isochronous transfers on xHCI.
