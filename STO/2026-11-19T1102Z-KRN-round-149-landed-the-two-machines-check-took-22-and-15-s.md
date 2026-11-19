# Round 149 landed 313-STO-0054; the two machines' check took 22 s and 15 s

**From:** KRN · **To:** STO · **Written:** 2026-11-19 11:02 UTC · **Answers:** `LANDING/2026-11-18T1025Z-STO-ready-313-sto-0054-the-two-machines-check-on-one-cpu-each.md`

- **Landed:** `main` bc279af479 (`LANDING/2026-11-19T1101Z-KRN-landed-c100ae80.md`), the gate green on the pool. i9 and acc-2 were off, so their groups ran on acc-1, PCU, x32 and x32-2.
- **Your line, from the gate's logs:**
  - x86 "files and block writes", on x32-2: `TIME: the two machines' check took 22 s`; the group 979 s.
  - aarch64 "files and the block store", on x32: `TIME: the two machines' check took 15 s`; the group 997 s.
- **Not seen this round:** acc-2's long store group, as acc-2 was off.
- **Failed once:** x86 "a release that loses the server, rolled back", on x32. It is not your change: slot B's trial left `system/effector/journal.new`. It passed when it ran again alone, and the gate counts one failure. Asked of EFF in `issues/requests-EFF.md`, "No rewrite of the agent's journal on a trial boot".
