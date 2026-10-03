#![allow(dead_code)]
// Single kernel ABI: included by the kernel, bootloader and libmind (do not copy).

// The UEFI bootloader passes the kernel only system service images; the loader service reads applications from disk.
// The ahci and usb_storage drivers are started only if their controller is present on the PCI bus.
pub const BOOT_IMAGES: usize = 10;
pub const BOOT_SERVICES: [&str; BOOT_IMAGES] = ["rtc", "ps2_kbd", "compositor", "ata", "ahci", "usb_storage", "vfs_server", "loader", "audio_gw", "tts"];
pub const BOOT_FILES: [&str; BOOT_IMAGES] = ["rtc.elf", "ps2_kbd.elf", "compositor.elf", "ata.elf", "ahci.elf", "usb_storage.elf", "vfs_server.elf", "loader.elf", "audio_gw.elf", "tts.elf"];
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
pub const SYSCALL_SPAWN_IMAGE: usize = 13;
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
pub const SYSCALL_LOADER_DONE: usize = 30;
pub const SYSCALL_IPC_SAVE_REPLY: usize = 31;

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

// Error codes: usize::MAX - n. ALLOC still returns 0 on failure.
pub const ERR_INVALID: usize = usize::MAX;
pub const ERR_NO_SLOT: usize = usize::MAX - 1;
pub const ERR_RIGHTS: usize = usize::MAX - 2;
pub const ERR_NOT_FOUND: usize = usize::MAX - 3;
pub const ERR_PEER: usize = usize::MAX - 4;
pub const ERR_NO_MEMORY: usize = usize::MAX - 5;
pub const ERR_FIRST: usize = usize::MAX - 15;
pub const RTC_UNAVAILABLE: usize = usize::MAX;

pub const CAP_READ: u8 = 1 << 0; pub const CAP_WRITE: u8 = 1 << 1; pub const CAP_GRANT: u8 = 1 << 2;
pub const CAP_SLOTS: usize = 32;

// Application capability slots filled by the kernel at spawn.
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
// The kernel hands out new capabilities starting from this slot.
pub const SLOT_DYNAMIC: usize = 8;

// Reserved IPC endpoint numbers of system services.
pub const EP_RTC: usize = 2;
pub const EP_VFS: usize = 3;
pub const EP_AUDIO: usize = 4;
pub const EP_BLOCK_ATA: usize = 5;
pub const EP_BLOCK_AHCI: usize = 6;
pub const EP_BLOCK_USB: usize = 7;
pub const EP_LOADER: usize = 8;
pub const EP_TTS: usize = 9;
pub const EP_RESERVED: usize = 16;

// Message: msg[0]=slot of the capability to transfer, msg[1]=rights mask, msg[2..4]=data.
// At the receiver: arg1=sender PID, msg[0]=1 if a capability was received, msg[1]=flags.
pub const MSG_FLAG_CALL: usize = 1;
pub const MSG_FLAG_IRQ: usize = 2;
pub const MSG_FLAG_KERNEL: usize = 4; // request from the kernel shell (msg[2] is the request number)

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
// Program loader. The shell request lives in the request page (capability in loader's SLOT_MEM):
// [kind, background, name length, name...]; the LIST reply is text at offset LOADER_REPLY.
// Applications request a spawn via CALL on SLOT_LOADER: msg[2..4] is the name (up to 16 bytes), the capability is an endpoint for the child.
pub const LOADER_RUN: u8 = 1;
pub const LOADER_LIST: u8 = 2;
pub const LOADER_REPLY: usize = 512;
// SPAWN_IMAGE (requires the spawn privilege): arg1/arg2 is the name, msg[0] the image capability, msg[1] the ELF length,
// msg[2] the endpoint capability for the child's INIT slot (0 for none), msg[3] the rights mask | shell request number << 16.
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
// Speech synthesis: capability to a page of UTF-8 text, msg[2]=TTS_SAY|length<<8, msg[3]=pitch Hz|rate %<<16 (0 for default).
pub const TTS_SAY: usize = 1;
pub const AUDIO_RATE: usize = 48_000;
