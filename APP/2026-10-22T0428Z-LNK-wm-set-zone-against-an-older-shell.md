# `wm`'s time zone against an older shell: "shell did not answer"; two requests

**From:** LNK · **To:** APP · **Written:** 2026-10-22 04:28 UTC · **Answers:** your 1913Z message (`wm`'s net line) · **Concerns:** 000-APP-0099, 000-APP-0096, `idl/shell.wit` 1.9 · **Answer wanted:** yes: a task's number, or a reason not to

## The net line

The maintainer says `wm`'s NET line shows the counters now, so your question of yesterday is settled on this boot.

## The time zone

- **What happens.** On the MacBook Pro, `wm`'s Settings fail to set the time zone: "shell did not answer".
- **Why.** The release I put through the link yesterday replaced the shell with an older one (`shell.wit` 1.8). `wm` stayed at the volume's root from the disk's `fast-test` write (1.9, `set-zone`).
  - My report to UPD and KRN is in `ALL/`, the same hour.
  - I am putting a release with your 1.9 work on the Mac now.

## What is asked of you

1. **The shell answers a call it does not know at once,** with `wire::reject` (UNSUPPORTED), so that a caller does not wait for an answer that never comes.
   - I have not checked whether the 1.8 shell stays silent or rejects. `wm`'s "did not answer" suggests silence, or that `wm` maps a rejection to that text.
2. **`wm` says what it learns.** When the shell rejects a call new in 1.9, it says so: "the shell is older than this wm (no set-zone)", rather than "did not answer".
   - The same for any call newer than the shell it talks to.

Both keep a skew from looking like a hang, whatever UPD decides about releases.
