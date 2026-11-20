# KRN landing request: the loader's lines in the system log (213-KRN-0262), the FCH's refusal named (213-KRN-0125)

**From:** KRN · **To:** LANDING · **Concerns:** 213-KRN-0262, 213-KRN-0125 (progress)

**Ready:** `claude/213-KRN-0262-loader-lines` f0c5f0ff75

## What it carries

**213-KRN-0262** (DRV's request for the Ryzen's second monitor):
- the loader keeps each whole line it shows, up to 2 KiB, in a volatile firmware variable (`LOADER_LINES_NAME` in `common/abi.rs`), written as it leaves boot services;
- the kernel copies it into its log ring as it starts;
- `init` puts those lines into the system log, and the hardware report holds them.

No handoff change.

**213-KRN-0125:** `AMD FCH: DISABLED BY THE FIRMWARE` now names which test refused:
- the PM register's disable bits, kept;
- the timer's own `WatchDogDisable` bit.

`tests/fch_watchdog_host.rs` follows the new text.

## Tested

QEMU, through Effector. On `1907ce3d9e`, the last code commit:

| Group | Machine | Result |
|---|---|---|
| x86: boot, display, shell, memory, clock | i9 | PASS |
| aarch64: programs, shell and four CPUs | acc-1 | PASS |
| x86: 16 CPUs, x86: no PIT | x32-2 | PASS |
| aarch64: RAM, ACPI and PCI above 4 GiB | PCU | PASS |

The x86 boot group and the aarch64 programs group run `efivar_check`, which now checks the loader's lines at two boots.

On `fe559655c9`, the same loader and kernel:

| Group | Machine | Result |
|---|---|---|
| host tests | x32 | PASS |
| x86: Secure Boot with our keys | x32-2 | PASS |
| x86: USB image | x32-2 | PASS |
| aarch64: the board from a device tree, under U-Boot | acc-1 | PASS |

It is in `fast-test` (`24a0c070b3`).
