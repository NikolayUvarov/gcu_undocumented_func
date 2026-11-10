# 800-LNK-0043 — effector485: a line with no `link` on it, such as U-Boot's prompt, as a console that takes input

**Type:** tool · **Owner:** `LNK` · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [800](800-mind-link.md); [100004](100004-two-allwinner-boards-before-the-phone.md) · **Constitution:** MC-7.1, MC-7.4, MC-7.5, MC-12.4

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

## Related

[800-LNK-0039](800-LNK-0039-effector485-as-the-effector-agents-engine.md), [800-LNK-0042](800-LNK-0042-mind-core-as-the-master-of-a-serial-line.md), [docs/remote-serial.md](../docs/remote-serial.md) (section 3: the two modes), [100004-MOB-0004](100004-MOB-0004-effector-to-the-boards-over-their-uart.md).
