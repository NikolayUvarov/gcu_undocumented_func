# 000-NET-0060 — The run-time question read whole from the log, wherever the prompt cut it

**Type:** test · **Owner:** `NET` · **Priority:** P1 · **Status:** in progress (fixed on the branch; tests running) · **Blocked by:** — · **Roadmap:** the landing gate · **Constitution:** MC-12.2

## Problem

**What failed.** The `net` suite on x86, in this session's container (QEMU TCG, 4 CPUs, 2026-11-05, the tree of 550-NET-0059 at eca4ea15), failed in `browse_check`, at the second question asked at run time (000-NET-0049, 000-APP-0120). It timed out waiting for `ALWAYS? `, though the question had come whole.

**Why.** `asked()` reads the output of `run netcheck … &` up to its prompt, then waits for the rest of the question.
- The prompt came while the shell was printing the question's first word: the command's output ended with `NETW`, the next output began with `ORK: netcheck (PID 6) ASKS…`.
- `question` (`NETWORK: netcheck (PID `) was then in neither part. So `expect("ALWAYS? ", after=question)` never matched.
- 000-NET-0056 handled a cut after the question's start, not one inside it.

**550-NET-0059 is not the cause.** It changes no grant, broker or shell code. The same tree passed `browse_check` on aarch64 in the same run.

## Plan

`asked()` notes the log's length before it sends the command, and waits for `ALWAYS? ` in the log from there (`vm.logged`). The log keeps every character, wherever `expect` cut the output. No other question is pending at that point, so the `ALWAYS? ` read is this question's.

## Acceptance criteria

The `net` suite passes `browse_check` on x86 and aarch64 with the change.

## Related

- [000-NET-0056](../issues-done/000-NET-0056-the-tls-lending-check-lists-the-grant-while-its-program-runs.done): the cut after the question's start.
- [000-NET-0049](../issues-done/000-NET-0049-a-flow-grant-asked-at-run-time.done), [000-APP-0120](../issues-done/000-APP-0120-the-shell-asks-the-user-about-a-programs-flow.done): the question and its check.
- [550-NET-0059](550-NET-0059-a-radios-own-channels-apart-from-its-limits.md): the run that showed it.
