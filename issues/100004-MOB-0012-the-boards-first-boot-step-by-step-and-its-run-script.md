# 100004-MOB-0012 — The boards' first boot step by step, and a run script that checks stage 1 from the console

**Type:** documentation, script · **Owner:** `MOB` · **Priority:** P1 · **Status:** in progress (2026-11-26: the page, the script and its host tests made; the try in QEMU and the boards' runs left) · **Blocked by:** for the runs, the boards wired ([issues-human, section 14](../issues-human/README.md#14-the-two-allwinner-boards-on-the-bench)) · **Main task:** [100004](100004-two-allwinner-boards-before-the-phone.md) · **Constitution:** MC-7.1, MC-12.1, MC-12.2, MC-12.3

**Asked by:** the kernel session (`requests-MOB.md`, request 2's items 2 and 3), made for the bench's boards by the maintainer's word of 2026-11-26: «MOB будет делать работу на стенде - это не телефон, а плата разработки с добавленными компонентами». The PinePhone's own page (`docs/phones/pinephone.md`, Tow-Boot's installer) waits for the phone.

## Problem

The boards' first boots (100004-MOB-0002, 0003) are to need only a card written and a cable plugged in. Before this task:
- **nothing said, step by step,** how the maintainer or an agent boots a board the first time and what it should show;
- **stage 1's criteria** ([100000](100000-a-phone-the-pinephone-first.md), and 100004-MOB-0002's step 3) were written as words, not as the console lines that show them;
- **nothing checked them from a console:** a person would read the log by eye.

## Plan

1. **The page,** [docs/boards/allwinner-first-boot.md](../docs/boards/allwinner-first-boot.md), for both boards:
   - what is needed; the card from `build_and_write.sh --board` (100004-MOB-0011); the wiring from issues-human section 14; the host and its port;
   - the first boot as the maintainer does it, and the commands to type at the prompt;
   - as an agent does it, through the Effector server's serial ports and through effector485's raw console, saying which exists today and which is a plan (MC-12.3);
   - stage 1's criteria as console lines;
   - the rehearsal in QEMU, as evidence for QEMU only (MC-12.1).
2. **The run script,** `scripts/board_stage1.py`:
   - reads a console from a file, from standard input as a stream, from a TCP line, from effector485's console API (800-LNK-0043, API 1.2), and from an Effector server's serial session (`/api/v1/serial/output`, the Effector project's task 108);
   - checks each criterion on the first boot's lines: U-Boot's banner, our loader's lines, the kernel's board line and `INIT STARTED`, four CPUs online and ticking (`cpus`), the timer (`time` twice, against the host's clock when its times are known), the memory map (`physmap`), the shell's prompt, a reboot through PSCI and U-Boot again;
   - prints each met or missing with the line that showed it, and exits non-zero when one is missing;
   - with `--type`, on a source that takes input, types those commands at the prompt.
3. **Host tests,** `tests/board_stage1_test.py`, in `scripts/host_tests.sh`: a recorded console of QEMU and broken ones made from it, and stand-ins of a TCP line, effector485's API and an Effector server that answer each typed command with the recording's answer.
4. **The try in QEMU:** `virt` under U-Boot's UEFI at EL2, four Cortex-A53, GICv2, as the CI group "aarch64: the board from a device tree, under U-Boot" boots it, with the board's card as the disk.

## Acceptance criteria

- [ ] The page, with what exists and what is a plan said plainly.
- [ ] The script reads all five sources and decides each criterion with its line; the host tests pass in `scripts/host_tests.sh`.
- [ ] In QEMU under U-Boot's UEFI with four Cortex-A53, the script finds all eight criteria met. Evidence for QEMU only (MC-12.1).
- [ ] On each board, the script finds all eight met from its UART0 (100004-MOB-0002, 0003). Not done: no board is wired.

## Progress (2026-11-26)

- **Made:** the page and its shorter Russian version ([docs/boards/](../docs/boards/allwinner-first-boot.md)); `scripts/board_stage1.py`; `tests/board_stage1_test.py`, in `scripts/host_tests.sh`.
- **The host tests** (9) run on a console composed from the formats in the code, in a board's shape (`tests/data/stage1/composed-h5-shape.txt`; no board printed it), and on broken ones made from it: a CPU's row missing, a CPU offline, `A CPU DID NOT COME ONLINE`, `BOOT ERROR`, a stop after `INIT STARTED` with U-Boot's `"Synchronous Abort" handler`, a clock that stands still, no reboot, another CPU count, no free RAM, no prompt, a console read after U-Boot started. Stand-ins of a TCP line, effector485's console API and an Effector server's serial session answer each typed command; a clock at half speed fails the timer.
- **Read, not run:** the Effector server's serial API in the Effector project's `docs/serial-ports.md` and `server/serial.go` (sst-test-deploy 127cc30, 2026-11-24): the output's `chunks` with base64 `data`, the input's `session_id`, `lease_id`, `pin`, `seq`, `text`, `pace_ms`. The stand-in follows that reading; no real server was asked.
- **Left:** the try in QEMU (section 8 of the page: it waits for the build machine); the boards' runs.

## Related

[100004-MOB-0002](100004-MOB-0002-the-first-boot-on-the-h5.md), [100004-MOB-0003](100004-MOB-0003-the-first-boot-on-the-h618.md), [100004-MOB-0004](100004-MOB-0004-effector-to-the-boards-over-their-uart.md), [100004-MOB-0011](100004-MOB-0011-the-boards-word-in-build-and-write.md), [100000-EFF-0026](100000-EFF-0026-serial-ports-through-effector.md), [800-LNK-0043](800-LNK-0043-effector485-a-line-without-link.md), [docs/remote-serial.md](../docs/remote-serial.md).
