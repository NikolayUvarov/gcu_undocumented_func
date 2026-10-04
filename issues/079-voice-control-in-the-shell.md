# 079 — Voice V2: voice control in the shell

**Type:** feature · **Owner:** tools track · **Priority:** P1 · **Status:** open · **Blocked by:** — (078 done) · **Roadmap:** track G · **Constitution:** Art. 8.1–8.2, 11.5, MC-3.7, MC-3.11

## Problem

With a recognizer (078) the system can hear commands but cannot act on them or answer. The voice path must propose, and the shell — the user's agent — must decide, with the same rules as typed commands ([docs/voice](../docs/voice/README.md) §2–3, V2).

## Plan

- `voice` (console program): the shell starts it in a launch session and lends the audio client, the `tts` client and an endpoint back to the shell (`SLOT_INIT`, the ping/pong slot). It records an utterance on request, recognizes it, sends `intent { verb, object, arguments, confidence, text }` to the shell, and speaks the shell's reply. It holds nothing else.
- Shell: push-to-talk with F12 (press: listen for one utterance; Esc cancels); the intent appears as `voice: открой файлы` and becomes a command line through the shell's own command table. Stop, kill, delete, format and reboot ask for confirmation, answered by voice (closed yes/no grammar) or Enter/Esc. The shell answers through `tts` ("Запускаю файловый менеджер", "Не понял").
- Commands: open and close the tools (`fm`, `edit <file>`, `view <file>`, `top`, `memmap`, `load`, `hw`, `dmesg`, `svc`), time and date, free memory, read a file aloud (`say` of its first lines), stop or restart a service (confirmed).
- `audio_gw`: one capture owner at a time — `record-start` fails with `busy` while another client records (`idl/audio.wit` 1.1), so `listen`, `hear` and `voice` never mix streams.
- Push-to-talk works while the shell has the focus; a key that works over any program is kernel issue [154](154-push-to-talk-routing.md).

## Acceptance criteria

- QEMU with a WAV source standing in for the microphone: "открой файлы" starts `fm`; "который час" makes the shell speak the time; "останови службу rtc" asks for confirmation and stops `rtc` only after "да"; an out-of-grammar phrase is answered "Не понял" and runs nothing.
- `voice` holds no capability besides the audio and `tts` clients, its file client and the endpoint to the shell (checked with `caps`).
- README, `docs/voice` (EN/RU) and the shell's `help` describe voice control.

## Related

[078](../issues-done/078-voice-command-recognizer.done), [154](154-push-to-talk-routing.md), [docs/voice](../docs/voice/README.md).
