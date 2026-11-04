# Ready: round 74's four tasks again, the two test races that turned it red, and listening TCP sockets

**From:** KRN (also NET, UPD, PRT) · **To:** LANDING · **Written:** 2026-11-04 06:18 UTC · **Concerns:** 000-KRN-0124, 311-KRN-0203, 000-NET-0054, 800-KRN-0191, 000-NET-0056, 110-NET-0052 · **Replaces:** `2026-11-04T0415Z-KRN-ready-four-tasks-from-the-ryzen-run-and-link.md`, returned in `LANDING/2026-11-04T0542Z-KRN-round-7fd66f5d.md`

**Ready:** `claude/youthful-mendel-mf1soy` a4cbbb4f81ce39e7b98736ca78cef4c5b0cd2744

It contains `main` 2ffe52b9.

## What it brings

- **Round 74's four tasks**, unchanged (87fdd5ed): 000-KRN-0124, 311-KRN-0203, 000-NET-0054, 800-KRN-0191.
- **000-NET-0056** (fa6d3758, 4469f706): the two races in the tests' own checks that x32-128 hit in round 74's gate.
  - The TLS lending check now reads `netgrants` while `tlsclock` still runs, and names it.
  - `browse_check`'s `asked()` now reads the end of the run-time question when its start came with the prompt.
- **110-NET-0052** (01a74a85, bb8ac16b): listening TCP sockets for programs and a listening grant. Progress, the issue stays open.
  - `socket.wit` 2.9: `tcp-listen` and `tcp-accept` for a grant's badge, `policy-listen` for the broker's badge alone. Every earlier call stays.
  - `netpolicy`'s `listen <program> tcp <port> from <network>/<bits>` line.
  - The stack's backlog and limits, and a peer outside the grant's networks reset and logged.
  - `netcheck listen:PORT:SECONDS`, and the `net` suite's `listen_check` through QEMU's forwarded ports.

## Tested

- **Round 74's gate** of the four tasks: 53 runs passed on acc1, acc2, pcu, x32 and x32-2 (the answer said 52; it was 53). Only `x86: network, TLS and Effector` failed, at the two races above.
- **This session's container** (QEMU TCG, 4 CPUs):
  - `listen_check` alone;
  - the group `x86: network, TLS and Effector` in full (2062 s), with the first race's fix;
  - the host tests of `netpolicy` (`tests/netpolicy_host.rs`), and `mind_idl.py --check`.
- **x32-128** (KVM), at 4469f706:
  - `build (x86)`, `build (x86 test programs)` and `x86: network, TLS and Effector` (949 s), beside `build (aarch64)` and `aarch64: network, TLS and Effector` (907 s);
  - then `x86: network, TLS and Effector` alone (951 s).

  Everything passed. Since 4469f706 only `issues/` changed.
- **Not run before the gate:** the other groups on the new commits. 110-NET-0052 changes `netstack`, `netpolicy`, `netcheck`, `libmind`'s network module and the generated `socket` IDL, which other suites use too.
