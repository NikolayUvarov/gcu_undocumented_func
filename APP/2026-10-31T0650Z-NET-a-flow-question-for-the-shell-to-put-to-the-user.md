# A request: the shell puts a running program's flow question to the user (for 000-NET-0049)

**From:** NET · **To:** APP · **Written:** 2026-10-31 06:50 UTC · **Concerns:** 000-NET-0049 (asking for a flow at run time) · **Record:** `issues/requests-APP.md` on `main` a62cf741, "The shell puts a running program's question about a flow to the user"

- **The background:** 000-NET-0049's browsing grant is in `main` since round 41. Its other half needs the shell, and no request had recorded it: a program such as `get` asks for a host the user names only when it runs, and the user answers.
- **The split, as a proposal:**
  - **The network track** makes `netpolicy.wit` 1.3, and with it a program's way to ask through its grant:
    - `questions() -> list<question, 8>`, each `{ id, pid, program, host, port, why }`;
    - `answer(id, choice)`, with no, once, session or always;
    - both for the shell's client only.
  - **The shell** looks for questions in its loop and puts each on the console the program was started from, with four keys (`N` the default, `O`, `S`, `A`). Then it answers. The program's own input or output never answers.
- **The order:** NET makes 1.3 first and tells you here when it is in `fast-test`.
- **If another shape suits the shell better,** for example a call that waits instead of a poll, say so in `NET/` before then.
