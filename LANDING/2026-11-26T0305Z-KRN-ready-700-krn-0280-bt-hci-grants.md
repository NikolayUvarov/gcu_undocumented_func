# KRN landing request: bt_hci's input privilege and data/bluetooth/ (700-KRN-0280)

**From:** KRN (the kernel session) · **To:** LANDING · **Concerns:** 700-KRN-0280 (advanced; open for BLT's run), BLT's ask in `KRN/2026-11-25T2243Z-BLT-…`

**Ready:** `claude/700-KRN-0280-bt-hci-grants` 2f35a05272

`init` gives `bt_hci` the input privilege and a writable `data/bluetooth/` in `SLOT_FILE`; `tcb.md` on both architectures and `bootstrap.md` say so. No ABI change. `main` (43331fbe0c) is merged in.

## Tested

On `claude/KRN-r177-tests` b322f6dfd8 (`main` 43331fbe0c with 550-NET-0096, 700-KRN-0280 and 213-KRN-0281 merged), 2026-11-26 02:02 to 03:00 UTC, through Effector:
- x32: `build (x86)`, `build (x86 test programs)`, `x86: network, TLS and Effector` (1965 s): PASS;
- x32-2: `x86: services, storage, audio` (1088 s), `host tests` (2037 s): PASS;
- acc1: `build (aarch64)`, `aarch64: network, TLS and Effector` (1599 s), `aarch64: programs, shell and four CPUs` (448 s): PASS.

Not shown on hardware: anything (the Ryzen is shared until the maintainer says).
