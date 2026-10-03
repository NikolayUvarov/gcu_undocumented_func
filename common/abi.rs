#![allow(dead_code)]
// Single kernel ABI: included by the kernel, bootloader and libmind (do not copy).

// The UEFI bootloader passes the kernel only system service images; the loader service reads applications from disk.
// The kernel starts only image 0 (`init`); init decides which of the others to start and what each one receives.
pub const BOOT_IMAGES: usize = 12;
pub const BOOT_SERVICES: [&str; BOOT_IMAGES] = ["init", "rtc", "ps2_kbd", "compositor", "ata", "ahci", "usb_storage", "vfs_server", "loader", "audio_gw", "tts", "shell"];
pub const BOOT_FILES: [&str; BOOT_IMAGES] = ["init.elf", "rtc.elf", "ps2_kbd.elf", "compositor.elf", "ata.elf", "ahci.elf", "usb_storage.elf", "vfs_server.elf", "loader.elf", "audio_gw.elf", "tts.elf", "shell.elf"];
pub const MAX_APPS: usize = 8; // applications (non-service tasks) running at once
pub const NAME_MAX: usize = 16; // task name in ps and in spawn requests

#[derive(Clone, Copy)] #[repr(C)] pub struct ProgramImage { pub data: *const u8, pub len: usize }
#[derive(Clone, Copy)] #[repr(C)] pub struct BootInfo { pub fb_ptr: *mut u32, pub width: usize, pub height: usize, pub stride: usize, pub programs: [ProgramImage; BOOT_IMAGES], pub heap_ptr: *mut u8, pub heap_len: usize, pub ap_trampoline: usize, pub cpu_count: usize, pub apic_ids: [u32; 8], }
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

// CAP_INFO reply: result=capability kind, arg2=base/address, msg[2]=size/port count/rights.
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

// Error codes: usize::MAX - n. ALLOC still returns 0 on failure.
pub const ERR_INVALID: usize = usize::MAX;
pub const ERR_NO_SLOT: usize = usize::MAX - 1;
pub const ERR_RIGHTS: usize = usize::MAX - 2;
pub const ERR_NOT_FOUND: usize = usize::MAX - 3;
pub const ERR_PEER: usize = usize::MAX - 4;
pub const ERR_NO_MEMORY: usize = usize::MAX - 5;
pub const ERR_BUSY: usize = usize::MAX - 6; // e.g. the service is already running
pub const ERR_LIMIT: usize = usize::MAX - 7; // task limit reached
pub const ERR_FIRST: usize = usize::MAX - 15;
pub const RTC_UNAVAILABLE: usize = usize::MAX;

pub const CAP_READ: u8 = 1 << 0; pub const CAP_WRITE: u8 = 1 << 1; pub const CAP_GRANT: u8 = 1 << 2;
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
// The kernel hands out new capabilities starting from this slot; slots below it are fixed by convention.
pub const SLOT_DYNAMIC: usize = 10;

// Reserved IPC endpoint numbers of system services.
pub const EP_RTC: usize = 2;
pub const EP_VFS: usize = 3;
pub const EP_AUDIO: usize = 4;
pub const EP_BLOCK_ATA: usize = 5;
pub const EP_BLOCK_AHCI: usize = 6;
pub const EP_BLOCK_USB: usize = 7;
pub const EP_LOADER: usize = 8;
pub const EP_TTS: usize = 9;
pub const EP_INIT: usize = 10;
pub const EP_RESERVED: usize = 16;

// Message: msg[0]=slot of the capability to transfer, msg[1]=rights mask, msg[2..4]=data.
// At the receiver: arg1=sender PID, msg[0]=1 if a capability was received, msg[1]=flags.
pub const MSG_FLAG_CALL: usize = 1;
pub const MSG_FLAG_IRQ: usize = 2;

pub const HEAP_PAGE_SIZE: usize = 4096; pub const HEAP_MAX_BLOCKS: usize = 32; pub const HEAP_MAX_BYTES: usize = 16 * 1024 * 1024;
// Separate quota for mapped foreign memory (frame, IPC buffers).
pub const SHARED_MAX_BYTES: usize = 48 * 1024 * 1024;

// RTC protocol: CALL with no data, reply msg[2]=seconds since midnight or RTC_UNAVAILABLE.
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
// init: CALL on EP_INIT with the service name in msg[2..4] starts that boot service; reply msg[2] = PID,
// ERR_BUSY if it is running, ERR_NOT_FOUND if there is no such service.

// SPAWN (requires the spawn privilege): arg1/arg2 = name, msg[0] = image memory capability or SPAWN_BOOT | boot image
// index (boot images need the platform privilege), msg[1] = ELF length, msg[2] = address of a Grant array,
// msg[3] = grant count | SPAWN_* flags << 8. Each grant copies the spawner's capability into a child slot;
// endpoint rights are narrowed by the mask, reply capabilities are not transferable.
pub const SPAWN_BOOT: usize = 1 << 63;
pub const SPAWN_SERVICE: usize = 1; // single instance, not counted in MAX_APPS (platform privilege only)
pub const SPAWN_SCREEN: usize = 2; // the task gets a screen buffer and can take the focus
pub const SPAWN_GRANTS_MAX: usize = 16;
// Program arguments: the SPAWN name buffer may be `name\0arguments`; the kernel copies the arguments into the child's
// read-only info page at ARGS_OFFSET as a u16 length followed by the bytes.
pub const ARGS_OFFSET: usize = 2048;
pub const ARGS_MAX: usize = 1024;
#[derive(Clone, Copy, Default)] #[repr(C)] pub struct Grant { pub child: u8, pub own: u8, pub rights: u8, pub reserved: u8 }

// PLATFORM_CAP: arg1 = kind, arg2 and msg[0] = arguments; result = new slot. The kernel validates every resource.
pub const PLATFORM_ENDPOINT: usize = 1; // reserved endpoint number, all rights
pub const PLATFORM_PORTS: usize = 2; // base, count: only legacy ranges from the platform profile
pub const PLATFORM_IRQ: usize = 3; // line 1..15 except the cascade (2)
pub const PLATFORM_DEVICE_BAR: usize = 4; // device index, BAR number: port range or MMIO
pub const PLATFORM_DEVICE_IRQ: usize = 5; // device index
pub const PLATFORM_FRAMEBUFFER: usize = 6;
pub const PLATFORM_DMA: usize = 7; // bytes; 64 KiB aligned, kept by the kernel for the platform's lifetime
pub const PLATFORM_PRIVILEGE: usize = 8; // CAP_KIND_INPUT, _DISPLAY, _SPAWN or _CONTROL
// DEVICE_FIND: arg1 = PCI class code (class<<16|subclass<<8|interface), arg2 = mask, msg[0] = n-th match; result = device index.

// TASK_LIST fills an array of TaskInfo (arg1 = address, arg2 = capacity) and returns the count.
#[derive(Clone, Copy)] #[repr(C)] pub struct TaskInfo { pub pid: u64, pub name: [u8; NAME_MAX], pub state: [u8; 8], pub cpu: u32, pub focus: u8, pub service: u8, pub screen: u8, pub reserved: u8, pub runs: u64, pub ticks: u64, pub calls: u64 }
// FAULTS fills an array of FaultInfo (arg1 = address, arg2 = capacity) and returns the count.
#[derive(Clone, Copy, Default)] #[repr(C)] pub struct FaultInfo { pub pid: u64, pub cpu: u64, pub vector: u64, pub error: u64, pub rip: u64, pub address: u64 }
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
