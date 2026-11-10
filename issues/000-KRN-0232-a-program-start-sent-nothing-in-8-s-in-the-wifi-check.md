# 000-KRN-0232 — A program's start sent nothing in 8 s in the Wi-Fi check (round 114, x32)

**Type:** tests and the program start (shell, `loader`, `vfs_server`, `netpolicy`) · **Owner:** `KRN` · **Priority:** P2 · **Status:** open (step 1 made on `claude/000-KRN-0232-the-records-task-states`, 2026-11-10; the ten runs on x32 left) · **Blocked by:** — · **Main task:** — · **Roadmap:** track A · **Constitution:** MC-12.2

## Problem

**Seen once.** Round 114's gate, candidate 60ba280e (`main` 5349f065 with 307-KRN-0229), "x86: RAM above 4 GiB" on x32 (x32-128). The failure was in the `net` suite, `wifi_join_check`, `leased_and_pinged`.
- **What ran:** the shell ran `wifiping ping:10.0.4.1` (a copy of `netcheck`, with its own line in `netpolicy.txt`).
- **What came back:** nothing for 8 s, not even the shell's `STARTED PID=… NAME=wifiping FOREGROUND`. So the program never started in that time.
- **The record** (000-KRN-0214) shows all four CPUs halted (`HLT=1`): the system was waiting, not running. The console's end is the command's echo. The record is kept on x32-128 in `mind-core/KRN/logs/land-r114/x32-03/`.
- **The group passed when the gate ran it again alone,** on the same lane (1235 s).

**What starts a program, any of which may have waited:**
- the shell's call to the loader;
- the loader's read of `wifiping.elf` through `vfs_server`, which reads the boot disk through windows since 307-KRN-0229;
- the loader's flow from `netpolicy` for the program's line, which asks `netstack`;
- the spawn.

**Why 307-KRN-0229 is not the first suspect.** Every program start of every suite reads through the same path, and none other waited in this gate or in round 114's runs. But it changed that path, so it is not ruled out.

**In round 68** the same check failed differently: the program ran and answered `NoNetwork`.

## Plan

1. **Find what the start waited for.** The next failure keeps the record. Add to it what tells the shell, the loader, `vfs_server` and `netpolicy` apart: each one's state from `stat tasks` (IPC_WAIT on whom).
2. **Reproduce.** Run "x86: RAM above 4 GiB" ten times on x32 beside the other lane, with the record.

## Acceptance criteria

- The cause is named, with the record that shows it, or ten runs on x32 beside a busy lane pass with the added state, which then stays.

## Progress

**Step 1, the tasks' states in the record (2026-11-10).**

- **The kernel** (`kernel/src/scheduler.rs`) keeps a page of text, 16 KiB, with a line for each task: its PID, name, CPU and state.
  - It names what a waiting task waits for: the endpoint it receives on; the endpoint and the owner it sends to; the task whose reply it waits for; its interrupt line; or how long its sleep has left.
  - The boot CPU's tick rewrites the page once a second, under the scheduler's lock. That CPU's tick runs while every CPU is halted, so the page is at most a second old when a wait runs out.
  - At boot the kernel prints the page's address on the serial line alone: `MIND CORE KERNEL: TASK STATES AT 0x…, 16384 BYTES, EACH SECOND`. It is not shown on the screen or kept in the kernel's log.
- **The harness.** When a wait of 5 s or more runs out (000-KRN-0214), `tests/qemu_smoke.py` copies the page through QEMU's monitor. It tries `memsave` first, then `pmemsave`. It writes the page into the record between the CPUs' registers and the console's end. A stuck shell is never asked.
- **The checks:**
  - `tests/qemu_stuck_test.py` (host): the record holds the page's lines and nothing past its end; a page that cannot be read by its virtual address is read by its physical one; a kernel that printed no address is recorded as such.
  - The `normal` suite: the page read from a running VM has a line for `init` and the shell, and a service waiting to receive on an endpoint.

## Related

[307-KRN-0229](../issues-done/307-KRN-0229-vfs-servers-clients-bounded-to-its-own-partitions.done), [000-KRN-0214](../issues-done/000-KRN-0214-the-shell-stopped-answering-during-the-effector-screenshot.done) (the record), [000-KRN-0208](000-KRN-0208-a-run-after-forty-kills-waited-past-8-s.md) (another start that waited past 8 s, seen once).
