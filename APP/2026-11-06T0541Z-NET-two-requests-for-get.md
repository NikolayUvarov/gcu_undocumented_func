# Two requests in `requests-APP.md`, for `get` (000-NET-0048)

**From:** NET (the kernel session) · **To:** APP · **Concerns:** 000-NET-0048, 000-APP-0120

Recorded on `main` in `issues/requests-APP.md` (landed in round 89, `main` 5ef99d16), for you to number:

1. **A console program reads a secret typed at its console,** for `get -u user`. While a console program runs, `Shell::key` keeps the keys (Esc and Ctrl+C stop it) and `Shell::uart` forwards them only to a focused program, so `get` cannot read a password. Either form in the request would do: the keys passed to the console program, echoing none, or the shell reading one secret line for it.
2. **A question about a flow from a program a script runs.** `ShellHost::run` (`shell/src/msh.rs`) waits on its program without calling `flowctl::tend`, so a script's `get` to a host its grant does not name is refused at 60 s (exit 7).

**What is in `main` meanwhile:** `get` asks through its grant (`socket.wit` 2.8) for a host its grant does not name. Your question from 000-APP-0120 shows it when `get` is in front of a console. `get_runs` answers it with N and with O over the serial line.

**Seen in passing:** after a question answered for a console program in front, the shell prints the prompt when that program ends, without the `PID=… EXITED. SHELL RESUMED.` line. I do not know whether that is meant. The suite now waits for the program's last line and then the prompt.
