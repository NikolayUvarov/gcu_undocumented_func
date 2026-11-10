# KRN ready: the store's client that may recover the volume key, the shell's alone (308-KRN-0204, ABI 30, for STO)

**Ready:** `claude/308-KRN-0204-the-recovery-client` 03425985c

**From:** the kernel session (KRN, PRT, NET, UPD)

**What it lands** (on `main` 4739c16a), ABI 30:
- **`common/abi.rs`:**
  - `SLOT_RECOVER` 43, a launch slot; `SLOT_DYNAMIC` 44; `ABI_VERSION` 30;
  - `CAP_SLOTS` 108, so that 64 dynamic slots stay. README and `docs/profile/kernel-objects.md` said 104, out of date since 107.
- **`libmind`:** `REQUEST_RECOVER` (1 << 30).
- **`init`:** while the store runs, the shell gets the store's client badged `BADGE_RECOVER` alone in `SLOT_RECOVER`.
- **Tests:**
  - `tests/runtime.rs`: the slots, the ABI, the launch slots;
  - `tests/boot_abi_host.rs`: handoff 30 left;
  - the `services` suite's `blockstore_check`: the shell holds slot 43 with badge 8 of `blockstore`'s endpoint, and `updater` and `storefs` hold no client with that badge.
- **Docs:** `docs/api` (ABI 30) and `docs/profile/bootstrap.md`.

**Every program is rebuilt for ABI 30.** libmind refuses a program built for 29.

**Tested** in the kernel session's container (QEMU 8.2.2, 4 CPUs):
- **x86:**
  - "build (x86)" and "build (x86 test programs)": passed;
  - "x86: services, storage, audio": passed, 742 s, 18 checks, the new one among them.
- **aarch64:**
  - "build (aarch64)": passed;
  - "aarch64: boot and fault containment": passed;
  - "aarch64: files and the block store": passed, 807 s, 40 checks.
- **Host tests:** `runtime` (46) and `boot_abi` (9) passed.

**Left to APP (308-APP-0122):** the shell's lending of a copy for `REQUEST_RECOVER`, with the user asked each time.
