# KRN ready: round 98's return fixed, and a request bit for a console program's secret line

**Ready:** `claude/youthful-mendel-mf1soy` 8d30012b

**From:** the kernel session (KRN, PRT)

**Replaces** `2026-11-09T0742Z-KRN-ready-platform-nodes-the-aps-caches-and-the-kernels-budget.md` (269410ee), returned by round 98 (`2026-11-09T0851Z-KRN-round-764a7a13.md`): `host tests` did not build.

**What changed since:**
- **8d30012b, 000-KRN-0220:** the BSP's `CR0` is read at the start of x86's `cpu::prepare` instead of in `mmu::init`. `tests/runtime.rs` compiles `mmu.rs` without the kernel's `cpu` module, so the old place broke it.
- **f91333db, 000-KRN-0224** (APP's request, for 000-APP-0126):
  - `mind::process::REQUEST_SECRET_LINE` (`1 << 29`) and `BADGE_SHELL_SECRET` (2);
  - `init` gives the shell `SLOT_COMMANDS` with `ALL | CAP_BADGE`.
  - No ABI change.
- **c31daf6e:** `main` ddc4a7cb merged (text: 000-KRN-0219 closed).

**Advances:** as in the request it replaces (100000-KRN-0171, 207, 211-KRN-0223, 000-KRN-0220, the kernel's budget at 7400), and 000-KRN-0224.

**Tested** in the kernel session's container (QEMU 8.2.2, TCG):
- **`scripts/host_tests.sh` on 8d30012b:**
  - every Rust test passed, `runtime` and `kernel_budget` among them;
  - two Python tests failed the same way on `main` in this container: `manifest_test.py` (its `cryptography` package) and `effector_server_test.py`'s TLS (`SSLEOFError`). The gate's machines pass both.
- **000-KRN-0224:** `x86: keys, shell, tools` and `aarch64: programs, shell and four CPUs` passed.
- **000-KRN-0220:**
  - as in the request it replaces;
  - round 98's gate passed every x86 group with it (`x86: boot, display, shell, memory, clock` on acc-2 among them).
  - That group failed once in this container, in `bar_move_check`'s MSI storm wait, while the host tests ran beside it on the same 4 CPUs. It is being run again here alone.

The full gate is round 99's.
