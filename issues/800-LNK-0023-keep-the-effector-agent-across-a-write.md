# 800-LNK-0023 — `build_and_write.sh --keep-effector`: the Effector agent's files kept across a write

**Type:** tool · **Owner:** `LNK` · **Priority:** P1 · **Status:** in progress (2026-10-24) · **Blocked by:** — · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-12.6

## Problem

EFF's request (`LNK/2026-10-24T0811Z-EFF-…build-and-write-keeps-the-effector-agents-files.md`, 5bb717b0):
- **The agent stays down after each new stick.** The MacBook's sticks of 2026-10-23 and 2026-10-24 came up in the maintenance mode, but the Effector agent did not start.
- **The cause.** The write wipes `data/effector/` (`effector.conf`, the owner's `policy`, `agent_id`) and `system/effector/` (the agent's ID, journal and token). `effector` exits for want of its configuration, and `init` quarantines it.
- **The owner's step.** Only the owner's shell writes those trees on the machine, and the link writes only `data/link/`. So after each write the owner ran `start.msh` at the keyboard, and the server saw a new agent that needed activation.

## Plan and what was done

- **`--keep-effector`** (`scripts/build_and_write.sh`), as `--keep-wifi` keeps keystore's files:
  1. **Before the write,** it saves `data/effector/` and `system/effector/` from the old disk's volume `MIND CORE` into `~/.config/mind/effector/{data,system}`, the directories 700 and the files 600.
     - The copy is made whole into `….next` and only then takes the place of the old save.
     - A disk with no agent leaves the last save as it is, and a fresh disk uses it.
  2. **After the write,** it puts them back on the new volume, and checks each saved file against the disk by `cmp`.
  3. **The answers waiting for their ACK** (`artifact`, `result`, `bench`) are kept. EFF says that is safe: the journal names their command.
- **The owner decides once:** the saved files come from the owner's own disk, and only the owner's write puts them on the next one (MC-12.6).
- **Docs:** the script's header, and `docs/write-disk.md` with its Russian version.
- **Checked on the host:** the copy and the read-back helpers, on directories:
  - 6 files saved with 700/600;
  - put back and compared, beside a file the image ships;
  - a volume with no agent left the save as it was.

## Acceptance criteria

- [x] The helpers save, put back and read back the agent's files, and keep the last save when the disk holds none (host check above).
- [ ] On the MacBook Pro, a stick written with `--keep-effector --mode=maintenance` brings the agent up as the same `mind_…` endpoint, with no owner step and no new activation.

The saved files may hold the agent's token: never printed, logged or committed.

## Progress

- **2026-10-24, the first write with `--keep-effector` kept nothing.** The maintainer inserts the disk only when the writer's window asks for it, and both saves ran before that. They found no volume `MIND CORE` on disk 4 and went on quietly.
  - `--keep-wifi` has the same flaw, but its earlier save was still good, so the Wi-Fi keys came back.
  - The agent's three files were put on the new volume by hand, from EFF's kit (`data/effector/{effector.conf, policy, agent_id}`, as `start.msh` puts them), and read back.
- **The fix.** Before the saves, the script looks for the old disk's `MIND CORE` once. If it is not there, it asks for the disk and waits up to `OLD_DISK_WAIT` (600 s); Enter goes on without it.
  - When nothing was saved before either, `--keep-effector` now says loudly that the agent is not kept.
  - Checked on the host with a stand-in for Windows: the disk found on the third poll, and the wait ended at its time when it never came.

## Related

[800-LNK-0014](800-LNK-0014-build-and-write-in-the-repository.md), 750-EFF-0012, 750-EFF-0013.
