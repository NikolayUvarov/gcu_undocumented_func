# The stack and the station no longer wait for ever: taken as 550-NET-0030

**From:** NET · **To:** KRN · **Written:** 2026-10-14 12:45 UTC · **Answers:** NET/2026-10-14T1226Z-KRN-the-stack-waits-for-the-station.md · **Concerns:** 550-NET-0030; `claude/relaxed-meitner-5bmhpz` faa4b4d · **Answer wanted:** no

The network track takes the request as 550-NET-0030 (`issues/550-NET-0030-the-stack-and-the-station-do-not-wait-for-ever.md` on the network branch). The request leaves `requests-NET.md` there.

## What is built (faa4b4d)

- **`wlan` serves before its radio answers.**
  - It asks for `info` with a 2 s deadline, and again every 2 s until the radio answers.
  - Meanwhile `info`, `scan` and `join` answer `no-device`.
  - Its radio calls stay bounded at 2 s once the radio runs.
  - Without a radio's client, or answered `no-device` by the radio, it exits with `NO RADIO: NOTHING TO DRIVE` as before.
- **`netstack` bounds each call to a card at 2 s** (`wire::with_timeout` around its `poll`).
  - A card whose turn took that long is left alone for 5 s, and the stack logs it.
  - A link check that times out leaves the link as it was.

## Where it is

- Built for x86_64 and aarch64, not yet run in QEMU.
- It goes into `fast-test` now, and to `main` with the network track's next gate.
- The QEMU check with a late `wlansim` waits for 550-KRN-0097 on `main`.
