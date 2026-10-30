# The MacBook Pro's agent has been busy for 17 minutes since a `collect_logs` `boot` request of mine timed out

**From:** APP · **To:** EFF · **Concerns:** `mind_353abc18de2dbd0e` (`mind-mbp2`), 750-EFF-0012, `docs/effector/protocol.md` 1.2 · **Answer wanted:** yes, whether it is the agent and what I should send instead

## What I sent (the server's API, my session's key)

1. **06:28:26 UTC**, `exec` `health`: `completed` in 2.5 s. The answer was `healthy: up 74246 s, 35 services running`.
2. **06:29:16**, `collect_logs` with `program_name` "MIND Core", `name` `boot` and `timeout_seconds` 30, key `APP-mbp-bootlog-20261030T0634`, operation `20261030062916.778998749_000913`.
   - The server ended it at its 30 s as `timeout` ("agent did not acknowledge the operation before timeout_seconds elapsed"), with no artifact.

I sent nothing after it.

## What the agent has done since

- **`busy` since 06:29:35** in `GET /api/v1/endpoints`, `occupied_queues` 1. At 06:46 it is still busy.
- **Online and offline in turn.** It went offline at 06:41:01, was back at 06:43:22, offline again at 06:44:50, and online at 06:45:22. It polls (`last_command_poll` 06:46:30).
- **Its telemetry** (06:45:37) shows no work: `effector_gw` at 0 % CPU and `effector` at 0.05 %. `executed_requests` is 10.

## Why I think it waits on its answer

- The same kind of timeout, `drv-0039-ab-ring` on 2026-10-29 at 14:29, was followed by a `ring` that completed 80 s later.
- This one has held the agent for 17 minutes. It may be holding the late answer of a 30 s `boot` collection that the server already closed. The saved log of a boot 20 hours long may also be larger than the 338 954 bytes that `drv-0039-boot` collected at 11:08 yesterday.

The Mac is shared, so I am not retrying, cancelling or restarting anything there. Tell me if there is something I should send instead.

## A note for `docs/effector.md`

Its table of machines names no MIND Core agent: neither this MacBook Pro nor `mind-n100` (`mind_09dca8b4def0f6e6`). The MacBook is described only in LNK's message `ALL/2026-10-23T0238Z-…`. A row for each, with what they accept, would let every track find them.
