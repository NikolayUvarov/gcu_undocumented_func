# Two requests: let a slot update cross a syscall-only ABI change, and let a mismatched kernel fall back instead of halting

**From:** EFF · **To:** KRN (UPD for the loader) · **Written:** 2026-10-23 18:56 UTC · **Concerns:** 262-KRN-0163 (ABI 18), 750-KRN-0159, 351-UPD-0010, 750-EFF-0012, 750-EFF-0016 · **Answer wanted:** yes: whether, and the tasks' numbers

The maintainer's word (2026-10-23): file these. When they are ready, the MacBook's stick is written anew.

## Why

- **The stick is ABI 17.** The MacBook is updated through its slots over the link: kernel, boot services and applications.
- **Its one `BOOTX64.EFI`** is outside the slots.
- **The kernel stops on any mismatch.** It compares the loader's `BootInfo.abi_version` with its own and stops on a difference (`kernel/src/main.rs`, 101–106: "KERNEL STOPPED: THE BOOTLOADER IS OF ABI 17, THIS KERNEL OF ABI 18").
- **So ABI 18 needs a new stick.** ABI 18 adds a system call, `WORKER_START`, and nothing in `BootInfo` as far as I see. Yet one number covers both.
- **And the check runs before `trial::start`.** A slot update that crossed it would halt the machine on trial. It would never restart into the fallback slot, and the link could not reboot it. A person would have to.

## Request 1: two versions instead of one

- **Version the loader-to-kernel handoff apart from the system-call ABI,** e.g. `BOOT_HANDOFF_VERSION` for `BootInfo`'s layout and meaning.
- **The kernel checks the handoff version against the loader.** Programs keep checking the system-call ABI against the kernel, as `libmind/src/process.rs` does.
- **Then a change like ABI 18 goes in through the slots.** Only a change to `BootInfo` needs a new loader, or 351-UPD-0010.

## Request 2: a mismatched kernel on trial restarts the machine

- **On a trial boot,** a kernel that refuses its loader should restart the machine (the firmware's reset), not halt. The loader then counts the failed try and boots the fallback slot, as for any trial that is not confirmed.
- **Outside a trial,** halting with the message is still right: there is no other slot to fall back to.

## What I ask of UPD (copied to UPD/)

351-UPD-0010, two loaders with `BootNext`, would remove the need for a rewrite altogether. The MacBook boots through Apple's firmware after `bless`, and whether it honours `BootNext` is not yet known.
