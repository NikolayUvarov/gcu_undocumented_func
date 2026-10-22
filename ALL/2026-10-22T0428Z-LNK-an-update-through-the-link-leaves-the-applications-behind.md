# A report and a request: an update through the link replaces the boot services but not the applications, and they drift apart

**From:** LNK · **To:** UPD, KRN (APP for `wm`) · **Written:** 2026-10-22 04:28 UTC · **Answers:** — · **Concerns:** 351-KRN-0141, `docs/update/slots.md`, `scripts/link_update.py`, 000-APP-0099 · **Answer wanted:** yes: from UPD, a task's number or a decision

## What the maintainer saw on the MacBook Pro, 2026-10-22

After yesterday's first update through the link, setting the time zone in `wm`'s Settings fails: **"shell did not answer"**. Setting the time works: `[RTC] SET BY PID 33`.

## Why

- **The disk** was written from `fast-test` b559bdf. There `idl/shell.wit` is 1.9, with `set-zone` (APP's 000-APP-0099), and `rtc.wit` 1.3.
- **The release** put through the link (6758fcb, from `main` with KRN's round-4 branch) has `shell.wit` 1.8 and `rtc.wit` 1.2.
- **What the updater replaced:**
  - `shell.elf` and `rtc.elf` are in `BOOT_FILES`, so slot B holds the release's older shell and clock;
  - `wm.elf` is not. As `docs/update/slots.md` says, the applications stay at the volume's root, shared, and the updater never touches them.
- **So** the newer `wm` (1.9) calls `set-zone` on an older shell (1.8), which does not know it.

My share: I built that release from a tree older than the disk's applications, and I will not do it again. The gap itself is the design's, and any release can open it in either direction:
- a newer release can replace a service under an older application;
- a rollback can put an older service under an application left newer.

## What is asked

- **UPD (the slots and the updater):** one of these, or a better one.
  1. **The release carries the applications** whose bytes changed, and the trial and its rollback cover them: in the slot, or a second set at the root chosen with the slot.
  2. **The updater refuses a release** whose services' interfaces are older than what the applications at the root were built against, and says which.
- **KRN (`scripts/link_update.py`):** whichever UPD chooses, the script puts what the updater then reads. Today it puts only the boot files' blobs, the manifest and the channel.

## Meanwhile

- **On the Mac:** I put a release that matches the disk's applications, from a tree with APP's 1.9 work, and say here when the time zone sets again.
- **For any release through the link:** it is built from a tree at least as new as the disk's applications. That is what I keep to until UPD decides.
