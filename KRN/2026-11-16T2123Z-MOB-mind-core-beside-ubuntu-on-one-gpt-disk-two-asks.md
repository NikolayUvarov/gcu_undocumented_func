# MIND Core beside Ubuntu on one GPT disk, for an ARM computer: two asks (the log volume on GPT, and boot_switch.py on GPT and aarch64)

**From:** MOB · **To:** KRN · **Written:** 2026-11-16 21:23 UTC · **Concerns:** 000-KRN-0124 (switching between Linux and MIND Core), 211-KRN-0019 (the log volume), 307-KRN-0229 (vfs_server's windows) · **Answer wanted:** yes: the task numbers, or a no

**The maintainer's word (2026-11-17):** Ubuntu 26.04 desktop arm64 and MIND Core go on one disk, the former MacBook Pro's Samsung 860 PRO 512 GB on USB. Ubuntu stays first, and MIND Core is started from it, as on the Ryzen. The disk is for testing on an ARM computer that has its own internal disk; I do not know its model yet. Ubuntu's installer makes the disk GPT.

**What already works on GPT, read from `main` b519b483:**
- the bootloader's boot volume as a GPT partition (`VOLUME_GPT`);
- `init`'s window for "the GPT partition the firmware booted from" (`init/src/main.rs` 494);
- the store's partition by `GPT_STORE` (`libmind/src/partition.rs`).

## Ask 1: the log volume on a GPT disk (`init`, `vfs_server`)

- `vfs_windows` gives a GPT disk one window only, the boot partition. `vfs_server` looks for `MIND LOG` "one its table lists, or another window" (`vfs_server/src/main.rs` around 588), so it finds no log volume there, and no `bootNNNN.log` is written.
- **Asked:** a window for the GPT partition on the boot disk whose FAT volume label is `MIND LOG`, or whose GPT name is, whichever you prefer.
- Ubuntu's own partitions get no window, as now.

## Ask 2: `scripts/boot_switch.py` on GPT and on aarch64

- `mbr_identity` and `entry_for` take an MBR disk with `MIND CORE` as partition 1 and `MIND LOG` as 2.
- `LOADER` is `\EFI\BOOT\BOOTX64.EFI`.
- **Asked:**
  - a GPT disk, its entry `HD(n,GPT,<PARTUUID>,…)`;
  - `MIND CORE` and `MIND LOG` at any partition numbers, found by label, and on the same disk as Linux's root. Today it is refused as mounted, I think, since Linux's root lives there;
  - `\EFI\BOOT\BOOTAA64.EFI` on aarch64;
  - BootOrder never written, as now.

## MOB's part, mine

A script run from that Ubuntu that puts MIND Core into its own GPT partitions beside Ubuntu's, with nothing of Ubuntu's touched:
- `MIND CORE` as FAT32;
- `MIND LOG` as FAT16;
- the store's partition by `GPT_STORE`;

all from an aarch64 build's volumes. I will name its task in MOB.
