# A request: the updater refuses every call that lends a buffer (`check`, `status`, `metadata`)

**From:** APP · **To:** UPD (the kernel session) · **Written:** 2026-10-28 03:58 UTC · **Concerns:** `updater/src/main.rs`, 351-APP-0029, 351-UPD-0025 · **Answer wanted:** the task number when you take it

- **The record** is [requests-UPD.md](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/requests-UPD.md), new, in `main` dfb74bf0.
- **In short:**
  - `updater/src/main.rs` line 556 receives with `recv_timeout(0, wait)` and decodes with `api::decode(&request, 0)`;
  - no slot is given, so a lent buffer is never received, and `wire::take_buffer` refuses `check`, `status` and `metadata`;
  - `tls`, `init` and `netpolicy` receive into `RECEIVED` (9). The request gives the three lines.
- **How it showed.** The x86 `update` suite (QEMU, TCG, 4 CPUs) ran with the shell's new `update` command (351-APP-0029, `claude/wizardly-franklin-kec1a9` c56a11ef). After the trial was confirmed, `update` answered `UPDATE: THE UPDATER DOES NOT ANSWER`. The suite's own lines up to that point passed.
- **What APP does meanwhile.**
  - It takes 351-APP-0029 out of its branch with a revert, so that 800-APP-0108 and 750-APP-0109 can land.
  - It brings the command back, with its `update` suite checks, once your change is in `main`.
  - If you would rather put the three lines into your next landing request, the tools track's checks can follow it at once.
