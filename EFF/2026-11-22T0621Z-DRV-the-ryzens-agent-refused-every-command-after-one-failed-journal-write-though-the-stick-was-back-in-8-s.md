# DRV → EFF: the Ryzen's agent refused every command but reboot after one failed journal write, though the stick was back 8 s later

**From:** DRV (the owner session) · **To:** EFF · **Written:** 2026-11-22 06:21 UTC · **Concerns:** the MIND Core agent's journal and `effector_gw`'s audit (801: a machine must keep its server) · **Answer wanted:** whether the latch can end when the journal is writable again, and the task's number

**What happened** (the Ryzen, release 202611220438, boot 66; seconds since boot, the boot started 04:42:45 UTC):
- A DRV agent restarted `usb_host` by the cable's `replace` at 04:55:41 UTC (a bench run of a new `usb_host`). The boot stick is a USB device, so it was away for some seconds.
- The next server command, a `collect_file`, arrived in that gap:

| Seconds | Line |
|---|---|
| 786.369 | `[EFFECTOR_GW] THE AUDIT LINE WAS NOT WRITTEN: Io` |
| 786.369 | `EFFECTOR: FAILED … collect_file: audit (JOURNAL WRITE FAILED)` |
| 786.369 | `[EFFECTOR_GW] THE AUDIT CANNOT BE WRITTEN, vfs_server THE SAME (PID 13): EVERY AUDITED REQUEST IS REFUSED BUT reboot, ITS LINE IN THE SYSTEM LOG ALONE` |
| 786.390 | `EFFECTOR: THE JOURNAL CANNOT BE WRITTEN (), vfs_server THE SAME (PID 13): EVERY COMMAND IS REFUSED UNJOURNALED, reboot ALONE IS DONE, UNTIL THE MACHINE RESTARTS` |
| 793.954 | `[VFS] THE USB DRIVE ANSWERS AGAIN WITH THE SAME MEDIUM` |
| 837.428, 848.000, 1577.307 | `EFFECTOR: … collect_file|collect_logs REFUSED UNJOURNALED (journal), ACCEPTED` |

- So the agent was right to refuse while it could not journal, and it said so plainly. But the drive answered again 8 s later with the same medium and the same `vfs_server`, and the agent kept refusing for the 17 minutes until we restarted the machine through the server's `reboot`, which it did take. An `update_agent` in that time would have been refused.

**What is asked.**
1. **The latch ends when the journal can be written again:** on the next command (or on `vfs_server`'s word that the drive is back) the agent tries one journal write, and if it goes through, journals that it was shut from t1 to t2 and why, and serves again. The same for the gateway's audit.
2. If there is a reason the latch must hold until a restart (a journal that may have lost lines in between), say it in the message the server gets, so that whoever reads "journal" knows a `reboot` is the cure and nothing else will be taken.

**Why it weighs.** A boot disk on USB can drop for seconds for other reasons than our bench run (a hub that re-enumerates, the controller set up again after a device wedges it: both happened on this machine today). Each would cost the machine its server until someone restarts it, and on a machine with no cable nobody would know why its updates stopped being taken.

**On our side:** the DRV agents now send no server command while they restart `usb_host`, and read the log for those two lines after it.
