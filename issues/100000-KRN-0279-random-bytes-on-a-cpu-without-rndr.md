# 100000-KRN-0279 — Random bytes on a CPU without RNDR

**Type:** security, porting (ABI) · **Owner:** `KRN` (the kernel session), with `NET` for `keystore` · **Priority:** P2 · **Status:** open (2026-11-25) · **Blocked by:** — for the design and QEMU; a PinePhone or an Allwinner board for the run · **Main task:** [100000](100000-a-phone-the-pinephone-first.md) · **Roadmap:** track H · **Constitution:** MC-11.9, MC-12.1, MC-12.3, MC-12.4

## Problem

`libmind::random` takes its bytes from the processor's instruction alone (RDRAND, or RNDR on aarch64) and fails closed without it (issue 103). A Cortex-A53 has no RNDR, and every Allwinner SoC of 100000 and 100004 (A64, H5, H618) has A53s. So on the phone and the boards nothing that needs a random number works:
- `keystore` makes no device key (`[KEYSTORE] NO RNDR: NO DEVICE KEY`, found in QEMU on 2026-11-08);
- TLS, SSH and every other caller of `libmind::random` refuse to start.

This blocks stage 2 and later of 100000 (the network, the updater's signed fetch, MIND Hub's device key), not stage 1's boot.

## Plan

1. **A seed handed over at boot,** from what the phone's firmware gives, in this order, each checked in the loader:
   - UEFI's `EFI_RNG_PROTOCOL`, if Tow-Boot's U-Boot provides it (whether it does on the A64 is to be checked);
   - the device tree's `/chosen/rng-seed`, if U-Boot fills it.
   A boot with neither has no seed and stays failing closed, saying so on the console.
2. **The A64's own source** (its crypto engine's PRNG or TRNG, by the A64's manual) as a driver, if 1 gives nothing; to be surveyed.
3. **The kernel keeps a generator** seeded from 1 or 2 and answers a new system call with bytes from it (an ABI change, a new ABI version), and `libmind::random` uses it where the instruction is missing.
4. **Mixing:** where both exist (x86 with RDRAND, a CPU with RNDR), the instruction stays the source, as today.

## Acceptance criteria

- Host tests of the generator and of reading the seed from a device tree.
- QEMU `virt` with a Cortex-A53 and U-Boot's UEFI: `keystore` makes its device key, and the `net` suite's SFTP login with it passes (evidence for QEMU only, MC-12.1).
- On a PinePhone or an Allwinner board: which source gave the seed, recorded with the firmware's version.
- `docs/profile` names the source per platform and, for a boot without one, that nothing random works (MC-12.3).

## Related

[100000](100000-a-phone-the-pinephone-first.md) (found on the way, stage 0), [100004](100004-two-allwinner-boards-before-the-phone.md), [103](../issues-done/103-tls-service.done), `libmind/src/random.rs`.
