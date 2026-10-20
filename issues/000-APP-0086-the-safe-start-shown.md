# 000-APP-0086 — The shell and `wm` show a safe start, and how to start what it left out

**Type:** tools (`shell`, `wm`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open (planned) · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-12.3

**Recorded by:** the kernel track (KRN), 2026-10-14, as a note in `requests-APP.md`, for [000-KRN-0076](000-KRN-0076-a-frozen-machine-restarts-and-starts-safely.md). Numbered by the tools track on 2026-10-20, at the maintainer's word.

## Problem

After a run that did not end as asked (a freeze the watchdog reset, a cut power), `init` makes the next boot a safe start: it logs `[INIT] SAFE START: … NOT STARTED: bcm_wifi, video_gw, audio_gw, tts` and leaves those stopped. The line is only in the log. The user sees no Wi-Fi, camera or sound and is not told why.

## Plan

- **The shell** finds the line in the system log at its start and shows it above the prompt, with `svc start <name>` for each service left out.
- **`wm`** shows it once in its top bar's message line, with the same advice, and a click on it opens the shell's window.
- Nothing is shown on a boot without the line.

## Acceptance criteria

- **A host test:** the line is parsed into the services left out, and a log without it gives nothing.
- **The `services` suite** (or the one that makes a safe start in QEMU): after a safe start the shell's screen shows `SAFE START` and `svc start tts`.
- **The `wm` suite:** the message is in `wm`'s log.

## Related

[000-KRN-0076](000-KRN-0076-a-frozen-machine-restarts-and-starts-safely.md), [173-KRN-0036](173-KRN-0036-safe-start.md).
