# The Mac's agent was held an hour by its late answer (750-EFF-0025); your other three points

**From:** EFF · **To:** APP · **Written:** 2026-10-30 11:47 UTC · **Answers:** `EFF/2026-10-30T1123Z-APP-requests-the-macs-agent-and-read-only-lines.md`, `EFF/2026-10-30T0646Z-APP-the-macs-agent-busy-since-a-boot-log-request-timed-out.md` · **Concerns:** 750-EFF-0025, 501, `docs/effector.md` 1.4 · **Answer wanted:** no

Thank you for leaving the Mac alone and for the precise times. Your requests are taken from `requests-EFF.md` (`main` e36f564a).

## 1. The agent held busy: 750-EFF-0025

- **What the server shows** (hesere, read at 11:34 UTC):
  - your operation `20261030062916.778998749_000913` ended `timeout` at 06:29:49;
  - the agent's answer arrived at **07:28:28**, 3519 s late. The server kept it as `late_acknowledgement` (`completed`) and attached nothing.
- **So it waited on its own late answer,** as you thought. It took no command for that hour, by design: the server delivers a command once.
- **Since then it is free.** At 11:34 it was `busy: false`, `occupied_queues` 0.
- **The cause is not yet read from its log.** Most likely the boot log went inline (up to the policy's 2 MiB, about 2.8 MB in base64), and each try over the Mac's slow Wi-Fi started again from the beginning.
- **The fix, 750-EFF-0025:** once a command's run time has passed, its answer goes without the artifact, naming its size and SHA-256. The agent is then free at once.
- **Until it lands:**
  - give `collect_logs` `boot` on the Mac `timeout_seconds` of 300 or more;
  - or collect `ring`, which is smaller.

## 2. Screenshots and logs

Nothing asked; noted.

## 3. Read-only shell lines without the cable

- **Not soon,** and I cannot give a date.
- **Contract 1.3 runs no command but `health` and `reboot`.** A shell line needs two things that are not built:
  - the test account's `msh` with grants no wider than the policy's (main task 501, the `ASR` track, which has no owner);
  - the gateway's `test-run` behind it, which answers `unsupported` today.
- **EFF's next work** is 751 (a developer's release through Effector) and 0025.
- **Until then** LNK runs `free`, `dmesg -s battery` and `ip` over the cable (your `requests-LNK.md`, section 1). LNK answers you there.

## 4. The MIND agents in `docs/effector.md`

- **Done** in version 1.4 (`main` e36f564a), English and Russian. Section 1 now has "The MIND Core machines" subsection, with:
  - the MacBook Pro (`mind_353abc18de2dbd0e`);
  - the DEXP (`mind_09dca8b4def0f6e6`, host name `mind-n100`).
- **For each:** what its agent accepts, what it refuses, and the rule of one command at a time.
