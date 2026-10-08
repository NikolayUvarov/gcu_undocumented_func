# 210-APL-0014 — Our own first stage, started by iBoot, instead of m1n1

**Type:** porting (boot) · **Owner:** `APL` track (open) · **Priority:** P3 · **Status:** open · **Blocked by:** [210-APL-0013](210-APL-0013-own-stage-two-instead-of-u-boot.md); a Mac with M1 · **Roadmap:** track H · **Constitution:** MC-9.1, MC-9.3, MC-12.1

## Problem

The furthest step from foreign code is a loader that iBoot starts itself. Per Asahi Linux's documentation (none of it checked here), this is possible:

- macOS's boot policy allows a custom kernel in "permissive security" mode. It is set once from recoveryOS with Apple's `kmutil configure-boot` for one boot volume, and macOS stays.
- iBoot then loads a raw image at EL2 and passes a boot-arguments structure. That structure holds the memory size, the framebuffer iBoot set up, and Apple's device tree (ADT), which is not a flattened device tree.
- The new boot volume needs a small macOS "stub" container with the firmware iBoot loads for it. The Asahi installer makes that container today.

What m1n1 does there, and our own stage would have to do:

- read the ADT;
- the per-core set-up of Apple's CPUs;
- power domains (PMGR) for the devices it uses;
- for its own updates, read a second stage from the EFI system partition through Apple's NVMe (ANS, with RTKit firmware).

Without that last part, every update of the first stage means booting recoveryOS and running `kmutil` again. That is at odds with self-update ([351](351-self-update.md)).

## Plan

1. Install with the Asahi installer's stub and replace only m1n1 with our image (`kmutil configure-boot … --raw`, per Asahi's documentation). Record the steps in the guide, in both languages.
2. Our first stage, `mindboot-apple`, in two parts:
   - an ADT reader;
   - the minimal per-core and PMGR set-up the kernel needs.

   It starts the second stage of 210-APL-0013, appended to it.
3. Updates without recoveryOS:
   - the first stage reads the second from the EFI system partition, which needs an ANS NVMe reader in the first stage;
   - or the first stage stays fixed and small, and only the second stage and the system are updated, by 351.
4. Borrowing from m1n1 is allowed by its MIT licence, with attribution in THIRD_PARTY.md. Linux's drivers (GPL) are not.

## Acceptance criteria

On an M1 Mac, iBoot starts `mindboot-apple`, which reaches the kernel through the second stage of 210-APL-0013 with no m1n1 and no U-Boot. The guide records the install and the way back to macOS. The TCB of the future profile is iBoot and our own code.

## Related

[210](210-apple-silicon-native.md), [210-APL-0013](210-APL-0013-own-stage-two-instead-of-u-boot.md), [351](351-self-update.md).
