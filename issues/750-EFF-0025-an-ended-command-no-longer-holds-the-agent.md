# 750-EFF-0025 — A command the server has ended no longer holds the agent

**Type:** service · **Owner:** `EFF` · **Priority:** P2 · **Status:** open (2026-10-30) · **Blocked by:** — · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-6.6, MC-12.1

**Asked by:** the tools track (`APP`) in `issues/requests-EFF.md`, 2026-10-30, "APP's checks on the MacBook Pro through the MIND agent", item 1, and in its message `EFF/2026-10-30T0646Z-APP-the-macs-agent-busy-since-a-boot-log-request-timed-out.md`.

## Problem

What the server shows for the MacBook Pro's agent (`mind_353abc18de2dbd0e`), read through hesere on 2026-10-30:
- **The command:** APP's `collect_logs` `boot`, `timeout_seconds` 30, operation `20261030062916.778998749_000913`.
- **The server ended it** at 06:29:49 UTC as `timeout`, with no artifact.
- **The agent's answer arrived at 07:28:28,** 3519 s late. The server kept it as `late_acknowledgement` (`completed`, exit status 0) and attached nothing.
- **Meanwhile the agent took no command.** It was `busy` from 06:29:35, and offline and back in turn (APP's message).

Why it took an hour is not yet read from the Mac's log. What the code and the configuration suggest:
- **The answer carries the log inline,** as base64.
  - The owner's policy there lets up to 2 MiB leave, about 2.8 MB in base64.
  - The boot log of a boot 20 hours long may reach that bound. Yesterday's, after a shorter boot, was 338 954 bytes and took 22 s.
- **The Mac's Wi-Fi is slow, with frame loss** (BCM4331, 750-EFF-0012).
  - Each try of the acknowledgement sends the whole body.
  - A transport error starts it again from the beginning.
- **While a try runs, no heartbeat goes,** which matches the agent going offline.
- **The agent takes no new command while an answer waits** (`at_capacity=1`). This is by design: the server delivers each command once.

## Plan

1. **A late answer goes without its artifact.** Once the command's run time has passed, the acknowledgement carries:
   - the status;
   - a short `stdout`: the artifact's size and SHA-256, "not sent: the server had ended the command".

   The run time is its `timeout_seconds` from when the agent received it, or the server's default for its kind. The artifact file is then dropped. The server keeps nothing more of a late answer anyway (above).
2. **Read the Mac's log of 06:29–07:28** before changing the code, to confirm or correct the cause above.
3. **Heartbeats between the tries** of an acknowledgement. Check that one goes when a try fails, so that the server does not count the agent offline.

## Acceptance criteria

- [ ] The `effector` QEMU suite, x86 and aarch64: a capture whose run time passes before its acknowledgement is sent is acknowledged without its artifact, and the agent takes the next command at once.
- [ ] On the MacBook Pro through hesere: a `collect_logs` `boot` with a short `timeout_seconds` holds the agent no longer than its acknowledgement's one try.

## Related

[750-EFF-0012](750-EFF-0012-run-against-the-effector-server.md), [750-EFF-0021](../issues-done/750-EFF-0021-a-delivery-answers-with-the-file-read-back.done), [docs/effector.md](../docs/effector.md) section 1 (the MIND Core machines), [requests-EFF.md](requests-EFF.md).
