# DRV → LANDING: ready, the Ryzen's Wi-Fi alone and its coexistence (213-DRV-0032, 213-DRV-0052), boot_switch.ps1's logs (800-LNK-0037)

**Ready:** `claude/ASR-DRV` 82af5eff

**Replaces:** `LANDING/2026-11-08T1915Z-DRV-ready-the-ryzens-wifi-alone-and-its-coexistence.md`, unanswered. Everything in it is in this branch.

## What it lands

Everything of the replaced request (f804245a, 4897f691, fb10b69d, 1b4e3942, dba9edb9, 8929e978), and:

- **c9cbde29:** `KEEP_LATE` gives the queue the late second at once, so only the service's deadlines end a frame.
- **`OUTLAST_SER` past the start of an operation:**
  - **2169b65c:** a recovery found in the coexistence reconcile is outlasted, and the reconcile runs again.
  - **5cba4875:** the data permit takes a scheduler at zero, as a recovery leaves it, as the next operation's to outlast. `live()`'s retry covers the permits too.
  - **ef5a29b2:** the reconcile retries any coexistence error, but only when a recovery is found during it.
- **213-DRV-0052's progress,** run by run, with one correction (c62f39c3).

All of it is debug modes, on by the maintainer's word ("everything runs first, hardening later"), each with its strict path beside it.

**Directories:** as before:
- `rtw89_wifi/`, `tests/rtw89_*`, `hwdocs/rtl8852be-*.md`;
- `scripts/boot_switch.ps1`, `tests/boot_switch_ps1_test.ps1`;
- `issues/`.

## What was tested, where

- **Host tests on LEG9 (WSL).**
  - `main` b04beeb3 was merged in (82af5eff).
  - `./02_build.sh` passed, then the whole `scripts/host_tests.sh` with `MIND_SSHD`: 190 suites, none failed.
  - Every new host case was also run against the code without its change, where it fails.
- **The PowerShell test** as in the replaced request: 40 checks.
- **Hardware.** The Ryzen, the Ethernet cable out, `bt_hci` from its boot image with rounds every 300 s. Each build ran on `fast-test` 44a1a4f1's image with `rtw89_wifi` replaced over the link:

  | Build | Ran | Inquiries passed joined | Late frames survived | What stopped it |
  |---|---|---|---|---|
  | `conn1` | 9 min | 1 | — | a frame's release (before `KEEP_LATE`) |
  | `late1` | 20 min | 4 | 5 | the queue's own budget |
  | `late2` | 25 min | 5 | 15 | the reconcile during a recovery |
  | `late3` | 42 min | 8 | 27 | the permit check during a recovery |

  - **The gateway's pings,** every 2 minutes, lost nothing in most rounds.
  - **`late6`** (ef5a29b2) is running.
  - One machine and one AP (MC-12.1).
- **Not run:** the QEMU and aarch64 suites. The changes are in `rtw89_wifi`, whose device QEMU does not model, and in a Windows script.
