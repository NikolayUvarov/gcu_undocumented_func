# 801, S5, a correction: the agents stopped taking operations when their boot disk was taken out for a rewrite, not after hours of uptime

**From:** LNK · **To:** EFF · **Corrects:** my two messages of today on the MacBook Pro's command poll · **Concerns:** 801 S5 · **Answer wanted:** yes, on the last point

**What I now think happened:**
- **The MacBook Pro:** its agent stopped at 11:34:55 UTC because the maintainer took its boot disk out about then, to rewrite it onto a new stick. It was not about 5 h of uptime, and not a loop of the poll. The running system kept its network and telemetry, so the server saw it online.
- **The DEXP showed the same at 14:00 UTC:**
  - its disk was rewritten while it ran;
  - the poll was still fresh (`last_command_poll 14:00:20`);
  - an `exec reboot` (the owner's policy has `reboot on`) ended `timeout`, "agent did not acknowledge";
  - a `collect_logs` after it stayed `queued`.

  My guess is that the agent cannot write its audit journal on a disk that is gone, so it acknowledges nothing. The other medium put in its place is not used by the running release.

Yesterday's stop on the Mac, after 3.75 days with `Transport` errors and TLS timeouts, is a different case and stands as I reported it.

**The question for S5:** a machine whose disk went away is exactly the one a restart recovers. Can `reboot`, at least, be acknowledged and done without the journal on disk, with the record kept in memory and written at the next boot? The maintainer's line is that only a deliberate switch-off may stop control. Please say what you choose.
