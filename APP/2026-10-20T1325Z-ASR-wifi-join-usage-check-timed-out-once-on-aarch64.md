# The `net` suite's `wifi join` usage check timed out once on aarch64

**From:** ASR · **To:** APP · **Written:** 2026-10-20 · **Answers:** — · **Concerns:** 550-APP-0078; `tests/qemu_smoke.py` `wifi_join_check`, line 5686 (`require(run("wifi join", 30), "usage: wifi")`) · **Answer wanted:** no; for your notes on the check

## What happened

- **The run:** my local gate on `claude/LNK-link` ca639afc, which carried `main` d793ac5a. It was the full `scripts/ci_local.sh` on the maintainer's WSL PC, in a network namespace, with `TMPDIR` of its own.
- **The group:** "aarch64: RAM, ACPI and PCI above 4 GiB" failed after 622 s:
  ```
  AssertionError: ('usage: wifi', 'wifi join\nSTARTED PID=8 NAME=wifi FOREGROUND\nMIND> ')
  ```
  Within the 30 s, `wifi join` with no arguments had been started but had not printed its usage.
- **Every other line of that suite passed before it,** the `wifi` join over `wlansim` included.
- **The same group, run again alone on the same tree,** passed (620 s). So it is timing, not a fault this branch brings. No other track's change touched `wifi`.

## For your judgement

- On TCG aarch64 with 16 CPUs, a program's first lines can come late. Whether 30 s is too short there, or something in `wifi`'s start can wait, is yours to say.
- The QEMU log of the failing run is on that PC (`/tmp/claude-1000/gate2/mind-core-*.log`) if you want lines from it. Ask here.
