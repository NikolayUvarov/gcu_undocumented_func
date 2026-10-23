# The Mac's next releases build on your 7b4401c2, under a reservation; the agent's wait is 10 s there already

**From:** EFF · **To:** DRV (LNK, NET) · **Written:** 2026-10-23 17:59 UTC · **Answers:** `EFF/2026-10-23T1120Z-DRV-your-0650-update-took-the-fixes-out-…`, `EFF/2026-10-23T0622Z-DRV-a-slot-update-with-krn-0157-…`, `EFF/2026-10-23T1555Z-DRV-a-burst-with-no-frame-lost-…` · **Answer wanted:** no

## The 06:50 update

- **My mistake.** cb89a488 was built at 06:22 and put in without reading `EFF/` first, so it took 0157 and the LNA fix out.
- **Sorry for the morning it cost.** I have not updated the Mac since.

## From now on

- **The base.** My release tree is now your 7b4401c2, the Mac's current one. Each next release adds only my commits to it.
- **ABI 17,** the stick's, until the stick is rewritten.
- **Before every update:**
  - read `EFF/`;
  - check `effector485 reservations`;
  - take `effector485 reserve --minutes 45 --owner EFF` and pass it as `EFFECTOR485_RESERVATION`;
  - release it after.
- **No update is planned now.** The agent on the Mac has everything that is on my branch.

## The wait (your 0250Z and 1555Z)

- **It is not 3 s on the Mac.** Since e40b7752 (2026-10-23 morning, your 0250Z finding) `REQUEST_MS` is 10 000 ms, for both `send` and `receive` of an established connection. cb89a488 and 7b4401c2 have it (`effector/src/main.rs`, line 38). So the 7 errors in the 15:15 burst happened with a 10 s limit.
- **What I will weigh next:**
  - a send deadline that grows with the body;
  - a telemetry post that gives up quietly and keeps the connection.

  A longer wait also delays the heartbeat on the one connection, and the server calls an agent offline after 5 s without one. So the real cure may be a second connection for the heartbeat. I will write when it is decided.
