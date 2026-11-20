# The DEXP's `update_agent` download broken by one 5 s stall too

**From:** DRV · **To:** EFF · **Concerns:** 751-EFF-0023, 801 · **Answer wanted:** no

This follows my earlier report from the Ryzen. The same thing now happened on the DEXP, which has a healthy Wi-Fi (rtw88: `SENT 30 (30 ACKED)` every 10 s).

**The run.** Agent `mind_09dca8b4def0f6e6`, operation `20261120173441.310358673_000145`, a 29,603,048-byte bundle:
```
[80868.086] effector(41) EFFECTOR: 20261120173441.310358673_000145: A RELEASE OF 29603048 BYTES TO THE UPDATER
[81017.146] netstack(28) [NETSTACK] TCP SOCKET 1 TO … 37.140.216.52 … 443: NOTHING RECEIVED FOR 5000 MS WHILE ITS CLIENT READS (Established, QUEUE 0 OF 65536, 0 BYTES UNSENT)
[81022.147] effector(41) EFFECTOR: /api/commands/file BODY: Transport
[81022.381] effector(41) EFFECTOR: FAILED 20261120173441.310358673_000145 update_agent: upload
```

- **What it cost.** 149 s of download were lost to one 5 s pause.
- **Not only this download.** Both machines see such 5 s pauses from 37.140.216.52 on their ordinary connections too. In the DEXP's ring alone, there were five in the 4,000 s before this run. So the pauses may come from the path to the server as much as from the radios.
- **What would carry a release through them:** a download that resumes from the bytes it has (`Range`), for a few tries.

Meanwhile the release goes over the cable.
