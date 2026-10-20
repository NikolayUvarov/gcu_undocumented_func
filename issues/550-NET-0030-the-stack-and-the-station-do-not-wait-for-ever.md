# 550-NET-0030 — The stack and the station do not wait for ever on a card or a radio

**Type:** network (services) · **Owner:** `NET` track (worked by the storage session at the maintainer's request) · **Priority:** P1 · **Status:** in progress (built; the QEMU check written, 2026-10-20) · **Blocked by:** — (550-KRN-0097, `init` starting `wlan`, is on `main`) · **Main task:** [550](550-network-on-real-hardware.md) · **Roadmap:** track D · **Constitution:** MC-5.2, MC-6.1, Appendix B.6

From the request "The stack waits for the station, and the station for its radio, with no deadline" in requests-NET.md (recorded by the kernel track, 2026-10-14, from reading `fast-test` fee095e).

## Problem

- **`wlan` asked its radio for `info` first, with no deadline**, and served its own endpoint only after the radio answered.
- **`bcm_wifi` answers only once the radio is up:** after its microcode and stage 3, and after its whole test when `data/wifitest.txt` is on the disk.
- **`netstack` called each card from its `poll` with no deadline**, and its loop does nothing else while a call waits.
- **So on the MacBook Pro** every socket call waited until `bcm_wifi` was up, and a radio alive but silent would hold the stack for ever. A radio or station that exits is not the problem: the kernel fails waiting callers with `ERR_PEER`.

## Plan

- **`wlan` serves before its radio answers.**
  - It asks the radio for `info` with a 2 s deadline, and again every 2 s until it answers.
  - Meanwhile it is a card without a radio: `info` answers `no-device`, as before a join; `scan` and `join` answer `no-device`.
  - It logs `THE RADIO HAS NOT ANSWERED IN 2 S: SERVING WITHOUT IT, ASKING AGAIN` once.
  - Its calls to the radio while it runs are bounded too.
  - Started without a radio's client, or answered `no-device` by its radio, it exits with `NO RADIO: NOTHING TO DRIVE` as before.
- **`netstack` bounds every call to a card at 2 s.**
  - A card whose turn took that long is left alone for 5 s and logged: `CARD n DID NOT ANSWER IN 2 S: LEFT ALONE FOR 5 S`.
  - A link check (`info`) that gets no answer leaves the link as it was, rather than dropping its address and connections.
  - Its replies to its own clients are not calls, and are not bounded.
- **A QEMU check**, once `init` starts `wlan` on `main`: a `wlansim` that answers late (a fixture flag), with the wired card's sockets working meanwhile.

## Acceptance criteria

- With a radio that answers late, the wired card's sockets work at once, and the station joins once the radio answers.
- A card that stops answering costs the stack at most 2 s every 5 s, and the other cards keep working.

## Progress (2026-10-14)

Built for x86_64 and aarch64 (`wlan/src/main.rs`, `netstack/src/main.rs`). The QEMU check waits for 550-KRN-0097 on `main`.

## Progress (2026-10-20): the QEMU check

- **A slow radio.** `tests/wlansim` built with the test-only feature `slow` (`scripts/x86_fixtures.sh`):
  - it answers nothing for its first 60 s (`[WLANSIM] SILENT FOR 60 S: A RADIO THAT ANSWERS LATE`);
  - it answers nothing for 20 s after the station asks (ARP) for 10.0.4.99;
  - the default build has the same loop, never silent.
- **The check** (`slow_radio_check`, the x86 `net` suite) boots a raw image with that radio and a VirtIO wired card:
  - while the radio is silent: DHCP on the wired card, `ping 10.0.2.2` 3 of 3, each within 2 s, and a page fetched over TCP from the host; `wlan` logs that the radio has not answered;
  - once the radio answers: `wifi` scans (the station's last scan was made while the radio was silent), `wifi join` joins MIND-TEST, and the stack gets 10.0.4.15 by DHCP on the station;
  - with the joined station's radio silent (`ping 10.0.4.99`): `ping 10.0.2.2` 3 of 3, each within 2 s, a page fetched over TCP, and no task faults.
- **Run locally** (x86, QEMU 8.2, TCG, 4 CPUs): it passed. The wired pings took at most 24 ms while the radio was silent, and at most 21 ms with the joined station's radio silent.
- **Gate 22** (i9, `6228d50e`, x86 `net` suite): it passed. The wired pings took at most 26 ms while the radio was silent, and at most 23 ms with the joined station's radio silent. In `main` since then (the merge `2123985a`).
- **A TCP connection added** (2026-10-20, after gate 22): the criterion speaks of sockets, so the check fetches a page from the host's web server in both phases. Run locally: it passed, pings within 28 and 24 ms.
- **What it does not measure.** The stack's own bound, at most 2 s of waiting every 5 s for a card that does not answer, is the budget code of 550-NET-0032, which its host tests exercise (`tests/network_budget`). This check shows the effect on the other card, not the bound.

## Related

[550-NET-0018](550-NET-0018-the-station-service.md), [550-DRV-0027](550-DRV-0027-bcm4331-frame-service.md), [351-NET-0029](../issues-done/351-NET-0029-download-calls-with-a-deadline.done) (the same bound for `download`).
