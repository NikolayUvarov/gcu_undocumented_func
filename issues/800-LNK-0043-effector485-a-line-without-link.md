# 800-LNK-0043 — effector485: a line with no `link` on it, such as U-Boot's prompt, as a console that takes input

**Type:** tool · **Owner:** `LNK` · **Priority:** P1 · **Status:** in progress (2026-11-10) · **Blocked by:** — · **Main task:** [800](800-mind-link.md); [100004](100004-two-allwinner-boards-before-the-phone.md) · **Constitution:** MC-7.1, MC-7.4, MC-7.5, MC-12.4

## Problem

**effector485 1.1** ([800-LNK-0039](800-LNK-0039-effector485-as-the-effector-agents-engine.md)):
- it shows the bytes outside frames as console text;
- it takes console input only for a machine that offers its console on the link (`link-console`, channel 1).

**What a board's line carries before MIND Core runs:** U-Boot's banner and its prompt, with no `link` to answer. To stop U-Boot's countdown, read its environment or boot another card's file, a person types at that prompt. Through effector485 today they cannot. The PinePhone's Tow-Boot is the same.

## Plan

1. **A line's raw console:**
   - while no machine on the line answers `hello`, `POST /api/v1/console/input` for the line writes its bytes to the port as they are. The line is named as its console is today: the machine's ID on a cable of one, `line` on a cable of several;
   - with its sequence number, written once;
   - with `pace_ms` between characters, default 20 ms (U-Boot drops a fast paste);
   - under the lease's rules, as for a machine's console.
2. **A break** (`POST /api/v1/console/break`, with its length) where the port has one: Windows `SetCommBreak`, Linux `tcsendbreak` or `TIOCSBRK`.
3. **The mode stated:** `GET /api/v1/endpoints` and the console's answer say whether the line is `console` (nobody answers the link) or `link`.
   - A raw write while a machine answers is refused, with that reason. A frame and typed bytes must not mix on a live link.
4. **API 1.2,** its change listed in `CHANGES` (MC-12.4).
5. **The page:** a line's console takes typing when the line is in `console` mode.
6. **Tests:**
   - host tests: a pseudo-terminal with a fake U-Boot prompt that echoes, input paced and written once, a stale holder refused, a write refused while `mindlink serve` answers;
   - on a board: U-Boot's countdown stopped and `version` answered.

## Acceptance criteria

- **The host tests pass** on Linux, and the Windows build builds.
- **On each Allwinner board,** through effector485 from a host: U-Boot's countdown is stopped, `printenv` is read, then `boot` continues to MIND Core, whose boot lines follow in the same console.

## Progress (2026-11-10)

**Plan steps 1 to 5 made** on `claude/LNK-link`, and step 6's host tests:
- **The mode:** `link` while a machine on the line answers `hello`, `console` while none does. `GET /api/v1/endpoints` and `GET /api/v1/console` give it as `mode`.
- **The line's raw console:** `POST /api/v1/console/input` to the machine's console on a cable of one, or to `line` on a cable of several.
  - In `console` mode the daemon's line thread writes it to the port as it is, `pace_ms` after each character (20 by default, 0 to 1000).
  - The console's `seq` rule holds, and so does the lease rule. On `line`, every lease on the cable holds it.
  - 503 while the port does not open.
- **The break:** `POST /api/v1/console/break` with `ms` (250 by default, 10 to 5000), after the input before it. Linux: `TIOCSBRK` and `TIOCCBRK`; Windows: `SetCommBreak` and `ClearCommBreak`; a TCP line: 409 `unsupported`.
- **The refusal:** in `link` mode, input to `line` and a break get 409 `link_mode`, "the link's frames share this line". A machine's own console still takes input through channel 1, as before.
- **Two changes in the worker this needed:**
  - no hello from an input or a break until 60 s after the last one, since U-Boot takes a frame's bytes as typed;
  - a machine that does not answer is asked with one try every 3 s on a cable of one too. Before, it was 8 tries, some 25 s with their doubled waits at 115200, and a first key would have waited for them; now it waits one try's wait at most, about 0.6 s.
- **No hello while the line prints** (a second commit, for the Allwinner boards, whose console UART is the link's line): a machine that does not answer gets no hello while the line carried bytes outside frames within the last 3 s.
  - It keeps frames out of U-Boot's countdown and the loader's `ESC OR S WITHIN 1 S` window, where a frame's byte could stop the boot or start a safe one.
  - The rule is the line's on a cable of several. A machine that answers is not affected.
  - `hello_held` in `GET /api/v1/endpoints` (`typed`, `printing` or `null`), and one log line when a hold begins and one when it ends.
- **API 1.2,** with its line in `CHANGES`; the help text; [effector485.md](../docs/link/effector485.md) and its Russian version.
- **The page:** in `console` mode the console pane says so, and an empty line sends its carriage return. `^C` and `break` are beside it. On a cable of several, `show` then shows `line`.

**Tested** on LEG9's WSL2, 2026-11-10:
- **`cargo test` in `tools/mindlink`:** all pass: three runs in a row of the effector485 tests with the first commit, and of the whole `cargo test` with the second. The new test, `a_line_without_link_as_a_console`, has a fake U-Boot on a pseudo-terminal that echoes and prints `=> `:
  - Ctrl-C and `version` written 20 ms apart, and nothing of the daemon's after them; the same `seq` twice written once;
  - a non-holder's input and break refused under a lease, the holder's taken, and a break answered on the pseudo-terminal;
  - with `mindlink serve` answering: `link` mode, and a break refused; on a cable of two with one answering, input to `line` refused;
  - on a TCP line nobody answers: input to `line` written, and a break answered `unsupported`.

  `no_hello_while_the_line_prints`: while the fake prints a countdown line every 500 ms for 5 s, no frame reaches it and `hello_held` is `printing`; a hello comes no sooner than 2.5 s after its last line and within 3 s and a round (3.0 s in a run). Unit tests in `console.rs` pace the typed bytes, share the input's numbers, and tell when the line last printed.
- **`tools/mindlink/build.sh`:** the Linux and the Windows builds, with no warnings, with each commit.
- **`scripts/host_tests.sh`** in full, after `./02_build.sh`, with OpenSSH's server from an unpacked package (`MIND_SSHD`): passed, with each commit.
- **The x86 QEMU suite `link`,** since `port_unix.rs` is `mindlink`'s too: passed, with each commit.

**Not done** (MC-12.3):
- the boards' run of the acceptance criteria;
- a break seen on a real line: a pseudo-terminal sends none;
- on each board, that the hold keeps the daemon's hellos out of U-Boot's countdown and the loader's window.

## Related

[800-LNK-0039](800-LNK-0039-effector485-as-the-effector-agents-engine.md), [800-LNK-0042](800-LNK-0042-mind-core-as-the-master-of-a-serial-line.md), [docs/remote-serial.md](../docs/remote-serial.md) (section 3: the two modes), [100004-MOB-0004](100004-MOB-0004-effector-to-the-boards-over-their-uart.md).
