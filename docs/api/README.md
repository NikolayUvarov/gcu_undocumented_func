# MIND Core API

**Version:** ABI 0.x (2026-10-04) · **Stability:** unstable, see [Stability](#stability)

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
- **Capabilities** are named by handles `slot | generation << 12` (`HANDLE_SLOT_MASK` 0xFFF, `HANDLE_GENERATION_SHIFT` 12; before issue 171: 8). A task's table starts with 96 slots and grows on demand up to 4095 (`CAP_SLOTS_MAX`). Slots 1–25 are fixed by convention (generation 0; 7 and 10–12 carry the capabilities a launcher grants on request, 13–25 are further clients, see below; 23, `SLOT_GPIO`, is the pin controller service's client, issue 207; 24, `SLOT_CAMERA`, the video gateway's, issue 158; 25, `SLOT_BLOCKSTORE`, the block store's, lent for `REQUEST_BLOCKSTORE`, 300-KRN-0001); the kernel hands out slots from 26 (`SLOT_DYNAMIC`). A stale handle is rejected. There are no global names: a task can use only what it was granted (Constitution MC-3.3).
- The framebuffer is described in `BootInfo`: `stride` pixels per line, 4 bytes per pixel, `pixel_format` (`PIXEL_RGB`, `PIXEL_BGR`, `PIXEL_BITMASK` with `pixel_masks`). A task's screen always holds `0x00RRGGBB`; only the compositor writes the framebuffer and converts (`pixel_to_device`).
- Application slots, filled by the loader: `SLOT_INIT` 1, `SLOT_RTC` 2, `SLOT_VFS` 3, `SLOT_AUDIO` 4, `SLOT_LOADER` 5, `SLOT_TTS` 6. A launcher fills more through a launch session (`idl/loader.wit` 1.1; `grant-memory` 1.3 takes a read-only memory capability, in `SLOT_DISPLAY` only) when the program asks for them with `mind::request!` and the launcher agrees: `SLOT_FILE` 7 (a VFS client for `REQUEST_FILE` / `REQUEST_FILES`), `SLOT_SYSINFO` 10 (`sysmon`), `SLOT_LIFECYCLE` 11 (`init`'s lifecycle requests), `SLOT_LOG` 12 (reading the system log); `SLOT_INIT` 1 may carry an endpoint for a ping/pong pair. Slots 13–15 are the shell's further clients for such grants: `SLOT_AUTHORITY` 13 (`sysmon` with `mind::stat::BADGE_AUTHORITY`, lent in `SLOT_SYSINFO` to a program that asks for `REQUEST_AUTHORITY`), `SLOT_KEYBOARD` 14 (`ps2_kbd`), `SLOT_DISPLAY` 15 (`compositor`, lent in the same slot to a program that asks for `REQUEST_DISPLAY`, issue 165; from `wm` that slot holds instead a read-only memory lease of the window in front, issue u014); 16–17 are the shell's network clients (`SLOT_NET`, `SLOT_SOCKET` with the operator's badge), `SLOT_NETWORK` 18 holds an application's flow grant (`REQUEST_NETWORK`; in the shell its broker client is `SLOT_NETPOLICY` 19), `SLOT_TLS` 20 is the shell's client of the TLS service, 21–22 its clients of the window broker (`SLOT_WINDOWS`, `SLOT_WINDOW_MANAGER` with `mind::window::BADGE_MANAGER`), lent in an application's `SLOT_WINDOW` 8 for `REQUEST_WINDOW` and `REQUEST_WINDOW_MANAGER`; `SLOT_CONSOLE` 9 may hold an endpoint of the program's launcher, and `mind::process::log` (`println!`) sends what the program prints there too, 15 bytes a message (`mind::output`, issue 162; `console` lends it); the kernel hands out slots from `SLOT_DYNAMIC` 23.

## System calls

Required authority is in brackets; "none" means every task may call it.

### Time, console, lifetime

| No. | Name | Arguments → result |
|---|---|---|
| 1 | `RDTSC` | → time stamp counter [none] |
| 6 | `UPTIME` | → milliseconds since boot [none] |
| 44 | `CLOCK` | → monotonic ns; `arg2` = resolution ns, `msg[2]` = calibrated TSC Hz (0: tick clock) [none] |
| 5 | `WAIT` | `arg1` = ms (10 ms granularity, at most 60 s; ends early on input) → uptime at the call [none] |
| 3 | `LOG` | `arg1` = address, `arg2` = length (≤ 4096) → bytes written to the task's log and console [none] |
| 7 | `EXIT` | ends the task [none] |
| 2 | `READ_KEY` | → legacy byte of the next input event that has one, 0 if none [focused task] |
| 50 | `READ_INPUT` | → next input event word (layout in `common/abi.rs`, `input_event`; pointer events: `KEY_POINTER`, `pointer_fields` for a mouse's movement, `pointer_absolute_fields` when `POINTER_ABSOLUTE` is set: a tablet's position as a share of the screen in `POINTER_SCALE` steps, issue 161), 0 if none [focused task] |
| 56 | `INPUT_POINTER` | `arg1` = 1 to receive pointer events, 0 to stop (the caller only; without it the kernel drops them) [none] |
| 57 | `INPUT_LISTEN` | `arg1` = key \| modifiers << 16 (`MOD_SHIFT`/`MOD_CTRL`/`MOD_ALT`, exactly those held), `arg2` = 1 to listen, 0 to stop: presses of that key and their releases go to the caller's input queue, not the focused task's; 8 in the system, ended with the task; `mind::input::listen` [process control] |

### Memory

| No. | Name | Arguments → result |
|---|---|---|
| 8 | `ALLOC` | `arg1` = bytes → address of a new zeroed heap block from the frame pool, 0 on failure (the memory quota of the task and of every payer above it, `HEAP_MAX_BLOCKS`) |
| 9 | `FREE` | `arg1` = block address; memory still referenced elsewhere is kept until released and stays charged to the owner |
| 15 | `MEM_SHARE` | `arg1` = address of an own heap block → handle of a memory capability (read, write, grant) |
| 16 | `MEM_MAP` | `arg1` = memory, DMA or MMIO handle → mapped address; `arg2` = size. Read-only without `CAP_WRITE`; MMIO uncached |
| 47 | `MEM_DETACH` | `arg1` = own heap block nobody else refers to → handle of a memory object (MOVE only, or minted read-only) |
| 26 | `MEM_PHYS` | `arg1` = DMA handle → physical address for the device |

Transfer modes (COPY, MOVE, SHARE_RO, LEASE) and the services' use of them: [profile, Memory transfers](../profile/README.md#memory-transfers-appendix-b2).

### Capabilities

| No. | Name | Arguments → result |
|---|---|---|
| 45 | `CAP_MINT` | `arg1` = handle, `arg2` = rights mask, `msg[0]` = offset, `msg[1]` = length (0: to the end), `msg[2]` = badge → child handle with no more authority |
| 46 | `CAP_REVOKE` | `arg1` = handle → number of descendants removed from all tasks; returns after no CPU can use them |
| 14 | `CAP_DROP` | `arg1` = handle; descendants stay revocable |
| 29 | `CAP_INFO` | `arg1` = handle → kind (`CAP_KIND_*`); `arg2` = port base or memory rights; `msg[2]` = size, port count or endpoint rights; `msg[3]` = 1 if a memory range is sealed |

Rights: `CAP_READ` 1, `CAP_WRITE` 2, `CAP_GRANT` 4, `CAP_KEEP` 8 (may mint receive rights, cannot receive).

### IPC

| No. | Name | Arguments → result |
|---|---|---|
| 12 | `ENDPOINT_CREATE` | → handle of a new endpoint with all rights (endpoint quota) |
| 10 | `IPC_SEND` | `arg1` = endpoint handle \| timeout ms << 32; `msg[0]` = capability handle to transfer (0: none), `msg[1]` = rights mask \| `CAP_TRANSFER_MOVE`, `msg[2..4]` = data [write right] |
| 22 | `IPC_CALL` | as `IPC_SEND`, `arg2` = slot for a capability in the reply; waits for the reply: `msg[2..4]` = reply data [write right] |
| 11 | `IPC_RECV` | `arg1` = endpoint handle \| timeout ms << 32, `arg2` = slot for a received capability → `arg1` = sender PID, `arg2` = badge, `msg[0]` = 1 if a capability arrived, `msg[1]` = `MSG_FLAG_*`, `msg[2..4]` = data [read right] |
| 23 | `IPC_REPLY` | answers the last received call; `msg` as for a send |
| 31 | `IPC_SAVE_REPLY` | → handle of a one-time reply capability for the last call (answer later) |
| — | reply through a saved capability | `IPC_REPLY` with `arg1` = the saved handle (`libmind::ipc::reply_saved`) |

At most `ENDPOINT_QUEUE` (4) senders wait on one endpoint; one more gets `ERR_BUSY` (back-pressure). On timeout the call leaves no trace: `ERR_TIMEOUT`.

### Tasks

| No. | Name | Arguments → result |
|---|---|---|
| 13 | `SPAWN` | name and grants, see `common/abi.rs` (`Grant`, `SPAWN_*`, quotas) → PID; `SPAWN_FOREGROUND` with a PID: in front, see the focus rules below (`ERR_FOCUS`) [spawn privilege] |
| 28 | `TASK_ALIVE` | `arg1` = PID → 1 if it exists [none] |
| 48 | `TASK_WATCH` | `arg1` = PID of an own child, `arg2` = endpoint with read right; its exit arrives there as a `MSG_FLAG_EXIT` message |
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
| 21 | `COMPOSITOR_PULL` | `arg1` = slot for the focused screen → 0 unchanged, 1 dirty, 2 new screen (read-only capability) [display privilege] |

### Platform (init only)

| No. | Name | Arguments → result |
|---|---|---|
| 32 | `PLATFORM_CAP` | `arg1` = `PLATFORM_*` kind, `arg2`, `msg[0]` = arguments → handle; every resource is validated by the kernel; `PLATFORM_DEVICE_MSIX` (device, table entry) gives an interrupt line 16–31 whose MSI-X entry the kernel programs (on aarch64 through the GICv3 ITS); `PLATFORM_MMIO` (an index: `PLATFORM_UART`, `PLATFORM_RTC`, `PLATFORM_PINS_PL061` + n, `PLATFORM_PINS_BCM2711` + n) the registers of a platform device outside PCI that the board has: on aarch64 the console UART the SPCR names, QEMU's PL031, and the pin controllers the DSDT and SSDTs name (issue 206); none on x86 [platform privilege] `PLATFORM_PRIVILEGE` with `msg[0]` = `PRIVILEGE_ESCROW` gives the privilege in escrow (`CAP_KIND_ESCROW`, issue 170): its holder cannot use it, and a grant of it at a `SPAWN_SERVICE` spawn gives the child the privilege; at any other spawn it is refused. |
| 59 | `MEMORY_RESERVE` | `arg1` = bytes of the frame pool kept for the system band (issue 169): an application-band task's heap block, or an application's image, stack and screen, is refused when it would leave less free (`ALLOC` → 0, `SPAWN` → `ERR_NO_MEMORY`); rounded up to pages, beyond the pool `ERR_INVALID` [platform privilege] |
| 33 | `DEVICE_FIND` | `arg1` = PCI class code, `arg2` = mask, `msg[0]` = n-th match, `msg[1]` = PCI vendor \| device << 16 (0: any) → device index |
| 54 | `DEVICE_CONFIG` | `arg1` = MMIO or port capability over a BAR of a PCI function, `arg2` = offset (< 256) → that function's configuration dword (read only; drivers find their capabilities); with the platform privilege as `arg1`, `msg[0]` = device index: any device, without enabling it |
| 49 | `DEVICE_STATE` | `arg1` = device index, `arg2` = `DEVICE_STOP` / `DEVICE_START` [platform privilege or a BAR capability of the device] |

### Observation and process control

| No. | Name | Arguments → result |
|---|---|---|
| 51 | `STAT` | `arg1` = `STAT_*` class, `arg2` = buffer, `msg[0]` = capacity, `msg[1]` = PID for VMAP/CAPS (1 for MEMORY: also find the largest free block) → records written (`StatHeader` with `STAT_VERSION`, then records; a reader accepts records larger than it knows) [observe or process control] |
| 34 | `TASK_LIST` | `arg1` = `TaskInfo` array, `arg2` = capacity → count [observe or process control] |
| 40 | `FAULTS` | `arg1` = `FaultInfo` array, `arg2` = capacity → count [observe or process control] |
| 41 | `CPU_INFO` | `arg1` = CPU index → APIC id; `arg2` = online, `msg[2]` = ticks [observe or process control] |
| 42 | `KERNEL_HEAP` | → used bytes; `arg2` = free, `msg[2]` = 1 if a test allocation was released [observe or process control] |
| 35 | `TASK_KILL` | `arg1` = PID [process control, or an ancestor of the task: its spawner or a live task above it (issue 170)] |
| 36 | `FOCUS` | `arg1` = PID (0: caller), `arg2` = 1 to keep buffered output → PID [process control] |
| 37 | `TASK_LOGS` | `arg1` = PID, `msg[0]` = buffer, `msg[1]` = length → bytes drained [process control] |
| 38 | `CONSOLE_READ` | as `TASK_LOGS`, the console copy; after the last focused or screenless program exited, both drain its unread console output [process control] |
| 39 | `NOTICE` | → 0, or PID \| `NOTICE_EXITED` / PID \| `NOTICE_FRONT` (started in front by the task in front, issue 160) / PID sent to the background [process control] |
| 43 | `HALT` | stops all CPUs [process control] |
| 55 | `REBOOT` | resets the machine: the ACPI FADT reset register, else port 0xCF9, else the 8042 controller, else a triple fault; on aarch64 PSCI `SYSTEM_RESET`; does not return. `arg1` = `REBOOT_POWER_OFF` turns the machine off instead (aarch64: PSCI `SYSTEM_OFF`; x86: `ERR_INVALID`, no ACPI sleep states yet) [process control] |

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
| `process` | `exit`, `spawn` (a launch session with one grant), `spawn_with_args`, `args`, `watch`, `alive`; `request!` and `REQUEST_*`: what a program asks its launcher for; `about!`: what the program does — the first statement of `main` in every application, printed for `--help` and kept in the `.mind_about` section, which the shell's `help <name>` reads with `section` |
| `time` | `sleep`, `uptime_ms`, `monotonic_ns` |
| `input` | `KeyEvent`, `read_event`, `wait_event` |
| `keys` | `Key`, `Code`, `Event`: key words for programs; PS/2 scan-code decoder with US/Russian layouts (`Ps2`), VT100/xterm and UTF-8 decoder for the serial line (`Vt`) |
| `tui` | text UI on the 8×16 font: cell grid with diffs, frames, lists, tables, menus, dialogs, input lines, graphs |
| `gfx` | `Screen`: pixels, text, rectangles on the task's screen |
| `fs`, `audio`, `tts`, `rtc` | clients of the VFS (`File`; `Dir` with `list`, `rename`, `remove`, `volume`, `check`, `scope`; `list`), audio, speech and clock services (`rtc::unix_time`) |
| `network` | badges of network stack clients (operator, policy, flow grants) and of the key service's signer |
| `window` | window surfaces (issue 157): the layout the window broker lends to a program and its manager — header, title, input events, cells or pixels, the changed rectangle (`Surface`) |
| `random` | random bytes from RDRAND (`available`, `u64`, `fill`); no fallback: callers fail closed |
| `log` | the system log: every `println!` line of a process holding a `logd` client goes there; `write`, `read`, `state` |
| `stat` | `STAT` records as typed slices (`read`, `one`) and their names |
| `block`, `block_protocol` | block device client; the common driver loop with the write badge checks (`Driver`, `read`, `write`, `flush`) |
| `blockstore` | rights of block store clients by badge: `BADGE_GET` (get, has, resolve), `BADGE_PUT` (put), `BADGE_PUBLISH` (publish), `allowed(badge, Operation)` ([docs/storage](../storage/README.md)); the calls are in `idl::blockstore` |
| `dag` | objects larger than a block ([docs/storage](../storage/README.md)): `Builder` (chunks and DAG-CBOR nodes, the root of the bytes), `size`, `read_at` and `complete` (every node and chunk checked against its CID and the shape), `encode`/`decode` of nodes, the `Blocks` trait of a store |
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
