# 000-APP-0086 — The shell and `wm` show a safe start, and how to start what it left out

**Type:** tools (`shell`, `wm`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** in progress (on the tools branch, 2026-10-20; the gate left) · **Blocked by:** — · **Main task:** — · **Roadmap:** track G · **Constitution:** MC-12.3

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

## Progress (2026-10-20)

- **`shell/src/safe.rs`:** reads init's lines in the boot's log: `[INIT] SAFE START:` and each `[INIT] <name> LEFT OUT BY THE SAFE START, PREPARED`. The notice is `SAFE START: THE LAST RUN DID NOT END AS ASKED. LEFT OUT: audio_gw, tts, video_gw (svc start audio_gw, svc start tts, svc start video_gw)`.
- **The shell** reads every line of the boot's log through it at start, shows the notice above its first prompt and logs it as `[SHELL] SAFE START: …`.
- **`idl/shell.wit` 1.6 `boot-notice`:** the notice for any client, empty after a normal boot. `wm` asks it at start: the notice goes on its status line with `(a click here opens the shell's window)`, and a click on that line opens the shell's window while the notice shows (`Wm::offer`). The status line is at the bottom, not in the top bar as planned.
- **Host tests:** `tests/shell_host.rs` (the lines parsed, a normal boot, the shell's own copy of the line), `tests/wm_host.rs` (the click, and none once another notice replaced it).
- **The `watchdog` suite** (x86, its safe start): the notice before the first prompt, with `svc start` for each service left out, and in `dmesg -s shell`; `wm` logs `[WM] SAFE START: …`; after the reboot that follows, no `SAFE START` from the shell. The `wm` suite makes no safe start, so `wm`'s line is checked there.
- **Docs:** `docs/tools` (EN, RU), `docs/idl`.
- **Left:** the gate on this batch.

## Related

[000-KRN-0076](000-KRN-0076-a-frozen-machine-restarts-and-starts-safely.md), [173-KRN-0036](173-KRN-0036-safe-start.md).
