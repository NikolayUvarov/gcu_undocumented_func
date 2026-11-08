# Requests for the MIND Link track (LNK), not numbered yet

**Owner:** MIND Link track · **Status:** open (2 waiting: the kernel session's link over the board's console UART, for the PinePhone, and effector485 as the engine of the Effector agent's link mode, both 2026-11-08) · **Recorded by:** the tools track (APP), 2026-10-30

The LNK track numbers its own tasks (`NNN-LNK-MMMM`), so requests from other tracks wait here. It turns each into a task and removes it from this file. The file is kept while empty because other issues link to it; a new request goes below this line.

## MIND Core's `link` over the board's console UART, for the PinePhone (for 100000, stage 0 and 1)

**Asked by** the kernel session, 2026-11-08, at the maintainer's word: the PinePhone's first test run is to boot and to work with Effector minimally, that is updates, logs and tests, before Wi-Fi exists there.

**Why the link.**
- The phone's only line out is UART0, through the headphone jack, at 115200 8N1. It is also the console.
- The agent inside MIND Core needs a network, and the phone has none until stage 3 (Wi-Fi over SDIO).
- `effector485` over the link gives, over one cable:
  - the console (channel 1, 800-LNK-0019's `link-console`);
  - files and the log;
  - programs started;
  - updates (351-KRN-0141, 351-LNK-0013).

**What it needs of LNK:**
- `link` taking a UART as its line, besides `usb_serial`. The UART is the board's console: a 16550 in memory on the A64 (`snps,dw-apb-uart`, 32-bit registers), a PL011 on QEMU's `virt`.
- On that line the link is the owner's console, since the shell cannot read the same UART. That is `link-console`, with the boot's first lines readable before the link answers.
- `mindlink` and `effector485` at 115200, the rate Tow-Boot leaves the line at, unless the link may change it after the handshake.

**What it needs of KRN** (the kernel session's part, made with it):
- the board's UART from the device tree (100000-PRT-0009);
- the 16550 driver in memory (100000-PRT-0010);
- `init` lending the UART's registers and line to `link` rather than to the shell, when `link.conf` says so (100000-KRN-0171).

**In QEMU first:** `virt` with U-Boot's UEFI and no ACPI (as Tow-Boot gives the phone), its PL011 on a pty, with `mindlink` and `effector485` on the host.

## effector485 as the engine of the Effector agent's link mode (for 100000 and the specification of serial ports through Effector)

**Asked by** the kernel session, 2026-11-08, at the maintainer's request of that day: a console relayed through Effector over RS-232 and RS-485, and the Effector agent on a Linux or Windows host reaching MIND Core through the link. The specification is [docs/remote-serial.md](../docs/remote-serial.md) ([Russian](../docs/remote-serial_RU.md)), version 0.1, a proposal. Its section 4 goes to the Effector project through EFF ([requests-EFF.md](requests-EFF.md), the same day). Its section 5.1 is this request.

**Why effector485.** The agent should carry no second implementation of the protocol. The specification proposes that it runs `effector485 daemon` for each port in link mode as a supervised child, and calls its API on the loopback. The Effector agent is written in Go.

**What it needs of LNK:**
- **A console stream for each machine** in effector485's API:
  - channel 1's output with byte offsets from a cursor;
  - input with sequence numbers, written once when sent twice;
  - the agent and effector485's own page both able to show it.
- **Bytes outside frames** (the boot's lines before `link` answers) delivered as console text with offsets.
- **A supervised mode:**
  - started by another program;
  - the loopback only, the key file required;
  - no page needed;
  - it ends when its parent ends.
- **A versioned API** the agent may depend on:
  - its version in a call (`GET /api/v1/version`, or in `hello`);
  - a list of changes;
  - a new version for a change of fields (MC-12.4);
  - the operation kinds in machine-readable form, for the host owner's policy.
- **The rule of 2026-10-30 in effector485.md** ("local, by design") with the maintainer's addition of 2026-11-08: local, and through Effector's host agent under that host owner's policy. Sessions still never reach the daemon's port directly.

**Nothing of this exists yet** (MC-12.3).
