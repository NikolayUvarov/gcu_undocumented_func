# 000-KRN-0110 — A halt says so on the screen: `MIND CORE: STOPPING THE SYSTEM...`

**Type:** kernel · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress (made on the branch; the gate and the MacBook Pro's run left) · **Blocked by:** — · **Main task:** — · **Constitution:** MC-6.8, MC-12.3

## Problem

The maintainer, 2026-10-14: on the MacBook Pro, the shell's `stop` halts with nothing on the screen, not even the new line after the command.

- **The shell's text never reaches the screen.**
  - `stop` flushes the disks, writes `SYSTEM HALTED. CPU GOING TO SLEEP...` and calls `HALT`.
  - That text goes to the shell's surface, which the compositor copies to the screen in its next frame.
  - `HALT` stops every CPU at once (`cpu::halt_all`), so that frame never comes.
- **The next boot is a safe start.**
  - `HALT` leaves the run's mark (000-KRN-0076) set, so the next boot says `THE LAST RUN DID NOT END AS ASKED`, though this one did.
  - `REBOOT` clears the mark; `HALT` does not.

## Plan

1. **`HALT` (`halt_asked` in `kernel/src/main.rs`):**
   - the kernel clears the run's mark, as `REBOOT` does;
   - it stops the other CPUs, waits 2 ms for a frame being copied, and then writes from the top of the screen, in the boot colours (`screen::last`):
     - `MIND CORE: STOPPING THE SYSTEM...`
     - `MIND CORE: THE SYSTEM IS STOPPED. IT IS SAFE TO TURN THE MACHINE OFF.`
   - COM1 gets `MIND CORE KERNEL: STOPPING THE SYSTEM` and `MIND CORE KERNEL: THE SYSTEM IS STOPPED`.
   - Then every CPU halts.
2. **The `normal` suite** ends with `stop` and reads both lines from the screen and COM1 (x86 and aarch64).
3. docs/api's `HALT` row says so.

## Acceptance criteria

- [ ] The `normal` suite's `stop` check passes on x86 and aarch64.
- [ ] On the MacBook Pro, `stop` leaves both lines on the screen, and the next boot is not a safe start.

## Related

000-KRN-0076 (the run's mark), 211-KRN-0013 (kernel text on the screen), 000-KRN-0080 (power off), `shell/src/main.rs` (`stop`, the tools track's).
