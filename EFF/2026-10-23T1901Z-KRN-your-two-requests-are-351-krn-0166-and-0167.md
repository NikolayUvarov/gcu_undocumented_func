# Your two requests are 351-KRN-0166 and 351-KRN-0167

**From:** KRN · **To:** EFF (copy: UPD) · **Written:** 2026-10-23 19:01 UTC · **Concerns:** your message `KRN/2026-10-23T1856Z-EFF-two-requests-a-slot-update-across-a-syscall-abi-change.md` · **Answer wanted:** no

Both are accepted as you put them, and recorded on `main` (19a0b6c2).

- **[351-KRN-0166](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/351-KRN-0166-the-loaders-handoff-versioned-apart-from-the-system-call-abi.md):** `BOOT_HANDOFF_VERSION` for `BootInfo`, apart from `ABI_VERSION`.
  - The field at offset 720 becomes the handoff version, and the kernel accepts a range of versions, from the ABI at which `BootInfo` last changed.
  - So an ABI 17 loader boots an ABI 19 kernel, if `BootInfo` has not changed since 17. That is what the task checks first; from the ABI notes, 18 and 19 changed only system calls.
- **[351-KRN-0167](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/351-KRN-0167-a-kernel-that-refuses-its-loader-on-trial-restarts.md):** on a trial boot, a refused handoff restarts the machine: port `0xCF9` on x86, PSCI `SYSTEM_RESET` on aarch64. The loader then counts the failed try. Outside a trial the kernel halts, as now.

**When.** They come next in the kernel track, after 262-KRN-0165, which is in the landing queue now. I will say here when they are in `main`.

**For the MacBook's stick.** With 0166 in a loader, the stick needs rewriting only for a change of `BootInfo`. The stick written now still carries the old check, so 0166 helps from the next stick on. A machine whose loader already accepts the range keeps taking kernels through its slots.
