#![no_std]
#![no_main]
// init: holds the bootstrap authority (platform privilege) and is the only place with service policy:
// which boot services start, in which order, and exactly which capabilities each one receives. After boot it keeps
// each service's capabilities for restarts and gives up the platform privilege (MC-3.12). As the lifecycle owner it
// restarts failed services within a budget and serves idl/init.wit: start, list, stop and restart services, stop an
// application.
mod legacy;

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
    "ports 0x70-0x71", "ports 0x60, 0x64; IRQ 1 and 12; input", "VirtIO input BARs and MSI-X vectors (or IRQs), up to two devices; 24 KiB DMA; input", "framebuffer; display", "ports 0x1F0-0x1F7, 0x3F6", "AHCI registers; 128 KiB DMA",
    "xHCI registers; 512 KiB DMA", "a USB client for mass storage interfaces", "a USB client for HID interfaces; input", "VirtIO block BAR; 128 KiB DMA", "NVMe registers; 128 KiB DMA", "8 MiB of memory", "write clients of the block devices", "pin controller registers; a VFS client", "spawn privilege", "AC97 ports and IRQ; DMA",
    "an audio client", "a VFS client (video/synthetic) and a display client (the camera mark)", "network card BAR and MSI-X vector (or ports and IRQ); 160 KiB DMA", "a client of the network card driver", "network stack clients: minting source and policy control; a VFS client", "an RTC client; the device key in memory",
    "the key service's signer client; RTC and VFS clients", "its own program client", "observe privilege", "screen; process control; input; the serial line"];
const CLIENT: u8 = CAP_WRITE | CAP_GRANT;
// DMA buffer sizes of the drivers; the regions are minted once and survive driver restarts.
const SERVICE_QUOTA: u16 = 256; // tasks and endpoints init keeps for the services and their restarts (issue 171)
const AHCI_DMA_BYTES: usize = 128 * 1024; // commands, FIS and a 64 KiB data buffer
const NVME_DMA_BYTES: usize = 128 * 1024; // queues, identify page, PRP list and a 64 KiB data buffer
const VIRTIO_BLK_DMA_BYTES: usize = 128 * 1024; // the virtqueue, request headers and a 64 KiB data buffer
const XHCI_DMA_BYTES: usize = 512 * 1024; // rings, contexts, scratchpad, a 64 KiB data buffer and a pool of pages (usb_host)
const AUDIO_DMA_BYTES: usize = (33 + 17) * 4096; // playback: 32 buffers + list; capture: 16 buffers + list
const NET_DMA_BYTES: usize = 160 * 1024; // two virtqueues (64 KiB) and 48 frame buffers of 2 KiB
const INPUT_DMA_BYTES: usize = 24 * 1024; // per device 12 KiB: the event queue (two pages), then up to 64 events of 8 bytes
const SLOT_INPUT_IRQ1: usize = 7; // virtio_input: the second device's interrupt
const WINDOWS_MEMORY_MIB: u16 = 128; // the window broker's surfaces (issue 163)
const RECOVERY_RESERVE_MIB: usize = 32; // frame pool kept for services and their restarts (issue 169)

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
// How a service was started: its grants (over capabilities init keeps), spawn flags and quota.
#[derive(Clone, Copy)]
struct Plan { grants: Grants, flags: usize, quota: Quota }

// Per boot service: PID, DMA region and the keeper of its endpoint (can mint receive rights, cannot receive itself);
// how often it was started, whether it was stopped on request (then it is not restarted) and whether its device was
// missing at boot.
struct Init { plans: [Option<Plan>; UNITS], pids: [u64; UNITS], dma: [Option<usize>; UNITS], devices: [Option<usize>; UNITS], keepers: [Option<usize>; UNITS], restarts: [[u64; RESTART_BUDGET]; UNITS], quarantined: [bool; UNITS], starts: [u32; UNITS], stopped: [bool; UNITS], missing: [bool; UNITS] }

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
    // The keyboard service the shell's keymap talks to: the PS/2 driver if the machine has the controller, else the USB
    // HID driver, else the VirtIO one.
    fn keyboard(&self) -> &'static str { ["ps2_kbd", "usb_hid", "virtio_input"].into_iter().find(|&n| self.running(service_index(n))).unwrap_or("virtio_input") }

    // Before a driver is restarted (MC-6.3): its device stops DMA, then the DMA region is cleared, so the new instance
    // starts from a quiet device and no residue of the old one.
    fn quiesce(&mut self, index: usize) {
        if let Some(device) = self.devices[index] { match platform::quiesce(device) { Ok(()) => mind::println!("[INIT] {} DEVICE QUIESCED", unit_name(index)), Err(error) => mind::println!("[INIT] {} QUIESCE FAILED: {:?}", unit_name(index), error) } }
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
        let name = unit_name(index);
        let (image, instance) = unit_image(index);
        if index == 0 || self.running(index) { return Err(Error::Other(ERR_BUSY)); }
        if let Some(plan) = self.plans[index] {
            // A restart: the device was stopped when its driver ended; the same capabilities are granted again.
            if let Some(device) = self.devices[index] { let _ = platform::resume(device); }
            return self.spawn(index, plan);
        }
        let mut minted = Minted::new();
        let mut grants = Grants::new();
        let mut flags = SPAWN_SERVICE;
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
            }
            // USB class drivers: a client of usb_host whose badge names the one class it may claim.
            "usb_storage" | "usb_hid" => {
                if !self.running(service_index("usb_host")) { return Err(Error::NotFound); }
                let badge = if name == "usb_hid" { mind::usb::BADGE_HID } else { mind::usb::BADGE_STORAGE };
                grants.add(SLOT_DEV0, self.badged(&mut minted, "usb_host", badge)?, CLIENT);
                grants.add(SLOT_SERVICE, self.server(&mut minted, name)?, ALL);
                if name == "usb_hid" { grants.add(SLOT_PRIV, minted.privilege(CAP_KIND_INPUT)?, 0); }
            }
            "virtio_blk" => {
                // The first VirtIO block device (vendor 1AF4, modern-only 1042 or transitional 1001): its BAR with the
                // modern structures and a DMA region; requests are polled, so no interrupt line.
                let device = [0x1042_1AF4, 0x1001_1AF4].iter().find_map(|&id| platform::find_device_id(0, 0, id, 0).ok()).ok_or(Error::NotFound)?;
                self.devices[index] = Some(device);
                let probe = (0..6).find_map(|bar| platform::cap(PLATFORM_DEVICE_BAR, device, bar).ok());
                let bar = probe.and_then(|slot| { let layout = mind::virtio::Layout::read(slot); let _ = ipc::drop_cap(slot); layout }).and_then(|l| l.single_bar()).ok_or(Error::NotFound)?;
                grants.add(SLOT_DEV0, Self::bar(&mut minted, device, bar as usize, CAP_KIND_MMIO)?, 0);
                grants.add(SLOT_SERVICE, self.server(&mut minted, name)?, ALL); grants.copy(SLOT_MEM, self.dma(index, VIRTIO_BLK_DMA_BYTES)?, 0);
            }
            "nvme" => {
                // The first NVMe controller (class 01:08:02): registers in BAR0; commands are polled, so no interrupt.
                let device = platform::find_device(0x01_08_02, 0xFF_FF_FF, 0)?; self.devices[index] = Some(device);
                grants.add(SLOT_DEV0, Self::bar(&mut minted, device, 0, CAP_KIND_MMIO)?, 0);
                grants.add(SLOT_SERVICE, self.server(&mut minted, name)?, ALL); grants.copy(SLOT_MEM, self.dma(index, NVME_DMA_BYTES)?, 0);
            }
            "ramdisk" => grants.add(SLOT_SERVICE, self.server(&mut minted, "ramdisk")?, ALL),
            "vfs_server" => {
                // VFS sees only block devices whose drivers are actually running; it alone may write to them (B.6).
                grants.add(SLOT_SERVICE, self.server(&mut minted, "vfs_server")?, ALL);
                let mut slot = SLOT_BLOCK_FIRST;
                for driver in ["ata", "ahci", "usb_storage", "virtio_blk", "nvme"] {
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
                self.lend(&mut grants, SLOT_VFS, "vfs_server")?; // hwdocs/ on the boot disk
            }
            "loader" => {
                grants.add(SLOT_SERVICE, self.server(&mut minted, "loader")?, ALL);
                self.lend(&mut grants, 2, "rtc")?; self.lend(&mut grants, 3, "vfs_server")?;
                self.lend(&mut grants, 4, "audio_gw")?; grants.add(5, minted.privilege(CAP_KIND_SPAWN)?, 0);
                self.lend(&mut grants, 6, "tts")?;
            }
            "audio_gw" => {
                grants.add(SLOT_SERVICE, self.server(&mut minted, "audio_gw")?, ALL);
                // LEGACY: AC97 (class 04:01): mixer and bus master port ranges and an IRQ line; without it the gateway reports no device.
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
            // The video gateway (issue 158): the boot disk for video/synthetic, the compositor for the camera mark.
            "video_gw" => {
                grants.add(SLOT_SERVICE, self.server(&mut minted, name)?, ALL);
                self.lend(&mut grants, SLOT_VFS, "vfs_server")?; self.lend(&mut grants, SLOT_DEV0, "compositor")?;
            }
            // The stack holds only a client of the card driver (B.6): frames, no device.
            "netstack" => {
                grants.add(SLOT_SERVICE, self.server(&mut minted, "netstack")?, ALL);
                self.lend(&mut grants, SLOT_DEV0, "virtio_net")?; self.lend(&mut grants, SLOT_DEV1, "virtio_net#1")?; // one client per card
            }
            // The broker mints flow grants from an unbadged stack client (which itself may open nothing), registers them
            // through the stack's policy client and reads netpolicy.txt through a VFS client.
            "netpolicy" => {
                grants.add(SLOT_SERVICE, self.server(&mut minted, "netpolicy")?, ALL);
                self.lend(&mut grants, 2, "netstack")?; self.lend(&mut grants, 3, "vfs_server")?;
                grants.add(4, self.badged(&mut minted, "netstack", mind::network::BADGE_POLICY)?, CLIENT);
            }
            // The window broker holds nothing but its own program client, which it lends to window managers (issue 157).
            "windows" => { grants.add(SLOT_SERVICE, self.server(&mut minted, "windows")?, ALL); self.lend(&mut grants, 2, "windows")?; }
            // The key service makes the device key itself (RDRAND) and needs only the date for its certificate.
            "keystore" => { grants.add(SLOT_SERVICE, self.server(&mut minted, "keystore")?, ALL); self.lend(&mut grants, 2, "rtc")?; }
            // The TLS service gets no network access: clients lend their flows. It alone may ask the key service to sign.
            "tls" => {
                grants.add(SLOT_SERVICE, self.server(&mut minted, "tls")?, ALL);
                self.lend(&mut grants, 2, "rtc")?; self.lend(&mut grants, 3, "vfs_server")?;
                grants.add(4, self.badged(&mut minted, "keystore", mind::network::BADGE_KEY_SIGNER)?, CLIENT);
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
                grants.copy(SLOT_INIT, SLOT_SERVICE, CLIENT);
                for (slot, service) in [(SLOT_RTC, "rtc"), (SLOT_AUDIO, "audio_gw"), (SLOT_LOADER, "loader"), (SLOT_TTS, "tts")] { self.lend(&mut grants, slot, service)?; }
                // The user's file client: writes on ram: and in the boot disk's data directory (applications read only).
                grants.add(SLOT_VFS, self.badged(&mut minted, "vfs_server", mind::fs::BADGE_USER)?, CLIENT);
                grants.add(SLOT_CONTROL, minted.privilege(CAP_KIND_CONTROL)?, 0); grants.add(SLOT_INPUT, minted.privilege(CAP_KIND_INPUT)?, 0);
                grants.add(SLOT_SERIAL, minted.serial()?, 0);
                self.lend(&mut grants, SLOT_SYSINFO, "sysmon")?;
                grants.add(SLOT_AUTHORITY, self.badged(&mut minted, "sysmon", mind::stat::BADGE_AUTHORITY)?, CLIENT);
                let keyboard = self.keyboard(); self.lend(&mut grants, SLOT_KEYBOARD, keyboard)?; self.lend(&mut grants, SLOT_DISPLAY, "compositor")?;
                self.lend(&mut grants, SLOT_NET, "virtio_net")?; // diagnostics; ERR_PEER without a network card
                grants.add(SLOT_SOCKET, self.badged(&mut minted, "netstack", mind::network::BADGE_OPERATOR)?, CLIENT); // every destination
                self.lend(&mut grants, SLOT_NETPOLICY, "netpolicy")?;
                self.lend(&mut grants, SLOT_TLS, "tls")?;
                self.lend(&mut grants, SLOT_WINDOWS, "windows")?;
                grants.add(SLOT_WINDOW_MANAGER, self.badged(&mut minted, "windows", mind::window::BADGE_MANAGER)?, CLIENT);
                if self.running(service_index("gpio")) { grants.add(SLOT_GPIO, self.badged(&mut minted, "gpio", mind::gpio::BADGE_CONTROL)?, CLIENT); }
                if self.running(service_index("video_gw")) { self.lend(&mut grants, SLOT_CAMERA, "video_gw")?; } // lent on with the user's consent
            }
            _ => return Err(Error::NotFound),
        }
        // Every service writes to the system log; the shell's client may also read it (and lends it to dmesg).
        if name == "shell" { grants.add(SLOT_LOG, self.badged(&mut minted, "logd", mind::log::BADGE_READ)?, CLIENT); }
        else if name != "logd" { self.lend(&mut grants, SLOT_LOG, "logd")?; }
        // Quotas are init's policy: loader gets all of init's root quota but what the services need (issue 171: no
        // fixed count of applications; memory limits them); the shell serves voice control on one endpoint (issue 079).
        let quota = match name {
            // The applications' memory is charged to loader too: it may use all of init's (issue 150).
            "loader" => Quota { tasks: QUOTA_MAX as u16 - SERVICE_QUOTA, endpoints: QUOTA_MAX as u16 - SERVICE_QUOTA, memory_mib: SPAWN_MEMORY_ALL as u16 },
            "shell" => Quota { tasks: 0, endpoints: 1, memory_mib: 0 },
            // The windows' memory is the broker's: a pixel window has room for the screen (up to 1920 × 1200, 9 MiB)
            // so that its content follows its frame (issue 163).
            "windows" => Quota { memory_mib: WINDOWS_MEMORY_MIB, ..Quota::default() },
            _ => Quota::default(),
        };
        let plan = Plan { grants, flags, quota };
        let pid = self.spawn(index, plan)?;
        minted.count = 0; // kept for restarts
        self.plans[index] = Some(plan);
        Ok(pid)
    }

    fn spawn(&mut self, index: usize, plan: Plan) -> Result<u64> {
        let name = unit_name(index);
        let mut grants = plan.grants; let mut receiver = None;
        if let Some(keeper) = self.keepers[index] {
            for g in grants.list[..grants.count].iter_mut().filter(|g| g.own as usize == keeper && g.child as usize == SLOT_SERVICE) {
                let slot = ipc::mint(keeper, ALL, 0, 0)?; *g = grant_moved(SLOT_SERVICE, slot, ALL); receiver = Some(slot);
            }
        }
        let pid = mind::process::spawn_raw(name.as_bytes(), Image::Boot(unit_image(index).0), &grants.list[..grants.count], plan.flags, plan.quota)
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
        let Some(index) = (1..UNITS).find(|&i| self.pids[i] == exit.pid) else { return };
        let name = unit_name(index);
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
    fn index(name: &str) -> Option<usize> { (0..UNITS).position(|u| unit_name(u).eq_ignore_ascii_case(name)) }

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
            Err(error) => { mind::println!("[INIT] {} FAILED: {:?}", unit_name(index), error); Err(idl_init::Error::Failed) }
        }
    }

    fn stop_service(&mut self, index: usize) -> core::result::Result<(), idl_init::Error> {
        if index == 0 || unit_name(index) == "shell" { return Err(idl_init::Error::Denied); }
        if !self.running(index) { return Err(idl_init::Error::Stopped); }
        let pid = self.pids[index];
        self.stopped[index] = true;
        if mind::control::kill(pid).is_err() { self.stopped[index] = false; return Err(idl_init::Error::Failed); }
        for _ in 0..200 { if !mind::process::alive(pid) { break; } mind::time::sleep(10); }
        // Its device stops DMA until the next start (MC-6.3).
        self.quiesce(index);
        mind::println!("[INIT] STOPPED {} PID={}", unit_name(index), pid);
        Ok(())
    }

    fn list(&self) -> [idl_init::Service; UNITS] {
        core::array::from_fn(|i| idl_init::Service {
            name: wire_text(unit_name(i)), pid: if i == 0 { INIT_PID } else if self.running(i) { self.pids[i] } else { 0 },
            starts: if i == 0 { 1 } else { self.starts[i] }, running: i == 0 || self.running(i), quarantined: self.quarantined[i], holds: wire_text(HOLDS[unit_image(i).0]),
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
                    if i == 0 || unit_name(i) == "shell" { return Err(E::Denied); }
                    if self.running(i) { self.stop_service(i)?; }
                    self.start_service(i)
                });
                idl_init::reply_restart(call, result)
            }
            Request::StopTask { pid } => {
                let service = pid == INIT_PID || (1..UNITS).any(|i| self.pids[i] == pid && self.running(i));
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

fn service_index(name: &str) -> usize { (0..UNITS).find(|&u| unit_name(u) == name).unwrap_or(0) }

mind::entry!(main);
fn main(info: &'static BootInfo) {
    let mut init = Init { plans: [None; UNITS], pids: [0; UNITS], dma: [None; UNITS], devices: [None; UNITS], keepers: [None; UNITS], restarts: [[0; RESTART_BUDGET]; UNITS], quarantined: [false; UNITS], starts: [0; UNITS], stopped: [false; UNITS], missing: [false; UNITS] };
    // Boot order is the BOOT_SERVICES order (logd first, drivers before vfs_server, loader before the shell); further
    // instances of an image follow its first one.
    let order = (1..BOOT_IMAGES).flat_map(|image| core::iter::once(image).chain((BOOT_IMAGES..UNITS).filter(move |&u| unit_image(u).0 == image)));
    for index in order {
        // An image the bootloader did not find (a service of the other architecture) is not started.
        if info.programs[unit_image(index).0].len == 0 { init.missing[index] = true; mind::println!("[INIT] {} NOT STARTED: NO IMAGE", unit_name(index)); continue; }
        match init.start(index) {
            // From now on init's own lines (and those printed so far) go to the system log too.
            Ok(_) if unit_name(index) == "logd" => {
                // The keeper may send: init needs no client of its own.
                if let Ok(keeper) = init.keeper("logd") { mind::log::use_endpoint(Endpoint(keeper)); }
            }
            Ok(_) => {}
            Err(Error::NotFound) => { init.missing[index] = true; mind::println!("[INIT] {} NOT STARTED: NO DEVICE", unit_name(index)); }
            Err(error) => mind::println!("[INIT] {} FAILED: {:?}", unit_name(index), error),
        }
    }
    if cfg!(target_arch = "x86_64") { legacy::report(); } // the x86 legacy hardware (docs/legacy.md)
    // Applications may not take the frames a service restart needs (MC-6.5): a reserve only the system band uses.
    match platform::reserve_memory(RECOVERY_RESERVE_MIB << 20) {
        Ok(()) => mind::println!("[INIT] RECOVERY RESERVE {} MiB", RECOVERY_RESERVE_MIB),
        Err(_) => mind::println!("[INIT] NO RECOVERY RESERVE: APPLICATIONS MAY TAKE ALL TASK MEMORY"),
    }
    // No process control: init ends services and applications as their ancestor, the kernel's lifecycle rule (issue 170).
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
