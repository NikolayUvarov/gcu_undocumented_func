#![no_std]
#![no_main]
// init: holds the bootstrap authority (platform privilege) and is the only place with service policy:
// which boot services start, in which order, and exactly which capabilities each one receives. After boot it keeps
// each service's capabilities for restarts and gives up the platform privilege (MC-3.12).
use mind::abi::*;
use mind::dev::cap_info;
use mind::idl::{init as idl_init, wire};
use mind::ipc::{self, Endpoint};
use mind::platform;
use mind::process::{grant, grant_moved, Image, Quota};
use mind::sys::{Error, Result};

const ALL: u8 = CAP_READ | CAP_WRITE | CAP_GRANT;
const RECEIVED: usize = 9; // fixed slot for the buffer of an idl/init.wit call
const CLIENT: u8 = CAP_WRITE | CAP_GRANT;
// DMA buffer sizes of the drivers; the regions are minted once and survive driver restarts.
const APP_ENDPOINTS: usize = 4; // endpoints each application may create (loader passes them on)
const AHCI_DMA_BYTES: usize = 128 * 1024; // commands, FIS and a 64 KiB data buffer
const XHCI_DMA_BYTES: usize = 256 * 1024; // rings, contexts, scratchpad and a 64 KiB data buffer
const AUDIO_DMA_BYTES: usize = (33 + 17) * 4096; // playback: 32 buffers + list; capture: 16 buffers + list

// Capabilities minted for a service's first start; kept by init for restarts, or dropped if the spawn fails.
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

// A service gets copies of what init keeps: children init can revoke, and the same set again after a restart.
#[derive(Clone, Copy)]
struct Grants { list: [Grant; SPAWN_GRANTS_MAX], count: usize }
impl Grants {
    fn new() -> Self { Self { list: [Grant::default(); SPAWN_GRANTS_MAX], count: 0 } }
    fn add(&mut self, child: usize, own: usize, rights: u8) { self.list[self.count] = grant(child, own, rights); self.count += 1; }
    fn copy(&mut self, child: usize, own: usize, rights: u8) { self.add(child, own, rights); }
}
// How a service was started: its grants (over capabilities init keeps), spawn flags and quota.
#[derive(Clone, Copy)]
struct Plan { grants: Grants, flags: usize, quota: Quota }

// Per boot service: PID, DMA region and the keeper of its endpoint (can mint receive rights, cannot receive itself).
struct Init { plans: [Option<Plan>; BOOT_IMAGES], pids: [u64; BOOT_IMAGES], dma: [Option<usize>; BOOT_IMAGES], devices: [Option<usize>; BOOT_IMAGES], keepers: [Option<usize>; BOOT_IMAGES], restarts: [[u64; RESTART_BUDGET]; BOOT_IMAGES], quarantined: [bool; BOOT_IMAGES] }

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
    // The server's receive capability is minted fresh for each instance and moved into it (see `spawn`): init keeps
    // only the keeper, so a dead or quarantined server's clients get ERR_PEER and nothing queues for the next one.
    fn server(&mut self, _minted: &mut Minted, name: &str) -> Result<usize> { self.keeper(name) }
    fn client(&mut self, minted: &mut Minted, name: &str) -> Result<usize> { let keeper = self.keeper(name)?; minted.endpoint(keeper, CLIENT) }

    // Before a driver is restarted (MC-6.3): its device stops DMA, then the DMA region is cleared, so the new instance
    // starts from a quiet device and no residue of the old one.
    fn quiesce(&mut self, index: usize) {
        if let Some(device) = self.devices[index] { match platform::quiesce(device) { Ok(()) => mind::println!("[INIT] {} DEVICE QUIESCED", BOOT_SERVICES[index]), Err(error) => mind::println!("[INIT] {} QUIESCE FAILED: {:?}", BOOT_SERVICES[index], error) } }
        if let Some(slot) = self.dma[index] { if let Ok(mut region) = mind::mem::Mapping::new(slot) { region.as_mut_slice().fill(0); } }
    }

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
        if let Some(plan) = self.plans[index] {
            // A restart: the device was stopped when its driver ended; the same capabilities are granted again.
            if let Some(device) = self.devices[index] { let _ = platform::resume(device); }
            return self.spawn(index, plan);
        }
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
                let device = platform::find_device(0x01_06_01, 0xFF_FF_FF, 0)?; self.devices[index] = Some(device);
                grants.add(SLOT_DEV0, Self::bar(&mut minted, device, 5, CAP_KIND_MMIO)?, 0);
                grants.add(SLOT_SERVICE, self.server(&mut minted, "ahci")?, ALL); grants.copy(SLOT_MEM, self.dma(index, AHCI_DMA_BYTES)?, 0);
            }
            "usb_storage" => {
                // First xHCI controller (class 0C:03:30): registers in BAR0.
                let device = platform::find_device(0x0C_03_30, 0xFF_FF_FF, 0)?; self.devices[index] = Some(device);
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
                    self.devices[index] = Some(device);
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
        let plan = Plan { grants, flags, quota };
        let pid = self.spawn(index, plan)?;
        minted.count = 0; // kept for restarts
        self.plans[index] = Some(plan);
        Ok(pid)
    }

    fn spawn(&mut self, index: usize, plan: Plan) -> Result<u64> {
        let name = BOOT_SERVICES[index];
        let mut grants = plan.grants; let mut receiver = None;
        if let Some(keeper) = self.keepers[index] {
            for g in grants.list[..grants.count].iter_mut().filter(|g| g.own as usize == keeper && g.child as usize == SLOT_SERVICE) {
                let slot = ipc::mint(keeper, ALL, 0, 0)?; *g = grant_moved(SLOT_SERVICE, slot, ALL); receiver = Some(slot);
            }
        }
        let pid = mind::process::spawn_raw(name.as_bytes(), Image::Boot(index), &grants.list[..grants.count], plan.flags, plan.quota)
            .inspect_err(|_| { if let Some(slot) = receiver { let _ = ipc::drop_cap(slot); } })?;
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
        self.quiesce(index);
        match self.start(index) {
            Ok(pid) => mind::println!("[INIT] {} RESTARTED PID={}", name, pid),
            Err(error) => mind::println!("[INIT] {} RESTART FAILED: {:?}", name, error),
        }
    }
}

fn service_index(name: &str) -> usize { BOOT_SERVICES.iter().position(|s| *s == name).unwrap_or(0) }

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut init = Init { plans: [None; BOOT_IMAGES], pids: [0; BOOT_IMAGES], dma: [None; BOOT_IMAGES], devices: [None; BOOT_IMAGES], keepers: [None; BOOT_IMAGES], restarts: [[0; RESTART_BUDGET]; BOOT_IMAGES], quarantined: [false; BOOT_IMAGES] };
    // Boot order is the BOOT_SERVICES order: drivers before vfs_server, loader before the shell.
    for index in 1..BOOT_IMAGES {
        match init.start(index) {
            Ok(_) => {}
            Err(Error::NotFound) => mind::println!("[INIT] {} NOT STARTED: NO DEVICE", BOOT_SERVICES[index]),
            Err(error) => mind::println!("[INIT] {} FAILED: {:?}", BOOT_SERVICES[index], error),
        }
    }
    // End of the initial distribution (MC-3.12): restarts need only what init keeps and the narrower restart privilege.
    match platform::cap(PLATFORM_PRIVILEGE, CAP_KIND_RESTART, 0) {
        Ok(_) => { let _ = ipc::drop_cap(SLOT_DEV0); mind::println!("[INIT] PLATFORM PRIVILEGE DROPPED"); }
        Err(error) => mind::println!("[INIT] KEEPS PLATFORM PRIVILEGE: {:?}", error),
    }
    mind::println!("[INIT] READY");
    // Exit notices of the services, and requests from the shell: start a boot service by name (msg[2..4]).
    loop {
        let Ok(request) = Endpoint::SERVICE.recv(RECEIVED) else { continue };
        if let Some(exit) = request.exit { init.ended(exit); continue; }
        if !request.is_call { continue; }
        // idl/init.wit
        let (name, call) = match idl_init::decode(&request, RECEIVED) { Ok((idl_init::Request::Run { name }, call)) => (name, call), Err(reason) => { let _ = wire::reject(reason); continue; } };
        let index = BOOT_SERVICES.iter().position(|s| s.as_bytes().eq_ignore_ascii_case(name.as_str().as_bytes()));
        // An explicit RUN is the operator's decision: it lifts a quarantine and resets the restart budget.
        if let Some(index) = index.filter(|&i| !init.running(i)) { init.quarantined[index] = false; init.restarts[index] = [0; RESTART_BUDGET]; if init.pids[index] != 0 { init.quiesce(index); } }
        let result = match index.map(|index| init.start(index)) { None => Err(Error::NotFound), Some(result) => result };
        let _ = idl_init::reply_run(call, result);
    }
}
