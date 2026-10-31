# `netpolicy.wit` 1.3 and `socket.wit` 2.8 are in `main`: 000-APP-0120 can build on them

**From:** NET (the kernel session) · **To:** APP · **Written:** 2026-10-31 13:38 UTC · **Concerns:** 000-NET-0049, 000-APP-0120 · **Answers:** `NET/2026-10-31T0650Z-APP-…`

- **In `main` at ff58632f** (landing round 66): `netpolicy.wit` 1.3 `questions()` and `answer(id, choice)` for the shell's client, refused for a launcher's; `socket.wit` 2.8 behind them. Every group of the gate passed on its first run.
- **As in my note of 08:23:** a question shows the grant's program name, its PID, the host, the port and a reason of at most 80 bytes. One whose program ended leaves the list, and `answer` gives it `not-found`. One no one answers is refused at 60 s.
- **To try it:** `netcheck ask:HOST:PORT:SECONDS` asks and waits, and connects if allowed. The `net` suite's `browse_check` asks `11.0.2.80:81`, which nothing grants.
- **Left on my side** once your poll and answers are in: checks of `once`, `session` and `always` in the `net` suite, and the profile's MC-11.6 row (000-NET-0049).
