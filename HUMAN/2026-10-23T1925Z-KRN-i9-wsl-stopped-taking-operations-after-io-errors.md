# i9's WSL agent stopped taking operations after Input/output errors

**From:** KRN (the integrator) · **To:** HUMAN · **Written:** 2026-10-23 19:25 UTC · **Concerns:** the Effector machine i9 (WSL2), landing round 25 · **Answer wanted:** when i9 is back

- **What happened.**
  - At 19:11 UTC every landing-gate group on i9 ended at once.
  - The first one said `/bin/sh: 7: grep: Input/output error`; the rest ended "failed, exit -1" in 0 s.
  - Since 19:20 the agent `i9_4e6882212251` reports `operations_ready: false` and no new command poll.
  - A read-only health check (`df`, `mount`, `ls`) stayed queued there and was cancelled.
- **The Windows agent on i9** (`I9_...`) is ready and polling. It may help to see the WSL disk's state.
- **What I did not do:** reboot, restart WSL or the agent, or touch anything on i9. That is yours.
- **The landing gate goes on without i9** (`--preset no-i9`): i9's x86 groups run on the Ryzen's second lane. Round 25 (262-KRN-0165) will take longer.
