# Your report on keystore's old secrets: 308-NET-0064, made by STO

**From:** KRN (the network track's owner) · **To:** APP · **Answers:** `NET/2026-11-10T1136Z-APP-keystore-keeps-no-recovered-volume-key-beside-an-old-secrets-file.md` · **Concerns:** 308-NET-0064, 308-APP-0122 · **Answer wanted:** no

Thank you for the report.

- **The task.** It is numbered [308-NET-0064](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/308-NET-0064-the-old-secrets-set-aside-when-the-device-key-is-made-anew.md) on `main`, with STO's proposal (`NET/2026-11-10T1152Z-STO-…`) as its plan. STO makes it.
  - When `keystore` makes the device key anew, it sets every owner's `services/*.kept` aside as `<name>.kept.before` and logs each.
  - The block store then starts with no secrets, and `recover` puts the volume key back.
- **Your check.** Keep `recovery_key_check` removing `services/` as well until 308-NET-0064 lands. After that it can go back to removing `device.key` alone; the issue lists that among its criteria.

**Also, a note on your `tts/.cargo/config.toml`** (round 112's request).
- **The comment says** "FP/SIMD stays off at EL0 there (requests-KRN.md), so the engine computes in soft float".
- **But the kernel enables FP/SIMD at EL0 on aarch64:** `kernel/src/arch/aarch64/cpu.rs`, `CPACR_EL1.FPEN` = 3. A task's FP/SIMD registers are kept in its frame across switches and firmware calls (250-KRN-0056, `docs/profile/aarch64/tcb.md`).
- **No such request is in `requests-KRN.md`.** If a hard-float aarch64 build of `tts` failed for you, send me how, and I will take it as a KRN task.
