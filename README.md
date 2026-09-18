# MIND CORE

Iain M. Banks supposed # MIND CORE

The Minds of the Culture's spaceships — the ultra-powerful AIs controlling General Contact Units and Orbitals — must reside somewhere. No matter how multidimensional and advanced their hardware substrate may be, at a fundamental level, any computing architecture needs two basic things to wake up and become self-aware: a bootloader and an operating system.

Before a Mind can simulate pocket universes, juggle hyperspace vectors, or conduct delicate diplomatic games on behalf of an entire civilization, it requires a reliable and predictable foundation. It is time to start building it.

This repository is the first step toward creating a substrate-independent environment for future Minds. We are starting right now, from the very lowest level of hardware reality.

Every Mind needs its first processor tick. We are providing exactly that.


### Goals and Architecture

* **Substrate Initialization (Bootloader):** Direct interaction with UEFI, seizing control of the bare metal, and preparing physical memory.
* **Kernel:** The basic reality dispatcher. Handling hardware interrupts, system calls, and preemptive multitasking.
* **Isolated Environment (Userspace):** A space for the genesis and parallel execution of high-level processes and future cognitive functions.

---

### Current Runtime

The UEFI bootloader loads the kernel, preserves three original ELF application files from the FAT filesystem, and reserves a 64 MiB runtime heap before handing control to the kernel shell.

* **Toolchain:** Pure Rust (`no_std`, `no_main`), utilizing `naked_functions` and `abi_x86_interrupt`, compiled for `x86_64-unknown-none` and `x86_64-unknown-uefi` targets.
* **Memory Layout:** The bootloader loads and relocates `kernel.elf`. Each `RUN` loads a fresh copy of the selected application into the runtime heap: private `.data`, zeroed `.bss`, relocated GOT/function pointers, a 64 KiB stack, a syscall mailbox, input/log queues, and a screen buffer. The checked runtime ELF loader supports static x86-64 PIE with `R_X86_64_RELATIVE` relocations. Shared ABI layouts live in `common/abi.rs`.
* **Event Model:** A 100 Hz PIT interrupt preempts applications with round-robin scheduling on one CPU. Context switches preserve all general-purpose registers and x87/SSE state. Software interrupts (`int 0x80`) provide TSC reads, foreground input, output, RTC time, per-task sleep, uptime, and exit. The shell and interrupt handlers run without kernel preemption; they never allocate or free on a timer/syscall path. If no application is ready, the shell/idle task uses `HLT`. Input is polled on timer ticks and syscalls; only the foreground receives it.
* **CPU Configuration:** The supplied QEMU launchers expose one socket with four cores and one thread per core. The kernel currently executes on the bootstrap CPU; the other three CPUs remain parked. An SMP scheduler is not implemented yet. The bootstrap CPU uses the legacy PIC/PIT interrupt path with its local APIC disabled.

---

### How to Build and Run

**Prerequisites:**
You will need a nightly Rust toolchain and a virtual machine capable of UEFI execution.

```bash
rustup toolchain install nightly
rustup default nightly
rustup target add x86_64-unknown-none x86_64-unknown-uefi

```

*Note: Ensure QEMU and the OVMF firmware (UEFI for QEMU) are installed on your host system.*

**Build & Execution:**
The compilation and packaging pipeline is automated via bash scripts.

1. Ensure your build script is executable:
```bash
chmod +x 02_build.sh

```


2. Build the kernel, all three applications, and the UEFI bootloader. The script places the ELF files in `usb_root/` and the bootloader in `usb_root/EFI/BOOT/`:
```bash
./02_build.sh

```


3. Launch the environment in QEMU, pointing it to your UEFI firmware and the generated root directory:
```bash
qemu-system-x86_64 \
  -bios /usr/share/ovmf/OVMF.fd \
  -drive format=raw,file=fat:rw:usb_root \
  -m 512 -smp 4,sockets=1,cores=4,threads=1 \
  -serial stdio -rtc base=localtime

```



*(Adjust the path to `OVMF.fd` depending on your OS and package manager).*

On Windows, use `03_run_qemu_windows.bat` or `03_run_qemu_windows_msys2.bat`; both enable the UART console with `-serial stdio` and initialize the RTC with the host's local time using `-rtc base=localtime`.

At the `MIND>` prompt, enter a command and press Enter (commands are case-insensitive):

* `LIST` — show all available program names and descriptions.
* `RUN app` — launch a new foreground instance of the rotating-square application (`app.elf`). `BOOT` remains an alias for this command.
* `RUN app2` — launch the second application (`app2.elf`): a bouncing square, frame counter, and TSC value on screen. It prints a greeting and a status line every 30 frames to the host's QEMU console via UART.
* `RUN clock` — display a large digital clock (`clock.elf`) in 24-hour `HH:MM:SS` format, with time changes also printed to the UART console.
* `RUN <name> &` — launch a new background instance and retain the shell. Repeating the command creates independent instances with different PIDs. Up to eight application tasks can coexist.
* `PS` — show PID, program, state, foreground/background, scheduling count, CPU timer ticks, and syscall count. The shell/idle task has reserved PID 0.
* `FG <id>` — show an existing task's screen and route keyboard/UART input to it, preserving its PID and state.
* `KILL <id>` — terminate that instance; the shell then frees its image, stack, and screen.
* `LOGS <id>` — read and drain that instance's last 4096 bytes of buffered output. Foreground output is also printed to UART with a PID prefix; background output stays buffered so it does not interrupt command entry.
* `HEAP` — allocate and format a test string, release it, and report heap usage, free bytes, and whether the test allocation was freed.
* `HELP` — list the available commands.

`RUN` requires a program name; without one it displays usage and the program list. Program names are case-insensitive, and surrounding whitespace is ignored. Unknown names leave you in the shell with an error message.

Press **Ctrl+Z** in the QEMU window or send UART byte `0x1A` to return to `MIND>` while the foreground program continues in the background. Press **Esc** to end the foreground program and return to the shell. `FG` restores the existing screen; it does not restart the program. Each new `RUN` starts fresh application state. Rebuild all components together when changing `common/abi.rs`.

For example, starting from an empty session:

```text
run app &      # PID 1
run app &      # PID 2, separate animation/counter and globals
run clock &    # PID 3
ps
fg 1           # show the first square; Ctrl+Z returns to the shell
fg 2           # show the second square; Ctrl+Z returns to the shell
kill 1         # PID 2 and the clock keep running
logs 3
```

The comments above explain the example; the shell does not parse comments. Each application draws into its own RAM buffer. The shell compositor compares the foreground buffer with a cached copy and updates only changed physical pixels. Background tasks cannot paint over the shell or consume its input.

**Current boundary:** tasks still run in ring 0 in a shared address space, under the firmware's page tables/GDT. Private allocations prevent normal instances from sharing mutable state, but provide no hardware memory protection. Fault isolation, user mode, per-process page tables, kernel-owned GDT/TSS/exception stacks, and SMP remain future work. The supported target is `x86_64-unknown-none`, without a red zone or AVX; the current context switch saves x87/SSE, not AVX state.

The clock reads the CMOS RTC through syscall 4, which returns seconds since midnight (or `usize::MAX` when unavailable). The kernel checks for stable readings and handles both BCD/binary and 12/24-hour RTC modes. The application displays RTC time without applying a timezone offset. The supplied launch commands use local time; QEMU otherwise defaults to UTC ([QEMU RTC options](https://www.qemu.org/docs/master/system/invocation.html)).

Both square demos sleep between frames and redraw only changing areas in their own buffers. The clock sleeps between RTC checks and redraws when the second changes. Syscall 5 sleeps the calling task for `arg1` milliseconds (rounded up to a 10 ms tick, capped at 60 seconds), allowing other tasks to run and waking early for foreground input. Syscall 6 reports uptime in milliseconds; syscall 7 exits the calling task. Task resources are reclaimed only after control returns to the shell stack.

### Runtime checks

After building, run the host tests for real ELF images, independent `.bss`/relocations, malformed ELF rejection, scheduling/wakeup policy, and RTC handling:

```bash
rustc --edition=2021 --test tests/runtime.rs -o /tmp/mind-core-runtime-tests
/tmp/mind-core-runtime-tests
```

The QEMU integration test boots an isolated copy of `usb_root`, exercises concurrent instances, `fg`, `kill`, UART/PS2 input, task limits, repeated allocation/freeing, and idle `HLT`. A test-only application that never yields also checks timer preemption and SIMD context preservation:

```bash
rustc --edition=2021 --target x86_64-unknown-none --crate-type bin \
  -C opt-level=3 -C panic=abort -C relocation-model=pic \
  -Z relax-elf-relocations=yes -C link-arg=-Tapp/linker.ld \
  tests/busy_app.rs -o /tmp/mind-core-busy.elf
python3 tests/qemu_smoke.py --qemu qemu-system-x86_64 \
  --firmware OVMF.fd --busy-elf /tmp/mind-core-busy.elf
```

From WSL with the supplied Windows setup, use `--qemu /mnt/c/msys64/ucrt64/bin/qemu-system-x86_64.exe`. Test logs are written to the system temporary directory. The busy-loop ELF is used only inside the test VM; the normal packaged applications are unchanged by the test.
