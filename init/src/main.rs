#![no_std]
#![no_main]
// init: holds the bootstrap authority (platform privilege) and is the only place with service policy:
// which boot services start, in which order, and exactly which capabilities each one receives.
use mind::abi::*;
use mind::dev::cap_info;
use mind::ipc::{self, Endpoint, Message};
use mind::platform;
use mind::process::{grant, grant_moved, Image, Quota};
use mind::sys::{Error, Result};

const ALL: u8 = CAP_READ | CAP_WRITE | CAP_GRANT;
const CLIENT: u8 = CAP_WRITE | CAP_GRANT;
// DMA buffer sizes of the drivers; the regions are minted once and survive driver restarts.
const APP_ENDPOINTS: usize = 4; // endpoints each application may create (loader passes them on)
const AHCI_DMA_BYTES: usize = 128 * 1024; // commands, FIS and a 64 KiB data buffer
const XHCI_DMA_BYTES: usize = 256 * 1024; // rings, contexts, scratchpad and a 64 KiB data buffer
const AUDIO_DMA_BYTES: usize = (33 + 17) * 4096; // playback: 32 buffers + list; capture: 16 buffers + list

// Capabilities minted for one spawn; moved into the child, or dropped if the spawn fails.
struct Minted { slots: [usize; SPAWN_GRANTS_MAX], count: usize }
impl Minted {
    fn new() -> Self { Self { slots: [0; SPAWN_GRANTS_MAX], count: 0 } }
    fn mint(&mut self, kind: usize, a: usize, b: usize) -> Result<usize> {
        let slot = platform::cap(kind, a, b)?;
        self.slots[self.count] = slot; self.count += 1;
        Ok(slot)
    }
    // A child of a service endpoint's keeper: all rights for the server, write/grant for a client.
    fn endpoint(&mut self, keeper: usize, rights: u8) -> Result<usize> {
        let slot = ipc::mint(keeper, rights, 0, 0)?;
        self.slots[self.count] = slot; self.count += 1;
        Ok(slot)
    }
    fn ports(&mut self, base: usize, count: usize) -> Result<usize> { self.mint(PLATFORM_PORTS, base, count) }
    fn privilege(&mut self, kind: usize) -> Result<usize> { self.mint(PLATFORM_PRIVILEGE, kind, 0) }
}
impl Drop for Minted { fn drop(&mut self) { for &slot in &self.slots[..self.count] { let _ = ipc::drop_cap(slot); } } }

// Minted capabilities are moved into the child; kept ones (init's endpoint, DMA regions) are copied, so init can revoke them.
struct Grants { list: [Grant; SPAWN_GRANTS_MAX], count: usize }
impl Grants {
    fn new() -> Self { Self { list: [Grant::default(); SPAWN_GRANTS_MAX], count: 0 } }
    fn add(&mut self, child: usize, own: usize, rights: u8) { self.list[self.count] = grant_moved(child, own, rights); self.count += 1; }
    fn copy(&mut self, child: usize, own: usize, rights: u8) { self.list[self.count] = grant(child, own, rights); self.count += 1; }
}

// Per boot service: PID, DMA region and the keeper of its endpoint (can mint receive rights, cannot receive itself).
struct Init { pids: [u64; BOOT_IMAGES], dma: [Option<usize>; BOOT_IMAGES], keepers: [Option<usize>; BOOT_IMAGES], restarts: [[u64; RESTART_BUDGET]; BOOT_IMAGES], quarantined: [bool; BOOT_IMAGES] }

// Restart budget (MC-6.5): at most RESTART_BUDGET automatic restarts of one service within RESTART_WINDOW_MS.
const RESTART_BUDGET: usize = 3;
const RESTART_WINDOW_MS: u64 = 60_000;

impl Init {
    fn running(&self, index: usize) -> bool { self.pids[index] != 0 && mind::process::alive(self.pids[index]) }

    // Keeper of the endpoint of service `name`, created on first use and kept across restarts, so clients granted
    // earlier reach the restarted server; while no server runs, sends fail with ERR_PEER.
    fn keeper(&mut self, name: &str) -> Result<usize> {
        let index = service_index(name);
        if let Some(slot) = self.keepers[index] { return Ok(slot); }
        let root = Endpoint::create()?.0;
        let keeper = ipc::mint(root, CAP_KEEP | CAP_WRITE | CAP_GRANT, 0, 0);
        let _ = ipc::drop_cap(root);
        let keeper = keeper?;
        self.keepers[index] = Some(keeper);
        Ok(keeper)
    }
    fn server(&mut self, minted: &mut Minted, name: &str) -> Result<usize> { let keeper = self.keeper(name)?; minted.endpoint(keeper, ALL) }
    fn client(&mut self, minted: &mut Minted, name: &str) -> Result<usize> { let keeper = self.keeper(name)?; minted.endpoint(keeper, CLIENT) }

    fn dma(&mut self, index: usize, bytes: usize) -> Result<usize> {
        if let Some(slot) = self.dma[index] { return Ok(slot); }
        let slot = platform::cap(PLATFORM_DMA, bytes, 0)?;
        self.dma[index] = Some(slot);
        Ok(slot)
    }

    // A PCI device BAR of the expected kind (port range or MMIO), or NotFound.
    fn bar(minted: &mut Minted, device: usize, bar: usize, kind: usize) -> Result<usize> {
        let slot = minted.mint(PLATFORM_DEVICE_BAR, device, bar)?;
        if cap_info(slot).0 == kind { Ok(slot) } else { Err(Error::NotFound) }
    }

    // Starts boot service `index` with its capabilities; Err(NotFound) if its hardware is absent.
    fn start(&mut self, index: usize) -> Result<u64> {
        let name = BOOT_SERVICES[index];
        if index == 0 || self.running(index) { return Err(Error::Other(ERR_BUSY)); }
        let mut minted = Minted::new();
        let mut grants = Grants::new();
        let mut flags = SPAWN_SERVICE;
        match name {
            "rtc" => { grants.add(SLOT_SERVICE, self.server(&mut minted, "rtc")?, ALL); grants.add(SLOT_DEV0, minted.ports(0x70, 2)?, 0); }
            "ps2_kbd" => {
                grants.add(SLOT_DEV0, minted.ports(0x60, 1)?, 0); grants.add(SLOT_DEV1, minted.ports(0x64, 1)?, 0);
                grants.add(SLOT_IRQ, minted.mint(PLATFORM_IRQ, 1, 0)?, 0); grants.add(SLOT_PRIV, minted.privilege(CAP_KIND_INPUT)?, 0);
            }
            "compositor" => { grants.add(SLOT_MEM, minted.mint(PLATFORM_FRAMEBUFFER, 0, 0)?, 0); grants.add(SLOT_PRIV, minted.privilege(CAP_KIND_DISPLAY)?, 0); }
            "ata" => {
                grants.add(SLOT_SERVICE, self.server(&mut minted, "ata")?, ALL);
                grants.add(SLOT_DEV0, minted.ports(0x1F0, 8)?, 0); grants.add(SLOT_DEV1, minted.ports(0x3F6, 1)?, 0);
            }
            "ahci" => {
                // First SATA controller in AHCI mode (class 01:06:01): ABAR is BAR5.
                let device = platform::find_device(0x01_06_01, 0xFF_FF_FF, 0)?;
                grants.add(SLOT_DEV0, Self::bar(&mut minted, device, 5, CAP_KIND_MMIO)?, 0);
                grants.add(SLOT_SERVICE, self.server(&mut minted, "ahci")?, ALL); grants.copy(SLOT_MEM, self.dma(index, AHCI_DMA_BYTES)?, 0);
            }
            "usb_storage" => {
                // First xHCI controller (class 0C:03:30): registers in BAR0.
                let device = platform::find_device(0x0C_03_30, 0xFF_FF_FF, 0)?;
                grants.add(SLOT_DEV0, Self::bar(&mut minted, device, 0, CAP_KIND_MMIO)?, 0);
                grants.add(SLOT_SERVICE, self.server(&mut minted, "usb_storage")?, ALL); grants.copy(SLOT_MEM, self.dma(index, XHCI_DMA_BYTES)?, 0);
            }
            "vfs_server" => {
                // VFS sees only block devices whose drivers are actually running.
                grants.add(SLOT_SERVICE, self.server(&mut minted, "vfs_server")?, ALL);
                let mut slot = SLOT_BLOCK_FIRST;
                for driver in ["ata", "ahci", "usb_storage"] {
                    if self.running(service_index(driver)) { grants.add(slot, self.client(&mut minted, driver)?, CLIENT); slot += 1; }
                }
            }
            "loader" => {
                grants.add(SLOT_SERVICE, self.server(&mut minted, "loader")?, ALL);
                grants.add(2, self.client(&mut minted, "rtc")?, CLIENT); grants.add(3, self.client(&mut minted, "vfs_server")?, CLIENT);
                grants.add(4, self.client(&mut minted, "audio_gw")?, CLIENT); grants.add(5, minted.privilege(CAP_KIND_SPAWN)?, 0);
                grants.add(6, self.client(&mut minted, "tts")?, CLIENT);
            }
            "audio_gw" => {
                grants.add(SLOT_SERVICE, self.server(&mut minted, "audio_gw")?, ALL);
                // AC97 (class 04:01): mixer and bus master port ranges and an IRQ line; without it the gateway reports no device.
                if let Ok(device) = platform::find_device(0x04_01_00, 0xFF_FF_00, 0) {
                    let devices = (|| -> Result<[usize; 3]> { Ok([Self::bar(&mut minted, device, 0, CAP_KIND_PORTS)?, Self::bar(&mut minted, device, 1, CAP_KIND_PORTS)?, minted.mint(PLATFORM_DEVICE_IRQ, device, 0)?]) })();
                    if let Ok([mixer, bus_master, irq]) = devices {
                        grants.add(SLOT_DEV0, mixer, 0); grants.add(SLOT_DEV1, bus_master, 0); grants.add(SLOT_IRQ, irq, 0);
                        grants.copy(SLOT_MEM, self.dma(index, AUDIO_DMA_BYTES)?, 0);
                    }
                }
            }
            "tts" => { grants.add(SLOT_SERVICE, self.server(&mut minted, "tts")?, ALL); grants.add(SLOT_AUDIO, self.client(&mut minted, "audio_gw")?, CLIENT); }
            "shell" => {
                // Application slots plus process control, input injection (UART) and the COM1 ports.
                flags |= SPAWN_SCREEN;
                grants.copy(SLOT_INIT, SLOT_SERVICE, CLIENT);
                for (slot, service) in [(SLOT_RTC, "rtc"), (SLOT_VFS, "vfs_server"), (SLOT_AUDIO, "audio_gw"), (SLOT_LOADER, "loader"), (SLOT_TTS, "tts")] { grants.add(slot, self.client(&mut minted, service)?, CLIENT); }
                grants.add(SLOT_CONTROL, minted.privilege(CAP_KIND_CONTROL)?, 0); grants.add(SLOT_INPUT, minted.privilege(CAP_KIND_INPUT)?, 0);
                grants.add(SLOT_SERIAL, minted.ports(0x3F8, 8)?, 0);
            }
            _ => return Err(Error::NotFound),
        }
        // Quotas are init's policy: loader may run MAX_APPS applications with APP_ENDPOINTS endpoints each.
        let quota = if name == "loader" { Quota { tasks: MAX_APPS as u16, endpoints: (MAX_APPS * APP_ENDPOINTS) as u16 } } else { Quota::default() };
        let pid = mind::process::spawn_raw(name.as_bytes(), Image::Boot(index), &grants.list[..grants.count], flags, quota)?;
        self.pids[index] = pid;
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
        let Some(index) = (1..BOOT_IMAGES).find(|&i| self.pids[i] == exit.pid) else { return };
        let name = BOOT_SERVICES[index];
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

fn service_index(name: &str) -> usize { BOOT_SERVICES.iter().position(|s| *s == name).unwrap_or(0) }

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut init = Init { pids: [0; BOOT_IMAGES], dma: [None; BOOT_IMAGES], keepers: [None; BOOT_IMAGES], restarts: [[0; RESTART_BUDGET]; BOOT_IMAGES], quarantined: [false; BOOT_IMAGES] };
    // Boot order is the BOOT_SERVICES order: drivers before vfs_server, loader before the shell.
    for index in 1..BOOT_IMAGES {
        match init.start(index) {
            Ok(_) => {}
            Err(Error::NotFound) => mind::println!("[INIT] {} NOT STARTED: NO DEVICE", BOOT_SERVICES[index]),
            Err(error) => mind::println!("[INIT] {} FAILED: {:?}", BOOT_SERVICES[index], error),
        }
    }
    mind::println!("[INIT] READY");
    // Exit notices of the services, and requests from the shell: start a boot service by name (msg[2..4]).
    loop {
        let Ok(request) = Endpoint::SERVICE.recv(0) else { continue };
        if let Some(exit) = request.exit { init.ended(exit); continue; }
        if !request.is_call { continue; }
        let (packed, len) = mind::process::unpack_name(request.data);
        let index = BOOT_SERVICES.iter().position(|s| s.as_bytes().eq_ignore_ascii_case(&packed[..len]));
        // An explicit RUN is the operator's decision: it lifts a quarantine and resets the restart budget.
        if let Some(index) = index.filter(|&i| !init.running(i)) { init.quarantined[index] = false; init.restarts[index] = [0; RESTART_BUDGET]; }
        let code = match index.map(|index| init.start(index)) {
            None => ERR_NOT_FOUND,
            Some(Ok(pid)) => pid as usize,
            Some(Err(error)) => error.code(),
        };
        let _ = ipc::reply(&Message::new(code, 0));
    }
}
