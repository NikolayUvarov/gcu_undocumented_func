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

The UEFI bootloader loads the kernel and three separate ELF applications from the FAT filesystem before handing control to the kernel shell.

* **Toolchain:** Pure Rust (`no_std`, `no_main`), utilizing `naked_functions` and `abi_x86_interrupt`, compiled for `x86_64-unknown-none` and `x86_64-unknown-uefi` targets.
* **Memory Layout:** The bootloader loads the `PT_LOAD` segments of `kernel.elf`, `app.elf`, `app2.elf`, and `clock.elf`, including zero-initialized memory. Their shared boot information and syscall mailbox layouts live in `common/abi.rs`.
* **Event Model:** Software interrupts (`int 0x80`) provide TSC reads, polled keyboard/UART input, UART output, and RTC time. Applications run synchronously and return to the shell on `Esc`.

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
  -m 512 -serial stdio -rtc base=localtime

```



*(Adjust the path to `OVMF.fd` depending on your OS and package manager).*

On Windows, use `03_run_qemu_windows.bat` or `03_run_qemu_windows_msys2.bat`; both enable the UART console with `-serial stdio` and initialize the RTC with the host's local time using `-rtc base=localtime`.

At the `MIND>` prompt, enter a command and press Enter (commands are case-insensitive):

* `LIST` — show all available program names and descriptions.
* `RUN app` — launch the original rotating-square application (`app.elf`). `BOOT` remains an alias for this command.
* `RUN app2` — launch the second application (`app2.elf`): a bouncing square, frame counter, and TSC value on screen. It prints a greeting and a status line every 30 frames to the host's QEMU console via UART.
* `RUN clock` — display a large digital clock (`clock.elf`) in 24-hour `HH:MM:SS` format, with time changes also printed to the UART console.
* `HELP` — list the available commands.

`RUN` requires a program name; without one it displays usage and the program list. Program names are case-insensitive, and surrounding whitespace is ignored. Unknown names leave you in the shell with an error message.

Press `Esc` in either the QEMU window or UART console to return to `MIND>`. Each `RUN app2` starts the second application's counter at zero. Rebuild all components together when changing `common/abi.rs`.

The clock reads the CMOS RTC through syscall 4, which returns seconds since midnight (or `usize::MAX` when unavailable). The kernel checks for stable readings and handles both BCD/binary and 12/24-hour RTC modes. The application displays RTC time without applying a timezone offset. The supplied launch commands use local time; QEMU otherwise defaults to UTC ([QEMU RTC options](https://www.qemu.org/docs/master/system/invocation.html)).

RTC conversion and rollover checks can also be run without booting the VM:

```bash
rustc --edition=2021 --test kernel/src/rtc.rs -o /tmp/mind-core-rtc-tests
/tmp/mind-core-rtc-tests
```
