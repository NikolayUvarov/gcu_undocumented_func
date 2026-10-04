#![no_std]
#![no_main]
// init: holds the bootstrap authority (platform privilege) and is the only place with service policy:
// which boot services start, in which order, and exactly which capabilities each one receives.
use mind::abi::*;
use mind::dev::cap_info;
use mind::idl::{lifecycle, wire};
use mind::ipc::{self, Endpoint};
use mind::mem::Mapping;
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
const RECEIVED: usize = 9; // the buffer a lifecycle request lends
const INIT_PID: u64 = 1; // the kernel's first task
// What each boot service holds, for `svc` (the grants below, in short).
const HOLDS: [&str; BOOT_IMAGES] = ["platform and spawn privileges", "observe privilege", "ports 0x70-0x71", "ports 0x60, 0x64; IRQ 1; input",
    "framebuffer; display", "ports 0x1F0-0x1F7, 0x3F6", "AHCI registers; 128 KiB DMA", "xHCI registers; 256 KiB DMA", "8 MiB of memory",
    "write clients of the block devices", "spawn privilege", "AC97 ports and IRQ; DMA", "an audio client", "observe privilege",
    "screen; process control; input; COM1"];

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
    // A client endpoint with a badge the server checks (the write right of a block device).
    fn badged(&mut self, keeper: usize, rights: u8, badge: u16) -> Result<usize> {
        let slot = ipc::mint_badged(keeper, rights, badge)?;
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
struct Init { pids: [u64; BOOT_IMAGES], dma: [Option<usize>; BOOT_IMAGES], keepers: [Option<usize>; BOOT_IMAGES], starts: [u32; BOOT_IMAGES] }

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
    // A client of service `name` in the child's `slot`: a copy of the keeper narrowed to send rights, so init needs no
    // slot of its own for it (only badged clients are minted first).
    fn lend(&mut self, grants: &mut Grants, slot: usize, name: &str) -> Result<()> { let keeper = self.keeper(name)?; grants.copy(slot, keeper, CLIENT); Ok(()) }

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
            "ramdisk" => grants.add(SLOT_SERVICE, self.server(&mut minted, "ramdisk")?, ALL),
            "vfs_server" => {
                // VFS sees only block devices whose drivers are actually running; it alone may write to them (B.6).
                grants.add(SLOT_SERVICE, self.server(&mut minted, "vfs_server")?, ALL);
                let mut slot = SLOT_BLOCK_FIRST;
                for driver in ["ata", "ahci", "usb_storage"] {
                    if self.running(service_index(driver)) { let keeper = self.keeper(driver)?; grants.add(slot, minted.badged(keeper, CLIENT, BLOCK_BADGE_WRITE)?, CLIENT); slot += 1; }
                }
                if self.running(service_index("ramdisk")) { let keeper = self.keeper("ramdisk")?; grants.add(SLOT_RAMDISK, minted.badged(keeper, CLIENT, BLOCK_BADGE_WRITE)?, CLIENT); }
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
                let keeper = self.keeper("vfs_server")?;
                grants.add(SLOT_VFS, minted.badged(keeper, CLIENT, VFS_BADGE_USER)?, CLIENT);
                grants.add(SLOT_CONTROL, minted.privilege(CAP_KIND_CONTROL)?, 0); grants.add(SLOT_INPUT, minted.privilege(CAP_KIND_INPUT)?, 0);
                grants.add(SLOT_SERIAL, minted.ports(0x3F8, 8)?, 0);
                self.lend(&mut grants, SLOT_SYSINFO, "sysmon")?;
            }
            _ => return Err(Error::NotFound),
        }
        // Every service writes to the system log; the shell's client may also read it (and lends it to dmesg).
        if name == "shell" { let keeper = self.keeper("logd")?; grants.add(SLOT_LOG, minted.badged(keeper, CLIENT, LOG_BADGE_READ)?, CLIENT); }
        else if name != "logd" { self.lend(&mut grants, SLOT_LOG, "logd")?; }
        // Quotas are init's policy: loader may run MAX_APPS applications with APP_ENDPOINTS endpoints each.
        let quota = if name == "loader" { Quota { tasks: MAX_APPS as u16, endpoints: (MAX_APPS * APP_ENDPOINTS) as u16 } } else { Quota::default() };
        let pid = mind::process::spawn_raw(name.as_bytes(), Image::Boot(index), &grants.list[..grants.count], flags, quota)?;
        self.pids[index] = pid;
        self.starts[index] += 1;
        mind::println!("[INIT] STARTED {} PID={}", name, pid);
        Ok(pid)
    }
}

// The lifecycle interface (idl/lifecycle.wit): init is the lifecycle owner (roadmap C6). Stopping uses the process
// control privilege init mints for itself; init and the shell are never stopped.
impl Init {
    fn index(name: &str) -> core::result::Result<usize, lifecycle::Error> {
        BOOT_SERVICES.iter().position(|s| s.eq_ignore_ascii_case(name)).ok_or(lifecycle::Error::NotFound)
    }
    fn start_service(&mut self, index: usize) -> core::result::Result<u64, lifecycle::Error> {
        if index == 0 || self.running(index) { return Err(lifecycle::Error::Running); }
        match self.start(index) {
            Ok(pid) => Ok(pid),
            Err(Error::NotFound) => Err(lifecycle::Error::NoDevice),
            Err(error) => { mind::println!("[INIT] {} FAILED: {:?}", BOOT_SERVICES[index], error); Err(lifecycle::Error::Failed) }
        }
    }
    fn stop_service(&mut self, index: usize) -> core::result::Result<(), lifecycle::Error> {
        if index == 0 || BOOT_SERVICES[index] == "shell" { return Err(lifecycle::Error::Denied); }
        if !self.running(index) { return Err(lifecycle::Error::Stopped); }
        let pid = self.pids[index];
        mind::control::kill(pid).map_err(|_| lifecycle::Error::Failed)?;
        for _ in 0..200 { if !mind::process::alive(pid) { break; } mind::time::sleep(10); }
        mind::println!("[INIT] STOPPED {} PID={}", BOOT_SERVICES[index], pid);
        Ok(())
    }
    fn serve(&mut self, request: lifecycle::Request, bytes: &mut [u8]) -> mind::Result<()> {
        use lifecycle::Request;
        let name = |bytes: &[u8], payload| lifecycle::args_start(bytes, payload).map(|n| { let mut owned = [0u8; NAME_MAX]; let len = n.len().min(NAME_MAX); owned[..len].copy_from_slice(&n.as_bytes()[..len]); (owned, len) });
        match request {
            Request::List { .. } => {
                let entries: [lifecycle::Service; BOOT_IMAGES] = core::array::from_fn(|i| lifecycle::Service {
                    name: BOOT_SERVICES[i], pid: if i == 0 { INIT_PID } else { self.pids[i] }, starts: if i == 0 { 1 } else { self.starts[i] },
                    running: i == 0 || self.running(i), holds: HOLDS[i] });
                lifecycle::reply_list(bytes, Ok(&entries[..]))
            }
            Request::Start { payload, .. } | Request::Stop { payload, .. } | Request::Restart { payload, .. } => {
                let (owned, len) = match name(bytes, payload) { Ok(n) => n, Err(reason) => return wire::reject(reason) };
                let text = core::str::from_utf8(&owned[..len]).unwrap_or("");
                let index = Self::index(text);
                match request {
                    Request::Start { .. } => lifecycle::reply_start(index.and_then(|i| self.start_service(i))),
                    Request::Stop { .. } => lifecycle::reply_stop(index.and_then(|i| self.stop_service(i))),
                    _ => lifecycle::reply_restart(index.and_then(|i| { if i == 0 || BOOT_SERVICES[i] == "shell" { return Err(lifecycle::Error::Denied); } if self.running(i) { self.stop_service(i)?; } self.start_service(i) })),
                }
            }
            Request::StopTask { pid } => {
                let service = pid == INIT_PID || (1..BOOT_IMAGES).any(|i| self.pids[i] == pid && self.running(i));
                let result = if service { Err(lifecycle::Error::Denied) } else if !mind::process::alive(pid) { Err(lifecycle::Error::NotFound) } else {
                    mind::control::kill(pid).map_err(|_| lifecycle::Error::NotFound)
                };
                lifecycle::reply_stop_task(result)
            }
        }
    }
}

fn service_index(name: &str) -> usize { BOOT_SERVICES.iter().position(|s| *s == name).unwrap_or(0) }

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut init = Init { pids: [0; BOOT_IMAGES], dma: [None; BOOT_IMAGES], keepers: [None; BOOT_IMAGES], starts: [0; BOOT_IMAGES] };
    // Boot order is the BOOT_SERVICES order: logd first, drivers before vfs_server, loader before the shell.
    for index in 1..BOOT_IMAGES {
        match init.start(index) {
            // From now on init's own lines (and those printed so far) go to the system log too.
            Ok(_) if BOOT_SERVICES[index] == "logd" => {
                // The keeper may send: init needs no client of its own.
                if let Ok(keeper) = init.keeper("logd") { mind::log::use_endpoint(Endpoint(keeper)); }
            }
            Ok(_) => {}
            Err(Error::NotFound) => mind::println!("[INIT] {} NOT STARTED: NO DEVICE", BOOT_SERVICES[index]),
            Err(error) => mind::println!("[INIT] {} FAILED: {:?}", BOOT_SERVICES[index], error),
        }
    }
    mind::println!("[INIT] READY");
    // Process control, to stop services and applications on request (init is their lifecycle owner).
    if platform::cap(PLATFORM_PRIVILEGE, CAP_KIND_CONTROL, 0).is_err() { mind::println!("[INIT] NO PROCESS CONTROL: STOP REQUESTS WILL FAIL"); }
    // Lifecycle requests (idl/lifecycle.wit) from the shell and the programs it lends init's endpoint to.
    loop {
        let Ok(request) = Endpoint::SERVICE.recv(RECEIVED) else { continue };
        let decoded = lifecycle::decode(&request, RECEIVED);
        let mut mapping = if request.cap_received { Mapping::new(RECEIVED).ok() } else { None };
        let mut empty = [0u8; 0];
        let bytes: &mut [u8] = match mapping.as_mut() { Some(m) => m.as_mut_slice(), None => &mut empty };
        let _ = match decoded {
            Ok(call) => init.serve(call, bytes),
            Err(reason) => wire::reject(reason),
        };
        drop(mapping);
        if request.cap_received { let _ = ipc::drop_cap(RECEIVED); }
    }
}
