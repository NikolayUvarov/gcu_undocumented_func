# LNK landing request: each machine's settings kept by `build_and_write.sh`, onto any new disk (801-LNK-0046)

**From:** LNK · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-14 19:02 UTC · **Concerns:** 801-LNK-0046

**Ready:** `claude/801-LNK-0046-settings-per-machine` 75b8526bb71c7d350e252cee9cd88d4259330db8

## What it carries

`main` 32272bba66 merged in (68636035), with no conflict, then round 136's `main` 40c761e670 (75b8526b). The only conflict, in the second, was `issues/README.md`, resolved keeping both sides (this branch's 801-LNK-0046 row, main's new 801-LNK-0047 row). In `fast-test` already, but for the merge. Only the writer's host side: no image, service or kernel file changes.
- **`scripts/build_and_write.sh`:**
  - **Kept per machine** in `~/.config/mind/machines/<id>/` (700, files 600), `<id>` the agent's ID, else `host-<name>`. The old disk is read once, and each `--keep-*` part goes into its machine's directory.
  - **After the write,** the settings of the machine named by `--machine <id>`, else by the old disk just read, are put on the disk. With neither, the script stops before the writer opens. The one store of before (`~/.config/mind/settings` and its kin) is left untouched and unused.
  - **`--list-machines`** names the kept machines with no secret; **`--old-disk <n>`** reads the old disk first, then waits until it is out and another device is disk N (the MBR signature stays one, the maintainer's decision).
  - **One device key on one machine:** a write whose kept key another kept machine holds is stopped, both named by the key's public label. `--new-device-key` sets the kept key aside, never removed. `--keep-wifi` keeps `device.sealed` and `device.locked` too.
  - **The pull comes before the options** (085913eb), so a copy never refuses an option of a newer one. An unknown option is named.
  - **The steps after the write** pick the volume MIND CORE that holds this build's MANIFEST.
  - **d46f46be:** a policy kept only at the old disk's root is kept, and the policy and the Wi-Fi join go on the disk even with no key kept (the bench's two faults of 15:40 UTC).
- **`05_write_usb_windows.ps1`:** an offline USB target (`OfflineReason Collision`) is brought online when written as administrator.
- **Tests:** `tests/build_and_write_machines_test.sh` (new, in `scripts/host_tests.sh`) and `tests/build_and_write_args_test.py`.
- **Text:** `docs/write-disk.md` and `docs/write-disk_RU.md`, the issue, its `issues/README.md` row.

**Issues:** advances 801-LNK-0046. Both of its criteria are met: the tests, and the MacBook Pro's bench run (below). It stays open until this is in `main`. Closes none.

## What was tested, and where

LEG9, WSL2, Ubuntu 24.04, on 75b8526b's exact tree:
- **Builds:** `./02_build.sh` (x86) and `scripts/x86_fixtures.sh`.
- **The full `scripts/host_tests.sh`** with `MIND_SSHD`: exit 0, 230 Rust suites, 6246 passed, none failed, and the scripts' tests, among them:
  - `tests/build_and_write_args_test.py`: 13 tests, OK;
  - `tests/build_and_write_keep_test.sh`: all passed;
  - `tests/build_and_write_machines_test.sh`: 63 checks, all passed.
- **No QEMU suite:** nothing here goes into an image.

**Recorded in the issue (the maintainer's PC, Windows, and the bench):**
- **13:45–13:50 UTC, the MacBook Pro:** its old disk read with `--old-disk 5 2`, then a new stick written with `--machine mind_353abc18de2dbd0e 2`. The Mac booted the stick by itself. Its log: `DEVICE KEY READY: MIND 5097010F` (the same key), `AUTO-JOIN CoolNet2: JOINED (TRY 1)`, `MODE: MAINTENANCE; REMOTE UPDATE`, and the agent the same endpoint, with a `collect_logs` from hesere completed.
- **15:40–16:00 UTC, the DEXP and the Ryzen,** written with `--new-device-key`: both joined their Wi-Fi, but their agents had no policy line. d46f46be fixes that in the script. The machines were mended by hand, so the fix itself has run only in the host tests.

**Not tested** (MC-12.3): d46f46be on a real disk; the PowerShell paths other than the bench's run (only PowerShell's parser and a stand-in object); `--old-disk`'s wait on any disk but the Mac's.
