# MIND CORE

The Minds of the Culture's spaceships — the ultra-powerful AIs controlling General Contact Units and Orbitals — must reside somewhere. No matter how multidimensional and advanced their hardware substrate may be, at a fundamental level, any computing architecture needs two basic things to wake up and become self-aware: a bootloader and an operating system.

Before a Mind can simulate pocket universes, juggle hyperspace vectors, or conduct delicate diplomatic games on behalf of an entire civilization, it requires a reliable and predictable foundation. It is time to start building it.

This repository is the first step toward creating a substrate-independent environment for future Minds. We are starting right now, from the very lowest level of hardware reality.

Every Mind needs its first processor tick. We are providing exactly that.


### Goals and Architecture

* **Substrate Initialization (Bootloader):** Direct interaction with UEFI, seizing control of the bare metal, and preparing physical memory.
* **Kernel:** The basic reality dispatcher. Handling hardware interrupts, system calls, and preemptive multitasking.
* **Isolated Environment (Userspace):** A space for the genesis and parallel execution of high-level processes and future cognitive functions.

The normative requirements are in the [Constitution v1.6](constitution/EN/MIND_CORE_Constitution_v1.6.md) and [RFC 001 Marain v0.4](constitution/EN/RFC_001_Marain_v0.4.md) (Russian texts in [constitution/RU](constitution/RU), index in [constitution/README.md](constitution/README.md)); the order of work, current gaps and the point from which parts can be developed in parallel are in [ROADMAP.md](ROADMAP.md) v1.2 ([Russian](ROADMAP_RU.md)); what the implementation guarantees and under which assumptions is in the platform profile [docs/profile](docs/profile/README.md); system calls, `libmind` and service interfaces are in [docs/api](docs/api/README.md); legacy hardware support, how `init` reports it at boot and how to drop it is in [docs/legacy.md](docs/legacy.md). Near-term work items are in [issues/](issues/README.md) (finished ones in [issues-done/](issues-done), ones that need a person in [issues-human/](issues-human/README.md)); background analyses are in [knowledge/](knowledge). The plan for system tools (file manager, editor, `top`, memory map, load monitor and others) is in [docs/tools](docs/tools/README.md) ([Russian](docs/tools/README_RU.md)). The plan for voice — speaking, listening and acting on what was heard — is in [docs/voice](docs/voice/README.md) ([Russian](docs/voice/README_RU.md)).


---

### Current Runtime

The UEFI bootloader loads the kernel and only the system service images (`BOOT_FILES` in `common/abi.rs`) from the FAT filesystem, enumerates enabled CPUs through UEFI MP Services, and reserves a 64 MiB runtime heap, a BSP stack, and a low-memory AP bootstrap page. The kernel takes over after ExitBootServices and starts exactly one program in ring 3, `init`, which holds the bootstrap authority and starts the other services, including the command shell. Applications are not kept in memory: the `loader` service reads them from the boot disk through `vfs_server` when they are started.

* **Toolchain:** Pure Rust (`no_std`, `no_main`), utilizing `naked_functions` and `abi_x86_interrupt`, compiled for `x86_64-unknown-none` and `x86_64-unknown-uefi` targets. Programs use the `libmind` SDK.
* **Memory and privilege:** Every task runs in ring 3 with IOPL=0 and a private four-level page table/CR3. Each `RUN` creates a fresh ELF image, 64 KiB user stack with unmapped guard pages, syscall mailbox, input/log queues, and (for applications) a screen buffer. Code is RX; writable data, stack, mailbox and screen are NX. The kernel's supervisor mappings are inaccessible to tasks.
* **Capabilities:** Each task has 96 capability slots. A capability names an IPC endpoint (with read/write/grant/keep rights), a shared memory block (read/write/grant rights), a DMA region, device registers (MMIO, mapped uncached), an I/O port range, an interrupt line, or a privilege (input, display, spawn, process control, platform). Drivers receive only the capabilities for their device; applications receive send-only endpoints of the RTC, VFS, audio, loader and TTS services. Every capability a task has was granted explicitly by its spawner (`init` for services, `loader` for applications). Capabilities are named by handles `slot | generation << 8`: slots 1–22 are fixed by convention (generation 0), slots the kernel hands out get a new generation when freed, so a stale handle is rejected instead of naming a newer capability. Every capability has a parent: a copy (IPC transfer, spawn grant) or a `CAP_MINT` (narrower endpoint rights, a port sub-range or a page-aligned memory/DMA/MMIO sub-range) is a child of its source, a move (`CAP_TRANSFER_MOVE`, `GRANT_MOVE`) keeps it and empties the source slot. `CAP_REVOKE` removes every descendant from every task, including one waiting in a blocked send, and returns how many were removed; the holder keeps its own capability. A memory capability without the write right maps read-only; mappings made from a revoked memory, DMA or MMIO capability are removed, and when the holder runs on another CPU `CAP_REVOKE` returns only after that CPU has switched address space (a lease ends at a defined point). `MEM_DETACH` turns a heap block nobody else refers to into a memory object: the pages leave the caller's address space and the capability (read/write, no grant) can only be moved — copying a writable memory capability needs the grant right — or minted read-only. `CAP_INFO` reports a memory range as *sealed* when no writable capability, writable mapping or DMA region overlaps it, so a reader can check that nobody writes while it reads (`SHARE_RO`).
* **Dynamic program memory:** Syscalls 8/9 allocate and free zeroed private page blocks (RW+NX, trailing guard page; up to 64 blocks). Task memory — images, stacks, screens, heap blocks and memory objects — comes from a frame pool over the free RAM of the firmware map, not from the kernel's 64 MiB arena. Private memory is limited by a memory quota delegated at `SPAWN` (16 MiB by default; a program asks for more with `mind::request!(…, memory: MiB)`), charged to the task and to every spawner above it. Mapped shared memory has its own 256 MiB limit. The test program `memtest` takes 128 MiB of heap and shares a 64 MiB sealed object between two tasks (issue 150). Memory that another task still maps or holds by capability is retained by the kernel until the last reference is gone, so freeing, exiting or killing an owner cannot leave a dangling mapping.
* **CPU configuration:** The QEMU launchers expose four cores. The kernel starts APs itself with INIT/SIPI and can use up to eight enabled xAPIC CPUs. Each CPU has its own GDT, TSS, interrupt-entry stack, idle stack, and double-fault/NMI stacks. `cpus` reports actual online CPUs and interrupt counters.
* **Scheduling:** New applications are assigned to the CPU with the fewest applications and remain pinned there. Round-robin scheduling on each CPU preserves GPRs and x87/SSE state. The BSP receives the 100 Hz PIC/PIT tick through LAPIC ExtINT and sends scheduling IPIs to online APs. A spinlock with local interrupts disabled serializes scheduler/syscall work. Idle CPUs use `HLT`; a CPU that sleeps while one of its tasks becomes ready (an IPC reply, an IRQ) gets a wake IPI instead of waiting for its next tick. The BSP idle loop reclaims exited tasks once no CPU runs them.
* **System calls and faults:** The DPL-3 `int 0x80` gate is the only user entry into kernel services. Every buffer pointer is checked against the calling task's user mappings. User exceptions terminate that task and appear in `faults`; a kernel exception remains fatal.

### System services

The kernel contains no list of services and no per-service capability table. It starts boot image 0, `init`, with its own endpoint and two privileges: **platform** (mint capabilities over resources the kernel has validated — legacy port ranges, IRQ lines, PCI BARs and IRQs from the kernel's enumeration, the framebuffer, DMA regions, privileges) and **spawn**. That is the whole bootstrap authority; `init` then starts the other boot images in `BOOT_SERVICES` order (`common/abi.rs`), each with exactly the capabilities listed below (`SPAWN` with a grant list), and keeps DMA regions across driver restarts. Endpoints have no global numbers: `init` creates one per service and keeps only a *keeper* capability (`CAP_KEEP`: may mint receive rights, cannot receive itself); the server gets a receive child, clients get write/grant children (copies of the keeper narrowed at spawn, or minted with a badge). A restarted service gets a new receive child of the same endpoint, so clients granted earlier reach it again; while no server runs their calls fail with `ERR_PEER`. `ahci`, `usb_storage`, `virtio_blk`, `nvme`, `virtio_input` and `virtio_net` start only when `init` finds their device (`DEVICE_FIND`), so later PIDs depend on the machine. Services cannot be brought to the foreground (except the shell) and each runs once. `init` supervises them: the kernel sends it an exit notice when one ends (`TASK_WATCH`), and it restarts the service at most three times in 60 s, then quarantines it (`[INIT] <name> QUARANTINED`) until the operator types `RUN <service> &`, which also resets the budget; `svc` stops, starts and restarts services through `init`'s lifecycle requests (`idl/init.wit` 1.1). A send still queued for the dead instance fails with `ERR_PEER` rather than reaching the new one. If `init` itself ends, the kernel halts the system. Before restarting a driver, `init` stops its device (no decoding, no DMA) and clears its DMA region; the VFS's block client notices the new driver instance by its PID and attaches its buffer again. `init` gives up the platform privilege before `READY` and restarts services only with the capabilities it handed out at their first start. The shell's `quotas` command shows each task's task and endpoint quota (used/limit), and `stat <class> [pid]` prints the kernel statistics of `STAT` (tasks with run time and IPC counts, CPUs with busy/idle time, the kernel arena by category, the physical layout, a task's address space and capabilities, endpoints, IRQ lines, PCI devices). Scheduling has two bands — `init` and the services before applications — and per-task CPU budgets per period (`SCHED_SET`, shell command `budget <pid> <ms> <period ms>`), enforced at the 10 ms tick. Statistics need only the observe privilege (`CAP_KIND_OBSERVE`); reading other tasks' output, kill and focus stay with process control.

| Service | Capabilities (granted by init) | Role |
|---|---|---|
| `init` | own endpoint, platform and spawn privileges (from the kernel), process control (minted for itself) | service policy; the lifecycle of services and applications (`idl/init.wit` 1.1: list, start, stop, restart, stop an application) |
| `logd` | service endpoint, observe privilege | the system log (`idl/log.wit`): 256 records of up to 200 bytes in a 64 KiB ring, each stamped with the time it arrived and the sender's PID and task name (from IPC and the kernel's task records, never from the text); at most 64 records a second per sender, the rest refused and counted; reading needs the shell's read badge |
| `rtc` | service endpoint, ports 0x70–0x71 (aarch64: the PL031's registers) | CMOS clock, or the PL031 on aarch64 `virt`; serves `idl/rtc.wit` (seconds since midnight, days since 2000-01-01) |
| `ps2_kbd` | ports 0x60, 0x64, IRQ 1 and 12, input | PS/2 keyboard → key events, and the PS/2 mouse → pointer events (buttons, movement, wheel) for the focused task if it asked for them (`mind::input::pointer`, issue 156); (modifiers, F-keys, navigation keys, US/Russian layout) for the focused task; serves `idl/keyboard.wit` (layout and switch key) to the shell's `keymap` |
| `virtio_input` | service endpoint, the BARs and MSI-X vectors (or IRQs) of up to two VirtIO input devices, 24 KiB DMA, input | QEMU's tablet (`-device virtio-tablet-pci`) → absolute pointer events: where the host's pointer is, as a share of the screen (issue 161), so the system's pointer follows it; a VirtIO mouse → movement; a VirtIO keyboard (issue 202) → keys through the PS/2 decoder, and the keyboard service (`idl/keyboard.wit`) where there is no `ps2_kbd`. Exits when there is no such device |
| `compositor` | GOP framebuffer, display | copies changed pixels of the focused screen to the framebuffer; serves `idl/display.wit` (the mode, a sealed read-only copy of the screen) to the shell's `screenshot` |
| `ata` | service endpoint, ports 0x1F0–0x1F7, 0x3F6 | primary IDE channel, PIO LBA28 |
| `ahci` | service endpoint, ABAR (MMIO), 128 KiB DMA | first SATA disk on an AHCI controller (class 01:06:01) |
| `usb_storage` | service endpoint, xHCI BAR0 (MMIO), 256 KiB DMA | first USB mass storage device (Bulk-Only, SCSI) on an xHCI controller (0C:03:30) |
| `virtio_blk` | service endpoint, the BAR of a VirtIO block device's modern interface, 128 KiB DMA | first VirtIO disk (issue 202): the boot disk on aarch64 `virt`; requests polled; absent without the device |
| `nvme` | service endpoint, an NVMe controller's registers (BAR0), 128 KiB DMA | the first namespace of the first NVMe controller (class 01:08:02, issue 205): admin and one I/O queue pair, commands polled, 512-byte sectors; absent without the device. `qemu_smoke.py --disk nvme` boots from it on both architectures |
| `ramdisk` | service endpoint | an 8 MiB block device in its own memory: the RAM disk `ram:` |
| `vfs_server` | service endpoint, write-badged clients of the running block drivers and of `ramdisk`, an `rtc` client | mounts the boot disk and `ram:` (FAT12/16/32) and serves files and directories through handles (`idl/vfs.wit`), writing included |
| `loader` | service endpoint, RTC/VFS/audio/TTS client endpoints, spawn privilege | reads application ELF files from the disk and starts them with the standard client capabilities; in a launch session (`idl/loader.wit`) also with the capabilities the launcher lends (slots 7–12) |
| `audio_gw` | service endpoint, AC97 BARs, its IRQ, 200 KiB DMA | audio gateway: playback (PCM, tones) and microphone capture through AC97 DMA rings |
| `tts` | service endpoint, audio gateway client | text to speech (Russian and Latin script), streamed to `audio_gw` |
| `virtio_net` | service endpoint, the memory BAR of a VirtIO network card's modern interface and an MSI-X vector (or, for a legacy card, I/O BAR0 and its IRQ line), 160 KiB DMA | network card driver (modern VirtIO with MSI-X, or legacy): raw Ethernet frames (`idl/net.wit`); its clients are `netstack` and the shell's `net` diagnostics. One instance per card (issue 105): `virtio_net` drives the first VirtIO card in PCI order, `virtio_net#1` the second, each with its own device, DMA region, endpoint and restart budget. Completes TCP/UDP checksums of sent frames when the stack asks and the card offers it (issue 106). Frames to and from `netstack` go through a ring the stack lends it (issue 107), not one call each |
| `netstack` | service endpoint, clients of `virtio_net` and `virtio_net#1` | network stack (smoltcp): DHCP with a static QEMU fallback, ARP, ICMP echo, a DNS resolver, UDP and TCP sockets (`idl/socket.wit`); an interface per card with its own DHCP, a flow goes out through the card whose network holds the destination, else through the first with a gateway; shell commands `ip` (every card with its frame counters; `ip offload on|off`: transmit checksum offload, off by default, see [docs/profile/network.md](docs/profile/network.md)), `ping`, `nslookup`, `fetch`; benchmark program `netbench` |
| `netpolicy` | service endpoint, network stack clients (an unbadged one to mint grants from, the policy badge), a VFS client | network policy broker: grants each program that asks for the network (`REQUEST_NETWORK`) a stack client limited to the destinations, term and volume `netpolicy.txt` names for it; `netgrants`, `netrevoke <program>` in the shell; test program `netcheck` |
| `keystore` | service endpoint, an `rtc` client | key service: the device key (Ed25519 from RDRAND at boot, held only in its memory) and its self-signed certificate; signs only for the TLS service's badge, only TLS 1.3 client handshakes, at most 4096 times per boot, each logged (`idl/keystore.wit`) |
| `tls` | service endpoint, `rtc` and VFS clients, the key service's signer client | TLS 1.3 client (rustls with a RustCrypto provider) over flows its clients lend (`idl/tls.wit`), so their network policy holds; server certificates verified against `tlsroots.pem`; the device certificate on request; shell commands `https`, `tls cert`. Needs RDRAND (in QEMU: `-cpu qemu64,+rdrand`, `-cpu max` or `-cpu host`) |
| `windows` | service endpoint, its own program client | window broker (`idl/window.wit`, issue 157): keeps the windows of programs (surfaces in its memory, lent to the program and to the window manager) so they outlive the manager; init gives it a 128 MiB memory quota, since a pixel window has room for the screen (issue 163); one manager at a time detaches (windows stay, hidden) or closes all; test programs `wintest`, `winmgr` |
| `sysmon` | service endpoint, observe privilege | system information (`idl/sysinfo.wit`): the kernel's `STAT` records, load samples every 100 ms (300 kept) and every second (600 kept), load averages; at most 20 requests at once and 40 per second per client; who holds what (`holders`, `authority`, the derivation links in `caps`) only to the client with the authority badge |
| `shell` | screen, init/loader/sysmon and other client endpoints, process control, input, the serial line (COM1 ports; the PL011 on aarch64) | the `MIND>` command shell |

Every service but `logd` also holds a client of `logd` in slot 12 (`SLOT_LOG`): its `println!` lines go to the system log as well as to its own buffer (`LOGS`). In the default QEMU setup (IDE disk, no xHCI/AHCI) eighteen services run and applications start at PID 19. The kernel has room for 32 tasks. Tasks and endpoints are charged to quotas delegated at spawn: `init` holds the root quota and gives `loader` eight application tasks (the application limit) and 32 endpoints; each application may create four endpoints.

The shell runs in ring 3. It reads the UART itself, forwards bytes to the focused program through the input privilege, and prints that program's console output to COM1 with a `[PID n]` prefix. Focus is a kernel mechanism set only by the holder of process control: the focused task's screen is shown and receives keyboard input; when it exits, or on Ctrl+Z (an input event flagged as attention), focus returns to the shell and the shell gets a notice. The kernel writes to COM1 only its boot line, kernel exceptions and panics.

### IPC

Endpoints are rendezvous points. `IPC_SEND` blocks until a receiver takes the message; several senders queue in arrival order. `IPC_CALL` sends and then waits for the server's `IPC_REPLY`, so a client needs no reply endpoint of its own. A message carries two data words and, optionally, one capability from the sender's slot (the rights mask narrows endpoint rights only — memory is narrowed with `CAP_MINT`; transfer requires the grant right on the endpoint used; with `CAP_TRANSFER_MOVE` in the rights word the capability is moved, not copied). The receiver learns the sender's PID and the badge of the capability it used: `CAP_MINT` can label an endpoint capability with a badge once (children keep it), so one endpoint tells clients with different rights apart. If a server dies while a client waits for its reply, the client is woken with `ERR_PEER`; a send to an endpoint that no live task can receive from fails with `ERR_PEER` at once. At most four senders wait on one endpoint; another send fails with `ERR_BUSY` (libmind waits a tick and retries). The endpoint word may carry a timeout in milliseconds (`IPC_TIMEOUT_SHIFT`, `call_timeout`, `recv_timeout`, `send_timeout`): when it passes, the operation fails with `ERR_TIMEOUT`, a queued send is withdrawn with its capability, and a server's late reply to an abandoned call fails with `ERR_PEER`. A server can keep a client waiting: `IPC_SAVE_REPLY` moves the pending reply into a one-time capability (it cannot be transferred), the server goes on receiving other requests and later answers with `IPC_REPLY` naming that slot.

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
| `process` | `exit`, `spawn` (through `loader`), `alive`, `log`, `print!`/`println!`; `request!` (what the program asks its launcher for: `REQUEST_CONSOLE`, `REQUEST_SYSINFO`, …); `about!` (what the program does, for `--help` and the shell's `help <name>`; `section` finds it in a program file); `spawn_image`/`loader_done` for `loader` |
| `time`, `input` | `sleep`, `uptime_ms`, `rdtsc`; `read_key`, `wait_key`, `wait_or_exit` (Esc exits) → `Key` (`code()`: character, Enter, Esc, arrows, Home/End, PgUp/PgDn, Ins/Del, F1–F12; `char()`, `text()`, `shift()`/`ctrl()`/`alt()`) |
| `tui::viewer` | file viewer core shared by `view` and the file manager: a `Source` read through a window cache, text with or without wrapping and line numbers, hex dump, search ignoring case, go to |
| `tui` | text UI on the 8×16 font: `Grid` of cells (text with clipping, frames with titles, fills, bars in 1/8 cells, braille time-series graphs), `Terminal` (the grid on the program's screen, redraws only changed cells, cursor), themes `CLASSIC` (Norton Commander colours) and `DARK`, widgets (`ListState`, `InputLine` with UTF-8 editing, `History`, `MenuBar`, `fkey_bar`, dialogs, `progress`) |
| `keys` | ring 3 key decoders: `Ps2` (scan code set 1, modifiers, Caps/Num Lock, US and Russian layouts) and `Vt` (UART: VT100/xterm sequences, UTF-8); `Key::latin()` gives letter commands in either layout |
| `ipc` | `Endpoint::{create, send, call, recv}`, `reply`, `drop_cap`, `Message` |
| `mem` | `Pages` (private blocks, freed on drop, `share()`), `Mapping` (shared memory by capability), `dma_physical` |
| `dev` | `Ports`, `Irq`, `Mmio`, `Dma`, `Uart` (a 16550 or a PL011, by the slot's capability), `input_event`, `compositor_pull`, `cap_info` — for drivers |
| `keyboard` | the keyboard service (`idl/keyboard.wit`) and key delivery over the PS/2 decoder, shared by `ps2_kbd` and `virtio_input` |
| `block` | block device client (`Device`) and the driver loop (`serve`, `Driver`) |
| `gfx` | `Screen`: pixels, rectangles, 8×8 font text, UTF-8 text in the 8×16 font (`text16`, `glyph16`) |
| `font16` | MIND Mono 16, the 8×16 text font: a subset of Terminus Font under the SIL OFL 1.1 with Cyrillic, box drawing, block elements and braille ([fonts/](fonts/README.md)) |
| `stat` | kernel observation (`STAT`): records of tasks, CPUs, memory, physical map, address spaces, capabilities, endpoints, IRQs, devices, and their names |
| `rtc`, `fs`, `audio`, `tts` | clients of the RTC, VFS, audio and speech services (`fs::File`, `fs::Dir`, `fs::list`/`mkdir`/`remove`/`rename`/`metadata`/`volume`, `fs::use_endpoint` for a lent VFS client, `fs::check`, `audio::Stream`, `audio::wait_space`, `tts::say`) |
| `util` | `Decimal`, `FixedBuf` (`core::fmt::Write` into a fixed buffer) |
| `pattern` (feature `alloc`) | name masks (`*`, `?`) and simple regular expressions with case folding (`fm`, `find`, `grep`) |
| `heap` | program heap: with the cargo feature `alloc` (`libmind = { path = "../libmind", features = ["alloc"] }`) a program gets a `GlobalAlloc` and can use `Vec`, `String`, `Box` after `extern crate alloc;`; `mind::heap_stats()` |

The SDK also supplies the panic handler (logs the message and exits the task) and `memset`/`memcpy`/`memmove`/`memcmp`. `common/abi.rs` remains the single ABI definition shared by the kernel, the bootloader and `libmind`.

### Text to speech

`tts` is a formant synthesizer written for MIND CORE (no recorded voice data): text → phonemes → targets for five cascade formants, a nasal pole/zero pair and a parallel noise branch, with a KLGLOTT88 glottal source at 16 kHz, then upsampled to 48 kHz stereo and streamed to `audio_gw`. Pauses and the end of a phrase are digital silence: 30 ms after the excitation stops the filters' state is cleared, so no fixed-point limit cycle keeps a quiet tone going.

* Russian: letter-to-sound rules with palatalization, final devoicing and voicing assimilation, «-тся», «-ого/-его», akanye/ikanye around the stress. Stress comes from `phonetics/data/stress_ru.txt` — about 13 700 word forms among the 30 000 most frequent ones on which the heuristic is wrong, plus forms with «ё» — and otherwise from the heuristic: words ending in a consonant are stressed on the last syllable, others on the penultimate, «ё» is always stressed. The lookup is a binary search that treats «е» and «ё» as the same letter, so text typed without «ё» («еще», «зеленый») gets it back. The forms are taken from the OpenRussian dictionary (CC BY-SA 4.0, https://github.com/Badestrand/russian-dictionary) by `python3 scripts/stress_openrussian.py <CSV directory> ru_50k.txt 30000 > forms.txt` (frequency list from hermitdave/FrequencyWords); homographs such as «дома» or «замок» are left out. `python3 scripts/stress_exceptions.py forms.txt` merges forms written with the stressed vowel in upper case (дОбрый) and keeps only those the heuristic gets wrong.
* English (Latin script): pronunciations of about 18 700 of the 20 000 most frequent English words from CMUdict (`phonetics/data/lexicon_en.txt`, BSD licence, binary search), a few hand corrections in `text.rs`, and simplified spelling rules for other words. The phoneme set adds the English vowels [ɪ ʊ ʌ ɝ] and voiced [ð]. `python3 scripts/lexicon_en.py cmudict.dict en_50k.txt 20000` rebuilds the lexicon (frequency list from hermitdave/FrequencyWords). Digits are read one by one.
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

The block drivers read and write (ATA WRITE SECTORS and FLUSH CACHE, AHCI WRITE DMA EXT and FLUSH CACHE EXT, USB SCSI WRITE(10) and SYNCHRONIZE CACHE(10), write protection from MODE SENSE(6)); `write` and `flush` (`idl/block.wit` 1.1; the data travel as sealed read-only memory, so nobody can change them while the driver writes) are served only to a client whose endpoint capability carries the write badge, which `init` gives to `vfs_server` alone (Appendix B.6). An endpoint capability can carry a 16-bit **badge**, set once by `CAP_MINT` (`mind::ipc::mint_badged`); the server sees it with every message sent through that capability (`Received::badge`), so one endpoint serves clients with different rights. 

Each request is a MIND IDL buffer call: the path and the file data (up to 16 KiB per `read` or `write`) travel in a buffer the client lends for one call and revokes before reading the reply. A listing gives each entry's size, attributes (`VFS_ENTRY_*`: directory, hidden, system, read-only, archive) and FAT modification time (`mind::fs::fat_time`), in pages of 16 entries. Handles of dead clients are recycled. A power loss before a flush can lose the changes since the last one; a power loss during one leaves what FAT allows (no journal): see [docs/profile/threat-model.md](docs/profile/threat-model.md). `RUN files` lists the boot disk and reads two files. The launchers' `fat:` drive is an IDE disk; the USB image is read through `usb_storage`; NVMe is not supported yet.

### Audio gateway

`audio_gw` drives an AC97 controller found on PCI by the kernel: a ring of 32 DMA buffers of 4 KiB (48 kHz, 16-bit stereo), buffer-completion interrupts delivered as IPC messages, and client PCM copied from a buffer the client lends for one call (read-only). `mind::audio` offers `info`, `tone(hz, ms)`, `play`/`play_all` (interleaved `i16`) and `stop`. `RUN beep` plays three tones and a PCM sweep. The microphone side is a second ring of 16 capture buffers (AC97 PCM in, 48 kHz stereo): `AUDIO_RECORD_START/READ/STOP`, `mind::audio::record_start`, `record_read`, `record_stop`. `listen [seconds]` (1–10, default 3) records with a level meter on its screen, reports frames, peak and RMS, and plays the recording back. `mind::voice` (feature `alloc`) is the voice front end ([docs/voice](docs/voice/README.md), V0): a `Source` (`Microphone`, or `Wav` — a 16-bit PCM file at any rate) becomes a 16 kHz mono `Stream` (polyphase low-pass, nothing above 8 kHz folds back), and `Detector` cuts it into utterances (frame energy against an adaptive noise floor plus zero crossings, 200 ms hangover, 0.2–8 s). `listen --vad [seconds]` prints each utterance the microphone hears (start, length, level in dBFS), `listen --vad --wav FILE` does it for a file, `listen --wav FILE` converts a file to 16 kHz mono and plays it back. QEMU's `wav` audiodev has no capture; with `-audiodev none` the microphone delivers silence at the real rate (the `listen` test suite uses it), with `pa`/`alsa`/`dsound`/`coreaudio` it records the host microphone. When the ring is full, `AUDIO_WAIT` parks the client with a saved reply capability and the gateway answers it from the AC97 interrupt that frees buffers, so producers wait for space instead of polling; while a client waits the gateway also looks at the ring every 20 ms, because an interrupt line shared with another device (the launchers' network card) may not reach it (issue 159). `mind::audio::Stream` collects samples in its buffer and lends it in 16 KiB blocks. Add the device to QEMU with, for example, `-audiodev wav,id=snd0,path=out.wav -device AC97,audiodev=snd0` (or a `pa`/`dsound`/`coreaudio` audiodev). Without AC97 the gateway answers `DEVICE=false`.

---

### How to Build and Run

**Prerequisites:**
You will need [rustup](https://rustup.rs) and a virtual machine capable of UEFI execution. The nightly toolchain and its targets are pinned in `rust-toolchain.toml`; rustup installs them on the first build (or run `./01_prepare_env.sh`). Dependencies are pinned by the committed `Cargo.lock` files.

```bash
rustup toolchain install   # reads rust-toolchain.toml
```

CI (`.github/workflows/ci.yml`) builds with the same toolchain on Ubuntu 24.04 and runs the host tests and all QEMU suites. The same groups run on a local machine: `scripts/ci_local.sh` (the working tree; `--main`, `--ref BRANCH` merged with main, `--all` branches, each in a temporary worktree; a PASS/FAIL table), and `scripts/ci_watch.sh` fetches origin every 10 minutes and runs it on each new commit of main and of the other branches (history in `~/.cache/mind-ci-watch/history.log`). Contributors who work through coding agents, and the agents themselves, follow [AGENTS.md](AGENTS.md): tracks, issue ranges, branches, the gate to `main` and a brief to paste into a session.

*Note: Ensure QEMU and the OVMF firmware (UEFI for QEMU) are installed on your host system.*

**Build & Execution:**
The compilation and packaging pipeline is automated via bash scripts.

1. Ensure your build script is executable:
```bash
chmod +x 02_build.sh

```


2. Build the kernel, all programs and services, and the UEFI bootloader. The script places the ELF files in `usb_root/` and the bootloader in `usb_root/EFI/BOOT/` (for aarch64: `ARCH=aarch64 ./02_build.sh` builds `aarch64_root/`, see [aarch64](#aarch64-qemu-virt-issues-201204) below):
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
  -cpu qemu64,+rdrand \
  -audiodev pa,id=snd0 -device AC97,audiodev=snd0 \
  -nic user,model=virtio-net-pci \
  -device virtio-tablet-pci

```



*(Adjust the path to `OVMF.fd` depending on your OS and package manager).*

What the last four lines add:
- **`-audiodev … -device AC97`:** the sound card. `audio_gw` drives an AC97 controller; without one, `beep`, `say` and `listen` find no device and stay silent. The `-audiodev` driver is the host's sound system: `pa` (PulseAudio, also PipeWire's), `pipewire`, `alsa`, `sdl`, `dsound` on Windows, `coreaudio` on macOS. `wav,path=out.wav` writes the sound to a file instead of playing it.
- **`-cpu qemu64,+rdrand`:** a processor with RDRAND. The TLS and key services refuse to work without it.
- **`-nic user,model=virtio-net-pci`:** a VirtIO network card on QEMU's user networking.
- **`-device virtio-tablet-pci`:** a VirtIO tablet. It tells the system where the host's pointer is (`virtio_input`, issue 161), so the system's pointer follows it exactly and reaches every edge of the screen; QEMU does not have to capture the pointer. Without it the PS/2 mouse reports only movement: the two pointers drift apart, and the host's leaves the window before the system's reaches the edge.

On Linux, `./03_run_qemu.sh` does the same with the VM settings of the other launchers: it uses `OVMF.fd` next to the script or, without it, the distribution's split `OVMF_CODE`/`OVMF_VARS` firmware as pflash (with a private copy of the variables); extra arguments go to QEMU (for example `-display none`). It adds the sound card, the network card, a VirtIO tablet and RDRAND as above:
- **Sound:** the backend is the first of PipeWire, PulseAudio, ALSA (with `/dev/snd`) and SDL (in a desktop session) that starts on this host. Without any, it starts without sound and says so. `MIND_AUDIO=<driver>` picks the backend, `MIND_AUDIO=none` leaves the card out.
- **Network:** `MIND_NET=none` leaves the network card out.
- **Pointer:** a VirtIO tablet (`virtio-tablet-pci`), an absolute device: QEMU does not grab the pointer, which moves freely in and out of the window. `MIND_POINTER=ps2` leaves only the PS/2 mouse, which QEMU grabs on a click (Ctrl+Alt+G releases it); under Wayland, WSLg or Windows that grab may not hold the pointer in the window.
- **CPU:** `MIND_CPU=<model>` replaces the CPU model.

On Windows, use `03_run_qemu_windows.bat` or `03_run_qemu_windows_msys2.bat`; both enable the UART console with `-serial stdio`, initialize the RTC with the host's local time using `-rtc base=localtime`, and add the AC97 sound card (through DirectSound), the VirtIO network card, the VirtIO tablet and RDRAND.

From WSL with Windows interop enabled, build and launch Windows QEMU directly:

```bash
./02_build.sh && ./03_run_qemu_wsl.sh
```

`03_run_qemu_wsl.sh` uses the same VM settings as the Windows launchers. It looks
for QEMU in MSYS2 UCRT64, then MinGW64, then `C:\Program Files\qemu`, then `PATH`.
It locates `OVMF.fd` and `usb_root/` beside the script and converts their paths
with `wslpath`, so it can be invoked from any directory. To use another Windows
installation, set `QEMU=/mnt/d/path/to/qemu-system-x86_64.exe`. Any script
arguments are passed through to QEMU. Sound goes through Windows (DirectSound);
`MIND_AUDIO`, `MIND_NET`, `MIND_POINTER` and `MIND_CPU` work as with `03_run_qemu.sh`.

### aarch64 (QEMU `virt`, issues 201–204)

The same kernel (with its aarch64 architecture layer), bootloader, services and programs build for aarch64 and run on QEMU's `virt` machine with VirtIO devices: the boot disk on `virtio-blk`, the network card, keyboard and tablet, the screen on `ramfb`, the shell on the PL011 in the terminal, the clock from the PL031. Four CPUs by default (`MIND_CPUS`), started through PSCI; `reboot --off` turns the machine off (issue 203). Needs the Rust targets of `rust-toolchain.toml`, `qemu-system-aarch64` and AAVMF (Debian/Ubuntu: `qemu-system-arm qemu-efi-aarch64 ipxe-qemu`):

```bash
ARCH=aarch64 ./02_build.sh && ./03_run_qemu_aarch64.sh   # aarch64_root/ (scripts/build_aarch64.sh)
python3 tests/aarch64_smoke.py   # after ARCH=aarch64 ./02_build.sh --fixtures
python3 tests/qemu_smoke.py --arch aarch64   # the normal, shell, vfs, net, tls, busy and smp suites on virt (after --fixtures)
```

The platform profile `aarch64/QEMU-virt-0` (what differs from x86-64, its TCB, threat model and evidence) is in [docs/profile/aarch64/](docs/profile/aarch64/README.md); CI runs the `aarch64` jobs next to the x86 ones.

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

* `LIST [-L] [MASK]` — the programs (`*.elf`) on the boot disk, read by `loader`, sorted down columns that fit the screen, and the boot services; `-L` — one line per program with its size and what it does; a mask (`*` any run, `?` one character, case-insensitive, several separated by spaces or commas) keeps the names that match: `LIST A*` the programs and services starting with a, `LIST -L *MON*`. `HELP` lists the shell's own commands, and Tab completes both.
* `RUN <name>` — load `<name>.elf` (or a path such as `extra/demo.elf`) from the disk and start it in the foreground; any ELF built for MIND CORE can be copied to the disk and run. `RUN app` starts the rotating-square application; `BOOT` remains an alias for it.
* `RUN app2` — launch the second application (`app2.elf`): a bouncing square, frame counter, and TSC value on screen. It prints a greeting and a status line every 30 frames to the host's QEMU console via UART.
* `clock` (or `RUN clock`) — display a large digital clock (`clock.elf`) in 24-hour `HH:MM:SS` format, with time changes also printed to the UART console. `RUN clock --text` (in `wm` and `fm`: `clock --text`) draws the time in large digits of text characters (`mind::tui::digits`) with the weekday and the date, sized to the screen or to a `wm` text window as it is resized.
* `RUN files` — list the boot disk and read files through `vfs_server`.
* `beep [hz]` / `beep hz ms [hz ms …]` — tones through `audio_gw`, with no screen (a console program, issue u005): one frequency sounds 500 ms, pairs of frequency and duration play one after the other, 0 Hz is a pause (`beep 440 200 0 100 880 300`); it ends when the sound has played. Without arguments, the gateway demo: a chord of tones and a PCM sweep.
* `say [-p <Hz>] [-r <%>] [text]` — speak the text (or `say.txt`, or a greeting) through `tts`; its screen shows the text whole, Cyrillic too, in rows cut at spaces (issue u010).
* `listen [seconds]` — record from the microphone, show the level, report peak/RMS and play it back; `listen --vad [seconds]` — detect speech in the microphone; `listen [--vad] --wav FILE` — the same for a WAV file.
* `hear [seconds]`, `hear --wav FILE` — offline voice command recognition ([docs/voice](docs/voice/README.md), V1): waits for one utterance from the microphone (default 8 s) or takes every utterance of a WAV file, and prints `HEARD "открой файлы" INTENT=open TOOL=fm CONFIDENCE=0.95` or `NOT UNDERSTOOD`. The commands are `voice/commands.txt` (intents, phrases in Russian and English, slots), spelled with the synthesizer's letter-to-sound rules (the `phonetics` crate both use); the acoustic model `voice/model.bin` (260 KB, int8) is trained by `scripts/voice_train.rs` on our synthesizer's speech only, so it is a starting point for real voices rather than a finished recognizer. `hear` holds the audio and read-only file clients and acts on nothing. The microphone has one owner at a time (`idl/audio.wit` 1.1): while `listen`, `hear` or `voice` records, the others get `THE MICROPHONE IS BUSY`.
* `RUN pong` — IPC demo: starts `ping`, which sends a string through a shared page with `CALL` every 1.5 s; `pong` reads it and replies, and its screen keeps the last string, its sender and the number of calls answered (Esc exits).
* `view <file>` — text and hex viewer: UTF-8 text (Cyrillic), ↑/↓/PgUp/PgDn/Space/Home/End, F2 wrap on/off (←/→ shift long lines), F4 hex/text, F5 go to a line, `0x` offset or `N%`, F7 search ignoring case (Shift+F7 next), F1 keys, Esc/F3/F10 exit. The file is read on demand through a 64 KiB window, so large files open at once. A click on the key bar presses that key, the wheel scrolls (issue u013).
* `edit [file]` — text editor: arrows, Home (to the indentation, then the line start)/End, PgUp/PgDn, Ctrl+Home/End, Ctrl+←/→ by words, Shift with any of them selects, Ctrl+A all, Ctrl+C/X/V, Tab, Ins overwrite, Enter keeps the indentation; Ctrl+U or Alt+Backspace undo, Ctrl+Y redo; F2 save, Shift+F2 save as, F7 find ignoring case (Shift+F7 next), Ctrl+F7 replace all, Alt+F8 go to line, F9 menu, F1 keys, F10 or Esc quit (asks about unsaved changes). UTF-8 with Cyrillic; line endings (LF or CRLF) and invalid bytes are kept byte for byte; up to 8 MiB. Saving writes `name.tmp`, flushes it and renames it over the file (best effort on FAT: no atomic replace). The editor asks for its file (`REQUEST_FILE`): the shell lends a VFS client confined to the file's directory (`ram:` without a file), writable where the user may write — so a file on `ram:` or in `data/` can be saved (Shift+F2 saves under another name in the same directory), others open read-only — the editor says so when it opens them and keeps READ-ONLY in its status line — and nothing outside that directory can be reached; `vfs_server` revokes the client when the editor exits. A missing file is a new one. The mouse: the key bar's buttons, the menu (F9), a click in the text, the wheel (issue u013).
* `console [program [arguments]]` — a terminal for programs (issue u004): a line typed starts a program; what a console program prints shows in console's window or screen (it lends the program an endpoint in `SLOT_CONSOLE`, issue 162), a program with a screen opens a window of its own in `wm`. `list`, `clear`, `exit`; ↑ ↓ history, PgUp/PgDn and the wheel scroll back. In `wm` and in `fm` in a window, console programs run in a `console` window by themselves.
* `RUN keys` — show the key events a program receives: key code, modifiers, character; mouse movement, buttons and wheel (Esc exits).
* `fm [directory]` — file manager (Norton Commander keys): two panels over the boot disk (`A:`) and the RAM disk (`ram:`) (full: name, size, date, time; brief: names in columns; Ctrl+L information, Ctrl+Q quick view of the file under the cursor in the other panel), Tab switches panels, Enter opens a directory, starts a program (in the background: `FG <pid>` shows it) or views a file in the built-in viewer, Backspace goes up, F3 view, F4 edit in the built-in editor (`edit`'s keys; Shift+F4 a new file), F5 copy, F6 move or rename, F7 make a directory, F8 delete — the marked entries or the one under the cursor, whole trees, between volumes too; the copy and move dialogs offer the other panel's directory (`A:/…`, `ram:/…`, or a new name); existing targets are overwritten or skipped on request; a progress window shows the file and the total, Esc stops, and a failed step asks Retry / Skip / Abort (a partly written copy is removed). Ins/+/-/* mark, Ctrl+F3–F6 sort by name/extension/time/size, Ctrl+H hidden files, Ctrl+R reread, Ctrl+U swap panels, Alt+F1/F2 volume, Alt+F7 find by mask from the current directory down, F9 menu, F1 keys, F10 or Esc quit. Typing goes to the command line under the panels: Enter runs `cd <dir>`, `edit <file>`, `view <file>` or a program with arguments (names of the active panel's entries become their paths; with fm in a `wm` window the program opens a window of its own in front, otherwise it runs in the background — bringing it forward from a full screen is kernel issue 160), Esc clears it, Alt+Enter adds the name under the cursor; Ctrl+O hides or shows both panels (their place shows what the command line did), Ctrl+F1 / Ctrl+F2 the left / right one, Ctrl+P the other one, as in Midnight Commander. The mouse (issue u001): a click puts the cursor on an entry, a double click opens it, a right click marks it, the wheel moves the cursor (or scrolls the viewer and the editor), a click on the key bar presses that key; on a screen of its own the cell under the mouse is shown inverted. fm asks for the user's files (`REQUEST_FILES`): the shell lends its own VFS client, so fm can change `ram:` and `data/` on the boot disk; other files open read-only in the editor, which says so (READ-ONLY in its status line; Shift+F2 saves a copy on `ram:` or in `data/`). In fm, `edit` and `view` the key bar shows what F1–F10 do with Shift, Ctrl or Alt while that modifier is held (on the PS/2 keyboard; a terminal sends a modifier only with a key, so its bar stays plain).
* `wm [program ...]` — window manager ([docs/tools §4.9](docs/tools/README.md), issue 088): `wm fm fm clock dzen-clock` shows the programs side by side in windows on one screen (`wm fm data, edit ram:a.txt` with arguments; Alt+R starts more). Text programs draw into a window at the size of its inside (`mind::tui::Terminal::open`), `clock` and `dzen-clock` into a pixel window (`mind::windowed::pixels`), laid out again when its frame changes (issue u009). Alt+Tab switches windows, Alt+arrows take half the screen, Alt+1…4 a quarter, Alt+Enter maximizes, Alt+M moves (arrows) and resizes (Shift+arrows) with snapping at the edges, Alt+W closes a window, Alt+H shows the keys, a right click on the desktop or Alt+P opens the programs by category (issue u003); the items of the top bar can be clicked instead of their keys, for a host that keeps Alt+Tab for itself (issue u008); with the PS/2 mouse a title is dragged (and snaps; a snapped or maximized window dragged off the edge gets its size back, as `[⇕]` gives it) and the `◆` corner resizes, `[▲]` maximizes, and clicks, drags and the wheel inside a window go to its program at the cell of its content (issue u001). Other keys go only to the window in front. Alt+Q leaves wm with the programs running — the next `wm` shows them where they were, as after a crash or `kill` — and Alt+X closes every window. The windows are kept by the window broker `windows` (issue 157); a program started from wm gets a broker client and, of what it asks for, only what wm holds (the user's files, system information), and no screen of its own.
* `find [path] [-name masks] [-type f|d] [-size +N|-N]` — entries of a volume, depth-first in name order (`A:` or nothing for the boot disk, `ram:` for the RAM disk; masks with `*` and `?`, case-insensitive; sizes in bytes or with `k`/`M`).
* `grep [-i] [-n] [-r] [-l] [-c] pattern [path...]` — lines matching a simple regular expression (`.`, `*`, `[a-z]`, `[^...]`, `^`, `$`, `\`); `-i` folds Latin and Cyrillic case; `-r` searches directories; a file with a NUL byte is reported as binary.
* `df` — the volumes: label, FAT type, cluster size, size, used and free space in KiB.
* `format ram: [-l LABEL] -y` — a new empty FAT volume on the RAM disk (`vfs.wit` 2.3 `format`, run by `vfs_server` for the user's client); without `-y` it only says what it would erase; the boot disk is refused.
* `fsck [A:|ram:]` — checks FAT volumes without changing them: `vfs_server` follows every chain from the directory tree and reports lost clusters (and the chains they form), cross-linked clusters, chains that run into a free or bad cluster, files whose size does not match their chain, invalid entries and the dirty flag, with the first problem's path.
* `top` — task monitor: uptime, task states, a busy bar per CPU, load averages, kernel memory, IPC/syscall/interrupt rates; a table with PID, parent, state, CPU, %CPU (from run-time deltas), CPU time, syscalls/s, memory, heap, shared mappings, capabilities and endpoints. P/M/N/T sort by CPU, memory, PID, time; S hides services; t shows the spawn tree; Enter shows a task's details (what it waits for, memory, address space, capabilities, quotas); k stops the selected task and r restarts a service, after a confirmation, through `init` (top asks for lifecycle control, `REQUEST_LIFECYCLE`); +/- refresh interval; q or Esc quits.
* `svc [list | start <service> | stop <service or PID> | restart <service>]` — boot services from `init`: PID, how often `init` started each, running or stopped, what `init` gave it; start, stop and restart a service (clients granted earlier reach the new instance), stop an application by PID. `init` and the shell cannot be stopped.
* `memmap` — memory map in four views (Tab or 1–4): the physical address space as a coloured bar with the firmware ranges and the platform layout (m merges or shows raw ranges, z zooms to RAM); the kernel arena by use with its limits and fragmentation; the address space of a chosen task with guard pages, heap and shared mappings; task and endpoint quotas as a tree by spawner.
* `load` — graphs over 30 s (1) or 10 min (2): busy time per CPU (c: all CPUs in one), interrupts, syscalls, IPC messages and context switches per second, kernel arena and tasks; load averages.
* `ipc` — IPC: a table of endpoints with their server, holders, waiting senders out of the queue bound, receivers, messages, `ERR_BUSY` refusals, timeouts and bound IRQ line (i/m/w sort by index, messages, waiting); Enter lists the tasks holding the selected endpoint with slot, rights and badge (sysinfo `holders`); view 2 shows who waits for whom (senders and callers to the server, callers to the task they wait a reply from) and marks cycles as deadlocks.
* `caps [pid]` (or `caps` in the shell) — capabilities: view 1 lists a task's slots (kind, rights, badge, the endpoint or size it names, and where it was derived from), ←/→ go to the next task; view 2 shows the derivation tree across tasks (init's originals with the services' copies below them); Enter lists what a revoke of the selected capability would remove. Who holds what is the authority graph: `caps` asks for `REQUEST_AUTHORITY`, and the shell lends it its `sysmon` client with the authority badge (slot 13) in place of the plain one; a program without it is refused (`denied`) and sees no derivation links. `caps <pid>` in the shell prints one task's slots as text.
* `hw` — hardware: processor (CPUID: vendor, model, NX, invariant TSC, APIC, SSE/AVX), TSC rate and clock resolution, the framebuffer, PCI devices with class, BARs, IRQ and the service holding each, interrupt lines with counts and holders, platform memory.
* `dzen-clock` (or `RUN dzen-clock`) — five color indicators for time (`dzen-clock.elf`); **D** toggles the thin digital time, **C** selects a simple 100-second orbit, **P** selects an orbit with 10-second ticks, **H** hides/shows the title and key hints. `RUN dzen-clock --text` shows the same face in colored text cells (round discs, the orbit and its dot in characters) with the same keys, on a screen or in a `wm` text window. On its own screen **T** switches between the pixel face and the text face (issue u007).
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
* `keymap [us|ru] [--switch both|ctrl-shift|alt-shift|caps|none]` — the PS/2 keyboard's layout and what switches it (Ctrl+Shift or Alt+Shift, one of them, Caps Lock — which then locks no capitals — or nothing), through the shell's `ps2_kbd` client; without arguments shows them.
* `screenshot [file]` — the screen in front (the shell's, when typed there) as a 24-bit BMP, from the compositor's sealed copy; without a file under the first free `ram:screen-NNN.bmp`.
* `record [-w] [-r fps] [-t seconds] [file]` — the screen in front as an AVI file of Motion JPEG frames (issue 093): started in the background (`run record -t 10 data/demo.avi &`), it records the program then brought to the front; 10 frames a second and 10 s unless told, `ram:record-NNN.avi` without a file. An unchanged screen repeats the last frame (an empty chunk); a changed one codes again only its rows of 16×16 blocks that changed (`mind::jpeg`: baseline JPEG, 4:2:0, a restart marker after each row; `mind::avi`). It asks for `REQUEST_DISPLAY`: the shell lends it its compositor client. While the screen is being captured, by `record` or `screenshot`, the compositor shows a red dot in its top right corner (issue 165), on the display only: no capture holds it and no program can hide it. In `wm`, `record -w` from the run line (issue u014) records the window in front alone, at its size: `wm` lends it a read-only lease of that window's surface, not the screen, and shows " ● REC " on the window's frame meanwhile.
* `voice on [--wav FILE] [SECONDS]`, `voice off`, `voice listen`, `voice` — voice control ([docs/voice](docs/voice/README.md), V2): starts the `voice` program in the background, lending it a line back to the shell (`idl/voice.wit`). F12 (or `voice listen`) while the shell has the focus is push-to-talk: `voice` listens for one utterance — up to SECONDS (default 6) from the microphone, or the next utterance of a WAV file standing in for it — and the shell decides what it does and answers in its language through `tts`: open a tool (`открой файлы` runs `fm`) or close it, the time, date and free memory in words, read `docs/notes.txt` aloud, stop or restart `rtc`, `netstack` or `sysmon` and reboot — these three only after «да» (or Enter; «нет» or Esc cancels). It shows as `VOICE: "который час" (973) -> Сейчас восемнадцать часов четыре минуты`; a phrase it does not understand is answered «Не понял» and runs nothing. `voice` keeps the line to the shell and the audio, `tts` and read-only file clients, nothing else.
* `msh <file> [args]`, `msh -c "code"`, `msh --check <file>` — the shell's script language ([docs/msh.md](docs/msh.md), issue 094): values, records and lists, `let`, `if`, `for`, `while`, `fn`, and results as in Marain (`ok`/`err`, `cmd?`, `cmd or { … }`, `try`/`catch`): nothing continues silently after an error. Commands are written as at the prompt; `capture("ps")` takes a command's output, `ps()`, `files(dir)`, `services()` give typed answers. A script gets no more authority than its `requires:` line declares (`files`, `network`, `log`, `lifecycle`, …), for its commands and the programs it starts; one from outside the boot disk asks first. Statements typed at the prompt keep their variables; Esc, Ctrl+C or Ctrl+Z (serial) stop a script, and so does a step budget.
* `reboot [-f]` — writes cached files to the disks, stops the boot services in reverse start order through `init` (drivers quiesce their devices), then resets the machine (`REBOOT`); `-f` skips stopping the services.
* `TIME` — one line: the time of day from the RTC, the uptime, the monotonic clock (ns), its resolution and the calibrated TSC frequency (issue u007; it was `CLOCK`, which now starts the clock program).
* `DATE` — show the calendar date and time from the RTC (no time zone).
* `FAULTS` — show the last 16 application exceptions: PID, CPU, exception vector/error code, instruction and fault addresses.
* `HEAP` — allocate and format a test string, release it, and report total runtime heap usage, free bytes, and whether the test allocation was freed. The measurement is serialized with program allocations so concurrent heap activity cannot produce a false leak report.
* `HELP` — list the shell's commands; `HELP <name>` — what a command or program does: the shell's lines that name it and the program's own text, read from its file without starting it (`help fm`, `help voice`, `help rtc`). Every application also answers `<name> --help`: a console program prints its text into the shell; for one with a screen the shell shows the same text from the file instead of starting it.

`RUN` requires a program name; without one it displays usage and the program list. Program names are case-insensitive, and surrounding whitespace is ignored. Unknown names leave you in the shell with an error message.

**Keyboard.** Programs receive key events, not scan codes: a 32-bit word with the character, a key code and the Shift/Ctrl/Alt modifiers (`common/abi.rs`, `KEY_*`). `ps2_kbd` decodes the PS/2 keyboard (arrows, Home/End, PgUp/PgDn, Ins/Del, F1–F12, keypad with Num Lock, Caps Lock) and has a US and a Russian (ЙЦУКЕН) layout: **Ctrl+Shift** or **Alt+Shift**, pressed and released without another key, switches it (`keymap` selects the layout and the switch key); Ctrl/Alt shortcuts stay positional (Ctrl+C is the same key in both layouts). The shell decodes the UART as a VT100/xterm terminal: escape sequences for the same keys with modifiers, UTF-8 text (a host terminal types Cyrillic directly), CR, LF and CRLF as one Enter, a lone Esc after 50 ms. `RUN keys` shows every event it receives. A modifier going down or up is an event of its own on the PS/2 keyboard (`KEY_SHIFT`, `KEY_CTRL`, `KEY_ALT`, without a legacy byte): `mind::input::modifiers()` tells which are held, and the key bars of `fm`, `edit` and `view` follow it. Ctrl+Z is reserved for the system.

**Ctrl+Alt+F1…F4** show the shell's four consoles (issue 155), whatever program has the keyboard. Each console has its own text, history and foreground program. The serial line is console 1's. Press **Ctrl+Z** in the QEMU window or send UART byte `0x1A` to return to `MIND>` while the foreground program continues in the background. Press **Esc** to end the foreground program and return to the shell. `FG` restores the existing screen; it does not restart the program. Each new `RUN` starts fresh application state. Rebuild all components together when changing `common/abi.rs`.

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

**Current scope:** x86-64 UEFI/QEMU with xAPIC and NX, tested with one and four CPUs. Runtime RAM and the GOP framebuffer must fit below 4 GiB; kernel structures use a reserved 64 MiB arena and task memory the free RAM below 4 GiB (RAM above 4 GiB is not used yet). There are at most eight application tasks and eight CPUs. CPU assignment is fixed for each task; there is no migration, work stealing, demand paging, or userspace allocator for sub-page objects yet. Kernel mappings are supervisor-only identity mappings (kernel text is not separately write-protected). The supported compiler target remains `x86_64-unknown-none`; context switching saves x87, SSE and, on CPUs with XSAVE and AVX, AVX state per task (XSAVE; FXSAVE otherwise; `cpus` shows `FPU=XSAVE+AVX` or `FPU=FXSAVE`).

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

Each process has a separate heap arena starting above `0x8006000000`. Every block is writable and non-executable, followed by an unmapped guard page. Limits are 64 live blocks (heap blocks and mappings together) and the task's memory quota (16 MiB by default) of rounded data, which also bounds what its descendants hold; availability also depends on the free frames. Physical backing is currently contiguous, so fragmentation can cause an allocation to fail. Failed mappings roll back all partially allocated resources. Guard pages catch accesses into those pages; they do not detect overruns that stay within a mapped page. An address can be reused after free, so this API does not provide temporal memory safety after reuse.

`mind::mem::Pages::new(bytes) -> Option<Pages>` wraps these calls with `as_slice()`/`as_mut_slice()`, `share()` and automatic free in `Drop`:

```rust
if let Some(mut buffer) = mind::mem::Pages::new(8192) {
    buffer.as_mut_slice()[0] = 42;
} // freed here; exit/kill/fault also reclaims any remaining blocks
```

`MEM_SHARE` accepts only the start of one of the caller's heap blocks (never code, stack or a sub-range beyond the block) and returns a capability slot; `MEM_MAP` maps a received capability into the shared quota (read-only without `CAP_WRITE`) and reports its size.

### System calls

`int 0x80` with the mailbox at `USER_MAILBOX` (`syscall_num`, `arg1`, `arg2`, `msg[4]` in; `result` out). Errors are `usize::MAX - n` (`ERR_INVALID`, `ERR_NO_SLOT`, `ERR_RIGHTS`, `ERR_NOT_FOUND`, `ERR_PEER`, `ERR_NO_MEMORY`, `ERR_BUSY`, `ERR_LIMIT`, `ERR_TIMEOUT`); `ALLOC` returns 0 on failure.

| # | Name | Arguments → result |
|---|---|---|
| 1 | RDTSC | → time stamp counter |
| 2 | READ_KEY | → legacy byte of the next input event of the calling (focused) task or 0 |
| 3 | LOG | buffer, length → bytes logged |
| 5 | WAIT | milliseconds (10 ms steps, ≤ 60 s) → uptime at sleep |
| 6 | UPTIME | → milliseconds since boot |
| 7 | EXIT | — |
| 8 / 9 | ALLOC / FREE | bytes → address / address → 0 |
| 10 / 22 | IPC_SEND / IPC_CALL | endpoint handle \| timeout ms << 32, reply-capability slot; msg = [cap slot, rights mask, data, data] |
| 11 | IPC_RECV | endpoint handle \| timeout ms << 32, slot for a received capability → arg1 = sender PID, arg2 = badge of the sender's capability, msg = [cap received, flags, data, data] |
| 23 | IPC_REPLY | arg1 = saved reply slot or 0 for the last caller; msg = [cap slot, rights mask, data, data] |
| 31 | IPC_SAVE_REPLY | → slot of a one-time reply capability for the last caller |
| 12 | ENDPOINT_CREATE | → slot of a new endpoint with all rights; `ERR_LIMIT` past the endpoint quota |
| 13 | SPAWN | `name\0arguments`, length; msg = [image memory slot or `SPAWN_BOOT` \| boot index, ELF length, grant array, count \| flags << 8 \| task quota << 16 \| endpoint quota << 32] → PID — spawn privilege; boot images and services need the platform privilege |
| 32 | PLATFORM_CAP | kind, argument; msg[0] = second argument → new slot — platform privilege (`init`) |
| 33 | DEVICE_FIND | PCI class, mask; msg[0] = n-th match, msg[1] = vendor \| device << 16 (0: any) → device index — platform privilege |
| 34–43 | TASK_LIST, TASK_KILL, FOCUS, TASK_LOGS, CONSOLE_READ, NOTICE, FAULTS, CPU_INFO, KERNEL_HEAP, HALT | process control for the shell (see `common/abi.rs`) — control privilege |
| 14 | CAP_DROP | slot |
| 45 | CAP_MINT | slot, rights mask; msg[0] = offset, msg[1] = length (0: to the end), msg[2] = endpoint badge (0: keep) → slot of a narrower child |
| 46 | CAP_REVOKE | slot → number of descendants removed; their mappings are gone everywhere when it returns |
| 49 | DEVICE_STATE | device index, `DEVICE_STOP`/`DEVICE_START` → 0 — platform privilege or a capability over one of the device's BARs |
| 48 | TASK_WATCH | PID of a task the caller spawned, endpoint with the read right → 0; the task's exit arrives there as a message with `MSG_FLAG_EXIT` (PID, reason) |
| 47 | MEM_DETACH | heap block address → slot of a move-only memory object; `ERR_BUSY` if the block is shared |
| 15 / 16 | MEM_SHARE / MEM_MAP | block address, bytes → slot / slot → address (arg2 = size) |
| 17 / 18 | PORT_IN / PORT_OUT | port-range slot, port; msg[1] = width 1/2/4, msg[0] = value |
| 27 | PORT_IN_BLOCK | port-range slot, port; msg[2] = buffer, msg[3] = 16-bit words |
| 53 | PORT_OUT_BLOCK | as PORT_IN_BLOCK, the words are written from the buffer to the port |
| 19 / 24 / 25 | IRQ_WAIT / IRQ_BIND / IRQ_ACK | IRQ slot (and endpoint slot for BIND) |
| 20 | INPUT_EVENT | app byte, focus-owner byte, msg[0] = attention (Ctrl+Z), msg[1]/msg[2] = full event words (0: byte only) — needs the input capability |
| 52 | SCHED_SET | PID, budget µs; msg[0] = period µs, msg[1] = band or `BAND_KEEP` → 0 — lifecycle owner or process control (band changes: control/platform/restart) |
| 51 | STAT | class, buffer; msg[0] = capacity, msg[1] = PID for VMAP/CAPS → records written (header: version, record size, count, total) — observe or control privilege; classes TASKS, CPUS, MEMORY, PHYSMAP, VMAP, CAPS, ENDPOINTS, IRQS, DEVICES (`common/abi.rs`) |
| 50 | READ_INPUT | → next input event word of the focused task (key, modifiers, pressed, character, legacy byte; see `common/abi.rs`), 0 if none |
| 21 | COMPOSITOR_PULL | slot → 0 unchanged, 1 dirty, 2 new screen in slot — needs the display capability |
| 26 | MEM_PHYS | DMA slot → physical address |
| 16 | MEM_MAP (MMIO) | device-register slot → address, mapped uncached |
| 28 | TASK_ALIVE | PID → 1/0 |
| 44 | CLOCK | → monotonic ns since boot, arg2 = resolution ns, msg[2] = TSC Hz (0: 10 ms tick) |
| 48 | STAT | class (`STAT_*`), argument (PID); msg[0] = buffer, msg[1] = capacity → records after a `StatHeader`: tasks, CPUs, kernel memory, physical map, address space, capabilities, endpoints, IRQs, devices — observe or control privilege |
| 29 | CAP_INFO | slot → kind, arg2 = port base, memory rights or endpoint badge, msg[2] = size/count/endpoint rights, msg[3] = 1 if the memory is sealed |

This is a page-block API. `mind::heap` (feature `alloc`) subdivides it for programs: objects up to 2 KiB come from power-of-two size classes carved out of single pages, objects up to 256 KiB are runs of pages in 1 MiB arenas (at most 12), larger ones get their own block; allocation failure ends in the panic handler, which logs and exits the task. Services keep static memory. `tests/heap_host.rs` checks alignment, disjointness, reuse and exhaustion on the host. `app2` already uses a block for its 64×64 sprite and handles allocation failure by reporting it and returning. Page-table edits are serialized with the scheduler; a process runs on only one pinned CPU, so local invalidation is sufficient. Kernel allocation locks disable local interrupts to avoid allocator/scheduler lock inversion. CR3 invalidation follows the [Intel system programming manual](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html).

### Interfaces (MIND IDL)

Service interfaces are described in `idl/*.wit`, a WIT subset with a version, size limits and the capability a call may carry ([docs/idl](docs/idl/README.md)). `scripts/mind_idl.py` generates client calls and a server-side `decode` that checks every request (method, version, unused bits, capability kind) into `libmind/src/idl/`. MIND IDL v0.2 adds records, strings and lists with declared bounds, carried in the client's buffer: the server copies the request into private memory before checking it, the client revokes the server's access before reading the reply. Its minor extension adds enums, `bytes<N>` and capability results. All services that answer requests (`rtc`, `loader`, `init`, `tts`, `audio_gw`, `vfs_server`, `sysmon`, `logd`, `ramdisk` and the block drivers) use MIND IDL.

A program states what it needs with `mind::request!(REQUEST_CONSOLE | REQUEST_SYSINFO)`: a `.mind_request` section in its ELF that grants nothing (MC-3.11). The shell, as the user's agent, reads it with the loader's `inspect`, opens a launch session (`begin`), lends what it holds and is willing to give (`grant`: the `sysmon` client in slot 10 for `REQUEST_SYSINFO`, in slot 7 a VFS client confined to the directory of the file named in the arguments for `REQUEST_FILE` (`vfs.wit` `scope`) or its own VFS client — writes on `ram:` and in `data/` — for `REQUEST_FILES`, its read-badged `logd` client in slot 12 for `REQUEST_LOG`, its client of `init` in slot 11 for `REQUEST_LIFECYCLE`, its `sysmon` client with the authority badge in slot 10 instead of the plain one for `REQUEST_AUTHORITY`, a client of the window broker in slot 8 for `REQUEST_WINDOW` and its manager client there for `REQUEST_WINDOW_MANAGER`, its compositor client in slot 15 for `REQUEST_DISPLAY`) and starts the program (`commit`); `REQUEST_CONSOLE` starts it without a screen, and so does a session that lends slot 8 to a program that is not a window manager: the program shows itself in a window (`wm` starts programs that way, issue 088). Nothing is granted by program name. A program says what it does with `mind::about!("name — what it does\nUsage: …")` as the first statement of `main`: the text goes into a `.mind_about` section (the program's linker script keeps it), the program prints it and exits when its argument is `--help`, and the shell's `help <name>` and `list -l` read it from the file.

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
rustc --edition=2021 --test tests/wm_host.rs -o /tmp/mind-core-wm-tests && /tmp/mind-core-wm-tests   # the window manager's desktop: placement, focus, snapping, keys, the mouse, what each cell shows
rustc --edition=2021 --test tests/virtio_input_host.rs -o /tmp/mind-core-virtio-input-tests && /tmp/mind-core-virtio-input-tests   # the VirtIO tablet's and mouse's reports as pointer events
rustc --edition=2021 --test tests/beep_host.rs -o /tmp/mind-core-beep-tests && /tmp/mind-core-beep-tests   # beep's notes and their samples
rustc --edition=2021 --test tests/console_host.rs -o /tmp/mind-core-console-tests && /tmp/mind-core-console-tests   # console's text and command line; a program's output messages
rustc --edition=2021 --test tests/say_host.rs -o /tmp/mind-core-say-tests && /tmp/mind-core-say-tests   # say's text cut into rows for its screen
rustc --edition=2021 --test tests/clock_host.rs -o /tmp/mind-core-clock-tests && /tmp/mind-core-clock-tests   # the text faces of clock and dzen-clock
rustc --edition=2021 --test tests/block_host.rs -o /tmp/mind-core-block-tests && /tmp/mind-core-block-tests   # block protocol: the write badge
rustc --edition=2021 --test tests/fat_host.rs -o /tmp/mind-core-fat-tests && /tmp/mind-core-fat-tests   # FAT writer and checker vs mkfs.fat, fsck.fat, mtools
rustc --edition=2021 --test tests/edit_host.rs -o /tmp/mind-core-edit-tests && /tmp/mind-core-edit-tests   # the editor: piece table, undo, search/replace, keys, drawing
rustc --edition=2021 --test tests/logd_host.rs -o /tmp/mind-core-logd-tests && /tmp/mind-core-logd-tests   # logd's ring: order, wrap-around, clipping, rate limit
rustc --edition=2021 --test tests/search_host.rs -o /tmp/mind-core-search-tests && /tmp/mind-core-search-tests   # find, grep and mind::pattern
python3 tests/idl_test.py   # MIND IDL generator; fails if libmind/src/idl is stale (regenerate: python3 scripts/mind_idl.py)
python3 tests/font_test.py  # font subset coverage, licence notice; fails if common/font16.rs is stale (python3 scripts/font_gen.py)
```

The QEMU integration test boots an isolated copy of `usb_root` (the `boot` suite first checks that the bootloader names a corrupt or missing boot file and, with `--panic-kernel`, that a kernel panic reports message, location, CPU and task; the `display` suite checks colours on VGA std, virtio-vga and ramfb), exercises concurrent instances, `fg`, `kill`, UART/PS2 input, task limits, repeated allocation/freeing, and idle `HLT`. Additional suites check concurrent CPU progress, remote termination, independent SIMD contexts, private heap stress/OOM recovery, deliberate ring-3 faults and capability checks without stopping other programs, the boot services (IPC call/reply with memory capabilities, VFS over the ATA driver, the system log: `dmesg` shows the boot lines of `init` and the services with the PIDs `logd` stamped, a line claiming another source keeps its real one, filters and follow mode; `svc` restarts `rtc` and the shell's `date` reaches the new instance, stops and starts a service, refuses `init` and the shell; `top` stops an application), the AHCI driver (`ahci` suite: the disk attached to an AHCI controller), text to speech (`tts` suite: duration and voiced pitch of the captured speech; with `--asr-model <Vosk Russian model directory>` also checks that the words are recognized), the audio gateway (AC97 output captured to a WAV file and checked for the expected tones) files on a raw FAT disk (`vfs` suite, with `mkfs.fat` and `mtools`: the shell writes in `data/`, the host checks the image with `fsck.fat -n` and `mtype`, a reboot reads the files back and finds `ram:` empty), the editor (`edit` suite, with `mkfs.fat` and `mtools`: Latin and Cyrillic text typed and saved on `ram:` and in `data/` of a raw FAT disk, read back, a CRLF file, a boot file read-only; the host runs `fsck.fat -n` and compares the files byte for byte), `fm`'s write operations, `df` and `fsck` (`disk` suite: a tree copied from `data/` to `ram:`, renamed, copied back, deleted, a new directory, an edit in place, `df` before and after; `fsck` finds a chain the harness broke on the boot disk and passes `ram:`; with the chain restored the host's `fsck.fat -n` is clean and mtools reads what fm wrote) and block writes (`block` suite, with `mkfs.fat` and `mtools`: a raw FAT image boots three times — IDE, AHCI, USB — with `tests/block_app.rs` standing in for `vfs_server`; it writes, flushes and reads back sectors through the write-badged clients and the harness checks the image and runs `fsck.fat -n`):

```bash
for fixture in busy_app isolation_app heap_app block_app; do
  rustc --edition=2021 --target x86_64-unknown-none --crate-type bin \
    -C opt-level=3 -C panic=abort -C relocation-model=pic \
    -Z relax-elf-relocations=yes -C link-arg=-Tapp/linker.ld \
    "tests/$fixture.rs" -o "/tmp/mind-core-$fixture.elf"
done
# Test-only kernel that panics on the first LOG call (boot suite checks the panic report):
(cd kernel && cargo build --release --features panic-test --target-dir /tmp/mind-panic-target)
python3 tests/qemu_smoke.py --qemu qemu-system-x86_64 \
  --firmware OVMF.fd --busy-elf /tmp/mind-core-busy_app.elf \
  --isolation-elf /tmp/mind-core-isolation_app.elf --heap-elf /tmp/mind-core-heap_app.elf \
  --block-elf /tmp/mind-core-block_app.elf \
  --panic-kernel /tmp/mind-panic-target/x86_64-unknown-none/release/kernel
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

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md): where to start, how to build and test, and the project's rules. Security issues: [SECURITY.md](SECURITY.md). Conduct: [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).

## License

Licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option. Data files from other projects keep their own licences, listed in [THIRD_PARTY.md](THIRD_PARTY.md): the Russian stress dictionary `phonetics/data/stress_ru.txt` is CC BY-SA 4.0 (OpenRussian), the English lexicon `phonetics/data/lexicon_en.txt` is under the CMUdict BSD-style licence, the voice model `voice/model.bin`, trained on speech made with them, is shared under CC BY-SA 4.0 to be safe, and the 8×16 font MIND Mono (`fonts/mind-mono-16.bdf`, a subset of Terminus Font 4.49.1) is under the SIL Open Font License 1.1.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
