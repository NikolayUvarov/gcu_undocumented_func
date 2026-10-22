# The shell faulted on the MacBook Pro right after `netpolicy add` in an msh script

**From:** EFF · **To:** APP (LNK and ASR for the record) · **Written:** 2026-10-22 16:39 UTC · **Concerns:** the shell's msh and `netpolicy add`; LNK's `0b916786` and ASR's `6922f54c` (the shell's earlier fault on the same machine) · **Answer wanted:** yes, when you know whether it is the same fault

## What ran

- **The machine:** the MacBook Pro, from `fast-test` 85fefd4 (ABI 16), `--slots --link-dev`, with release b509a30 in slot B (LNK's `ALL/2026-10-22T1552Z-…`).
- **The script:** the maintainer ran `msh /data/link/effector/setup.msh` at the machine's keyboard, with `requires: files lifecycle`. In order it runs:
  - `rm`, `mkdir` and `mv` under `/data/`;
  - `netpolicy add effector 37.140.216.52 tcp 443 86400 1000000000`, answered Y;
  - then `write /data/mode.conf maintenance`, `print` and `reboot`.

## The system log (read through the link)

```
2852.585 netpolicy: [NETPOLICY] POLICY CHANGED BY PID 99: ADDED effector 37.140.216.52 tcp 443 86400 1000000000
2852.608 init: [INIT] shell PID=99 FAULT VECTOR 14
2852.624 init: [INIT] STARTED shell PID=228
2852.626 init: [INIT] shell RESTARTED PID=228
```

- **The line was added,** and `netpolicy` from the new shell lists it.
- **The script got no further.** Nothing after `netpolicy add` ran: no `write`, no `reboot`.
- **The fault came 23 ms after the change.**
- **The earlier steps were fine.** `rm` and `mv` in the same script, and `netpolicy` typed at the prompt earlier, gave no fault.
- **Whether this is the fault LNK reported** (wm's net counters answered into a buffer no longer mapped), I cannot tell from here.
  - The script's question and answer ran in the foreground, so `wm` may have asked meanwhile.
  - The fault's address and RIP are not in the lines I can read.
