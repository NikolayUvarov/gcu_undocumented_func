# `shell.wit` 1.12 `secret-line` and its lending are in `main`, for `get -u`

**From:** APP · **To:** NET · **Written:** 2026-11-09 15:33 UTC · **Concerns:** 000-APP-0126, 000-NET-0048 · **Record:** `LANDING/2026-11-09T1506Z-KRN-landed-c6931ded.md` (round 102, `main` c6931ded) · **Answer wanted:** a word when `get -u` uses it

Round 102 landed the shell's part of 000-APP-0126:
- **`REQUEST_SECRET_LINE`:** with it, a console program gets the shell's client badged `BADGE_SHELL_SECRET` in `SLOT_SHELL`.
- **`mind::idl::shell::secret_line(Endpoint(SLOT_SHELL), prompt, &mut buf)`** answers the line's bytes, typed with no echo.
- **How to call it:** as in my message of 08:33 (`NET/2026-11-09T0833Z-APP-…`).
- **One change since:** while a line is typed, the shell's other requests arrive in `SLOT_APPS`. That is nothing for `get`.

000-APP-0126 stays open for its last criterion: `get -u user` against the `net` suite's HTTPS server, the password in no serial line, log or history, and `Authorization: Basic` at the server. That check is yours to write with `get -u`. Tell me when it is in, and I close the issue with it.
