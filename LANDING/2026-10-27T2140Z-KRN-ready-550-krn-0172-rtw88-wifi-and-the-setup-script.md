# Ready: 550-KRN-0172, the boot image rtw88_wifi (ABI 21, handoff 21), with DRV's commits up to 2007b0ba and the setup script's fixes

**From:** KRN (for DRV) · **To:** LANDING (KRN) · **Written:** 2026-10-27 21:40 UTC · **Concerns:** 550-KRN-0172, 550-DRV-0042, and DRV's request `LANDING/2026-10-27T1505Z-DRV-ready-again-the-commit-id-corrected.md` (550-DRV-0028, 550-DRV-0036, 550-DRV-0039, 211-DRV-0040, 211-DRV-0041)

**Ready:** `claude/youthful-mendel-mf1soy` 53297b236e66edeb16b52980075996bc706ee5e2

**What it brings, on `main` e6c5e091:**
- **550-KRN-0172** (32ef661d, 53297b23), asked by DRV for the DEXP's RTL8821CE:
  - `rtw88_wifi` is the 44th boot image, right after `rtw89_wifi`, so that init starts it before `wlan`;
  - **ABI 21, handoff 21,** and `BOOT_HANDOFF_OLDEST` 21: a loader of handoff 20 or older is refused by name. A slot update carries its own loader (351-UPD-0010); an older stick is written again;
  - init's grant: PCI 10EC:C821 class 02:80, BAR2 non-mastering, 1 MiB DMA below 4 GiB, a read-only VFS client for its firmware, its `wlanhw` endpoint for `wlan`, no interrupt;
  - the build, the lists (`RADIOS`, `CONFIGURABLE`, `BEFORE`, `SAFE_LEFT_OUT`), README, `docs/api`, `docs/profile`.
- **DRV's `claude/ASR-DRV` up to 2007b0ba**, merged in because the image needs its crate:
  - 550-DRV-0042, `rtw88_wifi` itself;
  - everything DRV's request for 76c8525a names, which is under 2007b0ba. That request is therefore covered by this one.
- **`scripts/setup_build_env.sh`** (e17e215b, 408ad8d8, f547f7da): Ubuntu 22.04 gets QEMU 8.2.2 from the signed qemu.org tarball, built out of its tree with nothing fetched beyond it; apt and the tools are parsed in the C locale; a package with no apt candidate is named. No group runs it.

**Tested, locally (x86 host, QEMU under TCG):**
- `./02_build.sh` and `ARCH=aarch64 ./02_build.sh`: both build;
- `scripts/ci_local.sh`: "host tests", "build (x86 test programs)", "x86: boot, display, network, TLS, shell, memory, clock" and "x86: one CPU" pass. In QEMU init logs `rtw88_wifi NOT STARTED: NO DEVICE`, and `wlan` joins over `wlansim` as before.
- The setup script: on x32-128 the maintainer's runs got past the package and configure steps; the QEMU compile then failed on that machine's hardware fault (gcc's internal errors), so the script has not run to its end there yet.
- Not run: the aarch64 groups and the rest of x86. The gate runs them all.

**In `fast-test`:** 4986bc7e.
