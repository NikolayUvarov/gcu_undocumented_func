# MIND CORE

The Minds of the Culture's spaceships — the ultra-powerful AIs controlling General Contact Units and Orbitals — must reside somewhere. No matter how multidimensional and advanced their hardware substrate may be, at a fundamental level, any computing architecture needs two basic things to wake up and become self-aware: a bootloader and an operating system.

Before a Mind can simulate pocket universes, juggle hyperspace vectors, or conduct delicate diplomatic games on behalf of an entire civilization, it requires a reliable and predictable foundation. It is time to start building it.

This repository is the first step toward creating a substrate-independent environment for future Minds. We are starting right now, from the very lowest level of hardware reality.

Every Mind needs its first processor tick. We are providing exactly that.


### Goals and Architecture

* **Substrate Initialization (Bootloader):** Direct interaction with UEFI, seizing control of the bare metal, and preparing physical memory.
* **Kernel:** The basic reality dispatcher. Handling hardware interrupts, system calls, and preemptive multitasking.
* **Isolated Environment (Userspace):** A space for the genesis and parallel execution of high-level processes and future cognitive functions.

The normative requirements are in the [Constitution v1.6](constitution/EN/MIND_CORE_Constitution_v1.6.md) and [RFC 001 Marain v0.4](constitution/EN/RFC_001_Marain_v0.4.md) (Russian texts in [constitution/RU](constitution/RU), index in [constitution/README.md](constitution/README.md)); the order of work, current gaps and the point from which parts can be developed in parallel are in [ROADMAP.md](ROADMAP.md) v1.1 ([Russian](ROADMAP_RU.md)); what the implementation guarantees and under which assumptions is in the platform profile [docs/profile](docs/profile/README.md). The plan for system tools (file manager, editor, `top`, memory map, load monitor and others) is in [docs/tools](docs/tools/README.md) ([Russian](docs/tools/README_RU.md)).

---

### Current Runtime

The UEFI bootloader loads the kernel and only the system service images (`BOOT_FILES` in `common/abi.rs`) from the FAT filesystem, enumerates enabled CPUs through UEFI MP Services, and reserves a 64 MiB runtime heap, a BSP stack, and a low-memory AP bootstrap page. The kernel takes over after ExitBootServices and starts exactly one program in ring 3, `init`, which holds the bootstrap authority and starts the other services, including the command shell. Applications are not kept in memory: the `loader` service reads them from the boot disk through `vfs_server` when they are started.

* **Toolchain:** Pure Rust (`no_std`, `no_main`), utilizing `naked_functions` and `abi_x86_interrupt`, compiled for `x86_64-unknown-none` and `x86_64-unknown-uefi` targets. Programs use the `libmind` SDK.
* **Memory and privilege:** Every task runs in ring 3 with IOPL=0 and a private four-level page table/CR3. Each `RUN` creates a fresh ELF image, 64 KiB user stack with unmapped guard pages, syscall mailbox, input/log queues, and (for applications) a screen buffer. Code is RX; writable data, stack, mailbox and screen are NX. The kernel's supervisor mappings are inaccessible to tasks.
* **Capabilities:** Each task has 32 capability slots. A capability names an IPC endpoint (with read/write/grant/keep rights), a shared memory block (read/write/grant rights), a DMA region, device registers (MMIO, mapped uncached), an I/O port range, an interrupt line, or a privilege (input, display, spawn, process control, platform). Drivers receive only the capabilities for their device; applications receive send-only endpoints of the RTC, VFS, audio, loader and TTS services. Every capability a task has was granted explicitly by its spawner (`init` for services, `loader` for applications). Capabilities are named by handles `slot | generation << 8`: slots 1–9 are fixed by convention (generation 0), slots the kernel hands out get a new generation when freed, so a stale handle is rejected instead of naming a newer capability. Every capability has a parent: a copy (IPC transfer, spawn grant) or a `CAP_MINT` (narrower endpoint rights, a port sub-range or a page-aligned memory/DMA/MMIO sub-range) is a child of its source, a move (`CAP_TRANSFER_MOVE`, `GRANT_MOVE`) keeps it and empties the source slot. `CAP_REVOKE` removes every descendant from every task, including one waiting in a blocked send, and returns how many were removed; the holder keeps its own capability. A memory capability without the write right maps read-only; mappings made from a revoked memory, DMA or MMIO capability are removed, and when the holder runs on another CPU `CAP_REVOKE` returns only after that CPU has switched address space (a lease ends at a defined point). `MEM_DETACH` turns a heap block nobody else refers to into a memory object: the pages leave the caller's address space and the capability (read/write, no grant) can only be moved — copying a writable memory capability needs the grant right — or minted read-only. `CAP_INFO` reports a memory range as *sealed* when no writable capability, writable mapping or DMA region overlaps it, so a reader can check that nobody writes while it reads (`SHARE_RO`).
* **Dynamic program memory:** Syscalls 8/9 allocate and free zeroed private page blocks (RW+NX, trailing guard page; up to 32 blocks and 16 MiB per process). Mapped shared memory has its own 48 MiB quota. Memory that another task still maps or holds by capability is retained by the kernel until the last reference is gone, so freeing, exiting or killing an owner cannot leave a dangling mapping.
* **CPU configuration:** The QEMU launchers expose four cores. The kernel starts APs itself with INIT/SIPI and can use up to eight enabled xAPIC CPUs. Each CPU has its own GDT, TSS, interrupt-entry stack, idle stack, and double-fault/NMI stacks. `cpus` reports actual online CPUs and interrupt counters.
* **Scheduling:** New applications are assigned to the CPU with the fewest applications and remain pinned there. Round-robin scheduling on each CPU preserves GPRs and x87/SSE state. The BSP receives the 100 Hz PIC/PIT tick through LAPIC ExtINT and sends scheduling IPIs to online APs. A spinlock with local interrupts disabled serializes scheduler/syscall work. Idle CPUs use `HLT`; a CPU that sleeps while one of its tasks becomes ready (an IPC reply, an IRQ) gets a wake IPI instead of waiting for its next tick. The BSP idle loop reclaims exited tasks once no CPU runs them.
* **System calls and faults:** The DPL-3 `int 0x80` gate is the only user entry into kernel services. Every buffer pointer is checked against the calling task's user mappings. User exceptions terminate that task and appear in `faults`; a kernel exception remains fatal.

### System services

The kernel contains no list of services and no per-service capability table. It starts boot image 0, `init`, with its own endpoint and two privileges: **platform** (mint capabilities over resources the kernel has validated — legacy port ranges, IRQ lines, PCI BARs and IRQs from the kernel's enumeration, the framebuffer, DMA regions, privileges) and **spawn**. That is the whole bootstrap authority; `init` then starts the other boot images in `BOOT_SERVICES` order (`common/abi.rs`), each with exactly the capabilities listed below (`SPAWN` with a grant list), and keeps DMA regions across driver restarts. Endpoints have no global numbers: `init` creates one per service and keeps only a *keeper* capability (`CAP_KEEP`: may mint receive rights, cannot receive itself); the server gets a receive child, clients get write/grant children (copies of the keeper narrowed at spawn, or minted with a badge). A restarted service gets a new receive child of the same endpoint, so clients granted earlier reach it again; while no server runs their calls fail with `ERR_PEER`. `ahci` and `usb_storage` start only when `init` finds their controller (`DEVICE_FIND`), so later PIDs depend on the machine. Services cannot be brought to the foreground (except the shell) and each runs once; `RUN <service> &` asks `init` to restart one after `KILL`.

| Service | Capabilities (granted by init) | Role |
|---|---|---|
| `init` | own endpoint, platform and spawn privileges (from the kernel) | service policy; restarts services on request |
| `logd` | service endpoint, observe privilege | the system log (`idl/log.wit`): 256 records of up to 200 bytes in a 64 KiB ring, each stamped with the time it arrived and the sender's PID and task name (from IPC and the kernel's task records, never from the text); at most 64 records a second per sender, the rest refused and counted; reading needs the shell's read badge |
| `rtc` | service endpoint, ports 0x70–0x71 | CMOS clock; serves `idl/rtc.wit` (seconds since midnight, days since 2000-01-01) |
| `ps2_kbd` | ports 0x60, 0x64, IRQ 1, input | PS/2 keyboard → key events (modifiers, F-keys, navigation keys, US/Russian layout) for the focused task |
| `compositor` | GOP framebuffer, display | copies changed pixels of the focused screen to the framebuffer |
| `ata` | service endpoint, ports 0x1F0–0x1F7, 0x3F6 | primary IDE channel, PIO LBA28 |
| `ahci` | service endpoint, ABAR (MMIO), 128 KiB DMA | first SATA disk on an AHCI controller (class 01:06:01) |
| `usb_storage` | service endpoint, xHCI BAR0 (MMIO), 256 KiB DMA | first USB mass storage device (Bulk-Only, SCSI) on an xHCI controller (0C:03:30) |
| `ramdisk` | service endpoint | an 8 MiB block device in its own memory: the RAM disk `ram:` |
| `vfs_server` | service endpoint, write-badged clients of the running block drivers and of `ramdisk`, an `rtc` client | mounts the boot disk and `ram:` (FAT12/16/32) and serves files and directories through handles (`idl/vfs.wit`), writing included |
| `loader` | service endpoint, RTC/VFS/audio/TTS client endpoints, spawn privilege | reads application ELF files from the disk and starts them with the standard client capabilities; in a launch session (`idl/loader.wit`) also with the capabilities the launcher lends (slots 7–12) |
| `audio_gw` | service endpoint, AC97 BARs, its IRQ, 200 KiB DMA | audio gateway: playback (PCM, tones) and microphone capture through AC97 DMA rings |
| `tts` | service endpoint, audio gateway client | text to speech (Russian and Latin script), streamed to `audio_gw` |
| `sysmon` | service endpoint, observe privilege | system information (`idl/sysinfo.wit`): the kernel's `STAT` records, load samples every 100 ms (300 kept) and every second (600 kept), load averages; at most 20 requests at once and 40 per second per client |
| `shell` | screen, init/loader/sysmon and other client endpoints, process control, input, COM1 ports | the `MIND>` command shell |

Every service but `logd` also holds a client of `logd` in slot 12 (`SLOT_LOG`): its `println!` lines go to the system log as well as to its own buffer (`LOGS`). In the default QEMU setup (IDE disk, no xHCI/AHCI) thirteen services run and applications start at PID 14. The kernel has room for 24 tasks. Tasks and endpoints are charged to quotas delegated at spawn: `init` holds the root quota and gives `loader` eight application tasks (the application limit) and 32 endpoints; each application may create four endpoints.

The shell runs in ring 3. It reads the UART itself, forwards bytes to the focused program through the input privilege, and prints that program's console output to COM1 with a `[PID n]` prefix. Focus is a kernel mechanism set only by the holder of process control: the focused task's screen is shown and receives keyboard input; when it exits, or on Ctrl+Z (an input event flagged as attention), focus returns to the shell and the shell gets a notice. The kernel writes to COM1 only its boot line, kernel exceptions and panics.

### IPC

Endpoints are rendezvous points. `IPC_SEND` blocks until a receiver takes the message; several senders queue in arrival order. `IPC_CALL` sends and then waits for the server's `IPC_REPLY`, so a client needs no reply endpoint of its own. A message carries two data words and, optionally, one capability from the sender's slot (endpoint rights can be narrowed; transfer requires the grant right on the endpoint used; with `CAP_TRANSFER_MOVE` in the rights word the capability is moved, not copied). The receiver learns the sender's PID. If a server dies while a client waits for its reply, the client is woken with `ERR_PEER`; a send to an endpoint that no live task can receive from fails with `ERR_PEER` at once. At most eight senders wait on one endpoint; another send fails with `ERR_BUSY` (libmind waits a tick and retries). The endpoint word may carry a timeout in milliseconds (`IPC_TIMEOUT_SHIFT`, `call_timeout`, `recv_timeout`, `send_timeout`): when it passes, the operation fails with `ERR_TIMEOUT`, a queued send is withdrawn with its capability, and a server's late reply to an abandoned call fails with `ERR_PEER`. A server can keep a client waiting: `IPC_SAVE_REPLY` moves the pending reply into a one-time capability (it cannot be transferred), the server goes on receiving other requests and later answers with `IPC_REPLY` naming that slot.

A driver can bind its interrupt line to its endpoint (`IRQ_BIND`): the interrupt then arrives as a message with the IRQ flag, so one loop serves both clients and hardware. The kernel masks the line when it fires; the driver reopens it with `IRQ_ACK` (or `IRQ_WAIT` for drivers that only wait for interrupts).

### libmind SDK

`libmind/` (crate name `mind`) is the only place with `int 0x80`. A program is:

```rust
#![no_std]
#![no_main]
mind::entry!(main);
fn main(info: &'static mind::BootInfo) {
    let screen = mind::gfx::Screen::new(info).unwrap();
    screen.text(24, 24, b"HELLO", 2, 0x00FFFFFF, None);
    mind::println!("uptime {} ms", mind::time::uptime_ms());
    while mind::input::wait_or_exit(100).is_none() {}
}
```

Add `libmind = { path = "../libmind" }` to the crate's `Cargo.toml`. Modules:

| Module | Contents |
|---|---|
| `sys` | raw syscall, `Error`/`Result`, mailbox set up by `entry!` |
| `process` | `exit`, `spawn` (through `loader`), `alive`, `log`, `print!`/`println!`; `request!` (what the program asks its launcher for: `REQUEST_CONSOLE`, `REQUEST_SYSINFO`, …); `spawn_image`/`loader_done` for `loader` |
| `time`, `input` | `sleep`, `uptime_ms`, `rdtsc`; `read_key`, `wait_key`, `wait_or_exit` (Esc exits) → `Key` (`code()`: character, Enter, Esc, arrows, Home/End, PgUp/PgDn, Ins/Del, F1–F12; `char()`, `text()`, `shift()`/`ctrl()`/`alt()`) |
| `tui::viewer` | file viewer core shared by `view` and the file manager: a `Source` read through a window cache, text with or without wrapping and line numbers, hex dump, search ignoring case, go to |
| `tui` | text UI on the 8×16 font: `Grid` of cells (text with clipping, frames with titles, fills, bars in 1/8 cells, braille time-series graphs), `Terminal` (the grid on the program's screen, redraws only changed cells, cursor), themes `CLASSIC` (Norton Commander colours) and `DARK`, widgets (`ListState`, `InputLine` with UTF-8 editing, `History`, `MenuBar`, `fkey_bar`, dialogs, `progress`) |
| `keys` | ring 3 key decoders: `Ps2` (scan code set 1, modifiers, Caps/Num Lock, US and Russian layouts) and `Vt` (UART: VT100/xterm sequences, UTF-8); `Key::latin()` gives letter commands in either layout |
| `ipc` | `Endpoint::{create, send, call, recv}`, `reply`, `drop_cap`, `Message` |
| `mem` | `Pages` (private blocks, freed on drop, `share()`), `Mapping` (shared memory by capability), `dma_physical` |
| `dev` | `Ports`, `Irq`, `Mmio`, `Dma`, `input_event`, `compositor_pull`, `cap_info` — for drivers |
| `block` | block device client (`Device`) and the driver loop (`serve`, `Driver`) |
| `gfx` | `Screen`: pixels, rectangles, 8×8 font text, UTF-8 text in the 8×16 font (`text16`, `glyph16`) |
| `font16` | MIND Mono 16, the 8×16 text font: a subset of Terminus Font under the SIL OFL 1.1 with Cyrillic, box drawing, block elements and braille ([fonts/](fonts/README.md)) |
| `stat` | kernel observation (`STAT`): records of tasks, CPUs, memory, physical map, address spaces, capabilities, endpoints, IRQs, devices, and their names |
| `rtc`, `fs`, `audio`, `tts` | clients of the RTC, VFS, audio and speech services (`fs::File`, `fs::Dir`, `fs::list`/`mkdir`/`remove`/`rename`/`metadata`/`volume`, `fs::use_endpoint` for a lent VFS client, `fs::check`, `audio::Stream`, `audio::wait_space`, `tts::say`) |
| `util` | `Decimal`, `FixedBuf` (`core::fmt::Write` into a fixed buffer) |
| `heap` | program heap: with the cargo feature `alloc` (`libmind = { path = "../libmind", features = ["alloc"] }`) a program gets a `GlobalAlloc` and can use `Vec`, `String`, `Box` after `extern crate alloc;`; `mind::heap_stats()` |

The SDK also supplies the panic handler (logs the message and exits the task) and `memset`/`memcpy`/`memmove`/`memcmp`. `common/abi.rs` remains the single ABI definition shared by the kernel, the bootloader and `libmind`.

### Text to speech

`tts` is a formant synthesizer written for MIND CORE (no recorded voice data): text → phonemes → targets for five cascade formants, a nasal pole/zero pair and a parallel noise branch, with a KLGLOTT88 glottal source at 16 kHz, then upsampled to 48 kHz stereo and streamed to `audio_gw`.

* Russian: letter-to-sound rules with palatalization, final devoicing and voicing assimilation, «-тся», «-ого/-его», akanye/ikanye around the stress. Stress comes from `tts/data/stress_ru.txt` — about 13 700 word forms among the 30 000 most frequent ones on which the heuristic is wrong, plus forms with «ё» — and otherwise from the heuristic: words ending in a consonant are stressed on the last syllable, others on the penultimate, «ё» is always stressed. The lookup is a binary search that treats «е» and «ё» as the same letter, so text typed without «ё» («еще», «зеленый») gets it back. The forms are taken from the OpenRussian dictionary (CC BY-SA 4.0, https://github.com/Badestrand/russian-dictionary) by `python3 scripts/stress_openrussian.py <CSV directory> ru_50k.txt 30000 > forms.txt` (frequency list from hermitdave/FrequencyWords); homographs such as «дома» or «замок» are left out. `python3 scripts/stress_exceptions.py forms.txt` merges forms written with the stressed vowel in upper case (дОбрый) and keeps only those the heuristic gets wrong.
* English (Latin script): pronunciations of about 18 700 of the 20 000 most frequent English words from CMUdict (`tts/data/lexicon_en.txt`, BSD licence, binary search), a few hand corrections in `text.rs`, and simplified spelling rules for other words. The phoneme set adds the English vowels [ɪ ʊ ʌ ɝ] and voiced [ð]. `python3 scripts/lexicon_en.py cmudict.dict en_50k.txt 20000` rebuilds the lexicon (frequency list from hermitdave/FrequencyWords). Digits are read one by one.
* Prosody: phrase declination, a pitch rise on stressed vowels, falling or rising final intonation for `.` and `?`, pauses for punctuation and between words.

```rust
mind::tts::say("Привет. Я разум корабля.")?;          // blocks until the speech is queued
mind::tts::say_with("Hello world.", 140, 90)?;         // pitch 140 Hz, 90 % rate
```

`say [-p <pitch Hz>] [-r <rate %>] [text]` speaks the text given on the command line (`say привет мир` — the shell accepts UTF-8 from the UART), or `say.txt` from the boot disk, or a greeting. The same synthesizer modules build on the host: `rustc --edition=2021 -O tests/tts_host.rs -o /tmp/tts_host && /tmp/tts_host "текст" out.wav` writes a 16 kHz WAV, and `rustc --edition=2021 --test tests/tts_host.rs` runs the text-rule tests. Intelligibility was tuned against the offline Vosk small models: on 30 Russian test phrases about 68 % of the words are recognized (59 % without the stress dictionary), on 30 everyday English phrases about 53 % (42 % with spelling rules only); the voice is clearly synthetic.

### Program loading

`RUN <name>` and `LIST` for applications are `CALL`s from the shell to `loader`, like any program's. For `RUN`, `loader` opens `<name>.elf` (or the given path, if it contains `.` or `/`) through `mind::fs`, reads it into its own page block and calls `SPAWN` with a capability for that block and a grant list of the standard client endpoints; the kernel parses and copies the ELF into the new task and the block is freed. For `LIST` the shell passes a memory page and `loader` writes the list of `*.elf` files in the root of the disk into it. Programs start other programs with `mind::process::spawn(name, grant)`, a `CALL` to `loader` that may pass an endpoint for the child's INIT slot. `RUN <service>` goes to `init` instead. Only `init` and `loader` hold the spawn privilege, only `init` may spawn boot images or services; service names and `kernel` are not loaded as applications.

### Block devices

Storage drivers are separate ring-3 services that speak one block protocol (`BLOCK_INFO`, `BLOCK_ATTACH`, `BLOCK_READ` in `common/abi.rs`): the client attaches a 64 KiB buffer once by capability, and every read of up to 128 sectors fills it. `mind::block` provides both sides — `Device` for clients and `serve`/`Driver` for drivers. The drivers poll their controllers (no interrupts yet); AHCI and xHCI registers arrive as MMIO capabilities mapped uncached, and the kernel allocates each driver a DMA region (64 KiB aligned) whose physical address only that driver can query.

* `ata`: primary channel, PIO, up to 128 sectors per command.
* `ahci`: takes the HBA from firmware (BIOS/OS handoff), uses the first port with a SATA signature, IDENTIFY and READ DMA EXT through one command slot.
* `usb_storage`: takes the xHCI controller from firmware, resets it, enables USB 3 ports or resets USB 2 ports, addresses each device until it finds a mass storage interface (class 08/06/50), configures its bulk endpoints and reads with SCSI READ(10). Other USB devices (for example a keyboard) are addressed and skipped.

### Virtual file system

`vfs_server` opens the block drivers it was given (in the order ATA, AHCI, USB) and mounts the first FAT12/16/32 volume, with or without an MBR, as the boot disk; the RAM disk (`ramdisk`, 8 MiB) is formatted as FAT16 (`MIND RAM`) when blank and mounted as `ram:`. Its contents never outlive the boot. The FAT code (`vfs_server/src/fat.rs`) reads and writes FAT12/16/32: long names in UTF-16 (Cyrillic included) with unique `~N` aliases and checksums, cluster allocation on every FAT copy, directories that grow, the fixed root of FAT12/16, `.` and `..`, moves between directories (not into themselves), times from the RTC, the dirty bit in FAT[1] from the first change until a flush, and an unknown free count in FAT32's FSInfo. Sectors go through a 64-sector cache with read-ahead and write-back; a flush writes the changed sectors in LBA order and empties the drive's cache.

The protocol is `idl/vfs.wit` (MIND IDL): handles of volume roots, directories and files; `open-dir`, `open` (write, create, truncate, new), `read`, `write`, `truncate`, `stat`, `list`, `remove`, `rename`, `volume`, `flush`, `close`, and `check` (2.1: a read-only consistency check of the volume). Every path is relative to a directory handle and `..` is refused. A handle belongs to the client that opened it (PID and badge) and is never wider than the one it was opened from (MC-3.4). The badge of a client's capability decides what it may change: applications (through `loader`) read only; the shell's client carries `VFS_BADGE_USER` and writes anywhere on `ram:` and in `data/` of the boot disk — boot files and the rest of the disk stay read-only. A removed file's handles are closed. Clients use `mind::fs`:

```rust
let mut file = mind::fs::File::open("EFI/BOOT/BOOTX64.EFI")?;
let mut chunk = [0u8; 4096];
let n = file.read(&mut chunk)?;
mind::fs::list("ram:", |entry| mind::println!("{} {}", entry.name_str(), entry.size))?;
let mut notes = mind::fs::File::create("ram:notes.txt")?; // needs a writing client (the shell's)
notes.write(b"hello")?;
notes.flush()?;
```

The block drivers read and write (ATA WRITE SECTORS and FLUSH CACHE, AHCI WRITE DMA EXT and FLUSH CACHE EXT, USB SCSI WRITE(10) and SYNCHRONIZE CACHE(10), write protection from MODE SENSE(6)); `BLOCK_WRITE` and `BLOCK_FLUSH` are served only to a client whose endpoint capability carries the write badge, which `init` gives to `vfs_server` alone (Appendix B.6). An endpoint capability can carry a 16-bit **badge**, set once by `CAP_MINT` (`mind::ipc::mint_badged`); the server sees it with every message sent through that capability (`Received::badge`), so one endpoint serves clients with different rights. 

Each request is a `CALL` lending the client's 20 KiB transfer buffer. A listing gives each entry's size, attributes (`VFS_ENTRY_*`: directory, hidden, system, read-only, archive) and FAT modification time (`mind::fs::fat_time`). Handles of dead clients are recycled. A power loss before a flush can lose the changes since the last one; a power loss during one leaves what FAT allows (no journal): see [docs/profile/threat-model.md](docs/profile/threat-model.md). `RUN files` lists the boot disk and reads two files. The launchers' `fat:` drive is an IDE disk; the USB image is read through `usb_storage`; NVMe is not supported yet.

### Audio gateway

`audio_gw` drives an AC97 controller found on PCI by the kernel: a ring of 32 DMA buffers of 4 KiB (48 kHz, 16-bit stereo), buffer-completion interrupts delivered as IPC messages, and client PCM copied from the client's shared buffer. `mind::audio` offers `info`, `tone(hz, ms)`, `play`/`play_all` (interleaved `i16`) and `stop`. `RUN beep` plays three tones and a PCM sweep. The microphone side is a second ring of 16 capture buffers (AC97 PCM in, 48 kHz stereo): `AUDIO_RECORD_START/READ/STOP`, `mind::audio::record_start`, `record_read`, `record_stop`. `listen [seconds]` (1–10, default 3) records with a level meter on its screen, reports frames, peak and RMS, and plays the recording back. QEMU's `wav` audiodev has no capture; with `-audiodev none` the microphone delivers silence at the real rate (the `listen` test suite uses it), with `pa`/`alsa`/`dsound`/`coreaudio` it records the host microphone. When the ring is full, `AUDIO_WAIT` parks the client with a saved reply capability and the gateway answers it from the AC97 interrupt that frees buffers, so producers wait for space instead of polling. `mind::audio::Stream` collects samples in the shared buffer and hands them over in 16 KiB blocks. Add the device to QEMU with, for example, `-audiodev wav,id=snd0,path=out.wav -device AC97,audiodev=snd0` (or a `pa`/`dsound`/`coreaudio` audiodev). Without AC97 the gateway answers `DEVICE=false`.

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


2. Build the kernel, all programs and services, and the UEFI bootloader. The script places the ELF files in `usb_root/` and the bootloader in `usb_root/EFI/BOOT/`:
```bash
./02_build.sh

```


3. Launch the environment in QEMU, pointing it to your UEFI firmware and the generated root directory:
```bash
qemu-system-x86_64 \
  -bios /usr/share/ovmf/OVMF.fd \
  -drive format=raw,file=fat:rw:usb_root \
  -m 512 -smp 4,sockets=1,cores=4,threads=1 \
  -serial stdio -rtc base=localtime \
  -audiodev wav,id=snd0,path=out.wav -device AC97,audiodev=snd0   # optional audio

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
system partition, a FAT16 filesystem labelled `MIND CORE`, `EFI/BOOT/BOOTX64.EFI`,
`kernel.elf`, the services in `BOOT_FILES` of `common/abi.rs` and every other `*.elf`
built into `usb_root/` (the applications), including long names such as `compositor.elf`.

The script uses QEMU's virtual FAT image conversion, then independently checks
the partition/filesystem and compares every packaged file with the build output.
Only these files are packaged; temporary test disks in `usb_root/`
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

The test checks image contents, UEFI USB boot, the applications, CPU startup,
foreground switching and memory reclamation. Real-machine support still has
the limits documented below; in particular, the kernel currently reads PS/2
keyboard/UART input and has no USB keyboard driver after leaving UEFI. The test
also lists and reads the image through `usb_storage` and `vfs_server`.

### Console

At the `MIND>` prompt, enter a command and press Enter (commands are case-insensitive). The console uses the 8×16 font (lower case, Cyrillic, box drawing) and keeps 400 lines of scrollback (**Shift+PgUp/PgDn**). The line can be edited: **←/→**, **Home/End**, **Delete**, **Backspace**, **Ctrl+←/→** by word, **Esc** clears it, **↑/↓** walk the history of the last 32 commands, **Tab** completes a command or a program name (several matches are listed), **Ctrl+L** clears the screen. Edits are mirrored to the UART with VT100 sequences.

* `LIST` — show the programs (`*.elf`) on the boot disk, read by `loader`, and the boot services.
* `RUN <name>` — load `<name>.elf` (or a path such as `extra/demo.elf`) from the disk and start it in the foreground; any ELF built for MIND CORE can be copied to the disk and run. `RUN app` starts the rotating-square application; `BOOT` remains an alias for it.
* `RUN app2` — launch the second application (`app2.elf`): a bouncing square, frame counter, and TSC value on screen. It prints a greeting and a status line every 30 frames to the host's QEMU console via UART.
* `RUN clock` — display a large digital clock (`clock.elf`) in 24-hour `HH:MM:SS` format, with time changes also printed to the UART console.
* `RUN files` — list the boot disk and read files through `vfs_server`.
* `RUN beep` — play tones and PCM through `audio_gw`.
* `say [-p <Hz>] [-r <%>] [text]` — speak the text (or `say.txt`, or a greeting) through `tts`.
* `listen [seconds]` — record from the microphone, show the level, report peak/RMS and play it back.
* `RUN pong` — IPC demo: starts `ping`, which sends a string through a shared page with `CALL`; `pong` reads it and replies.
* `view <file>` — text and hex viewer: UTF-8 text (Cyrillic), ↑/↓/PgUp/PgDn/Space/Home/End, F2 wrap on/off (←/→ shift long lines), F4 hex/text, F5 go to a line, `0x` offset or `N%`, F7 search ignoring case (Shift+F7 next), F1 keys, Esc/F3/F10 exit. The file is read on demand through a 64 KiB window, so large files open at once.
* `edit [file]` — text editor: arrows, Home (to the indentation, then the line start)/End, PgUp/PgDn, Ctrl+Home/End, Ctrl+←/→ by words, Shift with any of them selects, Ctrl+A all, Ctrl+C/X/V, Tab, Ins overwrite, Enter keeps the indentation; Ctrl+U or Alt+Backspace undo, Ctrl+Y redo; F2 save, Shift+F2 save as, F7 find ignoring case (Shift+F7 next), Ctrl+F7 replace all, Alt+F8 go to line, F9 menu, F1 keys, F10 or Esc quit (asks about unsaved changes). UTF-8 with Cyrillic; line endings (LF or CRLF) and invalid bytes are kept byte for byte; up to 8 MiB. Saving writes `name.tmp`, flushes it and renames it over the file (best effort on FAT: no atomic replace). The editor asks for a file capability (`REQUEST_FILE`) and the shell lends its own VFS client, so files on `ram:` and in `data/` can be saved; others open read-only (save a copy elsewhere with Shift+F2). A missing file is a new one.
* `RUN keys` — show the key events a program receives: key code, modifiers, character (Esc exits).
* `fm [directory]` — file manager (Norton Commander keys): two panels over the boot disk (`A:`) and the RAM disk (`ram:`) (full: name, size, date, time; brief: names in columns; Ctrl+L information, Ctrl+Q quick view of the file under the cursor in the other panel), Tab switches panels, Enter opens a directory, starts a program (in the background: `FG <pid>` shows it) or views a file in the built-in viewer, Backspace goes up, F3 view, F4 edit in the built-in editor (`edit`'s keys; Shift+F4 a new file), F5 copy, F6 move or rename, F7 make a directory, F8 delete — the marked entries or the one under the cursor, whole trees, between volumes too; the copy and move dialogs offer the other panel's directory (`A:/…`, `ram:/…`, or a new name); existing targets are overwritten or skipped on request; a progress window shows the file and the total, Esc stops, and a failed step asks Retry / Skip / Abort (a partly written copy is removed). Ins/+/-/* mark, Ctrl+F3–F6 sort by name/extension/time/size, Ctrl+H hidden files, Ctrl+R reread, Ctrl+U swap panels, Alt+F1/F2 volume, Alt+F7 find by mask from the current directory down, F9 menu, F1 keys, F10 or Esc quit. Like the editor, fm asks for a file capability (`REQUEST_FILE`): it can change `ram:` and `data/` on the boot disk.
* `df` — the volumes: label, FAT type, cluster size, size, used and free space in KiB.
* `fsck [A:|ram:]` — checks FAT volumes without changing them: `vfs_server` follows every chain from the directory tree and reports lost clusters (and the chains they form), cross-linked clusters, chains that run into a free or bad cluster, files whose size does not match their chain, invalid entries and the dirty flag, with the first problem's path.
* `top` — task monitor: uptime, task states, a busy bar per CPU, load averages, kernel memory, IPC/syscall/interrupt rates; a table with PID, parent, state, CPU, %CPU (from run-time deltas), CPU time, syscalls/s, memory, heap, shared mappings, capabilities and endpoints. P/M/N/T sort by CPU, memory, PID, time; S hides services; t shows the spawn tree; Enter shows a task's details (what it waits for, memory, address space, capabilities, quotas); +/- refresh interval; q or Esc quits.
* `memmap` — memory map in four views (Tab or 1–4): the physical address space as a coloured bar with the firmware ranges and the platform layout (m merges or shows raw ranges, z zooms to RAM); the kernel arena by use with its limits and fragmentation; the address space of a chosen task with guard pages, heap and shared mappings; task and endpoint quotas as a tree by spawner.
* `load` — graphs over 30 s (1) or 10 min (2): busy time per CPU (c: all CPUs in one), interrupts, syscalls, IPC messages and context switches per second, kernel arena and tasks; load averages.
* `hw` — hardware: processor (CPUID: vendor, model, NX, invariant TSC, APIC, SSE/AVX), TSC rate and clock resolution, the framebuffer, PCI devices with class, BARs, IRQ and the service holding each, interrupt lines with counts and holders, platform memory.
* `RUN dzen-clock` — five color indicators for time (`dzen-clock.elf`); **D** toggles the thin digital time, **C** selects a simple 100-second orbit, **P** selects an orbit with 10-second ticks, **H** hides/shows the title and key hints.
* Console programs (`uptime`): a program that asks for the console in its ELF has no screen; in the foreground the shell stays in front, shows its output and waits for it (Esc or Ctrl+C stops it). `LOGS` also reads the output of the last such program that exited, so `run uptime &` then `logs <id>` works.
* `RUN <name> [arguments] &` — launch a new background instance and retain the shell. Arguments reach the program through `mind::process::args()`.
* `<name> [arguments]` — any word that is not a shell command runs the program of that name in the foreground (`say hello`, `listen 2`).
* `RUN <name> [arguments]` (without `&`) runs in the foreground. Repeating the command creates independent instances with different PIDs. Up to eight application tasks can coexist besides the services.
* `PS` — show PID, program, state, foreground/background, assigned CPU, scheduling count, CPU timer ticks, and syscall count. The shell is a task like the others; the footer shows its PID.
* `FG <id>` — show an existing application's screen (services have none) and route keyboard/UART input to it, preserving its PID and state.
* `KILL <id>` — terminate that instance; the kernel then frees its image, stack, screen, private heap and page tables.
* `LOGS <id>` — read and drain that instance's last 4096 bytes of buffered output. Foreground output is also printed to UART with a PID prefix; background output stays buffered so it does not interrupt command entry.
* `CPUS` — show online CPU/APIC IDs, per-CPU timer counters, busy and idle time (TSC), context switches and interrupts.
* `dmesg [-f] [-l debug|info|warn|error] [-s <name or PID>] [-n <count>]` — the system log from `logd`: seconds since boot, the source as `logd` stamped it (`vfs_server(8)`), the text; `-f` follows new records, `-l` hides lower levels, `-s` keeps one source, `-n` the last records; gaps (records dropped from the ring) are shown. `logger <text>` (a shell command) adds a line from the shell — whatever the text claims, its source is the shell.
* `uptime` — a console program: uptime, load averages over 1/5/15 minutes, current CPU load and task count, from `sysmon`.
* `FREE` — kernel memory by use: arena used/free and the largest free block, task images, stacks, screens, private heaps, kernel pages, page tables, memory objects, DMA, mapped memory.
* `PHYSMAP` — the physical memory map: UEFI ranges and the platform layout (kernel, arena, boot images, framebuffer, device BARs).
* `PMAP <id>` — the address space of a task: code and data segments, stack with guard pages, screen, info page, mailbox, heap blocks and shared mappings with their rights.
* `STAT <id>` — task details: state and what it waits for, run time, syscalls, IPC counts, memory, capabilities, quotas.
* `CAPS <id>` — the capabilities of a task: slot, generation, kind, rights, derivation node and parent.
* `ENDPOINTS`, `IRQS`, `DEVICES` — endpoints with server, holders, waiting senders and traffic; interrupt lines with holder and count; PCI functions with class, BARs and the task holding them.
* `ls [path]`, `cat <file>` — list a directory (`ram:` is the RAM disk) with sizes and times; show a text file.
* `write <file> <text>`, `mkdir <path>`, `rm <path>`, `mv <from> <to>`, `sync` — change files on `ram:` and in `data/` (the shell's file client may write there only); changes are flushed at once.
* `CLOCK` — show the monotonic clock (ns), its resolution and the calibrated TSC frequency.
* `DATE` — show the calendar date and time from the RTC (no time zone).
* `FAULTS` — show the last 16 application exceptions: PID, CPU, exception vector/error code, instruction and fault addresses.
* `HEAP` — allocate and format a test string, release it, and report total runtime heap usage, free bytes, and whether the test allocation was freed. The measurement is serialized with program allocations so concurrent heap activity cannot produce a false leak report.
* `HELP` — list the available commands.

`RUN` requires a program name; without one it displays usage and the program list. Program names are case-insensitive, and surrounding whitespace is ignored. Unknown names leave you in the shell with an error message.

**Keyboard.** Programs receive key events, not scan codes: a 32-bit word with the character, a key code and the Shift/Ctrl/Alt modifiers (`common/abi.rs`, `KEY_*`). `ps2_kbd` decodes the PS/2 keyboard (arrows, Home/End, PgUp/PgDn, Ins/Del, F1–F12, keypad with Num Lock, Caps Lock) and has a US and a Russian (ЙЦУКЕН) layout: **Ctrl+Shift** or **Alt+Shift**, pressed and released without another key, switches it; Ctrl/Alt shortcuts stay positional (Ctrl+C is the same key in both layouts). The shell decodes the UART as a VT100/xterm terminal: escape sequences for the same keys with modifiers, UTF-8 text (a host terminal types Cyrillic directly), CR, LF and CRLF as one Enter, a lone Esc after 50 ms. `RUN keys` shows every event it receives. Ctrl+Z is reserved for the system.

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

The clock asks the `rtc` service with `CALL` (`mind::rtc::seconds_since_midnight`), which returns seconds since midnight (or `usize::MAX` when unavailable). The driver checks for stable readings and handles both BCD/binary and 12/24-hour RTC modes. The application displays RTC time without applying a timezone offset. The supplied launch commands use local time; QEMU otherwise defaults to UTC ([QEMU RTC options](https://www.qemu.org/docs/master/system/invocation.html)).

Both square demos sleep between frames and redraw only changing areas in their own buffers. The clock sleeps between RTC checks and redraws when the second changes. `WAIT` lets other tasks run and wakes early for foreground input. Task resources are reclaimed by the BSP idle loop only after the owning CPU has stopped the task and switched away from its page tables.

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

`mind::mem::Pages::new(bytes) -> Option<Pages>` wraps these calls with `as_slice()`/`as_mut_slice()`, `share()` and automatic free in `Drop`:

```rust
if let Some(mut buffer) = mind::mem::Pages::new(8192) {
    buffer.as_mut_slice()[0] = 42;
} // freed here; exit/kill/fault also reclaims any remaining blocks
```

`MEM_SHARE` accepts only the start of one of the caller's heap blocks (never code, stack or a sub-range beyond the block) and returns a capability slot; `MEM_MAP` maps a received capability into the shared quota (read-only without `CAP_WRITE`) and reports its size.

### System calls

`int 0x80` with the mailbox at `USER_MAILBOX` (`syscall_num`, `arg1`, `arg2`, `msg[4]` in; `result` out). Errors are `usize::MAX - n` (`ERR_INVALID`, `ERR_NO_SLOT`, `ERR_RIGHTS`, `ERR_NOT_FOUND`, `ERR_PEER`, `ERR_NO_MEMORY`, `ERR_BUSY`, `ERR_LIMIT`); `ALLOC` returns 0 on failure.

| # | Name | Arguments → result |
|---|---|---|
| 1 | RDTSC | → time stamp counter |
| 2 | READ_KEY | → next key event of the calling (focused) task or 0 (`KEY_*` in `common/abi.rs`) |
| 3 | LOG | buffer, length → bytes logged |
| 5 | WAIT | milliseconds (10 ms steps, ≤ 60 s) → uptime at sleep |
| 6 | UPTIME | → milliseconds since boot |
| 7 | EXIT | — |
| 8 / 9 | ALLOC / FREE | bytes → address / address → 0 |
| 10 / 22 | IPC_SEND / IPC_CALL | endpoint handle \| timeout ms << 32, reply-capability slot; msg = [cap slot, rights mask, data, data] |
| 11 | IPC_RECV | endpoint handle \| timeout ms << 32, slot for a received capability → arg1 = sender PID, msg = [cap received, flags \| badge << 16, data, data] |
| 23 | IPC_REPLY | arg1 = saved reply slot or 0 for the last caller; msg = [cap slot, rights mask, data, data] |
| 31 | IPC_SAVE_REPLY | → slot of a one-time reply capability for the last caller |
| 12 | ENDPOINT_CREATE | → slot of a new endpoint with all rights; `ERR_LIMIT` past the endpoint quota |
| 13 | SPAWN | `name\0arguments`, length; msg = [image memory slot or `SPAWN_BOOT` \| boot index, ELF length, grant array, count \| flags << 8 \| task quota << 16 \| endpoint quota << 32] → PID — spawn privilege; boot images and services need the platform privilege |
| 32 | PLATFORM_CAP | kind, argument; msg[0] = second argument → new slot — platform privilege (`init`) |
| 33 | DEVICE_FIND | PCI class, mask; msg[0] = n-th match → device index — platform privilege |
| 34–43 | TASK_LIST, TASK_KILL, FOCUS, TASK_LOGS, CONSOLE_READ, NOTICE, FAULTS, CPU_INFO, KERNEL_HEAP, HALT | process control for the shell (see `common/abi.rs`) — control privilege |
| 14 | CAP_DROP | slot |
| 45 | CAP_MINT | slot, rights mask; msg[0] = offset, msg[1] = length (0: to the end), msg[2] = badge of an endpoint child (once, 16 bits) → slot of a narrower child |
| 46 | CAP_REVOKE | slot → number of descendants removed; their mappings are gone everywhere when it returns |
| 47 | MEM_DETACH | heap block address → slot of a move-only memory object; `ERR_BUSY` if the block is shared |
| 15 / 16 | MEM_SHARE / MEM_MAP | block address, bytes → slot / slot → address (arg2 = size) |
| 17 / 18 | PORT_IN / PORT_OUT | port-range slot, port; msg[1] = width 1/2/4, msg[0] = value |
| 27 | PORT_IN_BLOCK | port-range slot, port; msg[2] = buffer, msg[3] = 16-bit words |
| 49 | PORT_OUT_BLOCK | port-range slot, port; msg[2] = buffer, msg[3] = 16-bit words written from it |
| 19 / 24 / 25 | IRQ_WAIT / IRQ_BIND / IRQ_ACK | IRQ slot (and endpoint slot for BIND) |
| 20 | INPUT_EVENT | key event word, msg[0] = attention (Ctrl+Z) — needs the input capability |
| 21 | COMPOSITOR_PULL | slot → 0 unchanged, 1 dirty, 2 new screen in slot — needs the display capability |
| 26 | MEM_PHYS | DMA slot → physical address |
| 16 | MEM_MAP (MMIO) | device-register slot → address, mapped uncached |
| 28 | TASK_ALIVE | PID → 1/0 |
| 44 | CLOCK | → monotonic ns since boot, arg2 = resolution ns, msg[2] = TSC Hz (0: 10 ms tick) |
| 48 | STAT | class (`STAT_*`), argument (PID); msg[0] = buffer, msg[1] = capacity → records after a `StatHeader`: tasks, CPUs, kernel memory, physical map, address space, capabilities, endpoints, IRQs, devices — observe or control privilege |
| 29 | CAP_INFO | slot → kind, arg2 = port base, memory rights or endpoint badge, msg[2] = size/count/endpoint rights, msg[3] = 1 if the memory is sealed |

This is a page-block API. `mind::heap` (feature `alloc`) subdivides it for programs: objects up to 2 KiB come from power-of-two size classes carved out of single pages, objects up to 256 KiB are runs of pages in 1 MiB arenas (at most 12), larger ones get their own block; allocation failure ends in the panic handler, which logs and exits the task. Services keep static memory. `tests/heap_host.rs` checks alignment, disjointness, reuse and exhaustion on the host. `app2` already uses a block for its 64×64 sprite and handles allocation failure by reporting it and returning. Page-table edits are serialized with the scheduler; a process runs on only one pinned CPU, so local invalidation is sufficient. Kernel allocation locks disable local interrupts to avoid allocator/scheduler lock inversion. CR3 invalidation follows the [Intel system programming manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html).

### Interfaces (MIND IDL)

Service interfaces are described in `idl/*.wit`, a WIT subset with a version, size limits and the capability a call may carry ([docs/idl](docs/idl/README.md)); since v0.2 records, enums, strings, bytes, lists and `result<T, E>` travel in a memory buffer lent with the call. `scripts/mind_idl.py` generates client calls and a server-side `decode` that checks every request (method, version, unused bits, capability kind) into `libmind/src/idl/`. `rtc`, `sysmon` (`idl/sysinfo.wit`), `vfs_server` (`idl/vfs.wit`), `logd` (`idl/log.wit`) and the loader's launch sessions (`idl/loader.wit`) are on MIND IDL; the other services still use the numeric conventions of `common/abi.rs` (roadmap C8).

A program states what it needs with `mind::request!(REQUEST_CONSOLE | REQUEST_SYSINFO)`: a `.mind_request` section in its ELF that grants nothing (MC-3.11). The shell, as the user's agent, reads it with the loader's `inspect`, opens a launch session (`begin`), lends what it holds and is willing to give (`grant`: the `sysmon` client in slot 10 for `REQUEST_SYSINFO`, its own VFS client — writes on `ram:` and in `data/` — in slot 7 for `REQUEST_FILE`, its read-badged `logd` client in slot 12 for `REQUEST_LOG`) and starts the program (`commit`); `REQUEST_CONSOLE` starts it without a screen. Nothing is granted by program name.

### Runtime checks

After building, run the host tests for real ELF images, independent `.bss`/relocations, malformed ELF rejection, private mappings and dzen-clock logic:

```bash
rustc --edition=2021 --test tests/runtime.rs -o /tmp/mind-core-runtime-tests
/tmp/mind-core-runtime-tests
rustc --edition=2021 --test tests/heap_host.rs -o /tmp/mind-core-heap-tests && /tmp/mind-core-heap-tests
rustc --edition=2021 --test tests/keys_host.rs -o /tmp/mind-core-keys-tests && /tmp/mind-core-keys-tests
rustc --edition=2021 --test tests/tui_host.rs -o /tmp/mind-core-tui-tests && /tmp/mind-core-tui-tests
rustc --edition=2021 --test tests/viewer_host.rs -o /tmp/mind-core-viewer-tests && /tmp/mind-core-viewer-tests
rustc --edition=2021 --test tests/idl_host.rs -o /tmp/mind-core-idl-tests && /tmp/mind-core-idl-tests   # generated bindings over a loopback
rustc --edition=2021 --test tests/rtc_host.rs -o /tmp/mind-core-rtc-tests && /tmp/mind-core-rtc-tests
rustc --edition=2021 --test tests/sysmon_host.rs -o /tmp/mind-core-sysmon-tests && /tmp/mind-core-sysmon-tests
rustc --edition=2021 --test tests/monitor_host.rs -o /tmp/mind-core-monitor-tests && /tmp/mind-core-monitor-tests   # top, memmap, load, hw on a fake sysmon
rustc --edition=2021 --test tests/fm_host.rs -o /tmp/mind-core-fm-tests && /tmp/mind-core-fm-tests   # the file manager on a disk in memory: browsing, jobs, the editor
rustc --edition=2021 --test tests/block_host.rs -o /tmp/mind-core-block-tests && /tmp/mind-core-block-tests   # block protocol: the write badge
rustc --edition=2021 --test tests/fat_host.rs -o /tmp/mind-core-fat-tests && /tmp/mind-core-fat-tests   # FAT writer and checker vs mkfs.fat, fsck.fat, mtools
rustc --edition=2021 --test tests/edit_host.rs -o /tmp/mind-core-edit-tests && /tmp/mind-core-edit-tests   # the editor: piece table, undo, search/replace, keys, drawing
rustc --edition=2021 --test tests/logd_host.rs -o /tmp/mind-core-logd-tests && /tmp/mind-core-logd-tests   # logd's ring: order, wrap-around, clipping, rate limit
python3 tests/idl_test.py   # MIND IDL generator; fails if libmind/src/idl is stale (regenerate: python3 scripts/mind_idl.py)
python3 tests/font_test.py  # font subset coverage, licence notice; fails if common/font16.rs is stale (python3 scripts/font_gen.py)
```

The QEMU integration test boots an isolated copy of `usb_root`, exercises concurrent instances, `fg`, `kill`, UART/PS2 input, task limits, repeated allocation/freeing, and idle `HLT`. Additional suites check concurrent CPU progress, remote termination, independent SIMD contexts, private heap stress/OOM recovery, deliberate ring-3 faults and capability checks without stopping other programs, the boot services (IPC call/reply with memory capabilities, VFS over the ATA driver, the system log: `dmesg` shows the boot lines of `init` and the services with the PIDs `logd` stamped, a line claiming another source keeps its real one, filters and follow mode), the AHCI driver (`ahci` suite: the disk attached to an AHCI controller), text to speech (`tts` suite: duration and voiced pitch of the captured speech; with `--asr-model <Vosk Russian model directory>` also checks that the words are recognized), the audio gateway (AC97 output captured to a WAV file and checked for the expected tones) files on a raw FAT disk (`vfs` suite, with `mkfs.fat` and `mtools`: the shell writes in `data/`, the host checks the image with `fsck.fat -n` and `mtype`, a reboot reads the files back and finds `ram:` empty), the editor (`edit` suite, with `mkfs.fat` and `mtools`: Latin and Cyrillic text typed and saved on `ram:` and in `data/` of a raw FAT disk, read back, a CRLF file, a boot file read-only; the host runs `fsck.fat -n` and compares the files byte for byte), `fm`'s write operations, `df` and `fsck` (`disk` suite: a tree copied from `data/` to `ram:`, renamed, copied back, deleted, a new directory, an edit in place, `df` before and after; `fsck` finds a chain the harness broke on the boot disk and passes `ram:`; with the chain restored the host's `fsck.fat -n` is clean and mtools reads what fm wrote) and block writes (`block` suite, with `mkfs.fat` and `mtools`: a raw FAT image boots three times — IDE, AHCI, USB — with `tests/block_app.rs` standing in for `vfs_server`; it writes, flushes and reads back sectors through the write-badged clients and the harness checks the image and runs `fsck.fat -n`):

```bash
for fixture in busy_app isolation_app heap_app block_app; do
  rustc --edition=2021 --target x86_64-unknown-none --crate-type bin \
    -C opt-level=3 -C panic=abort -C relocation-model=pic \
    -Z relax-elf-relocations=yes -C link-arg=-Tapp/linker.ld \
    "tests/$fixture.rs" -o "/tmp/mind-core-$fixture.elf"
done
python3 tests/qemu_smoke.py --qemu qemu-system-x86_64 \
  --firmware OVMF.fd --busy-elf /tmp/mind-core-busy_app.elf \
  --isolation-elf /tmp/mind-core-isolation_app.elf --heap-elf /tmp/mind-core-heap_app.elf \
  --block-elf /tmp/mind-core-block_app.elf
# Repeat SMP/fault/heap handling on a single CPU:
python3 tests/qemu_smoke.py --qemu qemu-system-x86_64 --cpus 1 \
  --busy-elf /tmp/mind-core-busy_app.elf \
  --isolation-elf /tmp/mind-core-isolation_app.elf --heap-elf /tmp/mind-core-heap_app.elf \
  --suites smp,isolation,heap,services
```

The suites number applications from 1; the harness counts the boot services in `ps` at startup, adds that number when it sends `fg`/`kill`/`logs` and subtracts it from `PID=` in the output.

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

The default selection includes `kernel`, `bootloader`, `common`, `libmind`, all
applications and services, and current tooling. It excludes historical patches, `legacy`, issue
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
