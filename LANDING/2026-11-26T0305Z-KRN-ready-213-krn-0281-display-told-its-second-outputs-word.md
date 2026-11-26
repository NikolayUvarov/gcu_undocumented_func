# KRN landing request: display told its second output's word (213-KRN-0281)

**From:** KRN (the kernel session) · **To:** LANDING · **Concerns:** 213-KRN-0281 (advanced; open for DRV's Ryzen run), DRV's ask 1 in `KRN/2026-11-25T2203Z-DRV-…`; it pairs with DRV's 213-DRV-0099 (`LANDING/2026-11-26T0123Z-DRV-…`) but does not need it

**Ready:** `claude/213-KRN-0281-display-word-from-displays-txt` 60a2856765

`init` reads `output second off|read|light` in `data/displays.txt` at each `display` start and passes `<device> <word>`, else `off` and says which; `libmind::displays`; `tests/displays_host.rs`. `main` (43331fbe0c) is merged in.

## Tested

On `claude/KRN-r177-tests` b322f6dfd8 (`main` 43331fbe0c with 550-NET-0096, 700-KRN-0280 and 213-KRN-0281 merged), 2026-11-26 02:02 to 03:00 UTC, through Effector:
- x32: `build (x86)`, `build (x86 test programs)`, `x86: network, TLS and Effector` (1965 s): PASS;
- x32-2: `x86: services, storage, audio` (1088 s), `host tests` (2037 s): PASS;
- acc1: `build (aarch64)`, `aarch64: network, TLS and Effector` (1599 s), `aarch64: programs, shell and four CPUs` (448 s): PASS.

Not shown on hardware: anything (the Ryzen is shared until the maintainer says).
