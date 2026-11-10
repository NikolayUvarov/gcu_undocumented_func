# 800-LNK-0042 — MIND Core as the master of a serial line: `serialhost`, a console and MIND Link to the machines on it

**Type:** service, interface · **Owner:** `LNK` · **Priority:** P1 · **Status:** open · **Blocked by:** [800-LNK-0041](800-LNK-0041-usb-serial-cp210x-pl2303-and-several-converters.md) (a second converter); `init`'s part, a `KRN` task (requested with the interface) · **Main task:** [800](800-mind-link.md); [100004](100004-two-allwinner-boards-before-the-phone.md) · **Constitution:** MC-2.3, MC-6.1, MC-7.1, MC-7.4, MC-7.5, MC-12.3, MC-12.4

## Problem

**The maintainer (2026-11-10):** Effector works with MIND Core over RS-485 and RS-232 as with Windows and Linux, as the line's master and as its slave.

**What MIND Core can be today:**
- **The slave:** `link` answers the master on its line.
- **Not the master.** The master side of MIND Link, and the console of a machine on a cable, live only in `effector485` and `mindlink` on Windows and Linux.

**What [docs/remote-serial.md](../docs/remote-serial.md) put "later, not for the phone"** (its section 5.2, stage S4): a MIND Core machine as the host of another machine's cable. The maintainer's word makes it needed now: a board plugged into the MacBook while the MacBook runs MIND Core.

## Plan

1. **A service, `serialhost`.** It holds the lines its settings name: a converter's `serial.wit` client, never `link`'s own line. Each line is in one mode at a time, switched by the holder of its lease:
   - **`console`:** the remote-serial specification's section 4.3, sized for MIND Core:
     - a ring of what the line sends, with absolute byte offsets and the time each piece came, 256 KiB a line at least, and `gap` for what was dropped;
     - input with a sequence number, written once, at most 4 KiB a call, with `pace_ms`;
     - a break, and DTR and RTS where the converter has them (`serial.wit` 1.2, 800-LNK-0041).
   - **`link`:** MIND Link's master, for the machines on the line at their addresses.
     - The operations are effector485's kinds (`hello`, `log`, `list`, `stat`, `get`, `put`, `mkdir`, `run`, `shell`, `reboot` and the rest), with their `reason` and `idempotency_key`.
     - Each machine's console is its channel 1. The bytes outside frames are its console text, as in effector485 1.1.
   - **The protocol** stays one: `linkproto`, with the master's state machine moved into it from `tools/mindlink` where it is not tied to `std`. Its no-`std` part is tested on the host against `mindlink serve`'s.
2. **The interface, `idl/serialhost.wit` 1.0:**
   - the lines, with their identity, mode and lease;
   - a lease: take, renew, release;
   - the mode;
   - the console: read from an offset, write with a sequence number, break;
   - the operations: submit, status, cancel.

   The names are this issue's proposal. They follow effector485's API, so that the Effector project's names and ours can meet (MC-12.4).
3. **Clients:**
   - **MIND Core's `effector`,** which offers the lines to the Effector server under the owner's policy: `EFF`'s part, asked in 100004's message;
   - **a program `term`,** for the owner at the machine's own console: `term lines`, `term attach <line>` as a terminal, and `term <line> <operation> …`.
4. **`init`'s part** (`KRN`): `serialhost` started with the converters its settings name, and its client lent to `effector` and to the shell.
5. **Tests:**
   - in QEMU: two machines on one virtual line (a socket chardev), the first as the master through `serialhost`, the second's `link` as the slave: `hello`, `put` and `get` of 64 KiB, `run`, the console;
   - then on hardware: the MacBook under MIND Core as the master of a board's line (100004-MOB-0004).

## Acceptance criteria

- **In QEMU:** the two-machine run of plan step 5 passes on x86 and on aarch64, in a suite of its own.
- **On hardware:** a MIND Core host reads a board's boot from U-Boot's banner on, and runs `hello`, `log`, `put` and `run` on it.
- **What is not done** is listed in the issue: several machines on one RS-485 pair, RTS-driven direction (MC-12.3).

## Related

[docs/remote-serial.md](../docs/remote-serial.md), [docs/link/effector485.md](../docs/link/effector485.md), [800-LNK-0039](800-LNK-0039-effector485-as-the-effector-agents-engine.md), [800-LNK-0043](800-LNK-0043-effector485-a-line-without-link.md), [100004-MOB-0004](100004-MOB-0004-effector-to-the-boards-over-their-uart.md).
