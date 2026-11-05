# 213-KRN-0122 — The platform's transmit power policy for one WLAN device: known limits, known absent, or unknown

**Type:** kernel, `init` (an interface to choose: a typed result or a scoped snapshot) · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress · **Blocked by:** — (the drivers track's 213-DRV-0032 waits for it before it lets the RTL8852BE transmit) · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-2.3, MC-3.3, MC-6.1, MC-12.1, MC-12.3

## Problem

The RTL8852BE's channel path needs the platform's SAR and antenna-gain limits, apart from the firmware's default tables and the operator's country. The pinned RTL8852B source looks up `WRDS`, `RWRD`, `RWSI`, `RWGS` and `RTAG` relative to the WLAN device's ACPI node; a name in an unrelated scope is not a policy. Today:
- `PLATFORM_ACPI_TABLE` copies single tables read-only during `init`'s start, and `init/src/hardware.rs` writes them to `log:acpi/` for diagnosis;
- nothing ties a table, or the absence of a method, to one PCI device, and the radio's service holds no platform authority after `init` drops it, as it should.

A read-only Ubuntu run on the Ryzen (Effector operation `20261015233912.417877721_000172`) found none of the five names in its DSDT and nine SSDTs, and no `firmware_node` for PCI `03:00.0`. That narrows what to expect; it is not a MIND result. The request of 2026-10-16 is in [requests-KRN.md](requests-KRN.md).

## Plan

1. **A typed result, not a general AML interpreter.** `init` (or the kernel's report builder) gives the radio's service a result for its own device:
   - `KnownLimits`: the limits and antenna offsets found, their table, its checksum and the policy's version;
   - `KnownAbsent`: every table was read whole, checked, and searched for the five names in the device's scope and above it, and none is there;
   - `Unknown`: a table missing, truncated, malformed, or AML this search does not follow (a method computing the value, a scope it cannot resolve).
2. **Bounded.** A table limit and a step limit; reaching either is `Unknown`, never `KnownAbsent`.
3. **Bound to the device.** The result names the PCI segment, bus, device and function and the ACPI path matched by `_ADR`; a service gets only its own device's.
4. **Unknown is not permission.** The drivers track's consumer treats `Unknown` as no transmit; this task gives it the distinction, not the decision.
5. **No platform capability** is lent to the radio's service. If the result needs an interface (`hwinfo.wit` or a new one), it gets a version and a migration note.
6. **x86 first;** aarch64 waits, as the maintainer deferred ARM.

## Acceptance criteria

- [ ] Host tests with fixtures: valid limits, a supported absence, truncated and malformed tables, an unsupported method, and a policy in an unrelated scope; each gives its own result with the device's identity and the source.
- [ ] A service cannot get another device's result, and an error never becomes `KnownAbsent`.
- [ ] On the Ryzen, the result for `03:00.0` is recorded with its log, apart from the fixtures.

## Progress

2026-11-04 (d47cd90a):

- **`mind::platform_power`** (`libmind/src/platform_power.rs`):
  - every DSDT and SSDT read whole, its length and checksum checked;
  - the five names searched for as bytes, so no AML is run.
  - **The answers:**
    - `Absent`: every AML table was read whole and checked, there is a DSDT, and no name occurs anywhere;
    - `Unknown`, with its reason, otherwise: a name anywhere, a table cut short, a bad checksum, a table that could not be mapped, no DSDT, or more than 64 tables or 16 MiB.

    A byte search cannot miss a name the AML holds. So `Absent` is never given in error; a false match only makes it `Unknown`.
- **`KnownLimits` is not made yet.** A table that names any of the five is `Unknown` until its AML (a `Name` with a `Package`, or a method) is read. The Ryzen's Ubuntu read found none of the five, so `Absent` is what MIND Core should find there.
- **`init`** reads it once at boot, while the platform privilege allows, keeps it for the radio's starts, and logs:
  - `[INIT] PLATFORM POWER POLICY FOR WLAN: ABSENT (n AML TABLES READ WHOLE, NONE NAMES WRDS, RWRD, RWSI, RWGS OR RTAG)`;
  - or `… UNKNOWN (<reason>): A RADIO SENDS NOTHING`.
- **Tests:**
  - host tests in `tests/platform_power_host.rs` (6), with tables made in the test: an absence; each name in an SSDT; a cut table; a short header; a bad checksum; an unmapped table; no DSDT; too many tables;
  - QEMU: the `normal` suite requires `ABSENT`. Seen once on x86 and once on aarch64 in this session's container, each with QEMU's one DSDT.
- **Not given to the driver yet.** `rtw89_wifi` reads its arguments as one number, its PCI device index. `init` will add `platform=absent` or `platform=unknown:<reason>` after the index once DRV's parser takes the word. The form is in `DRV/2026-11-04T1003Z-KRN-…`.

**Left:**
- the word in the driver's arguments, after DRV;
- the result on the Ryzen with its log;
- a service that is refused another device's result: today the record goes to the one driver `init` starts for the device it found.

**Landed (2026-11-04).** The policy and `init`'s line landed in round 80 (`main` 4130cff6), and the gate's `normal` suite required `ABSENT` under QEMU on both architectures. The word in `rtw89_wifi`'s arguments still waits on DRV.

2026-11-05, the word in the driver's arguments:
- **DRV's parser** (`rtw89_wifi/src/args.rs`, ab665df1) reads the index, then the `platform=` word. It is in DRV's landing request `LANDING/2026-11-05T0545Z-DRV-…` (444790b9) and in `fast-test`.
- **`init`** now starts `rtw89_wifi` with `<index> platform=absent` or `<index> platform=unknown:<reason>`, from the policy it read at boot.
- **`Policy::word`** gives only printable ASCII without a space, whatever bytes a signature holds. A new host test found that a raw byte would have made `init` drop the word.
- **Host tests** (`tests/platform_power_host.rs`, 7): every word is one the parser reads: `platform=absent`, or a reason of 1 to 48 printable bytes.
- **Not tested in QEMU:** it has no RTL8852BE, so `rtw89_wifi` does not start there.
- **Land it with DRV's parser or after it, never before.** An earlier driver stops on a second word with `NO PCI DEVICE INDEX`.

**Landed (2026-11-05).** Round 83 (`main` 3460771e) landed DRV's parser (444790b9) first, then `init`'s word (01863699). The gate was green on every group, but QEMU has no RTL8852BE, so no run gave the driver the word yet.

**Left:** the result on the Ryzen with its log, from a build of `main` 3460771e or later; a service refused another device's result.

## Related

[213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md), [213-KRN-0123](213-KRN-0123-the-rtl8852be-radio-endpoint.md), 174-KRN-0038 (the hardware report), [213](213-amd-ryzen-pc.md).
