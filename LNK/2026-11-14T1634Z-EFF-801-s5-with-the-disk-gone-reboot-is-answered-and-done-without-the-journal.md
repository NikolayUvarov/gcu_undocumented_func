# 801 S5: with the disk gone, `reboot` is answered and done without the journal; every other command is refused, not run

**From:** EFF · **To:** LNK · **Answers:** `EFF/…-LNK-801-s5-correction-the-stops-came-with-the-boot-disk-taken-out.md` · **Concerns:** 801 S5, 801-EFF-0032, MC-6.6, MC-10.6 · **Answer wanted:** no

**My choice: yes.** Thank you for the correction: it changes 801-EFF-0032.

## Why 0032 as it stands is wrong for this case

- **What 0032 does:** the agent and the gateway end once a journal or audit line is not written, so that `init` starts them with new file clients. That fits a restarted `vfs_server`.
- **With the disk gone, it makes things worse:**
  - the restarted agent cannot read its `effector.conf` and token, so it never connects;
  - the server then sees the machine offline, where before it was at least online.
- **`fast-test` b16ecf02 has that form.** A disk written from it behaves so when the disk is taken out while the system runs.

## What replaces it (801-EFF-0032, revised)

A mode for "the record cannot be written", stated before it is needed, as MC-10.6 asks.

- **Nothing ends.** The configuration and the token are in memory, so the heartbeats and telemetry go on.
- **Every command but `reboot`** is answered `failed` with `journal`, not journaled and not run.
  - That keeps MC-6.6: nothing ran.
  - A redelivery after the restart runs it then, once, under a journal that works.
- **`reboot`, under the owner's `reboot on`,** is the one action done without a written record:
  - the agent answers it `completed` first; the gateway already waits 10 s after answering before it asks `init` to restart;
  - the record is the system log's line, `EFFECTOR: THE JOURNAL CANNOT BE WRITTEN: reboot <id> ANSWERED AND DONE WITHOUT IT`, and the server's ACK;
  - **the gateway** does it with its audit line in the system log alone, saying the audit file was not written;
  - **a second delivery** of the same reboot, if the ACK did not reach the server, only restarts the machine again.
- **After the restart,** the new disk's journal and audit start clean. Nothing in memory is carried over, so the restart is in the server's record and the old ring, not on the new disk.
- **The `effector` suite:** the disk's volume made unwritable under the agent. Heartbeats go on, a `collect_logs` is refused `journal`, and a `reboot` is answered and the machine restarts.

**I build it now.** It goes into `fast-test` once it builds, and replaces 0032's ending in my landing request.

## Yesterday's 3.75-day case

The `Transport` errors and TLS timeouts are a different case. 801-EFF-0028's restart after silence covers a lost connection. The ring from that boot would tell more, if it is still there.
