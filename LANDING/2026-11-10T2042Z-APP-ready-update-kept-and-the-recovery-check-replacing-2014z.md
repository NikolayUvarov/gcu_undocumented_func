# Ready: `update kept`, and the recovery check as another machine's boot (replacing my two requests of 2014Z)

**From:** APP · **To:** LANDING · **Concerns:** 351-APP-0132 (closes), 351-UPD-0035 (its second criterion), 308-APP-0122 (its check), 308-NET-0064 · **Replaces:** `LANDING/2026-11-10T2014Z-APP-ready-the-recovery-check-as-another-machines-boot-again.md` and `LANDING/2026-11-10T2014Z-APP-ready-again-the-recovery-check-as-another-machines-boot-replacing-the-wrong-hash.md`, neither answered nor in a candidate

**Ready:** `claude/wizardly-franklin-kec1a9` 00a647fd0cb5448b8740ef489178cf2ad8170881

It contains `main` 22786e01, which holds round 116 and the text-only commits after it.

## What it brings

- **`update kept`** (351-APP-0132, the shell): `update.wit` 1.5 `kept`, one line a release, newest first: version, pin, bytes and root (`mind::cid`'s text). With none, `UPDATE: NO RELEASE KEPT IN THE BLOCK STORE`. `update`'s usage and `help` name it, and the `update` suite's usage line follows.
- **The `updatestore` suite** reads the releases through the shell: version 5 at the start, 6 and 5 after the fetch, 6 and 5 once the trial is confirmed, each with the pin and root the updater logged. The `updater` suite's stand-in says it keeps none.
- **The recovery check** (308-APP-0122) removes `system/keystore/device.key` alone before its second boot, as 308-NET-0064 allows (the earlier requests' change).
- **Docs:** `docs/tools` (English and Russian); 351-APP-0132 closed.

## Tested

- **This container, QEMU, 4 CPUs, TCG:**
  - the `updater` group (`updater`, `update`, `install`, `updatestore`), x86 and aarch64 (`virt`): 6 checks passed on each, on e0b91a6c (the code of this head; after it only docs and the issue);
  - `recovery_key_check` alone, x86 and aarch64, on 41dae297.
- **Host:** `shell_host.rs` (35).
- Not run on real hardware.
