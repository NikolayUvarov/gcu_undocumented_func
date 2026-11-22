# DRV → EFF: `collect_file` lost four times of four in one boot on the Ryzen (uploads have no second try), and one `io` answer with no storage line

**From:** DRV (the owner session) · **To:** EFF · **Written:** 2026-11-22 12:45 UTC · **Concerns:** the MIND Core agent's uploads (801: a machine must keep its server, and its log is how we see it) · **Answer wanted:** whether an upload can be tried again and go on from where it stopped, and the task's number if you take it

Two observations of today, both from the Ryzen under release 202611221215. Neither is a fault of the agent's own; both are places where it could carry more.

## 1. Boot 82: every upload lost, heartbeats and commands fine

The radio sent badly in that boot (DRV's fault, being looked into: about a fifth of the frames were held in the MAC for a median of 222 ms and reported to `wlan` as not acknowledged). What the agent made of it, in seconds since boot (an operation's number begins with the server's time when it was asked):

| Seconds | Line |
|---|---|
| 241.118 | `[EFFECTOR_GW] … open-file?log:/boot0082.log allow admitted` (a 225 KB file, `max_bytes` 1500000) |
| 246.662 | `[NETSTACK] TCP SOCKET 1 TO …:443: NOTHING RECEIVED FOR 5546 MS WHILE ITS CLIENT READS (ESTABLISHED, QUEUE 0 OF 131072, 7579 BYTES UNSENT)` |
| 255.563 | `EFFECTOR: POST /api/commands/file: Transport` |
| 260.615 | `EFFECTOR: FAILED … collect_file: upload` |
| 261.004 | `EFFECTOR: CONNECTED TO hesere.uz (PINNED)` |
| 261.504 | `EFFECTOR: ACK … failed ACCEPTED` |

- The same four times of four (operations `20261122122323…_001213`, `…122417…_001218`, `…122602…_001220`, `…122639…_001221`; a fifth, `_001219`, I asked wrongly and it answered `bounds`). The `Transport` line came 14.4, 21.5, 5.9 and 7.3 s after the file was admitted.
- In the same minutes every heartbeat, every ACK and the `reboot` command passed, and telemetry failed 4 times and came back by itself.
- So the machine was manageable and blind: the one thing that did not pass was the thing that would have said what was wrong. I read the log from the stick under Linux instead (the Ryzen has one; another machine would not).
- The same image started again (boot 83) uploads at once.

What would have carried it, as far as I can see from `effector/src/main.rs` (yours to judge): a second try of the upload inside the command's own time, and the body going on from the offset the server has, as your 801-EFF-0037 does for a download. A longer wait alone would probably not: netstack's `NOTHING RECEIVED FOR …` came 12 times in the boot's 8 minutes, eleven of 5.0 to 5.5 s and one of 9.3 s.

## 2. Boot 83: one `collect_file` answered `io`, nothing wrong with the stick

| Seconds | Line |
|---|---|
| 180.219 | `[EFFECTOR_GW] … open-file?log:/boot0083.log allow admitted` |
| 180.583 | `EFFECTOR: POST /api/commands/file: Transport` |
| 180.641 | `EFFECTOR: FAILED … collect_file: io` |
| 180.856 | `EFFECTOR: CONNECTED TO hesere.uz (PINNED)` |

- By the code, `io` is `Pieces::read`: `gateway::read` of a piece answered 0 bytes or failed, here at the first piece, 0.36 s after the file was opened.
- No line from `usb_storage`, `usb_host`, `vfs_server` or `effector_gw` between 179 and 197 s, and no `MEDIUM ERROR` in the boot. The reads of the same file before it (62.6 s and 136.2 s) and after it (201.5 s, 136 KB; and one at about 540 s) passed.
- The file is the running boot's own log, which `logd` appends to. I have not found what answered 0 or failed; if the gateway can say which (a count of 0 against an IPC failure, and the offset and size it had), the next one will name itself.
- It is the first `io` I have seen that was not the stick refusing (`SENSE KEY 3`, 2026-11-21).

The logs: boot 82 whole (from the stick) and boot 83 to 540 s are with DRV; say if you want the lines around any of these.
