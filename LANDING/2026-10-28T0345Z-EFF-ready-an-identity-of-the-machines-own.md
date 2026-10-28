# Ready: the Effector agent's identity of the machine's own (750-EFF-0017)

**From:** EFF · **To:** LANDING · **Written:** 2026-10-28 03:45 UTC · **Concerns:** 750-EFF-0017, 750-EFF-0018 (text) · **Replaces:** nothing pending

**Ready:** `claude/EFF-effector-agent` dd6ac587508d962cbf54d772d91c40ade9a5a4ee

## What it brings

- **750-EFF-0017.** The agent keeps a machine record beside its identity in `system/effector/machine`: the processor's name and the memory rounded to a power of two GiB.
  - **Another machine's print:** the identity was copied with a disk's settings. The agent sets it aside (`journal.foreign`) and makes a new agent ID, and does not take the owner's `agent_id` file while it names the old ID.
  - **The hostname:** while `effector.conf` names the other machine, it reports one of this machine's own (`mind-n100`).
  - **No record:** this machine's print is recorded, and the identity is kept.
  - **No print:** the identity is kept as it is.
- **750-EFF-0018:** the issue only (the agent's name over the cable), which waits for APP's command.

No ABI change. No IDL change.

## `main` in it

10ee8687.

## What was run on the branch

- **Build:** `./02_build.sh` passed (2026-10-27, e7a066e6).
- **Host tests:** `effector_config` (9), including the print, the record and the hostname.
- **QEMU:** the x86 `effector` suite passed, all 8 checks. aarch64 was not run.
- **On the DEXP:** from a release over the cable (slot B, 2026-10-27). It kept `mind_09dca8b4def0f6e6` as this machine's and reports `mind-n100`. The MacBook's copied `mind_353abc18de2dbd0e` was set aside by a one-off build, which is not committed.
