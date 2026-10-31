# 000-KRN-0198 — Two waits of the x86 suites are short for a loaded host: the loader's error and a program's start

**Type:** tests · **Owner:** `KRN` · **Priority:** P2 · **Status:** in progress (made 2026-10-31; runs on the machines left) · **Blocked by:** — · **Main task:** — · **Roadmap:** the landing queue (AGENTS.md section 4) · **Constitution:** MC-12.2

## Problem

In round 62 (candidate 74cf7265), two groups on `un-rizen-service` failed their first run while the machine ran a second gate lane and the host tests. Each passed when run again alone.

- **"x86: boot, display, shell, memory, clock", the `boot` suite.** It changes `rtc.elf` and waits 30 s from QEMU's start for `BOOT ERROR: rtc.elf: not as the manifest says`.
  - The loader had printed `STARTED; READING THE KERNEL AND THE SERVICES FROM ITS OWN VOLUME` and its safe-start line, and then nothing within the 30 s.
  - The loader reads and hashes the images in order, from a virtual FAT disk, before it reaches the one it names. Under TCG on a loaded host, the firmware's start and those reads together took longer than 30 s.
- **"x86: RAM above 4 GiB", the `vfs` suite.** It waits for the prompt after `run record -t 3 data/rec.avi &` with the shell's default limit of 8 s, and the prompt did not come in time.
  - The step before it wrote two recordings into `data/`.
  - A program's image is read through `vfs_server`, which may still be writing the last file. Round 48 failed later in the same step on the same machine.

## Plan

- **The loader's error:** wait for the loader's start (120 s, as the `modes` suite's safe start does), and then for its error (60 s). A loader that stops without naming the file still fails.
- **The program's start:** 30 s for that one `run … &`. The suite still checks that `record` starts in the background and records 3 s.

## Acceptance criteria

- [ ] Both groups pass on `un-rizen-service` and `acc-1` with the change.
- [ ] The landing gate runs them with neither wait failing.

## Related

- 350-UPD-0003: the signed boot volume, whose errors the `boot` suite checks.
- 093: `record`.
- Round 62 of the landing queue; 000-KRN-0197 (the other gate failure of that round, on PCU).
