#![allow(dead_code)]
// Single kernel ABI: included by the kernel, bootloader and libmind (do not copy).

// The UEFI bootloader passes the kernel only system service images; the loader service reads applications from disk.
// The kernel starts only image 0 (`init`); init decides which of the others to start and what each one receives.
pub const BOOT_IMAGES: usize = 13;
pub const BOOT_SERVICES: [&str; BOOT_IMAGES] = ["init", "rtc", "ps2_kbd", "compositor", "ata", "ahci", "usb_storage", "vfs_server", "loader", "audio_gw", "tts", "sysmon", "shell"];
pub const BOOT_FILES: [&str; BOOT_IMAGES] = ["init.elf", "rtc.elf", "ps2_kbd.elf", "compositor.elf", "ata.elf", "ahci.elf", "usb_storage.elf", "vfs_server.elf", "loader.elf", "audio_gw.elf", "tts.elf", "sysmon.elf", "shell.elf"];
pub const MAX_APPS: usize = 8; // init's policy: live applications loader may start (its task quota)
pub const NAME_MAX: usize = 16; // task name in ps and in spawn requests

#[derive(Clone, Copy)] #[repr(C)] pub struct ProgramImage { pub data: *const u8, pub len: usize }
// Firmware memory map entry handed over by the bootloader (UEFI memory type, physical start, 4 KiB pages).
#[derive(Clone, Copy, Default)] #[repr(C)] pub struct MemoryRange { pub start: u64, pub pages: u64, pub kind: u32, pub reserved: u32 }
pub const MEMORY_MAP_MAX: usize = 170; // one page of entries after the BootInfo page
#[derive(Clone, Copy)] #[repr(C)] pub struct BootInfo { pub fb_ptr: *mut u32, pub width: usize, pub height: usize, pub stride: usize, pub programs: [ProgramImage; BOOT_IMAGES], pub heap_ptr: *mut u8, pub heap_len: usize, pub ap_trampoline: usize, pub cpu_count: usize, pub apic_ids: [u32; 8], pub memory_map: *const MemoryRange, pub memory_map_len: usize, }
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
// CAP_MINT: arg1 = handle, arg2 = rights mask (endpoints and memory), msg[0] = offset, msg[1] = length (0: to the end) for port and
// memory ranges -> handle of a child with no more authority. CAP_REVOKE: arg1 = handle -> number of descendants removed
// from all tasks; the capability itself stays (MC-3.4-3.6).
pub const SYSCALL_CAP_MINT: usize = 45;
pub const SYSCALL_CAP_REVOKE: usize = 46;
// MEM_DETACH: arg1 = start of a heap block no one else refers to -> handle of a memory object (read/write, no grant).
// The block leaves the caller's address space; the object lives while a capability or mapping refers to it. Copying a
// writable memory capability needs CAP_GRANT, so an object can only be moved (MOVE: one owner) or minted read-only.
pub const SYSCALL_MEM_DETACH: usize = 47;
// STAT (observation, MC-10.2): arg1 = class (STAT_*), arg2 = argument (a PID for STAT_VMAP and STAT_CAPS), msg[0] = buffer
// address, msg[1] = capacity in bytes -> number of records written after a StatHeader. Needs the observe or the
// process-control privilege. Records describe kernel objects; they never contain memory contents or physical addresses
// of task memory, and nothing in them can be used as an authority.
pub const SYSCALL_STAT: usize = 48;
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
pub const CAP_KIND_OBSERVE: usize = 13; // read-only statistics (STAT, TASK_LIST, CPU_INFO, KERNEL_HEAP, FAULTS)

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
pub const CAP_SLOTS: usize = 32;

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
// Capabilities a launcher grants on request (the shell holds them; loader v1 passes them on): system information
// from sysmon, and (later) service lifecycle control.
pub const SLOT_SYSINFO: usize = 10;
pub const SLOT_LIFECYCLE: usize = 11;
// The kernel hands out new capabilities starting from this slot; slots below it are fixed by convention.
pub const SLOT_DYNAMIC: usize = 12;
// A capability handle is `slot | generation << HANDLE_GENERATION_SHIFT`. Fixed slots (below SLOT_DYNAMIC) are named with
// generation 0; a slot the kernel hands out gets a new generation every time it is freed, so an old handle stays invalid.
// Received capabilities and the compositor's screen are placed only in fixed slots.
pub const HANDLE_SLOT_MASK: usize = 0xFF;
pub const HANDLE_GENERATION_SHIFT: usize = 8;

// Endpoints have no global names: every one is created by ENDPOINT_CREATE (init's own by the kernel) and reached only
// through capabilities (MC-3.3).
// Input events (READ_KEY, INPUT_EVENT): one 32-bit word per key press. Bits 0-20: Unicode character (0 if none);
// bits 21-27: key code (KEY_*, 0 for a plain character); bits 28-30: Shift, Ctrl, Alt. Enter, Esc, Tab and Backspace
// also carry their control character ('\n', 0x1B, '\t', 0x08); Ctrl or Alt + a key carries the key's US character.
// Decoding (scan codes, layouts, terminal sequences) is done in ring 3 by ps2_kbd and the shell (libmind::keys).
pub const KEY_CHAR_MASK: u32 = 0x1F_FFFF;
pub const KEY_CODE_SHIFT: u32 = 21;
pub const KEY_CODE_MASK: u32 = 0x7F;
pub const KEY_ENTER: u32 = 1; pub const KEY_ESC: u32 = 2; pub const KEY_BACKSPACE: u32 = 3; pub const KEY_TAB: u32 = 4;
pub const KEY_UP: u32 = 5; pub const KEY_DOWN: u32 = 6; pub const KEY_LEFT: u32 = 7; pub const KEY_RIGHT: u32 = 8;
pub const KEY_HOME: u32 = 9; pub const KEY_END: u32 = 10; pub const KEY_PGUP: u32 = 11; pub const KEY_PGDN: u32 = 12;
pub const KEY_INSERT: u32 = 13; pub const KEY_DELETE: u32 = 14;
pub const KEY_F1: u32 = 15; pub const KEY_F6: u32 = 20; pub const KEY_F11: u32 = 25; pub const KEY_F12: u32 = 26; // F1..F12 = 15..26
pub const KEY_MOD_SHIFT: u32 = 1 << 28; pub const KEY_MOD_CTRL: u32 = 1 << 29; pub const KEY_MOD_ALT: u32 = 1 << 30;
pub const INPUT_QUEUE: usize = 64; // events per task; the oldest is dropped when full
// Block device kinds reported by BLOCK_INFO (protocol data, not authority).
pub const BLOCK_KIND_ATA: usize = 1;
pub const BLOCK_KIND_AHCI: usize = 2;
pub const BLOCK_KIND_USB: usize = 3;

// Message: msg[0]=handle of the capability to transfer, msg[1]=rights mask | CAP_TRANSFER_MOVE, msg[2..4]=data.
// A transfer is a copy (a child the sender can revoke) unless CAP_TRANSFER_MOVE moves it out of the sender's table.
pub const CAP_TRANSFER_MOVE: usize = 1 << 8;
// IPC_SEND, IPC_CALL, IPC_RECV: arg1 = endpoint handle | timeout in milliseconds << IPC_TIMEOUT_SHIFT (0: wait without
// limit; 10 ms granularity). On expiry the call fails with ERR_TIMEOUT: a waiting send leaves the queue with its
// capability, a caller stops waiting and the server's later reply fails with ERR_PEER.
pub const IPC_TIMEOUT_SHIFT: usize = 32;
// Senders waiting on one endpoint; one more fails with ERR_BUSY at once (back-pressure).
pub const ENDPOINT_QUEUE: usize = 8;
// At the receiver: arg1=sender PID, msg[0]=1 if a capability was received, msg[1]=flags.
pub const MSG_FLAG_CALL: usize = 1;
pub const MSG_FLAG_IRQ: usize = 2;

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
// Program loader: CALL on SLOT_LOADER. msg[2..4] is the program name (up to 16 bytes) and the optional capability
// is an endpoint for the child's INIT slot; reply msg[2] = PID or error. With msg[2] = 0 and msg[3] = LOADER_LIST
// the capability is a memory page: the loader writes the program list there and replies with its length.
pub const LOADER_LIST: usize = 2;
// With msg[2] = 0 and msg[3] = LOADER_RUN the capability is a memory page with `name\0arguments\0`: start with arguments.
pub const LOADER_RUN: usize = 1;
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
pub const PLATFORM_PRIVILEGE: usize = 8; // CAP_KIND_INPUT, _DISPLAY, _SPAWN, _CONTROL or _OBSERVE
// DEVICE_FIND: arg1 = PCI class code (class<<16|subclass<<8|interface), arg2 = mask, msg[0] = n-th match; result = device index.

// TASK_LIST fills an array of TaskInfo (arg1 = address, arg2 = capacity) and returns the count.
#[derive(Clone, Copy)] #[repr(C)] pub struct TaskInfo { pub pid: u64, pub name: [u8; NAME_MAX], pub state: [u8; 8], pub cpu: u32, pub focus: u8, pub service: u8, pub screen: u8, pub reserved: u8, pub runs: u64, pub ticks: u64, pub calls: u64 }
// FAULTS fills an array of FaultInfo (arg1 = address, arg2 = capacity) and returns the count.
#[derive(Clone, Copy, Default)] #[repr(C)] pub struct FaultInfo { pub pid: u64, pub cpu: u64, pub vector: u64, pub error: u64, pub rip: u64, pub address: u64 }
// STAT classes and records (version STAT_VERSION; a reader checks `record_size`).
pub const STAT_VERSION: u32 = 1;
pub const STAT_TASKS: usize = 1; // TaskStat per task
pub const STAT_CPUS: usize = 2; // CpuStat per CPU
pub const STAT_MEMORY: usize = 3; // one MemoryStat; argument 1 also finds the largest free block (by trial allocations)
pub const STAT_PHYSMAP: usize = 4; // PhysRange: the firmware memory map, then the platform layout (kind >= PHYS_LAYOUT)
pub const STAT_VMAP: usize = 5; // VmRegion per region of the address space of task arg2
pub const STAT_CAPS: usize = 6; // CapStat per occupied slot of task arg2
pub const STAT_ENDPOINTS: usize = 7; // EndpointStat per live endpoint
pub const STAT_IRQS: usize = 8; // IrqStat per line 1..15
pub const STAT_DEVICES: usize = 9; // DeviceStat per PCI function
#[derive(Clone, Copy, Default, Debug)] #[repr(C)] pub struct StatHeader { pub version: u32, pub record_size: u32, pub count: u32, pub total: u32 }
// Task states in TaskStat.state; `wait` names what the task waits for (endpoint index, PID, IRQ line, deadline ms).
pub const TASK_READY: u8 = 1; pub const TASK_RUNNING: u8 = 2; pub const TASK_SLEEPING: u8 = 3; pub const TASK_SEND: u8 = 4;
pub const TASK_RECV: u8 = 5; pub const TASK_REPLY: u8 = 6; pub const TASK_IRQ: u8 = 7; pub const TASK_FLUSH: u8 = 8; pub const TASK_EXITED: u8 = 9;
pub const TASK_FLAG_SERVICE: u8 = 1; pub const TASK_FLAG_SCREEN: u8 = 2; pub const TASK_FLAG_FOCUS: u8 = 4;
#[derive(Clone, Copy, Default, Debug)] #[repr(C)]
pub struct TaskStat {
    pub pid: u64, pub parent: u64, pub run_ns: u64, pub runs: u64, pub ticks: u64, pub calls: u64, pub sent: u64, pub received: u64,
    pub started_ns: u64, pub image_bytes: u64, pub stack_bytes: u64, pub screen_bytes: u64, pub heap_bytes: u64, pub shared_bytes: u64,
    pub kernel_bytes: u64, // context, mailbox, info and exit pages, page tables
    pub wait: u64, pub heap_blocks: u32, pub caps: u32, pub quota_tasks: u32, pub used_tasks: u32, pub quota_endpoints: u32, pub used_endpoints: u32,
    pub name: [u8; NAME_MAX], pub state: u8, pub cpu: u8, pub flags: u8, pub reserved: [u8; 5],
}
#[derive(Clone, Copy, Default, Debug)] #[repr(C)]
pub struct CpuStat { pub busy_ns: u64, pub idle_ns: u64, pub ticks: u64, pub switches: u64, pub interrupts: u64, pub current: u64, pub apic: u32, pub online: u32 }
#[derive(Clone, Copy, Default, Debug)] #[repr(C)]
pub struct MemoryStat {
    pub arena_bytes: u64, pub arena_used: u64, pub arena_free: u64, pub largest_free: u64,
    pub task_images: u64, pub task_stacks: u64, pub task_screens: u64, pub task_heaps: u64, pub task_kernel: u64, pub page_tables: u64,
    pub objects: u64, pub objects_limit: u64, pub dma: u64, pub dma_limit: u64, pub shared_mapped: u64, pub kernel_other: u64,
    pub tasks: u32, pub tasks_limit: u32, pub endpoints: u32, pub endpoints_limit: u32,
}
// PhysRange.kind: 0..15 are UEFI memory types (7 = conventional memory); from PHYS_LAYOUT on, the platform layout.
pub const PHYS_LAYOUT: u32 = 16;
pub const PHYS_KERNEL: u32 = 16; pub const PHYS_HEAP: u32 = 17; pub const PHYS_BOOT_IMAGE: u32 = 18; pub const PHYS_FRAMEBUFFER: u32 = 19;
pub const PHYS_TRAMPOLINE: u32 = 20; pub const PHYS_DEVICE: u32 = 21; pub const PHYS_DMA: u32 = 22;
#[derive(Clone, Copy, Default, Debug)] #[repr(C)]
pub struct PhysRange { pub start: u64, pub bytes: u64, pub kind: u32, pub detail: u32 } // detail: boot image or device index
// VmRegion.kind and flags (R/W/X; SHARED: memory of another owner, DEVICE: registers).
pub const VM_CODE: u32 = 1; pub const VM_DATA: u32 = 2; pub const VM_STACK: u32 = 3; pub const VM_GUARD: u32 = 4; pub const VM_SCREEN: u32 = 5;
pub const VM_INFO: u32 = 6; pub const VM_MAILBOX: u32 = 7; pub const VM_EXIT: u32 = 8; pub const VM_HEAP: u32 = 9; pub const VM_SHARED: u32 = 10; pub const VM_DEVICE: u32 = 11;
pub const VM_READ: u32 = 1; pub const VM_WRITE: u32 = 2; pub const VM_EXEC: u32 = 4;
#[derive(Clone, Copy, Default, Debug)] #[repr(C)]
pub struct VmRegion { pub start: u64, pub bytes: u64, pub kind: u32, pub flags: u32 }
#[derive(Clone, Copy, Default, Debug)] #[repr(C)]
pub struct CapStat {
    pub node: u64, pub parent: u64, pub size: u64, // memory/DMA/MMIO bytes or port count
    pub base: u64, // port base or IRQ line; never a physical address
    pub slot: u32, pub generation: u32, pub kind: u32, pub rights: u32, pub endpoint: u32, pub reserved: u32,
}
#[derive(Clone, Copy, Default, Debug)] #[repr(C)]
pub struct EndpointStat {
    pub messages: u64, pub busy: u64, pub timeouts: u64,
    pub index: u32, // observation label: no system call accepts it
    pub creator: u32, pub server: u32, pub receivers: u32, pub holders: u32, pub waiting: u32, pub receiving: u32, pub irq: u32,
}
#[derive(Clone, Copy, Default, Debug)] #[repr(C)]
pub struct IrqStat { pub count: u64, pub line: u32, pub holder: u32, pub endpoint: u32, pub masked: u32 }
#[derive(Clone, Copy, Default, Debug)] #[repr(C)]
pub struct DeviceStat { pub bar_bytes: [u64; 6], pub class: u32, pub irq: u32, pub holder: u32, pub index: u32, pub location: u32, pub io_bars: u32 }

// FOCUS: arg1 = PID (0 = the caller), arg2 = 1 to keep the task's buffered console output; result = PID.
// The caller becomes the focus owner: focus returns to it when the focused task exits or on an attention key.
// NOTICE: 0 if none, else PID | NOTICE_EXITED (the focused task exited) or PID (sent to the background).
pub const NOTICE_EXITED: usize = 1 << 63;
// CONSOLE_READ / TASK_LOGS: arg1 = PID, msg[0] = buffer address, msg[1] = length; drains and returns the byte count.
// CPU_INFO: arg1 = CPU index; result = APIC id, arg2 = online, msg[2] = timer ticks. KERNEL_HEAP: result = used,
// arg2 = free, msg[2] = 1 if a test allocation was fully released.
// Block device protocol: msg[2]=op|sector count<<8, msg[3]=LBA.
// ATTACH passes the client's buffer capability (up to BLOCK_MAX_SECTORS sectors), READ fills it.
pub const BLOCK_INFO: usize = 1;
pub const BLOCK_ATTACH: usize = 2;
pub const BLOCK_READ: usize = 3;
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
