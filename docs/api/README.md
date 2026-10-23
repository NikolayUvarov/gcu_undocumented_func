# MIND Core API

**Version:** ABI 19 (`ABI_VERSION` in `common/abi.rs`; 262-KRN-0161) · **Stability:** unstable, see [Stability](#stability)

Programs reach MIND Core through three layers:

| Layer | What it is | Reference |
|---|---|---|
| System call ABI | `int 0x80` with a per-task mailbox; the only interface of the kernel | this page; constants and records in [`common/abi.rs`](../../common/abi.rs) |
| `libmind` (crate `mind`) | Rust library for ring-3 programs: system calls, IPC, memory, devices, generated service clients | [libmind modules](#libmind) |
| Service interfaces | Typed, versioned requests to services (`vfs`, `audio`, `tts`, ...) described in MIND IDL | [docs/idl](../idl/README.md), files in [`idl/`](../../idl) |

An application normally uses only `libmind` and the service clients; the raw ABI matters for other languages and for the kernel's tests.

## Calling convention

- Every task has a **mailbox** page (`SyscallMailbox` in `common/abi.rs`): `syscall_num`, `arg1`, `arg2`, `result`, `msg[4]`. The kernel enters the program at `_start(info, mailbox)` (System V calling convention: `rdi` = the read-only **info page** with `BootInfo` and the program arguments at `ARGS_OFFSET`, `rsi` = the mailbox; the info page is the page just below the mailbox).
- A call: write `syscall_num`, `arg1`, `arg2`, `msg`, execute `int 0x80`, read `result` (and, for some calls, `arg1`, `arg2`, `msg`).
- **Errors** are `result` values `usize::MAX - n` (`ERR_INVALID`, `ERR_NO_SLOT`, `ERR_RIGHTS`, `ERR_NOT_FOUND`, `ERR_PEER`, `ERR_NO_MEMORY`, `ERR_BUSY`, `ERR_LIMIT`, `ERR_TIMEOUT`; everything from `ERR_FIRST` up is an error). `ALLOC` returns 0 on failure.
- **Capabilities** are named by 64-bit handles `slot | generation << 24` (`HANDLE_SLOT_MASK` 0xFF_FFFF, `HANDLE_GENERATION_SHIFT` 24: a 32-bit generation; issue 172, ABI version 2; version 3 since senders wait in order, 000-KRN-0010; version 4 since `BootInfo` names the boot volume and the slot and `BOOT_CONFIRM` exists, 211-KRN-0012 and 351-KRN-0014; version 5 since `SLOT_SHELL` and `SLOT_CLIPBOARD` moved `SLOT_DYNAMIC` to 32, 211-KRN-0058; version 6 since setting a badge needs `CAP_BADGE`, 000-KRN-0073; version 7 since `BootInfo.photo_pause_s`, 211-KRN-0089; version 8 since `SLOT_BATTERY` moved `SLOT_DYNAMIC` to 33, 211-KRN-0090; version 9 since a task may have a text input, `SPAWN_TEXT`, `TEXT_WRITE` and `TEXT_READ`, 000-KRN-0094; version 10 since `SLOT_WIFI` moved `SLOT_DYNAMIC` to 34, `SLOT_NET` became a launch slot, and the Wi-Fi station `wlan` and its stand-in radio `wlansim` became boot images, with `tpm` and `keystore` started before them, 550-KRN-0097; version 11 since the Realtek boot images, the non-mastering BAR grant and DEVICE_REGISTERS, 213-KRN-0114; version 12 since the SSH service `ssh` is a boot image after `tls`, and `SLOT_SSH` and `SLOT_UPDATE` moved `SLOT_DYNAMIC` to 36, 351-KRN-0115 and 351-KRN-0116; version 13 since `usb_serial` and `link` are boot images after `vfs_server` and `BootInfo` carries `link.conf`'s words after the images, 800-KRN-0128; version 14 since `SLOT_MODE` (36), the shell's client of `init` badged `BADGE_MODE` for `init.wit` 1.5 `set-mode`, moved `SLOT_DYNAMIC` to 37, 750-KRN-0143; version 15 since `SLOT_COMMANDS` (37), the shell's command endpoint made by `init` and kept across its restarts, moved `SLOT_DYNAMIC` to 38, 800-KRN-0148, with `KEY_PRINT_SCREEN`, 000-KRN-0149, and an absolute pointer event's horizontal steps, 211-KRN-0150; version 16 since the Effector agent's `effector_gw` and `effector` are boot images after `updater`, started by `init` only in the development or the maintenance mode, 750-KRN-0119; version 17 since `CAP_REVOKE` leaves a task that mapped a revoked memory lease a zeroed page of its own at each of its pages instead of unmapping them, 000-KRN-0152; version 18 since `WORKER_START`, `SPAWN_WORKERS` and `WORKER_OFFSET`, 262-KRN-0163). Every task's `BootInfo` carries `abi_version` (`ABI_VERSION`); `libmind` stops a program built for another version (`ABI MISMATCH`, exit code 126), and the kernel stops when the bootloader's differs from its own. ABI 11 preserves the ABI 10 prefix, including its 36-image array and the version offset; additional images follow the original fields. Update the bootloader, kernel and services together. This transition does not establish compatibility with older layouts. `BootInfo.boot_volume` is the partition the bootloader was loaded from (MBR disk signature or GPT partition GUID, number, start, size); `boot_slot` is the slot (`BOOT_SLOT_ROOT`, `_A`, `_B`), whether it runs on trial, its deadline and the SHA-256 of the manifest it verified; `launch` is the rest of the launch record (the signing key's identity, whether it is the public test key, the images checked), which init also logs (350-KRN-0023); `efi_runtime` is the address of the firmware's `EFI_RUNTIME_SERVICES` table (351-KRN-0027), which the kernel keeps for `FIRMWARE_VARIABLE` and gives no task; `device_tree` is the flattened device tree the firmware hands over where there is no ACPI (210-KRN-0029), for the kernel's board code, and no task gets it either. `photo_pause_s` is the pause for a photo of the screen the bootloader read from `data/pause.txt` on its volume (at most `PHOTO_PAUSE_MAX_S`, 60; 0 without the file), which init waits before the compositor takes the screen (211-KRN-0089). `link_present` and `link_words` say whether the root of that volume holds `link.conf` and give its words (at most `LINK_WORDS_MAX`, 64 bytes, joined by single spaces), read by the bootloader through the firmware so that the link to another machine starts even when MIND Core cannot read its disk; `init` starts `usb_serial` and `link` only with the file (800-KRN-0128). `BOOT_CONFIRM` (60, platform privilege) ends a trial: the kernel no longer restarts the machine at the deadline. A task's table starts with 104 slots (`CAP_SLOTS`, at least 64 of them dynamic) and grows on demand up to 4096 (`CAP_SLOTS_MAX`, a bound on kernel heap). Slots 1–37 are fixed by convention (generation 0; 7 and 10–12 carry the capabilities a launcher grants on request, 13–25 are further clients, see below; 23, `SLOT_GPIO`, is the pin controller service's client, issue 207; 24, `SLOT_CAMERA`, the video gateway's, issue 158; 25, `SLOT_BLOCKSTORE`, the block store's, lent for `REQUEST_BLOCKSTORE`, 300-KRN-0001, or for `REQUEST_BLOCKSTORE_READ` the shell's client from 26, `SLOT_BLOCKSTORE_READ`, badged get alone, 300-KRN-0024; 27, `SLOT_FIRMWARE`, the firmware variable privilege `CAP_KIND_FIRMWARE`, the shell's, lent in the same slot for `REQUEST_FIRMWARE` once the user allows it, 351-KRN-0027; 28, `SLOT_PARSE`, the parser service's client, lent in the same slot for `REQUEST_PARSE`, 109-KRN-0042; 29, `SLOT_TPM`, the shell's client of the TPM service, without the seal badge, 351-KRN-0043; 30, `SLOT_SHELL`, and 31, `SLOT_CLIPBOARD`, 211-KRN-0058; 32, `SLOT_BATTERY`, the battery service's client, lent in the same slot for `REQUEST_BATTERY`, 211-KRN-0090; 33, `SLOT_WIFI`, the Wi-Fi station's configuration client with `BADGE_WIFI_CHANGE`, the shell's, lent in the same slot for `REQUEST_WIFI`, 550-KRN-0097; 34, `SLOT_SSH`, the SSH service's client, the shell's with the right to badge, lent in the same slot for `REQUEST_SSH` with a flow grant, 351-KRN-0115; 35, `SLOT_UPDATE`, the updater's client, the shell's alone, for its `update` command, 351-KRN-0116; 36, `SLOT_MODE`, `init`'s client badged `BADGE_MODE`, the shell's alone, 750-KRN-0143; 37, `SLOT_COMMANDS`, the shell's own command endpoint, which `init` makes and keeps across its restarts and of which it mints `link`'s client badged `BADGE_SHELL_LINK` in developer mode, 800-KRN-0148); the kernel hands out slots from 38 (`SLOT_DYNAMIC`). A stale handle is rejected. There are no global names: a task can use only what it was granted (Constitution MC-3.3).
- The framebuffer is described in `BootInfo`: `stride` pixels per line, 4 bytes per pixel, `pixel_format` (`PIXEL_RGB`, `PIXEL_BGR`, `PIXEL_BITMASK` with `pixel_masks`). A task's screen always holds `0x00RRGGBB`; only the compositor writes the framebuffer and converts (`pixel_to_device`). On x86 the compositor's mapping of it (`MEM_MAP` of `PLATFORM_FRAMEBUFFER`) is write-combining (PAT entry 1, 000-KRN-0081).
- Application slots, filled by the loader: `SLOT_INIT` 1, `SLOT_RTC` 2, `SLOT_VFS` 3, `SLOT_AUDIO` 4, `SLOT_LOADER` 5, `SLOT_TTS` 6. A launcher fills more through a launch session (`idl/loader.wit` 1.1; `grant-memory` 1.3 takes a read-only memory capability, in `SLOT_DISPLAY` only; `grant-firmware` 1.6 the firmware variable privilege, in `SLOT_FIRMWARE` only) when the program asks for them with `mind::request!` and the launcher agrees: `SLOT_FILE` 7 (a VFS client for `REQUEST_FILE` / `REQUEST_FILES`), `SLOT_SYSINFO` 10 (`sysmon`), `SLOT_LIFECYCLE` 11 (`init`'s lifecycle requests), `SLOT_LOG` 12 (reading the system log); `SLOT_INIT` 1 may carry an endpoint for a ping/pong pair. Slots 13–15 are the shell's further clients for such grants: `SLOT_AUTHORITY` 13 (`sysmon` with `mind::stat::BADGE_AUTHORITY`, lent in `SLOT_SYSINFO` to a program that asks for `REQUEST_AUTHORITY`), `SLOT_KEYBOARD` 14 (`ps2_kbd`), `SLOT_DISPLAY` 15 (`compositor`, lent in the same slot to a program that asks for `REQUEST_DISPLAY`, issue 165; from `wm` that slot holds instead a read-only memory lease of the window in front, issue u014); 16–17 are the shell's network clients (`SLOT_NET`, `SLOT_SOCKET` with the operator's badge), `SLOT_NETWORK` 18 holds an application's flow grant (`REQUEST_NETWORK`; in the shell its broker client is `SLOT_NETPOLICY` 19), `SLOT_TLS` 20 is the shell's client of the TLS service (lent in the same slot for `REQUEST_TLS`, 351-KRN-0034), 21–22 its clients of the window broker (`SLOT_WINDOWS`, `SLOT_WINDOW_MANAGER` with `mind::window::BADGE_MANAGER`), lent in an application's `SLOT_WINDOW` 8 for `REQUEST_WINDOW` and `REQUEST_WINDOW_MANAGER`; `SLOT_CONSOLE` 9 may hold an endpoint of the program's launcher, and `mind::process::log` (`println!`) sends what the program prints there too, 15 bytes a message (`mind::output`, issue 162; `console` lends it); `SLOT_SHELL` 30 holds a client of the shell's commands (`idl/shell.wit`, lent for `REQUEST_SHELL` to a window manager and to what the shell's window starts, and by `wm` on to `console`: the shell's window from the menu, the shell's commands from `console`, 211-APP-0044) and `SLOT_CLIPBOARD` 31 a client of the clipboard service (`REQUEST_CLIPBOARD`, 000-APP-0032); `SLOT_NET` 16 may carry a network card's read-only client (`BADGE_COUNTERS`: `info`, `counters` and `offloads` only), which the shell mints from its own copy and lends for `REQUEST_NET_COUNTERS` (000-NET-0015); `LAUNCH_SLOTS` in `common/abi.rs` lists the fixed slots a launcher may fill; the kernel hands out slots from `SLOT_DYNAMIC` (34).

## System calls

This is the full list: every `SYSCALL_*` constant of `common/abi.rs`, which remains the definition. The README's table is a short form of it. Required authority is in brackets; "none" means every task may call it.

### Time, console, lifetime

| No. | Name | Arguments → result |
|---|---|---|
| 1 | `RDTSC` | → time stamp counter [none] |
| 6 | `UPTIME` | → milliseconds since boot [none] |
| 44 | `CLOCK` | → monotonic ns; `arg2` = resolution ns, `msg[2]` = calibrated TSC Hz (0: tick clock) [none] |
| 5 | `WAIT` | `arg1` = ms (at least 1, at most 60 s; ends early on input) → uptime at the call [none]. It ends at the first tick after the time asked (1 ms on x86, 10 ms on aarch64), never sooner by the monotonic clock (000-KRN-0065, 000-KRN-0083). IPC timeouts count from the uptime and may end up to a tick early |
| 3 | `LOG` | `arg1` = address, `arg2` = length (≤ 4096) → bytes written to the task's log and console. While a reader drains the console (`CONSOLE_READ` within the last second; an application counts as read from its start), only what fits in the console's 4 KiB is taken and the caller sends the rest again (`mind::process::log` waits 2 ms between tries), so a console program's output is not lost to a slow reader; after a second without reads the console takes everything and drops its oldest bytes, as before (000-KRN-0030) [none] |
| 7 | `EXIT` | `arg1` = code (0: success; issue 166); ends the task, does not return; a watch and `EXIT_STATUS` see `EXIT_NORMAL` \| code << 8 [none] |
| 2 | `READ_KEY` | → legacy byte of the next key event that has one (pointer events are skipped: an absolute one's byte is its horizontal steps, ABI 15), 0 if none [focused task] |
| 50 | `READ_INPUT` | → next input event word (layout in `common/abi.rs`, `input_event`; pointer events: `KEY_POINTER`, `pointer_fields` for a mouse's movement, `pointer_absolute_fields` when `POINTER_ABSOLUTE` is set: a tablet's position as a share of the screen in `POINTER_SCALE` steps, issue 161; `pointer_across` for the horizontal wheel, a trackpad's three-finger swipe, 211-DRV-0018: a relative event's, and since ABI 15 an absolute one's, made with `pointer_absolute_scroll`, 211-KRN-0150; `KEY_PRINT_SCREEN` (33) for PrintScreen, ABI 15, 000-KRN-0149), 0 if none [focused task] |
| 56 | `INPUT_POINTER` | `arg1` = 1 to receive pointer events, 0 to stop (the caller only; without it the kernel drops them) [none] |
| 57 | `INPUT_LISTEN` | `arg1` = key \| modifiers << 16 (`MOD_SHIFT`/`MOD_CTRL`/`MOD_ALT`, exactly those held), `arg2` = 1 to listen, 0 to stop: presses of that key and their releases go to the caller's input queue, not the focused task's; 8 in the system, ended with the task; `mind::input::listen` [process control] |

### Memory

| No. | Name | Arguments → result |
|---|---|---|
| 8 | `ALLOC` | `arg1` = bytes, `arg2` = 0 or `ALLOC_SMALL_PAGES` (any other bit: 0) → address of a new zeroed heap block from the frame pool, 0 on failure (the memory quota of the task and of every payer above it, `HEAP_MAX_BLOCKS`). The heap window is the rest of the task's 512 GiB top-level entry. A block of 2 MiB or more lies on a 2 MiB boundary, one of 1 GiB or more on a 1 GiB boundary, in its address and, where the pool allows, in its frames; a block of more than 64 MiB is zeroed after the kernel lets go of its lock, so other CPUs go on meanwhile (262-KRN-0160). The block is mapped in the largest pages that lie whole in it with both their addresses on their boundary: 1 GiB pages where the CPU has them (x86-64: `CPUID 80000001h` EDX bit 26; aarch64: always), else 2 MiB pages, else 4 KiB pages; with `ALLOC_SMALL_PAGES`, 4 KiB pages only (262-KRN-0161) |
| 9 | `FREE` | `arg1` = block address; memory still referenced elsewhere is kept until released and stays charged to the owner |
| 15 | `MEM_SHARE` | `arg1` = address of an own heap block → handle of a memory capability (read, write, grant) |
| 16 | `MEM_MAP` | `arg1` = memory, DMA or MMIO handle → mapped address; `arg2` = size. Read-only without `CAP_WRITE`; MMIO uncached |
| 47 | `MEM_DETACH` | `arg1` = own heap block nobody else refers to → handle of a memory object (MOVE only, or minted read-only) |
| 26 | `MEM_PHYS` | `arg1` = DMA handle → physical address for the device |

Transfer modes (COPY, MOVE, SHARE_RO, LEASE) and the services' use of them: [profile, Memory transfers](../profile/README.md#memory-transfers-appendix-b2).

### Capabilities

| No. | Name | Arguments → result |
|---|---|---|
| 45 | `CAP_MINT` | `arg1` = handle, `arg2` = rights mask, `msg[0]` = offset, `msg[1]` = length (0: to the end), `msg[2]` = badge → child handle with no more authority. A badge is set once, on an unbadged endpoint capability that has `CAP_BADGE` (`ERR_INVALID` otherwise); a badged child never has `CAP_BADGE` |
| 46 | `CAP_REVOKE` | `arg1` = handle → number of descendants removed from all tasks; returns after no CPU can use them. A task that mapped a revoked memory lease keeps the range: each of its pages is one zeroed page of that task's own, charged to it, until it frees the range, so a late access reads zeros and writes there, never to the owner's memory (ABI 17, 000-KRN-0152). With no room for that page the range is unmapped, and a later access faults |
| 14 | `CAP_DROP` | `arg1` = handle; descendants stay revocable |
| 29 | `CAP_INFO` | `arg1` = handle → kind (`CAP_KIND_*`); `arg2` = port base, memory rights or, for a text input's writer, its reader's PID; `msg[2]` = size, port count or endpoint rights; `msg[3]` = 1 if a memory range is sealed |

Rights: `CAP_READ` 1, `CAP_WRITE` 2, `CAP_GRANT` 4, `CAP_KEEP` 8 (may mint receive rights, cannot receive), `CAP_BADGE` 16 (may set a badge with `CAP_MINT`; ABI 6, 000-KRN-0073). A new endpoint has all five; a copy or child keeps `CAP_BADGE` only when the mask does, and the IDL runtime (`libmind` `idl::wire`) never passes it.

### IPC

| No. | Name | Arguments → result |
|---|---|---|
| 12 | `ENDPOINT_CREATE` | → handle of a new endpoint with all rights (endpoint quota) |
| 10 | `IPC_SEND` | `arg1` = endpoint handle, `arg2` = timeout ms << 32 (issue 172: before, in `arg1`); `msg[0]` = capability handle to transfer (0: none), `msg[1]` = rights mask \| `CAP_TRANSFER_MOVE`, `msg[2..4]` = data [write right] |
| 22 | `IPC_CALL` | as `IPC_SEND`, `arg2` = slot for a capability in the reply \| timeout ms << 32; waits for the reply: `msg[2..4]` = reply data [write right] |
| 11 | `IPC_RECV` | `arg1` = endpoint handle, `arg2` = slot for a received capability \| timeout ms << 32 → `arg1` = sender PID, `arg2` = badge, `msg[0]` = 1 if a capability arrived, `msg[1]` = `MSG_FLAG_*`, `msg[2..4]` = data [read right] |
| 23 | `IPC_REPLY` | answers the last received call; `msg` as for a send |
| 31 | `IPC_SAVE_REPLY` | → handle of a one-time reply capability for the last call (answer later) |
| — | reply through a saved capability | `IPC_REPLY` with `arg1` = the saved handle (`libmind::ipc::reply_saved`) |

Senders wait on an endpoint in the order they sent, each until a receiver takes its message or its timeout passes (back-pressure). A task has at most one send waiting, so the tasks bound the wait. On timeout the call leaves no trace: `ERR_TIMEOUT`. **ABI 3 (000-KRN-0010):** before, at most `ENDPOINT_QUEUE` (4) senders waited and a fifth got `ERR_BUSY` at once, and `libmind` retried every 10 ms without order. A program built for ABI 2 stops with `ABI MISMATCH` (exit 126) and is rebuilt; its `ERR_BUSY` handling for sends becomes dead code.

### Tasks

| No. | Name | Arguments → result |
|---|---|---|
| 13 | `SPAWN` | name and grants, see `common/abi.rs` (`Grant`: 16 bytes, a 64-bit handle; `SPAWN_*`, quotas) → PID; `SPAWN_FOREGROUND` with a PID: in front, see the focus rules below (`ERR_FOCUS`); `SPAWN_TEXT`: the task gets a text input, and its writer (`CAP_KIND_TEXT`) goes to the spawner, its handle in `arg2` (`ERR_NO_SLOT` without room for it; 000-KRN-0094). The task's stack is what its ELF's `PT_GNU_STACK` asks: `p_memsz` in whole pages, at least `STACK_MIN_BYTES` (64 KiB); an ELF asking more than `STACK_MAX_BYTES` (8 MiB) is refused. An unmapped page lies below it. A linker script with `PHDRS` sets the size with a `NOLOAD` section placed in that header (`memtest/memstack.ld`; 262-KRN-0164) [spawn privilege] |
| 28 | `TASK_ALIVE` | `arg1` = PID → 1 if it exists [none] |
| 48 | `TASK_WATCH` | `arg1` = PID of an own child, `arg2` = endpoint with read right; its exit arrives there as a `MSG_FLAG_EXIT` message; at once for a child that has already ended, also one already reclaimed while it is among the last `EXIT_STATUSES` that ended (800-KRN-0138) |
| 52 | `SCHED_SET` | `arg1` = PID, `arg2` = budget µs per period (0: none), `msg[0]` = period µs (≥ 10 000), `msg[1]` = band [lifecycle owner or process control] |

### Drivers

| No. | Name | Arguments → result |
|---|---|---|
| 17 | `PORT_IN` | `arg1` = port range handle, `arg2` = port, `msg[1]` = width 1/2/4 → value |
| 18 | `PORT_OUT` | as `PORT_IN`, `msg[0]` = value |
| 27 | `PORT_IN_BLOCK` | `arg1` = handle, `arg2` = port, `msg[2]` = buffer, `msg[3]` = 16-bit words (≤ 2048) → words read |
| 53 | `PORT_OUT_BLOCK` | as `PORT_IN_BLOCK`; the words are written from the buffer → words written |
| 19 | `IRQ_WAIT` | `arg1` = IRQ handle; blocks until the line fires |
| 24 | `IRQ_BIND` | `arg1` = IRQ handle, `arg2` = endpoint with read right: the line arrives as `MSG_FLAG_IRQ` messages; up to `IRQ_SHARERS` (4) drivers share a line, each gets every interrupt, and the line stays masked until each has called `IRQ_ACK` |
| 25 | `IRQ_ACK` | `arg1` = IRQ handle; unmasks the line |
| 20 | `INPUT_EVENT` | routes decoded input [input privilege] |
| 21 | `COMPOSITOR_PULL` | `arg1` = slot for the focused screen, `arg2` = 0 → 0 unchanged, 1 dirty, 2 new screen (read-only capability); `arg2` = `COMPOSITOR_FRAME`: the same, or `COMPOSITOR_BUSY` while the focused task runs with a changed screen, and on 1 the task is held until `arg2` = `COMPOSITOR_RELEASE` or 50 ms (000-KRN-0086) [display privilege] |

### Platform (init only)

| No. | Name | Arguments → result |
|---|---|---|
| 32 | `PLATFORM_CAP` | `arg1` = `PLATFORM_*` kind, `arg2`, `msg[0]` = arguments → handle; every resource is validated by the kernel; `PLATFORM_DEVICE_MSIX` (device, table entry) gives an interrupt line 16–31 whose MSI-X entry the kernel programs (on aarch64 through the GICv3 ITS); `PLATFORM_DEVICE_MSI` (device) the same for a device's MSI capability, one message, unmasked (550-KRN-0091: the BCM4331 has MSI, not MSI-X); `PLATFORM_MMIO` (an index: `PLATFORM_UART`, `PLATFORM_RTC`, `PLATFORM_PINS_PL061` + n, `PLATFORM_PINS_BCM2711` + n, `PLATFORM_TPM`: the TPM 2.0's 4 KiB of locality 0, from the ACPI TPM2 table on x86 or the DSDT's `MSFT0101` on aarch64, 351-KRN-0043) the registers of a platform device outside PCI that the board has: on aarch64 the console UART the SPCR names, QEMU's PL031, and the pin controllers the DSDT and SSDTs name (issue 206); none on x86; `PLATFORM_REPORT` a read-only memory object with the hardware report's text, built when asked (CPU and every CPUID leaf or ID register, model-specific registers read so that a missing one shows as absent, the firmware's memory map, the ACPI tables decoded, every PCI function with its configuration space, the kernel's choices and its last 32 KiB of lines); `PLATFORM_ACPI_TABLE` (an index) a read-only copy of the n-th ACPI table of that list, `ERR_NOT_FOUND` past the last (174-KRN-0038) [platform privilege] `PLATFORM_PRIVILEGE` with `msg[0]` = `PRIVILEGE_ESCROW` gives the privilege in escrow (`CAP_KIND_ESCROW`, issue 170): its holder cannot use it, and a grant of it at a `SPAWN_SERVICE` spawn gives the child the privilege; at any other spawn it is refused. |
| 59 | `MEMORY_RESERVE` | `arg1` = bytes of the frame pool kept for the system band (issue 169): an application-band task's heap block, or an application's image, stack and screen, is refused when it would leave less free (`ALLOC` → 0, `SPAWN` → `ERR_NO_MEMORY`); rounded up to pages, beyond the pool `ERR_INVALID` [platform privilege] |
| 33 | `DEVICE_FIND` | `arg1` = PCI class code, `arg2` = mask, `msg[0]` = n-th match, `msg[1]` = PCI vendor \| device << 16 (0: any) → device index |
| 54 | `DEVICE_CONFIG` | `arg1` = MMIO or port capability over a BAR of a PCI function, `arg2` = offset (< 256) → that function's configuration dword (read only; drivers find their capabilities); with the platform privilege as `arg1`, `msg[0]` = device index: any device, without enabling it |
| 49 | `DEVICE_STATE` | `arg1` = device index, `arg2` = `DEVICE_STOP` / `DEVICE_START` / `DEVICE_REGISTERS` [platform privilege or a BAR capability of the device]; checked Command readback, `ERR_IO` on failure |
| 60 | `BOOT_CONFIRM` | ends a trial boot (351-KRN-0014): the kernel no longer restarts the machine at the deadline → 1 if the boot was on trial, 0 if not [platform privilege] |
| 61 | `FIRMWARE_VARIABLE` | `arg1` = the firmware variable privilege (`CAP_KIND_FIRMWARE`, in `SLOT_FIRMWARE`), `arg2` = `FIRMWARE_GET` / `FIRMWARE_SET`, `msg[0]` = buffer, `msg[1]` = its length (`FIRMWARE_HEADER` 26 to `FIRMWARE_BUFFER` 65536): GUID (16 bytes), attributes (u32), the name's length in UTF-16 units (u16), the data's length (u32), the name (UTF-16LE), the data. GET writes the attributes, the length and the data back → length; `ERR_NOT_FOUND` for a variable not set or a firmware without runtime variable services; `ERR_INVALID` with the needed length in `arg2` when the data does not fit; `ERR_RIGHTS` when the firmware refuses a write (an authenticated variable such as `dbx` takes only a write signed with a key it trusts: `mind::firmware::AUTHENTICATED`, `APPEND`, 351-KRN-0028). The kernel calls the firmware's `GetVariable` / `SetVariable` in its identity map, one CPU at a time (351-KRN-0027; on aarch64 FP/SIMD is enabled throughout and the task's registers come back from its frame, 351-KRN-0028, 250-KRN-0056; `mind::firmware`) [firmware privilege] |
| 62 | `TEXT_WRITE` | `arg1` = the writer of a task's text input (`CAP_KIND_TEXT`, from `SPAWN_TEXT`; `CAP_INFO` names the reader's PID), `msg[0]` = bytes, `msg[1]` = their length → how many were queued (at most `TEXT_QUEUE`, 4096, wait to be read; 0 while it is full); with `TEXT_CLOSE` in `arg2` and every byte queued, the input ends after them. `ERR_PEER` once the reader ended, `ERR_INVALID` once the input was closed, `ERR_RIGHTS` without a writer (000-KRN-0094; `mind::io::Writer`) [the writer] |
| 63 | `TEXT_READ` | the task's own text input: `arg1` = buffer, `arg2` = its length → the bytes queued, up to it; 0 at the end, which comes once the input was closed or no copy of its writer is left, and stays. With nothing queued it waits up to `TEXT_WAIT_MS` (1000) for bytes, the close or any input event (a key), then answers `ERR_BUSY`: the caller asks again. `ERR_NOT_FOUND` at once for a task started without one, whose input stays its keys alone (000-KRN-0094; `mind::io`) |
| 64 | `WORKER_START` | `arg1` = the worker's argument word, `arg2` = its stack in bytes (0: what the ELF asks; more than `STACK_MAX_BYTES`: `ERR_INVALID`), `msg[2]` = address of a `Grant` array, `msg[3]` = their count → PID. A worker runs the caller's own program in an address space of its own, with exactly the granted capabilities (copied or moved as at `SPAWN`, never a privilege); its info page holds its index (1, 2, …) and argument at `WORKER_OFFSET`. It goes to a CPU where neither its creator nor another of its workers runs while one is free, counts in the caller's task quota and is charged to its memory quota, and ends when the caller ends; `TASK_WATCH` gives its exit notice. Only a task started with `SPAWN_WORKERS` may (`ERR_RIGHTS`; the loader sets it for a program the owner names in `data/memory.conf`), at most one live worker per online CPU (`ERR_LIMIT`). Workers, not threads: see 262-KRN-0163 (ABI 18) |

### Observation and process control

| No. | Name | Arguments → result |
|---|---|---|
| 51 | `STAT` | `arg1` = `STAT_*` class, `arg2` = buffer, `msg[0]` = capacity, `msg[1]` = PID for VMAP/CAPS (1 for MEMORY: also find the largest free block), `msg[2]` = the first record to write (0: from the start; pages through any number of records, issue 171) → records written (`StatHeader` with `STAT_VERSION`, then records; a reader accepts records larger than it knows) [observe or process control] |
| 34 | `TASK_LIST` | `arg1` = `TaskInfo` array, `arg2` = capacity → count [observe or process control] |
| 40 | `FAULTS` | `arg1` = `FaultInfo` array, `arg2` = capacity → count [observe or process control] |
| 41 | `CPU_INFO` | `arg1` = CPU index → APIC id; `arg2` = online, `msg[2]` = ticks [observe or process control] |
| 42 | `KERNEL_HEAP` | → used bytes; `arg2` = free, `msg[2]` = 1 if a test allocation was released [observe or process control] |
| 35 | `TASK_KILL` | `arg1` = PID [process control, or an ancestor of the task: its spawner or a live task above it (issue 170)] |
| 36 | `FOCUS` | `arg1` = PID (0: caller), `arg2` = 1 to keep buffered output → PID [process control] |
| 37 | `TASK_LOGS` | `arg1` = PID, `msg[0]` = buffer, `msg[1]` = length → bytes drained [process control] |
| 38 | `CONSOLE_READ` | as `TASK_LOGS`, the console copy; after the last focused or screenless program exited, both drain its unread console output [process control] |
| 58 | `EXIT_STATUS` | `arg1` = PID of one of the last `EXIT_STATUSES` (16) tasks that ended → how it ended: `EXIT_NORMAL` \| code << 8, `EXIT_KILLED`, `EXIT_FAULT` \| vector << 8; `ERR_NOT_FOUND` for another PID (issue 166) [process control] |
| 39 | `NOTICE` | → 0, or PID \| `NOTICE_EXITED` / PID \| `NOTICE_FRONT` (started in front by the task in front, issue 160) / PID sent to the background [process control] |
| 43 | `HALT` | stops all CPUs: the others first, then COM1 and the screen say `MIND CORE: THE SYSTEM IS STOPPED`; the run's mark is cleared, as at a `REBOOT` (000-KRN-0110) [process control] |
| 55 | `REBOOT` | resets the machine: the ACPI FADT reset register, else port 0xCF9, else the 8042 controller, else a triple fault; on aarch64 PSCI `SYSTEM_RESET`; does not return. `arg1` = `REBOOT_POWER_OFF` turns the machine off instead (aarch64: PSCI `SYSTEM_OFF`; x86: ACPI S5, the sleep type from the DSDT's `\_S5` written to PM1a/PM1b_CNT with SLP_EN, 000-KRN-0080; `ERR_INVALID` when the tables name no S5; the firmware's `_PTS` is not run, and a machine still on is halted with a message on the screen) [process control, or the restart privilege: `init`, which resets the machine for a client of its lifecycle interface badged `BADGE_REBOOT`, the updater's, 351-KRN-0022] |

**The focus** (MC-10.2): one task is in front. Its screen is shown and it gets the input events, except keys taken with `INPUT_LISTEN`.

- **`FOCUS`.** A holder of process control (the shell) puts a task with a screen in front and becomes the *focus owner*. The task it focuses returns to it.
- **`SPAWN_FOREGROUND`.** The spawner (the loader, for `commit-in-front`) starts a task with a screen in front, for the task whose PID it names.
  - The kernel checks, under the same lock as the start, that the named task is in front. Otherwise `SPAWN` fails with `ERR_FOCUS` and nothing starts, so a task in the background cannot take the screen.
  - The new task's input starts empty, as after `FOCUS` (issue 160).
  - The focus owner gets a `NOTICE` with the new task's PID and `NOTICE_FRONT`. The shell then also shows what that task prints.
  - The shell starts its own foreground programs this way too (issue 158). One that ends before the shell could focus it then still leaves its output and `NOTICE_EXITED`, because it was in front when it ended.
- **Ctrl+Z** (the attention key) puts the focus owner in front, and the owner gets a `NOTICE` with the PID that was sent to the background.
- **When the task in front ends:**
  - A task started with `SPAWN_FOREGROUND` gives the focus back to the task that started it, if that task still runs and is not the focus owner. The owner gets no notice: for it, that task never left the front.
  - Otherwise the focus goes to the focus owner, which gets `NOTICE_EXITED`, or to no task if the owner has ended.
- **A `FOCUS` on a task** makes it return to the focus owner from then on.

## libmind

`libmind` (crate name `mind`, [`libmind/src`](../../libmind/src)) is `no_std`; a program declares `mind::entry!(main)` and gets `main(info: &'static BootInfo)`.

| Module | Content |
|---|---|
| `sys` | `syscall`, `Error`, `Result`, `check` |
| `ipc` | `Endpoint` (send, call, recv with timeouts), `Message`, `Received`, `mint`, `mint_badged`, `revoke`, `reply`, `save_reply` |
| `mem` | `Pages` (own heap block: share, detach), `Mapping`, `sealed` |
| `firmware` | `get`, `set`: the firmware's UEFI variables (`GLOBAL`, `BOOT_VARIABLE`: `BootNext`, `BootOrder`, `Boot####`) through `FIRMWARE_VARIABLE` with the privilege in `SLOT_FIRMWARE`, which a program gets for `REQUEST_FIRMWARE` when the user allows it (351-KRN-0027; `efivar`) |
| `process` | `exit`, `spawn` (a launch session with one grant), `spawn_with_args`, `spawn_fed` (with `SPAWN_TEXT`: the PID and the writer of its text input, 000-KRN-0094), `start_worker` and `worker` (262-KRN-0163), `args`, `watch`, `alive`; `request!` and `REQUEST_*`: what a program asks its launcher for; `about!`: what the program does — the first statement of `main` in every application, printed for `--help` and kept in the `.mind_about` section, which the shell's `help <name>` reads with `section` |
| `time` | `sleep`, `uptime_ms`, `monotonic_ns` |
| `input` | `KeyEvent`, `read_event`, `wait_event` |
| `io` | a program's text input (000-KRN-0094): `has_input`, `read` (0 at the end), `read_ready`, `read_to_end`, `Lines` (`next`; `next_from` takes any source: tests/io_host.rs); the launcher's `Writer` (`write_some`, `write_all`, `close`, `reader`), from `loader.wit` 1.9 `commit-with-input` or `process::spawn_fed` |
| `keys` | `Key`, `Code`, `Event`: key words for programs; PS/2 scan-code decoder with US/Russian layouts (`Ps2`), VT100/xterm and UTF-8 decoder for the serial line (`Vt`) |
| `tui` | text UI on the 8×16 font: cell grid with diffs, frames, lists, tables, menus, dialogs, input lines, graphs |
| `gfx` | `Screen`: pixels, text, rectangles on the task's screen |
| `fs`, `audio`, `tts`, `rtc` | clients of the VFS (`File`; `Dir` with `list`, `rename`, `remove`, `volume`, `check`, `scope`; `list`), audio, speech and clock services (`rtc::unix_time`) |
| `network` | badges of network stack clients (operator, policy, flow grants) and of the key service's signer |
| `http` | HTTP/1.1 downloads (351-NET-0001, [docs/network/downloads.md](../network/downloads.md)): `Url::parse`, `get` (a GET from an offset, `Range` when not 0, the body into a `Sink` over the caller's `Transport`, the head parsed by a `Parser` and checked against what was asked), `Got` (how far it got: a cut is not an error, the caller resumes from `end`); a chunked body, a redirect or another status is refused; `parse_head` and `Local`, the parser in the process (host tests) |
| `parse` | the parser service's client (109-NET-0008, `idl/parse.wit`): `Service(endpoint)`, an `http::Parser` that has the service parse each response head, and the typed record's conversions; `read_channel` and `read_manifest` (351-NET-0011), which take the service's reading of a release file only if it encodes again to exactly the file (`Refused::Lied` otherwise) |
| `json` | a JSON reader without allocation (251-STO-0014): `Reader` gives each value's kind, walks objects (`object`, `key`) and arrays (`array`, `item`), copies strings with escapes decoded, reads non-negative integers, skips any value; UTF-8 only, nesting at most 32 |
| `models` | a model disk's `MANIFEST.json` (251-STO-0014): `read` gives the number of models and one model's id, entry bounds and files with sizes and SHA-256 |
| `release` | release metadata (351-UPD-0013, [docs/update/publishing.md](../update/publishing.md)): `parse_channel` and `Manifest::parse` read a channel file and a boot manifest in their one encoding only; `encode_channel`, `encode_file`, `channel_is` and `Rebuild` encode them again for the check of a parser's answer; `expires_unix` |
| `window` | window surfaces (issue 157): the layout the window broker lends to a program and its manager — header, title, input events, cells or pixels, the changed rectangle (`Surface`) |
| `windowed` | a program in a window (issue 088): `Terminal::open` and `pixels` draw into the window its launcher's broker client gives it; `Window` opens another window through a broker client the program names (the shell's window in `wm`, 211-APP-0040), and `tui::Terminal::in_window` draws into it at the sizes its manager asks for |
| `random` | random bytes from RDRAND (`available`, `u64`, `fill`); no fallback: callers fail closed |
| `log` | the system log: every `println!` line of a process holding a `logd` client goes there; `write`, `read`, `state` |
| `stat` | `STAT` records as typed slices (`read`, `one`) and their names |
| `block`, `block_protocol` | block device client; the common driver loop with the write badge checks (`Driver`, `read`, `write`, `flush`) |
| `checkpoint` | checkpoints of a component's state in the block store ([docs/storage/checkpoints.md](../storage/checkpoints.md)): `Manifest` (contract, schema, epoch, sequence, state, resources, authorities, effect journal with `begin` and `settle`), `restore`, `save` (fenced if another instance saved first), `rebind`, over a `Store` trait |
| `blockstore` | rights of block store clients by badge: `BADGE_GET` (get, has, resolve, history, snapshot), `BADGE_PUT` (put, collect), `BADGE_PUBLISH` (publish, unpublish, commit, pin, unpin), any of them (stat, pins, usage), `allowed(badge, Operation)` ([docs/storage](../storage/README.md)); the calls are in `idl::blockstore` |
| `dag` | objects larger than a block ([docs/storage](../storage/README.md)): `Builder` (chunks and DAG-CBOR nodes, the root of the bytes), `size`, `read_at`, `walk` and `complete` (every node and chunk checked against its CID and the shape), `encode`/`decode` of nodes, the `Blocks` trait of a store |
| `cid`, `sha256` | content identifiers ([docs/storage](../storage/README.md)): `Cid` (CIDv1, `raw` or `dag-cbor` content, SHA-256) with `raw`, `matches`, binary (`to_bytes`, `from_bytes`, `read`) and text (`to_text`, `from_text`, `Display`) forms; unsupported versions, types, algorithms and non-canonical encodings are refused (`cid::Error`); SHA-256 (`digest`, `Sha256`) |
| `control` | process control and statistics (`stat`, `records`, `sched_set`) |
| `dev`, `platform` | ports, IRQ, MMIO, DMA, device state for drivers and init |
| `util`, `font`, `font16` | fixed-capacity text buffers (`FixedBuf`), the 8×8 font, MIND Mono 16 (8×16 with Cyrillic and box drawing) |
| `pattern` (feature `alloc`) | name masks (`matches`, `glob`) and simple regular expressions (`Pattern`: `.`, `*`, classes, anchors, case folding) |
| `voice` (feature `alloc`) | voice front end: `Source` (`Microphone`, `Wav`), `Stream` (16 kHz mono), `Resampler`, `Detector` → `Utterance { start_ms, samples, level }`; recognition: `features` (integer log-mel), `model` (`voice/model.bin`, int8 phone scores), `grammar` (`voice/commands.txt`, Viterbi against a phone-loop filler), `recognizer::Recognizer` ([docs/voice](../voice/README.md)) |
| `heap` (feature `alloc`) | the program heap behind `alloc` (`Vec`, `String`, `Box`): size classes in arenas taken with `ALLOC`, large blocks directly |
| `idl` | generated MIND IDL bindings (`idl::vfs`, `idl::audio`, ...) and their codec |

## Stability

MIND Core is pre-1.0. The system call numbers, records and service interfaces can change between commits; a change is recorded in the commit, in `common/abi.rs` and here. Service interfaces carry a version: a change that breaks an existing function increments the major version and old clients get status 0x81 instead of misread data ([docs/idl, Evolution](../idl/README.md#evolution-mc-124)). A stable ABI is a goal of a later roadmap stage, not a current promise.

### Staged PCI startup (ABI 11)

`PLATFORM_DEVICE_BAR_IDLE` (kind 14; device index and memory BAR) grants MMIO
with memory decoding on, I/O decoding and bus mastering off. It rejects an
I/O BAR. `DEVICE_REGISTERS` (state 2) restores the same checked state through
a retained BAR capability, without retaining the platform privilege.
`DEVICE_STOP`, `DEVICE_REGISTERS` and `DEVICE_START` return `ERR_IO` when
Command readback does not match; START enables decoding only for implemented
BAR spaces. MSI configuration never changes Command; MSI-X requires MEM
already enabled. Drivers must prepare their DMA engine before DEVICE_START.

The RTL8852BE native diagnostic (213-KRN-0114) uses the existing ABI 11
`SLOT_MEM` DMA grant with 8 MiB retained storage. Command DMA occupies a
128 KiB reservation and RX DMA the remaining disjoint region. No new syscall,
IDL version or WLAN endpoint is introduced. Drivers install every ring before
mastering and retain memory through shutdown failures; these conventions do
not establish PCIe transaction drainage or safe reuse.

### RTL8852BE startup diagnostic

The ABI-11 RTL8852BE boot service receives its MEM-only BAR2, retained 8 MiB
DMA, read-only firmware client and PCI index from init. A verified firmware
download is followed by a real PHY-capability mailbox query, runtime MAC and
receive-only setup. One consumed firmware token then binds normal H2C, actual
BTC policy, BB/RF tables, initial DM and RCK/DACK/RXDCK to the same command/RX
allocations. The report tail ends this diagnostic session; independent
stop/mastering-disable/power-off follows every modified attempt. Channel
calibration and transmit-policy admission remain pending. Logs distinguish raw firmware capabilities and successful
zero-field defaults. No RTL8852BE radio endpoint or WLAN service is exposed yet.
See [the service scope](../../rtw89_wifi/README.md) and
[213-KRN-0114](../../issues/213-KRN-0114-realtek-driver-startup.md).
