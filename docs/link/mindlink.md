# mindlink: the link's end on Linux and WSL

Russian: [mindlink_RU.md](mindlink_RU.md).

`mindlink` speaks the [MIND Link protocol](protocol.md) over a serial port. Task: [800-LNK-0004](../../issues/800-LNK-0004-mindlink-the-host-tool.md).
- **As master:** it moves files and directories, starts programs and reads their output, reads the other end's log and types at its console.
- **[effector485](effector485.md)**, built beside it, queues such work as operations for agents to hand over and come back to.
- **As `serve`:** it plays the slave over a directory, so the line can be tested between two Linux ends before MIND Core's end (800-LNK-0003) exists.

## Build

```bash
tools/mindlink/build.sh
# Linux and WSL:  tools/mindlink/target/release/mindlink
# Windows:        tools/mindlink/target/x86_64-pc-windows-gnu/release/mindlink.exe
```

- **It uses the standard library and `linkproto` only.**
- **On Linux** the serial port is set through glibc's termios.
- **On Windows** a COM port is set through the Win32 communications calls. The program needs no DLL beyond Windows' own.
- **The Windows build** is cross-compiled. It needs `rustup target add x86_64-pc-windows-gnu` and MinGW-w64's linker.

## Commands

```text
mindlink [options] <port> <command> [arguments]
```

Options come before the command; what follows the command is its own, a program's arguments too.

**Names** on the other end are paths below its directory, their parts joined by `/` (`apps/test/x.elf`), none of them `.` or `..` ([protocol, section 5](protocol.md#5-requests)). An older end takes one part only.

**Firmware on MIND Core** (800-LNK-0027). With `dev` in `link.conf`, init lets `link` write `data/firmware/` too (800-KRN-0173). There it is the name `firmware/`:
- `put`, `get`, `stat`, `ls`, `mkdir`, `rm` and `rmdir` work there;
- the top's `ls` lists `firmware/` last;
- a move between `firmware/` and the rest is refused, as are a change of `firmware` itself and `run` or `replace` of a name there;
- without `dev` such names are unsupported.

A driver's proprietary file thus goes to a running machine with no disk written again: `scripts/proprietary.sh link PORT`, or `scripts/proprietary.sh link effector485` through the daemon. The driver reads it at its next start (a restart, or `replace <unit> --back`). `rtw88_wifi`, `rtw89_wifi` and `bt_hci` refuse a file whose SHA-256 is not the one they pin. `bcm_wifi` only logs its microcode's SHA-256, for a file from the cable as for one from the disk.

**Ports:**

| Port | What it is |
|---|---|
| `/dev/ttyUSB0`, `COM10` | a serial port on Linux or Windows, set raw (8 bits, no parity, no flow control) at `--baud` |
| `tcp:HOST:PORT` | a TCP connection, for example to QEMU's serial chardev |
| `listen:PORT` | waits for one TCP connection; `listen:0` picks a port and prints it |
| `pty` | `serve` only, Linux: a pseudo-terminal for tests; prints its path |

**Commands:**

| Command | Does |
|---|---|
| `hello` | the other end's name, protocol version and payload size; on an FTDI port, warns when the latency timer slows answers |
| `put <file> [<name>] [--force]` | sends a file. Skipped when the other end has the same bytes (its SHA-256), unless `--force`. An interrupted put of the same name and size resumes |
| `get <name> [<file>]` | fetches a file into `<file>.part`, renamed once its SHA-256 matches |
| `ls [<dir>]` | a directory's files with their sizes, and its directories, ending in `/`; the top without `<dir>` |
| `stat <name>` | a file's size and SHA-256 |
| `rm <name>`, `delete <name>` | deletes a file |
| `mkdir <dir>` | makes a directory and its missing parents |
| `mv <from> <to> [--force]` | moves or renames a file or a whole directory; `--force` replaces a file at `<to>` |
| `rmdir <dir> [-r]` | removes an empty directory; `-r`: with everything in it |
| `run <program> [<argument>...]` | starts a program in the other end's directory and shows its output until it ends. Exits with its exit code where the other end knows it, and 1 after a fault. On MIND Core, `link.conf` must hold the word `run` |
| `start <program> [<argument>...]` | the same, printing only the run's number and the program's PID |
| `sh <line>` | one line for the other end's shell, and its output; exits 1 when the shell refused or failed it. MIND Core: `dev` in `link.conf` (800-LNK-0015) |
| `replace <unit> <file> [<name>]`, `replace <unit> --back` | puts the file (as `replace/<its name>`) and starts the other end's service from it, unverified; `--back` returns it to its boot image. MIND Core: `dev` in `link.conf` (800-LNK-0016) |
| `out <run> [-f]` | a run's output that the other end still holds; `-f` follows it to its end |
| `runs` | the runs the other end knows: number, PID, state, program |
| `stop <run>` | ends a run's program |
| `reboot` | restarts the other end's machine. On MIND Core, `link.conf` must hold the word `dev` |
| `log [-f]` | the other end's log; `-f` follows it |
| `console` | lines from standard input to the other end's console, its output back; Ctrl-D ends |
| `sniff [<seconds>]` | the line's raw bytes in hex as they come (default 10 s): on an RS-485 pair, listen on one converter while the other sends, to check the wiring and the rate |
| `board <line>` | one line to a [multi-device](multi-device.md) board's control port (`type …`, `key …`, `move …`, `status`), and its answer |
| `serve <dir> [--log <file>] [--console <command>] [--run]` | the slave: files in `<dir>` and its directories, the log read from `<file>`, a console running `<command>` through `/bin/sh -c`. Without `--console` there is no console. `--run` lets the master start programs in `<dir>`, as the user who serves: as good as a shell for whoever holds the cable. A program put over the link gets its owner's execute bit when it is run |

**Options:**

| Option | Default |
|---|---|
| `--baud <rate>` | 115200. Rates from 9600 to 3000000 that Linux names (921600, 2000000 and others). Both ends must use the same rate |
| `--timeout <ms>` | how long one try waits: from the rate on a serial port, 2 s on TCP |
| `--tries <n>` | 8 tries for each request |
| `--turnaround <us>` | `serve`: 500 µs before each answer, for an RS-485 converter to turn the line round |
| `--payload <bytes>` | 1024: the largest frame's payload, from 64. Less for a noisy line, and 256 for QEMU's emulated converter (below) |
| `--address <n>` | none: a line with one machine. On a line shared by several (below): the other end's address, 1 to 247; with `serve`, this end's own |

Each file moved is reported with its size, time, rate and SHA-256. A refusal or a missing answer is reported with its reason, and the exit status is 1.

**The mark on MIND Core's screen** (800-LNK-0020). While a master is heard over the link, MIND Core's `link` lights a ring with a dot in the screen's top right corner:
- **red** with `dev` in `link.conf`: the master can change the machine;
- **yellow** without it: the master can only read;
- **grey** once the master has been quiet for 5 s. The mark goes away when `link` stops.

The compositor draws the mark, so no screen capture holds it and no other program can light it (800-KRN-0156). `effector485` keeps the line warm with a POLL every 2 s, so the mark stays lit between its operations. A `mindlink` command lights it only while it runs.

## The converters on Windows

`mindlink.exe` drives a converter as Windows names it, with no usbipd:

```text
mindlink.exe --baud 921600 COM10 hello
mindlink.exe --baud 921600 COM10 put build.img
```

- **Device Manager** shows each converter's COM number.
- **An FTDI port's latency timer** is the port's Advanced property, 16 ms by default; 1 ms speeds up every answer.
- **A port in WSL is not in Windows.** A converter attached to WSL through usbipd is gone from Windows until it is detached.
- **From WSL, through a Windows port:** `tools/mindlink/mindlink-exe.sh` runs `mindlink.exe` (`MINDLINK_EXE`, or the one on the PATH). It passes each argument that names a file here as its Windows path. `mindlink.exe` reads WSL's files as `\\wsl.localhost\…`.
  - Scripts that call `mindlink` use it so.
  - An update through the link (351-KRN-0141) on the MacBook Pro, 2026-10-21:

    ```bash
    MINDLINK_EXE=/mnt/c/…/mindlink.exe scripts/link_update.py COM22 --mindlink tools/mindlink/mindlink-exe.sh \
        --mindlink-option=--baud --mindlink-option=460800
    ```

## The converters on WSL

WSL 2 does not see Windows' COM ports. A USB converter reaches WSL through [usbipd-win](https://github.com/dorssel/usbipd-win), and Windows then no longer sees it.

1. **Install usbipd once,** in Windows:
   ```powershell
   winget install usbipd
   ```
2. **Find the converters** (an elevated PowerShell):
   ```powershell
   usbipd list
   ```
   - The FTDI FT232R is `0403:6001`; on the maintainer's PC it was COM4.
   - The WCH CH340 is `1a86:7523`; there it was COM10.
3. **Share each one once,** by its bus ID from the list:
   ```powershell
   usbipd bind --busid <BUSID>
   ```
4. **Attach it to WSL** each time it is plugged in. `--auto-attach` keeps it attached:
   ```powershell
   usbipd attach --wsl --busid <BUSID>
   ```
5. **In WSL,** load the drivers if the port does not appear, and allow your user to open it:
   ```bash
   sudo modprobe ftdi_sio; sudo modprobe ch341
   ls -l /dev/ttyUSB*
   sudo usermod -aG dialout "$USER"   # then log in again; or for now: sudo chmod 666 /dev/ttyUSB0
   ```
6. **On an FTDI port, set the latency timer to 1 ms.** The driver otherwise holds short answers up to 16 ms each:
   ```bash
   echo 1 | sudo tee /sys/bus/usb-serial/devices/ttyUSB0/latency_timer
   ```

## RS-485 wiring

- **Two wires,** A to A and B to B, and the ground between the two machines when they share none. If nothing comes through, swap A and B: makers label them differently.
- **On a long cable,** a 120 Ω resistor across A and B at each end. A short desk cable works without.
- **The line is half-duplex.** The protocol never has two senders. Converters that switch direction by themselves (the FT232R's TXDEN, the CH340 modules) need nothing more.

## Several machines on one line

An RS-485 pair can carry up to 32 standard loads (more with fractional-load converters), so one converter at the master reaches several machines (800-LNK-0026, the protocol's version 2).

- **The wiring:** one pair from machine to machine, A to every A and B to every B, with short stubs to each converter. The 120 Ω resistors go only at the pair's two far ends, however many machines sit between them.
- **Each machine gets an address,** 1 to 247, no two alike:
  - MIND Core: `address=<n>` in `link.conf` (`scripts/build_and_write.sh --link-address=<n>`). `link` logs `[LINK] ADDRESS <n> ON A SHARED LINE` and then answers only frames to it;
  - `mindlink serve --address <n>`.
- **The master names the machine** in each command: `mindlink --address 2 /dev/ttyUSB0 log`. Commands to different machines take turns on the line, as the protocol answers one request at a time.
- **One machine without an address** may stay on the pair, as every machine before version 2 is. Commands without `--address` reach it alone.

```bash
mindlink --address 1 --baud 460800 /dev/ttyUSB0 hello     # the MacBook, address=1 in its link.conf
mindlink --address 2 --baud 460800 /dev/ttyUSB0 log       # a second machine on the same pair
```

Evidence: the host tests below run three addressed slaves and one without an address on a simulated shared line. A real pair's is recorded in 800-LNK-0026 when measured (MC-12.1).

## Two converters back to back

The first test of the line (800-LNK-0004's acceptance), with both converters on one PC and wired to each other:

```bash
mindlink --baud 921600 /dev/ttyUSB1 serve ~/link-srv &        # one converter is the slave
head -c 10M /dev/urandom > /tmp/10M.bin
mindlink --baud 921600 /dev/ttyUSB0 hello
mindlink --baud 921600 /dev/ttyUSB0 put /tmp/10M.bin
mindlink --baud 921600 /dev/ttyUSB0 get 10M.bin /tmp/10M.back
cmp /tmp/10M.bin /tmp/10M.back
```

**What to expect,** an estimate rather than a measurement: a frame takes about 10 bits per byte on the line, so 921 600 baud carries at most about 90 KB/s. Each 1 KiB request waits for its answer, so the result is lower. The rates a real pair reaches are recorded in 800-LNK-0004 when measured.

## MIND Core in QEMU

QEMU's `-device usb-serial` emulates an FTDI FT232, which MIND Core's `usb_serial` drives. With its line on a pseudo-terminal, `mindlink` reaches the `link` service as it would through a converter:

```bash
qemu-system-x86_64 ... -device qemu-xhci,id=xhci -chardev pty,id=link -device usb-serial,bus=xhci.0,chardev=link
# QEMU prints: char device redirected to /dev/pts/N (label link)
mindlink --payload 256 /dev/pts/N hello
```

- **Use `--payload 256` there.** The emulated chip keeps 384 bytes, and QEMU fills it again from the pseudo-terminal only when its main loop wakes for something else. A larger frame then waits half a second or more for its tail.
- **The device is attached only while its chardev is open.** With `-chardev socket,...,server=on`, QEMU attaches the device when a client connects and detaches it when the client leaves. A pseudo-terminal stays open.

## Tests

`tools/mindlink/tests/mindlink.rs` runs `mindlink` against its own `serve`:
- over a pseudo-terminal, the termios path: every command, with 1.5 MB each way;
- over TCP;
- against a stopped server, which must fail in bounded time;
- a tree made, moved and removed, with `ls` at each step; programs started with `serve --run`, their output and exit code, a stop; none started without `--run`.

`tools/mindlink/tests/bus.rs` runs three `mindlink serve` with addresses 1 to 3 and one without, all on one simulated shared line (a TCP hub that repeats each end's bytes to the others): each address is answered by its own slave alone, the one without by itself, an unknown address by none.

`tools/mindlink/tests/effector485.rs` drives [effector485](effector485.md) the same way.

`scripts/host_tests.sh` runs them. They say nothing about a real converter or cable (MC-12.1).
