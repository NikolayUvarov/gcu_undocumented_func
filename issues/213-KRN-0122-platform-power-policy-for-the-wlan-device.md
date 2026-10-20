# 213-KRN-0122 — The platform's transmit power policy for one WLAN device: known limits, known absent, or unknown

**Type:** kernel, `init` (an interface to choose: a typed result or a scoped snapshot) · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** — (the drivers track's 213-DRV-0032 waits for it before it lets the RTL8852BE transmit) · **Main task:** [213](213-amd-ryzen-pc.md) · **Constitution:** MC-2.3, MC-3.3, MC-6.1, MC-12.1, MC-12.3

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

## Related

[213-DRV-0032](213-DRV-0032-rtl8852be-wifi.md), [213-KRN-0123](213-KRN-0123-the-rtl8852be-radio-endpoint.md), 174-KRN-0038 (the hardware report), [213](213-amd-ryzen-pc.md).
