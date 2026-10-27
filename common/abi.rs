#![allow(dead_code)]
// Single kernel ABI: included by the kernel, bootloader and libmind (do not copy).

// The UEFI bootloader passes the kernel only system service images; the loader service reads applications from disk.
// The kernel starts only image 0 (`init`); init decides which of the others to start and what each one receives.
pub const BOOT_IMAGES: usize = 44;
// ABI 10's image array stays in the fixed prefix; later images are appended after all ABI 10 fields.
pub const BOOT_PREFIX_IMAGES: usize = 36;
pub const BOOT_SERVICES: [&str; BOOT_IMAGES] = ["init", "logd", "rtc", "ps2_kbd", "virtio_input", "compositor", "ata", "ahci", "usb_host", "usb_storage", "usb_hid", "virtio_blk", "nvme", "ramdisk", "vfs_server", "usb_serial", "link", "blockstore", "gpio", "battery", "loader", "audio_gw", "tts", "video_gw", "virtio_net", "rtl8168", "rtw89_wifi", "rtw88_wifi", "bcm_wifi", "tpm", "keystore", "wlansim", "wlan", "netstack", "netpolicy", "parse", "tls", "ssh", "windows", "sysmon", "updater", "effector_gw", "effector", "shell"];
pub const BOOT_FILES: [&str; BOOT_IMAGES] = ["init.elf", "logd.elf", "rtc.elf", "ps2_kbd.elf", "virtio_input.elf", "compositor.elf", "ata.elf", "ahci.elf", "usb_host.elf", "usb_storage.elf", "usb_hid.elf", "virtio_blk.elf", "nvme.elf", "ramdisk.elf", "vfs_server.elf", "usb_serial.elf", "link.elf", "blockstore.elf", "gpio.elf", "battery.elf", "loader.elf", "audio_gw.elf", "tts.elf", "video_gw.elf", "virtio_net.elf", "rtl8168.elf", "rtw89_wifi.elf", "rtw88_wifi.elf", "bcm_wifi.elf", "tpm.elf", "keystore.elf", "wlansim.elf", "wlan.elf", "netstack.elf", "netpolicy.elf", "parse.elf", "tls.elf", "ssh.elf", "windows.elf", "sysmon.elf", "updater.elf", "effector_gw.elf", "effector.elf", "shell.elf"];
// Further instances of a boot image, one per device (issue 105): `<image>#<n>` runs image `<image>` for its n-th device.
// init starts each right after the image's first instance; netstack holds the network card drivers in slots 2 and 3.
pub const SERVICE_INSTANCES: [&str; 4] = ["virtio_net#1", "ramdisk#1", "virtio_blk#1", "virtio_blk#2"]; // ramdisk#1: the block store's disk without one of its own (300-KRN-0001); virtio_blk#1, #2: the second and third VirtIO disks (300-KRN-0025, 251-KRN-0031: boot, models and store on aarch64)
// The largest task or endpoint quota SPAWN can delegate (16 bits): the root quota init holds (issue 171).
pub const QUOTA_MAX: usize = 0xFFFF;
pub const NAME_MAX: usize = 16; // task name in ps and in spawn requests

#[derive(Clone, Copy)] #[repr(C)] pub struct ProgramImage { pub data: *const u8, pub len: usize }
// Framebuffer: `stride` pixels per line, 4 bytes per pixel, in `pixel_format`. Screens of tasks always hold 0x00RRGGBB
// (PIXEL_BGR in memory); the compositor converts to the framebuffer's format.
pub const PIXEL_RGB: u32 = 0; pub const PIXEL_BGR: u32 = 1; pub const PIXEL_BITMASK: u32 = 2; // pixel_masks = red, green, blue
pub const fn pixel_to_device(pixel: u32, format: u32, masks: [u32; 3]) -> u32 {
    let (r, g, b) = ((pixel >> 16) & 0xFF, (pixel >> 8) & 0xFF, pixel & 0xFF);
    match format {
        PIXEL_RGB => r | g << 8 | b << 16,
        PIXEL_BITMASK => channel(r, masks[0]) | channel(g, masks[1]) | channel(b, masks[2]),
        _ => pixel & 0x00FF_FFFF,
    }
}
// An 8-bit channel value scaled to the width of `mask` and placed at its position.
const fn channel(value: u32, mask: u32) -> u32 {
    if mask == 0 { return 0; }
    let (shift, width) = (mask.trailing_zeros(), mask.count_ones());
    let scaled = if width >= 8 { value << (width - 8) } else { value >> (8 - width) };
    (scaled << shift) & mask
}
// The system-call ABI's version (MC-11.1, issue 172): 2 since 64-bit handles, 3 since senders wait in order without
// ERR_BUSY (000-KRN-0010), 4 since BootInfo names the boot volume and the slot (211-KRN-0012, 351-KRN-0014) and
// BOOT_CONFIRM ends a trial, 5 since SLOT_SHELL and SLOT_CLIPBOARD moved SLOT_DYNAMIC to 32 (211-KRN-0058) and bcm_wifi is a 33rd boot image
// (550-KRN-0059), 6 since setting a badge needs CAP_BADGE (000-KRN-0073), 10 since SLOT_WIFI moved SLOT_DYNAMIC to 34 and
// the Wi-Fi station wlan and its stand-in radio wlansim are boot images, tpm and keystore before them (550-KRN-0097).
// ABI 11 adds the two Realtek boot images and a non-mastering MMIO grant (213-KRN-0114). ABI 12 adds the SSH service
// `ssh` after `tls` and moves SLOT_DYNAMIC to 36 for SLOT_SSH and SLOT_UPDATE (351-KRN-0115, 351-KRN-0116).
// ABI 13 adds the link to another machine: `usb_serial` and `link` right after `vfs_server` (800-KRN-0128).
// ABI 14 moves SLOT_DYNAMIC to 37 for SLOT_MODE, the shell's client of init that sets the machine's modes (750-KRN-0143).
// ABI 15 moves it to 38 for SLOT_COMMANDS, the shell's command endpoint made by init, so that link may hold a client (800-KRN-0148);
// it also adds KEY_PRINT_SCREEN (000-KRN-0149) and an absolute pointer event's horizontal wheel steps (211-KRN-0150).
// ABI 16 adds the Effector agent's two boot images, `effector_gw` and `effector`, which run only in a mode (750-KRN-0119).
// ABI 17: CAP_REVOKE leaves a borrower one zeroed page of its own at each page it mapped from the lease, so a late access
// reads zeros and writes there instead of faulting (000-KRN-0152). ABI 18 adds workers: WORKER_START, SPAWN_WORKERS and
// the info page's WORKER_OFFSET (262-KRN-0163). ABI 19: memory is mapped in 2 MiB and 1 GiB pages where both its
// addresses allow, and ALLOC's arg2 takes ALLOC_SMALL_PAGES (262-KRN-0161). ABI 20 moves SLOT_DYNAMIC to 39 for SLOT_APPS,
// the Effector gateway's writable scope of data/apps/ (750-KRN-0169), and CAP_REVOKE walks in steps, refusing a copy,
// mint or transfer under its root with ERR_REVOKING meanwhile, a copy past DERIVE_DEPTH_MAX refused (000-KRN-0104).
// ABI 21 adds the boot image `rtw88_wifi`, the RTL8821CE's driver, after `rtw89_wifi`, so that init starts it before
// `wlan` (550-KRN-0172, for 550-DRV-0042).
// The kernel writes it into every task's BootInfo, and libmind refuses to run a program built for another one; the
// bootloader writes the handoff version below instead (351-KRN-0166). ABI 11 preserves ABI 10's prefix and checks its
// version before dereferencing the full record.
// This transition does not promise detection of ABI 9 or older layouts.
pub const ABI_VERSION: u32 = 21;
// The loader's handoff (351-KRN-0166): BootInfo's layout and meaning, and which boot image each index is, versioned apart
// from the system calls. The loader writes BOOT_HANDOFF_VERSION where ABI 10 put the ABI; the kernel takes the versions from
// BOOT_HANDOFF_OLDEST to it. A change to BootInfo or BOOT_FILES sets both to one number above every ABI_VERSION so far
// (MC-12.4). Handoff 21 (550-KRN-0172): a 44th image renumbers the images after it and moves every field after
// extra_programs, so an older loader is refused by name; before the first release no older handoff is read (AGENTS.md, section 3).
// Every handoff keeps ABI 10's record in front (the screen, acpi_rsdp, abi_version, boot_slot), so that a kernel that
// refuses one still says why and, on a trial, restarts into the fallback (351-KRN-0167).
pub const BOOT_HANDOFF_VERSION: u32 = 21;
pub const BOOT_HANDOFF_OLDEST: u32 = 21;
pub const fn handoff_accepted(version: u32) -> bool { version >= BOOT_HANDOFF_OLDEST && version <= BOOT_HANDOFF_VERSION }
// BootInfo.boot_flags: BOOT_SAFE, a safe start asked at the bootloader (a key held), in which init ignores
// data/services.txt (173-KRN-0036).
pub const BOOT_SAFE: u32 = 1;
#[derive(Clone, Copy)] #[repr(C)] pub struct BootInfo { pub fb_ptr: *mut u32, pub width: usize, pub height: usize, pub stride: usize, pub programs: [ProgramImage; BOOT_PREFIX_IMAGES], pub heap_ptr: *mut u8, pub heap_len: usize, pub ap_trampoline: usize, pub cpu_count: usize, pub apic_ids: [u32; 8], pub memory_map: *const StatPhys, pub memory_map_len: usize, pub pixel_format: u32, pub pixel_masks: [u32; 3], pub acpi_rsdp: u64, pub cpu_features: u64, pub abi_version: u32,
    pub boot_volume: BootVolume, pub boot_slot: BootSlot, pub launch: LaunchRecord, pub efi_runtime: u64, pub device_tree: u64, pub photo_pause_s: u32,
    pub extra_programs: [ProgramImage; BOOT_IMAGES - BOOT_PREFIX_IMAGES],
    pub link_present: u32, pub link_words: [u8; LINK_WORDS_MAX], pub boot_flags: u32 }
impl BootInfo {
    pub fn image(&self, index: usize) -> Option<&ProgramImage> {
        if index < BOOT_PREFIX_IMAGES { self.programs.get(index) } else { self.extra_programs.get(index - BOOT_PREFIX_IMAGES) }
    }
    pub fn images(&self) -> impl Iterator<Item = &ProgramImage> { self.programs.iter().chain(self.extra_programs.iter()) }
    /// Reads only ABI 10's fixed version field; the caller supplies a readable ABI 10 or later prefix.
    pub unsafe fn read_abi_version(info: *const Self) -> Option<u32> {
        if info.is_null() || (info as usize) % core::mem::align_of::<Self>() != 0 { return None; }
        Some(core::ptr::addr_of!((*info).abi_version).read())
    }
    /// What a kernel that refuses the handoff still reads (351-KRN-0167): whether the boot is a trial, and the ACPI root;
    /// the caller supplies an ABI 10 or later record.
    pub unsafe fn read_refused(info: *const Self) -> (bool, u64) {
        (core::ptr::addr_of!((*info).boot_slot.trial).read() != 0, core::ptr::addr_of!((*info).acpi_rsdp).read())
    }
}
// Both supported architectures have 64-bit pointers; changing this offset breaks the ABI 10 rejection path.
#[cfg(target_pointer_width = "64")]
const _: () = assert!(core::mem::offset_of!(BootInfo, abi_version) == 720);
// BootInfo.photo_pause_s: seconds to wait for a photo of the screen, from data/pause.txt on the boot volume (at most
// 60); 0, the default, waits for nothing (ABI 7, 211-KRN-0089).
pub const PHOTO_PAUSE_MAX_S: u32 = 60;
// BootInfo.link_present and link_words: whether the boot volume's root holds link.conf, and its words (NUL-padded), as
// the bootloader read them through the firmware, so that a link to another machine starts even when MIND Core cannot
// read its boot disk; init starts `usb_serial` and `link` only with the file (ABI 13, 800-KRN-0128).
pub const LINK_WORDS_MAX: usize = 64;
// BootInfo.efi_runtime: the address of the firmware's EFI_RUNTIME_SERVICES table, 0 without one; the kernel calls its
// variable services in the identity map (no SetVirtualAddressMap) and gives tasks 0 (ABI 4, 351-KRN-0027).
// BootInfo.device_tree: the flattened device tree in the firmware's configuration table (a board without ACPI: U-Boot on
// a Mac, QEMU virt with acpi=off), 0 without one; the kernel checks its header and gives tasks 0 (ABI 4, 210-KRN-0029).
// The partition the bootloader read the system from, from the firmware's device path of its own image (211-KRN-0012):
// kind VOLUME_MBR (signature: the disk's 32-bit signature in its first 4 bytes) or VOLUME_GPT (signature: the
// partition's GUID); VOLUME_UNKNOWN when the firmware names no partition. start and sectors in 512-byte sectors.
#[derive(Clone, Copy, Default)] #[repr(C)] pub struct BootVolume { pub kind: u32, pub partition: u32, pub start: u64, pub sectors: u64, pub signature: [u8; 16] }
pub const VOLUME_UNKNOWN: u32 = 0;
pub const VOLUME_MBR: u32 = 1;
pub const VOLUME_GPT: u32 = 2;
// Which copy of the system runs (351-KRN-0014): BOOT_SLOT_ROOT (a volume without boot records), BOOT_SLOT_A or _B; trial
// nonzero for a slot booted on trial, which the kernel restarts after deadline_s seconds unless init confirms it
// (BOOT_CONFIRM); manifest: the SHA-256 of the boot manifest the bootloader verified and loaded from.
#[derive(Clone, Copy, Default)] #[repr(C)] pub struct BootSlot { pub slot: u32, pub trial: u32, pub deadline_s: u32, pub manifest: [u8; 32] }
pub const BOOT_SLOT_ROOT: u32 = 0;
pub const BOOT_SLOT_A: u32 = 1;
pub const BOOT_SLOT_B: u32 = 2;
pub const TRIAL_DEADLINE_S: u32 = 120;
// The launch record (350-UPD-0004, MC-9.5): the signing key's identity as the verified manifest names it (16 hex
// digits), whether it is the public test key, and how many images were checked against it; with boot_slot.manifest,
// what the serial line's BOOT: MANIFEST line says. Evidence only: nothing grants or refuses on it.
#[derive(Clone, Copy, Default)] #[repr(C)] pub struct LaunchRecord { pub key: [u8; 16], pub test_key: u32, pub images: u32 }
// BootInfo.cpu_features, set by the kernel in every task's copy: what the processor offers programs (issue 201).
pub const FEATURE_ENTROPY: u64 = 1; // a hardware random number instruction (RDRAND, RNDR)
pub const FEATURE_AVX: u64 = 2; // x86: AVX and AVX2 registers are saved per task (174-KRN-0037)
pub const FEATURE_AVX512: u64 = 4; // x86: AVX-512's ZMM and opmask registers are saved per task
pub const FEATURE_AMX: u64 = 8; // x86: AMX's tile configuration and tiles are saved per task
#[derive(Clone, Copy)] #[repr(C)] pub struct SyscallMailbox { pub syscall_num: usize, pub arg1: usize, pub arg2: usize, pub result: usize, pub msg: [usize; 4], }
impl SyscallMailbox { pub const EMPTY: Self = Self { syscall_num: 0, arg1: 0, arg2: 0, result: 0, msg: [0; 4] }; }

pub const SYSCALL_RDTSC: usize = 1;
pub const SYSCALL_READ_KEY: usize = 2;
// LOG: arg1 = address, arg2 = length (at most 4096) -> bytes taken. A console its reader drained within the last second
// (an application's from its start) takes only what fits; the caller sends the rest again (ABI 4, 000-KRN-0030).
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
// of the next key event that has one (a pointer event's byte is its horizontal steps, ABI 15).
pub const SYSCALL_READ_INPUT: usize = 50;
// INPUT_POINTER: arg1 = 1 to receive pointer events (KEY_POINTER), 0 to stop; for the calling task only. Without it
// the kernel drops pointer events instead of queueing them (issue 156).
pub const SYSCALL_INPUT_POINTER: usize = 56;
// INPUT_LISTEN (process control, issue 154): arg1 = key (KEY_*) | modifiers (MOD_SHIFT|MOD_CTRL|MOD_ALT, exactly those
// held) << 16, arg2 = 1 to listen, 0 to stop. Presses of that key with those modifiers, and their releases, go to the
// caller's own input queue instead of the focused task's, whatever has the focus. INPUT_LISTENERS registrations in
// the system; one for the same key and modifiers is replaced; they end with the task. Pointer events cannot be taken.
pub const SYSCALL_INPUT_LISTEN: usize = 57;
pub const INPUT_LISTENERS: usize = 8;
pub const SYSCALL_COMPOSITOR_PULL: usize = 21;
// COMPOSITOR_PULL's arg2 (000-KRN-0086). 0: the focused screen's state, as before. COMPOSITOR_FRAME: the same, but a
// changed screen whose task still runs gives COMPOSITOR_BUSY (it may be mid-frame); one whose task waits gives 1 and the
// task is held, not run, until COMPOSITOR_RELEASE or 50 ms, so the copy is of one whole frame.
pub const COMPOSITOR_FRAME: usize = 1;
pub const COMPOSITOR_RELEASE: usize = 2;
pub const COMPOSITOR_BUSY: usize = 3;
pub const SYSCALL_IPC_CALL: usize = 22;
pub const SYSCALL_IPC_REPLY: usize = 23;
pub const SYSCALL_IRQ_BIND: usize = 24;
// IRQ_BIND: arg1 = interrupt handle, arg2 = endpoint (read right). Up to IRQ_SHARERS drivers bind one line (PCI INTx
// lines are shared): each gets every interrupt of the line, which stays masked until each has called IRQ_ACK. A binder
// binding again replaces its binding; one that ends is dropped. IRQ_WAIT works only on a line nobody bound (issue 159).
pub const IRQ_SHARERS: usize = 4;
pub const SYSCALL_IRQ_ACK: usize = 25;
pub const SYSCALL_MEM_PHYS: usize = 26;
pub const SYSCALL_PORT_IN_BLOCK: usize = 27;
// PORT_OUT_BLOCK: as PORT_IN_BLOCK, 16-bit words from the process buffer to the port (ATA sector writes).
pub const SYSCALL_PORT_OUT_BLOCK: usize = 53;
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
// Setting a badge needs CAP_BADGE on the unbadged capability (ERR_INVALID without it, ABI 6, 000-KRN-0073); a badged
// capability never carries it.
pub const BADGE_MAX: usize = 0xFFFF;
// CAP_MINT: arg1 = handle, arg2 = rights mask (endpoints and memory), msg[0] = offset, msg[1] = length (0: to the end) for port and
// memory ranges -> handle of a child with no more authority; one more than DERIVE_DEPTH_MAX derivations from its root, or
// of a capability a revocation in progress covers (ERR_REVOKING), is refused. CAP_REVOKE: arg1 = handle -> number of
// descendants removed from all tasks; the capability itself stays (MC-3.4-3.6). The walk visits only the descendants,
// in steps of at most 256 of them, the kernel letting go of its lock between steps; it returns after the last, with
// arg2 = the steps and msg[0] = the longest step in nanoseconds (000-KRN-0104, MC-5.4). While it runs, a copy, mint or
// transfer of anything under the handle is refused with ERR_REVOKING.
pub const SYSCALL_CAP_MINT: usize = 45;
pub const SYSCALL_CAP_REVOKE: usize = 46;
// MEM_DETACH: arg1 = start of a heap block no one else refers to -> handle of a memory object (read/write, no grant).
// The block leaves the caller's address space; the object lives while a capability or mapping refers to it. Copying a
// writable memory capability needs CAP_GRANT, so an object can only be moved (MOVE: one owner) or minted read-only.
pub const SYSCALL_MEM_DETACH: usize = 47;
// TASK_WATCH: arg1 = PID of a task the caller spawned, arg2 = endpoint handle with the read right. When the task ends,
// a receive on that endpoint gets msg[1] = MSG_FLAG_EXIT, data = [PID, reason | lost notices << 32] (sender PID 0).
pub const SYSCALL_TASK_WATCH: usize = 48;
// DEVICE_STATE: a platform privilege or an owned BAR authorizes checked Command transitions for the device index.
// STOP disables decoding and mastering; START enables supported decoding and mastering; REGISTERS enables only MEM.
pub const SYSCALL_DEVICE_STATE: usize = 49;
pub const DEVICE_STOP: usize = 0;
pub const DEVICE_START: usize = 1;
pub const DEVICE_REGISTERS: usize = 2;
// Why a task ended (a watch's notice, EXIT_STATUS): EXIT_NORMAL | code << 8 (EXIT's code, 0: success, issue 166),
// EXIT_KILLED, or EXIT_FAULT | vector << 8.
pub const EXIT_NORMAL: usize = 0;
pub const EXIT_KILLED: usize = 1;
pub const EXIT_FAULT: usize = 2; // | vector << 8
// EXIT_STATUS (process control): arg1 = PID of one of the last EXIT_STATUSES tasks that ended → its reason.
pub const SYSCALL_EXIT_STATUS: usize = 58;
pub const EXIT_STATUSES: usize = 16;
// MEMORY_RESERVE (platform privilege): arg1 = bytes of the frame pool kept for the system band, the supervisor's recovery
// reserve (MC-6.5, issue 169). A heap block of an application-band task, or an application's image, stack and screen,
// is refused (ALLOC: 0, SPAWN: ERR_NO_MEMORY) when it would leave less free. Rounded up to pages; beyond the pool: ERR_INVALID.
pub const SYSCALL_MEMORY_RESERVE: usize = 59;
pub const EXIT_NOTICES_MAX: usize = 16; // undelivered exit notices kept by the kernel; further ones are counted as lost
pub const DETACHED_MAX_BYTES: usize = 256 * 1024 * 1024; // all memory objects and freed-but-referenced blocks together

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
// A privilege held in escrow (issue 170): its holder cannot use it, only grant it at a service spawn, where the child
// gets the privilege itself; `rights` in STAT and CAP_INFO's arg2 name the privilege's kind.
pub const CAP_KIND_ESCROW: usize = 15;
// Reading and writing the firmware's variables (FIRMWARE_VARIABLE): BootNext, BootOrder, Boot####; init gives it to
// the shell, which lends it once the user agrees, and later to the updater (ABI 4, 351-KRN-0027).
pub const CAP_KIND_FIRMWARE: usize = 16;
// The writer of one task's text input (SPAWN_TEXT, TEXT_WRITE; ABI 9, 000-KRN-0094): CAP_INFO's arg2 is that task's PID.
pub const CAP_KIND_TEXT: usize = 17;

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
pub const ERR_FOCUS: usize = usize::MAX - 9; // SPAWN_FOREGROUND from a task that does not have the focus (issue 160)
pub const ERR_IO: usize = usize::MAX - 10; // services only: a drive did not answer or a medium could not be read (211-KRN-0050)
pub const ERR_REVOKING: usize = usize::MAX - 11; // a copy, mint or transfer of a capability a revocation in progress covers (000-KRN-0104)
pub const ERR_FIRST: usize = usize::MAX - 15;
pub const RTC_UNAVAILABLE: usize = usize::MAX;

pub const CAP_READ: u8 = 1 << 0; pub const CAP_WRITE: u8 = 1 << 1; pub const CAP_GRANT: u8 = 1 << 2;
// Keeper: may mint children with CAP_READ without being able to receive itself (init keeps service endpoints this way).
pub const CAP_KEEP: u8 = 1 << 3;
// Badger: CAP_MINT may set a badge on a child (000-KRN-0073). A new endpoint carries it; a copy or child keeps it only
// when the mask does, and a badged one never.
pub const CAP_BADGE: u8 = 1 << 4;
// Capability slots a task starts with; the table grows on demand up to CAP_SLOTS_MAX (issue 171), a bound on the
// kernel heap a task's table takes (about 56 bytes a slot), not on what a handle can name (issue 172).
pub const CAP_SLOTS: usize = 104;
pub const CAP_SLOTS_MAX: usize = 4096;
// Derivations from a root a capability may be (000-KRN-0104): a copy or mint deeper than this is refused with ERR_LIMIT.
pub const DERIVE_DEPTH_MAX: usize = 64;

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
// For vfs_server, slots 2..5 are block driver endpoints (the first three of ata, ahci, usb_storage, virtio_blk, nvme
// that run); then the RAM disk and an rtc client (calendar time for directory entries).
pub const SLOT_BLOCK_FIRST: usize = 2;
pub const BLOCK_DEVICES: usize = 3;
pub const SLOT_RAMDISK: usize = 5;
pub const SLOT_VFS_RTC: usize = 6;
// Shell: application slots plus process control, the input privilege and the COM1 port range.
pub const SLOT_CONTROL: usize = 7;
pub const SLOT_INPUT: usize = 8;
pub const SLOT_SERIAL: usize = 9;
// Capabilities a launcher grants an application that asks for them (loader launch sessions): its own VFS client for
// files the user may change (7: applications hold no process control), system information from sysmon (10),
// lifecycle control, a client of init (11), and the system log, logd (12; services hold their log client there too).
// Slots 13..15: further clients the shell holds and lends (issue 151): the authority view of sysmon (13, badged
// mind::stat::BADGE_AUTHORITY), the keyboard driver (14) and the compositor (15).
pub const SLOT_FILE: usize = 7;
pub const SLOT_SYSINFO: usize = 10;
pub const SLOT_LIFECYCLE: usize = 11;
pub const SLOT_LOG: usize = 12;
pub const SLOT_AUTHORITY: usize = 13;
pub const SLOT_KEYBOARD: usize = 14;
pub const SLOT_DISPLAY: usize = 15;
// The shell's client of the network card driver (idl/net.wit), for the `net` diagnostics.
pub const SLOT_NET: usize = 16;
// The shell's client of the network stack (idl/socket.wit): ip, ping, nslookup, fetch.
pub const SLOT_SOCKET: usize = 17;
// A flow grant of the network policy broker: in an application, a network stack client limited to what the policy
// names for it (REQUEST_NETWORK, issue 102); in the shell, its client of the broker.
pub const SLOT_NETWORK: usize = 18;
pub const SLOT_NETPOLICY: usize = 19;
// The shell's client of the TLS service (idl/tls.wit, issue 103): https, tls; lent in a program's same slot for
// REQUEST_TLS (351-KRN-0034), where `tls` runs the session over the program's own flow.
pub const SLOT_TLS: usize = 20;
// The shell's clients of the window broker (idl/window.wit, issue 157): a program's (lent for REQUEST_WINDOW) and the
// manager's, with mind::window::BADGE_MANAGER (lent for REQUEST_WINDOW_MANAGER). Both go to the program's SLOT_WINDOW.
pub const SLOT_WINDOWS: usize = 21;
pub const SLOT_WINDOW_MANAGER: usize = 22;
// In an application: its client of the window broker (8 is the input privilege only in the shell).
pub const SLOT_WINDOW: usize = 8;
// In an application: where what it prints goes besides the kernel's log (issue 162) — an endpoint of the program that
// started it (`console`), which shows it; mind::process::log sends it there, 15 bytes a message (9 is the serial port
// only in the shell).
pub const SLOT_CONSOLE: usize = 9;
// A client of the pin controller service (idl/gpio.wit, issue 207): the shell's, with the control badge, which it lends
// for REQUEST_GPIO to the program's same slot.
pub const SLOT_GPIO: usize = 23;
// A client of the video gateway (idl/video.wit, issue 158): the shell's, which it lends for REQUEST_CAMERA to the
// program's same slot once the user agreed.
pub const SLOT_CAMERA: usize = 24;
// A client of the block store (idl/blockstore.wit, 300-KRN-0001): the shell's (get, put and publish), which it lends
// for REQUEST_BLOCKSTORE to the program's same slot.
pub const SLOT_BLOCKSTORE: usize = 25;
// The shell's block store client with the get badge alone, which it lends for REQUEST_BLOCKSTORE_READ to the program's
// SLOT_BLOCKSTORE: a program that only reads holds a client that cannot put or publish (ABI 4, 300-KRN-0024).
pub const SLOT_BLOCKSTORE_READ: usize = 26;
// The firmware variable privilege (CAP_KIND_FIRMWARE): the shell's, which it lends for REQUEST_FIRMWARE to the
// program's same slot once the user agreed (ABI 4, 351-KRN-0027).
pub const SLOT_FIRMWARE: usize = 27;
// A client of the parser service (idl/parse.wit, 109): the shell's, which it lends for REQUEST_PARSE to the program's
// same slot (109-KRN-0042).
pub const SLOT_PARSE: usize = 28;
// The shell's client of the TPM service (idl/tpm.wit), without the seal badge: `tpm` shows the TPM (351-KRN-0043).
pub const SLOT_TPM: usize = 29;
// The shell's command endpoint (idl/shell.wit), lent for REQUEST_SHELL and passed by wm to console (ABI 5, 211-KRN-0058).
pub const SLOT_SHELL: usize = 30;
// A client of the clipboard service (idl/clipboard.wit), lent for REQUEST_CLIPBOARD (ABI 5, 211-KRN-0058).
pub const SLOT_CLIPBOARD: usize = 31;
// A client of the battery service (idl/battery.wit, 211-PRT-0008): the shell's, which it lends for REQUEST_BATTERY to
// the program's same slot (ABI 8, 211-KRN-0090).
pub const SLOT_BATTERY: usize = 32;
// The Wi-Fi station's configuration client (idl/wifi.wit, with mind::network::BADGE_WIFI_CHANGE): the shell's, which it
// lends for REQUEST_WIFI to the program's same slot (ABI 10, 550-KRN-0097, for 550-APP-0033).
pub const SLOT_WIFI: usize = 33;
// The SSH service's client (idl/ssh.wit): the shell's, with the right to badge, which it lends for REQUEST_SSH to the
// program's same slot (ABI 12, 351-KRN-0115).
pub const SLOT_SSH: usize = 34;
// The updater's client (idl/update.wit): the shell's alone, for its `update` command; no program gets it (ABI 12, 351-KRN-0116).
pub const SLOT_UPDATE: usize = 35;
// init's client badged BADGE_MODE (init.wit 1.5 set-mode): the shell's alone, for its `mode` command; no program gets it
// (ABI 14, 750-KRN-0143).
pub const SLOT_MODE: usize = 36;
// The shell's command endpoint (idl/shell.wit), made by init and kept across the shell's restarts: the shell serves it, and
// init mints link's client of it in developer mode (ABI 15, 800-KRN-0148).
pub const SLOT_COMMANDS: usize = 37;
// The Effector gateway's writable scope of data/apps/, made by init at each of its starts, where it installs signed
// application packages; no program gets it (ABI 20, 750-KRN-0169).
pub const SLOT_APPS: usize = 38;
// The kernel hands out new capabilities starting from this slot; slots below it are fixed by convention.
pub const SLOT_DYNAMIC: usize = 39;
// The fixed slots a launcher may fill in a launch session (loader.wit `grant`); 2..6 stay the loader's own. SLOT_NET
// carries a card's read-only client (BADGE_COUNTERS) for REQUEST_NET_COUNTERS (ABI 10, 000-NET-0015).
pub const LAUNCH_SLOTS: [usize; 20] = [SLOT_INIT, SLOT_FILE, SLOT_WINDOW, SLOT_CONSOLE, SLOT_SYSINFO, SLOT_LIFECYCLE, SLOT_LOG, SLOT_NET, SLOT_NETWORK,
    SLOT_DISPLAY, SLOT_GPIO, SLOT_CAMERA, SLOT_BLOCKSTORE, SLOT_TLS, SLOT_PARSE, SLOT_SHELL, SLOT_CLIPBOARD, SLOT_BATTERY, SLOT_WIFI, SLOT_SSH];
// A capability handle is `slot | generation << HANDLE_GENERATION_SHIFT`. Fixed slots (below SLOT_DYNAMIC) are named with
// generation 0; a slot the kernel hands out gets a new generation every time it is freed, so an old handle stays invalid.
// Received capabilities and the compositor's screen are placed only in fixed slots. A handle is 64 bits (issue 172):
// 24 bits of slot and 32 of generation, which wraps after 4 294 967 295 reuses of one slot.
pub const HANDLE_SLOT_MASK: usize = 0xFF_FFFF;
pub const HANDLE_GENERATION_SHIFT: usize = 24;
pub const HANDLE_GENERATION_MAX: usize = 0xFFFF_FFFF;

// Endpoints have no global names: every one is created by ENDPOINT_CREATE (init's own by the kernel) and reached only
// through capabilities (MC-3.3).
// Block device kinds reported by block.kind (protocol data, not authority).
pub const BLOCK_KIND_ATA: usize = 1;
pub const BLOCK_KIND_AHCI: usize = 2;
pub const BLOCK_KIND_USB: usize = 3;
pub const BLOCK_KIND_VIRTIO: usize = 5; // after mind::block::KIND_RAM (4)
pub const BLOCK_KIND_NVME: usize = 6;

// Message: msg[0]=handle of the capability to transfer, msg[1]=rights mask | CAP_TRANSFER_MOVE, msg[2..4]=data.
// The mask narrows endpoint rights only; other capabilities keep their rights (narrow memory with CAP_MINT first).
// A transfer is a copy (a child the sender can revoke) unless CAP_TRANSFER_MOVE moves it out of the sender's table.
pub const CAP_TRANSFER_MOVE: usize = 1 << 8;
// IPC_SEND, IPC_CALL, IPC_RECV: arg1 = endpoint handle, arg2 = receive slot | timeout in milliseconds << IPC_TIMEOUT_SHIFT
// (0: wait without limit; 10 ms granularity; issue 172: before, the timeout was in arg1). On expiry the call fails with ERR_TIMEOUT: a waiting send leaves the queue with its
// capability, a caller stops waiting and the server's later reply fails with ERR_PEER.
pub const IPC_TIMEOUT_SHIFT: usize = 32;
// Senders wait on an endpoint in the order they sent, each until received or its timeout (back-pressure, MC-2.5,
// MC-5.2): a task has at most one send waiting, so the wait is bounded by the tasks. ABI 3 (000-KRN-0010): before, a
// fifth sender got ERR_BUSY (ENDPOINT_QUEUE, 4) and retried without order.
// At the receiver: arg1=sender PID, msg[0]=1 if a capability was received, msg[1]=flags.
pub const MSG_FLAG_CALL: usize = 1;
pub const MSG_FLAG_IRQ: usize = 2;
pub const MSG_FLAG_EXIT: usize = 4; // exit notice of a watched task (TASK_WATCH)

// HEAP_MAX_BYTES is the default memory quota of a task (SPAWN); blocks are heap blocks and mappings together.
pub const HEAP_PAGE_SIZE: usize = 4096; pub const HEAP_MAX_BLOCKS: usize = 64; pub const HEAP_MAX_BYTES: usize = 16 * 1024 * 1024;
// ALLOC's arg2 holds flags: ALLOC_SMALL_PAGES maps the block in 4 KiB pages only, not 2 MiB and 1 GiB ones, as for a
// measurement of what large pages give (262-KRN-0161, ABI 19). Any other bit refuses the call.
pub const ALLOC_SMALL_PAGES: usize = 1;
// A task's stack is what its ELF's PT_GNU_STACK asks (p_memsz, the linker's -z stack-size=), in whole pages, at least
// STACK_MIN_BYTES; an ELF asking more than STACK_MAX_BYTES is refused. An unmapped guard page lies below it (262-KRN-0164).
pub const STACK_MIN_BYTES: usize = 64 * 1024; pub const STACK_MAX_BYTES: usize = 8 * 1024 * 1024;
// Separate limit for mapped foreign memory (frame, IPC buffers, shared read-only objects).
pub const SHARED_MAX_BYTES: usize = 256 * 1024 * 1024;

// RTC protocol: idl/rtc.wit (MIND IDL, bindings in mind::idl::rtc).
// VFS protocol: idl/vfs.wit (bindings in mind::idl::vfs).
// Program loader: idl/loader.wit (list, run with arguments). Legacy adapter until loader v1: CALL on SLOT_LOADER with
// the program name packed into msg[2..4] and an optional endpoint for the child's INIT slot; reply msg[2] = PID or error.

// SPAWN (requires the spawn privilege): arg1/arg2 = name, msg[0] = image memory capability or SPAWN_BOOT | boot image
// index (boot images need the platform privilege), msg[1] = ELF length | PID << 32 (SPAWN_FOREGROUND), msg[2] = address of a Grant array,
// msg[3] = grant count | SPAWN_* flags << 8 | child task quota << 16 | child endpoint quota << 32 | child memory quota in
// MiB << 48 (0: HEAP_MAX_BYTES, SPAWN_MEMORY_ALL: the spawner's). The quotas are taken
// from the spawner's (MC-3.13): a spawner's live children each reserve 1 + their task quota of its task quota, and their
// endpoint quotas plus the endpoints it created count against its endpoint quota. Each grant copies the spawner's capability into a child slot;
// endpoint rights are narrowed by the mask, reply capabilities are not transferable.
pub const SPAWN_BOOT: usize = 1 << 63;
// Memory quota (issue 150): private memory (heap blocks, memory objects) of a task and its live descendants is charged
// to it and to every spawner above; a child's quota is at most its spawner's. init's quota is the frame pool.
pub const SPAWN_MEMORY_ALL: usize = 0xFFFF;
pub const SPAWN_SERVICE: usize = 1; // system service (platform privilege only)
pub const SPAWN_SCREEN: usize = 2; // the task gets a screen buffer and can take the focus
// With SPAWN_SCREEN: the task starts in front if the task named by PID in msg[1] has the focus now, else SPAWN fails with
// ERR_FOCUS and nothing is created; when it ends in front the focus goes back to that task (issue 160). The spawner
// (the loader) names the task it starts the program for.
pub const SPAWN_FOREGROUND: usize = 4;
// The task gets a text input (000-KRN-0094, ABI 9): its writer, a CAP_KIND_TEXT capability, goes to the spawner, its
// handle in arg2 of the reply. Without the flag the task has none: TEXT_READ answers ERR_NOT_FOUND.
pub const SPAWN_TEXT: usize = 8;
// The kernel keeps the task's program (its ELF, or its boot image's index), charged with its image, so that the task may
// start workers of it (WORKER_START, 262-KRN-0163, ABI 18). Without the flag WORKER_START answers ERR_RIGHTS.
pub const SPAWN_WORKERS: usize = 16;
pub const SPAWN_GRANTS_MAX: usize = 32; // 32 since ABI 4: the shell's grants grew (351-KRN-0027)
// Program arguments: the SPAWN name buffer may be `name\0arguments`; the kernel copies the arguments into the child's
// read-only info page at ARGS_OFFSET as a u16 length followed by the bytes.
pub const ARGS_OFFSET: usize = 2048;
pub const ARGS_MAX: usize = 1024;
// 16 bytes (issue 172): the spawner's handle, the child's slot, rights and flags.
#[derive(Clone, Copy, Default)] #[repr(C)] pub struct Grant { pub own: u64, pub child: u8, pub rights: u8, pub flags: u16, pub reserved: u32 }
pub const GRANT_MOVE: u16 = 1; // move the capability into the child instead of copying it

// PLATFORM_CAP: arg1 = kind, arg2 and msg[0] = arguments; result = new slot. The kernel validates every resource.
pub const PLATFORM_PORTS: usize = 2; // base, count: only legacy ranges from the platform profile
pub const PLATFORM_IRQ: usize = 3; // line of a platform device: ISA 1..15 except the cascade (2); aarch64 virt: 1 UART, 2 RTC
pub const PLATFORM_DEVICE_BAR: usize = 4; // device index, BAR number: port range or MMIO
pub const PLATFORM_DEVICE_IRQ: usize = 5; // device index
pub const PLATFORM_FRAMEBUFFER: usize = 6;
pub const PLATFORM_DMA: usize = 7; // bytes; 64 KiB aligned, kept by the kernel for the platform's lifetime
pub const PLATFORM_PRIVILEGE: usize = 8; // CAP_KIND_INPUT, _DISPLAY, _SPAWN, _CONTROL, _RESTART or _OBSERVE; b = PRIVILEGE_ESCROW: in escrow (not _RESTART)
pub const PRIVILEGE_ESCROW: usize = 1;
pub const PLATFORM_DEVICE_MSIX: usize = 9; // device index, MSI-X table entry: an interrupt line 16..31 the kernel aims the entry at
pub const PLATFORM_REPORT: usize = 11; // a read-only memory object with the hardware report's text, built now (174-KRN-0038)
pub const PLATFORM_ACPI_TABLE: usize = 12; // index: a read-only copy of the n-th ACPI table of the report's list, ERR_NOT_FOUND past the last
pub const PLATFORM_DEVICE_MSI: usize = 13; // device index: its MSI (one message), aimed by the kernel at an interrupt line 16..31 (550-KRN-0091)
pub const PLATFORM_DEVICE_BAR_IDLE: usize = 14; // device index, memory BAR: MEM decode only, no I/O or DMA; verified readback
pub const PLATFORM_MMIO: usize = 10; // index: registers of a platform device outside PCI (aarch64: the board's UART, RTC)
pub const PLATFORM_UART: usize = 0; // the console UART (aarch64: a PL011 the SPCR names)
pub const PLATFORM_RTC: usize = 1; // the RTC (aarch64: a PL031)
pub const PLATFORM_PINS_PL061: usize = 0x10; // + n: the n-th PL061 pin controller the firmware's tables name (issue 206)
pub const PLATFORM_PINS_BCM2711: usize = 0x20; // + n: the n-th BCM2711 GPIO (Raspberry Pi 4)
pub const PLATFORM_PINS_MAX: usize = 4; // controllers of one kind
// The TPM 2.0's registers of locality 0 (4 KiB) (351-KRN-0043): the CRB the ACPI TPM2 table names, or the FIFO. NOT_FOUND
// until the kernel reads their place from the firmware's tables (issues/requests-KRN.md).
pub const PLATFORM_TPM: usize = 0x30;
// DEVICE_FIND: arg1 = PCI class code (class<<16|subclass<<8|interface), arg2 = mask, msg[0] = n-th match, msg[1] = PCI
// vendor | device << 16 to match as well (0: any); result = device index.

// TASK_LIST fills an array of TaskInfo (arg1 = address, arg2 = capacity) and returns the count.
#[derive(Clone, Copy)] #[repr(C)] pub struct TaskInfo { pub pid: u64, pub name: [u8; NAME_MAX], pub state: [u8; 8], pub cpu: u32, pub focus: u8, pub service: u8, pub screen: u8, pub reserved: u8, pub runs: u64, pub ticks: u64, pub calls: u64, pub quota_tasks: u16, pub used_tasks: u16, pub quota_endpoints: u16, pub used_endpoints: u16 }
// FAULTS fills an array of FaultInfo (arg1 = address, arg2 = capacity) and returns the count.
#[derive(Clone, Copy, Default)] #[repr(C)] pub struct FaultInfo { pub pid: u64, pub cpu: u64, pub vector: u64, pub error: u64, pub rip: u64, pub address: u64 }
// FOCUS: arg1 = PID (0 = the caller), arg2 = 1 to keep the task's buffered console output; result = PID.
// The caller becomes the focus owner: focus returns to it when the focused task exits or on an attention key.
// NOTICE: 0 if none, else PID | NOTICE_EXITED (the focused task exited), PID | NOTICE_FRONT (the task in front started it
// in its place, SPAWN_FOREGROUND) or PID (sent to the background).
pub const NOTICE_EXITED: usize = 1 << 63;
pub const NOTICE_FRONT: usize = 1 << 62; // issue 160

// Input events: one word per key press or release, queued per task (64, oldest dropped). The kernel stores and routes
// them; decoding and layouts live in ring 3 (ps2_kbd, the shell's UART decoder).
// bits 0-7: legacy byte for READ_KEY (raw scancode or UART byte; 0: none; in an absolute pointer event, its horizontal steps) · 8-23: key (KEY_*; 0: not decoded) ·
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
// PrintScreen (E0 37 from a PC keyboard, HID usage 0x46), for a screenshot key (ABI 15, 000-KRN-0149).
pub const KEY_PRINT_SCREEN: u16 = 33;
// A pointer event (issue 156): the modifier byte holds the buttons (POINTER_*), the character field the movement:
// bits 0-8 dx, 9-17 dy (both signed, dy grows downwards), 18-21 the wheel (signed, positive: towards the user).
pub const KEY_POINTER: u16 = 32;
pub const POINTER_LEFT: u8 = 1; pub const POINTER_RIGHT: u8 = 2; pub const POINTER_MIDDLE: u8 = 4;
pub fn pointer_event(buttons: u8, dx: i32, dy: i32, wheel: i32) -> usize {
    let field = (dx.clamp(-256, 255) as u32 & 0x1FF) | (dy.clamp(-256, 255) as u32 & 0x1FF) << 9 | (wheel.clamp(-8, 7) as u32 & 0xF) << 18;
    input_event(0, KEY_POINTER, buttons, true, field)
}
/// (buttons, dx, dy, wheel) of a pointer event.
pub fn pointer_fields(event: usize) -> (u8, i32, i32, i32) {
    let field = event_char(event);
    let signed = |value: u32, bits: u32| ((value << (32 - bits)) as i32) >> (32 - bits);
    (event_mods(event), signed(field & 0x1FF, 9), signed(field >> 9 & 0x1FF, 9), signed(field >> 18 & 0xF, 4))
}
// A relative pointer event's horizontal wheel (211-DRV-0018: a trackpad's three-finger swipe): bits 34-37 of the event,
// signed, positive to the right, where an absolute event keeps its wheel. Zero in every event made before; consumers that
// do not read it see an ordinary pointer event.
pub fn pointer_scroll(buttons: u8, dx: i32, dy: i32, wheel: i32, across: i32) -> usize {
    pointer_event(buttons, dx, dy, wheel) | (across.clamp(-8, 7) as usize & 0xF) << 34
}
/// The horizontal wheel steps of a pointer event, relative or absolute (0 in an event made without them).
pub fn pointer_across(event: usize) -> i32 {
    let steps = if event & POINTER_ABSOLUTE != 0 { event & 0xF } else { (event >> 34) & 0xF };
    (steps as i32) << 28 >> 28
}
// An absolute pointer event (issue 161): a pointer event with POINTER_ABSOLUTE set carries a position instead of the
// movement — x in bits 0-11 and y in bits 12-23 of the character field — and the wheel in bits 34-37 (signed). In the
// kernel's input queues the position is a share of the screen, 0 to POINTER_SCALE - 1 from the left and top edges (a
// tablet: the host's pointer in a virtual machine); in a window's queue it is a cell of the window's content (issue u001).
pub const POINTER_ABSOLUTE: usize = 1 << 33;
pub const POINTER_SCALE: usize = 4096;
pub fn pointer_absolute(buttons: u8, x: usize, y: usize, wheel: i32) -> usize {
    let field = (x.min(POINTER_SCALE - 1) | y.min(POINTER_SCALE - 1) << 12) as u32;
    input_event(0, KEY_POINTER, buttons, true, field) | POINTER_ABSOLUTE | (wheel.clamp(-8, 7) as usize & 0xF) << 34
}
// An absolute event's horizontal wheel steps (211-KRN-0150): bits 0-3 of the byte field, signed, positive to the right;
// zero in every event pointer_absolute makes, so its callers and pointer_absolute_fields are unchanged (ABI 15).
pub fn pointer_absolute_scroll(buttons: u8, x: usize, y: usize, wheel: i32, across: i32) -> usize {
    pointer_absolute(buttons, x, y, wheel) | (across.clamp(-8, 7) as usize & 0xF)
}
/// (buttons, x, y, wheel) of an absolute pointer event; None for any other event.
pub fn pointer_absolute_fields(event: usize) -> Option<(u8, usize, usize, i32)> {
    if event_key(event) != KEY_POINTER || event & POINTER_ABSOLUTE == 0 { return None; }
    let field = event_char(event) as usize;
    Some((event_mods(event), field & 0xFFF, field >> 12 & 0xFFF, (((event >> 34 & 0xF) as i32) << 28) >> 28))
}
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
// After the last focused or screenless (console) program exited, both drain its unread console output.
// CPU_INFO: arg1 = CPU index; result = APIC id, arg2 = online, msg[2] = timer ticks. KERNEL_HEAP: result = used,
// arg2 = free, msg[2] = 1 if a test allocation was fully released.
// Block device protocol: idl/block.wit (bindings in mind::idl::block).
pub const BLOCK_SECTOR: usize = 512;
pub const BLOCK_MAX_SECTORS: usize = 128;
// Audio protocol: idl/audio.wit (bindings in mind::idl::audio).
// Speech synthesis: idl/tts.wit. init: idl/init.wit.
pub const AUDIO_RATE: usize = 48_000;

// STAT (observe or control privilege): arg1 = class, arg2 = buffer, msg[0] = capacity in bytes, msg[1] = argument
// (a PID for VMAP and CAPS), msg[2] = the first record to write (0: from the start; issue 171: callers page through any
// number). The buffer receives a StatHeader and then up to (capacity - header) / record_size records from that one on;
// the result is the number written, `total` says how many exist. Copies are bounded by the kernel's tables.
// Nothing returned is authority: endpoint indices are labels no system call accepts, and no task memory contents or
// physical addresses of task memory are exported (MC-10.2).
pub const SYSCALL_STAT: usize = 51;
// SCHED_SET (the task's lifecycle owner, or process control): arg1 = PID, arg2 = budget in microseconds per period
// (0: no limit), msg[0] = period in microseconds (>= 10 000), msg[1] = band (BAND_*, or BAND_KEEP; changing the band
// needs process control or the platform/restart privilege). A task that spent its budget waits for its next period;
// a ready task of band 0 always runs before one of band 1 (MC-5.1-5.5). Budgets are enforced at the end of each 10 ms slice.
pub const SYSCALL_SCHED_SET: usize = 52;
// DEVICE_CONFIG: arg1 = an MMIO or port capability over a BAR of a PCI function, arg2 = offset (< 256) -> the
// configuration dword at that offset (aligned down to 4) of that function; read only (drivers find their capabilities).
// With the platform privilege as arg1, msg[0] = device index: any device, without enabling it (init's inventory).
pub const SYSCALL_DEVICE_CONFIG: usize = 54;
// REBOOT (process control, or the restart privilege: init, for a service that may ask it, 351-KRN-0022): stops all CPUs
// and resets the machine: the ACPI reset register (FADT), else port 0xCF9,
// else the 8042 controller, else a triple fault; on aarch64 PSCI SYSTEM_RESET. arg1 = REBOOT_POWER_OFF turns the
// machine off instead (aarch64: PSCI SYSTEM_OFF, issue 203; x86: ACPI S5 from the DSDT, 000-KRN-0080, ERR_INVALID
// when the tables name none). It does not return when it works.
pub const SYSCALL_REBOOT: usize = 55;
// BOOT_CONFIRM (platform privilege: init, before it drops it): the boot is good. On a trial boot the kernel no longer
// restarts the machine at the deadline; writing the confirmed boot record is the updater's (351-UPD-0007, 0008).
// -> 1 if the boot was on trial, 0 if not (ABI 4, 351-KRN-0014).
pub const SYSCALL_BOOT_CONFIRM: usize = 60;
// FIRMWARE_VARIABLE (a CAP_KIND_FIRMWARE privilege in arg1; ABI 4, 351-KRN-0027): arg2 = FIRMWARE_GET or FIRMWARE_SET,
// msg[0] = a buffer in the caller's memory, msg[1] = its length (at most FIRMWARE_BUFFER): the GUID (16 bytes), the
// attributes (u32), the name's length in UTF-16 units without the terminator (u16), the data's length (u32), then
// the name (UTF-16LE) and the data. GET writes the attributes, the data's length and the data back in place and
// returns the length; ERR_NOT_FOUND for a variable not set, or a firmware without runtime variable services;
// ERR_INVALID with the needed length in arg2 when the data does not fit; ERR_RIGHTS when the firmware refuses a write.
pub const SYSCALL_FIRMWARE_VARIABLE: usize = 61;
pub const FIRMWARE_GET: usize = 0;
pub const FIRMWARE_SET: usize = 1;
pub const FIRMWARE_BUFFER: usize = 65536; // room for a signed dbx update (351-KRN-0028)
pub const FIRMWARE_HEADER: usize = 26;
// The mark of a run that has not ended well (000-KRN-0076): a firmware variable init sets at boot and clears a minute
// after READY, and the kernel clears at a reboot or power-off a task asked for. Found at boot, the last run stopped early
// (a freeze the watchdog reset, a cut power), and init starts safely. GUID {444E494D-4F43-4552-8076-52554E4D4152}.
pub const RUN_MARK_GUID: [u8; 16] = [0x4D, 0x49, 0x4E, 0x44, 0x43, 0x4F, 0x52, 0x45, 0x80, 0x76, 0x52, 0x55, 0x4E, 0x4D, 0x41, 0x52];
pub const RUN_MARK_NAME: &str = "MindCoreRun";
// TEXT_WRITE (a CAP_KIND_TEXT writer in arg1; ABI 9, 000-KRN-0094): msg[0] = bytes, msg[1] = their length; appends what
// fits of them (at most TEXT_QUEUE queued) to the task's text input and returns the count, 0 when it is full. With
// TEXT_CLOSE in arg2 and every byte taken, the input ends after them. ERR_PEER when the task ended, ERR_INVALID once
// closed, ERR_RIGHTS without the writer.
pub const SYSCALL_TEXT_WRITE: usize = 62;
// TEXT_READ (the task itself): arg1 = buffer, arg2 = its length → the bytes queued, up to it; 0 at the end, which comes
// once the input was closed or no writer is left. With nothing queued yet it waits up to TEXT_WAIT_MS for bytes, a
// close or any input event and answers ERR_BUSY: the caller asks again. ERR_NOT_FOUND for a task started without one.
pub const SYSCALL_TEXT_READ: usize = 63;
// WORKER_START (262-KRN-0163, ABI 18): arg1 = the worker's argument word, arg2 = its stack in bytes (0: what the
// program's ELF asks; more than STACK_MAX_BYTES: ERR_INVALID), msg[2] = address of a Grant array, msg[3] = grant count
// -> PID. The worker runs the caller's own program in an address space of its own, with exactly the granted
// capabilities (copied or moved as at SPAWN; never a privilege), on another CPU while one is free. Only a task started
// with SPAWN_WORKERS may (else ERR_RIGHTS); at most one live worker per online CPU, each counted in the caller's task
// quota (ERR_LIMIT). Its image, stack and memory are charged to the caller; it ends when the caller ends.
pub const SYSCALL_WORKER_START: usize = 64;
// A worker's info page holds at WORKER_OFFSET its index (1, 2, ...; 0 in a task that is no worker), then its argument
// word, a u64 each.
pub const WORKER_OFFSET: usize = 3584;
pub const TEXT_CLOSE: usize = 1;
pub const TEXT_QUEUE: usize = 4096;
pub const TEXT_WAIT_MS: u64 = 1000;
pub const REBOOT_POWER_OFF: usize = 1;
pub const BAND_SYSTEM: usize = 0; // init and services: their reserve survives application overload
pub const BAND_APPLICATION: usize = 1;
pub const BAND_KEEP: usize = 0xFF;
pub const STAT_VERSION: u32 = 2; // 2: the fields of issue 075 appended
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
    pub quota_tasks: u16, pub used_tasks: u16, pub quota_endpoints: u16, pub used_endpoints: u16, pub band: u8, pub throttled: u8, pub focus: u8, pub reserved: u8,
    pub budget_ns: u64, pub period_ns: u64,
    pub kernel_bytes: u64, // the task record, context, mailbox, info and exit pages, page tables, the capability table: in the frame pool, charged to its payers (171-KRN-0032)
    pub memory_quota: u64, pub memory_used: u64, // private memory of the task and its live descendants (issue 150)
}
// `xsave`: the state components saved per task with XSAVE (XCR0: 1 x87, 2 SSE, 4 AVX, 0xE0 AVX-512, 0x60000 AMX), 0 with
// FXSAVE (issue 153, 174-KRN-0037). On aarch64: 1, FP/SIMD (V0-V31, FPCR, FPSR; 250-KRN-0056).
#[derive(Clone, Copy, Default, Debug)] #[repr(C)] pub struct StatCpu { pub apic_id: u32, pub online: u32, pub ticks: u64, pub busy_ns: u64, pub idle_ns: u64, pub interrupts: u64, pub switches: u64, pub current_pid: u64, pub xsave: u64 }
// Task memory (bytes) by category, the kernel arena, the frame pool and the global limits. `largest_free` is searched for (trial allocations) only when
// msg[1] = 1 asks for it, 0 otherwise; `shared` is memory of other owners mapped by tasks.
#[derive(Clone, Copy, Default, Debug)] #[repr(C)] pub struct StatMemory {
    pub arena: u64, pub used: u64, pub free: u64, pub images: u64, pub stacks: u64, pub task_pages: u64, pub screens: u64,
    pub heaps: u64, pub objects: u64, pub dma: u64, pub dma_limit: u64, pub objects_limit: u64, pub tasks: u64, pub endpoints: u64,
    pub largest_free: u64, pub page_tables: u64, pub shared: u64, pub tasks_limit: u32, pub endpoints_limit: u32,
    pub frames: u64, pub frames_free: u64, // the frame pool for task memory (issue 150): images, stacks, screens, heaps, objects
}
// Physical layout: firmware memory map entries (kind = UEFI memory type) and the platform layout (kind >= PHYS_PLATFORM).
pub const PHYS_PLATFORM: u32 = 0x100; pub const PHYS_ARENA: u32 = 0x100; pub const PHYS_FRAMEBUFFER: u32 = 0x101;
pub const PHYS_BOOT_IMAGE: u32 = 0x102; pub const PHYS_AP_TRAMPOLINE: u32 = 0x103; pub const PHYS_PCI_BAR: u32 = 0x104; pub const PHYS_KERNEL: u32 = 0x105;
// DMA regions are not listed: they are mapped into a driver, and physical addresses of task memory are never exported.
#[derive(Clone, Copy, Default, Debug)] #[repr(C)] pub struct StatPhys { pub kind: u32, pub index: u32, pub start: u64, pub pages: u64 }
// Address-space regions of a task (VMAP).
pub const REGION_IMAGE: u32 = 1; pub const REGION_STACK: u32 = 2; pub const REGION_SCREEN: u32 = 3; pub const REGION_INFO: u32 = 4;
pub const REGION_MAILBOX: u32 = 5; pub const REGION_EXIT: u32 = 6; pub const REGION_HEAP: u32 = 7; pub const REGION_SHARED: u32 = 8; pub const REGION_DEVICE: u32 = 9;
pub const REGION_GUARD: u32 = 10; // unmapped on purpose (below the stack); no rights
pub const REGION_READ: u32 = 1; pub const REGION_WRITE: u32 = 2; pub const REGION_EXECUTE: u32 = 4;
#[derive(Clone, Copy, Default, Debug)] #[repr(C)] pub struct StatRegion { pub start: u64, pub size: u64, pub kind: u32, pub flags: u32 }
// `endpoint`: the endpoint index of an endpoint capability (a label, as in StatEndpoint), 0 for other kinds.
#[derive(Clone, Copy, Default, Debug)] #[repr(C)] pub struct StatCap { pub slot: u32, pub generation: u32, pub kind: u32, pub rights: u32, pub size: u64, pub badge: u32, pub endpoint: u32, pub node: u64, pub parent: u64 }
// A holder (`server`, `holder`) is the task with the most recently derived copy of the capability: a driver rather than
// init, which keeps the copies it granted; `holders` counts the tasks holding one. `irq`: the line bound, 0 if none.
#[derive(Clone, Copy, Default, Debug)] #[repr(C)] pub struct StatEndpoint { pub index: u32, pub receivers: u32, pub waiting_senders: u32, pub waiting_receivers: u32, pub creator: u64, pub messages: u64, pub busy: u64, pub timeouts: u64,
    pub server: u64, pub holders: u32, pub irq: u32 }
// `endpoints`: every endpoint bound to the line (0: none); `endpoint` is the first (issue 159).
#[derive(Clone, Copy, Default, Debug)] #[repr(C)] pub struct StatIrq { pub line: u32, pub endpoint: u32, pub masked: u32, pub holders: u32, pub holder: u64, pub count: u64, pub endpoints: [u32; IRQ_SHARERS] }
// `location`: bus << 8 | device << 3 | function; `io_bars`: bit i set if BAR i is an I/O port range.
#[derive(Clone, Copy, Default, Debug)] #[repr(C)] pub struct StatDevice { pub class: u32, pub irq: u32, pub bar_sizes: [u64; 6], pub holder: u64, pub location: u32, pub io_bars: u32 }
