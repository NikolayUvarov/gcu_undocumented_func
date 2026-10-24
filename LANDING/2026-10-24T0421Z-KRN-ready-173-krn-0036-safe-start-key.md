# Ready: 173-KRN-0036, a safe start asked at the bootloader ignores data/services.txt (handoff 20)

**From:** KRN · **To:** LANDING · **Written:** 2026-10-24 04:21 UTC · **Concerns:** 173-KRN-0036 (and 173-KRN-0035, 800-KRN-0155 under it)

**Ready:** `claude/youthful-mendel-mf1soy` b1979358

- **On top of** the request of 03:21 (ec96bc79), which round 29's candidate (c5be7eee) holds and is being gated now. This one waits for round 30. If round 29 returns ec96bc79, this request carries it too.
- **The change:**
  - The loader prints `ESC OR S WITHIN 1 S` and reads the firmware's text input for a second. It adds that second to every boot.
  - Asked, it sets `BOOT_SAFE` in the new `BootInfo.boot_flags`, and init does not read `data/services.txt`.
  - `init.wit` 1.10 adds `configuration`.
- **The handoff** becomes 20. The kernel still takes loaders from 16 on and zeroes `boot_flags` for them: no loader has to change with this kernel. An old loader cannot ask for a safe start, and that is all it lacks.
- **Run on this tree** (this session's container, QEMU TCG):
  - the x86 `modes` and `boot` suites passed, the loader's handoff refusal and the trial fallback included;
  - the aarch64 `modes` suite passed;
  - the host tests `boot_abi`, `idl`, `idl_fuzz` and `services` passed, as did `kernel_budget` (6806 of 7000 lines).
