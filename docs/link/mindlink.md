# mindlink: the link's end on Linux and WSL

Russian: [mindlink_RU.md](mindlink_RU.md).

`mindlink` speaks the [MIND Link protocol](protocol.md) over a serial port. Task: [800-LNK-0004](../../issues/800-LNK-0004-mindlink-the-host-tool.md).
- **As master:** it moves files, reads the other end's log and types at its console.
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
| `ls` | the other end's files with their sizes |
| `stat <name>` | a file's size and SHA-256 |
| `rm <name>` | deletes a file |
| `log [-f]` | the other end's log; `-f` follows it |
| `console` | lines from standard input to the other end's console, its output back; Ctrl-D ends |
| `sniff [<seconds>]` | the line's raw bytes in hex as they come (default 10 s): on an RS-485 pair, listen on one converter while the other sends, to check the wiring and the rate |
| `board <line>` | one line to a [multi-device](multi-device.md) board's control port (`type …`, `key …`, `move …`, `status`), and its answer |
| `serve <dir> [--log <file>] [--console <command>]` | the slave: files in `<dir>`, the log read from `<file>`, a console running `<command>` through `/bin/sh -c`. Without `--console` there is no console |

**Options:**

| Option | Default |
|---|---|
| `--baud <rate>` | 115200. Rates from 9600 to 3000000 that Linux names (921600, 2000000 and others). Both ends must use the same rate |
| `--timeout <ms>` | how long one try waits: from the rate on a serial port, 2 s on TCP |
| `--tries <n>` | 8 tries for each request |
| `--turnaround <us>` | `serve`: 500 µs before each answer, for an RS-485 converter to turn the line round |
| `--payload <bytes>` | 1024: the largest frame's payload, from 64. Less for a noisy line, and 256 for QEMU's emulated converter (below) |

Each file moved is reported with its size, time, rate and SHA-256. A refusal or a missing answer is reported with its reason, and the exit status is 1.

## The converters on Windows

`mindlink.exe` drives a converter as Windows names it, with no usbipd:

```text
mindlink.exe --baud 921600 COM10 hello
mindlink.exe --baud 921600 COM10 put build.img
```

- **Device Manager** shows each converter's COM number.
- **An FTDI port's latency timer** is the port's Advanced property, 16 ms by default; 1 ms speeds up every answer.
- **A port in WSL is not in Windows.** A converter attached to WSL through usbipd is gone from Windows until it is detached.

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
- against a stopped server, which must fail in bounded time.

`scripts/host_tests.sh` runs them. They say nothing about a real converter or cable (MC-12.1).
