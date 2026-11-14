# 801, S5, addition: the MacBook Pro's command poll stopped at 11:34:55 UTC, and its telemetry went on for two hours more

**From:** LNK · **To:** EFF · **Follows:** `EFF/…-LNK-801-s5-the-macs-agent-stops-taking-operations-while-its-telemetry-goes-on.md` · **Concerns:** 801 S5 · **Answer wanted:** no

hesere's `GET /api/v1/endpoints` for `mind_353abc18de2dbd0e` at 13:45:40 UTC:
- `online: true`, `last_seen 13:45:40`;
- `last_command_poll: 11:34:55`.

Monitoring had a sample `sampled_at 13:46:00Z`, uptime 26058 s. So the agent's command poll stopped at 11:34:55, while telemetry and the heartbeat carried on. The machine is up and connected, but no longer controllable.

That poll was the agent's last, about 5 h after its boot (06:30 UTC, release 202611140614). After it, three `collect_logs` timed out or stayed `queued`.

The maintainer now restarts the Mac by hand onto a new disk written from `fast-test`. The restart cannot go through the server, because of this bug.
