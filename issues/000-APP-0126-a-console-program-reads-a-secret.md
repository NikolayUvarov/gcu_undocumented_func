# 000-APP-0126 — A console program reads a secret typed at its console (`get -u`)

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** in progress (the shell's part written, 2026-11-09) · **Blocked by:** `KRN`: `REQUEST_SECRET_LINE` and `BADGE_SHELL_SECRET` ([requests-KRN.md](requests-KRN.md), 2026-11-09), for the lending and the QEMU check · **Main task:** — · **Constitution:** MC-11.5, MC-11.9

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
- **How the program asks** (settled 2026-11-09):
  - **`REQUEST_SECRET_LINE`**, a bit `KRN` gives: the program says at its start that it may ask.
  - **The shell lends, in `SLOT_SHELL`, a copy of its commands client badged `BADGE_SHELL_SECRET`.** Through it the shell answers only `shell.wit` 1.12 `secret-line(prompt: string<80>) -> result<bytes<256>, error>`.
  - **Not the program's text input.** A text input for every console program would make `grep`, `sha256` and others wait instead of printing their usage, and the secret would mix with a pipe's data.
- **When the shell answers:** only for the console program in front of a console, the sender of the call.
  - It draws the prompt itself: `SECRET FOR get (PID n): <the program's words>`, with control characters shown as `?`, as the flow question does.
  - The line is typed with no echo. Enter ends it, Esc refuses it (`declined`), and Backspace takes a character back.
  - The answer goes back in the call's buffer, which is revoked after the reply. The shell's copy is wiped.
- **`get`**, NET's, is its first user.

## Acceptance criteria

- **Host tests:** the line's keys (no echo, Enter, Esc, Backspace) and its wiping.
- **QEMU:** `get -u user` against the `net` suite's HTTPS server reads a password typed at the console. The password is in no serial line, log or history, and the server sees `Authorization: Basic` with it.

## Progress (2026-11-09): the shell's part

- **`shell.wit` 1.12:** `secret-line(prompt: string<80>) -> result<bytes<256>, error>`. Its bindings are generated (`scripts/mind_idl.py`).
- **`shell/src/secret.rs`** (no system calls):
  - the shell's prompt, `SECRET FOR <program> (PID n), TYPED WITH NO ECHO (ENTER ENDS IT, ESC REFUSES IT): <the program's words>`, with both names made safe as the flow question's are;
  - the line: printable characters up to 256 bytes, a character that does not fit whole left out; Backspace takes one character back (UTF-8 too); Enter ends it; Esc and Ctrl+C refuse it;
  - its wiping, by writes the compiler keeps, when it is given or refused and when it is dropped.
- **`shell/src/secretctl.rs`:**
  - **Who is answered:** only through the badged copy (`BADGE`, the value proposed as `BADGE_SHELL_SECRET`) and only for the console program of an open session. Any other caller gets `refused`, and a second line while one is typed gets `busy`.
  - **Nothing else through that copy:** every other call that comes through it is refused and logged, so a program that asked only for a secret line gets none of the shell's other commands (KRN's condition, 2026-11-09).
  - **The call is held** (`Call::defer`) while the user types. Its buffer stays where it came, so the shell's next requests arrive in another slot it holds nothing in (`SLOT_CLIPBOARD`).
  - **The keys** of that session go to the line first after a flow question's, from the serial line, the PS/2 keyboard and the shell's window. Nothing is echoed.
  - **The answer:**
    - the bytes, in the caller's buffer, or `declined`;
    - `unavailable`, with nothing written, when the program has ended or stopped waiting (its buffer revoked);
    - the copy wiped where it is kept before the state goes, and the session's line says `(GIVEN)`, `(REFUSED)` or why it went.
  - **The system log** names the program, its PID and the outcome: never the words or the bytes.
- **Tests:** host, `tests/shell_host.rs` (the keys, the limit, the wiping, the prompt).
- **Left:**
  - the lending, for KRN's bit;
  - the QEMU check with NET's `get -u`;
  - docs/tools in both languages.
- **Not covered:** a script's wait serves no calls of the shell's commands, so a program a script runs gets no line until it ends.

## Related

[000-NET-0048](000-NET-0048-get-a-wget-and-curl-analog.md), [550-APP-0033](550-APP-0033-wifi-setup-program.md) (`wifi`'s passphrase shown as dots).
