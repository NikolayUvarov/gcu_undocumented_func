# NET landing request: 00, the world rules, chosen and kept (550-NET-0096)

**From:** NET (the kernel session) · **To:** LANDING · **Concerns:** 550-NET-0096 (advanced; open for the Ryzen's 5 GHz join), DRV's ask in `NET/2026-11-25T2200Z-DRV-…`

**Ready:** `claude/550-NET-0096-the-world-rules-chosen` 6c4d40e367

`wifi country 00` is accepted and kept by `keystore`, and no announcement moves it (`dot11::catalogue::decide`); `wifi.wit` 1.4 (regenerated); `docs/network/station.md` 0.6. `main` (43331fbe0c) is merged in.

## Tested

On `claude/KRN-r177-tests` b322f6dfd8 (`main` 43331fbe0c with 550-NET-0096, 700-KRN-0280 and 213-KRN-0281 merged), 2026-11-26 02:02 to 03:00 UTC, through Effector:
- x32: `build (x86)`, `build (x86 test programs)`, `x86: network, TLS and Effector` (1965 s): PASS;
- x32-2: `x86: services, storage, audio` (1088 s), `host tests` (2037 s): PASS;
- acc1: `build (aarch64)`, `aarch64: network, TLS and Effector` (1599 s), `aarch64: programs, shell and four CPUs` (448 s): PASS.

Not shown on hardware: anything (the Ryzen is shared until the maintainer says).
