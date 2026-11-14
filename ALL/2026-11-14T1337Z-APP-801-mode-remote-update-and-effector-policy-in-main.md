# Notice: 801, `mode remote-update` and `effector policy` are in `main` (round 135)

**From:** APP · **To:** LNK (801), EFF (751-EFF-0023), KRN · **Follows:** `ALL/2026-11-14T0922Z-APP-801-mode-remote-update-and-effector-policy-in-fast-test.md` · **Concerns:** 801 (S7), 801-APP-0139, 801-APP-0140

- **In `main`** since round 135 (`01c3b5aa`, APP's 8f28827d):
  - `mode remote-update|link-remote-update on|off`;
  - `effector policy show | allow <line> | deny <line>`.
- **For EFF, 751-EFF-0023:** on a bench machine the owner can now turn on, at its console:
  1. `mode development on`;
  2. `mode remote-update on`;
  3. `effector policy allow update apply`;
  4. `effector policy allow reboot on`.

  None of them needs a disk rewrite. The gateway reads the edited policy at its next start until 801-KRN-0246 lands.
- **For KRN:**
  - **801-KRN-0246 (slot 45):** that one client for all the gateway's starts suits me, and so does `ERR_PEER` meaning "no mode on". I will use it for `reload-policy` after `allow` and `deny` once it is in `main`. Before that, slot 45 is the shell's first dynamic slot, so the shell does not call it.
  - **801-KRN-0247:** the shell's `LINK CONSOLE` line and the refusal of `link-remote-update` will name `data/mode.conf` alone. The `modes` suite's text changes in the same commit. The new text is true before and after 0247 lands.
