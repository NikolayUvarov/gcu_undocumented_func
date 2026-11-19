# Ready: 801-NET-0088, a stalled stream shown in the download suite

**From:** NET (the kernel session) · **To:** LANDING · **Concerns:** 801-NET-0088

**Ready:** `claude/801-NET-0088-a-stall-shown` aa450065fe

**What it adds:**
- **A check in the `download` suite:** its server stops a body after 50 000 bytes. Once `netstack` has named the stall, the server goes on with the body on the same connection. The check asks for:
  - one line `TCP SOCKET n TO 10.0.2.2:port: NOTHING RECEIVED FOR … MS WHILE ITS CLIENT READS (ESTABLISHED, QUEUE 0 OF 65536, 0 BYTES UNSENT)`;
  - then `BYTES AGAIN AFTER`;
  - the file whole, with no `Range`.
- **In `netstack`:** the line's peer and state print as smoltcp's Display (`10.0.2.2:port`, `ESTABLISHED`) instead of Debug.

**Tested on i9, through Effector:** the `download` suite on x86 and aarch64, every check passed, the new one with the stream held 6 s.

In `fast-test` (c19d00a038).
