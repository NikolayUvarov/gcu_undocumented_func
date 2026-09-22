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

The UEFI bootloader loads the kernel, preserves four original ELF application files from the FAT filesystem, enumerates enabled CPUs through UEFI MP Services, and reserves a 64 MiB runtime heap, a BSP stack, and a low-memory AP bootstrap page. The kernel takes over after ExitBootServices.

* **Toolchain:** Pure Rust (`no_std`, `no_main`), utilizing `naked_functions` and `abi_x86_interrupt`, compiled for `x86_64-unknown-none` and `x86_64-unknown-uefi` targets.
* **Memory and privilege:** Every application runs in ring 3 with IOPL=0 and a private four-level page table/CR3. Each `RUN` creates a fresh ELF image, 64 KiB user stack with unmapped guard pages, syscall mailbox, input/log queues, and screen buffer. Code is RX; writable data, stack, mailbox and screen are NX. User allocations occupy complete private pages. The kernel's supervisor mappings are inaccessible to applications. ELF relocations use user virtual addresses, not physical RAM addresses.
* **Dynamic program memory:** Syscalls 8/9 allocate and free zeroed private page blocks, with RW+NX permissions and trailing guard pages. Each process can own up to 32 blocks and 16 MiB of heap data. A common Rust ownership wrapper releases blocks on drop; the kernel also reclaims outstanding allocations on exit, kill or fault. `app2` uses this API for its sprite buffer.
* **CPU configuration:** The QEMU launchers expose four cores. The kernel starts APs itself with INIT/SIPI and can use up to eight enabled xAPIC CPUs. Each CPU has its own GDT, TSS, interrupt-entry stack, idle stack, and double-fault/NMI stacks. `cpus` reports actual online CPUs and interrupt counters.
* **Scheduling:** New tasks are assigned to the least populated CPU and remain pinned there. Round-robin scheduling on each CPU preserves GPRs and x87/SSE state. The BSP receives the 100 Hz PIC/PIT tick through LAPIC ExtINT and sends scheduling IPIs to online APs. Applications execute concurrently; a spinlock with local interrupts disabled serializes scheduler/syscall work. Kernel code is not preempted. Idle CPUs use `HLT`.
* **System calls and faults:** The DPL-3 `int 0x80` gate is the only user entry into kernel services. Every output-buffer pointer is checked against the calling task's user mappings, including overflow and page boundaries. SYSENTER/SYSCALL firmware entry paths and AVX/XSAVE are disabled. User exceptions terminate that task and appear in `faults`; a kernel exception remains fatal. Reclamation waits until no CPU is running the task and its CR3 is no longer active.


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


2. Build the kernel, all four applications, and the UEFI bootloader. The script places the ELF files in `usb_root/` and the bootloader in `usb_root/EFI/BOOT/`:
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

From WSL with Windows interop enabled, build and launch Windows QEMU directly:

```bash
./02_build.sh && ./03_run_qemu_wsl.sh
```

`03_run_qemu_wsl.sh` uses the same VM settings as the Windows launchers. It looks
for QEMU in MSYS2 UCRT64, then MinGW64, then `C:\Program Files\qemu`, then `PATH`.
It locates `OVMF.fd` and `usb_root/` beside the script and converts their paths
with `wslpath`, so it can be invoked from any directory. To use another Windows
installation, set `QEMU=/mnt/d/path/to/qemu-system-x86_64.exe`. Any script
arguments are passed through to QEMU.

### Bootable USB image

Run from Linux, WSL or MSYS2 with the project's Rust toolchain, Bash, Python 3
and `qemu-img` installed (the latter comes with QEMU):

```bash
./04_make_usb_image.sh
```

This rebuilds all components and creates **`dist/mind-core-usb.img`**, a complete
raw disk image of approximately 504 MiB. The image contains an MBR with a UEFI
system partition, a FAT16 filesystem labelled `MIND CORE`, and these files:

```text
EFI/BOOT/BOOTX64.EFI
kernel.elf
app.elf
app2.elf
clock.elf
dzenclk.elf
```

The script uses QEMU's virtual FAT image conversion, then independently checks
the partition/filesystem and compares every packaged file with the build output.
Only the six required files are packaged; temporary test disks in `usb_root/`
are excluded. It prints the image's SHA-256. Root/administrator access is not
needed, and the script does not write to physical disks.

Write the **entire `.img` to the USB device in RAW/DD mode** with your chosen
image-writing tool; copying the `.img` as a file onto the drive will not make it
bootable. Writing the image replaces the drive's partition table and data.
Use a drive of at least 512 MiB, select **UEFI x64** in the boot menu, and disable
**Secure Boot** for this unsigned loader. Legacy BIOS boot is not supported.
FAT16 is supported for UEFI removable media; the fallback loader path is
`EFI/BOOT/BOOTX64.EFI` ([UEFI media specification](https://uefi.org/specs/UEFI/2.11/13_Protocols_Media_Access.html)).

Options:

```bash
./04_make_usb_image.sh --force                 # rebuild and replace an existing image
./04_make_usb_image.sh --no-build --force      # package the existing usb_root/
./04_make_usb_image.sh --output /tmp/mind.img  # choose another output file
./04_make_usb_image.sh --qemu-img /path/to/qemu-img
```

`QEMU_IMG` can also specify the executable. Under WSL, the script automatically
checks the standard MSYS2 UCRT64/MinGW64 and Windows QEMU installation paths and
converts paths for Windows QEMU. A failed build/conversion/verification leaves
the previous output image intact. Use `--force` to explicitly replace it.

#### Write the image from Linux or Windows

Two writers use `dist/mind-core-usb.img` by default. They require an explicit
**whole USB disk**, display its identity and size, and require a typed `ERASE ...`
confirmation. **All existing data on the selected disk will be lost.** They
refuse internal/system disks, read-only or undersized devices, non-512-byte
logical sectors, and a disk containing the image or writer itself. Neither
writer builds the image; run `04_make_usb_image.sh` first if it is missing.

Linux (Python 3 and util-linux; normally already installed):

```bash
./05_write_usb_linux.sh --list
./05_write_usb_linux.sh --device /dev/sdX --check
sudo ./05_write_usb_linux.sh --device /dev/sdX
# Optional: --image /path/to/mind-core-usb.img
```

Replace `/dev/sdX` with the drive from `--list`, for example `/dev/sdb`, **not**
`/dev/sdb1`. `/dev/disk/by-id/...` disk paths also work. `--check` only validates
the selection and image; it does not unmount or write anything. The write mode
unmounts that disk's filesystems and acquires exclusive block-device access;
active swap/LVM/RAID/crypt devices are refused. Close files/programs using the
flash drive before writing. Normal WSL disks are virtual disks, not USB drives:
use the Windows writer for a USB drive attached to Windows.

Windows 10/11, Windows PowerShell 5.1 or PowerShell 7 (no QEMU/Python needed
for writing). Open **PowerShell as Administrator** for the final write:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\05_write_usb_windows.ps1 -List
powershell -NoProfile -ExecutionPolicy Bypass -File .\05_write_usb_windows.ps1 -DiskNumber 2 -Check
powershell -NoProfile -ExecutionPolicy Bypass -File .\05_write_usb_windows.ps1 -DiskNumber 2
# Optional: -Image "C:\path\mind-core-usb.img"
```

Replace `2` with the **disk number from `-List`**, not a drive letter. The
execution-policy override applies only to that PowerShell process. `-Check`
does not lock, dismount or write. The writer uses Windows volume locks and raw
disk I/O; it stops if a volume is busy and releases its locks on error. Close
Explorer windows and applications using the drive. Keep the image and scripts
on another local Windows disk (not on the target USB disk or a network share).

Both writers flush the image to disk, clear stale backup GPT metadata beyond
the image and verify SHA-256 by reading back the written image-sized prefix.
The rest of a larger drive is not expanded into the FAT partition. After a
successful verification, safely remove/reconnect the USB drive. A cancelled or
failed write may leave an incomplete image; rerun the writer before using it.
These scripts are image writers, not secure data-erasure tools.

Non-destructive writer checks (temporary regular files only):

```bash
python3 tests/test_usb_writer.py
```

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\tests\usb_writer_windows.ps1
```

To test the actual image as a USB storage device in QEMU/OVMF:

```bash
python3 tests/usb_image_smoke.py --qemu qemu-system-x86_64 --firmware OVMF.fd
# WSL with the project's Windows QEMU installation:
python3 tests/usb_image_smoke.py --qemu /mnt/c/msys64/ucrt64/bin/qemu-system-x86_64.exe
```

The test checks image contents, UEFI USB boot, all three programs, CPU startup,
foreground switching and memory reclamation. Real-machine support still has
the limits documented below; in particular, the kernel currently reads PS/2
keyboard/UART input and has no USB keyboard driver after leaving UEFI.

### Console

At the `MIND>` prompt, enter a command and press Enter (commands are case-insensitive):

* `LIST` — show all available program names and descriptions.
* `RUN app` — launch a new foreground instance of the rotating-square application (`app.elf`). `BOOT` remains an alias for this command.
* `RUN app2` — launch the second application (`app2.elf`): a bouncing square, frame counter, and TSC value on screen. It prints a greeting and a status line every 30 frames to the host's QEMU console via UART.
* `RUN clock` — display a large digital clock (`clock.elf`) in 24-hour `HH:MM:SS` format, with time changes also printed to the UART console.
* `RUN dzen-clock` — five color indicators for time (`dzenclk.elf`); **D** toggles the thin digital time, **C** selects a simple 100-second orbit, **P** selects an orbit with 10-second ticks, **H** hides/shows the title and key hints.
* `RUN <name> &` — launch a new background instance and retain the shell. Repeating the command creates independent instances with different PIDs. Up to eight application tasks can coexist.
* `PS` — show PID, program, state, foreground/background, assigned CPU, scheduling count, CPU timer ticks, and syscall count. The shell/idle task has reserved PID 0.
* `FG <id>` — show an existing task's screen and route keyboard/UART input to it, preserving its PID and state.
* `KILL <id>` — terminate that instance; the shell then frees its image, stack, screen, private heap and page tables.
* `LOGS <id>` — read and drain that instance's last 4096 bytes of buffered output. Foreground output is also printed to UART with a PID prefix; background output stays buffered so it does not interrupt command entry.
* `CPUS` — show online CPU/APIC IDs and per-CPU timer counters.
* `FAULTS` — show the last 16 application exceptions: PID, CPU, exception vector/error code, instruction and fault addresses.
* `HEAP` — allocate and format a test string, release it, and report total runtime heap usage, free bytes, and whether the test allocation was freed. The measurement is serialized with program allocations so concurrent heap activity cannot produce a false leak report.
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

**Current scope:** x86-64 UEFI/QEMU with xAPIC and NX, tested with one and four CPUs. Runtime RAM and the GOP framebuffer must fit below 4 GiB; the heap is a reserved 64 MiB arena shared by kernel resources and all private program heaps. There are at most eight application tasks and eight CPUs. CPU assignment is fixed for each task; there is no migration, work stealing, demand paging, or userspace allocator for sub-page objects yet. Kernel mappings are supervisor-only identity mappings (kernel text is not separately write-protected). The supported compiler target remains `x86_64-unknown-none`; context switching saves x87/SSE, and AVX/XSAVE is disabled in CR4.

The clock reads the CMOS RTC through syscall 4, which returns seconds since midnight (or `usize::MAX` when unavailable). The kernel checks for stable readings and handles both BCD/binary and 12/24-hour RTC modes. The application displays RTC time without applying a timezone offset. The supplied launch commands use local time; QEMU otherwise defaults to UTC ([QEMU RTC options](https://www.qemu.org/docs/master/system/invocation.html)).

Both square demos sleep between frames and redraw only changing areas in their own buffers. The clock sleeps between RTC checks and redraws when the second changes. Syscall 5 sleeps the calling task for `arg1` milliseconds (rounded up to a 10 ms tick, capped at 60 seconds), allowing other tasks to run and waking early for foreground input. Syscall 6 reports uptime in milliseconds; syscall 7 exits the calling task. Task resources are reclaimed by the BSP shell only after the owning CPU has stopped the task and switched away from its page tables.

### Dzen clock

Run `run dzen-clock` (or `run dzen-clock &`, followed by `fg <id>`).
Four circular indicators form a square around a fifth, central indicator:

| Corner | Hour range |
|---|---|
| Bottom right | 00–05 |
| Bottom left | 06–11 |
| Top left | 12–17 |
| Top right | 18–23 |

The active hour corner uses **RED, YELLOW, GREEN, CYAN, BLUE, MAGENTA**
for the six hours in its range. The center uses the same color sequence for
minutes **00–09, 10–19, 20–29, 30–39, 40–49, 50–59**.

Number the three remaining corners clockwise, starting just after the hour
corner. Within each ten-minute interval, dim white lights encode 100-second
steps:

| Elapsed time | White corners |
|---|---|
| 0:00–1:39 | First |
| 1:40–3:19 | Second |
| 3:20–4:59 | Third |
| 5:00–6:39 | Second and third (first is dark) |
| 6:40–8:19 | First and third (second is dark) |
| 8:20–9:59 | First and second (third is dark) |

The thin `HH:MM:SS` line starts visible. Press **D** to hide/show it (PS/2 or
UART). A small gray dot moves on a faint, one-pixel orbit between the center
and corner indicators. **C** selects the orbit with one thin reference tick
toward the first hour corner (bottom right, 00–05); **P** selects the same orbit with nine additional, shorter
ticks every ten seconds. Pressing the active mode's key again hides the entire
orbit and dot; pressing the other key switches modes directly. The orbit
starts hidden. The simple **C** orbit uses a darker gray than the **P** orbit.
The dot's diameter is half the former moving stroke's length
(six pixels at the usual display size).
The dot starts toward the bottom-right corner and moves clockwise, completing
one turn per 100 seconds, synchronized
with the white corner intervals. It updates every 100 ms using uptime
interpolation between RTC readings. The drawing restores the orbit/ticks
behind the dot without trails or repainting the five indicators.
**H** hides/shows the title and key hints, independently of the digital time
and orbit. Text starts visible. Each instance retains its text visibility,
digital display and orbit mode when backgrounded.
**Ctrl+Z** returns to the shell; **Esc** exits. Time comes from the same RTC
as `clock`, without timezone conversion. Until the first valid reading, the
indicators stay dark and the digital line shows `--:--:--`.
The app sleeps between RTC checks, redraws the indicators only when their
state changes, and updates visible digits once per second. Indicator changes
are also logged to the console.

Check the real display, both input paths and independent instances:

```bash
python3 tests/qemu_smoke.py --suites dzen --qemu qemu-system-x86_64
```

### Private program heap

The mailbox ABI in `common/abi.rs` provides `SYSCALL_ALLOC = 8` and `SYSCALL_FREE = 9`:

* Allocate: `arg1` is a nonzero byte count; the kernel rounds it up to 4096 bytes. The result is a page-aligned virtual address, or `0` for an invalid size, exhausted quota or insufficient memory. `arg2` is reserved. All mapped bytes, including alignment padding, start at zero.
* Free: `arg1` must exactly match a live block's starting address in the calling process. The result is `0` on success or `usize::MAX` for an invalid/interior/already-freed address. `arg2` is reserved. A successful free unmaps the block, flushes the local TLB and releases its RAM and any empty page tables.

Each process has a separate heap arena starting above `0x8006000000`. Every block is writable and non-executable, followed by an unmapped guard page. Limits are 32 live blocks and 16 MiB of rounded data per process; availability also depends on the shared 64 MiB kernel arena. Physical backing is currently contiguous, so fragmentation can cause an allocation to fail. Failed mappings roll back all partially allocated resources. Guard pages catch accesses into those pages; they do not detect overruns that stay within a mapped page. An address can be reused after free, so this API does not provide temporal memory safety after reuse.

`common/user_memory.rs` supplies `Pages::new(mailbox, bytes) -> Option<Pages>` and `as_mut_slice()`, with automatic free in `Drop`. The unsafe constructor requires the process's valid mailbox. For example, with that module imported:

```rust
if let Some(mut buffer) = unsafe { user_memory::Pages::new(mailbox, 8192) } {
    buffer.as_mut_slice()[0] = 42;
} // freed here; exit/kill/fault also reclaims any remaining blocks
```

This is a page-block API; a `malloc`/Rust `GlobalAlloc` implementation can later subdivide these blocks. `app2` already uses a block for its 64×64 sprite and handles allocation failure by reporting it and returning. Page-table edits are serialized with the scheduler; a process runs on only one pinned CPU, so local invalidation is sufficient. Kernel allocation locks disable local interrupts to avoid allocator/scheduler lock inversion. CR3 invalidation follows the [Intel system programming manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html).

### Runtime checks

After building, run the host tests for real ELF images, independent `.bss`/relocations, malformed ELF rejection, scheduling/wakeup policy, RTC handling, private mappings, heap limits and allocation rollback:

```bash
rustc --edition=2021 --test tests/runtime.rs -o /tmp/mind-core-runtime-tests
/tmp/mind-core-runtime-tests
```

The QEMU integration test boots an isolated copy of `usb_root`, exercises concurrent instances, `fg`, `kill`, UART/PS2 input, task limits, repeated allocation/freeing, and idle `HLT`. Additional suites check concurrent CPU progress, remote termination, independent SIMD contexts, private heap stress/OOM recovery, and deliberate ring-3 faults without stopping other programs:

```bash
for fixture in busy_app isolation_app heap_app; do
  rustc --edition=2021 --target x86_64-unknown-none --crate-type bin \
    -C opt-level=3 -C panic=abort -C relocation-model=pic \
    -Z relax-elf-relocations=yes -C link-arg=-Tapp/linker.ld \
    "tests/$fixture.rs" -o "/tmp/mind-core-$fixture.elf"
done
python3 tests/qemu_smoke.py --qemu qemu-system-x86_64 \
  --firmware OVMF.fd --busy-elf /tmp/mind-core-busy_app.elf \
  --isolation-elf /tmp/mind-core-isolation_app.elf --heap-elf /tmp/mind-core-heap_app.elf
# Repeat SMP/fault/heap handling on a single CPU:
python3 tests/qemu_smoke.py --qemu qemu-system-x86_64 --cpus 1 \
  --busy-elf /tmp/mind-core-busy_app.elf \
  --isolation-elf /tmp/mind-core-isolation_app.elf --heap-elf /tmp/mind-core-heap_app.elf \
  --suites smp,isolation,heap
```

From WSL with the supplied Windows setup, use `--qemu /mnt/c/msys64/ucrt64/bin/qemu-system-x86_64.exe`. Test logs are written to the system temporary directory. The busy-loop, isolation and heap-test ELFs are used only inside test VMs; the normal packaged applications are unchanged by the test.

### Compact context for agents

Both exporters write to `code_handoff/` in the project root by default,
independently of the working directory. The directory is ignored by Git.
The full exporter keeps its original file markers and writes `code_context.txt`:

```bash
./code_concat.sh
```

The compact exporter needs only Bash and Python 3:

```bash
./code_concat_compact.sh
```

Compact results also go to `code_handoff/`:

| File | Contents / when to paste |
|---|---|
| `index.txt` | File inventory, scope, sizes and limitations; use it to choose relevant files. |
| `code.txt` | Current Rust sources, including comments and inline tests, with short file markers. |
| `build.txt` | Manifests, `.cargo/config.toml`, linkers, build/run/USB/export scripts and their helpers. |
| `agent.txt` | Compact Rust code for discussion: comments/formatting reduced; inline `#[cfg(test)] mod ...` blocks omitted by default. |
| `parts/agent-001.txt`, etc. | The same agent bodies split for copy/paste, at most 16,000 Unicode characters per file including headers. |
| `manifest.json` | Selected source paths/hashes, original/output sizes, estimates and explicit test-module omissions. |

Paste **`agent.txt` OR the numbered parts in order**, not both. Keep the file
markers and part headers: parts identify the file and character offset in its
compact body. Splits prefer line boundaries; a very long line/string can be
split across parts without dropping any text. This is an analysis view, not a
patch to apply to the repository. Original source files are never modified.

The default selection includes `kernel`, `bootloader`, the four applications,
`common`, and current tooling. It excludes historical patches, `legacy`, issue
files, old `knowledge` snapshots, dependencies/build outputs, `Cargo.lock`,
symlinks and previous context dumps. Files do not need to be committed to Git.
Byte-identical files (such as shared linker/config files) are included once per
bundle; later occurrences explicitly reference the first file.

Rust string/byte/character/raw literals, inline assembly, identifiers, numeric
data and runtime function bodies are preserved. The scanner handles nested
comments and preserves operator separation. Python, shell, PowerShell, C#,
TOML and linker file bodies are kept intact: their indentation, here-documents
and strings are not subject to Rust minification. For comments, rationale or
inline tests, use `code.txt` or enable the relevant option below.

For the largest reduction in context, select only the files needed for the
question. The exporter does **not** automatically resolve their dependencies:

```bash
# A focused scheduling/ABI discussion:
./code_concat_compact.sh \
  --include kernel/src/scheduler.rs --include common/abi.rs \
  --out-dir code_handoff/scheduler

# Full kernel context, including shared ABI and the imported relocation helper:
./code_concat_compact.sh --include kernel --include common \
  --include bootloader/src/elf_reloc.rs --out-dir code_handoff/kernel

# Include build tooling in the agent packet:
./code_concat_compact.sh --agent-groups code,build

# Export separate tests/docs and keep inline Rust tests in the agent view:
./code_concat_compact.sh --with-tests --with-docs --keep-tests-in-agent
# To paste test files too, add: --agent-groups code,tests

# Smaller messages, or no splitting:
./code_concat_compact.sh --chunk-chars 8000
./code_concat_compact.sh --chunk-chars 0
```

`--include` accepts a file, directory prefix or quoted glob and can be repeated.
An unmatched selection is an error, not an empty or silently truncated export.
`--agent-groups` defaults to `code`; enabling `--with-tests`/`--with-docs`
creates `tests.txt`/`docs.txt` without automatically enlarging `agent.txt`.
Generated stale bundles/parts from a previous export in the same output
directory are removed; unrelated files are retained. Identical inputs/options
produce identical output, and a source/selection error leaves the prior export
intact.

Reported token counts use **characters / 4**, explicitly an estimate rather
than a particular model's tokenizer. Byte and character counts are exact;
the chunk limit is in characters, not tokens. Use the destination model's
tokenizer if you need an exact context-budget calculation.

Exporter checks:

```bash
python3 tests/test_context_export.py
```

These cover lexical preservation on all runtime Rust sources, literal/comment
edge cases, explicit test omissions, scope/deduplication, determinism,
source immutability, exact reconstruction of split bodies and stale cleanup.
