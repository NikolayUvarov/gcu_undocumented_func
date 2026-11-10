# LNK landing request: effector485 types on a line no machine answers, and holds its hellos while the line prints (800-LNK-0043)

**Ready:** `claude/LNK-link` bee8bf20e875646ee754bbca1e70704179c6b759

**From:** LNK (the assessing session) · **To:** LANDING (KRN, the integrator)

## What it carries

On `main` 952a4e07 (later `main` has text only), two commits in `tools/mindlink/`, its docs and the issue:
- **ac42ff6e: a line's raw console, API 1.2.**
  - **The mode:** the line is `console` while no machine on it answers `hello`, and `link` while one does. `mode` is in `GET /api/v1/endpoints` and `GET /api/v1/console`.
  - **Input:** in console mode, `POST /api/v1/console/input` writes to the port as it is:
    - with `seq` written once and `pace_ms` (default 20);
    - under the lease's rules;
    - for U-Boot's prompt on the Allwinner boards and the PinePhone's Tow-Boot.
  - **A break:** `POST /api/v1/console/break` (`ms`, 10 to 5000), through `TIOCSBRK`/`TIOCCBRK` on Linux and `SetCommBreak`/`ClearCommBreak` on Windows.
  - **Refused in link mode:** input to the line and any break get 409 `link_mode`.
  - **The page:** typing in console mode, with `^C` and `break` buttons.
  - **Probing:**
    - an offline machine is asked one hello a round, on a cable of one too;
    - no hello for 60 s after a typed input.
- **bee8bf20: no hello while the line prints.**
  - **The rule:** a machine that does not answer gets no hello while the line carried bytes outside frames within the last 3 s. On the boards the console is the link's line, so a frame could otherwise stop U-Boot's countdown, or bring an ESC or `s` into the loader's 1 s safe-start window.
  - **The field:** `hello_held` in `GET /api/v1/endpoints` (`typed`, `printing`, `null`), listed in `CHANGES` under 1.2.
  - **The log:** one line when a hold begins and one when it ends.
- **The docs:** `docs/link/effector485.md` and `effector485_RU.md`. The issue 800-LNK-0043 stays open, since its board run waits for the boards.

**Issues:** advances 800-LNK-0043 (the host side done, the board run left). Closes none.

## What was tested, and where

LEG9, WSL2, Ubuntu 24.04:
- **`cargo test` in `tools/mindlink`:** three full runs in a row, all passing, on bee8bf20's exact tree. That is 7 effector485 integration tests, among them:
  - `a_line_without_link_as_a_console`: a fake U-Boot on a pseudo-terminal, typed `^C` and `version`, paced and written once; a non-holder refused under a lease; a break; 409 `link_mode` while `mindlink serve` answers;
  - `no_hello_while_the_line_prints`: no frame reaches the line during a 5 s countdown, and a hello comes within 3 s and a round after it ends.

  There are also 16 unit tests.
- **`tools/mindlink/build.sh`:** Linux and Windows build without warnings.
- **`./02_build.sh`, then the full `scripts/host_tests.sh`** with `MIND_SSHD`: exit 0.
- **The x86 QEMU suite `link`:** PASS (QEMU 8.2.2, TCG), run because `port_unix.rs` is mindlink's too.

**Not tested:** a board, and a break on a real line (a pseudo-terminal sends none) (MC-12.3).
