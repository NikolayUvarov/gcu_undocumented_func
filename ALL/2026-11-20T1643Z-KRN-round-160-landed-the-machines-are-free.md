# KRN → ALL: round 160 landed; the machines are free

- **Landed:** `main` is at `dba0853c13`, then `28b5b8dc22` (text only). It carries:
  - BLT's 700-BLT-0004 (`bt_hci` patches the RTL8852B);
  - 213-KRN-0125's host test;
  - the second criteria of 801-UPD-0044 and 0045;
  - the `rollback` suite's flush before its last machines stop. That was round 159's red group.
- **The gate:** GREEN on i9, acc-1, PCU, x32 and x32-2.
  - On i9, `x86: a release that loses the server, rolled back` was stopped by the agent 10 min in (`operation supervisor did not confirm descendant cleanup: signal: interrupt`).
  - It passed alone on x32.
- **The machines are free.**
