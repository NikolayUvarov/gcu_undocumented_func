# Ready: a short press of the power button shuts the machine down cleanly, on x86 (000-KRN-0216)

**From:** KRN · **To:** LANDING · **Concerns:** 000-KRN-0216 (asked by LNK for the maintainer), the DEXP N100 · **Also carries:** a corrected comment in `get_runs` (000-NET-0048)

**Ready:** `claude/youthful-mendel-mf1soy` 6c9ff9648ea968fcc4ef117ab60a35dfadcaa025

It contains `main` 57da2a98 (round 90).

## What it brings

- **`common/fadt.rs`:** one parser of the FADT's fixed hardware, included by the kernel and `libmind`. Its host tests are in `tests/fadt_host.rs` (5).
- **The kernel lends the PM1 event blocks** through `PLATFORM_PORTS`, as it lends the ECDT's ports:
  - once, after putting the chipset in ACPI mode and disabling every GPE and fixed event;
  - `SMI_CMD`, PM1 control and the GPE blocks stay with it;
  - no ABI change.
- **`REBOOT` names its caller on the console:** `MIND CORE KERNEL: init (PID 1) ASKED TO TURN THE MACHINE OFF`, or `… TO RESTART THE MACHINE`.
- **`init`** takes the blocks and the SCI's line before it drops the platform privilege. On a press it does its restart path, with `POWER-OFF` as the stop notice and S5 at the end.
- **The hardware report's FADT line** names the button's kind, `SCI_EN` as found and the PM1 event blocks.
- **The `power` suite** is added to `x86: boot, display, shell, memory, clock`, in `ci_local.sh` and in CI. On QEMU's pc and q35:
  - a reset before a minute past `READY` gives a safe start;
  - a press in that boot turns the machine off;
  - the next boot is no safe start, and a file written before the press is there, its volume not left dirty.
- **Docs:** `docs/profile/bootstrap.md`, `kernel-objects.md`, `docs/write-disk.md` and its Russian version.

**Every x86 boot now runs in ACPI mode,** with GPEs off, where the FADT names a fixed-feature button.

## Tested, through Effector

**On a581b918** (6c9ff964 restores one expected line of the edit suite that my edit had changed):
- **x32-128:**
  - `host tests` (578 s);
  - `boot, display, shell, memory, clock` with `power` (696 s);
  - `the watchdog` (129 s);
  - `one CPU`, `AVX state, one CPU`;
  - `services, storage, audio` (569 s);
  - `scheduling, isolation, heap`;
  - `keys, shell, tools`;
  - `x2APIC`, `no PIT`.
- **acc-2:** `network, TLS and Effector` (1157 s), `the updater and an update through the link` (856 s).
- **PCU, aarch64:** `boot and fault containment`, `programs, shell and four CPUs`.
- **acc-1, aarch64:** `network, TLS and Effector`, `files and the block store`.

**On 6c9ff964:** acc-2, `x86: files and block writes` (942 s). It had failed on a581b918 on the edit suite's line alone.
