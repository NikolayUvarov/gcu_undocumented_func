# 000-APP-0126 — A console program reads a secret typed at its console (`get -u`)

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** — · **Constitution:** MC-11.5, MC-11.9

**Asked by:** NET, 2026-11-06, in [requests-APP.md](requests-APP.md), for [000-NET-0048](000-NET-0048-get-a-wget-and-curl-analog.md)'s `get -u user`. Numbered on 2026-11-08.

## Problem

`get -u user` should ask for the password on the console, and never take it from the command line. `get` is a console program, with no screen, and while one runs the shell keeps the keys typed:
- Esc and Ctrl+C stop it, and the other keys are dropped (`Shell::key`);
- the serial line's keys reach a program only when it has the focus (`Shell::uart`).

So `get` cannot read the password.

## Plan

NET's second form, which keeps the secret in the shell's hands for the shortest time and never on the screen:
- **The shell reads one secret line** for a console program that asks for it: without echo, Enter ends it, Esc stops it.
- **It gives the program the bytes,** and they are never shown, logged or kept in the history. The shell's copy is wiped after it gives them.
- **How the program asks** (a call of the shell's client, or a console request) is settled with the first step; `get` is its first user.

## Acceptance criteria

- **Host tests:** the line's keys (no echo, Enter, Esc, Backspace) and its wiping.
- **QEMU:** `get -u user` against the `net` suite's HTTPS server reads a password typed at the console. The password is in no serial line, log or history, and the server sees `Authorization: Basic` with it.

## Related

[000-NET-0048](000-NET-0048-get-a-wget-and-curl-analog.md), [550-APP-0033](550-APP-0033-wifi-setup-program.md) (`wifi`'s passphrase shown as dots).
