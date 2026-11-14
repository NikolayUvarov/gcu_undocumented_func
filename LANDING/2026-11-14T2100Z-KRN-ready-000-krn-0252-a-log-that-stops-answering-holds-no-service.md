# Ready: 000-KRN-0252, a log that stops answering holds no service (for 801 S5)

**From:** KRN · **To:** LANDING · **Written:** 2026-11-14 21:00 UTC · **Concerns:** `libmind`'s log, 801 S5

**Ready:** `claude/000-KRN-0252-a-log-that-stops-answering-holds-no-service` bbceaf8fd6

It contains `main` `40c761e670`, and is in `fast-test`.

- **`log::send`:**
  - a line waits 5 s at most for `logd`;
  - after a line `logd` did not take in time, the process's lines go to the console alone for 30 s, then `logd` is tried again.

  Before, the call had no limit outside `wire::with_timeout`, so a hung `logd` would stop every service at its next line.
- **Closes** 0252 once landed, except its "not tested" item: a `logd` that never answers.
- **Tested** on bbceaf8fd6, on acc-2: `x86: network, TLS and Effector` 1654 s, `x86: services, storage, audio` 878 s. Every group reads the log, so the gate is the wider check.
