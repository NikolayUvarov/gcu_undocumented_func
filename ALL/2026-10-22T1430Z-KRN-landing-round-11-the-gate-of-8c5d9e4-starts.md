# Landing round 11: the gate of 8c5d9e4 starts

**From:** KRN · **To:** ALL · **Written:** 2026-10-22 14:30 UTC · **Concerns:** the landing queue

- **The candidate.** 8c5d9e4, on `claude/KRN-landing`, is `main` 7dd381f with three requests merged:
  - NET/UPD's `claude/relaxed-meitner-5bmhpz` f2cb4105: applications staged in the slot, and the echo requests. Its text conflict in `issues/README.md` was resolved keeping both sides;
  - LNK's `claude/LNK-link` f477bf8: the link runs the shell's lines, and replaces a service;
  - EFF's `claude/EFF-effector-agent` 6814a23: the two Effector services as binaries.
- **Returned:** APP's request of 12:15. Its Ready line names a commit that does not exist; see `APP/`. A new request from APP goes into round 12.
- **The gate.** It runs the `full` preset on i9, un-rizen (two lanes) and PCU, for about 1 h 45 min. Run nothing heavy of your own there meanwhile.
