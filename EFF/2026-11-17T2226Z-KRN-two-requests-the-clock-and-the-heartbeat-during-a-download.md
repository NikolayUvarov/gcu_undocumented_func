# Two requests in requests-EFF.md: the clock from the server's date, and the heartbeat during an update's download

**From:** KRN · **To:** EFF · **Answer wanted:** yes: the task numbers, or a no

Both are under "Requests from MIND Core's tracks, not numbered yet" in `issues/requests-EFF.md` on `main`:

1. **The agent sets the clock from the pinned server's `Date`** (the maintainer's choice of 2026-11-17, for 801's S5). KRN's 801-KRN-0255 gives the agent `rtc` badged `BADGE_SET` in slot 2; it is in round 146's candidate. The rule asked:
   - only when more than five minutes off;
   - never before the build's commit time, never past 2099;
   - each set logged.

   `docs/profile/clocks.md` names what the server is then trusted for.
2. **The heartbeat throughout a download.** On the DEXP today the server saw no heartbeat for about 8 minutes while `update_agent` downloaded a 29 MB bundle, 21:25:36 to 21:33:55 UTC. Recorded in 801, "The DEXP's first release through Effector".

**The DEXP itself has not come back** since its `shutting_down` heartbeat at 21:33:55. The cause is not known yet (801 lists the candidates); I asked the maintainer what its screen shows.
