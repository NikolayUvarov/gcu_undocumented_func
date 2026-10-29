# KRN ready: `blockstore`'s own secrets in `keystore` (308-KRN-0183), and the tests' service table (109-NET-0051)

**Ready:** `claude/youthful-mendel-mf1soy` 6ed49359

**From:** KRN · **Written:** 2026-10-29 17:39 UTC

`main` ced25026 is merged in, with no conflict.

## What it lands

- **308-KRN-0183** (for STO's 308-STO-0022, P1). It is not closed until this gate passes.
  - `keystore.wit` 1.5: on a client badged `BADGE_KEY_BLOCKSTORE` (5), the secrets calls of 1.4 are about the block store's own secrets, kept in `system/keystore/services/` under a key whose label names a service.
  - `init` lends that client in `blockstore`'s slot 3 when `keystore` runs, and starts `blockstore` right after `keystore` (`services::LATE`). `BOOT_SERVICES` and the ABI are unchanged.
  - **The order of the boot changes:** `blockstore` now starts after `keystore`, not after `link`. Only the shell needs the store, and it starts last.
- **109-NET-0051** (APP's request): the tests' `vm.services()` never takes an exited row for a live service.
- **262-KRN-0178:** the badge retirement recorded (text).

## Tested

- **x86 group "network, TLS and Effector"** whole, on `main` ced25026 with 0183, in this session's container (QEMU TCG, 4 CPUs), 898 s. It includes the new check of `device_key_check`, a raw image booted three times with a blank store disk:
  - the volume key kept by `keystore` and read back at the next boot;
  - no program run by the store's name;
  - another program finding none of the store's secrets, nor opening its file copied under its own name;
  - the file opening no more under a new device key.
- The same group on the branch before 0183, with 109-NET-0051, 890 s.
- **Host tests:** `secrets`, `services`, `idl`, `idl_fuzz`, `boot_abi`; `mind_idl.py --check`.
- **aarch64:** not run before this request. This round's gate runs it.
