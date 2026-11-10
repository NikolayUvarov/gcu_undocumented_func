# KRN ready: the DesignWare busy-detect on the kernel's 16550 console (100000-PRT-0010, for MOB)

**Ready:** `claude/100000-PRT-0010-busy-detect` 39a0e6b48

**From:** the kernel session (KRN, PRT, NET, UPD)

**What it lands** (on `main` 952a4e07):
- **The kernel** (`kernel/src/arch/aarch64/serial.rs`, one line of code, 7395 of 7400).
  - When it takes the console's interrupt, `console_take` reads IIR. At IIR 7, it reads USR (register 31).
  - That clears a DesignWare UART's busy-detect interrupt, which would otherwise hold the channel's line up on MOB's boards.
  - A plain 16550 never reads 7 there.
- **`tests/console16550_host.rs`**, added to `scripts/host_tests.sh`. It runs serial.rs against a register file at shifts 0, 1, 2 and widths 1, 2, 4.
  - It checks the byte sent, the bytes taken, THRE as room, IER, and that no other register is written.
  - It shows that USR is read for IIR 0xC7 and 0x07 and for no other value, by a fault on an unreadable page in a child process.
  - It fails against the kernel without the line.
- **Issue text:**
  - 100000-PRT-0010's progress;
  - 100000-KRN-0222's finding: the lines written through `PanicSerial` still reach the line while the channel stands, and a frame one falls into is resent by `link`.

**Tested** in the kernel session's container:
- the new host test;
- the kernel's line budget;
- "build (aarch64)";
- "aarch64: boot and fault containment": 20 checks passed;
- "aarch64: the board from a device tree, under U-Boot": 9 checks passed.

QEMU's `virt` has a PL011 only, so no QEMU suite runs the 16550 line. MOB's boards are its first run.
