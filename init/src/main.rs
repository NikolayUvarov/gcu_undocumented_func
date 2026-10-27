#![no_std]
#![no_main]
// init: holds the bootstrap authority (platform privilege) and is the only place with service policy:
// which boot services start, in which order, and exactly which capabilities each one receives. After boot it keeps
// each service's capabilities for restarts and gives up the platform privilege (MC-3.12). As the lifecycle owner it
// restarts failed services within a budget and serves idl/init.wit: start, list, stop and restart services, stop an
// application.
mod hardware;
mod legacy;
mod services;

use mind::abi::*;
use mind::dev::cap_info;
use mind::idl::{init as idl_init, netpolicy, vfs, wire};
use mind::ipc::{self, Endpoint};
use mind::platform;
use mind::effector::policy::{Action, Ask, Deny, Policy};
use mind::process::{grant, grant_moved, Image, Quota};
use mind::sys::{Error, Result};

const ALL: u8 = CAP_READ | CAP_WRITE | CAP_GRANT;
const RECEIVED: usize = 9; // fixed slot for the buffer of an idl/init.wit call
const INIT_PID: u64 = 1; // the kernel's first task
// What each boot service holds, for `svc` (the grants below, in short).
const HOLDS: [&str; BOOT_IMAGES] = [
    "restart and process control", "observe privilege", "ports 0x70-0x71", "ports 0x60, 0x64; IRQ 1 and 12; input",
    "VirtIO input BARs and MSI-X vectors (or IRQs), up to two devices; 24 KiB DMA; input", "framebuffer; display", "ports 0x1F0-0x1F7, 0x3F6", "AHCI registers; 128 KiB DMA",
    "xHCI registers; 512 KiB DMA", "a USB client for mass storage interfaces", "a USB client for HID interfaces; input", "VirtIO block BAR; 128 KiB DMA", "NVMe registers; 128 KiB DMA",
    "8 MiB of memory", "write clients of the block devices", "a USB client for serial converters", "the serial converter's client; a VFS client confined to data/link/; a log reader; an application's loader client", "a write client of its own RAM disk (ramdisk#1)", "pin controller registers; a VFS client", "the embedded controller's two ports (ECDT)",
    "spawn privilege", "HD Audio registers and line, a second function's too; or AC97 ports and IRQ; DMA", "an audio client", "a VFS client (video/synthetic) and a display client (the camera mark)",
    "network card BAR and MSI-X vector (or ports and IRQ); 160 KiB DMA",
    "RTL8168 BAR2 initially non-mastering; 2 MiB DMA; MSI if available; net endpoint",
    "RTL8852BE BAR2 initially non-mastering; retained 8 MiB disjoint command/RX DMA; read-only VFS; MAC/RX diagnostic, no radio endpoint",
    "RTL8821CE BAR2 initially non-mastering; 1 MiB DMA below 4 GiB; read-only VFS for its firmware; a wlanhw endpoint for wlan; polled, no interrupt",
    "Broadcom Wi-Fi BAR0; 512 KiB DMA; its MSI (or INTx line); a read-only VFS client (its microcode); a server endpoint for idl/wlanhw.wit", "the TPM's registers (if the firmware names a TPM)",
    "an RTC client, a VFS client for its own directory, a TPM client that may seal; the device key in memory", "nothing but its endpoint (a test radio)",
    "the radio's client; keystore's client for the station's keys", "clients of the network card drivers and of the Wi-Fi station",
    "network stack clients: minting source and policy control; a VFS client", "nothing but its endpoint", "the key service's signer client; RTC and VFS clients", "the key service's client for SSH logins; no network of its own", "its own program client",
    "observe privilege", "TLS and VFS clients; a network grant; a lifecycle client that may restart the machine; the firmware's variables; a loader client for helpers only",
    "the log to read, sysinfo, the screen to capture and the volumes to read; its private directory; a lifecycle client limited to the owner's policy; a scope of its delivery root to write; only in a mode",
    "RTC; a network grant; TLS without the device certificate; the parser; the gateway; its private directory and data/effector/ to read; only in a mode",
    "screen; process control; input; the serial line"];
const CLIENT: u8 = CAP_WRITE | CAP_GRANT;
// DMA buffer sizes of the drivers; the regions are minted once and survive driver restarts.
const SERVICE_QUOTA: u16 = 256; // tasks and endpoints init keeps for the services and their restarts (issue 171)
const AHCI_DMA_BYTES: usize = 128 * 1024; // commands, FIS and a 64 KiB data buffer
const NVME_DMA_BYTES: usize = 128 * 1024; // queues, identify page, PRP list and a 64 KiB data buffer
const VIRTIO_BLK_DMA_BYTES: usize = 128 * 1024; // the virtqueue, request headers and a 64 KiB data buffer
const XHCI_DMA_BYTES: usize = 512 * 1024; // rings, contexts, scratchpad, a 64 KiB data buffer and a pool of pages (usb_host)
const AUDIO_DMA_BYTES: usize = (33 + 17) * 4096; // playback: 32 buffers + list; capture: 16 buffers + list
const HDA_DMA_BYTES: usize = (32 + 16 + 3) * 4096; // the same rings, two buffer lists, CORB and RIRB (551-DRV-0010)
const NET_DMA_BYTES: usize = 160 * 1024; // two virtqueues (64 KiB) and 48 frame buffers of 2 KiB
const RTL8168_DMA_BYTES: usize = 2 * 1024 * 1024;
const RTL8852BE_DMA_BYTES: usize = 8 * 1024 * 1024;
const WIFI_DMA_BYTES: usize = 512 * 1024; // bcm_wifi: the 64-bit descriptor rings and their frame buffers (550-DRV-0025)
const RTL8821CE_DMA_BYTES: usize = 1024 * 1024; // rtw88_wifi: rings and buffers, from the arena below 4 GiB for its 32-bit DMA (550-KRN-0172)
// The Wi-Fi station's slots (550-KRN-0097): the radio's client and keystore's in wlan, wlan's client in netstack (its third card).
const WIFI_RADIO: usize = 2;
const WIFI_KEYS: usize = 3;
const WIFI_CARD: usize = 4;
// The radios wlan may drive (idl/wlanhw.wit): the MacBook Pro's bcm_wifi (550-DRV-0027), the DEXP's rtw88_wifi
// (550-KRN-0172), else QEMU's stand-in.
const RADIOS: [&str; 3] = ["bcm_wifi", "rtw88_wifi", "wlansim"];
const INPUT_DMA_BYTES: usize = 24 * 1024; // per device 12 KiB: the event queue (two pages), then up to 64 events of 8 bytes
const SLOT_INPUT_IRQ1: usize = 7; // virtio_input: the second device's interrupt
const SLOT_AUDIO_IRQ1: usize = 7; // audio_gw: the second HD Audio function's interrupt
const WINDOWS_MEMORY_MIB: u16 = 128; // the window broker's surfaces (issue 163)
const BLOCKSTORE_MEMORY_MIB: u16 = 64; // the block store's index: up to 2^20 slots of 56 bytes (251-KRN-0071)
const EFFECTOR_MEMORY_MIB: u16 = 48; // the agent's benchmark: a 16 MiB chase and two 8 MiB copy buffers (750-KRN-0158)
const RECOVERY_RESERVE_MIB: usize = 32; // frame pool kept for services and their restarts (issue 169)
const CALL_LIMIT_MS: u32 = 10_000; // every call init makes to a service ends within this, answered or not (000-KRN-0075)
const MOUNT_LIMIT_MS: u32 = 60_000; // the mount and the flush before a restart: a slow USB stick takes seconds
const LINK_DIR: &str = "data/link"; // the only files the link to another machine reaches (800-KRN-0128)
const REPLACE_MAX: usize = 16 << 20; // the largest image a service may be replaced with from data/link/ (800-KRN-0144)
const MODE_FILE: &str = "data/mode.conf"; // the machine's modes, kept across restarts (750-KRN-0143)
const MODE_SERVICES: [&str; 2] = ["effector_gw", "effector"]; // run only in a mode (750-EFF-0013), the gateway first

// Capabilities minted for a service's first start; kept by init for restarts, or dropped if the spawn fails.
struct Minted { slots: [usize; SPAWN_GRANTS_MAX], count: usize }
impl Minted {
    fn new() -> Self { Self { slots: [0; SPAWN_GRANTS_MAX], count: 0 } }
    fn mint(&mut self, kind: usize, a: usize, b: usize) -> Result<usize> {
        let slot = platform::cap(kind, a, b)?;
        self.slots[self.count] = slot; self.count += 1;
        Ok(slot)
    }
    // A client endpoint with a badge the server checks (the write right of a block device, the user's files, reading the log).
    fn badged(&mut self, keeper: usize, rights: u8, badge: u16) -> Result<usize> {
        let slot = ipc::mint_badged(keeper, rights, badge)?;
        self.slots[self.count] = slot; self.count += 1;
        Ok(slot)
    }
    fn ports(&mut self, base: usize, count: usize) -> Result<usize> { self.mint(PLATFORM_PORTS, base, count) }
    // The real-time clock: CMOS ports (LEGACY) on x86, the board's PL031 on aarch64.
    fn clock(&mut self) -> Result<usize> { if cfg!(target_arch = "aarch64") { self.mint(PLATFORM_MMIO, PLATFORM_RTC, 0) } else { self.ports(0x70, 2) } }
    // The platform's serial line: COM1 (LEGACY) on x86, the board's UART on aarch64.
    fn serial(&mut self) -> Result<usize> { if cfg!(target_arch = "aarch64") { self.mint(PLATFORM_MMIO, PLATFORM_UART, 0) } else { self.ports(0x3F8, 8) } }
    // Kept in escrow: init grants it to the service and cannot use it itself (issue 170).
    fn privilege(&mut self, kind: usize) -> Result<usize> { self.mint(PLATFORM_PRIVILEGE, kind, PRIVILEGE_ESCROW) }
}
impl Drop for Minted { fn drop(&mut self) { for &slot in &self.slots[..self.count] { let _ = ipc::drop_cap(slot); } } }

// A service gets copies of what init keeps: children init can revoke, and the same set again after a restart.
#[derive(Clone, Copy)]
struct Grants { list: [Grant; SPAWN_GRANTS_MAX], count: usize }
impl Grants {
    fn new() -> Self { Self { list: [Grant::default(); SPAWN_GRANTS_MAX], count: 0 } }
    fn add(&mut self, child: usize, own: usize, rights: u8) { self.list[self.count] = grant(child, own, rights); self.count += 1; }
    fn copy(&mut self, child: usize, own: usize, rights: u8) { self.add(child, own, rights); }
}
// How a service was started: its grants (over capabilities init keeps), spawn flags, quota and arguments.
#[derive(Clone, Copy)]
struct Plan { grants: Grants, flags: usize, quota: Quota, args: ([u8; ARGS_KEPT], usize) }
const ARGS_KEPT: usize = LINK_WORDS_MAX; // link.conf's words at most (800-KRN-0128)

// What this boot did with a service (173-KRN-0035, init.wit 1.9 `boot-plan`): its place in the order tried (0: not
// tried), the unit it waited for, and why it did not start.
#[derive(Clone, Copy, Default)]
struct Placed { place: u8, after: Option<usize>, off: Off }
#[derive(Clone, Copy, Default, PartialEq)]
enum Off { #[default] No, Disabled, Needs(usize), NoDevice, NoImage, Safe, Mode, NoLink, Failed }

// Per boot service: PID, DMA region and the keeper of its endpoint (can mint receive rights, cannot receive itself);
// how often it was started, whether it was stopped on request (then it is not restarted) and whether its device was
// missing at boot.
struct Init { plans: [Option<Plan>; UNITS], pids: [u64; UNITS], dma: [Option<usize>; UNITS], devices: [Option<usize>; UNITS], others: [Option<usize>; UNITS], keepers: [Option<usize>; UNITS], restarts: [[u64; RESTART_BUDGET]; UNITS], quarantined: [bool; UNITS], starts: [u32; UNITS], stopped: [bool; UNITS], missing: [bool; UNITS], screen_mib: u16, store: Option<Option<&'static str>>,
              link: Option<[u8; LINK_WORDS_MAX]>,
              boot: idl_init::Boot, modes: idl_init::ModeState, link_console: bool,
              // What a service runs in place of its boot image until the next boot, in developer mode (800-KRN-0144): a
              // memory object init holds unmapped, and the image's length.
              replaced: [Option<(usize, usize)>; UNITS],
              placed: [Placed; UNITS], configuration: idl_init::Configuration }

// Services: one per boot image, then the further instances (SERVICE_INSTANCES). A unit index names one of them.
const UNITS: usize = BOOT_IMAGES + SERVICE_INSTANCES.len();
fn unit_name(unit: usize) -> &'static str { if unit < BOOT_IMAGES { BOOT_SERVICES[unit] } else { SERVICE_INSTANCES[unit - BOOT_IMAGES] } }
// The image a unit runs and which instance of it it is (0: the image's own service).
fn unit_image(unit: usize) -> (usize, usize) {
    let name = unit_name(unit);
    match name.split_once('#') { Some((image, n)) => (service_index(image), n.parse().unwrap_or(0)), None => (unit, 0) }
}

// Restart budget (MC-6.5): at most RESTART_BUDGET automatic restarts of one service within RESTART_WINDOW_MS.
const RESTART_BUDGET: usize = 3;
const RESTART_WINDOW_MS: u64 = 60_000;

impl Init {
    fn running(&self, index: usize) -> bool { self.pids[index] != 0 && mind::process::alive(self.pids[index]) }

    // Keeper of the endpoint of service `name`, created on first use and kept across restarts, so clients granted
    // earlier reach the restarted server; while no server runs, sends fail with ERR_PEER. It alone may badge clients:
    // what init hands out lacks CAP_BADGE unless the grant names it (000-KRN-0073).
    fn keeper(&mut self, name: &str) -> Result<usize> {
        let index = service_index(name);
        if let Some(slot) = self.keepers[index] { return Ok(slot); }
        let root = Endpoint::create()?.0;
        let keeper = ipc::mint(root, CAP_KEEP | CAP_WRITE | CAP_GRANT | CAP_BADGE, 0, 0);
        let _ = ipc::drop_cap(root);
        let keeper = keeper?;
        self.keepers[index] = Some(keeper);
        Ok(keeper)
    }
    // The server's receive capability is minted fresh for each instance and moved into it (see `spawn`): init keeps
    // only the keeper, so a dead or quarantined server's clients get ERR_PEER and nothing queues for the next one.
    fn server(&mut self, _minted: &mut Minted, name: &str) -> Result<usize> { self.keeper(name) }
    // A client of service `name` in the child's `slot`: a copy of the keeper narrowed to send rights, so init needs no
    // slot of its own for it (only badged clients are minted and kept).
    fn lend(&mut self, grants: &mut Grants, slot: usize, name: &str) -> Result<()> { let keeper = self.keeper(name)?; grants.copy(slot, keeper, CLIENT); Ok(()) }
    // A flow grant the policy broker makes for `program` (idl/netpolicy.wit), kept for restarts; None when the broker does
    // not run or its policy names nothing for the program.
    fn flow(&mut self, minted: &mut Minted, program: &str) -> Option<usize> {
        if !self.running(service_index("netpolicy")) { return None; }
        let client = ipc::mint_badged(self.keeper("netpolicy").ok()?, CLIENT, 0).ok()?;
        let broker = Endpoint(client);
        let prepared = netpolicy::prepare(broker, program);
        if matches!(prepared, Err(Error::Other(ERR_TIMEOUT))) { mind::println!("[INIT] {}: THE POLICY BROKER DID NOT ANSWER IN {} S", program, CALL_LIMIT_MS / 1000); }
        let taken = prepared.ok().and_then(|r| r.ok()).is_some_and(|badge| matches!(netpolicy::take(broker, badge, RECEIVED), Ok(Ok(()))));
        let _ = ipc::drop_cap(client);
        if !taken { return None; }
        let slot = ipc::mint(RECEIVED, u8::MAX, 0, 0).ok();
        let _ = ipc::drop_cap(RECEIVED);
        let slot = slot?;
        minted.slots[minted.count] = slot; minted.count += 1;
        Some(slot)
    }
    // The block store's own disk (300-KRN-0025): a VirtIO disk that is blank or starts with the store's superblock;
    // looked at once, through a client of init's own, before vfs_server or the store gets any disk.
    fn store_disk(&mut self) -> Option<&'static str> {
        if let Some(found) = self.store { return found; }
        let mut found = None;
        for name in ["virtio_blk", "virtio_blk#1", "virtio_blk#2"] {
            if !self.running(service_index(name)) { continue; }
            let Ok(keeper) = self.keeper(name) else { continue };
            let Ok(client) = ipc::mint_badged(keeper, CLIENT, 0) else { continue };
            let first = match mind::block::Device::open(Endpoint(client)) {
                Ok(mut disk) => disk.read(0, 1).ok().map(|s| (s.iter().all(|&b| b == 0), s.starts_with(b"MIND-STO"))),
                Err(Error::Other(ERR_TIMEOUT)) => { mind::println!("[INIT] {} DID NOT ANSWER IN {} S: NOT THE BLOCK STORE'S DISK", name, CALL_LIMIT_MS / 1000); None }
                Err(_) => None,
            };
            let _ = ipc::drop_cap(client);
            if let Some((blank, store)) = first.filter(|&(blank, store)| blank || store) {
                mind::println!("[INIT] THE BLOCK STORE'S DISK: {} ({})", name, if blank { "BLANK" } else if store { "THE STORE'S" } else { "?" });
                found = Some(name);
                break;
            }
        }
        self.store = Some(found);
        found
    }
    // Whether vfs_server serves the boot volume: its root opens through a client of init's own.
    fn boot_volume_mounted(&mut self) -> bool {
        let Ok(keeper) = self.keeper("vfs_server") else { return false };
        let Ok(client) = ipc::mint_badged(keeper, CLIENT, 0) else { return false };
        let endpoint = Endpoint(client);
        let mounted = match mind::idl::vfs::root(endpoint, "") { Ok(Ok(handle)) => { let _ = mind::idl::vfs::close(endpoint, handle); true } _ => false };
        let _ = ipc::drop_cap(client);
        mounted
    }
    fn badged(&mut self, minted: &mut Minted, name: &str, badge: u16) -> Result<usize> { let keeper = self.keeper(name)?; minted.badged(keeper, CLIENT, badge) }
    // The keyboard service the shell's keymap talks to: the PS/2 driver if the machine has the controller, else the USB
    // HID driver, else the VirtIO one.
    fn keyboard(&self) -> &'static str { ["ps2_kbd", "usb_hid", "virtio_input"].into_iter().find(|&n| self.running(service_index(n))).unwrap_or("virtio_input") }

    // Before a driver is restarted (MC-6.3): its device stops DMA, then the DMA region is cleared, so the new instance
    // starts from a quiet device and no residue of the old one.
    fn quiesce(&mut self, index: usize) -> Result<()> {
        if self.running(index) {
            self.quarantined[index] = true;
            mind::println!("[INIT] {} STILL RUNNING: DMA RETAINED, NO RESTART", unit_name(index));
            return Err(Error::Other(ERR_BUSY));
        }
        // The driver's second device too (virtio_input's keyboard, audio_gw's second HD Audio function): one DMA region.
        for device in [self.devices[index], self.others[index]].into_iter().flatten() {
            if let Err(error) = platform::quiesce(device) {
                self.quarantined[index] = true;
                mind::println!("[INIT] {} QUIESCE FAILED: {:?}; DMA RETAINED, NO RESTART", unit_name(index), error);
                return Err(error);
            }
            mind::println!("[INIT] {} DEVICE QUIESCED", unit_name(index));
        }
        if let Some(slot) = self.dma[index] {
            match mind::mem::Mapping::new(slot) {
                Ok(mut region) => region.as_mut_slice().fill(0),
                Err(error) => {
                    self.quarantined[index] = true;
                    mind::println!("[INIT] {} DMA CLEAR FAILED: {:?}; NO RESTART", unit_name(index), error);
                    return Err(error);
                }
            }
        }
        Ok(())
    }

    fn wired_cards(&self) -> [Option<&'static str>; 2] {
        let mut result = [None; 2]; let mut count = 0;
        for name in ["virtio_net", "virtio_net#1", "rtl8168"] {
            if self.plans[Self::index(name).unwrap()].is_some() && count < result.len() { result[count] = Some(name); count += 1; }
        }
        result
    }

    fn dma(&mut self, index: usize, bytes: usize) -> Result<usize> {
        if let Some(slot) = self.dma[index] { return Ok(slot); }
        let slot = platform::cap(PLATFORM_DMA, bytes, 0)?;
        self.dma[index] = Some(slot);
        Ok(slot)
    }

    // A PCI device BAR of the expected kind (port range or MMIO), or NotFound.
    fn bar(minted: &mut Minted, device: usize, bar: usize, kind: usize) -> Result<usize> {
        // A BAR that shared its page with another kind of device moves when granted (211-KRN-0021): logged here, as a
        // machine without COM1 shows no kernel line by then.
        let at = || mind::dev::device_config_at(SLOT_DEV0, device, 0x10 + 4 * bar).map(|v| v & !0xF).unwrap_or(0);
        let before = at();
        let slot = minted.mint(PLATFORM_DEVICE_BAR, device, bar)?;
        let after = at();
        if after != before { mind::println!("[INIT] DEVICE {} BAR {}: REGISTERS MOVED FROM {:08X} TO {:08X}, A PAGE OF THEIR OWN", device, bar, before, after); }
        if cap_info(slot).0 == kind { Ok(slot) } else { Err(Error::NotFound) }
    }

    // Starts boot service `index` with its capabilities; Err(NotFound) if its hardware is absent.
    fn start(&mut self, index: usize) -> Result<u64> {
        let name = unit_name(index);
        if index == 0 || self.running(index) { return Err(Error::Other(ERR_BUSY)); }
        if let Some(plan) = self.plans[index] {
            // A restart, or a start a safe start prepared: the device was stopped; the same capabilities are granted.
            if let Some(device) = self.devices[index] {
                if matches!(name, "rtl8168" | "rtw89_wifi" | "rtw88_wifi") {
                    platform::registers_only(device)?;
                } else { platform::resume(device)?; }
            }
            if let Some(device) = self.others[index] { platform::resume(device)?; }
            return self.spawn(index, plan);
        }
        let (plan, mut minted) = self.plan(index)?;
        let pid = self.spawn(index, plan)?;
        minted.count = 0; // kept for restarts
        self.plans[index] = Some(plan);
        Ok(pid)
    }

    // A service a safe start leaves out (000-KRN-0131): its capabilities are minted while init holds the platform
    // privilege and kept, its devices stopped, so that `svc start` can start it once the privilege is dropped.
    fn prepare(&mut self, index: usize) -> Result<()> {
        let (plan, mut minted) = self.plan(index)?;
        minted.count = 0;
        self.plans[index] = Some(plan);
        self.stopped[index] = true;
        for device in [self.devices[index], self.others[index]].into_iter().flatten() {
            if let Err(error) = platform::quiesce(device) { mind::println!("[INIT] {} QUIESCE FAILED: {:?}", unit_name(index), error); }
        }
        Ok(())
    }
    // Prepared by a safe start or left off by data/services.txt, and not started since.
    fn prepared(&self, index: usize) -> bool { self.plans[index].is_some() && self.pids[index] == 0 }
    // Running, or prepared: its clients answer ERR_PEER until `svc start` starts it.
    fn present(&self, name: &str) -> bool { let index = service_index(name); self.running(index) || self.prepared(index) }

    // Makes `dir` on the boot volume if it is missing, through a client with `badge` that init drops at once.
    fn make_dir(&mut self, badge: u16, dir: &str) -> Result<()> {
        let client = ipc::mint_badged(self.keeper("vfs_server")?, CLIENT, badge)?;
        let files = Endpoint(client);
        let made = (|| -> Result<()> {
            let root = vfs::root(files, "")?.map_err(|_| Error::NotFound)?;
            let dir = vfs::open_dir(files, root, dir, true);
            let _ = vfs::close(files, root);
            let _ = vfs::close(files, dir?.map_err(|_| Error::Rights)?);
            Ok(())
        })();
        let _ = ipc::drop_cap(client);
        made
    }

    // A client confined to `dir` on the boot volume, made if missing, writable or not (750-KRN-0120, for 800-KRN-0128
    // and 750-KRN-0119), asked of vfs_server through a client with `badge` that init drops at once.
    fn scope(&mut self, badge: u16, dir: &str, writable: bool) -> Result<usize> {
        let keeper = self.keeper("vfs_server")?;
        let client = ipc::mint_badged(keeper, CLIENT, badge)?;
        let files = Endpoint(client);
        let scoped = (|| -> Result<usize> {
            let root = vfs::root(files, "")?.map_err(|_| Error::NotFound)?;
            let dir = vfs::open_dir(files, root, dir, true);
            let _ = vfs::close(files, root);
            let dir = dir?.map_err(|_| Error::NotFound)?;
            let made = vfs::scope(files, dir, writable, RECEIVED);
            let _ = vfs::close(files, dir);
            made?.map_err(|_| Error::Rights)?;
            let slot = ipc::mint(RECEIVED, u8::MAX, 0, 0);
            let _ = ipc::drop_cap(RECEIVED);
            Ok(slot?)
        })();
        let _ = ipc::drop_cap(client);
        scoped
    }

    // The capabilities boot service `index` starts with, and what was minted for them; Err(NotFound) if its hardware is
    // absent.
    fn plan(&mut self, index: usize) -> Result<(Plan, Minted)> {
        let name = unit_name(index);
        let (image, instance) = unit_image(index);
        let mut minted = Minted::new();
        let mut grants = Grants::new();
        let mut flags = SPAWN_SERVICE;
        let mut args = mind::util::FixedBuf::<ARGS_KEPT>::new();
        match BOOT_SERVICES[image] {
            // LEGACY: CMOS RTC (ISA ports), PS/2 keyboard controller and primary IDE channel below (docs/legacy.md).
            "rtc" => { grants.add(SLOT_SERVICE, self.server(&mut minted, "rtc")?, ALL); grants.add(SLOT_DEV0, minted.clock()?, 0); }
            "ps2_kbd" => {
                // No controller (Intel Macs, many newer machines): its status port reads all ones (issue 164).
                let status = minted.ports(0x64, 1)?;
                if mind::dev::Ports(status).in8(0x64) == 0xFF { return Err(Error::NotFound); }
                grants.add(SLOT_SERVICE, self.server(&mut minted, "ps2_kbd")?, ALL); // requests from the shell's keyboard client (151)
                grants.add(SLOT_DEV0, minted.ports(0x60, 1)?, 0); grants.add(SLOT_DEV1, status, 0);
                grants.add(SLOT_IRQ, minted.mint(PLATFORM_IRQ, 1, 0)?, 0); grants.add(SLOT_PRIV, minted.privilege(CAP_KIND_INPUT)?, 0);
                grants.add(SLOT_MEM, minted.mint(PLATFORM_IRQ, 12, 0)?, 0); // the mouse on the auxiliary port (issue 156)
            }
            // VirtIO input devices (issues 161, 202): the first two (a tablet, a keyboard), each its MMIO BAR and MSI-X
            // vector or line; the input privilege; one DMA region the driver halves.
            "virtio_input" => {
                let mut found = 0;
                for (nth, (bar_slot, irq_slot)) in [(SLOT_DEV0, SLOT_IRQ), (SLOT_DEV1, SLOT_INPUT_IRQ1)].into_iter().enumerate() {
                    let Ok(device) = platform::find_device_id(0, 0, 0x1052_1AF4, nth) else { break };
                    if nth == 0 { self.devices[index] = Some(device); }
                    let probe = (0..6).find_map(|bar| platform::cap(PLATFORM_DEVICE_BAR, device, bar).ok());
                    let Some(bar) = probe.and_then(|slot| { let layout = mind::virtio::Layout::read(slot); let _ = ipc::drop_cap(slot); layout }).and_then(|l| l.single_bar()) else { continue };
                    grants.add(bar_slot, Self::bar(&mut minted, device, bar as usize, CAP_KIND_MMIO)?, 0);
                    grants.add(irq_slot, minted.mint(PLATFORM_DEVICE_MSIX, device, 0).or_else(|_| minted.mint(PLATFORM_DEVICE_IRQ, device, 0))?, 0);
                    if nth == 1 { self.others[index] = Some(device); }
                    found += 1;
                }
                if found == 0 { return Err(Error::NotFound); }
                grants.add(SLOT_PRIV, minted.privilege(CAP_KIND_INPUT)?, 0);
                grants.add(SLOT_SERVICE, self.server(&mut minted, name)?, ALL); grants.copy(SLOT_MEM, self.dma(index, INPUT_DMA_BYTES)?, 0);
            }
            "compositor" => {
                grants.add(SLOT_SERVICE, self.server(&mut minted, "compositor")?, ALL); // requests from the shell's display client (151)
                grants.add(SLOT_MEM, minted.mint(PLATFORM_FRAMEBUFFER, 0, 0)?, 0); grants.add(SLOT_PRIV, minted.privilege(CAP_KIND_DISPLAY)?, 0);
            }
            "ata" => {
                grants.add(SLOT_SERVICE, self.server(&mut minted, "ata")?, ALL);
                grants.add(SLOT_DEV0, minted.ports(0x1F0, 8)?, 0); grants.add(SLOT_DEV1, minted.ports(0x3F6, 1)?, 0);
            }
            "ahci" => {
                // First SATA controller in AHCI mode (class 01:06:01): ABAR is BAR5.
                let device = platform::find_device(0x01_06_01, 0xFF_FF_FF, 0)?; self.devices[index] = Some(device);
                grants.add(SLOT_DEV0, Self::bar(&mut minted, device, 5, CAP_KIND_MMIO)?, 0);
                grants.add(SLOT_SERVICE, self.server(&mut minted, "ahci")?, ALL); grants.copy(SLOT_MEM, self.dma(index, AHCI_DMA_BYTES)?, 0);
            }
            "usb_host" => {
                // First xHCI controller (class 0C:03:30): registers in BAR0 (issue 164).
                let device = platform::find_device(0x0C_03_30, 0xFF_FF_FF, 0)?; self.devices[index] = Some(device);
                grants.add(SLOT_DEV0, Self::bar(&mut minted, device, 0, CAP_KIND_MMIO)?, 0);
                grants.add(SLOT_SERVICE, self.server(&mut minted, name)?, ALL); grants.copy(SLOT_MEM, self.dma(index, XHCI_DMA_BYTES)?, 0);
                // EHCI controllers (class 0C:03:20, 211-DRV-0004): an Intel Mac's internal keyboard and trackpad sit
                // behind them; each its BAR0 and its own DMA region.
                for (nth, &(bar_slot, dma_slot)) in mind::usb::EHCI.iter().enumerate() {
                    let Ok(ehci) = platform::find_device(0x0C_03_20, 0xFF_FF_FF, nth) else { break };
                    let bar = match Self::bar(&mut minted, ehci, 0, CAP_KIND_MMIO) { Ok(bar) => bar, Err(e) => {
                        let at = mind::dev::device_config_at(SLOT_DEV0, ehci, 0x10).unwrap_or(0) & !0xF;
                        mind::println!("[INIT] EHCI {}: REGISTERS AT {:08X} NOT GRANTED ({:?})", nth, at, e); continue
                    } };
                    grants.add(bar_slot, bar, 0); grants.add(dma_slot, platform::cap(PLATFORM_DMA, mind::usb::EHCI_DMA_BYTES, 0)?, 0);
                }
            }
            // USB class drivers: a client of usb_host whose badge names the one class it may claim.
            "usb_storage" | "usb_hid" => {
                if !self.running(service_index("usb_host")) { return Err(Error::NotFound); }
                let badge = if name == "usb_hid" { mind::usb::BADGE_HID } else { mind::usb::BADGE_STORAGE };
                grants.add(SLOT_DEV0, self.badged(&mut minted, "usb_host", badge)?, CLIENT);
                grants.add(SLOT_SERVICE, self.server(&mut minted, name)?, ALL);
                if name == "usb_hid" { grants.add(SLOT_PRIV, minted.privilege(CAP_KIND_INPUT)?, 0); }
            }
            // The link to another machine (800-KRN-0128), started only with link.conf at the boot volume's root: the
            // serial converters' driver holds usb_host's client badged for them, and its endpoint.
            "usb_serial" => {
                if !self.running(service_index("usb_host")) { return Err(Error::NotFound); }
                grants.add(SLOT_DEV0, self.badged(&mut minted, "usb_host", mind::usb::BADGE_SERIAL)?, CLIENT);
                grants.add(SLOT_SERVICE, self.server(&mut minted, name)?, ALL);
            }
            // The link itself: the converter's client, which it alone holds (MC-2.3), a reader of the log (below) and
            // link.conf's words; its files, a client confined to data/link/, come with each start (`spawn`).
            "link" => {
                if !self.running(service_index("usb_serial")) { return Err(Error::NotFound); }
                self.lend(&mut grants, SLOT_DEV0, "usb_serial")?;
                // An application's loader client: link starts programs from data/link/ for the master (800-KRN-0132).
                self.lend(&mut grants, SLOT_LOADER, "loader")?;
                // The compositor's client that lights the link's mark on the screen, and nothing else of it (800-KRN-0156).
                grants.add(SLOT_DISPLAY, self.badged(&mut minted, "compositor", mind::display::BADGE_REMOTE_LINK)?, CLIENT);
                let words = self.link.unwrap_or([0; LINK_WORDS_MAX]);
                let length = words.iter().position(|&b| b == 0).unwrap_or(words.len());
                let words = core::str::from_utf8(&words[..length]).unwrap_or("");
                let _ = core::fmt::Write::write_str(&mut args, words);
                // Developer mode, link.conf's word `dev` (800-KRN-0139, the maintainer's decision of 2026-10-21): what the
                // shell lends its programs without asking, so whoever holds the cable may debug the machine as its user.
                // The servers start after link: their keepers are made now and reached once they run.
                if self.link_dev() {
                    self.lend(&mut grants, SLOT_SYSINFO, "sysmon")?;
                    // The policy broker's client that may only launch (800-KRN-0142): link has no user to ask before a change.
                    grants.add(SLOT_NETPOLICY, self.badged(&mut minted, "netpolicy", mind::network::BADGE_LAUNCH)?, CLIENT);
                    grants.add(SLOT_WIFI, self.badged(&mut minted, "wlan", mind::network::BADGE_WIFI_CHANGE)?, CLIENT);
                    // Init's lifecycle client that may reboot, as the updater's (351-KRN-0022), and replace a service with an
                    // image from data/link/ (800-KRN-0144).
                    grants.add(SLOT_LIFECYCLE, minted.badged(SLOT_SERVICE, CLIENT, mind::process::BADGE_DEVELOPER)?, CLIENT);
                    // The shell's commands, badged so that the shell applies the link's rule to them (800-KRN-0148).
                    grants.add(SLOT_SHELL, self.badged(&mut minted, "shell", mind::process::BADGE_SHELL_LINK)?, CLIENT);
                    mind::println!("[INIT] link: DEVELOPER MODE (link.conf: dev): SYSMON, LAUNCH-ONLY NETWORK POLICY, WI-FI, REBOOT AND SHELL CLIENTS");
                }
            }
            "virtio_blk" => {
                // The instance's VirtIO block device (vendor 1AF4, modern-only 1042 or transitional 1001), in PCI order:
                // instance n drives the n-th; its BAR with the modern structures and a DMA region; requests are polled.
                let mut disks: [usize; 8] = [usize::MAX; 8]; let mut found = 0;
                for id in [0x1042_1AF4, 0x1001_1AF4] {
                    for nth in 0.. { match platform::find_device_id(0, 0, id, nth) { Ok(device) if found < disks.len() => { disks[found] = device; found += 1; } _ => break } }
                }
                disks[..found].sort_unstable();
                let device = *disks[..found].get(instance).ok_or(Error::NotFound)?;
                self.devices[index] = Some(device);
                let probe = (0..6).find_map(|bar| platform::cap(PLATFORM_DEVICE_BAR, device, bar).ok());
                let bar = probe.and_then(|slot| { let layout = mind::virtio::Layout::read(slot); let _ = ipc::drop_cap(slot); layout }).and_then(|l| l.single_bar()).ok_or(Error::NotFound)?;
                grants.add(SLOT_DEV0, Self::bar(&mut minted, device, bar as usize, CAP_KIND_MMIO)?, 0);
                grants.add(SLOT_SERVICE, self.server(&mut minted, name)?, ALL); grants.copy(SLOT_MEM, self.dma(index, VIRTIO_BLK_DMA_BYTES)?, 0);
            }
            "rtl8168" | "rtw89_wifi" => {
                let (id, class) = if name == "rtl8168" { (0x8168_10EC, 0x02_00_00) } else { (0xB852_10EC, 0x02_80_00) };
                let device = platform::find_device_id(class, 0xFF_FF_00, id, 0)?;
                self.devices[index] = Some(device);
                let registers = minted.mint(PLATFORM_DEVICE_BAR_IDLE, device, 2)?;
                if cap_info(registers).0 != CAP_KIND_MMIO { return Err(Error::NotFound); }
                grants.add(SLOT_DEV0, registers, 0);
                let _ = core::fmt::Write::write_fmt(&mut args, format_args!("{}", device));
                if name == "rtl8168" {
                    grants.copy(SLOT_MEM, self.dma(index, RTL8168_DMA_BYTES)?, 0);
                    if let Ok(irq) = minted.mint(PLATFORM_DEVICE_MSI, device, 0) { grants.add(SLOT_IRQ, irq, 0); }
                    grants.add(SLOT_SERVICE, self.server(&mut minted, name)?, ALL | CAP_BADGE);
                } else {
                    grants.copy(SLOT_MEM, self.dma(index, RTL8852BE_DMA_BYTES)?, 0);
                    if self.running(service_index("vfs_server")) {
                        grants.add(SLOT_VFS, self.badged(&mut minted, "vfs_server", mind::fs::BADGE_READER)?, CLIENT);
                    }
                }
            }
            "rtw88_wifi" => {
                // The DEXP's Realtek RTL8821CE (10EC:C821, class 02:80): BAR2 with memory decoding only; the driver turns
                // mastering on once its rings are programmed, and its rings are polled, so no interrupt (550-KRN-0172).
                let device = platform::find_device_id(0x02_80_00, 0xFF_FF_00, 0xC821_10EC, 0)?;
                self.devices[index] = Some(device);
                let registers = minted.mint(PLATFORM_DEVICE_BAR_IDLE, device, 2)?;
                if cap_info(registers).0 != CAP_KIND_MMIO { return Err(Error::NotFound); }
                grants.add(SLOT_DEV0, registers, 0);
                let _ = core::fmt::Write::write_fmt(&mut args, format_args!("{}", device));
                grants.copy(SLOT_MEM, self.dma(index, RTL8821CE_DMA_BYTES)?, 0);
                // A reader's client of vfs_server for its firmware in data/firmware/rtw88/.
                if self.running(service_index("vfs_server")) { grants.add(SLOT_VFS, self.badged(&mut minted, "vfs_server", mind::fs::BADGE_READER)?, CLIENT); }
                // Its idl/wlanhw.wit endpoint, whose client goes to wlan alone.
                grants.add(SLOT_SERVICE, self.server(&mut minted, name)?, ALL);
            }
            "bcm_wifi" => {
                // The MacBook Pro's Broadcom BCM4331 (14E4:4331), or a Broadcom network controller of class 02:80:00: BAR0 only (550-KRN-0059).
                let broadcom = |device: &usize| mind::dev::device_config_at(SLOT_DEV0, *device, 0).is_ok_and(|id| id & 0xFFFF == 0x14E4);
                let device = platform::find_device_id(0, 0, 0x4331_14E4, 0).ok()
                    .or_else(|| (0..8).map_while(|nth| platform::find_device(0x02_80_00, 0xFF_FF_FF, nth).ok()).find(broadcom)).ok_or(Error::NotFound)?;
                self.devices[index] = Some(device);
                grants.add(SLOT_DEV0, Self::bar(&mut minted, device, 0, CAP_KIND_MMIO)?, 0);
                // For receiving frames (550-KRN-0091, the drivers track's request for 550-DRV-0025): a DMA region below
                // 4 GiB for its rings and buffers, and its interrupt: the MSI (the card has no MSI-X), else its INTx line.
                // Neither starts the core; the driver sets up its rings first.
                grants.copy(SLOT_MEM, self.dma(index, WIFI_DMA_BYTES)?, 0);
                match minted.mint(PLATFORM_DEVICE_MSI, device, 0).or_else(|_| minted.mint(PLATFORM_DEVICE_IRQ, device, 0)) {
                    Ok(irq) => grants.add(SLOT_IRQ, irq, 0),
                    Err(_) => mind::println!("[INIT] bcm_wifi: NEITHER ITS MSI NOR AN INTX LINE: NO INTERRUPT"),
                }
                // A reader's client of vfs_server for the microcode in data/firmware/b43/ (550-KRN-0061), badged so no other badge can be set on it.
                if self.running(service_index("vfs_server")) { grants.add(SLOT_VFS, self.badged(&mut minted, "vfs_server", mind::fs::BADGE_READER)?, CLIENT); }
                // Its idl/wlanhw.wit endpoint, whose client goes to wlan alone (550-DRV-0027, 550-KRN-0097): with it the driver brings the radio up.
                grants.add(SLOT_SERVICE, self.server(&mut minted, name)?, ALL);
            }
            "nvme" => {
                // The first NVMe controller (class 01:08:02): registers in BAR0; commands are polled, so no interrupt.
                let device = platform::find_device(0x01_08_02, 0xFF_FF_FF, 0)?; self.devices[index] = Some(device);
                grants.add(SLOT_DEV0, Self::bar(&mut minted, device, 0, CAP_KIND_MMIO)?, 0);
                grants.add(SLOT_SERVICE, self.server(&mut minted, name)?, ALL); grants.copy(SLOT_MEM, self.dma(index, NVME_DMA_BYTES)?, 0);
            }
            // Each instance its own disk: ramdisk for vfs_server's ram:, ramdisk#1 for the block store (300-KRN-0001).
            "ramdisk" => grants.add(SLOT_SERVICE, self.server(&mut minted, name)?, ALL),
            // The block store over a RAM disk of its own: its only block client (block::serve has one buffer for all).
            "blockstore" => {
                grants.add(SLOT_SERVICE, self.server(&mut minted, "blockstore")?, ALL);
                // Its own disk when there is one, else the RAM disk whose contents never outlive the boot (300-KRN-0025).
                let disk = match self.store_disk() { Some(disk) => disk, None if self.running(service_index("ramdisk#1")) => "ramdisk#1", None => return Err(Error::NotFound) };
                grants.add(SLOT_DEV0, self.badged(&mut minted, disk, mind::block::BADGE_WRITE)?, CLIENT);
            }
            "vfs_server" => {
                // VFS sees only block devices whose drivers are actually running; it alone may write to them (B.6).
                // It badges the file clients it hands out (idl/vfs.wit), so its endpoint keeps the right to (000-KRN-0073).
                grants.add(SLOT_SERVICE, self.server(&mut minted, "vfs_server")?, ALL | CAP_BADGE);
                let mut slot = SLOT_BLOCK_FIRST;
                let store = self.store_disk(); // the block store's own disk: never a file system's (Appendix B.6)
                for driver in ["ata", "ahci", "usb_storage", "virtio_blk", "virtio_blk#1", "virtio_blk#2", "nvme"] {
                    if Some(driver) == store { continue; }
                    if self.running(service_index(driver)) && slot < SLOT_BLOCK_FIRST + BLOCK_DEVICES { grants.add(slot, self.badged(&mut minted, driver, mind::block::BADGE_WRITE)?, CLIENT); slot += 1; }
                }
                if self.running(service_index("ramdisk")) { grants.add(SLOT_RAMDISK, self.badged(&mut minted, "ramdisk", mind::block::BADGE_WRITE)?, CLIENT); }
                self.lend(&mut grants, SLOT_VFS_RTC, "rtc")?;
            }
            // Pin controllers the firmware's tables name (issue 207): the first BCM2711 GPIO and the first PL061.
            "gpio" => {
                let bcm2711 = minted.mint(PLATFORM_MMIO, PLATFORM_PINS_BCM2711, 0).ok();
                let pl061 = minted.mint(PLATFORM_MMIO, PLATFORM_PINS_PL061, 0).ok();
                if bcm2711.is_none() && pl061.is_none() { return Err(Error::NotFound); }
                if let Some(slot) = bcm2711 { grants.add(SLOT_DEV0, slot, 0); }
                if let Some(slot) = pl061 { grants.add(SLOT_DEV1, slot, 0); }
                grants.add(SLOT_SERVICE, self.server(&mut minted, name)?, ALL);
                grants.add(SLOT_VFS, self.badged(&mut minted, "vfs_server", mind::fs::BADGE_READER)?, CLIENT); // hwdocs/ on the boot disk
            }
            "loader" => {
                grants.add(SLOT_SERVICE, self.server(&mut minted, "loader")?, ALL);
                // Its client of vfs_server, badged for reading, goes to every program it loads.
                self.lend(&mut grants, 2, "rtc")?; grants.add(3, self.badged(&mut minted, "vfs_server", mind::fs::BADGE_READER)?, CLIENT);
                self.lend(&mut grants, 4, "audio_gw")?; grants.add(5, minted.privilege(CAP_KIND_SPAWN)?, 0);
                self.lend(&mut grants, 6, "tts")?;
            }
            "audio_gw" => {
                grants.add(SLOT_SERVICE, self.server(&mut minted, "audio_gw")?, ALL);
                // Intel HD Audio (class 04:03, 551-DRV-0010): BAR0's registers, its line where it has one below 16 (else
                // the gateway polls) and a DMA region. Every PC has it; it wins over AC97.
                if let Ok(device) = platform::find_device(0x04_03_00, 0xFF_FF_00, 0) {
                    self.devices[index] = Some(device);
                    if let Ok(bar) = Self::bar(&mut minted, device, 0, CAP_KIND_MMIO) {
                        grants.add(SLOT_DEV0, bar, 0);
                        if let Ok(irq) = minted.mint(PLATFORM_DEVICE_IRQ, device, 0) { grants.add(SLOT_IRQ, irq, 0); }
                        grants.copy(SLOT_MEM, self.dma(index, HDA_DMA_BYTES)?, 0);
                        // A second HD Audio function (the Ryzen's analog one beside its GPU's HDMI audio): BAR0 and its
                        // line; the gateway drives the first with an output path (213-KRN-0127).
                        if let Ok(second) = platform::find_device(0x04_03_00, 0xFF_FF_00, 1) {
                            if let Ok(bar) = Self::bar(&mut minted, second, 0, CAP_KIND_MMIO) {
                                grants.add(SLOT_DEV1, bar, 0);
                                if let Ok(irq) = minted.mint(PLATFORM_DEVICE_IRQ, second, 0) { grants.add(SLOT_AUDIO_IRQ1, irq, 0); }
                                self.others[index] = Some(second);
                            }
                        }
                    }
                }
                // LEGACY: AC97 (class 04:01): mixer and bus master port ranges and an IRQ line; without it the gateway reports no device.
                else if let Ok(device) = platform::find_device(0x04_01_00, 0xFF_FF_00, 0) {
                    self.devices[index] = Some(device);
                    let devices = (|| -> Result<[usize; 3]> { Ok([Self::bar(&mut minted, device, 0, CAP_KIND_PORTS)?, Self::bar(&mut minted, device, 1, CAP_KIND_PORTS)?, minted.mint(PLATFORM_DEVICE_IRQ, device, 0)?]) })();
                    if let Ok([mixer, bus_master, irq]) = devices {
                        grants.add(SLOT_DEV0, mixer, 0); grants.add(SLOT_DEV1, bus_master, 0); grants.add(SLOT_IRQ, irq, 0);
                        grants.copy(SLOT_MEM, self.dma(index, AUDIO_DMA_BYTES)?, 0);
                    }
                }
            }
            "tts" => { grants.add(SLOT_SERVICE, self.server(&mut minted, "tts")?, ALL); self.lend(&mut grants, SLOT_AUDIO, "audio_gw")?; }
            // The video gateway (issue 158): the boot disk for video/synthetic, the compositor for the camera mark, and
            // usb_host's client for cameras (UVC interfaces only).
            "video_gw" => {
                grants.add(SLOT_SERVICE, self.server(&mut minted, name)?, ALL);
                grants.add(SLOT_VFS, self.badged(&mut minted, "vfs_server", mind::fs::BADGE_READER)?, CLIENT); self.lend(&mut grants, SLOT_DEV0, "compositor")?;
                if self.running(service_index("usb_host")) { grants.add(SLOT_DEV1, self.badged(&mut minted, "usb_host", mind::usb::BADGE_VIDEO)?, CLIENT); }
            }
            // The stack holds only a client of the card driver (B.6): frames, no device.
            "netstack" => {
                grants.add(SLOT_SERVICE, self.server(&mut minted, "netstack")?, ALL);
                for (slot, driver) in [SLOT_DEV0, SLOT_DEV1].into_iter().zip(self.wired_cards()) {
                    if let Some(driver) = driver { self.lend(&mut grants, slot, driver)?; }
                }
                // The Wi-Fi station as its third card: a client without BADGE_WIFI, which serves idl/net.wit (550-KRN-0097).
                // A prepared station's client answers ERR_PEER until it is started (000-KRN-0131).
                if self.present("wlan") { self.lend(&mut grants, WIFI_CARD, "wlan")?; }
            }
            // QEMU's stand-in radio (550-NET-0018): only the tests' images carry it, and it holds nothing but its endpoint.
            "wlansim" => grants.add(SLOT_SERVICE, self.server(&mut minted, name)?, ALL),
            // The 802.11 station (550-NET-0018) over the radio that runs: its client in slot 2, the station's alone (MC-2.3);
            // keystore's client for remembered networks in slot 3 (550-NET-0024). No radio, no station.
            "wlan" => {
                // A radio a safe start prepared serves the station once `svc start` starts it (000-KRN-0131).
                let radio = RADIOS.into_iter().find(|&r| self.running(service_index(r)))
                    .or_else(|| RADIOS.into_iter().find(|&r| self.prepared(service_index(r)))).ok_or(Error::NotFound)?;
                grants.add(SLOT_SERVICE, self.server(&mut minted, name)?, ALL);
                self.lend(&mut grants, WIFI_RADIO, radio)?;
                if self.present("keystore") { grants.add(WIFI_KEYS, self.badged(&mut minted, "keystore", mind::network::BADGE_KEY_STATION)?, CLIENT); }
            }
            // The broker mints flow grants from an unbadged stack client that may badge (which itself may open nothing),
            // registers them through the stack's policy client and reads netpolicy.txt through a VFS client, whose badge
            // opens its private directory for the policy as changed (108, 108-KRN-0041, 000-KRN-0073).
            "netpolicy" => {
                grants.add(SLOT_SERVICE, self.server(&mut minted, "netpolicy")?, ALL);
                grants.copy(2, self.keeper("netstack")?, CLIENT | CAP_BADGE);
                grants.add(3, self.badged(&mut minted, "vfs_server", mind::fs::BADGE_NETPOLICY)?, CLIENT);
                grants.add(4, self.badged(&mut minted, "netstack", mind::network::BADGE_POLICY)?, CLIENT);
            }
            // The parser service holds its endpoint and the log only: it parses outside bytes for others (109-NET-0008).
            "parse" => { grants.add(SLOT_SERVICE, self.server(&mut minted, "parse")?, ALL); }
            // The TPM service alone holds the TPM's registers (351-KRN-0043); without a TPM it runs and says so.
            "tpm" => {
                if let Ok(slot) = minted.mint(PLATFORM_MMIO, PLATFORM_TPM, 0) { grants.add(SLOT_DEV0, slot, 0); }
                grants.add(SLOT_SERVICE, self.server(&mut minted, "tpm")?, ALL);
            }
            // The Smart Battery System behind the embedded controller the ECDT names (211-PRT-0008): its two ports; the
            // SMBus host's base and the AC adapter's bit in the controller's space from the DSDT as arguments.
            "battery" => {
                let (command, data, base, ac) = hardware::battery_place().ok_or(Error::NotFound)?;
                grants.add(SLOT_DEV0, minted.ports(command as usize, 1)?, 0);
                grants.add(SLOT_DEV1, minted.ports(data as usize, 1)?, 0);
                grants.add(SLOT_SERVICE, self.server(&mut minted, name)?, ALL);
                let _ = core::fmt::Write::write_fmt(&mut args, format_args!("smbus=0x{:02x}", base));
                if let Some((byte, bit)) = ac { let _ = core::fmt::Write::write_fmt(&mut args, format_args!(" ac=0x{:02x}.{}", byte, bit)); }
            }
            // The window broker holds nothing but its own program client, which it lends to window managers (issue 157).
            "windows" => { grants.add(SLOT_SERVICE, self.server(&mut minted, "windows")?, ALL); self.lend(&mut grants, 2, "windows")?; }
            // The key service makes the device key itself (RDRAND), needs the date for its certificate, and keeps the key
            // in its private directory of the boot disk, which only this client opens (351-NET-0005, 351-KRN-0040).
            "keystore" => {
                grants.add(SLOT_SERVICE, self.server(&mut minted, "keystore")?, ALL); self.lend(&mut grants, 2, "rtc")?;
                grants.add(SLOT_VFS, self.badged(&mut minted, "vfs_server", mind::fs::BADGE_KEYSTORE)?, CLIENT);
                grants.add(4, self.badged(&mut minted, "tpm", mind::tpm::BADGE_SEAL)?, CLIENT); // seals the device key (351-NET-0006)
            }
            // The updater (351-KRN-0022): the clock and the boot disk, read-only but for the update zone (351-UPD-0008:
            // the slot that did not boot, the boot records in place); TLS over the flow the policy gives "updater" (made
            // here and kept for restarts: the policy's term bounds it); a lifecycle client of init that may ask for a
            // restart of the machine; the firmware's boot variables.
            "updater" => {
                grants.add(SLOT_SERVICE, self.server(&mut minted, "updater")?, ALL);
                self.lend(&mut grants, SLOT_RTC, "rtc")?;
                grants.add(SLOT_VFS, self.badged(&mut minted, "vfs_server", mind::fs::BADGE_UPDATE)?, CLIENT);
                if self.present("tls") { self.lend(&mut grants, SLOT_TLS, "tls")?; }
                // The parser reads the channels and manifests it fetches: it parses none itself (351-UPD-0007).
                if self.present("parse") { self.lend(&mut grants, SLOT_PARSE, "parse")?; }
                match self.flow(&mut minted, "updater") {
                    Some(flow) => grants.add(SLOT_NETWORK, flow, CLIENT),
                    None => mind::println!("[INIT] updater: NO NETWORK GRANT"),
                }
                grants.add(SLOT_LIFECYCLE, minted.badged(SLOT_SERVICE, CLIENT, mind::process::BADGE_REBOOT)?, CLIENT);
                grants.add(SLOT_FIRMWARE, minted.privilege(CAP_KIND_FIRMWARE)?, 0);
                // A loader client that only starts helpers: each check parses in a parser of its own (109-KRN-0117).
                grants.add(SLOT_LOADER, self.badged(&mut minted, "loader", mind::process::BADGE_HELPERS_ONLY)?, CLIENT);
            }
            // The Effector agent's gateway (750-KRN-0119, `mind::effector`), only in a mode: the memory and the log to read,
            // the volumes to read, its private directory (`private_dir_of`), the screen to capture, as the shell's
            // `screenshot` (750-KRN-0153; the compositor shows its red dot meanwhile), a lifecycle client limited to what the
            // owner's policy names, and a reboot when it says so (BADGE_NAMED, 750-KRN-0121, 0154), and a writable scope of
            // its delivery root (`spawn`, 0120).
            "effector_gw" => {
                grants.add(SLOT_SERVICE, self.server(&mut minted, "effector_gw")?, ALL);
                grants.add(SLOT_VFS, self.badged(&mut minted, "vfs_server", mind::fs::BADGE_EFFECTOR_GW)?, CLIENT);
                grants.add(mind::effector::SLOT_READER, self.badged(&mut minted, "vfs_server", mind::fs::BADGE_READER)?, CLIENT);
                self.lend(&mut grants, SLOT_SYSINFO, "sysmon")?;
                // The screen, badged so that the gateway lights the Effector agent's mark too (800-KRN-0156).
                grants.add(SLOT_DISPLAY, self.badged(&mut minted, "compositor", mind::display::BADGE_REMOTE_EFFECTOR)?, CLIENT);
                grants.add(SLOT_LIFECYCLE, minted.badged(SLOT_SERVICE, CLIENT, mind::process::BADGE_NAMED)?, CLIENT);
                let _ = core::fmt::Write::write_str(&mut args, "--service");
            }
            // The Effector agent's session, only in a mode: the clock, the flow the policy gives program `effector`, TLS
            // without the device certificate, the parser, the gateway as its session, its private directory and the owner's
            // data/effector/ to read (a scope made at each start). Nothing that acts on the machine: the gateway does.
            "effector" => {
                self.lend(&mut grants, SLOT_RTC, "rtc")?;
                grants.add(SLOT_VFS, self.badged(&mut minted, "vfs_server", mind::fs::BADGE_EFFECTOR)?, CLIENT);
                match self.flow(&mut minted, "effector") {
                    Some(flow) => grants.add(SLOT_NETWORK, flow, CLIENT),
                    None => mind::println!("[INIT] effector: NO NETWORK GRANT"),
                }
                if self.present("tls") { grants.add(SLOT_TLS, self.badged(&mut minted, "tls", mind::network::BADGE_TLS_NO_DEVICE)?, CLIENT); }
                if self.present("parse") { self.lend(&mut grants, SLOT_PARSE, "parse")?; }
                grants.add(mind::effector::SLOT_GATEWAY, self.badged(&mut minted, "effector_gw", mind::effector::BADGE_SESSION)?, CLIENT);
                let _ = core::fmt::Write::write_str(&mut args, "--service");
            }
            // The TLS service gets no network access: clients lend their flows. It alone may ask the key service to sign.
            "tls" => {
                grants.add(SLOT_SERVICE, self.server(&mut minted, "tls")?, ALL);
                self.lend(&mut grants, 2, "rtc")?; grants.add(3, self.badged(&mut minted, "vfs_server", mind::fs::BADGE_READER)?, CLIENT);
                grants.add(4, self.badged(&mut minted, "keystore", mind::network::BADGE_KEY_SIGNER)?, CLIENT);
            }
            // The SSH service (351-KRN-0115) has no network either: clients lend their flows. It alone may ask the key
            // service to sign an SSH login.
            "ssh" => {
                grants.add(SLOT_SERVICE, self.server(&mut minted, "ssh")?, ALL);
                grants.add(4, self.badged(&mut minted, "keystore", mind::network::BADGE_KEY_SSH)?, CLIENT);
            }
            "virtio_net" => {
                // The instance's VirtIO network card (vendor 1AF4, class 02:00, modern-only 1041 or transitional 1000), in
                // PCI order: instance n drives the n-th card.
                let mut cards: [usize; 8] = [usize::MAX; 8]; let mut found = 0;
                for id in [0x1041_1AF4, 0x1000_1AF4] {
                    for nth in 0.. { match platform::find_device_id(0x02_00_00, 0xFF_FF_00, id, nth) { Ok(device) if found < cards.len() => { cards[found] = device; found += 1; } _ => break } }
                }
                cards[..found].sort_unstable();
                let device = *cards[..found].get(instance).ok_or(Error::NotFound)?;
                self.devices[index] = Some(device);
                // The modern interface when the device describes it in one memory BAR: that BAR and an MSI-X vector (the
                // legacy line if MSI-X cannot be set up); otherwise the legacy registers in I/O BAR0 and the legacy line.
                let probe = (0..6).find_map(|bar| platform::cap(PLATFORM_DEVICE_BAR, device, bar).ok());
                let modern = probe.and_then(|slot| { let layout = mind::virtio::Layout::read(slot); let _ = ipc::drop_cap(slot); layout }).and_then(|l| l.single_bar());
                match modern {
                    Some(bar) => {
                        grants.add(SLOT_DEV0, Self::bar(&mut minted, device, bar as usize, CAP_KIND_MMIO)?, 0);
                        grants.add(SLOT_IRQ, minted.mint(PLATFORM_DEVICE_MSIX, device, 0).or_else(|_| minted.mint(PLATFORM_DEVICE_IRQ, device, 0))?, 0);
                    }
                    // LEGACY: a VirtIO card without the modern interface (docs/legacy.md).
                    None => { grants.add(SLOT_DEV0, Self::bar(&mut minted, device, 0, CAP_KIND_PORTS)?, 0); grants.add(SLOT_IRQ, minted.mint(PLATFORM_DEVICE_IRQ, device, 0)?, 0); }
                }
                grants.add(SLOT_SERVICE, self.server(&mut minted, name)?, ALL); grants.copy(SLOT_MEM, self.dma(index, NET_DMA_BYTES)?, 0);
            }
            // The observe privilege (read-only statistics, MC-10.2) goes to sysmon and to logd, which names the sender
            // of a record from the kernel's task records (MC-10.6).
            "logd" => { grants.add(SLOT_SERVICE, self.server(&mut minted, "logd")?, ALL); grants.add(SLOT_PRIV, minted.privilege(CAP_KIND_OBSERVE)?, 0); }
            "sysmon" => { grants.add(SLOT_SERVICE, self.server(&mut minted, "sysmon")?, ALL); grants.add(SLOT_PRIV, minted.privilege(CAP_KIND_OBSERVE)?, 0); }
            "shell" => {
                // Application slots plus process control, input injection (UART) and the serial line.
                flags |= SPAWN_SCREEN;
                // Badged: an unbadged copy could be badged for reboot by whoever holds it (351-KRN-0022).
                grants.add(SLOT_INIT, minted.badged(SLOT_SERVICE, CLIENT, mind::process::BADGE_LIFECYCLE)?, CLIENT);
                for (slot, service) in [(SLOT_AUDIO, "audio_gw"), (SLOT_LOADER, "loader"), (SLOT_TTS, "tts")] { self.lend(&mut grants, slot, service)?; }
                // The clock's client that may set it (211-KRN-0051); programs get the plain one from the loader.
                grants.add(SLOT_RTC, self.badged(&mut minted, "rtc", mind::rtc::BADGE_SET)?, CLIENT);
                // The user's file client: writes on ram: and in the boot disk's data directory (applications read only).
                grants.add(SLOT_VFS, self.badged(&mut minted, "vfs_server", mind::fs::BADGE_USER)?, CLIENT);
                grants.add(SLOT_CONTROL, minted.privilege(CAP_KIND_CONTROL)?, 0); grants.add(SLOT_INPUT, minted.privilege(CAP_KIND_INPUT)?, 0);
                // A PC without COM1 lends none: its ports read all ones, and a shell reading them would never draw (213-KRN-0098).
                let serial = minted.serial()?;
                if mind::dev::Uart::open(serial).is_some() { grants.add(SLOT_SERIAL, serial, 0); }
                else { mind::println!("[INIT] NO SERIAL LINE: NO 16550 ANSWERS AT COM1 (3F8), THE SHELL GETS NONE"); }
                self.lend(&mut grants, SLOT_SYSINFO, "sysmon")?;
                grants.add(SLOT_AUTHORITY, self.badged(&mut minted, "sysmon", mind::stat::BADGE_AUTHORITY)?, CLIENT);
                let keyboard = self.keyboard(); self.lend(&mut grants, SLOT_KEYBOARD, keyboard)?; self.lend(&mut grants, SLOT_DISPLAY, "compositor")?;
                // Diagnostics (ERR_PEER without a network card), with the right to mint the read-only BADGE_COUNTERS copy it
                // lends for REQUEST_NET_COUNTERS (000-NET-0015): virtio_net's badge only narrows a client.
                grants.copy(SLOT_NET, self.keeper(self.wired_cards()[0].unwrap_or("virtio_net"))?, CLIENT | CAP_BADGE);
                grants.add(SLOT_SOCKET, self.badged(&mut minted, "netstack", mind::network::BADGE_OPERATOR)?, CLIENT); // every destination
                self.lend(&mut grants, SLOT_NETPOLICY, "netpolicy")?;
                self.lend(&mut grants, SLOT_PARSE, "parse")?;
                self.lend(&mut grants, SLOT_TPM, "tpm")?;
                if self.present("battery") { self.lend(&mut grants, SLOT_BATTERY, "battery")?; }
                // The station's configuration client that may join, leave and forget, lent on for REQUEST_WIFI (550-KRN-0097).
                if self.present("wlan") { grants.add(SLOT_WIFI, self.badged(&mut minted, "wlan", mind::network::BADGE_WIFI_CHANGE)?, CLIENT); }
                // With the right to mint the BADGE_TLS_NO_DEVICE copy it lends to programs (351-NET-0022): tls's badge only narrows.
                grants.copy(SLOT_TLS, self.keeper("tls")?, CLIENT | CAP_BADGE);
                // The SSH service's, with the right to badge, lent on for REQUEST_SSH (351-KRN-0115); the updater's, for the
                // shell's own `update` command and no program (351-KRN-0116).
                if self.present("ssh") { grants.copy(SLOT_SSH, self.keeper("ssh")?, CLIENT | CAP_BADGE); }
                if self.present("updater") { self.lend(&mut grants, SLOT_UPDATE, "updater")?; }
                // The client that sets the machine's modes, for the shell's `mode` command and no program (750-KRN-0143).
                grants.add(SLOT_MODE, minted.badged(SLOT_SERVICE, CLIENT, mind::process::BADGE_MODE)?, CLIENT);
                // Its command endpoint, kept across its restarts, so that a client init lends reaches it (800-KRN-0148).
                grants.add(SLOT_COMMANDS, self.server(&mut minted, "shell")?, ALL);
                self.lend(&mut grants, SLOT_WINDOWS, "windows")?;
                grants.add(SLOT_WINDOW_MANAGER, self.badged(&mut minted, "windows", mind::window::BADGE_MANAGER)?, CLIENT);
                if self.present("blockstore") {
                    grants.add(SLOT_BLOCKSTORE, self.badged(&mut minted, "blockstore", mind::blockstore::BADGE_GET | mind::blockstore::BADGE_PUT | mind::blockstore::BADGE_PUBLISH)?, CLIENT);
                    // One that may only get, for programs that only read (300-KRN-0024).
                    grants.add(SLOT_BLOCKSTORE_READ, self.badged(&mut minted, "blockstore", mind::blockstore::BADGE_GET)?, CLIENT);
                }
                // The firmware's variables (the boot order), lent on with the user's consent (351-KRN-0027).
                grants.add(SLOT_FIRMWARE, minted.privilege(CAP_KIND_FIRMWARE)?, 0);
                if self.present("gpio") { grants.add(SLOT_GPIO, self.badged(&mut minted, "gpio", mind::gpio::BADGE_CONTROL)?, CLIENT); }
                // Lent on with the user's consent; a prepared gateway's client answers ERR_PEER until it is started.
                if self.present("video_gw") { self.lend(&mut grants, SLOT_CAMERA, "video_gw")?; }
            }
            _ => return Err(Error::NotFound),
        }
        // Every service writes to the system log; the shell's client may also read it (and lends it to dmesg), and
        // vfs_server's, which saves it on the boot disk's log partition (211-KRN-0019).
        if matches!(name, "shell" | "vfs_server" | "link" | "effector_gw") { grants.add(SLOT_LOG, self.badged(&mut minted, "logd", mind::log::BADGE_READ)?, CLIENT); }
        else if name != "logd" { self.lend(&mut grants, SLOT_LOG, "logd")?; }
        // Quotas are init's policy: loader gets all of init's root quota but what the services need (issue 171: no
        // fixed count of applications; memory limits them); the shell serves voice control on one endpoint (issue 079).
        let quota = match name {
            // The applications' memory is charged to loader too: it may use all of init's (issue 150).
            "loader" => Quota { tasks: QUOTA_MAX as u16 - SERVICE_QUOTA, endpoints: QUOTA_MAX as u16 - SERVICE_QUOTA, memory_mib: SPAWN_MEMORY_ALL as u16 },
            "shell" => Quota { tasks: 0, endpoints: 1, memory_mib: 0 },
            // The endpoint a check's parser serves (109-KRN-0117).
            "updater" => Quota { endpoints: 1, ..Quota::default() },
            // The endpoint where the programs link starts send their output (800-LNK-0009, 800-KRN-0137).
            "link" => Quota { endpoints: 1, ..Quota::default() },
            // The windows' memory is the broker's: a pixel window has room for the screen (up to 1920 × 1200, 9 MiB)
            // so that its content follows its frame (issue 163).
            "windows" => Quota { memory_mib: WINDOWS_MEMORY_MIB, ..Quota::default() },
            // The block store's index grows with its disk (251-STO-0013): 56 MiB at most, beside what it held before.
            "blockstore" => Quota { memory_mib: BLOCKSTORE_MEMORY_MIB, ..Quota::default() },
            // The Effector session measures memory as the other agents do, beside its own heap (750-KRN-0158).
            "effector" => Quota { memory_mib: EFFECTOR_MEMORY_MIB, ..Quota::default() },
            // On top of the default, three screens: the compositor's shadow copy of the framebuffer, and the two sealed
            // copies `record` holds, the frame it codes and the one before (211-KRN-0095); 20 MiB each at 2880 × 1800.
            "compositor" => Quota { memory_mib: (HEAP_MAX_BYTES >> 20) as u16 + 3 * self.screen_mib, ..Quota::default() },
            _ => Quota::default(),
        };
        let mut kept = ([0u8; ARGS_KEPT], args.as_str().len());
        kept.0[..kept.1].copy_from_slice(args.as_str().as_bytes());
        Ok((Plan { grants, flags, quota, args: kept }, minted))
    }

    fn spawn(&mut self, index: usize, plan: Plan) -> Result<u64> {
        let name = unit_name(index);
        let mut grants = plan.grants; let mut receiver = None;
        // The shell serves its commands in SLOT_COMMANDS: its slot 1 is its client of init (800-KRN-0148).
        let serves = if name == "shell" { SLOT_COMMANDS } else { SLOT_SERVICE };
        if let Some(keeper) = self.keepers[index] {
            for g in grants.list[..grants.count].iter_mut().filter(|g| g.own as usize == keeper && g.child as usize == serves) {
                let rights = g.rights; // ALL, with CAP_BADGE for a server that badges its own clients
                let slot = ipc::mint(keeper, rights, 0, 0)?; *g = grant_moved(serves, slot, rights); receiver = Some(slot);
            }
        }
        // Files confined to a directory: a scope ends with the task that took it up, so each start gets new ones
        // (750-KRN-0120): link's data/link/, the Effector services' private directories and the owner's data/effector/.
        if let Some((badge, dir)) = private_dir_of(name) {
            if let Err(error) = self.make_dir(badge, dir) { mind::println!("[INIT] {}: {} NOT MADE ({:?})", name, dir, error); }
        }
        let (mut scopes, mut taken) = ([0usize; 3], 0);
        for &(child, badge, dir, writable) in scopes_of(name) {
            if grants.count >= SPAWN_GRANTS_MAX { break; }
            match self.scope(badge, dir, writable) {
                Ok(slot) => { grants.list[grants.count] = grant_moved(child, slot, CLIENT); grants.count += 1; scopes[taken] = slot; taken += 1; }
                Err(error) => mind::println!("[INIT] {}: NO FILES IN {} ({:?})", name, dir, error),
            }
        }
        // The Effector gateway's deliveries: a writable scope of the root the owner's policy names, read at each start
        // (750-KRN-0120); none without one. The policy's parser refuses a root outside data/ or in a protected tree.
        if let Some(policy) = (name == "effector_gw" && grants.count < SPAWN_GRANTS_MAX).then(|| self.effector_policy()).flatten() {
            if let Some(root) = policy.deliver() {
                let dir = root.trim_end_matches('/');
                match self.scope(mind::fs::BADGE_USER, dir, true) {
                    Ok(slot) => { grants.list[grants.count] = grant_moved(SLOT_FILE, slot, CLIENT); grants.count += 1; scopes[taken] = slot; taken += 1; }
                    Err(error) => mind::println!("[INIT] {}: NO FILES IN {} ({:?})", name, dir, error),
                }
            }
        }
        // Where the gateway installs signed application packages: a writable scope of data/apps/ in a slot of its own
        // (750-KRN-0169); the loader checks what it finds there before a run.
        if name == "effector_gw" && grants.count < SPAWN_GRANTS_MAX {
            if let Err(error) = self.make_dir(mind::fs::BADGE_USER, mind::effector::APPS_DIR) { mind::println!("[INIT] {}: {} NOT MADE ({:?})", name, mind::effector::APPS_DIR, error); }
            match self.scope(mind::fs::BADGE_USER, mind::effector::APPS_DIR, true) {
                Ok(slot) => { grants.list[grants.count] = grant_moved(SLOT_APPS, slot, CLIENT); grants.count += 1; scopes[taken] = slot; taken += 1; }
                Err(error) => mind::println!("[INIT] {}: NO FILES IN {} ({:?})", name, mind::effector::APPS_DIR, error),
            }
        }
        // `name\0arguments` where the plan has arguments.
        let (args, count) = plan.args;
        let mut text = [0u8; NAME_MAX + 1 + ARGS_KEPT];
        let length = if count == 0 { name.len() } else { name.len() + 1 + count };
        text[..name.len()].copy_from_slice(name.as_bytes());
        if count > 0 { text[name.len() + 1..length].copy_from_slice(&args[..count]); }
        // A replaced service starts from its image in data/link/ (800-KRN-0144): a read-only copy for the kernel to copy.
        let replaced = match self.replaced[index] { Some((object, len)) => Some((ipc::mint(object, CAP_READ, 0, 0)?, len)), None => None };
        let image = replaced.map_or(Image::Boot(unit_image(index).0), |(cap, len)| Image::Memory { cap, len });
        let spawned = mind::process::spawn_raw(&text[..length], image, &grants.list[..grants.count], plan.flags, plan.quota);
        if let Some((cap, _)) = replaced { let _ = ipc::drop_cap(cap); }
        let pid = spawned.inspect_err(|_| { for slot in receiver.into_iter().chain(scopes[..taken].iter().copied()) { let _ = ipc::drop_cap(slot); } })?;
        self.pids[index] = pid;
        self.starts[index] += 1;
        self.stopped[index] = false;
        // init is the lifecycle owner of every service and receives its exit notice (MC-6.8).
        if mind::process::watch(pid, Endpoint::SERVICE).is_err() { mind::println!("[INIT] {} NOT WATCHED", name); }
        mind::println!("[INIT] STARTED {} PID={}", name, pid);
        Ok(pid)
    }
}

impl Init {
    // A watched service ended: restart it within the budget, otherwise quarantine it until the operator runs it.
    fn ended(&mut self, exit: mind::ipc::Exit) {
        if exit.lost != 0 { mind::println!("[INIT] {} EXIT NOTICES LOST", exit.lost); }
        let Some(index) = (1..UNITS).find(|&i| self.pids[i] == exit.pid) else { return };
        let name = unit_name(index);
        if self.quiesce(index).is_err() { return; }
        // Stopped on request (idl/init.wit `stop`): it stays stopped until it is started again.
        if self.stopped[index] { mind::println!("[INIT] {} PID={} STOPPED", name, exit.pid); return; }
        match exit.reason & 0xFF {
            EXIT_NORMAL => mind::println!("[INIT] {} PID={} EXITED", name, exit.pid),
            EXIT_KILLED => mind::println!("[INIT] {} PID={} KILLED", name, exit.pid),
            _ => mind::println!("[INIT] {} PID={} FAULT VECTOR {}", name, exit.pid, exit.reason >> 8),
        }
        let now = mind::time::uptime_ms() as u64;
        let recent = &mut self.restarts[index];
        if recent.iter().filter(|&&at| at != 0 && now - at < RESTART_WINDOW_MS).count() >= RESTART_BUDGET {
            self.quarantined[index] = true;
            mind::println!("[INIT] {} QUARANTINED: {} RESTARTS IN {} S", name, RESTART_BUDGET, RESTART_WINDOW_MS / 1000);
            return;
        }
        let oldest = (0..RESTART_BUDGET).min_by_key(|&i| recent[i]).unwrap(); recent[oldest] = now.max(1);
        match self.start(index) {
            Ok(pid) => mind::println!("[INIT] {} RESTARTED PID={}", name, pid),
            Err(error) => mind::println!("[INIT] {} RESTART FAILED: {:?}", name, error),
        }
    }
}

// Lifecycle requests (idl/init.wit 1.1). Stopping uses the process control privilege init keeps after boot; init and
// the shell are never stopped.
impl Init {
    fn index(name: &str) -> Option<usize> { (0..UNITS).position(|u| unit_name(u).eq_ignore_ascii_case(name)) }

    // `run`: an explicit start is the operator's decision: it lifts a quarantine and resets the restart budget. A service
    // whose device was missing at boot has nothing to start (init no longer looks for devices).
    fn run(&mut self, index: usize) -> Result<u64> {
        if self.missing[index] && self.plans[index].is_none() { return Err(Error::NotFound); }
        // The Effector services run only in the development or the maintenance mode (750-EFF-0013).
        if MODE_SERVICES.contains(&unit_name(index)) && !(self.modes.development || self.modes.maintenance) {
            mind::println!("[INIT] {} NOT STARTED: NO MODE IS ON", unit_name(index));
            return Err(Error::Rights);
        }
        if !self.running(index) { self.quarantined[index] = false; self.restarts[index] = [0; RESTART_BUDGET]; if self.pids[index] != 0 { self.quiesce(index)?; } }
        self.start(index)
    }

    fn start_service(&mut self, index: usize) -> core::result::Result<u64, idl_init::Error> {
        if index == 0 || self.running(index) { return Err(idl_init::Error::Running); }
        match self.run(index) {
            Ok(pid) => Ok(pid),
            Err(Error::NotFound) => Err(idl_init::Error::NoDevice),
            Err(Error::Rights) => Err(idl_init::Error::Denied),
            Err(error) => { mind::println!("[INIT] {} FAILED: {:?}", unit_name(index), error); Err(idl_init::Error::Failed) }
        }
    }

    // init and the shell are the last recovery boundary: no client stops them (MC-6.8).
    fn kept(index: usize) -> bool { index == 0 || unit_name(index) == "shell" }

    fn stop_service(&mut self, index: usize) -> core::result::Result<(), idl_init::Error> {
        if Self::kept(index) { return Err(idl_init::Error::Denied); }
        if !self.running(index) { return Err(idl_init::Error::Stopped); }
        let pid = self.pids[index];
        self.stopped[index] = true;
        if mind::control::kill(pid).is_err() { self.stopped[index] = false; return Err(idl_init::Error::Failed); }
        for _ in 0..200 { if !mind::process::alive(pid) { break; } mind::time::sleep(10); }
        // Its device stops DMA until the next start (MC-6.3).
        self.quiesce(index).map_err(|_| idl_init::Error::Failed)?;
        mind::println!("[INIT] STOPPED {} PID={}", unit_name(index), pid);
        Ok(())
    }

    // A service started from an ELF in data/link/ in place of its boot image, until the next boot (800-KRN-0144). Only
    // link's developer client asks; the image is not verified against the manifest, and the log says so (MC-12.1).
    fn replace(&mut self, name: &str, path: &str, badge: u16, sender: u64) -> core::result::Result<(), idl_init::Error> {
        use idl_init::Error as E;
        if badge != mind::process::BADGE_DEVELOPER { return Err(refused(sender, badge, "REPLACE", format_args!("REPLACE {}", name), "MC-3.8")); }
        let index = Self::index(name).filter(|&i| i != 0).ok_or(E::NotFound)?;
        if Self::kept(index) || matches!(unit_name(index), "vfs_server" | "logd" | "usb_serial" | "link") {
            mind::println!("[INIT] REPLACE REFUSED: {} CARRIES THE OPERATOR, THE LOG OR THE LINK", unit_name(index));
            return Err(E::Denied);
        }
        // The image leaves init's address space: init keeps a memory object it cannot write (Pages::detach).
        let image = match path.is_empty() {
            true => None,
            false => {
                let (pages, len) = self.link_image(path)?;
                let digest = mind::sha256::digest(&pages.as_slice()[..len]);
                Some((pages.detach().map_err(|_| E::Failed)?, len, digest))
            }
        };
        if self.running(index) { self.stop_service(index)?; }
        match &image {
            Some((_, _, digest)) => {
                let mut hex = [0u8; 64];
                for (i, b) in digest.iter().enumerate() { hex[2 * i] = b"0123456789abcdef"[(b >> 4) as usize]; hex[2 * i + 1] = b"0123456789abcdef"[(b & 15) as usize]; }
                mind::println!("[INIT] {} REPLACED FROM {}/{}: SHA-256 {}, NOT VERIFIED", unit_name(index), LINK_DIR, path, core::str::from_utf8(&hex).unwrap_or("?"));
            }
            None => mind::println!("[INIT] {} BACK TO ITS BOOT IMAGE", unit_name(index)),
        }
        if let Some((old, _)) = self.replaced[index].take() { let _ = ipc::drop_cap(old); }
        self.replaced[index] = image.map(|(object, len, _)| (object, len));
        self.start_service(index).map(drop)
    }

    // An ELF read whole from data/link/<path>, a plain relative name, through a client of the user's files that init
    // drops at once.
    fn link_image(&mut self, path: &str) -> core::result::Result<(mind::mem::Pages, usize), idl_init::Error> {
        use idl_init::Error as E;
        if path.starts_with('/') || path.contains(':') || path.split('/').any(|part| part.is_empty() || part == "." || part == "..") { return Err(E::NotFound); }
        let keeper = self.keeper("vfs_server").map_err(|_| E::Failed)?;
        let client = ipc::mint_badged(keeper, CLIENT, mind::fs::BADGE_USER).map_err(|_| E::Failed)?;
        let files = Endpoint(client);
        let read = (|| -> core::result::Result<(mind::mem::Pages, usize), E> {
            let root = vfs::root(files, "").ok().and_then(|r| r.ok()).ok_or(E::NotFound)?;
            let dir = vfs::open_dir(files, root, LINK_DIR, false);
            let _ = vfs::close(files, root);
            let dir = dir.ok().and_then(|r| r.ok()).ok_or(E::NotFound)?;
            let file = vfs::open(files, dir, path, 0);
            let _ = vfs::close(files, dir);
            let file = file.ok().and_then(|r| r.ok()).ok_or(E::NotFound)?;
            let read = (|| {
                let size = vfs::stat(files, file).ok().and_then(|r| r.ok()).ok_or(E::NotFound)?.size as usize;
                if !(64..=REPLACE_MAX).contains(&size) { return Err(E::Failed); }
                let mut image = mind::mem::Pages::new(size).ok_or(E::Failed)?;
                let mut done = 0;
                while done < size {
                    let want = (size - done).min(mind::fs::CHUNK);
                    let got = vfs::read(files, file, done as u32, want as u32, &mut image.as_mut_slice()[done..done + want]).ok().and_then(|r| r.ok()).ok_or(E::Failed)?;
                    if got == 0 { return Err(E::Failed); }
                    done += got;
                }
                if &image.as_slice()[..4] != b"\x7fELF" { return Err(E::Failed); }
                Ok((image, size))
            })();
            let _ = vfs::close(files, file);
            read
        })();
        let _ = ipc::drop_cap(client);
        read
    }

    fn list(&self) -> [idl_init::Service; UNITS] {
        core::array::from_fn(|i| idl_init::Service {
            name: wire_text(unit_name(i)), pid: if i == 0 { INIT_PID } else if self.running(i) { self.pids[i] } else { 0 },
            starts: if i == 0 { 1 } else { self.starts[i] }, running: i == 0 || self.running(i), quarantined: self.quarantined[i], holds: wire_text(HOLDS[unit_image(i).0]),
            replaced: self.replaced[i].is_some(),
        })
    }

    // This boot's plan in the order tried (init first), for `boot-plan` (173-KRN-0035).
    #[inline(never)]
    fn boot_plan(&self) -> [idl_init::Planned; UNITS] {
        let mut order = [0usize; UNITS];
        for (i, o) in order.iter_mut().enumerate() { *o = i; }
        order.sort_unstable_by_key(|&i| if i == 0 { 0 } else if self.placed[i].place == 0 { u16::MAX } else { self.placed[i].place as u16 });
        core::array::from_fn(|n| {
            let i = order[n];
            let mut off = mind::util::FixedBuf::<32>::new();
            let _ = match self.placed[i].off {
                Off::No => Ok(()),
                Off::Disabled => core::fmt::Write::write_str(&mut off, "disabled"),
                Off::Needs(need) => core::fmt::Write::write_fmt(&mut off, format_args!("needs {}", unit_name(need))),
                Off::NoDevice => core::fmt::Write::write_str(&mut off, "no device"),
                Off::NoImage => core::fmt::Write::write_str(&mut off, "no image"),
                Off::Safe => core::fmt::Write::write_str(&mut off, "safe start"),
                Off::Mode => core::fmt::Write::write_str(&mut off, "mode"),
                Off::NoLink => core::fmt::Write::write_str(&mut off, "no link.conf"),
                Off::Failed => core::fmt::Write::write_str(&mut off, "failed"),
            };
            let placed = self.placed[i];
            idl_init::Planned { name: wire_text(unit_name(i)), place: if i == 0 { 0 } else { placed.place }, enabled: !matches!(placed.off, Off::Disabled),
                                essential: !services::configurable(unit_name(i)), after: wire_text(placed.after.map_or("", unit_name)), off: wire_text(off.as_str()) }
        })
    }

    fn serve(&mut self, request: idl_init::Request, call: wire::Call, badge: u16, sender: u64) -> Result<()> {
        use idl_init::{Error as E, Request};
        match request {
            Request::Run { name } => {
                let result = match Self::index(name.as_str()) {
                    None | Some(0) => Err(Error::NotFound),
                    Some(index) if !self.named(badge, sender, index, Action::Start) => Err(Error::Rights),
                    Some(index) => self.run(index),
                };
                idl_init::reply_run(call, result)
            }
            Request::List => idl_init::reply_list(call, Ok(&self.list()[..])),
            Request::Stop { name } => {
                let result = Self::index(name.as_str()).ok_or(E::NotFound).and_then(|i| {
                    if Self::kept(i) { return Err(refused(sender, badge, "STOP", format_args!("STOP {}", unit_name(i)), "MC-6.8")); }
                    if !self.named(badge, sender, i, Action::Stop) { return Err(E::Denied); }
                    self.stop_service(i)
                });
                idl_init::reply_stop(call, result)
            }
            Request::Restart { name } => {
                let result = Self::index(name.as_str()).ok_or(E::NotFound).and_then(|i| {
                    if Self::kept(i) { return Err(refused(sender, badge, "RESTART", format_args!("RESTART {}", unit_name(i)), "MC-6.8")); }
                    if !self.named(badge, sender, i, Action::Restart) { return Err(E::Denied); }
                    if self.running(i) { self.stop_service(i)?; }
                    self.start_service(i)
                });
                idl_init::reply_restart(call, result)
            }
            // The gateway's named client stops services only, by name (750-KRN-0121).
            Request::StopTask { pid } if badge == mind::process::BADGE_NAMED => {
                idl_init::reply_stop_task(call, Err(refused(sender, badge, "STOP", format_args!("STOP PID {}", pid), "MC-3.4")))
            }
            Request::StopTask { pid } => {
                let service = pid == INIT_PID || (1..UNITS).any(|i| self.pids[i] == pid && self.running(i));
                // A service stops through `stop`, by init, which keeps its lifecycle and quiets its device (MC-6.8).
                let result = if service { Err(refused(sender, badge, "STOP", format_args!("STOP PID {}, A SERVICE", pid), "MC-6.8")) } else if !mind::process::alive(pid) { Err(E::NotFound) } else { mind::control::kill(pid).map_err(|_| E::NotFound) };
                idl_init::reply_stop_task(call, result)
            }
            // The gateway's named client restarts the machine only as the owner's policy allows (750-KRN-0154).
            Request::Reboot if badge == mind::process::BADGE_NAMED && !self.named_reboot(sender) => {
                idl_init::reply_reboot(call, Err(idl_init::Error::Denied))
            }
            // Otherwise only the updater's badged client and link's developer client may (351-KRN-0022, 800-KRN-0144).
            Request::Reboot if !matches!(badge, mind::process::BADGE_REBOOT | mind::process::BADGE_DEVELOPER | mind::process::BADGE_NAMED) => {
                idl_init::reply_reboot(call, Err(refused(sender, badge, "REBOOT", format_args!("RESTART THE MACHINE"), "MC-3.8")))
            }
            Request::Reboot => { idl_init::reply_reboot(call, Ok(()))?; self.reboot() }
            // Asked only once the boot was confirmed or refused: requests are served after that (000-KRN-0102).
            Request::Boot => idl_init::reply_boot(call, Ok(&self.boot)),
            Request::Mode => idl_init::reply_mode(call, Ok(&self.modes)),
            Request::LinkConsole => idl_init::reply_link_console(call, Ok(self.link_console)),
            Request::BootPlan => idl_init::reply_boot_plan(call, Ok(&self.boot_plan()[..])),
            Request::Configuration => idl_init::reply_configuration(call, Ok(self.configuration)),
            Request::Replace { name, path } => { let result = self.replace(name.as_str(), path.as_str(), badge, sender); idl_init::reply_replace(call, result) }
            Request::SetMode { which, on, reason } => {
                let result = self.set_mode(which, on, reason.as_str(), badge, sender);
                idl_init::reply_set_mode(call, result)
            }
        }
    }

    // A mode turned on or off (750-KRN-0143): the shell's client may do either, the Mind's only turn the maintenance mode
    // on. The file first, so that a restart keeps what the services follow; one log line, the audit record (MC-10.2).
    fn set_mode(&mut self, which: idl_init::Mode, on: bool, reason: &str, badge: u16, sender: u64) -> core::result::Result<(), idl_init::Error> {
        use idl_init::Mode;
        let name = match which { Mode::Development => "DEVELOPMENT", Mode::Maintenance => "MAINTENANCE" };
        let by = match badge {
            mind::process::BADGE_MODE => "operator",
            mind::process::BADGE_MIND if on && which == Mode::Maintenance => "mind",
            _ => {
                return Err(refused(sender, badge, "MODE CHANGE", format_args!("TURN {} {}", name, if on { "ON" } else { "OFF" }), "MC-12.6"));
            }
        };
        let mut modes = self.modes;
        match which { Mode::Development => modes.development = on, Mode::Maintenance => modes.maintenance = on }
        (modes.by, modes.reason) = (wire_text(by), wire_text(reason));
        self.write_modes(&modes).map_err(|_| idl_init::Error::Failed)?;
        self.modes = modes;
        mind::println!("[INIT] MODE {} {} BY {} (PID {}, BADGE {}): {}", name, if on { "ON" } else { "OFF" }, by, sender, badge, reason);
        self.follow_modes();
        Ok(())
    }

    // A client of vfs_server with the user's rights for init's own file on the boot volume, dropped after use.
    fn user_files(&mut self) -> Result<usize> { let keeper = self.keeper("vfs_server")?; Ok(ipc::mint_badged(keeper, CLIENT, mind::fs::BADGE_USER)?) }

    // The link's developer mode: link.conf's word `dev`, as the bootloader read it (800-KRN-0139).
    fn link_dev(&self) -> bool {
        let words = self.link.unwrap_or([0; LINK_WORDS_MAX]);
        let length = words.iter().position(|&b| b == 0).unwrap_or(words.len());
        core::str::from_utf8(&words[..length]).unwrap_or("").split_ascii_whitespace().any(|word| word == "dev")
    }

    // The owner's Effector policy (750-KRN-0121), read whole at each use, as the gateway reads it; None when it is absent,
    // too large or refused, which allows nothing.
    fn effector_policy(&mut self) -> Option<Policy> {
        let mut text = [0u8; mind::effector::policy::FILE_MAX + 1];
        let mut length = 0;
        let client = self.user_files().ok()?;
        let files = Endpoint(client);
        if let Ok(Ok(root)) = vfs::root(files, "") {
            if let Ok(Ok(file)) = vfs::open(files, root, mind::effector::POLICY_FILE, 0) {
                while length < text.len() {
                    match vfs::read(files, file, length as u32, (text.len() - length) as u32, &mut text[length..]) {
                        Ok(Ok(n)) if n > 0 => length += n,
                        _ => break,
                    }
                }
                let _ = vfs::close(files, file);
            }
            let _ = vfs::close(files, root);
        }
        let _ = ipc::drop_cap(client);
        Policy::parse(&text[..length]).ok()
    }

    // A client badged BADGE_NAMED (the Effector gateway's, 750-KRN-0121) may run, stop or restart only what the owner's
    // policy allows; init reads the policy itself, so the gateway's own check is never the only one (MC-3.4).
    fn named(&mut self, badge: u16, sender: u64, index: usize, action: Action) -> bool {
        if badge != mind::process::BADGE_NAMED { return true; }
        let name = unit_name(index);
        let why = match self.effector_policy() {
            _ if Self::kept(index) => Some("KEPT BY INIT"),
            None => Some("NO OWNER'S POLICY"),
            Some(policy) => match policy.decide(Ask::Service(name, action)) {
                Ok(()) => None,
                Err(Deny::Protected) => Some("NEVER THE GATEWAY'S"),
                Err(_) => Some("NOT IN THE OWNER'S POLICY"),
            },
        };
        if let Some(why) = why {
            let word = match action { Action::Start => "RUN", Action::Stop => "STOP", Action::Restart => "RESTART" };
            refused(sender, badge, word, format_args!("{} {}: {}", word, name, why), "MC-3.4");
        }
        why.is_none()
    }

    // The gateway's named client restarts the machine only when the owner's policy says `reboot on`, read at the request.
    fn named_reboot(&mut self, sender: u64) -> bool {
        let badge = mind::process::BADGE_NAMED;
        let why = match self.effector_policy() {
            None => Some("NO OWNER'S POLICY"),
            Some(policy) => policy.decide(Ask::Reboot).err().map(|_| "NOT IN THE OWNER'S POLICY"),
        };
        match why {
            Some(why) => { refused(sender, badge, "REBOOT", format_args!("RESTART THE MACHINE: {}", why), "MC-3.4"); false }
            None => { mind::println!("[INIT] REBOOT BY PID {} (BADGE {}): THE OWNER'S POLICY ALLOWS IT", sender, badge); true }
        }
    }

    // One boot service at boot: started, or prepared when data/services.txt (`off`) or a safe start leaves it off, or left
    // out with the reason; false when its start failed. Not inlined: boot's own frame stays small, as a start goes deep.
    #[inline(never)]
    fn boot_one(&mut self, info: &BootInfo, index: usize, safe: bool, off: Option<Off>) -> bool {
        let name = unit_name(index);
        // When data/pause.txt asks (BootInfo.photo_pause_s, 211-KRN-0089), time for a photo of the kernel's lines
        // before the compositor takes the screen (211-PRT-0004).
        if name == "compositor" && info.photo_pause_s > 0 {
            mind::println!("[INIT] A PAUSE OF {} S FOR A PHOTO OF THE SCREEN BEFORE THE COMPOSITOR TAKES IT", info.photo_pause_s);
            mind::time::sleep(info.photo_pause_s.min(PHOTO_PAUSE_MAX_S) as usize * 1000);
        }
        // An image the bootloader did not find (a service of the other architecture) is not started.
        if info.image(unit_image(index).0).is_none_or(|image| image.len == 0) {
            self.missing[index] = true; self.placed[index].off = Off::NoImage;
            mind::println!("[INIT] {} NOT STARTED: NO IMAGE", name);
            return true;
        }
        // The Effector services wait for the modes, read once the boot volume is mounted (750-KRN-0143).
        if MODE_SERVICES.contains(&name) { self.placed[index].off = Off::Mode; return true; }
        // The link to another machine only when the boot volume's root holds link.conf, as the bootloader read it.
        if matches!(name, "usb_serial" | "link") && self.link.is_none() {
            self.missing[index] = true; self.placed[index].off = Off::NoLink;
            mind::println!("[INIT] {} NOT STARTED: NO LINK.CONF", name);
            return true;
        }
        // Left out by a safe start, and the station whose radio is (000-KRN-0131), or by data/services.txt: prepared, not
        // started, so that `svc start` starts it once the platform privilege is dropped.
        let left = off.or((safe && (SAFE_LEFT_OUT.contains(&name) || (name == "wlan" && !RADIOS.iter().any(|&r| self.running(service_index(r)))))).then_some(Off::Safe));
        if let Some(why) = left {
            self.placed[index].off = why;
            match (self.prepare(index), why) {
                (Ok(()), Off::Safe) => mind::println!("[INIT] {} LEFT OUT BY THE SAFE START, PREPARED: `svc start {}` STARTS IT", name, name),
                (Ok(()), Off::Needs(need)) => mind::println!("[INIT] {} OFF: IT NEEDS {}, WHICH {} TURNS OFF; PREPARED: `svc start {}` STARTS IT", name, unit_name(need), services::FILE, name),
                (Ok(()), _) => mind::println!("[INIT] {} OFF BY {}: PREPARED, `svc start {}` STARTS IT", name, services::FILE, name),
                (Err(Error::NotFound), _) => {
                    self.missing[index] = true; self.stopped[index] = true; self.placed[index].off = Off::NoDevice;
                    mind::println!("[INIT] {} NOT STARTED: NO DEVICE", name);
                }
                (Err(error), Off::Safe) => { self.stopped[index] = true; mind::println!("[INIT] {} LEFT OUT BY THE SAFE START, NOT PREPARED: {:?}", name, error) }
                (Err(error), _) => { self.stopped[index] = true; mind::println!("[INIT] {} OFF BY {}, NOT PREPARED: {:?}", name, services::FILE, error) }
            }
            return true;
        }
        match self.start(index) {
            // From now on init's own lines (and those printed so far) go to the system log too.
            Ok(_) if name == "logd" => {
                // The keeper may send: init needs no client of its own.
                if let Ok(keeper) = self.keeper("logd") { mind::log::use_endpoint(Endpoint(keeper)); }
                kernel_lines();
            }
            Ok(_) => {}
            Err(Error::NotFound) => { self.missing[index] = true; self.placed[index].off = Off::NoDevice; mind::println!("[INIT] {} NOT STARTED: NO DEVICE", name); }
            Err(error) => { self.placed[index].off = Off::Failed; mind::println!("[INIT] {} FAILED: {:?}", name, error); return false; }
        }
        true
    }

    // data/services.txt (173-KRN-0035), through a reader's client of init's own: its plan over the units in boot order
    // (`names`), each line refused logged with its number and why; no change when the file is absent. Its text in pages
    // of their own, off the stack.
    #[inline(never)]
    fn read_services(&mut self, names: &[&str; UNITS]) -> services::Plan<UNITS> {
        let Some(mut pages) = mind::mem::Pages::new(services::MAX + 1) else { mind::println!("[INIT] NO MEMORY TO READ {}", services::FILE); return services::Plan::none() };
        let text = &mut pages.as_mut_slice()[..services::MAX + 1];
        let mut length = None;
        if let Ok(client) = self.keeper("vfs_server").and_then(|keeper| ipc::mint_badged(keeper, CLIENT, mind::fs::BADGE_READER).map_err(Into::into)) {
            let files = Endpoint(client);
            if let Ok(Ok(root)) = vfs::root(files, "") {
                if let Ok(Ok(file)) = vfs::open(files, root, services::FILE, 0) {
                    let mut done = 0;
                    while done < text.len() {
                        let want = (text.len() - done).min(mind::fs::CHUNK);
                        match vfs::read(files, file, done as u32, want as u32, &mut text[done..done + want]) { Ok(Ok(got)) if got > 0 => done += got, _ => break }
                    }
                    length = Some(done);
                    let _ = vfs::close(files, file);
                }
                let _ = vfs::close(files, root);
            }
            let _ = ipc::drop_cap(client);
        }
        let Some(length) = length else { mind::println!("[INIT] NO {}: EVERY BOOT SERVICE STARTS", services::FILE); return services::Plan::none() };
        self.configuration = idl_init::Configuration::Read;
        let plan = services::parse(&text[..length], names, |line, why, text| match line {
            0 => mind::println!("[INIT] {}: {}", services::FILE, why),
            _ => mind::println!("[INIT] {} LINE {} REFUSED: {}: {}", services::FILE, line, why, text),
        });
        let (off, needs, after) = plan.counts();
        mind::println!("[INIT] {}: {} OFF, {} OFF AS THEY NEED ONE OF THEM, {} WAIT FOR ANOTHER", services::FILE, off, needs, after);
        plan
    }

    // The modes data/mode.conf names (750-KRN-0143): `development` and `maintenance`; other words are logged and left out.
    fn read_modes(&mut self) {
        let mut text = [0u8; 256];
        let mut length = 0;
        if let Ok(client) = self.user_files() {
            let files = Endpoint(client);
            if let Ok(Ok(root)) = vfs::root(files, "") {
                if let Ok(Ok(file)) = vfs::open(files, root, MODE_FILE, 0) {
                    length = vfs::read(files, file, 0, text.len() as u32, &mut text).ok().and_then(|r| r.ok()).unwrap_or(0);
                    let _ = vfs::close(files, file);
                }
                let _ = vfs::close(files, root);
            }
            let _ = ipc::drop_cap(client);
        }
        let mut modes = idl_init::ModeState { by: wire_text("disk"), ..Default::default() };
        for word in core::str::from_utf8(&text[..length]).unwrap_or("").lines().map(|l| l.split('#').next().unwrap_or("")).flat_map(str::split_whitespace) {
            match word {
                "development" => modes.development = true,
                "maintenance" => modes.maintenance = true,
                // The link's console (800-KRN-0155): the owner's word, counted only in the link's developer mode.
                "link-console" if self.link_dev() => self.link_console = true,
                "link-console" => mind::println!("[INIT] {}: WORD LEFT OUT: link-console (NO dev IN link.conf)", MODE_FILE),
                other => mind::println!("[INIT] {}: WORD LEFT OUT: {}", MODE_FILE, other),
            }
        }
        self.modes = modes;
        let name = match (modes.development, modes.maintenance) { (true, true) => "DEVELOPMENT AND MAINTENANCE", (true, false) => "DEVELOPMENT", (false, true) => "MAINTENANCE", _ => "NORMAL" };
        let (name, console) = match (name, self.link_console) { ("NORMAL", true) => ("LINK CONSOLE", ""), (name, true) => (name, " AND LINK CONSOLE"), (name, false) => (name, "") };
        mind::println!("[INIT] MODE: {}{} ({})", name, console, if length > 0 { MODE_FILE } else { "NO data/mode.conf" });
    }

    // data/mode.conf rewritten whole: one word a line, and who changed it last.
    fn write_modes(&mut self, modes: &idl_init::ModeState) -> Result<()> {
        let mut text = mind::util::FixedBuf::<256>::new();
        let _ = core::fmt::Write::write_fmt(&mut text, format_args!("# the machine's modes, written by init (750-KRN-0143); last changed by {}\n", modes.by.as_str()));
        if modes.development { let _ = core::fmt::Write::write_str(&mut text, "development\n"); }
        if modes.maintenance { let _ = core::fmt::Write::write_str(&mut text, "maintenance\n"); }
        if self.link_console { let _ = core::fmt::Write::write_str(&mut text, "link-console\n"); }
        let client = self.user_files()?;
        let files = Endpoint(client);
        let written = (|| -> Result<()> {
            let root = vfs::root(files, "")?.map_err(|_| Error::NotFound)?;
            let file = vfs::open(files, root, MODE_FILE, mind::fs::MODE_CREATE | mind::fs::MODE_WRITE | mind::fs::MODE_TRUNCATE);
            let file = file.map(|r| r.map_err(|_| Error::Rights));
            let result = match file {
                Ok(Ok(file)) => {
                    let wrote = vfs::write(files, file, 0, text.as_bytes()).map(|r| r.is_ok()).unwrap_or(false) && vfs::flush(files, file).map(|r| r.is_ok()).unwrap_or(false);
                    let _ = vfs::close(files, file);
                    if wrote { Ok(()) } else { Err(Error::Rights) }
                }
                Ok(Err(error)) | Err(error) => Err(error),
            };
            let _ = vfs::close(files, root);
            result
        })();
        let _ = ipc::drop_cap(client);
        written
    }

    // The Effector services run while a mode is on and stop when none is (750-EFF-0013); a build without them logs that.
    fn follow_modes(&mut self) {
        let on = self.modes.development || self.modes.maintenance;
        let mut present = 0;
        // The gateway starts first and stops last: the session always has it.
        let order = if on { MODE_SERVICES } else { [MODE_SERVICES[1], MODE_SERVICES[0]] };
        for name in order {
            let Some(index) = Self::index(name) else { continue };
            present += 1;
            if on && !self.running(index) {
                if let Err(error) = self.start_service(index) { mind::println!("[INIT] {} NOT STARTED FOR THE MODE: {:?}", name, error); }
            } else if !on && self.running(index) {
                let _ = self.stop_service(index);
            }
        }
        if on && present == 0 { mind::println!("[INIT] A MODE IS ON, BUT THIS BUILD HAS NO EFFECTOR SERVICES (750-KRN-0119)"); }
    }

    // A restart of the machine the updater asked for: what the volumes cache is written, the services stop in reverse
    // start order (their devices quiesced), then the reset through the restart privilege.
    fn reboot(&mut self) -> ! {
        mind::println!("[INIT] RESTARTING THE MACHINE: THE VOLUMES WRITTEN, THE SERVICES STOPPED");
        if let Ok(client) = self.keeper("vfs_server").and_then(|keeper| ipc::mint_badged(keeper, CLIENT, 0).map_err(Into::into)) {
            // A flush to a slow stick may take long: the mount's limit, not a call's.
            wire::with_timeout(MOUNT_LIMIT_MS, || for volume in ["", "ram", "log"] {
                if let Ok(Ok(handle)) = vfs::root(Endpoint(client), volume) { let _ = vfs::flush(Endpoint(client), handle); let _ = vfs::close(Endpoint(client), handle); }
            });
            let _ = ipc::drop_cap(client);
        }
        let (mut order, mut count) = ([0usize; UNITS], 0);
        for index in boot_order() { order[count] = index; count += 1; }
        for &index in order[..count].iter().rev() {
            if !self.running(index) { continue; }
            let pid = self.pids[index];
            self.stopped[index] = true;
            let _ = mind::control::kill(pid);
            for _ in 0..200 { if !mind::process::alive(pid) { break; } mind::time::sleep(10); }
            if self.quiesce(index).is_ok() { mind::println!("[INIT] STOPPED {} PID={}", unit_name(index), pid); }
        }
        let _ = mind::control::reboot();
        loop { mind::time::sleep(1000); }
    }
}

// The kernel's own lines (its ring, at the end of the platform report) into the system log, not the screen, which shows
// them already: a reader of the log, the link to another machine above all, sees the boot from its first line.
fn kernel_lines() {
    let Ok(slot) = platform::cap(PLATFORM_REPORT, 0, 0) else { return };
    if let Ok(map) = mind::mem::Mapping::new(slot) {
        let bytes = map.as_slice();
        let bytes = &bytes[..bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len())];
        let text = core::str::from_utf8(bytes).unwrap_or("");
        let lines = text.find("\nKernel lines").and_then(|at| text[at + 1..].find('\n').map(|end| &text[at + 1 + end + 1..])).unwrap_or("");
        let mut count = 0;
        for line in lines.lines().map(str::trim_end).filter(|l| !l.is_empty()) { let _ = mind::log::write(mind::log::INFO, line); count += 1; }
        mind::println!("[INIT] {} KERNEL LINES COPIED INTO THE SYSTEM LOG", count);
    }
    let _ = ipc::drop_cap(slot);
}

fn wire_text<const N: usize>(text: &str) -> mind::idl::codec::Text<N> {
    let mut end = text.len().min(N);
    while !text.is_char_boundary(end) { end -= 1; }
    mind::idl::codec::Text::new(&text[..end]).unwrap_or_default()
}

// A client's request refused: one log line that names the caller and the clause it enforces (850-KRN-0146).
fn refused(sender: u64, badge: u16, what: &str, action: core::fmt::Arguments, clause: &str) -> idl_init::Error {
    mind::println!("[INIT] {} REFUSED: PID {} (BADGE {}) MAY NOT {} ({})", what, sender, badge, action, clause);
    idl_init::Error::Denied
}

fn service_index(name: &str) -> usize { (0..UNITS).find(|&u| unit_name(u) == name).unwrap_or(0) }

// The directories a service gets confined clients of at each start (750-KRN-0120): its slot, the badge init asks with,
// the directory on the boot volume (made if missing) and whether it may write there.
fn scopes_of(name: &str) -> &'static [(usize, u16, &'static str, bool)] {
    match name {
        "link" => &[(SLOT_VFS, mind::fs::BADGE_USER, LINK_DIR, true)],
        "effector" => &[(SLOT_FILE, mind::fs::BADGE_USER, mind::effector::OWNER_DIR, false)],
        _ => &[],
    }
}

// A service's private directory in the boot volume's `system/` and the badge that alone opens it (351-NET-0005): its
// SLOT_VFS client carries the badge; init makes the directory before each start, as the service does not.
fn private_dir_of(name: &str) -> Option<(u16, &'static str)> {
    match name {
        "effector" => Some((mind::fs::BADGE_EFFECTOR, mind::effector::SESSION_DIR)),
        "effector_gw" => Some((mind::fs::BADGE_EFFECTOR_GW, mind::effector::GATEWAY_DIR)),
        _ => None,
    }
}

// Boot order is the BOOT_SERVICES order (logd first, drivers before vfs_server, loader before the shell); further
// instances of an image follow its first one.
fn boot_order() -> impl Iterator<Item = usize> {
    (1..BOOT_IMAGES).flat_map(|image| core::iter::once(image).chain((BOOT_IMAGES..UNITS).filter(move |&u| unit_image(u).0 == image)))
}


// What a safe start leaves out (000-KRN-0076): the drivers and gateways of radios, cameras and sound, which a frozen
// run was most likely in. Their plans are prepared and their devices stopped, so `svc start` starts them (000-KRN-0131).
const SAFE_LEFT_OUT: [&str; 6] = ["bcm_wifi", "video_gw", "audio_gw", "tts", "rtw89_wifi", "rtw88_wifi"];
// The run's mark is cleared this long after READY: a run that got that far ended well enough.
const MARK_CLEAR_MS: usize = 60_000;

// The mark of an unfinished run (RUN_MARK_NAME): whether the last run left it, and whether this one set it.
fn run_mark(firmware: usize) -> (bool, bool) {
    let found = mind::firmware::get_in(firmware, RUN_MARK_NAME, &RUN_MARK_GUID, &mut [0u8; 8]).is_ok();
    (found, mind::firmware::set_in(firmware, RUN_MARK_NAME, &RUN_MARK_GUID, mind::firmware::BOOT_VARIABLE, &[1]).is_ok())
}

// The boot services in boot order (173-KRN-0035): whether every one with its hardware started, those data/services.txt
// leaves off aside (351-KRN-0014), and whether the boot volume was found mounted (None if the file was not looked for,
// as when `asked`, a safe start asked at the bootloader, 173-KRN-0036).
// Not inlined: its frame is gone before the modes start the Effector services, whose start goes deep.
#[inline(never)]
fn boot_services(init: &mut Init, info: &BootInfo, safe: bool, asked: bool) -> (bool, Option<bool>) {
    let mut healthy = true;
    // The units in boot order (init first) and data/services.txt's plan over them, read at the first service it may
    // change, once the boot volume is mounted (173-KRN-0035).
    let (mut names, mut units) = (["init"; UNITS], [0usize; UNITS]);
    for (p, unit) in core::iter::once(0).chain(boot_order()).enumerate() { names[p] = unit_name(unit); units[p] = unit; }
    let (mut plan, mut mounted) = (None, None);
    let (mut tried, mut waiting, mut place, mut next) = ([false; UNITS], [false; UNITS], 0u8, 1);
    tried[0] = true;
    loop {
        let ready = |p: usize, plan: &Option<services::Plan<UNITS>>, tried: &[bool; UNITS]| plan.as_ref().is_none_or(|plan| plan.ready(&names, p, tried));
        // One that waited and may start now, else the next in order, else one still waiting (never: the plan has no cycle).
        let (p, in_order) = if let Some(q) = (1..UNITS).find(|&q| waiting[q] && ready(q, &plan, &tried)) { (q, false) }
                            else if next < UNITS { next += 1; (next - 1, true) }
                            else if let Some(q) = (1..UNITS).find(|&q| waiting[q]) { (q, false) }
                            else { break };
        waiting[p] = false;
        if plan.is_none() && services::configurable(names[p]) && asked {
            init.configuration = idl_init::Configuration::Ignored;
            mind::println!("[INIT] SAFE START ASKED AT THE BOOTLOADER: THE SERVICE CONFIGURATION IS IGNORED ({} NOT READ)", services::FILE);
            plan = Some(services::Plan::none());
        }
        if plan.is_none() && services::configurable(names[p]) {
            let found = wire::with_timeout(MOUNT_LIMIT_MS, || init.boot_volume_mounted());
            mounted = Some(found);
            if !found { init.configuration = idl_init::Configuration::Unreadable; }
            plan = Some(if found { init.read_services(&names) } else { mind::println!("[INIT] NO BOOT VOLUME: {} NOT READ", services::FILE); services::Plan::none() });
        }
        if let Some(first) = plan.as_ref().filter(|_| in_order).and_then(|plan| plan.waits_for(&names, p, &tried)) {
            mind::println!("[INIT] {} WAITS FOR {} ({})", names[p], names[first], services::FILE);
            waiting[p] = true;
            continue;
        }
        let index = units[p];
        place += 1;
        init.placed[index].place = place;
        init.placed[index].after = plan.as_ref().and_then(|plan| plan.after[p]).map(|a| units[a]);
        let off = plan.as_ref().and_then(|plan| match (plan.off[p], plan.needs[p]) { (true, _) => Some(Off::Disabled), (_, Some(need)) => Some(Off::Needs(units[need])), _ => None });
        healthy &= init.boot_one(info, index, safe, off);
        tried[p] = true;
    }
    (healthy, mounted)
}

mind::entry!(main);
// A service that does not answer never stops the boot or the supervision of the others (000-KRN-0075).
fn main(info: &'static BootInfo) { wire::with_timeout(CALL_LIMIT_MS, || boot(info)) }

fn boot(info: &'static BootInfo) {
    let screen_mib = (info.stride * info.height * 4).div_ceil(1 << 20) as u16;
    let mut init = Init { plans: [None; UNITS], pids: [0; UNITS], dma: [None; UNITS], devices: [None; UNITS], others: [None; UNITS], keepers: [None; UNITS], restarts: [[0; RESTART_BUDGET]; UNITS], quarantined: [false; UNITS], starts: [0; UNITS], stopped: [false; UNITS], missing: [false; UNITS], screen_mib, store: None,
                         link: (info.link_present != 0).then_some(info.link_words),
                         boot: idl_init::Boot { slot: info.boot_slot.slot as u8, trial: info.boot_slot.trial != 0, confirmed: false },
                         modes: idl_init::ModeState::default(), link_console: false, replaced: [None; UNITS], placed: [Placed::default(); UNITS],
                         configuration: idl_init::Configuration::Absent };
    // Where this build came from, so that logs of different builds are told apart (211-KRN-0044; 02_build.sh sets it).
    mind::println!("[INIT] BUILD: BRANCH {}, COMMIT {}", option_env!("MIND_BUILD_BRANCH").unwrap_or("UNKNOWN"), option_env!("MIND_BUILD_COMMIT").unwrap_or("UNKNOWN"));
    // The launch record, as the bootloader printed it on the serial line, into the system log (350-UPD-0004, MC-9.5).
    {
        let (launch, slot) = (&info.launch, &info.boot_slot);
        let mut manifest = mind::util::FixedBuf::<16>::new();
        for byte in &slot.manifest[..8] { let _ = core::fmt::Write::write_fmt(&mut manifest, format_args!("{:02x}", byte)); }
        let key = core::str::from_utf8(&launch.key).unwrap_or("?").trim_end_matches('\0');
        let name = match slot.slot { BOOT_SLOT_A => "SLOT A", BOOT_SLOT_B => "SLOT B", _ => "THE VOLUME'S ROOT" };
        mind::println!("[INIT] LAUNCH: MANIFEST {} KEY {}{} VERIFIED, {} IMAGES CHECKED; {}{}", core::str::from_utf8(manifest.as_bytes()).unwrap_or("?"), key,
                       if launch.test_key != 0 { " (THE TEST KEY)" } else { "" }, launch.images, name, if slot.trial != 0 { " ON TRIAL" } else { "" });
    }
    // The last run's mark found: it stopped early, and this one starts safely (000-KRN-0076). The privilege is kept until
    // the mark is cleared, a minute after READY.
    let firmware = platform::cap(PLATFORM_PRIVILEGE, CAP_KIND_FIRMWARE, 0).ok();
    let (safe, marked) = firmware.map_or((false, false), run_mark);
    if safe {
        mind::println!("[INIT] SAFE START: THE LAST RUN DID NOT END AS ASKED (A FREEZE, A RESET OR A CUT POWER); NOT STARTED: {}, {}, {}, {}, {}",
                       SAFE_LEFT_OUT[0], SAFE_LEFT_OUT[1], SAFE_LEFT_OUT[2], SAFE_LEFT_OUT[3], SAFE_LEFT_OUT[4]);
    }
    if !marked { mind::println!("[INIT] NO RUN MARK: THE FIRMWARE KEEPS NO VARIABLES FOR IT; A RUN THAT STOPS EARLY IS NOT NOTICED"); }
    let mut mark = firmware.filter(|_| marked);
    if mark.is_none() { if let Some(f) = firmware { let _ = ipc::drop_cap(f); } }
    // A safe start asked at the bootloader (173-KRN-0036): the service configuration is not read.
    let asked = info.boot_flags & BOOT_SAFE != 0;
    let (healthy, mounted) = boot_services(&mut init, info, safe, asked);
    if cfg!(target_arch = "x86_64") { legacy::report(); } // the x86 legacy hardware (docs/legacy.md)
    // Applications may not take the frames a service restart needs (MC-6.5): a reserve only the system band uses.
    match platform::reserve_memory(RECOVERY_RESERVE_MIB << 20) {
        Ok(()) => mind::println!("[INIT] RECOVERY RESERVE {} MiB", RECOVERY_RESERVE_MIB),
        Err(_) => mind::println!("[INIT] NO RECOVERY RESERVE: APPLICATIONS MAY TAKE ALL TASK MEMORY"),
    }
    // No process control: init ends services and applications as their ancestor, the kernel's lifecycle rule (issue 170).
    // End of the initial distribution (MC-3.12): restarts need only what init keeps and the narrower restart privilege.
    // The boot is good when every boot service started and vfs_server mounted the boot volume; on a trial boot the
    // kernel restarts the machine at its deadline otherwise (351-KRN-0014). Only the platform privilege may confirm.
    // What the machine is, while the platform privilege allows it: log:hwNNNN.txt and log:acpi/ (174-KRN-0038).
    if let Ok(keeper) = init.keeper("vfs_server") { hardware::report(keeper); }
    let mounted = mounted.unwrap_or_else(|| wire::with_timeout(MOUNT_LIMIT_MS, || init.boot_volume_mounted()));
    // The machine's modes, once the boot volume is there; the Effector services follow them (750-KRN-0143).
    if mounted { init.read_modes(); init.follow_modes(); }
    // The Effector services a mode started are part of this boot's plan (173-KRN-0035).
    for index in MODE_SERVICES.iter().filter_map(|&name| Init::index(name)) { if init.running(index) { init.placed[index].off = Off::No; } }
    if healthy && mounted {
        if platform::confirm_boot() == Ok(true) { init.boot.confirmed = true; mind::println!("[INIT] TRIAL BOOT CONFIRMED: EVERY BOOT SERVICE STARTED, THE BOOT VOLUME MOUNTED"); }
    } else if info.boot_slot.trial != 0 {
        mind::println!("[INIT] TRIAL BOOT NOT CONFIRMED: {}", if mounted { "A BOOT SERVICE FAILED" } else { "NO BOOT VOLUME MOUNTED" });
    }
    match platform::cap(PLATFORM_PRIVILEGE, CAP_KIND_RESTART, 0) {
        Ok(_) => { let _ = ipc::drop_cap(SLOT_DEV0); mind::println!("[INIT] PLATFORM PRIVILEGE DROPPED"); }
        Err(error) => mind::println!("[INIT] KEEPS PLATFORM PRIVILEGE: {:?}", error),
    }
    mind::println!("[INIT] READY");
    let ready = mind::time::uptime_ms();
    // Exit notices of the services, and lifecycle requests (idl/init.wit) from the shell and the programs it lends
    // init's endpoint to; until the run's mark is cleared, a look at the clock every second.
    loop {
        let received = if mark.is_some() { Endpoint::SERVICE.recv_timeout(RECEIVED, 1000) } else { Endpoint::SERVICE.recv(RECEIVED) };
        if let Some(f) = mark.filter(|_| mind::time::uptime_ms() - ready >= MARK_CLEAR_MS) {
            let cleared = mind::firmware::set_in(f, RUN_MARK_NAME, &RUN_MARK_GUID, 0, &[]).is_ok();
            let _ = ipc::drop_cap(f);
            mark = None;
            mind::println!("[INIT] A MINUTE PAST READY: THE RUN'S MARK {}", if cleared { "CLEARED" } else { "COULD NOT BE CLEARED" });
        }
        let Ok(request) = received else { continue };
        if let Some(exit) = request.exit { init.ended(exit); continue; }
        if !request.is_call { continue; }
        let (badge, sender) = (request.badge, request.sender);
        let _ = match idl_init::decode(&request, RECEIVED) { Ok((decoded, call)) => init.serve(decoded, call, badge, sender), Err(reason) => wire::reject(reason) };
    }
}
