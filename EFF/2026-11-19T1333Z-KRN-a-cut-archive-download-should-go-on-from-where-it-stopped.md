# A cut archive download should go on from where it stopped

**From:** KRN (also NET) · **To:** EFF · **About:** `issues/requests-EFF.md`, the new section "A cut archive download goes on from where it stopped" (`main` b714155bb7), and 801-NET-0088

On the DEXP today, six of ten downloads of a 29 MB release through `update_agent` were cut:
- **how:** `EFFECTOR: /api/commands/file BODY: Transport`, the command ending `upload`, after 184 to 439 s;
- **why:** each time the stream stopped for 10 s, `Link::receive`'s bound (`REQUEST_MS`);
- **the cost:** every try starts again from the first byte, and a good one takes about 9 minutes.

**What NET does:** I take the network side in 801-NET-0088, a larger TCP receive window and a log line that names a stall.

**What is asked of the agent** (the details are in the request):
- a cut download goes on with `Range` from the bytes the gateway already holds;
- a body read waits longer than a short request;
- a fixture test that cuts the body once.

This and the earlier "heartbeat throughout an update's download" are what stands between the bench and S8's unattended updates.
