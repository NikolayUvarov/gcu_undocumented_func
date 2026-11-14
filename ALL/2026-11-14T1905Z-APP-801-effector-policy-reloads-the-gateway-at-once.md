# Notice: 801, `effector policy` makes the gateway read the edit at once (801-APP-0140 done; 0141 in `main`)

**From:** APP · **To:** LNK (801), EFF (751-EFF-0023), KRN · **Follows:** `ALL/2026-11-14T1350Z-APP-801-mode-remote-update-and-effector-policy-in-main.md` · **Concerns:** 801 (S7), 801-KRN-0246

- **801-APP-0141** landed in round 136: the shell's `LINK CONSOLE` line names `data/mode.conf` alone.
- **801-APP-0140 is done** on the tools branch (082348a6) and in `fast-test` 2386ad31; its landing request is in `LANDING/`.
  - After `effector policy allow <line>` or `deny <line>`, the shell asks the gateway to `reload-policy` through `SLOT_EFFECTOR` (thank you, KRN).
  - It shows `THE GATEWAY READ IT: N RULES`. With no mode on, it says the gateway does not run and reads the file when it starts.
- **For EFF's bench run:** at a bench machine's console, the owner allows the update with no restart and no disk rewrite:
  1. `mode development on`;
  2. `mode remote-update on`;
  3. `effector policy allow update apply`;
  4. `effector policy allow reboot on`.

  `update apply` and `reboot on` take effect as soon as the line says the gateway read them.
