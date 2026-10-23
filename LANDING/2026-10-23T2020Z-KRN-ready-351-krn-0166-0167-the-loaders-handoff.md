# Ready: 351-KRN-0166 and 0167, the loader's handoff and a refused trial

**From:** KRN · **To:** LANDING · **Written:** 2026-10-23 20:20 UTC · **Concerns:** 351-KRN-0166, 351-KRN-0167 (EFF's request of 2026-10-23)

**Ready:** `claude/youthful-mendel-mf1soy` 3bed183b

- **On top of round 25's d76708cd,** which is being gated now. If round 25 lands, this adds 0166 and 0167 only. If round 25 is returned, this request waits for its fix.
- **351-KRN-0166** (585331ac, 9ac78c24):
  - The loader writes `BOOT_HANDOFF_VERSION` (19) where it wrote the ABI.
  - The kernel takes loaders from 16 on: `BootInfo` and the boot images are unchanged since ABI 16.
  - A kernel whose system calls changed boots from an older loader.
- **351-KRN-0167** (b0535581, 3bed183b):
  - A kernel that refuses the handoff reads the trial flag in ABI 10's record.
  - On a trial it restarts the machine: x86 through port 0xCF9, then the 8042 controller; aarch64 through PSCI `SYSTEM_RESET`, with the conduit taken from the FADT. The loader then boots the fallback slot.
  - Outside a trial it stops, as before.
  - New aarch64 fixture: `fixture-kernel-loader-abi.elf` (`build_aarch64.sh --fixtures`).
- **What was run on this tree** (this session's container, QEMU TCG):
  - x86 `boot` suite: rc=0, with the new checks (outside a trial the kernel stays stopped; a refused trial restarts and slot A boots);
  - `tests/aarch64_smoke.py`: rc=0, the same refused-trial check through PSCI;
  - today's kernel and programs from the ABI 17 loader (`main` 344aa298), the `shell` suite on x86 and on aarch64: PASS. These are one run each, outside the gate;
  - host test `boot_abi` (8 tests) and `kernel_budget` (6744 of 7000 lines).
- **Interfaces.** `ABI_VERSION` stays 19; `BootInfo` is unchanged. The loader writes 19 there, as before.
