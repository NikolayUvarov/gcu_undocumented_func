# 352-APP-0157 — The shell's `install`: MIND Core from a live boot onto a disk of this machine

**Type:** shell command · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** in progress (done on x86 in QEMU by hand, 2026-11-21; the `iso` suite's run, KRN's, and aarch64 left) · **Blocked by:** — ([352-KRN-0269](../issues-done/352-KRN-0269-the-installer-service.done), the installer's service, is in `main`, round 165) · **Main task:** 352, through [352-UPD-0048](352-UPD-0048-the-installer-onto-a-disk-of-this-machine.md) · **Roadmap:** track C (update) and G · **Constitution:** MC-3.11, MC-10.3, MC-12.3

**Recorded by:** the tools track, 2026-11-21, from KRN's request in `issues/requests-APP.md` ("The shell's `install` command, for 352-UPD-0048").

## Problem

- **The installer's service exists:** `install.wit` 1.0, served by `vfs_server` on a live boot to the shell's client in `SLOT_INSTALL`.
- **Nothing calls it yet.** A person on a live boot has no way to put MIND Core on the machine's disk.

## Plan

- **`install`,** where `SLOT_INSTALL` holds an endpoint (a live boot); elsewhere it says it is not a live boot.
- **`install` alone** lists the disks:
  - each with its bus, model where the drive names one, MiB, and whether it is empty or its MBR and partitions;
  - then either the words to type back (`INSTALL 1, THEN TYPE VIRTIO 512 EMPTY`) or why it is refused (the live medium, too small, past 2 TiB, read-only, mounted).
- **`install N`:**
  1. shows disk N, and that its partitions and all their files are erased when it lists any;
  2. refuses a disk the installer refuses, naming why;
  3. asks for the disk's words, typed at this machine's own console. A line of the link, of the link's console or of an SSH session is not asked (MC-12.3), and says so;
  4. asks a second time for a disk with partitions, naming how many;
  5. says it is writing, and that the file service answers nothing else meanwhile, then calls `install`;
  6. shows the layout written (each partition's first sector and size, the MBR's signature), or the step that failed. After a failed step of the writing it says `THE DISK HAS NO ESP: INSTALL AGAIN`.
- **Logged:** what it writes, what it wrote, what failed, and a line typed elsewhere that was not asked.

## Acceptance criteria

- **Host tests** (`tests/shell_host.rs`): the arguments; a disk's line, empty, with partitions and a model, and refused; the second question; each error, and which ones leave no ESP; the layout's words.
- **QEMU:** the `iso` suite boots the ISO with a blank disk. `install` lists it, and `install 1` with its words writes it. The disk then boots alone. The suite is KRN's: KRN adds this run once the command is on this branch.
- **Docs:** the shell's `help` line, `docs/tools/README.md` and `README_RU.md`.

## Progress (2026-11-21)

- **Written:**
  - `shell/src/installer.rs` holds the command's words, `installctl.rs` its calls;
  - the line is read at the console with Backspace and Esc, 32 characters at most, within 2 minutes;
  - the `help` line.
- **Then:** mistyped words are refused at once, before the second question. The installer compares them again.
- **Tested:**
  - `tests/shell_host.rs`'s `install_lists_shows_and_names_what_failed`; the shell and tools suites pass with the command.
  - **The whole install on x86 in QEMU,** by a script outside the repository on the `iso` suite's machine: the ISO as a CD-ROM and a blank 512 MiB disk.
    - `install` listed `1  ATA  512 MIB  EMPTY  TO INSTALL: INSTALL 1, THEN TYPE ATA 512 EMPTY`.
    - `install 1` with `ata 512 empty` wrote it: `INSTALL: DONE ON DISK 1, MBR 0A073267: THE ESP AT SECTOR 2048 (184320 SECTORS), MIND LOG AT 186368 (131072), THE STORE AT 317440 (262144)`.
    - A second `install 1` showed `MBR 0A073267, 3 PARTITIONS` and refused mistyped words.
    - The disk then booted alone, without the ISO, to the shell's prompt, its `store.txt` there.
  - This is a run by hand, not a check of the repository's (MC-12.2).
- **Left:**
  - the `iso` suite's run, KRN's, asked in `KRN/`;
  - aarch64 (`VIRTIO 512 EMPTY`);
  - a real machine.

## Related

[352-UPD-0048](352-UPD-0048-the-installer-onto-a-disk-of-this-machine.md), [352-KRN-0269](../issues-done/352-KRN-0269-the-installer-service.done), [351-APP-0029](../issues-done/351-APP-0029-update-in-the-shell.done) (the shell's `update`).
