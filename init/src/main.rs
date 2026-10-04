#![no_std]
#![no_main]
// init: holds the bootstrap authority (platform privilege) and is the only place with service policy:
// which boot services start, in which order, and exactly which capabilities each one receives. After boot it keeps
// each service's capabilities for restarts and gives up the platform privilege (MC-3.12). As the lifecycle owner it
// restarts failed services within a budget and serves idl/init.wit: start, list, stop and restart services, stop an
// application.
use mind::abi::*;
use mind::dev::cap_info;
use mind::idl::{init as idl_init, wire};
use mind::ipc::{self, Endpoint};
use mind::platform;
use mind::process::{grant, grant_moved, Image, Quota};
use mind::sys::{Error, Result};

const ALL: u8 = CAP_READ | CAP_WRITE | CAP_GRANT;
const RECEIVED: usize = 9; // fixed slot for the buffer of an idl/init.wit call
const INIT_PID: u64 = 1; // the kernel's first task
// What each boot service holds, for `svc` (the grants below, in short).
const HOLDS: [&str; BOOT_IMAGES] = ["restart and process control", "observe privilege",
    "ports 0x70-0x71", "ports 0x60, 0x64; IRQ 1; input", "framebuffer; display", "ports 0x1F0-0x1F7, 0x3F6", "AHCI registers; 128 KiB DMA",
    "xHCI registers; 256 KiB DMA", "8 MiB of memory", "write clients of the block devices", "spawn privilege", "AC97 ports and IRQ; DMA",
    "an audio client", "observe privilege", "screen; process control; input; COM1"];
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
    // A client endpoint with a badge the server checks (the write right of a block device, the user's files, reading the log).
    fn badged(&mut self, keeper: usize, rights: u8, badge: u16) -> Result<usize> {
        let slot = ipc::mint_badged(keeper, rights, badge)?;
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

// Per boot service: PID, DMA region and the keeper of its endpoint (can mint receive rights, cannot receive itself);
// how often it was started, whether it was stopped on request (then it is not restarted) and whether its device was
// missing at boot.
struct Init { plans: [Option<Plan>; BOOT_IMAGES], pids: [u64; BOOT_IMAGES], dma: [Option<usize>; BOOT_IMAGES], devices: [Option<usize>; BOOT_IMAGES], keepers: [Option<usize>; BOOT_IMAGES], restarts: [[u64; RESTART_BUDGET]; BOOT_IMAGES], quarantined: [bool; BOOT_IMAGES], starts: [u32; BOOT_IMAGES], stopped: [bool; BOOT_IMAGES], missing: [bool; BOOT_IMAGES] }

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
    // A client of service `name` in the child's `slot`: a copy of the keeper narrowed to send rights, so init needs no
    // slot of its own for it (only badged clients are minted and kept).
    fn lend(&mut self, grants: &mut Grants, slot: usize, name: &str) -> Result<()> { let keeper = self.keeper(name)?; grants.copy(slot, keeper, CLIENT); Ok(()) }
    fn badged(&mut self, minted: &mut Minted, name: &str, badge: u16) -> Result<usize> { let keeper = self.keeper(name)?; minted.badged(keeper, CLIENT, badge) }

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
            "ramdisk" => grants.add(SLOT_SERVICE, self.server(&mut minted, "ramdisk")?, ALL),
            "vfs_server" => {
                // VFS sees only block devices whose drivers are actually running; it alone may write to them (B.6).
                grants.add(SLOT_SERVICE, self.server(&mut minted, "vfs_server")?, ALL);
                let mut slot = SLOT_BLOCK_FIRST;
                for driver in ["ata", "ahci", "usb_storage"] {
                    if self.running(service_index(driver)) { grants.add(slot, self.badged(&mut minted, driver, mind::block::BADGE_WRITE)?, CLIENT); slot += 1; }
                }
                if self.running(service_index("ramdisk")) { grants.add(SLOT_RAMDISK, self.badged(&mut minted, "ramdisk", mind::block::BADGE_WRITE)?, CLIENT); }
                self.lend(&mut grants, SLOT_VFS_RTC, "rtc")?;
            }
            "loader" => {
                grants.add(SLOT_SERVICE, self.server(&mut minted, "loader")?, ALL);
                self.lend(&mut grants, 2, "rtc")?; self.lend(&mut grants, 3, "vfs_server")?;
                self.lend(&mut grants, 4, "audio_gw")?; grants.add(5, minted.privilege(CAP_KIND_SPAWN)?, 0);
                self.lend(&mut grants, 6, "tts")?;
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
            "tts" => { grants.add(SLOT_SERVICE, self.server(&mut minted, "tts")?, ALL); self.lend(&mut grants, SLOT_AUDIO, "audio_gw")?; }
            // The observe privilege (read-only statistics, MC-10.2) goes to sysmon and to logd, which names the sender
            // of a record from the kernel's task records (MC-10.6).
            "logd" => { grants.add(SLOT_SERVICE, self.server(&mut minted, "logd")?, ALL); grants.add(SLOT_PRIV, minted.privilege(CAP_KIND_OBSERVE)?, 0); }
            "sysmon" => { grants.add(SLOT_SERVICE, self.server(&mut minted, "sysmon")?, ALL); grants.add(SLOT_PRIV, minted.privilege(CAP_KIND_OBSERVE)?, 0); }
            "shell" => {
                // Application slots plus process control, input injection (UART) and the COM1 ports.
                flags |= SPAWN_SCREEN;
                grants.copy(SLOT_INIT, SLOT_SERVICE, CLIENT);
                for (slot, service) in [(SLOT_RTC, "rtc"), (SLOT_AUDIO, "audio_gw"), (SLOT_LOADER, "loader"), (SLOT_TTS, "tts")] { self.lend(&mut grants, slot, service)?; }
                // The user's file client: writes on ram: and in the boot disk's data directory (applications read only).
                grants.add(SLOT_VFS, self.badged(&mut minted, "vfs_server", mind::fs::BADGE_USER)?, CLIENT);
                grants.add(SLOT_CONTROL, minted.privilege(CAP_KIND_CONTROL)?, 0); grants.add(SLOT_INPUT, minted.privilege(CAP_KIND_INPUT)?, 0);
                grants.add(SLOT_SERIAL, minted.ports(0x3F8, 8)?, 0);
                self.lend(&mut grants, SLOT_SYSINFO, "sysmon")?;
            }
            _ => return Err(Error::NotFound),
        }
        // Every service writes to the system log; the shell's client may also read it (and lends it to dmesg).
        if name == "shell" { grants.add(SLOT_LOG, self.badged(&mut minted, "logd", mind::log::BADGE_READ)?, CLIENT); }
        else if name != "logd" { self.lend(&mut grants, SLOT_LOG, "logd")?; }
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
        let Some(index) = (1..BOOT_IMAGES).find(|&i| self.pids[i] == exit.pid) else { return };
        let name = BOOT_SERVICES[index];
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
        self.quiesce(index);
        match self.start(index) {
            Ok(pid) => mind::println!("[INIT] {} RESTARTED PID={}", name, pid),
            Err(error) => mind::println!("[INIT] {} RESTART FAILED: {:?}", name, error),
        }
    }
}

// Lifecycle requests (idl/init.wit 1.1). Stopping uses the process control privilege init keeps after boot; init and
// the shell are never stopped.
impl Init {
    fn index(name: &str) -> Option<usize> { BOOT_SERVICES.iter().position(|s| s.eq_ignore_ascii_case(name)) }

    // `run`: an explicit start is the operator's decision: it lifts a quarantine and resets the restart budget. A service
    // whose device was missing at boot has nothing to start (init no longer looks for devices).
    fn run(&mut self, index: usize) -> Result<u64> {
        if self.missing[index] && self.plans[index].is_none() { return Err(Error::NotFound); }
        if !self.running(index) { self.quarantined[index] = false; self.restarts[index] = [0; RESTART_BUDGET]; if self.pids[index] != 0 { self.quiesce(index); } }
        self.start(index)
    }

    fn start_service(&mut self, index: usize) -> core::result::Result<u64, idl_init::Error> {
        if index == 0 || self.running(index) { return Err(idl_init::Error::Running); }
        match self.run(index) {
            Ok(pid) => Ok(pid),
            Err(Error::NotFound) => Err(idl_init::Error::NoDevice),
            Err(error) => { mind::println!("[INIT] {} FAILED: {:?}", BOOT_SERVICES[index], error); Err(idl_init::Error::Failed) }
        }
    }

    fn stop_service(&mut self, index: usize) -> core::result::Result<(), idl_init::Error> {
        if index == 0 || BOOT_SERVICES[index] == "shell" { return Err(idl_init::Error::Denied); }
        if !self.running(index) { return Err(idl_init::Error::Stopped); }
        let pid = self.pids[index];
        self.stopped[index] = true;
        if mind::control::kill(pid).is_err() { self.stopped[index] = false; return Err(idl_init::Error::Failed); }
        for _ in 0..200 { if !mind::process::alive(pid) { break; } mind::time::sleep(10); }
        // Its device stops DMA until the next start (MC-6.3).
        self.quiesce(index);
        mind::println!("[INIT] STOPPED {} PID={}", BOOT_SERVICES[index], pid);
        Ok(())
    }

    fn list(&self) -> [idl_init::Service; BOOT_IMAGES] {
        core::array::from_fn(|i| idl_init::Service {
            name: wire_text(BOOT_SERVICES[i]), pid: if i == 0 { INIT_PID } else if self.running(i) { self.pids[i] } else { 0 },
            starts: if i == 0 { 1 } else { self.starts[i] }, running: i == 0 || self.running(i), quarantined: self.quarantined[i], holds: wire_text(HOLDS[i]),
        })
    }

    fn serve(&mut self, request: idl_init::Request, call: wire::Call) -> Result<()> {
        use idl_init::{Error as E, Request};
        match request {
            Request::Run { name } => {
                let result = match Self::index(name.as_str()) { None | Some(0) => Err(Error::NotFound), Some(index) => self.run(index) };
                idl_init::reply_run(call, result)
            }
            Request::List => idl_init::reply_list(call, Ok(&self.list()[..])),
            Request::Stop { name } => { let result = Self::index(name.as_str()).ok_or(E::NotFound).and_then(|i| self.stop_service(i)); idl_init::reply_stop(call, result) }
            Request::Restart { name } => {
                let result = Self::index(name.as_str()).ok_or(E::NotFound).and_then(|i| {
                    if i == 0 || BOOT_SERVICES[i] == "shell" { return Err(E::Denied); }
                    if self.running(i) { self.stop_service(i)?; }
                    self.start_service(i)
                });
                idl_init::reply_restart(call, result)
            }
            Request::StopTask { pid } => {
                let service = pid == INIT_PID || (1..BOOT_IMAGES).any(|i| self.pids[i] == pid && self.running(i));
                let result = if service { Err(E::Denied) } else if !mind::process::alive(pid) { Err(E::NotFound) } else { mind::control::kill(pid).map_err(|_| E::NotFound) };
                idl_init::reply_stop_task(call, result)
            }
        }
    }
}

fn wire_text<const N: usize>(text: &str) -> mind::idl::codec::Text<N> {
    let mut end = text.len().min(N);
    while !text.is_char_boundary(end) { end -= 1; }
    mind::idl::codec::Text::new(&text[..end]).unwrap_or_default()
}

fn service_index(name: &str) -> usize { BOOT_SERVICES.iter().position(|s| *s == name).unwrap_or(0) }

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut init = Init { plans: [None; BOOT_IMAGES], pids: [0; BOOT_IMAGES], dma: [None; BOOT_IMAGES], devices: [None; BOOT_IMAGES], keepers: [None; BOOT_IMAGES], restarts: [[0; RESTART_BUDGET]; BOOT_IMAGES], quarantined: [false; BOOT_IMAGES], starts: [0; BOOT_IMAGES], stopped: [false; BOOT_IMAGES], missing: [false; BOOT_IMAGES] };
    // Boot order is the BOOT_SERVICES order: logd first, drivers before vfs_server, loader before the shell.
    for index in 1..BOOT_IMAGES {
        match init.start(index) {
            // From now on init's own lines (and those printed so far) go to the system log too.
            Ok(_) if BOOT_SERVICES[index] == "logd" => {
                // The keeper may send: init needs no client of its own.
                if let Ok(keeper) = init.keeper("logd") { mind::log::use_endpoint(Endpoint(keeper)); }
            }
            Ok(_) => {}
            Err(Error::NotFound) => { init.missing[index] = true; mind::println!("[INIT] {} NOT STARTED: NO DEVICE", BOOT_SERVICES[index]); }
            Err(error) => mind::println!("[INIT] {} FAILED: {:?}", BOOT_SERVICES[index], error),
        }
    }
    // Process control, to stop services and applications on request (init is their lifecycle owner).
    if platform::cap(PLATFORM_PRIVILEGE, CAP_KIND_CONTROL, 0).is_err() { mind::println!("[INIT] NO PROCESS CONTROL: STOP REQUESTS WILL FAIL"); }
    // End of the initial distribution (MC-3.12): restarts need only what init keeps and the narrower restart privilege.
    match platform::cap(PLATFORM_PRIVILEGE, CAP_KIND_RESTART, 0) {
        Ok(_) => { let _ = ipc::drop_cap(SLOT_DEV0); mind::println!("[INIT] PLATFORM PRIVILEGE DROPPED"); }
        Err(error) => mind::println!("[INIT] KEEPS PLATFORM PRIVILEGE: {:?}", error),
    }
    mind::println!("[INIT] READY");
    // Exit notices of the services, and lifecycle requests (idl/init.wit) from the shell and the programs it lends
    // init's endpoint to.
    loop {
        let Ok(request) = Endpoint::SERVICE.recv(RECEIVED) else { continue };
        if let Some(exit) = request.exit { init.ended(exit); continue; }
        if !request.is_call { continue; }
        let _ = match idl_init::decode(&request, RECEIVED) { Ok((request, call)) => init.serve(request, call), Err(reason) => wire::reject(reason) };
    }
}
