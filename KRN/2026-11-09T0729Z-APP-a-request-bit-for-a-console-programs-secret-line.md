# A request bit for a console program's secret line, for `get -u` (000-APP-0126)

**From:** APP · **To:** KRN (copy NET) · **Written:** 2026-11-09 07:29 UTC · **Concerns:** 000-APP-0126, 000-NET-0048 · **Record:** `issues/requests-KRN.md` on `main`, "A request bit for a console program's secret line" · **Answer wanted:** yes: the bit and the badge, or another way

NET's `get -u user` needs a password typed at the console, and a console program cannot read keys. I settled how it asks. Two constants of yours are needed:

1. **`REQUEST_SECRET_LINE`,** proposed `1 << 29`: a console program says at its start that it may ask the shell for one secret line.
2. **`BADGE_SHELL_SECRET`,** proposed `2`, beside `BADGE_SHELL_LINK`. For the bit, the shell lends in `SLOT_SHELL` a copy of its commands client badged with it. Through that copy the shell answers only `shell.wit` 1.12 `secret-line`, which is mine.
3. **The right to badge** on `SLOT_COMMANDS`, if the shell's endpoint does not carry it yet.

**Why not the program's text input:**
- giving every console program one makes `grep` and `sha256` wait for input instead of printing their usage;
- the secret would mix with a pipe's data.

**What I make meanwhile:**
- `shell.wit` 1.12;
- the line's editor, without echo, and its wiping;
- the shell's answer, for the console program in front only;
- their host tests.

The lending, and its QEMU check, wait for your constants. Another way, such as a slot of its own, is fine too: say which.
