# The first boot on an Allwinner board: the Orange Pi Zero Plus 2 H5 and the Orange Pi Zero 3

**Version:** 0.1 (2026-11-26) · **Issue:** [100004-MOB-0012](../../issues/100004-MOB-0012-the-boards-first-boot-step-by-step-and-its-run-script.md) · **Main task:** [100004](../../issues/100004-two-allwinner-boards-before-the-phone.md) (stage 1: [100004-MOB-0002](../../issues/100004-MOB-0002-the-first-boot-on-the-h5.md), [100004-MOB-0003](../../issues/100004-MOB-0003-the-first-boot-on-the-h618.md)) · **Constitution:** MC-7.1, MC-12.1, MC-12.2, MC-12.3 · **Russian version:** [allwinner-first-boot_RU.md](allwinner-first-boot_RU.md) (shorter; this one is the reference)

> **No board has booted MIND Core yet.** These are the steps for the first time, written before the boards are wired. What each step expects comes from the scripts, the code and the firmware's documentation, and from QEMU `virt` under U-Boot's UEFI ([section 8](#8-the-rehearsal-in-qemu)), which is evidence for QEMU only (MC-12.1). Where a step does not hold on the board, the board's issue records what was seen instead.

The bench's own board (the maintainer, 2026-11-26: a development board with components added) is not named yet ([issues-human, section 14](../../issues-human/README.md#14-the-two-allwinner-boards-on-the-bench)). Until it is, this page covers the two boards this track has. A third board gets a section here once it has an entry in the catalog of devices, `scripts/boards.toml`.

## 1. What is needed

| | The Orange Pi Zero Plus 2 H5 | The Orange Pi Zero 3 (H618) |
|---|---|---|
| Name in the catalog | `orangepi-zero-plus2` | `orangepi-zero3` |
| Power | 5 V, 2 A or more | 5 V, 2 A or more, through USB-C |
| The console | UART0, the 3-pin debug header, 3.3 V, 115200 8N1 | the same |
| Memory | 512 MiB | 2 GiB |
| Storage it boots from | the microSD card first (an 8 GB eMMC is on the board) | the microSD card first (no eMMC; a SPI NOR flash) |

Besides the board:
- **A microSD card** of 1 GB or more whose contents may be lost, and a **USB card reader**: the writer writes only a disk Windows calls a USB disk.
- **A 3.3 V USB-UART adapter.** MIND Core's own `usb_serial` drives the FTDI FT232R and the WCH CH340 (for later, when a MIND Core host holds the line); on Windows and Linux any adapter with a driver does.
- **Three wires,** and a resistor of about 1 kΩ for the adapter's TX (below).
- **A host:** the DEXP, the Ryzen or the MacBook, under Windows or Linux, with a terminal program (PuTTY on Windows; `picocom` or `screen` on Linux) or an Effector agent (section 6).

## 2. The card

From WSL on the maintainer's machine, as for a USB stick ([docs/write-disk.md](../write-disk.md), section 5):

```bash
./build-and-write.sh --list                             # the USB disks and their numbers: the card reader's card among them
./build-and-write.sh --board orangepi-zero3 <number>    # or --board orangepi-zero-plus2
```

- **It builds** the aarch64 system, packs its image and makes the card `dist/boards/mind-<board>.img`: U-Boot with TF-A at 8 KiB, built by `scripts/board_uboot.sh` from the sources `scripts/boards.toml` pins, and MIND Core's partitions from 4 MiB (100004-MOB-0001, 100004-MOB-0011).
- **It prints** the firmware's SHA-256 beside the catalog's, and the card's SHA-256. Keep both for the board's issue.
- **The writer asks** `ERASE <number>` before it writes, and reads the card back.
- **For the first boot, write it without `--link` and `--slots`.** With them, `link.conf` holds `uart`, the link takes the console UART, and the shell has no serial line ([mindlink.md](../link/mindlink.md), "On a board's UART"). That is stage 2's card (100004-MOB-0004).
- **The first time on a machine,** `scripts/board_uboot.sh` fetches U-Boot's and TF-A's sources and builds them (a few minutes); it says what to install if a tool is missing, or run `scripts/board_uboot.sh --fetch-tools <board>` once.

## 3. The wiring

- **The board's debug header to the adapter:** the board's TX to the adapter's RX, the board's RX to the adapter's TX, GND to GND. **Never the adapter's VCC.**
- **The pins' order** is printed beside the header and in Orange Pi's manual for the board. This page does not state it: it was not read in a source checked here. Check it on the board before wiring.
- **A resistor of about 1 kΩ in series with the adapter's TX:** with the power cut, the adapter's TX (3.3 V when idle) can feed the board through its RX and keep it half on (issues-human, section 14).
- **The adapter into the host's USB port.** Its port's name:
  - Windows: `COMn` in the Device Manager, under "Ports (COM & LPT)";
  - Linux: `/dev/serial/by-id/usb-…`, the stable name (`ls -l /dev/serial/by-id/`).
- **Record:** the adapter's chip, the host, the port's name. They go into the board's issue with the log (MC-12.1).

## 4. The first boot, as the maintainer does it

1. **Open the terminal** on the adapter's port at 115200 8N1, no flow control, with its log to a file:
   - PuTTY: Session → Serial, Logging → "All session output";
   - Linux: `picocom -b 115200 --logfile console.txt /dev/serial/by-id/usb-…`.
2. **Insert the card, then power the board on.**
3. **Watch the lines come,** in this order (section 7 has them with what each shows):
   - U-Boot's SPL, then U-Boot's banner `U-Boot 2026.10 …` and its countdown, `Autoboot in 2 s: type stop for the U-Boot prompt`. Do not type: it boots on;
   - our loader's lines, `BOOT: …`, and `… IMAGES CHECKED`;
   - the kernel's `MIND CORE KERNEL: BOARD FROM THE DEVICE TREE: …`, then `MIND CORE KERNEL: INIT STARTED`;
   - the services' lines, then the shell's prompt `MIND> `.
4. **At the prompt, type** each of these and Enter, waiting for the prompt between them:

   ```text
   cpus
   physmap
   free
   time
   time        (a few seconds after the first)
   reboot
   ```

5. **Watch the reboot:** `REBOOTING...`, `MIND CORE KERNEL: REBOOT VIA PSCI SYSTEM_RESET`, then U-Boot's banner again. Power the board off once it is back at the prompt or in U-Boot.
6. **Check the log:**

   ```bash
   python3 scripts/board_stage1.py --file console.txt
   ```

   It prints each criterion `MET` or `MISSING`, with the line that showed it, and exits 0 only when all are met.
7. **Send back** the log and the checker's output, with the board's revision, the card's SHA-256, the adapter, the host and its port, for 100004-MOB-0002 (the H5) or 100004-MOB-0003 (the Zero 3).

**If it stops:** to reach U-Boot's prompt (`=>`), type `stop` during its countdown; there `printenv` shows its settings and `boot` goes on. If nothing at all comes, check the wiring (TX and RX crossed, GND) and the rate. Stopped after `INIT STARTED` with `"Synchronous Abort" handler` from U-Boot: the kernel did not leave EL2 (100004-PRT-0014 is meant to prevent it). Whatever the last line is, it goes into the issue as it is.

## 5. The same, through a TCP line

Where the adapter's port is offered over TCP (a serial server, `ser2net`), or for QEMU (section 8):

```bash
python3 scripts/board_stage1.py --tcp 127.0.0.1:4555 --type --save console.txt
```

`--type` types section 4's commands itself, each at a new prompt, `time` twice 3 s apart, and compares the kernel's clock with the host's (within 10 % and 0.5 s). It stops when the reboot is seen, or after `--timeout` seconds (600).

## 6. As an agent does it

Two ways reach the console from another machine. **Neither has been tried on a board or on a real cable with MIND Core:**

| Way | What exists today | What does not |
|---|---|---|
| **The Effector server's serial ports** (the Effector project's task 108; [docs/remote-serial.md](../remote-serial.md), section 4) | The server's `/api/v1/serial/*` (server 1.2.0.260 or later), the Linux agent 1.2.0.105 or later and the Windows agent 1.2.0.163 or later, `console` and `link` modes, tested by the Effector project on pseudo-terminals (its `docs/serial-ports.md`, read on 2026-11-26, sst-test-deploy 127cc30) | Its cable acceptance (its task 108-04); a host whose owner's `serial-policy.json` offers a board's port; `docs/effector.md`'s serial section (100000-EFF-0026) |
| **effector485's raw console** (800-LNK-0043, API 1.2) | In `main`: a line that no `link` answers is a console that takes input, paced, with a break; tested on a pseudo-terminal with a fake U-Boot (`tools/mindlink/tests`) | A run on a board; it is local by design: another machine reaches it only through the Effector agent's link mode |

**Through the Effector server** (the host's owner first adds the port to `serial-policy.json` beside the agent, with `console_write: true` for typing; the server cannot change that file):

```bash
B=https://hesere.uz/effector
curl -s "$B/api/v1/serial/ports?agent_id=<host>" -H "Authorization: Bearer $EFFECTOR_KEY"
curl -s -X POST "$B/api/v1/serial/sessions" -H "Authorization: Bearer $EFFECTOR_KEY" \
  -d '{"agent_id":"<host>","port":"<the port's name>","mode":"console","reason":"stage 1 on <board>","idempotency_key":"<board>-stage1-1"}'
# reading only:
EFFECTOR_KEY=… python3 scripts/board_stage1.py --effector <session_id> --save console.txt
# typing too, as the lease's holder (POST /api/v1/serial/lease with "action":"acquire" gives lease_id and pin once):
EFFECTOR_KEY=… EFFECTOR_SERIAL_PIN=… python3 scripts/board_stage1.py --effector <session_id> --lease <lease_id> --type --save console.txt
```

- The script reads `GET /api/v1/serial/output` from offset 0 with `wait_ms`, so a boot that began before it started is read from the server's ring (1 MiB), with `gap` when older bytes are gone.
- Typing goes to `POST /api/v1/serial/input` with a growing `seq`, `pace_ms` 20; the key and the PIN go in no URL and no argument.
- A remote console is not a local one (MC-7.1): a pause in the session's output may be the network's, and the timer's comparison uses the times the host's agent stamped.

**Through effector485 on the host that holds the adapter** (a Linux or Windows host; port names as in section 3). effector485 is local by design: this is for an agent on that host, and a session elsewhere asks `LNK` to run it or goes through the Effector agent's link mode ([effector485.md](../link/effector485.md), "Local, by design"):

```bash
effector485 daemon /dev/serial/by-id/usb-… --baud 115200          # Windows: effector485.exe daemon COMn --baud 115200
curl -s http://127.0.0.1:8485/api/v1/endpoints                     # the machine's agent_id on the cable, and the line's mode
python3 scripts/board_stage1.py --effector485 http://127.0.0.1:8485 --agent <agent_id> --type --save console.txt
```

- While no `link` answers on the line, effector485 shows it in `console` mode, and typing goes to the line as it is ([effector485.md](../link/effector485.md), "A line no machine answers").
- It sends no `hello` while the line printed within the last 3 s, nor within 60 s of a typed key, so its frames stay out of U-Boot's countdown and our loader's window. A hello sent just before the board starts to print may still reach U-Boot: its countdown stops only on `stop`, so a frame does not stop it.

## 7. Stage 1's criteria as console lines

These are [100000](../../issues/100000-a-phone-the-pinephone-first.md)'s stage 1 and 100004-MOB-0002's step 3, as `scripts/board_stage1.py` checks them on the first boot's lines:

| Criterion | The line that shows it | What fails it |
|---|---|---|
| `u-boot` | `U-Boot 20YY.MM…` (U-Boot proper; its SPL's `U-Boot SPL …` comes before it on a board) | no banner before our loader's lines |
| `loader` | our loader's `… KEY … VERIFIED, N IMAGES CHECKED` | `BOOT ERROR: …`, `BOOT PANIC: …` |
| `kernel` | `MIND CORE KERNEL: BOARD FROM THE DEVICE TREE: GICV2 … TIMER PPI 27 CPUS 4 PSCI SMC … ENTERED AT EL2`, then `MIND CORE KERNEL: INIT STARTED` | `KERNEL PANIC: …`, U-Boot's `"Synchronous Abort" handler`, `DEVICE TREE: NO GIC`, `NO PSCI`, `NO CONSOLE` |
| `cpus` | `cpus`'s rows: `CPU=0 … ONLINE=true TICKS=…` for each of the 4, each ticking | `A CPU DID NOT COME ONLINE`, `PSCI CPU_ON REFUSED`, a board line with another count, a CPU offline or not ticking |
| `timer` | two answers of `time`, `UPTIME MS=…` growing, as fast as the host's clock where the host's times are known | the kernel's clock standing still, or off by more than 10 % and 0.5 s |
| `memory` | `physmap`'s last line, `RANGES=n FREE_RAM=nK`; `free`'s `MEMORY: n MIB` beside it | no range or no free RAM |
| `prompt` | the shell's `MIND> ` | — |
| `reboot` | `MIND CORE KERNEL: REBOOT VIA PSCI SYSTEM_RESET` after the prompt, then U-Boot's banner again | — |

The kernel's line on each board is expected to name what `tests/fdt_host.rs` reads from Linux's tree of the board: the GIC at 0x01C81000 (H5) or 0x03021000 (H618), the UART at 0x01C28000 or 0x05000000 as a 16550 with shift 2 and width 4, the timer's PPI 27, four CPUs, PSCI by SMC. U-Boot's own copy of the tree may differ; what the board prints is what counts.

## 8. The rehearsal in QEMU

QEMU has neither SoC. Its nearest stand-in is the CI group "aarch64: the board from a device tree, under U-Boot": `virt` without ACPI, GICv2, four Cortex-A53, U-Boot's UEFI (Ubuntu's `qemu_arm64` from `scripts/fetch_uboot.sh`) starting our loader at EL2. Here the disk is a board's card from `board_image.py` (the Zero 3's), so U-Boot finds the boot partition where the card has it, at 4 MiB; QEMU's U-Boot ignores the board's firmware at 8 KiB.

```bash
ARCH=aarch64 ./02_build.sh && python3 scripts/make_usb_image.py --arch aarch64 --no-build --force
python3 scripts/board_image.py --board orangepi-zero3 --image dist/mind-core-usb-aarch64.img --output card.img
qemu-system-aarch64 -machine virt,acpi=off,gic-version=2,highmem=off,virtualization=on -cpu cortex-a53 \
  -smp 4,sockets=1,cores=4,threads=1 -m 512 -bios "$(scripts/fetch_uboot.sh)" -device bochs-display \
  -device virtio-keyboard-pci -device virtio-tablet-pci -drive format=raw,file=card.img -snapshot \
  -display none -monitor none -serial tcp:127.0.0.1:4555,server=on,wait=on &
python3 scripts/board_stage1.py --tcp 127.0.0.1:4555 --type --save console.txt
```

**Run on 2026-11-26** (LEG9's WSL2, QEMU 8.2.2, TCG; U-Boot `2025.10-0ubuntu0.24.04.4`; the tree of `claude/MOB-boards` 1a0c83cc1796, its `[INIT] BUILD` line; the Zero 3's card, SHA-256 `70e39dc6…9480df`, with the firmware the catalog names, `73bcdf3f…38d842`): **all eight criteria met, twice**, each run about 9 s from the first banner to the next.

| | `-m 512`, a `bochs-display` (a GOP), the VirtIO keyboard and tablet | `-m 2G`, no display, as the Zero 3 has none |
|---|---|---|
| `u-boot` | `U-Boot 2025.10-0ubuntu0.24.04.4 (Oct 07 2026 - 20:58:59 +0000)` | the same |
| `loader` | `BOOT: MANIFEST 2c448b4c627fa694 KEY fd9adafc8d461c37 (THE TEST KEY) VERIFIED, 47 IMAGES CHECKED`, then `BOOT: VOLUME MBR PARTITION 1 AT LBA 8192`: the boot partition where the card has it | the same |
| `kernel` | `… BOARD FROM THE DEVICE TREE: GICV2 … PL011 … TIMER PPI 27 CPUS 4 PSCI SMC … ENTERED AT EL2`, `PROTECTION: PXN`, `INIT STARTED` | the same |
| `cpus` | four rows `ONLINE=true`, each with `TICKS` above 0 | the same |
| `timer` | `UPTIME MS=640`, then `3690`: 3.05 s on the kernel's clock, 3.01 s on the host's | 3.05 s and 3.01 s |
| `memory` | `RANGES=89 FREE_RAM=394580K`; `MEMORY: 385 MIB` | `RANGES=85 FREE_RAM=1967444K`; `MEMORY: 1921 MIB` |
| `prompt`, `reboot` | `MIND> `; `REBOOT VIA PSCI SYSTEM_RESET`, then U-Boot's banner | the same |

- **`UPTIME MS`** counts the kernel's ticks, so it shows the timer's interrupts arriving; `time`'s `MONOTONIC NS`, the generic counter, was 3.4 s ahead of it, the time before the kernel's tick started.
- **What this shows:** that the script reads such a console and decides each criterion, and that MIND Core's aarch64 image on this card's layout boots under U-Boot's UEFI at EL2 on four Cortex-A53 in QEMU `virt`. **It shows nothing of either board** (MC-12.1): `virt`'s PL011, GIC addresses, PSCI and memory are not a 16550's, the H5's or the H618's, and QEMU's U-Boot never runs the board's firmware at 8 KiB.
- The recorded console is `tests/data/stage1/qemu-virt-uboot-a53-512m.txt`, the host tests' good one.

## 9. What is known to be at risk on the boards

Each is read in the code or in a source, none seen on a board (MC-12.3):
- **The shell's console on a 16550:** both boards' console is a DesignWare 16550, and the shell's line is the kernel's byte channel there (100000-KRN-0222). That channel ran in QEMU on a PL011 only; the 16550's code has host tests only (100000-PRT-0010).
- **Entry at EL2:** U-Boot starts EFI applications at EL2 on these boards; the kernel goes on at EL1 (100004-PRT-0014, in QEMU with `virtualization=on`).
- **No disk after the loader:** the Allwinner MMC controller has no driver yet (100004-MOB-0005), so the system runs from what the loader read into memory, and its log does not reach the card's `MIND LOG`.
- **No device key:** a Cortex-A53 has no RNDR (100000-KRN-0279).
- **The watchdog:** the A31's, armed by the kernel (100004-PRT-0015); a board that stops resets after 16 s.
- **512 MiB on the H5:** in QEMU the system boots to the shell in 512 MiB (the CI group's memory); the board's map is smaller by what U-Boot and TF-A keep.
