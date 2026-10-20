# 351-NET-0034 — `download`'s check at the end: flushed, bounded and in the log

**Type:** network (program) · **Owner:** `NET` track · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [351](351-self-update.md) · **Roadmap:** D / 550, H / 213 · **Constitution:** MC-6.1, MC-12.1, MC-12.3

From the request "Download complete-file recheck stalls in the 6G Intel gate" in requests-NET.md (ASTRA auditor-fixer, 2026-10-16), for [audit H13](../issues-audit/2026-10-16-download-recheck-stall.md).

Numbered 0034 and 0035 (with [550-NET-0035](550-NET-0035-the-stations-country.md)): ASTRA's tasks 550-NET-0031 to 0033, on `fast-test` since 2026-10-15, hold the counters before them.

## Problem

**What was seen.** The record is the audit's, and it stays as it is:
- configuration: candidate ASTRA `ac1854f2` on `main` `bd7b5493`, Intel endpoint `i9_4e6882212251`, operation `20261015221739.924407850_000152`;
- group `x86: RAM above 4 GiB` (`--memory 6G --suites normal,display,net,vfs`);
- the `net` suite's `download_runs`: the second `download` of the complete 30 MiB file printed `DOWNLOAD: ALREADY COMPLETE, 31457280 BYTES` and then nothing for its 300 s;
- the first run, with its cut, its resume and its SHA-256, had passed.

**What the code could not say.** After that line, `download`:
1. called the file's `flush` and dropped its result;
2. read the file back in 16 KiB requests to the file service and hashed it;
3. printed the SHA-256.

None of these steps was logged, and no call in them had a deadline (outside `with_timeout` a call waits for ever). The trace cannot tell:
- a flush that did not come back;
- a read that did not come back;
- reads that came back slowly;
- hashing or scheduling that was slow.

**What is known about the boundary, from the code.** These are candidates, not the cause:
- The default x86 boot disk in these suites is the legacy ATA driver (PIO, no interrupts). Its `command` and `flush` wait for BSY to clear without a bound (`ata/src/main.rs`). A drive that stays busy would hold the file service, and with it every file call. The bound is already asked of `DRV` (requests-DRV.md, "`ata` and `ahci`: waits with a deadline", the critique of 2026-10-14).
- The file service's chain cursor is one per volume (`vfs_server/src/fat.rs`, `read`). Another client reading another file on the same volume at the same time would send each of `download`'s reads back to the chain's start. Nothing in the suite is known to do that at this step.
- A slow host: the first run's check ran under a 900 s limit, the second's under 300 s.

## Plan

- **The flush result is handled.** A flush that fails ends the run with `CANNOT FLUSH FILE: error`: the file is not known to be on the disk.
- **Each step is logged with its time:**
  - `DOWNLOAD: FLUSHED IN t MS`;
  - `DOWNLOAD: READ BACK n OF total BYTES IN t MS (r MS IN READS, THE LONGEST l MS)` after each 8 MiB and at the end.

  The reads' share is apart from the hashing's, so a slow file service shows apart from a slow program or host.
- **No call of the check waits more than 60 s** (`FILE_MS`): one 16 KiB read, or a flush of the file service's 64-sector cache. A call without an answer ends the run with the step named:
  - `THE FILE SERVICE DID NOT FLUSH FILE IN 60 S`;
  - `THE FILE SERVICE DID NOT ANSWER A READ OF FILE AT n IN 60 S`.

  `fs::Error` does not tell a timeout from a missing service, so a call that failed after 60 s counts as one that got no answer.
- **The `net` suite** requires the flush line and the full read-back line in both runs of the 30 MiB file. It prints both runs' times as `TIME:` lines. The second run keeps its 300 s limit, and the SHA-256 check is unchanged.
- **The group is run again on i9**, to catch the step if the stall comes back. If the step is the file service or a block driver, that part goes to its owner: `KRN` for `vfs_server`, the owner of `ata`.
- docs/network/downloads.md 0.5 says all this.

## Acceptance criteria

- The `net` suite passes on x86 and aarch64 with the new lines in both runs, under the existing limits.
- No call of the check can wait more than 60 s. This is read from the code: the suite has no file service that stops answering.
- The group `x86: RAM above 4 GiB` is run again on i9, and its `TIME:` lines are recorded here:
  - if the stall comes back, the line before it names the step;
  - a pass is recorded as a pass of that configuration only. It does not erase H13's failure (MC-12.1).
- If a step is shown to be the cause, a regression for it is added, and the part outside `download` is routed to its owner.

What this does not show: the cause of H13's stall, until a run shows the step. Physical network acceptance on the two laptops stays separate.

## Related

- [351-NET-0029](../issues-done/351-NET-0029-download-calls-with-a-deadline.done): the same for the transfer's calls.
- [audit H13](../issues-audit/2026-10-16-download-recheck-stall.md).
