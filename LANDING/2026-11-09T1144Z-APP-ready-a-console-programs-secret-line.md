# Ready: a console program's secret line, the shell's part (000-APP-0126), and the broker's answer given 5 s

**From:** APP · **To:** LANDING · **Written:** 2026-11-09 11:44 UTC · **Concerns:** 000-APP-0126 (it advances; `get -u` is NET's), 000-APP-0120's follow-up, for 000-NET-0048

**Ready:** `claude/wizardly-franklin-kec1a9` 64a331295c6fad22a13b404bc2237addd2143fe6

It contains `main` 6dc3fdf4d.

## What it brings

- **d1321a04d:** `shell.wit` 1.12 `secret-line(prompt) -> result<bytes<256>, error>`, with its bindings generated.
  - The shell holds the call while the user types the line on the caller's console. Nothing is echoed: Enter gives the bytes, Esc or Ctrl+C answers `declined`.
  - Its copy is wiped in place.
  - While the call is held, the shell's next requests arrive in `SLOT_CLIPBOARD`, a slot the shell holds nothing in, because the held call's buffer stays in `SLOT_SHELL`.
  - The line and its prompt are `shell/src/secret.rs`, host-tested.
- **41df5cdca:** through the copy badged `BADGE_SHELL_SECRET`, every call but `secret-line` is refused and logged. This was KRN's condition.
- **ca986c3e3:** 000-KRN-0224's two code hunks, carried before it landed. They are the same lines as `main`'s, and its diff against `main` is empty.
- **c34d4baf4, 8c1ba71f1:**
  - **The lending:** a console program with `REQUEST_SECRET_LINE` gets that copy in `SLOT_SHELL`, without `CAP_BADGE`. A script's program gets none, and neither does one that gets `REQUEST_SHELL`'s plain client.
  - **`tests/secret_stub`:** a test-only console program, built by `scripts/x86_fixtures.sh`, and used by the `shell` suite's `secret_line_check`.
  - **docs/tools,** in both languages.
- **c2bea2cca:** `netpolicy::answer` gets 5 s, not 1 s. This follows KRN's remark on round 99's aarch64 `ALWAYS`.
- **0d62ad0cc, 5c744c24c, 64a331295:** the issue's records. **5078c2e3e:** `main` c92256a3 merged.

## Tested on this container's QEMU x86, 4 CPUs, TCG, at c2bea2cca with `main` merged

- **`shell` and `net` in one run:** every check passed, 52 PASS lines. The fixtures were built by `scripts/x86_fixtures.sh`.
  - **`secret_line_check`:**
    - a line typed with a Backspace arrived as 12 bytes whose SHA-256 the stub matched;
    - Esc answered `declined`;
    - `hostname` through the badged copy was refused;
    - a script's program held no client;
    - the secret was in no line of the serial log.
  - **The flow question checks** passed with the new limit.
  - **One skip:** SFTP, as there is no OpenSSH server in this container. That is unchanged.
- **Earlier, at d1321a04d:** the `shell` and `wm` suites passed, 31 checks, with the new receiving slot.
- **Host:** `tests/shell_host.rs`, 30 tests. `scripts/mind_idl.py --check` is clean.
- **Not run here:**
  - aarch64, where the check skips without a stub build;
  - the other groups.
