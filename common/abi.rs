#![allow(dead_code)]
// Единый ABI ядра: подключается ядром, загрузчиком и libmind (не копировать).

// Порядок образов задаёт загрузчик; системные сервисы ядро запускает при старте.
pub const PROGRAM_COUNT: usize = 13;
pub const PROGRAM_NAMES: [&str; PROGRAM_COUNT] = [
    "app", "app2", "clock", "dzen-clock", "ping", "pong", "files", "beep",
    "rtc", "ps2_kbd", "compositor", "vfs_server", "audio_gw",
];
pub const PROGRAM_FILES: [&str; PROGRAM_COUNT] = [
    "app.elf", "app2.elf", "clock.elf", "dzenclk.elf", "ping.elf", "pong.elf", "files.elf", "beep.elf",
    "rtc.elf", "ps2_kbd.elf", "compositor.elf", "vfs_server.elf", "audio_gw.elf",
];
pub const BOOT_SERVICES: [&str; 5] = ["rtc", "ps2_kbd", "compositor", "vfs_server", "audio_gw"];

#[derive(Clone, Copy)] #[repr(C)] pub struct ProgramImage { pub data: *const u8, pub len: usize }
#[derive(Clone, Copy)] #[repr(C)] pub struct BootInfo { pub fb_ptr: *mut u32, pub width: usize, pub height: usize, pub stride: usize, pub programs: [ProgramImage; PROGRAM_COUNT], pub heap_ptr: *mut u8, pub heap_len: usize, pub ap_trampoline: usize, pub cpu_count: usize, pub apic_ids: [u32; 8], }
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

// Ответ CAP_INFO: result=вид мандата, arg2=база/адрес, msg[2]=размер/число портов/права.
pub const CAP_KIND_NONE: usize = 0;
pub const CAP_KIND_ENDPOINT: usize = 1;
pub const CAP_KIND_MEMORY: usize = 2;
pub const CAP_KIND_DMA: usize = 3;
pub const CAP_KIND_PORTS: usize = 4;
pub const CAP_KIND_IRQ: usize = 5;
pub const CAP_KIND_INPUT: usize = 6;
pub const CAP_KIND_DISPLAY: usize = 7;

// Коды ошибок: usize::MAX - n. ALLOC по-прежнему возвращает 0 при отказе.
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

// Слоты мандатов приложения, которые ядро заполняет при запуске.
pub const SLOT_INIT: usize = 1;
pub const SLOT_RTC: usize = 2;
pub const SLOT_VFS: usize = 3;
pub const SLOT_AUDIO: usize = 4;
// Слоты мандатов сервиса: обслуживаемая точка, устройства, IRQ, DMA/кадр, привилегия.
pub const SLOT_SERVICE: usize = 1;
pub const SLOT_DEV0: usize = 2;
pub const SLOT_DEV1: usize = 3;
pub const SLOT_IRQ: usize = 4;
pub const SLOT_MEM: usize = 5;
pub const SLOT_PRIV: usize = 6;
// Новые мандаты ядро выдаёт начиная с этого слота.
pub const SLOT_DYNAMIC: usize = 8;

// Зарезервированные номера точек IPC системных сервисов.
pub const EP_RTC: usize = 2;
pub const EP_VFS: usize = 3;
pub const EP_AUDIO: usize = 4;
pub const EP_RESERVED: usize = 8;

// Сообщение: msg[0]=слот передаваемого мандата, msg[1]=маска прав, msg[2..4]=данные.
// У получателя: arg1=PID отправителя, msg[0]=1 если мандат получен, msg[1]=флаги.
pub const MSG_FLAG_CALL: usize = 1;
pub const MSG_FLAG_IRQ: usize = 2;

pub const HEAP_PAGE_SIZE: usize = 4096; pub const HEAP_MAX_BLOCKS: usize = 32; pub const HEAP_MAX_BYTES: usize = 16 * 1024 * 1024;
// Отдельная квота для отображённой чужой памяти (кадр, буферы IPC).
pub const SHARED_MAX_BYTES: usize = 48 * 1024 * 1024;

// Протокол RTC: CALL без данных, ответ msg[2]=секунды от полуночи или RTC_UNAVAILABLE.
// Протокол VFS: msg[2]=операция|fd<<8|длина<<16, msg[3]=смещение; буфер передаётся мандатом памяти.
pub const VFS_OPEN: usize = 1;
pub const VFS_READ: usize = 2;
pub const VFS_CLOSE: usize = 3;
pub const VFS_LIST: usize = 4;
pub const VFS_STAT: usize = 5;
// Протокол аудио: msg[2]=операция|аргумент<<8, msg[3]=второй аргумент.
pub const AUDIO_INFO: usize = 1;
pub const AUDIO_PLAY: usize = 2;
pub const AUDIO_TONE: usize = 3;
pub const AUDIO_STOP: usize = 4;
pub const AUDIO_RATE: usize = 48_000;
