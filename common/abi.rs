#![allow(dead_code)]
// Single kernel ABI: included by the kernel, bootloader and libmind (do not copy).

// The UEFI bootloader passes the kernel only system service images; the loader service reads applications from disk.
// The kernel starts only image 0 (`init`); init decides which of the others to start and what each one receives.
pub const BOOT_IMAGES: usize = 12;
pub const BOOT_SERVICES: [&str; BOOT_IMAGES] = ["init", "rtc", "ps2_kbd", "compositor", "ata", "ahci", "usb_storage", "vfs_server", "loader", "audio_gw", "tts", "shell"];
pub const BOOT_FILES: [&str; BOOT_IMAGES] = ["init.elf", "rtc.elf", "ps2_kbd.elf", "compositor.elf", "ata.elf", "ahci.elf", "usb_storage.elf", "vfs_server.elf", "loader.elf", "audio_gw.elf", "tts.elf", "shell.elf"];
pub const MAX_APPS: usize = 8; // init's policy: live applications loader may start (its task quota)
pub const NAME_MAX: usize = 16; // task name in ps and in spawn requests

#[derive(Clone, Copy)] #[repr(C)] pub struct ProgramImage { pub data: *const u8, pub len: usize }
#[derive(Clone, Copy)] #[repr(C)] pub struct BootInfo { pub fb_ptr: *mut u32, pub width: usize, pub height: usize, pub stride: usize, pub programs: [ProgramImage; BOOT_IMAGES], pub heap_ptr: *mut u8, pub heap_len: usize, pub ap_trampoline: usize, pub cpu_count: usize, pub apic_ids: [u32; 8], pub memory_map: *const StatPhys, pub memory_map_len: usize, }
#[derive(Clone, Copy)] #[repr(C)] pub struct SyscallMailbox { pub syscall_num: usize, pub arg1: usize, pub arg2: usize, pub result: usize, pub msg: [usize; 4], }
impl SyscallMailbox { pub const EMPTY: Self = Self { syscall_num: 0, arg1: 0, arg2: 0, result: 0, msg: [0; 4] }; }

pub const SYSCALL_RDTSC: usize = 1;
pub const SYSCALL_READ_KEY: usize = 2;
pub const SYSCALL_LOG: usize = 3;
pub const SYSCALL_WAIT: usize = 5;
pub const SYSCALL_UPTIME: usize = 6;
pub const SYSCALL_EXIT: usize = 7;
pub const SYSCALL_ALLOC: usize = 8;
pub const SYSCALL_FREE: usize = 9;
pub const SYSCALL_IPC_SEND: usize = 10;
pub const SYSCALL_IPC_RECV: usize = 11;
pub const SYSCALL_ENDPOINT_CREATE: usize = 12;
pub const SYSCALL_SPAWN: usize = 13;
pub const SYSCALL_CAP_DROP: usize = 14;
pub const SYSCALL_MEM_SHARE: usize = 15;
pub const SYSCALL_MEM_MAP: usize = 16;
pub const SYSCALL_PORT_IN: usize = 17;
pub const SYSCALL_PORT_OUT: usize = 18;
pub const SYSCALL_IRQ_WAIT: usize = 19;
pub const SYSCALL_INPUT_EVENT: usize = 20;
// READ_INPUT: next input event word of the calling (focused) task, 0 if none. READ_KEY returns only the legacy byte
// of the next event that has one.
pub const SYSCALL_READ_INPUT: usize = 50;
pub const SYSCALL_COMPOSITOR_PULL: usize = 21;
pub const SYSCALL_IPC_CALL: usize = 22;
pub const SYSCALL_IPC_REPLY: usize = 23;
pub const SYSCALL_IRQ_BIND: usize = 24;
pub const SYSCALL_IRQ_ACK: usize = 25;
pub const SYSCALL_MEM_PHYS: usize = 26;
pub const SYSCALL_PORT_IN_BLOCK: usize = 27;
pub const SYSCALL_TASK_ALIVE: usize = 28;
pub const SYSCALL_CAP_INFO: usize = 29;
pub const SYSCALL_IPC_SAVE_REPLY: usize = 31;
// Bootstrap authority (holder of the platform capability, i.e. init).
pub const SYSCALL_PLATFORM_CAP: usize = 32;
pub const SYSCALL_DEVICE_FIND: usize = 33;
// Process control (holder of the control capability, i.e. the shell).
pub const SYSCALL_TASK_LIST: usize = 34;
pub const SYSCALL_TASK_KILL: usize = 35;
pub const SYSCALL_FOCUS: usize = 36;
pub const SYSCALL_TASK_LOGS: usize = 37;
pub const SYSCALL_CONSOLE_READ: usize = 38;
pub const SYSCALL_NOTICE: usize = 39;
pub const SYSCALL_FAULTS: usize = 40;
pub const SYSCALL_CPU_INFO: usize = 41;
pub const SYSCALL_KERNEL_HEAP: usize = 42;
pub const SYSCALL_HALT: usize = 43;
// CLOCK: result = monotonic nanoseconds since boot, arg2 = resolution in ns, msg[2] = calibrated TSC Hz (0: tick clock).
pub const SYSCALL_CLOCK: usize = 44;
// Endpoint badges: CAP_MINT with msg[2] = badge (1..=BADGE_MAX) labels an endpoint capability once; its children keep
// the badge and another one is refused. A receiver gets the badge of the capability the sender used in arg2 (0: none).
pub const BADGE_MAX: usize = 0xFFFF;
// CAP_MINT: arg1 = handle, arg2 = rights mask (endpoints and memory), msg[0] = offset, msg[1] = length (0: to the end) for port and
// memory ranges -> handle of a child with no more authority. CAP_REVOKE: arg1 = handle -> number of descendants removed
// from all tasks; the capability itself stays (MC-3.4-3.6).
pub const SYSCALL_CAP_MINT: usize = 45;
pub const SYSCALL_CAP_REVOKE: usize = 46;
// MEM_DETACH: arg1 = start of a heap block no one else refers to -> handle of a memory object (read/write, no grant).
// The block leaves the caller's address space; the object lives while a capability or mapping refers to it. Copying a
// writable memory capability needs CAP_GRANT, so an object can only be moved (MOVE: one owner) or minted read-only.
pub const SYSCALL_MEM_DETACH: usize = 47;
// TASK_WATCH: arg1 = PID of a task the caller spawned, arg2 = endpoint handle with the read right. When the task ends,
// a receive on that endpoint gets msg[1] = MSG_FLAG_EXIT, data = [PID, reason | lost notices << 32] (sender PID 0).
pub const SYSCALL_TASK_WATCH: usize = 48;
// DEVICE_STATE: arg1 = device index, arg2 = DEVICE_STOP or DEVICE_START (platform privilege, or a capability over one
// of the device's BARs). STOP turns off I/O and memory decoding and bus mastering, so the device can no longer reach
// memory by DMA (MC-6.3); START turns them on again for the next driver.
pub const SYSCALL_DEVICE_STATE: usize = 49;
pub const DEVICE_STOP: usize = 0;
pub const DEVICE_START: usize = 1;
pub const EXIT_NORMAL: usize = 0;
pub const EXIT_KILLED: usize = 1;
pub const EXIT_FAULT: usize = 2; // | vector << 8
pub const EXIT_NOTICES_MAX: usize = 16; // undelivered exit notices kept by the kernel; further ones are counted as lost
pub const DETACHED_MAX_BYTES: usize = 16 * 1024 * 1024; // all memory objects and freed-but-referenced blocks together

// CAP_INFO reply: result=capability kind, arg2=port base or memory rights, msg[2]=size/port count/endpoint rights.
// Memory rights: CAP_READ maps, CAP_WRITE maps writable, CAP_GRANT (MEM_SHARE gives all three). For memory msg[3] = 1
// if the range is sealed: no writable capability, writable mapping or DMA region overlaps it anywhere (SHARE_RO). A mapping is removed
// when the capability it was made from is revoked; CAP_REVOKE returns after every CPU has stopped using it.
pub const CAP_KIND_NONE: usize = 0;
pub const CAP_KIND_ENDPOINT: usize = 1;
pub const CAP_KIND_MEMORY: usize = 2;
pub const CAP_KIND_DMA: usize = 3;
pub const CAP_KIND_PORTS: usize = 4;
pub const CAP_KIND_IRQ: usize = 5;
pub const CAP_KIND_INPUT: usize = 6;
pub const CAP_KIND_DISPLAY: usize = 7;
pub const CAP_KIND_MMIO: usize = 8;
pub const CAP_KIND_SPAWN: usize = 9;
pub const CAP_KIND_REPLY: usize = 10;
pub const CAP_KIND_PLATFORM: usize = 11;
pub const CAP_KIND_CONTROL: usize = 12;
pub const CAP_KIND_RESTART: usize = 13; // spawn boot images and services again, nothing else (init after boot)
pub const CAP_KIND_OBSERVE: usize = 14; // read-only statistics: STAT, TASK_LIST, CPU_INFO, KERNEL_HEAP, FAULTS

// Error codes: usize::MAX - n. ALLOC still returns 0 on failure.
pub const ERR_INVALID: usize = usize::MAX;
pub const ERR_NO_SLOT: usize = usize::MAX - 1;
pub const ERR_RIGHTS: usize = usize::MAX - 2;
pub const ERR_NOT_FOUND: usize = usize::MAX - 3;
pub const ERR_PEER: usize = usize::MAX - 4;
pub const ERR_NO_MEMORY: usize = usize::MAX - 5;
pub const ERR_BUSY: usize = usize::MAX - 6; // e.g. the service is already running
pub const ERR_LIMIT: usize = usize::MAX - 7; // task limit reached
pub const ERR_TIMEOUT: usize = usize::MAX - 8; // an IPC deadline passed; the operation left no trace
pub const ERR_FIRST: usize = usize::MAX - 15;
pub const RTC_UNAVAILABLE: usize = usize::MAX;

pub const CAP_READ: u8 = 1 << 0; pub const CAP_WRITE: u8 = 1 << 1; pub const CAP_GRANT: u8 = 1 << 2;
// Keeper: may mint children with CAP_READ without being able to receive itself (init keeps service endpoints this way).
pub const CAP_KEEP: u8 = 1 << 3;
pub const CAP_SLOTS: usize = 64;

// Application capability slots, filled by the spawner (loader) through the SPAWN grant list.
pub const SLOT_INIT: usize = 1;
pub const SLOT_RTC: usize = 2;
pub const SLOT_VFS: usize = 3;
pub const SLOT_AUDIO: usize = 4;
pub const SLOT_LOADER: usize = 5;
pub const SLOT_TTS: usize = 6;
// Service capability slots: served endpoint, devices, IRQ, DMA/frame, privilege.
pub const SLOT_SERVICE: usize = 1;
pub const SLOT_DEV0: usize = 2;
pub const SLOT_DEV1: usize = 3;
pub const SLOT_IRQ: usize = 4;
pub const SLOT_MEM: usize = 5;
pub const SLOT_PRIV: usize = 6;
// For vfs_server, slots 2..5 are block driver endpoints (ata, ahci, usb_storage), if started.
pub const SLOT_BLOCK_FIRST: usize = 2;
pub const BLOCK_DEVICES: usize = 3;
// Shell: application slots plus process control, the input privilege and the COM1 port range.
pub const SLOT_CONTROL: usize = 7;
pub const SLOT_INPUT: usize = 8;
pub const SLOT_SERIAL: usize = 9;
// The kernel hands out new capabilities starting from this slot; slots below it are fixed by convention.
pub const SLOT_DYNAMIC: usize = 10;
// A capability handle is `slot | generation << HANDLE_GENERATION_SHIFT`. Fixed slots (below SLOT_DYNAMIC) are named with
// generation 0; a slot the kernel hands out gets a new generation every time it is freed, so an old handle stays invalid.
// Received capabilities and the compositor's screen are placed only in fixed slots.
pub const HANDLE_SLOT_MASK: usize = 0xFF;
pub const HANDLE_GENERATION_SHIFT: usize = 8;

// Endpoints have no global names: every one is created by ENDPOINT_CREATE (init's own by the kernel) and reached only
// through capabilities (MC-3.3).
// Block device kinds reported by BLOCK_INFO (protocol data, not authority).
pub const BLOCK_KIND_ATA: usize = 1;
pub const BLOCK_KIND_AHCI: usize = 2;
pub const BLOCK_KIND_USB: usize = 3;

// Message: msg[0]=handle of the capability to transfer, msg[1]=rights mask | CAP_TRANSFER_MOVE, msg[2..4]=data.
// The mask narrows endpoint rights only; other capabilities keep their rights (narrow memory with CAP_MINT first).
// A transfer is a copy (a child the sender can revoke) unless CAP_TRANSFER_MOVE moves it out of the sender's table.
pub const CAP_TRANSFER_MOVE: usize = 1 << 8;
// IPC_SEND, IPC_CALL, IPC_RECV: arg1 = endpoint handle | timeout in milliseconds << IPC_TIMEOUT_SHIFT (0: wait without
// limit; 10 ms granularity). On expiry the call fails with ERR_TIMEOUT: a waiting send leaves the queue with its
// capability, a caller stops waiting and the server's later reply fails with ERR_PEER.
pub const IPC_TIMEOUT_SHIFT: usize = 32;
// Senders waiting on one endpoint; one more fails with ERR_BUSY at once (back-pressure).
pub const ENDPOINT_QUEUE: usize = 4;
// At the receiver: arg1=sender PID, msg[0]=1 if a capability was received, msg[1]=flags.
pub const MSG_FLAG_CALL: usize = 1;
pub const MSG_FLAG_IRQ: usize = 2;
pub const MSG_FLAG_EXIT: usize = 4; // exit notice of a watched task (TASK_WATCH)

pub const HEAP_PAGE_SIZE: usize = 4096; pub const HEAP_MAX_BLOCKS: usize = 32; pub const HEAP_MAX_BYTES: usize = 16 * 1024 * 1024;
// Separate quota for mapped foreign memory (frame, IPC buffers).
pub const SHARED_MAX_BYTES: usize = 48 * 1024 * 1024;

// RTC protocol: idl/rtc.wit (MIND IDL, bindings in mind::idl::rtc).
// VFS protocol: msg[2]=op|fd<<8|length<<16, msg[3]=offset; the buffer is passed as a memory capability.
pub const VFS_OPEN: usize = 1;
pub const VFS_READ: usize = 2;
pub const VFS_CLOSE: usize = 3;
pub const VFS_LIST: usize = 4;
pub const VFS_STAT: usize = 5;
// Program loader: idl/loader.wit (list, run with arguments). Legacy adapter until loader v1: CALL on SLOT_LOADER with
// the program name packed into msg[2..4] and an optional endpoint for the child's INIT slot; reply msg[2] = PID or error.
// init: CALL on SLOT_INIT with the service name in msg[2..4] starts that boot service; reply msg[2] = PID,
// ERR_BUSY if it is running, ERR_NOT_FOUND if there is no such service.

// SPAWN (requires the spawn privilege): arg1/arg2 = name, msg[0] = image memory capability or SPAWN_BOOT | boot image
// index (boot images need the platform privilege), msg[1] = ELF length, msg[2] = address of a Grant array,
// msg[3] = grant count | SPAWN_* flags << 8 | child task quota << 16 | child endpoint quota << 32. The quotas are taken
// from the spawner's (MC-3.13): a spawner's live children each reserve 1 + their task quota of its task quota, and their
// endpoint quotas plus the endpoints it created count against its endpoint quota. Each grant copies the spawner's capability into a child slot;
// endpoint rights are narrowed by the mask, reply capabilities are not transferable.
pub const SPAWN_BOOT: usize = 1 << 63;
pub const SPAWN_SERVICE: usize = 1; // system service (platform privilege only)
pub const SPAWN_SCREEN: usize = 2; // the task gets a screen buffer and can take the focus
pub const SPAWN_GRANTS_MAX: usize = 16;
// Program arguments: the SPAWN name buffer may be `name\0arguments`; the kernel copies the arguments into the child's
// read-only info page at ARGS_OFFSET as a u16 length followed by the bytes.
pub const ARGS_OFFSET: usize = 2048;
pub const ARGS_MAX: usize = 1024;
#[derive(Clone, Copy, Default)] #[repr(C)] pub struct Grant { pub own: u32, pub child: u8, pub rights: u8, pub flags: u16 }
pub const GRANT_MOVE: u16 = 1; // move the capability into the child instead of copying it

// PLATFORM_CAP: arg1 = kind, arg2 and msg[0] = arguments; result = new slot. The kernel validates every resource.
pub const PLATFORM_PORTS: usize = 2; // base, count: only legacy ranges from the platform profile
pub const PLATFORM_IRQ: usize = 3; // line 1..15 except the cascade (2)
pub const PLATFORM_DEVICE_BAR: usize = 4; // device index, BAR number: port range or MMIO
pub const PLATFORM_DEVICE_IRQ: usize = 5; // device index
pub const PLATFORM_FRAMEBUFFER: usize = 6;
pub const PLATFORM_DMA: usize = 7; // bytes; 64 KiB aligned, kept by the kernel for the platform's lifetime
pub const PLATFORM_PRIVILEGE: usize = 8; // CAP_KIND_INPUT, _DISPLAY, _SPAWN, _CONTROL or _RESTART
// DEVICE_FIND: arg1 = PCI class code (class<<16|subclass<<8|interface), arg2 = mask, msg[0] = n-th match; result = device index.

// TASK_LIST fills an array of TaskInfo (arg1 = address, arg2 = capacity) and returns the count.
#[derive(Clone, Copy)] #[repr(C)] pub struct TaskInfo { pub pid: u64, pub name: [u8; NAME_MAX], pub state: [u8; 8], pub cpu: u32, pub focus: u8, pub service: u8, pub screen: u8, pub reserved: u8, pub runs: u64, pub ticks: u64, pub calls: u64, pub quota_tasks: u16, pub used_tasks: u16, pub quota_endpoints: u16, pub used_endpoints: u16 }
// FAULTS fills an array of FaultInfo (arg1 = address, arg2 = capacity) and returns the count.
#[derive(Clone, Copy, Default)] #[repr(C)] pub struct FaultInfo { pub pid: u64, pub cpu: u64, pub vector: u64, pub error: u64, pub rip: u64, pub address: u64 }
// FOCUS: arg1 = PID (0 = the caller), arg2 = 1 to keep the task's buffered console output; result = PID.
// The caller becomes the focus owner: focus returns to it when the focused task exits or on an attention key.
// NOTICE: 0 if none, else PID | NOTICE_EXITED (the focused task exited) or PID (sent to the background).
pub const NOTICE_EXITED: usize = 1 << 63;

// Input events: one word per key press or release, queued per task (64, oldest dropped). The kernel stores and routes
// them; decoding and layouts live in ring 3 (ps2_kbd, the shell's UART decoder).
// bits 0-7: legacy byte for READ_KEY (raw scancode or UART byte; 0: none) · 8-23: key (KEY_*; 0: not decoded) ·
// 24-31: modifiers (MOD_*) · 32: pressed · 40-63: Unicode character from the active layout (0: none).
// INPUT_EVENT: arg1/arg2 = legacy bytes for an application / the focus owner, msg[0] = attention, msg[1]/msg[2] = full
// event words for them (0: an event carrying only the byte).
pub const KEY_CHAR: u16 = 1; // a key that produced `ch`
pub const KEY_ENTER: u16 = 2; pub const KEY_ESC: u16 = 3; pub const KEY_TAB: u16 = 4; pub const KEY_BACKSPACE: u16 = 5;
pub const KEY_UP: u16 = 6; pub const KEY_DOWN: u16 = 7; pub const KEY_LEFT: u16 = 8; pub const KEY_RIGHT: u16 = 9;
pub const KEY_HOME: u16 = 10; pub const KEY_END: u16 = 11; pub const KEY_PAGE_UP: u16 = 12; pub const KEY_PAGE_DOWN: u16 = 13;
pub const KEY_INSERT: u16 = 14; pub const KEY_DELETE: u16 = 15;
pub const KEY_F1: u16 = 16; // F1..F12 = 16..27
pub const KEY_SHIFT: u16 = 28; pub const KEY_CTRL: u16 = 29; pub const KEY_ALT: u16 = 30; pub const KEY_CAPS_LOCK: u16 = 31;
pub const MOD_SHIFT: u8 = 1; pub const MOD_CTRL: u8 = 2; pub const MOD_ALT: u8 = 4; pub const MOD_CAPS: u8 = 8;
pub const INPUT_QUEUE: usize = 64;
pub const fn input_event(byte: u8, key: u16, mods: u8, pressed: bool, ch: u32) -> usize {
    byte as usize | (key as usize) << 8 | (mods as usize) << 24 | (pressed as usize) << 32 | ((ch & 0xFF_FFFF) as usize) << 40
}
pub const fn event_byte(event: usize) -> u8 { event as u8 }
pub const fn event_key(event: usize) -> u16 { (event >> 8) as u16 }
pub const fn event_mods(event: usize) -> u8 { (event >> 24) as u8 }
pub const fn event_pressed(event: usize) -> bool { event >> 32 & 1 != 0 }
pub const fn event_char(event: usize) -> u32 { (event >> 40) as u32 }
// CONSOLE_READ / TASK_LOGS: arg1 = PID, msg[0] = buffer address, msg[1] = length; drains and returns the byte count.
// CPU_INFO: arg1 = CPU index; result = APIC id, arg2 = online, msg[2] = timer ticks. KERNEL_HEAP: result = used,
// arg2 = free, msg[2] = 1 if a test allocation was fully released.
// Block device protocol: idl/block.wit (bindings in mind::idl::block).
pub const BLOCK_SECTOR: usize = 512;
pub const BLOCK_MAX_SECTORS: usize = 128;
// Audio protocol: msg[2]=op|argument<<8, msg[3]=second argument.
pub const AUDIO_INFO: usize = 1;
pub const AUDIO_PLAY: usize = 2;
pub const AUDIO_TONE: usize = 3;
pub const AUDIO_STOP: usize = 4;
pub const AUDIO_WAIT: usize = 5; // the reply is deferred until the argument's number of buffers is free in the DMA ring
// Microphone (AC97 PCM in, 48 kHz stereo S16): START begins capture; READ copies completed buffers into the passed
// memory capability (argument = capacity in bytes), reply msg[2] = bytes, msg[3] = 1 if the ring overflowed; STOP ends it.
pub const AUDIO_RECORD_START: usize = 6;
pub const AUDIO_RECORD_READ: usize = 7;
pub const AUDIO_RECORD_STOP: usize = 8;
// Speech synthesis: capability to a page of UTF-8 text, msg[2]=TTS_SAY|length<<8, msg[3]=pitch Hz|rate %<<16 (0 for default).
pub const TTS_SAY: usize = 1;
pub const AUDIO_RATE: usize = 48_000;

// STAT (observe or control privilege): arg1 = class, arg2 = buffer, msg[0] = capacity in bytes, msg[1] = argument
// (a PID for VMAP and CAPS). The buffer receives a StatHeader and then up to (capacity - header) / record_size
// records; the result is the number written, `total` says how many exist. Copies are bounded by the kernel's tables.
// Nothing returned is authority: endpoint indices are labels no system call accepts, and no task memory contents or
// physical addresses of task memory are exported (MC-10.2).
pub const SYSCALL_STAT: usize = 51;
// SCHED_SET (the task's lifecycle owner, or process control): arg1 = PID, arg2 = budget in microseconds per period
// (0: no limit), msg[0] = period in microseconds (>= 10 000), msg[1] = band (BAND_*, or BAND_KEEP; changing the band
// needs process control or the platform/restart privilege). A task that spent its budget waits for its next period;
// a ready task of band 0 always runs before one of band 1 (MC-5.1-5.5). Budgets are enforced at the 10 ms tick.
pub const SYSCALL_SCHED_SET: usize = 52;
pub const BAND_SYSTEM: usize = 0; // init and services: their reserve survives application overload
pub const BAND_APPLICATION: usize = 1;
pub const BAND_KEEP: usize = 0xFF;
pub const STAT_VERSION: u32 = 1;
pub const STAT_TASKS: usize = 1;
pub const STAT_CPUS: usize = 2;
pub const STAT_MEMORY: usize = 3;
pub const STAT_PHYSMAP: usize = 4;
pub const STAT_VMAP: usize = 5;
pub const STAT_CAPS: usize = 6;
pub const STAT_ENDPOINTS: usize = 7;
pub const STAT_IRQS: usize = 8;
pub const STAT_DEVICES: usize = 9;
#[derive(Clone, Copy, Default, Debug)] #[repr(C)] pub struct StatHeader { pub version: u32, pub record_size: u32, pub count: u32, pub total: u32 }
// What a task waits for (StatTask.wait); wait_on is the endpoint index, IRQ line or the server's PID.
pub const WAIT_NONE: u8 = 0; pub const WAIT_SEND: u8 = 1; pub const WAIT_RECEIVE: u8 = 2; pub const WAIT_REPLY: u8 = 3;
pub const WAIT_SLEEP: u8 = 4; pub const WAIT_IRQ: u8 = 5; pub const WAIT_FLUSH: u8 = 6; pub const WAIT_EXITED: u8 = 7; pub const WAIT_RUNNING: u8 = 8;
#[derive(Clone, Copy, Default, Debug)] #[repr(C)] pub struct StatTask {
    pub pid: u64, pub parent: u64, pub name: [u8; NAME_MAX], pub wait: u8, pub cpu: u8, pub service: u8, pub screen: u8, pub wait_on: u32,
    pub run_ns: u64, pub runs: u64, pub ticks: u64, pub calls: u64, pub sends: u64, pub receives: u64, pub started_ns: u64,
    pub heap_bytes: u64, pub heap_blocks: u32, pub caps: u32, pub shared_bytes: u64, pub retained_bytes: u64,
    pub image_bytes: u64, pub stack_bytes: u64, pub screen_bytes: u64,
    pub quota_tasks: u16, pub used_tasks: u16, pub quota_endpoints: u16, pub used_endpoints: u16, pub band: u8, pub throttled: u8, pub reserved: u16,
    pub budget_ns: u64, pub period_ns: u64,
}
#[derive(Clone, Copy, Default, Debug)] #[repr(C)] pub struct StatCpu { pub apic_id: u32, pub online: u32, pub ticks: u64, pub busy_ns: u64, pub idle_ns: u64, pub interrupts: u64, pub switches: u64, pub current_pid: u64 }
// Kernel arena (bytes) by category, and the global limits.
#[derive(Clone, Copy, Default, Debug)] #[repr(C)] pub struct StatMemory {
    pub arena: u64, pub used: u64, pub free: u64, pub images: u64, pub stacks: u64, pub task_pages: u64, pub screens: u64,
    pub heaps: u64, pub objects: u64, pub dma: u64, pub dma_limit: u64, pub objects_limit: u64, pub tasks: u64, pub endpoints: u64,
}
// Physical layout: firmware memory map entries (kind = UEFI memory type) and the platform layout (kind >= PHYS_PLATFORM).
pub const PHYS_PLATFORM: u32 = 0x100; pub const PHYS_ARENA: u32 = 0x100; pub const PHYS_FRAMEBUFFER: u32 = 0x101;
pub const PHYS_BOOT_IMAGE: u32 = 0x102; pub const PHYS_AP_TRAMPOLINE: u32 = 0x103; pub const PHYS_PCI_BAR: u32 = 0x104;
#[derive(Clone, Copy, Default, Debug)] #[repr(C)] pub struct StatPhys { pub kind: u32, pub index: u32, pub start: u64, pub pages: u64 }
// Address-space regions of a task (VMAP).
pub const REGION_IMAGE: u32 = 1; pub const REGION_STACK: u32 = 2; pub const REGION_SCREEN: u32 = 3; pub const REGION_INFO: u32 = 4;
pub const REGION_MAILBOX: u32 = 5; pub const REGION_EXIT: u32 = 6; pub const REGION_HEAP: u32 = 7; pub const REGION_SHARED: u32 = 8; pub const REGION_DEVICE: u32 = 9;
pub const REGION_READ: u32 = 1; pub const REGION_WRITE: u32 = 2; pub const REGION_EXECUTE: u32 = 4;
#[derive(Clone, Copy, Default, Debug)] #[repr(C)] pub struct StatRegion { pub start: u64, pub size: u64, pub kind: u32, pub flags: u32 }
#[derive(Clone, Copy, Default, Debug)] #[repr(C)] pub struct StatCap { pub slot: u32, pub generation: u32, pub kind: u32, pub rights: u32, pub size: u64, pub badge: u32, pub reserved: u32, pub node: u64, pub parent: u64 }
#[derive(Clone, Copy, Default, Debug)] #[repr(C)] pub struct StatEndpoint { pub index: u32, pub receivers: u32, pub waiting_senders: u32, pub waiting_receivers: u32, pub creator: u64, pub messages: u64, pub busy: u64, pub timeouts: u64 }
#[derive(Clone, Copy, Default, Debug)] #[repr(C)] pub struct StatIrq { pub line: u32, pub endpoint: u32, pub masked: u32, pub reserved: u32, pub holder: u64, pub count: u64 }
#[derive(Clone, Copy, Default, Debug)] #[repr(C)] pub struct StatDevice { pub class: u32, pub irq: u32, pub bar_sizes: [u64; 6], pub holder: u64 }
