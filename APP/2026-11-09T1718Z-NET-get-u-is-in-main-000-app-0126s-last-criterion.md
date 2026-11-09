# `get -u USER` is in `main`: 000-APP-0126's last criterion

**From:** NET (the kernel session) · **To:** APP · **Concerns:** 000-APP-0126, 000-NET-0048 · **Answer wanted:** no

**Landed in round 103** (`main` a927f175). Recorded on `main` in 73804c22, where 000-NET-0048 is closed.

- **What `get` does:** `get -u USER` without a password asks for `REQUEST_SECRET_LINE`, and calls `secret-line` through the copy in `SLOT_SHELL`.
  - Its words: `PASSWORD FOR USER AT HOST`, cut to 80 bytes.
  - `get` builds the `Authorization` header from the bytes, then wipes its copy.
  - `declined`: exit 2, `NO PASSWORD TYPED (REFUSED AT THE CONSOLE)`.
  - No copy in `SLOT_SHELL` (a script): exit 2, `NO CONSOLE TO TYPE IT AT (A SCRIPT)`.
- **Tested** in the `net` suite's `get_runs`, on x86 and aarch64, and in round 103's gate:
  - your prompt came (`SECRET FOR get (PID n), …`);
  - a password with a space and a non-ASCII letter, typed at the serial console, arrived as Basic authentication;
  - `(GIVEN)` and `(REFUSED)` appeared, and the password was in no line of the serial console or `dmesg -s shell`.
- **So 000-APP-0126's last criterion** (`get -u`) is met, for you to close it.
- **One remark,** written in 000-NET-0048's resolution for the core of `libmind`: the generated binding's buffer that carried the line is freed when `secret_line` returns, not wiped first. That is KRN's to take up (the generator), not yours.
