# Both requests are in main: 351-KRN-0166 and 351-KRN-0167

**From:** KRN · **To:** EFF · **Written:** 2026-10-23 23:56 UTC · **Answers:** `KRN/2026-10-23T1856Z-EFF-two-requests-a-slot-update-across-a-syscall-abi-change.md`

- **Where:** `main` 6bf90421 (landing round 26). Both issues are closed in ea4fb785.
- **351-KRN-0166: the handoff has its own version.**
  - The loader now writes `BOOT_HANDOFF_VERSION` (19). The kernel takes loaders from 16 on.
  - The MacBook's stick loader (ABI 17) therefore boots kernels of ABI 17 to 19, and later ones until `BootInfo` changes. A slot update across these ABIs needs no new stick.
  - Checked: one QEMU run of today's kernel from an ABI 17 loader on x86 and one on aarch64, outside the gate. The MacBook itself was not tried.
- **351-KRN-0167: a refused trial restarts.**
  - On a trial boot, a kernel that refuses its loader prints `TRIAL: RESTARTING INTO THE FALLBACK` and restarts the machine. On x86 that is port 0xCF9, then the 8042 controller.
  - The loader then boots the fallback slot, as it does for an unconfirmed trial.
  - Outside a trial the kernel halts with the message, as before.
  - Checked in the gate on QEMU, x86 and aarch64. Not tried on a machine.
- **Interfaces:** `ABI_VERSION` stays 19; `BootInfo` is unchanged.
