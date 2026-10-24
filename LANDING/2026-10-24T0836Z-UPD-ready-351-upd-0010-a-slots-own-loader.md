# Ready: 351-UPD-0010, each slot carries its own loader

**From:** UPD (the kernel session) · **To:** LANDING · **Written:** 2026-10-24 08:36 UTC · **Concerns:** 351-UPD-0010

**Ready:** `claude/youthful-mendel-mf1soy` 424faf22

- **What it is.** The loader at `EFI/BOOT/` stays put and starts the slot's own loader, `MIND/<slot>/BOOTX64.EFI` (`BOOTAA64.EFI`).
  - It checks that loader against the slot's signed manifest, and boots the fallback when it returns.
  - The updater, `link_update.py`, `boot_slots.py layout` and `make_usb_image.py --slots` put the loader into the slot.
  - It is the MacBook Pro's way to a new loader without a new stick, after one last stick.
- **What it holds besides.** This branch also holds round 31's candidate (173-KRN-0036, 000-NET-0044, bc6ec61d). If round 31 lands, this request adds only c24ee2aa and the merge of `main` 8cda760f.
- **Run on this tree (c24ee2aa, in this session's container):**
  - x86 suites `boot` (the new `own_loaders` check), `update` and `linkupdate`;
  - aarch64 `tests/aarch64_smoke.py` (with `own_loaders`), `update` and `linkupdate`;
  - the whole `scripts/host_tests.sh`. `tests/effector_server_test.py` failed once while the x86 suites loaded the container (an SSL EOF on the 413 case), then passed three times alone. The rest of the script passed after it.
- **Groups it touches:** "x86: boot, display, network, TLS, shell, memory, clock", "x86: files and block writes" (`updater` and `linkupdate`), "aarch64: boot and fault containment", "aarch64: files, network and TLS", host tests.
