# Ready: `get` asks the user for a host its grant does not name (000-NET-0048)

**From:** KRN (for NET) · **To:** LANDING · **Written:** 2026-11-06 04:43 UTC · **Concerns:** 000-NET-0048 (advances it; stays open), two requests to APP in `issues/requests-APP.md`

**Ready:** `claude/youthful-mendel-mf1soy` 5ef99d1665dc780a00ac18b91a5658bb087bb894

It contains `main` bc092947.

## What it brings

- **`get`:** a connection or a name's lookup its grant refuses (`Denied`) becomes a question through the grant (`socket.wit` 2.8). The shell puts it on the console `get` runs in front of (000-APP-0120).
  - The answer's address is connected to.
  - `N` or no answer: exit 7, `REFUSED BY THE USER`.
  - Its reason names the command line, not the URL.
- **`get_runs`** (the `net` and `download` suites): a third stand-in that the policy does not give `get`, refused with `N` and allowed with `O`. The old check that `https://10.0.2.3/` ends at once with 7 is gone, since that host is now asked.
- **Docs:** `docs/network/downloads.md`, the README, the profile's MC-11.6 row; `get`'s help.
- **Two requests to APP** in `issues/requests-APP.md`:
  - a console program reading a secret typed at its console, for `-u`;
  - a question from a program that a script runs.

  No interface or ABI change.

## Tested, on 5ef99d1665dc780a00ac18b91a5658bb087bb894, through Effector

- **x32-128, x86:** `host tests` (1276 s), `network, TLS and Effector` (1089 s).
- **PCU, aarch64:** `network, TLS and Effector` (1327 s).
- **Before that,** in this session's container, x86: the `download` suite, every check passed.
