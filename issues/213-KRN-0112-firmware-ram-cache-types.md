# 213-KRN-0112 — Keep firmware RAM cached above 3 GiB

**Type:** kernel / x86 paging · **Owner:** ASTRA auditor-fixer (the maintainer's assignment, 2026-10-15) · **Priority:** P1 · **Status:** in progress · **Blocked by:** — · **Roadmap:** H / [213](213-amd-ryzen-pc.md) · **Constitution:** MC-12.1, MC-12.3

## Problem

The maintainer's Ryzen 9 5900HX boots of `fast-test` commit `e6730d7a1fa5`
on 2026-10-15 reported a roughly five-second save of a small text file, delayed
input, a slow program menu and high CPU use at the desktop. The preserved
hardware reports identify 76.355 MiB of loader-data RAM above `0xC0000000`,
including the bootloader's 64 MiB kernel arena. The comparable MacBook report
has no loader-data RAM above that address.

`kernel/src/arch/x86_64/mmu.rs` maps every address from 3 GiB to 4 GiB with
PCD and PWT set, selecting the uncached PAT entry. The bootloader allocates the
kernel, arena, stack and service images below 4 GiB at the highest addresses
available, so this policy makes real RAM uncached on the Ryzen. It also assumes
every address below 3 GiB is RAM, despite possible device windows there.

The cache-policy error is established by code and the firmware memory map. Its
contribution to the reported delays has not been measured on corrected Ryzen
hardware. The USB adapter enumerates at SuperSpeed; the logs do not establish
a recurring storage transport failure. Storage durability must not be weakened
to conceal the latency. The source files involved are unchanged between the
tested commit and the audit's starting `origin/main`, `94ec83a0`.

## Plan

1. Derive the low identity map's cache policy from the firmware RAM descriptors,
   keeping unknown, reserved and device ranges uncached.
2. Use 2 MiB leaves for uniform ranges and 4 KiB leaves at RAM/device boundaries;
   adjacent RAM descriptors must not leave the kernel arena partly uncached.
3. Keep the device remapping helper correct for either leaf size. Do not change
   the ABI, MTRRs, user-page permissions or the storage flush protocol.
4. Add host regressions for the observed Ryzen RAM placement, descriptor
   boundaries and mixed RAM/MMIO mappings. Run the required kernel suites and
   gate on the agent's own branch.
5. Repeat the Ryzen run and compare file-save latency, program-menu loading,
   input responsiveness and idle CPU. Keep hardware evidence distinct from
   host/QEMU evidence.

## Acceptance criteria

- The actual Ryzen loader-data address ranges select the write-back PAT entry
  throughout, including their boundary pages.
- Unknown and MMIO pages stay uncached, including when next to RAM in one
  2 MiB region; mappings remain supervisor-only and writable.
- The helper used for MSI-X register mappings handles both large and small
  identity-map leaves without changing a page-table pointer's cache type.
- Focused host regressions and the project's kernel gate pass on the stated
  configuration.
- A new Ryzen run records whether the reported delays improved; until then,
  the issue remains open and hardware performance is not claimed fixed.

## Related

- [213](213-amd-ryzen-pc.md), the Ryzen platform task.
- `bootloader/src/main.rs`: `load_elf`, `keep_program`, arena and stack allocation.
- `kernel/src/arch/x86_64/mmu.rs`, `tests/runtime.rs`.
- [Profile evidence](../docs/profile/evidence.md).

## Progress

**2026-10-15, ASTRA:** the identity map now derives cache selection from UEFI RAM
types, joins adjacent RAM descriptors and splits mixed 2 MiB regions into 4 KiB
leaves. The device remapping helper updates actual leaves at either size.
Five focused `identity_cache_*` host tests pass, inspecting generated entries
for the reported Ryzen ranges, RAM/MMIO boundaries, unknown ranges, reserved
overlaps and device remapping. The full kernel gate and Ryzen rerun remain.

`StatPhys` carries memory kinds and ranges but no UEFI cache attributes. This
change does not extend that ABI or rewrite firmware MTRRs. Consequently, the
new tests establish the page-table PAT choice, not the CPU's effective memory
type for every possible firmware allocation.
