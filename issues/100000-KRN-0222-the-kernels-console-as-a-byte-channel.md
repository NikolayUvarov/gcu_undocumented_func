# 100000-KRN-0222 — The kernel's console as a byte channel (the A64's page of four UARTs)

**Type:** kernel (ABI) · **Owner:** `KRN` (the kernel session) · **Priority:** P2 · **Status:** open (in `main` since round 102; the restarted shell's check in round 104, 9a0b0c8e; the end criterion's choice with 100000-KRN-0221, a 16550 and `link` left) · **Blocked by:** — (100000-KRN-0171 in `main` since round 99) · **Main task:** [100000](100000-a-phone-the-pinephone-first.md), stage 0 · **Roadmap:** track H · **Constitution:** MC-1.1, MC-2.3, MC-3.1, MC-12.4

## Problem

- **The A64's four UARTs share one 4 KiB page:** UART0 (the console, the headphone jack's), UART1 (the Bluetooth chip's) and UART3 are enabled on the PinePhone.
- **100000-KRN-0171 lends that page to none,** so neither `uart_serial` (100000-KRN-0221) nor anything else can drive UART0 by its registers.
- **LNK's answer** (2026-11-09): a byte channel from the kernel is enough for `link`. At 115200 the line moves about 11.5 KB/s, and a frame carries at most 1 KiB.

## Plan

The kernel has a ring and a blocking read already: a task's text input (`TEXT_QUEUE`, 4 KiB; `TEXT_READ` waits up to `TEXT_WAIT_MS` for bytes and answers `ERR_BUSY`, to be asked again; 000-KRN-0094). The channel feeds that input from the console's line, so the kernel grows by the console's interrupt and one call, not by a second ring and a second wait (MC-1.2). The owner revised the plan's first draft (its own ring and read) so, on 2026-11-09.

1. **The ring to `common/`** (`common/text_ring.rs`): the text input's ring as it is, shared by the kernel and a host test.
2. **`PLATFORM_CONSOLE`** (a platform call; ABI 29): `init` asks for the console as a channel for a task it started, by PID, where the console's node cannot be lent by its page.
   - **What the kernel does:**
     - it gives that task a text input if it has none, and feeds it from the console's receive interrupt;
     - it returns a `CAP_KIND_CONSOLE` capability, the channel's writer, for `init` to grant to that task;
     - it keeps the console's line itself (not lent while the channel stands) and turns the UART's receive interrupt on: 16550 `IER.ERBFI`, PL011 `IMSC.RXIM | RTIM`.
   - **Refused:** where the kernel drives no console it can read (x86's COM1 is lent by its ports), and while another channel stands.
3. **Writing:** `TEXT_WRITE` with the console capability queues the bytes in a second ring, and returns the count queued (0 when it is full). The kernel drains that ring from the transmit interrupt: 16550 `IER.ETBEI`, PL011 `IMSC.TXIM`.
4. **While the channel stands,** the kernel's own lines stop going to the console, as when the PL011 is lent (`MIRROR_LOGS`). The panic and stop lines still go, polled.
5. **When the holder ends,** the interrupts go off, and the line and the console's lines are the kernel's again.
6. **UART1 and UART3 stay the kernel's** (MC-2.3): no one drives Bluetooth's UART by sharing the console's page. Their nodes are lent to none, as now.
7. **Users:** `uart_serial` serves `serial.wit` over the channel on the A64 (100000-KRN-0221, after LNK's `uart_serial` is in `main`), and `link` does not change. Until then a test program echoes through it.

## Acceptance criteria

- **Host tests** of the ring: its bounds, a full ring, a timeout.
- **In QEMU,** with a tree whose console shares its page with another enabled UART (a test tree, or the forum's A64 fork):
  - the console's node is not lent;
  - the channel carries `link`'s frames at 115200;
  - the kernel's lines resume when the holder ends.
- **docs/api** names the capability, and the ABI's version is raised.

## Progress (2026-11-09)

- **Made** (dbad7079, d9bd4d90; ABI 29):
  - `common/text_ring.rs`: the text input's ring, out of `scheduler.rs`; `tests/text_ring_host.rs` (a full ring, the order across its end, empty buffers).
  - `PLATFORM_CONSOLE` and `CAP_KIND_CONSOLE`; `TEXT_READ` with the channel's handle in `msg[0]` (`TEXT_NOWAIT` in `msg[1]`); `TEXT_WRITE` with it.
  - `kernel/src/arch/aarch64/serial.rs`: the console's receive, room to send, and the interrupts of a PL011 or a 16550.
  - `init` asks for the channel when the console's node is no PL011 it can lend; `mind::dev::Uart` drives it as a third kind, so the shell's serial line runs over it unchanged.
- **The plan changed in one point:** the channel's receive ring is the console's own, not a task's text input. `init` grants the capability at the shell's start, before it knows the shell's PID, and the channel ends when no live task holds it.
- **Found on the way:** the receiver stalled after 16 bytes in QEMU. Every send cleared all of the PL011's interrupts, a pending receive one among them, and QEMU raises that one again only when its FIFO goes from empty to one byte. Only the transmit interrupt is cleared now.
- **Tested** in the kernel session's container (QEMU 8.2.2, TCG, AAVMF, `acpi=off`):
  - the `devicetree` suite: virt's own tree, dumped by QEMU, with a second UART's node in the console's page (`-dtb`);
    - **The node's kind:** first a PL011's. Round 101's gate showed that AAVMF on PCU drove that one as its console and hung on its empty registers, so it is a DesignWare UART's now (`snps,dw-apb-uart`, as the A64's UART1), which the firmware leaves alone.
  - the kernel lends the console's node to none (`NOT LENT: ITS PAGES AT 0x9000000 HOLD NODE 35'S REGISTERS (snps,dw-apb-uart)`), and `init` gives the shell the channel;
  - every command of the check came through it: `cpus`, a write of 219 bytes typed in one go, and 25 reads of a line of 201 bytes, more than a ring's worth back.
  - The host tests: `text_ring`, `runtime` (ABI 29).
- **The kernel's size:** 7394 lines of its budget of 7400.
- **Left:**
  - the gate;
  - a 16550 console (the A64's), which QEMU's `virt` has no model of; the phone's run;
  - `link`'s frames through the channel with `uart_serial` (100000-KRN-0221, after LNK's `uart_serial` is in `main`);
  - the kernel's lines after the holder ends, and a restarted shell getting the channel again: not tested yet.

**Landed** in round 102 (`main` c6931ded, 2026-11-09), after round 101 returned it for two of its tests (the handoff's host test, 200acdc5; the test tree's PL011 node that PCU's AAVMF hung on, 9c9529ab). Every group of the gate passed with it, PCU's `devicetree` suite with the channel check among them.

## Progress (2026-11-09, after round 102)

- **A restarted shell,** in the `devicetree` suite's channel check (`tests/qemu_smoke.py`, `console_channel_check`):
  - the shell is killed (`kill`), and init starts it again;
  - the new shell gets the same channel and answers through it (`cpus`);
  - between the kill and the new shell's prompt no line of init's reached the console;
  - init's log has the shell's end and restart, and no second request for the channel.
  - **Tested** in the kernel session's container (QEMU 8.2.2, TCG, AAVMF), the `devicetree` suite on aarch64: passed.
- **Found: the channel does not end when the shell ends.** init keeps what it minted for a service's first start, to give it again at a restart (`init/src/main.rs`, `Minted`), and the channel is among it. So:
  - a restarted shell gets the same channel, which is what a restart wants;
  - the channel stands for the whole boot, and the kernel's lines do not come back to the console when the shell ends.
- **The criterion "the kernel's lines resume when the holder ends"** cannot be met as written while init keeps a copy: the last holder is init, which never ends.
  - The kernel's end of the channel (`terminate`: no live task holds it) is in place but not reached in this system.
  - **The owner's choice, left open until 100000-KRN-0221:**
    - (a) the criterion becomes "a restarted holder gets the same channel" (tested above), and the kernel's end path stays for a holder init does not keep;
    - (b) init drops its copy, so the kernel's lines come back between a holder's end and its restart, and a restart asks for the channel again.
  - (a) keeps the console's bytes in one place across a restart. (b) shows the restart's lines on the console.
  - Until 0221 decides, the criterion is not counted as met.

**Landed** in round 104 (`main` 9a0b0c8e, 2026-11-09): the restarted shell's check. Every group of the gate passed with it, PCU's `devicetree` suite among them.

## Progress (2026-11-10): what still reaches the line while the channel stands

**Asked by `MOB`** for the two Allwinner boards (its message of 2026-11-10, point 3). On both boards `init` gives `uart_serial` this channel, because it lends a console's node only when it is a PL011 (`init/src/hardware.rs`).

**Found: more than the panic and stop lines still go.** Plan item 4 says only those go while the channel stands. In fact, every line the kernel writes through `PanicSerial` still goes to the UART at once (`kernel/src/main.rs`, `serial_print`), as it does while a PL011 is lent:

- a platform node not lent, because its page holds another node's registers (`scheduler.rs`, PLATFORM_MMIO);
- a trial boot's confirmation (`trial.rs`) and its restart;
- a task asking to restart or power off the machine;
- PCI's BAR lines and an MSI storm, which do not arise on these boards (no PCI, no GICv2m);
- the fatal reports.

**What `MIRROR_LOGS` stops** is the tasks' log lines and the kernel's log kept in the ring (`dmesg` still shows them).

**What a stray line costs `link`.** A line that comes while a frame is on the line corrupts that frame: `linkproto` frames by COBS with a CRC-32C, and drops a frame that fails it. The master then sends its request again, a bounded number of times (`linkproto/src/endpoint.rs`). So such a line costs one retry, not the link.

**What the channel's code has run on.** Only a PL011: QEMU's `virt` has no 16550, so the 16550 path has run nowhere.
- That path is the channel's three functions in `serial.rs`: LSR's data ready, LSR's THRE, and IER's bits 0 and 1.
- 100000-PRT-0010's host test covers it against a register file.

**Not changed, and offered to `MOB`:** holding those lines back while the channel stands, keeping only the fatal and stop lines (about one line of kernel code, against 5 left in the budget).

**Landed** in round 110 (`main` 91e74b59, 2026-11-10), with 100000-PRT-0010's busy-detect: the finding above of what still reaches the line while the channel stands. MOB keeps those lines on the line for the boards' first boots (its message of 2026-11-10, 0738Z).

## Related

[100000-KRN-0171](../issues-done/100000-KRN-0171-platform-devices-from-the-device-tree.done), [100000-KRN-0221](../issues-done/100000-KRN-0221-init-starts-uart-serial-for-link.done), [100000-PRT-0010](100000-PRT-0010-a-16550-uart-in-memory.md) (the 16550 the kernel drives).
