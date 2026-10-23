# 351-KRN-0167 — A kernel that refuses its loader on a trial boot restarts the machine

**Type:** kernel (`main.rs`, `trial.rs`, `arch/*`) · **Owner:** kernel session · **Priority:** P1 · **Status:** open (made on `claude/youthful-mendel-mf1soy`; landing left) · **Blocked by:** — · **Main task:** [351](351-self-update.md) · **Constitution:** MC-6.5, MC-6.8, MC-12.4

From EFF's request of 2026-10-23 (`internal-dev-messages`, `KRN/2026-10-23T1856Z-EFF-two-requests-a-slot-update-across-a-syscall-abi-change.md`), filed at the maintainer's word.

## Problem

- **The kernel refuses before the trial starts.** It checks the loader's version before `trial::start` (`kernel/src/main.rs`) and halts on a mismatch.
- **On a trial boot that halt leaves the machine stuck.** It never restarts into the fallback slot, the link cannot reboot it, and a person has to.
- **Outside a trial the halt is right.** There is no other slot to fall back to, and the message says what to do.

## Plan

1. **Read whether the boot is a trial without trusting the rest of `BootInfo`.** The flag must sit where every loader that sets trials writes it. Either it joins the fixed prefix that is read before the check (with `abi_version`), or the kernel reads it at the offset that the loaders of every accepted era agree on. Which of the two, is decided against `BootInfo`'s history (see [351-KRN-0166](351-KRN-0166-the-loaders-handoff-versioned-apart-from-the-system-call-abi.md)).
2. **On a trial, a refusal restarts the machine** without using `BootInfo`'s other fields:
   - x86: the reset control register (port `0xCF9`), then the keyboard controller's reset;
   - aarch64: PSCI `SYSTEM_RESET`.

   Before that, the message goes to the serial line and the screen's early text, as now, with "TRIAL: RESTARTING INTO THE FALLBACK".
3. **The loader then counts the failed try.** It already counts a trial that is not confirmed and boots the fallback slot (351-KRN-0014).
4. **Outside a trial the kernel halts,** as now.

## Acceptance criteria

- [x] QEMU, x86 and aarch64: a trial boot of the test kernel that refuses its loader (`loader-abi-test`) restarts the machine, and the loader boots the fallback slot, which confirms. (Slot A boots as the confirmed slot, not on trial, so there is nothing for it to confirm; see Progress.)
- [x] The same kernel outside a trial halts with the message.
- [x] `docs/profile` states what happens on a refused handoff, in a trial and outside one.

## Progress (2026-10-23)

Made on `claude/youthful-mendel-mf1soy` (b0535581):
- **Where the flag is read.** The kernel reads it at its offset in ABI 10's record, the second choice of the plan. Every handoff keeps that record in front: the framebuffer, `acpi_rsdp`, the version word and `boot_slot` (`common/abi.rs`; `BootInfo::read_refused`). `BootSlot` has sat there since ABI 4, and the host test `boot_abi` pins the offsets against an independent ABI 10 record. A version below 10 is not read for a trial: the kernel halts, as before.
- **The restart.**
  - On a trial, the refusal adds `TRIAL: RESTARTING INTO THE FALLBACK`.
  - x86: `acpi::reset` writes port `0xCF9`, then the 8042 controller, then a triple fault. This is also the tail of the ordinary `reboot`.
  - aarch64: `acpi::early` reads only the SPCR (the console, so the refusal now reaches the PL011) and the FADT (PSCI's conduit). `acpi::reset` calls PSCI `SYSTEM_RESET` when the FADT names PSCI; otherwise it says so and the kernel halts.
- **The fallback slot.** The loader counted the try, so the next boot reads `BOOT: SLOT B NOT CONFIRMED, NO TRIES LEFT` and loads slot A as the confirmed slot. A slot booted that way is not on trial, so it has nothing to confirm; the criterion's "which confirms" is met by A starting init.
- **Checked** (this session's container, QEMU TCG):
  - `boot_slots_check.refused_trial`, with slot B holding the `loader-abi-test` kernel: the x86 `boot` suite and `tests/aarch64_smoke.py` (with a new aarch64 fixture, `fixture-kernel-loader-abi.elf`);
  - outside a trial, the x86 `boot` suite: the kernel stays stopped with QEMU's `-no-reboot`;
  - host test `a_refused_handoff_reads_the_trial_and_the_acpi_root_from_abi_10s_record`.
- **Not done.** An aarch64 board without ACPI, or whose FADT names no PSCI, halts instead. The restart was not tried on a machine.

## Related

[351-KRN-0166](351-KRN-0166-the-loaders-handoff-versioned-apart-from-the-system-call-abi.md), [351-KRN-0141](351-KRN-0141-an-update-through-the-link.md), [351-UPD-0010](351-UPD-0010-updating-the-bootloader.md), [351](351-self-update.md).
