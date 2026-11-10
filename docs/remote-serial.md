# Serial ports through Effector: the console over RS-232 and RS-485, and MIND Link to MIND Core

**Version:** 0.2 (2026-11-10), a proposal; 0.1 of 2026-11-08 · **0.2:** link mode refuses the signal controls (LNK, agreed with KRN); effector485's part of 5.1 is made (800-LNK-0039); section 4 is the Effector project's task 108 · **Asked by:** the maintainer, 2026-11-08 · **Written by:** the kernel session (`KRN`, `PRT`) · **Owners of the parts:** `EFF` (section 4: hands it over to the Effector project; section 5.2), `LNK` (section 5.1), the kernel session (section 5.3) · **Constitution:** MC-7.1, MC-7.4, MC-7.5, MC-7.6, MC-12.1, MC-12.3, MC-12.4 · **Russian version:** [remote-serial_RU.md](remote-serial_RU.md) (kept in sync; the English text is the reference)

**Nothing on this page exists yet.** It says what is to be built and by whom (MC-12.3). Each part's owner turns it into tasks and may change the design. A change of what this page calls decided goes back to the maintainer.

## 1. Why

- **The PinePhone ([100000](../issues/100000-a-phone-the-pinephone-first.md)) has one line out:** UART0, through the headphone jack, 3.3 V, 115200 8N1.
  - The phone has no network until its Wi-Fi works (stage 3).
  - MIND Core's own Effector agent (`effector`, main task [750](../issues/750-effector-agent.md)) needs a network, so it cannot serve the phone before then.
  - The first test run still has to update the system, collect its logs and run tests.
- **`effector485` does all that over a cable** ([docs/link/effector485.md](link/effector485.md)), but only locally, by design (the maintainer, 2026-10-30). A session on another machine asks `LNK` to run its work.
- **The maintainer's request (2026-11-08):**
  - a remote-control module, so that a console can be relayed through Effector over both RS-232 and RS-485;
  - the Effector agent on a Linux or Windows host reaches MIND Core through the link;
  - `EFF` explains to the Effector project what to build there, and hands it over.
- **Not only MIND Core.** A plain console through Effector also serves:
  - the phone's own firmware (Tow-Boot, U-Boot), which prints before MIND Core starts;
  - our loader's lines;
  - any other board with a serial console.

## 2. The picture

```
 an agent session, anywhere
        │  HTTPS, the session's key
        ▼
 the Effector server (hesere.uz/effector)
        │  the host agent's own channel and token
        ▼
 the Effector agent on a Linux or Windows host ──── its owner's policy (section 4.1)
        │ the serial module
        ├── "console" mode: raw bytes ── /dev/ttyUSB0, 3.3 V TTL ── the PinePhone's UART0
        │                                        (Tow-Boot, the loader, the kernel, then the shell)
        └── "link" mode: MIND Link's master, through effector485 (section 4.4)
                         └── COM22, an RS-485 pair ── MIND Core machines at addresses 1, 2, 3: `link`
```

- **The host** is a computer the maintainer or another owner registers with Effector, with a serial adapter plugged in.
- **The machine on the cable** is the target: a PinePhone, a board, a PC.
- **One port, one mode at a time** (section 3).

## 3. Two modes of a port

| | `console` | `link` |
|---|---|---|
| What goes over the line | the bytes as they are, both ways | MIND Link's frames ([protocol](link/protocol.md)), the host as the master |
| The other end | any system | MIND Core's `link`, or `mindlink serve` |
| What a session gets | a terminal: output from a cursor, input, break, DTR and RTS | effector485's operations (put, get, list, log, run, shell, reboot, replace and the rest) for each machine on the cable, and each machine's console through the link's channel 1 |
| Several machines on one line | no | yes, on RS-485, at their addresses |

- **The mode is switched explicitly,** by the session that holds the port's lease (section 4.3).
- **Bytes outside the link's frames** are shown as the console in `link` mode too. These are the boot's first lines, before `link` answers (section 5.1).

## 4. The Effector agent's part, for Linux and Windows

This is the part `EFF` hands over to the Effector project. The server's agents are written in Go (`agent/linux/worker.go` in `sst-test-deploy`). The names of calls below are this page's proposal. The Effector project chooses its own, and how they travel between the server and the agent.

### 4.1 Ports and the owner's policy

- **An inventory of serial ports**, reported with the agent's endpoints, each port with:
  - a stable identity: on Linux `/dev/serial/by-id/…` (USB vendor, product, serial number, interface), and the `/dev/ttyS*` that exist; on Windows the `COMn` name with the device's instance path (SetupAPI: vendor, product, serial number);
  - whether it is present, and whether it is in use and by whom (a session, or another program).
- **The owner's policy, a file on the host,** which nothing from the server can change. For each port:
  - whether it is offered at all, and its name for sessions (`phone-uart`, `rs485-bus`);
  - the modes allowed (`console`, `link`), and the line settings allowed (a list of speeds);
  - **whether a session may write to the console:** no, by default. A console is powerful: U-Boot's prompt rewrites the flash, and a shell runs commands;
  - **whether DTR, RTS and break may be driven:** no, by default, since they reset some boards;
  - **in `link` mode, the operation kinds allowed:**
    - by default only those that read: `hello`, `log`, `list`, `stat`, `get`;
    - those that write or need MIND Core's developer mode (`put`, `mkdir`, `move`, `delete`, `rmdir`, `run`, `install`, `uninstall`, `reboot`, `shell`, `replace`, `key`, `type`) only where the owner lists them;
  - the addresses on an RS-485 pair, and the IDs the machines get (section 4.4);
  - how much the capture keeps (section 4.3).
- **A port in use by another program is reported busy, never taken.**
  - Linux: `TIOCEXCL` and a lock on the device.
  - Windows: `CreateFile` with no sharing.
- **Linux:** the agent's user must be in `dialout`. That is a root step, done once by the host's owner. The setup notes say so.

### 4.2 The line

- **Settings:**
  - the speed: what the adapter takes, 300 to 3,000,000; 115200 by default. On Linux, `termios2` with `BOTHER` for speeds outside the standard list;
  - data bits 5 to 8; parity none, even, odd, mark or space; stop bits 1, 1.5 or 2;
  - flow control: none, RTS/CTS or XON/XOFF.
- **RS-485's direction, per port in the policy:**
  - `auto`: the converter switches it itself (FTDI's TXDEN, converters with automatic direction). This is the default, and on Windows the only one at first;
  - `rts`: RTS drives the transmitter, with delays before and after sending, in milliseconds. Linux: `TIOCSRS485`. Windows: `RTS_CONTROL_TOGGLE`;
  - an echo filter for two-wire converters that echo what they send.
- **When they apply.** The settings apply when the port opens. Only the holder of the port's lease changes them while it is open, and the session's state shows them.

### 4.3 The console session

- **Opening.** A call names:
  - the agent, the port and the mode `console`;
  - the settings;
  - a `reason` and an `idempotency_key`, as every Effector operation has.

  Several sessions may read a port. Only the holder of its lease writes.
- **The lease on a port** is like Effector's terminal reservations: one at a time, with a term, a renewal and a release, its owner and reason shown to all. The PIN is never logged.
- **What the port sends:**
  - It goes into a ring the agent keeps, with absolute byte offsets: 1 MiB a port at least, the oldest dropped first.
  - It also goes into a capture file on the host, rotated at the size the policy sets.
  - So nothing is lost while nobody watches. A reader that comes back reads from its cursor.
  - A reader asking from an offset already dropped gets the oldest bytes kept and the count lost (`gap`).
  - Each piece keeps the time it arrived (the host's clock, in milliseconds), so the console can be set against the machine's other events.
- **What a session writes:**
  - Each call carries bytes (base64) or text, and a sequence number.
  - The writes go out in order. A call sent twice is written once: replay is refused at the agent, the boundary that accepts the port's input (MC-7.5).
  - `pace_ms` between characters, for consoles that drop fast input. U-Boot at 115200 loses a pasted line without it.
  - A call carries at most 4 KiB.
- **Control:**
  - a break, with its length in milliseconds;
  - DTR and RTS set or pulsed;
  - the line's settings changed;
  - the buffers flushed.
- **Live:**
  - A stream for each session (a WebSocket, or the server's long polls) carries the output as it comes and takes input.
  - The agent forwards what arrived within 100 ms, in batches.
  - The aim is a typed key echoed within half a second across the server. This is to be measured and reported, not promised. A remote console is not a local one (MC-7.1): the session sees the line's state, and a lost connection is shown, never hidden.
- **The end:**
  - a call that closes it;
  - no reader for the policy's idle time (30 minutes by default);
  - the agent stopping;
  - the adapter unplugged: the session's state becomes `lost`, with the time. A replug of the same device (the same by-id or instance path) opens it again and says so.
- **The audit.** The agent records each write, break, DTR or RTS change and change of settings, with:
  - the session and its reason;
  - the key's owner.

  The record is the host owner's. It is kept on the host and read through the agent's logs.

### 4.4 The link mode

- **One implementation of the protocol stays,** `LNK`'s. The agent adds no second one.
  - **Proposed:** the agent runs `effector485 daemon <port> --supervised --baud … --addresses … --listen 127.0.0.1:<its port> --key-file <a file only the agent's user reads>` as a supervised child, one for each port in `link` mode, and calls its API ([effector485.md](link/effector485.md), "The API").
  - effector485 is built for Linux and Windows already (`tools/mindlink/build.sh`).
  - An agent written in Rust could link the `mindlink` crate instead (MIT OR Apache-2.0).
- **What the server sees:** each machine on the cable as an endpoint of its own.
  - Its ID is the host agent's ID with `link` or `link-<n>` after it. The exact form is the Effector project's choice.
  - Its state (online, name, last answer) comes from effector485's `GET /api/v1/endpoints`.
- **Operations:**
  - **The server's operation API** carries effector485's kinds with their fields (`link.put`, `link.run`, `link.log` …). The agent hands them to effector485 with their `idempotency_key` and `reason` as they are.
  - **What comes back:** the output and the status (`queued`, `running`, `completed`, `failed`, `cancelled`, `timeout`, `lost`).
  - **Files go in two hops:**
    - `put`: a file reaches the host through the server's own file transfer first, then effector485 sends it on. Its SHA-256 is checked on both hops;
    - `get`: the other way.
- **Leases.** Effector's lease on such an endpoint is effector485's lease on that machine, with effector485's rules:
  - the work of others waits in the queue, saying for whom;
  - reads pass.
- **The console in `link` mode** is the machine's channel 1 through effector485. The agent offers it as a console session with the API of section 4.3, over effector485's console stream (API 1.1, section 5.1).
- **The signal controls are refused on a port in `link` mode** (0.2, LNK, agreed with KRN on 2026-11-09):
  - a break, DTR and RTS set or pulsed, and any change of the line's settings are answered `denied`, with the reason "the link's frames share this line";
  - why: a break or a changed rate cuts the link's frames under way, and a converter that drives RS-485's direction from RTS stops sending;
  - the console session's output and input still pass, as channel 1.
- **Two locks:**
  - **On the host:** the agent refuses a kind its owner's policy does not list (`denied`, with the reason), before effector485 sees it.
  - **On the machine:** MIND Core's own `link.conf` (`run`, `dev`) still decides what runs there.

### 4.5 Safety

- **No new door into the host.**
  - The agent's existing token and channel to the server carry all of this.
  - effector485 listens only on the loopback, with a key file that only the agent's user reads.
- **Secrets.** A session's key and a lease's PIN never go into a URL or a log.
- **A stale holder is refused (MC-7.4).** Writes and control carry the lease's session. Once a lease ends or passes to another session, the agent refuses the old one's.
- **When the server is unreachable (MC-7.6):**
  - the agent keeps capturing and never sends anything to the line on its own;
  - a link operation that was running goes on to its end on the host. Its result is kept durably and reported once the server is back, before it is acknowledged, as the agent's other results are.
- **Vulnerabilities.** Their details go to the maintainer privately ([SECURITY.md](../SECURITY.md)).

### 4.6 Platforms and adapters

- **Linux:**
  - `termios2`, `TIOCSRS485`, `/dev/serial/by-id`;
  - the agent's user in `dialout`.
- **Windows:**
  - names: `\\.\COMn` (COM10 and above need the prefix);
  - I/O: overlapped I/O, with `SetCommState` and `SetCommTimeouts`;
  - control lines: `EscapeCommFunction` for DTR and RTS, `SetCommBreak` and `ClearCommBreak` for a break;
  - the device's identity from SetupAPI.
- **WSL.** A USB adapter belongs to Windows, so the agent on Windows holds it. `usbipd` only if the owner chooses it.
- **To test with:**
  - FTDI FT232R and FT232H, CH340, CP2102;
  - the maintainer's RS-485 converters (main task [800](../issues/800-mind-link.md), [800-LNK-0002](../issues/800-LNK-0002-usb-serial-ftdi-and-ch340.md));
  - the PinePhone's serial cable (headphone jack, **3.3 V** TTL only, 115200 8N1).

### 4.7 Tests in the Effector project

- **Without hardware:**
  - **The setup:** a pseudo-terminal pair (Linux) or com0com (Windows), with `mindlink serve` at the far end for `link` mode.
  - **The console:** an echo, a reader coming back from its cursor after a drop, the ring's `gap`, a write sent twice and written once, a stale holder refused, the far end closed as an unplug.
  - **The link:** a `put`, a `run` with its output, a `log` and a lease.
- **On a cable:**
  - two TTL adapters wired to each other;
  - two RS-485 converters on one pair, with two `mindlink serve` at addresses 1 and 2;
  - then the PinePhone.

## 5. MIND Core's part

### 5.1 `LNK`: the link and effector485

- **Asked already (2026-11-08, [requests-LNK.md](../issues/requests-LNK.md)):** `link` on a board's console UART, with:
  - the shell's console as channel 1 ([800-LNK-0019](../issues/800-LNK-0019-the-link-as-the-owners-console.md));
  - the boot's first lines readable before the link answers.
- **effector485 as the agent's engine** (asked with this page; in `main` through [800-LNK-0039](../issues/800-LNK-0039-effector485-as-the-effector-agents-engine.md), API 1.1: `GET /api/v1/version`, `GET /api/v1/console?agent_id=&from=`, `POST /api/v1/console/input` with `seq`, `daemon --supervised`):
  - **A console stream for each machine** in its API: channel 1's output with byte offsets from a cursor, and input with sequence numbers. The agent and effector485's own page can both show it.
  - **Bytes outside frames**, the boot's lines, delivered as console text with offsets.
  - **A supervised mode:**
    - started by another program;
    - the loopback only, the key file required;
    - no page needed;
    - it ends when its parent ends.
  - **A versioned API** the agent may depend on:
    - its version in `GET /api/v1/version` (or in `hello`);
    - a list of changes;
    - a new version for a change of fields (MC-12.4);
    - the operation kinds listed in machine-readable form, for the agent's policy.
- **The rule of 2026-10-30** in effector485.md ("local, by design") gets the maintainer's addition of 2026-11-08: local, and through Effector's host agent under that host owner's policy. Sessions still never reach the daemon's port directly.

### 5.2 `EFF`: the hand-over, and MIND Core's own agent

- **To the Effector project:** section 4, as a task of its own (beside its task 81 for the MIND Core agent). Handed over on 2026-11-10 as its `issues/108-serial-ports-console-and-mind-link.task.md` (sst-test-deploy f5eae2c), with 0.2's link-mode refusals; EFF's record is [100000-EFF-0026](../issues/100000-EFF-0026-serial-ports-through-effector.md).
- **Back from it:**
  - the names it chose, and how streams travel between the server and the agent;
  - the agent's revision;
  - the setup steps for a host's owner.
- **docs/effector.md** gets a section on serial ports once the server and agent serve them, with the machines that have a cable and what is on it.
- **Later, and not needed for the phone:**
  - MIND Core's own `effector` gives the same console contract, so that a MIND Core machine with a USB adapter can be the host of another machine's cable;
  - it would do so through `usb_serial` (`idl/serial.wit`).

### 5.3 The kernel session: the board's UART

- **The board's UART from the device tree:** [100000-PRT-0009](../issues-done/100000-PRT-0009-the-board-from-the-device-tree.done).
- **The 16550 in memory:** [100000-PRT-0010](../issues/100000-PRT-0010-a-16550-uart-in-memory.md). This covers the kernel's lines and the loader's.
- **The UART lent to `link`** rather than to the shell, when `link.conf` says so: by its node ([100000-KRN-0171](../issues-done/100000-KRN-0171-platform-devices-from-the-device-tree.done), done), from `init` ([100000-KRN-0221](../issues-done/100000-KRN-0221-init-starts-uart-serial-for-link.done)); on the A64, whose console shares its page, as the kernel's byte channel ([100000-KRN-0222](../issues/100000-KRN-0222-the-kernels-console-as-a-byte-channel.md)).
- **In QEMU first:** `virt` with U-Boot's UEFI and no ACPI (as Tow-Boot gives the phone), its PL011 on a pseudo-terminal. The agent's tests can drive that too.

## 6. Stages

| Stage | What | Enough for |
|---|---|---|
| S1 | `console` mode through Effector, one port on one host (sections 4.1 to 4.3) | seeing the PinePhone boot: Tow-Boot, the loader, the kernel; typing at U-Boot and at the shell |
| S2 | `link` mode with effector485 as the engine (4.4, 5.1) | updates, logs and tests on the phone: `put`, `get`, `log`, `run`, `reboot`, `replace` |
| S3 | several machines on one RS-485 pair; the direction driven by RTS | the maintainer's RS-485 bench |
| S4 | MIND Core as a host (5.2, later) | a MIND Core machine hosting another one's cable |

**Evidence** for each stage names its configuration: the host's system, the adapter, the speed and the machine on the cable (MC-12.1). A run on one adapter or host is not carried over to another.

## 7. Decided, and open

- **Decided by the maintainer (2026-11-08):**
  - through Effector;
  - RS-232 and RS-485;
  - the agent on Linux and Windows reaches MIND Core through the link;
  - `EFF` hands it over to the Effector project.
- **Open, for the owners and the maintainer:**
  - **The default policy:** whether console writing is on for the maintainer's own hosts. This page proposes off everywhere until the owner turns it on.
  - **The transport for streams** between the server and the agent: the Effector project's choice.
  - **Supervising effector485 or reimplementing the link** in the agent: this page proposes supervising.
  - **The endpoints' names** for the machines on a cable.
