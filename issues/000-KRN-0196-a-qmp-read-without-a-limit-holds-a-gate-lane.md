# 000-KRN-0196 — A QMP read without a time limit holds a gate lane for an hour

**Type:** tests · **Owner:** `KRN` · **Priority:** P1 · **Status:** made (2026-10-31); closes once landed · **Blocked by:** — · **Main task:** — · **Roadmap:** the landing queue (AGENTS.md section 4) · **Constitution:** MC-12.2

## Problem

- **What happened.** In rounds 60 (b08d3a62) and 61 (077e0fb9), the group `x86: RAM above 4 GiB` on i9 gave no result:
  - the Effector operation ran out of time after about 58 minutes;
  - its lane ran nothing else meanwhile, and the group ran again on rz.
- **Where it stood.** On i9, during round 61:
  - `tests/qemu_smoke.py` was blocked in a read on a Unix socket (`unix_stream_read_generic`);
  - its QEMU, `no_serial_check`'s machine (213-KRN-0098: `-serial none`, seen only through QMP's `screendump`), had run for 45 minutes and was waiting in `poll`;
  - the check's `screen.ppm` was there.
- **Why it can wait for ever.** `no_serial_check` opens a new QMP connection every 2 s and reads it with no time limit:
  - QEMU serves one QMP client at a time;
  - the file `makefile()` made kept the earlier connection open after its `with` block, until the name was bound again;
  - a connection QEMU has not taken yet waits for its greeting with no end.
- **The same in `VM.qmp`.** The one connection a VM keeps has no time limit either: a QEMU that stops answering holds the suite.

## Plan

- **`no_serial_check`:**
  - each try closes its connection and its file;
  - it gives up after 10 s and counts as one of the 60 tries.
- **`VM.qmp`:** its socket has a time limit of 60 s, so a QEMU that stops answering fails the suite with `TimeoutError`.

## Acceptance criteria

- [x] `no_serial_check` closes each connection and bounds each try to 10 s; a socket error or a broken reply counts as a try.
- [x] `VM.qmp` reads with a time limit of 60 s.
- [ ] `x86: RAM above 4 GiB` (with `no_serial_check` in its `display` suite) passes locally (done) and on i9.
- [ ] The landing gate runs it with no hour-long lane.

## Progress

- **2026-10-31, made** on `claude/youthful-mendel-mf1soy`.
- **Tested locally (x86, QEMU TCG, 4 CPUs):** `x86: RAM above 4 GiB` PASS in 1119 s, with `no_serial_check`'s line.
- **Left:** a gate that runs the group on i9.

## Related

- 213-KRN-0098: the no-serial check.
- 311-KRN-0190: the request round 60 returned, for another reason.
