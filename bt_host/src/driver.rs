//! bt_hci's driver logic apart from its I/O (700-BLT-0005), so that the whole start and loop run on the host against a
//! scripted controller before hardware sees them: the bring-up (reset, version, a Realtek patch, the address), then
//! one of two paths.
//! - **Quiet** (no device listed, no listening asked): the two commands main's bt_hci sent last (the event mask and the
//!   inquiry mode), then nothing more: no polling of usb_host at all, as main's bt_hci did for weeks on the DEXP and
//!   the Ryzen.
//! - **Active**: the host stack. The event endpoint is polled every 20 ms (2 ms while a link is up), the bulk IN endpoint
//!   only while a link is up; every command is answered or given up after its time, none is retried.
use crate::acl;
use crate::buf::{Addr, LINE};
use crate::hci;
use crate::rtl;
use crate::stack::{Stack, Up, UP_MAX};

/// A command's answer, at most this long in the bring-up; the polling of it meanwhile.
pub const COMMAND_MS: u64 = 2_000;
pub const COMMAND_POLL_MS: u64 = 10;
/// How often the loop runs while a link is up, and while none is.
pub const BUSY_MS: u64 = 2;
pub const IDLE_MS: u64 = 20;
/// Failed polls of the event endpoint in a row after which the controller is taken as gone.
pub const FAILED_POLLS: u32 = 50;
/// main's bt_hci's event mask: the default (bits 0-44) with Extended Inquiry Result (46) and the LE meta event (61).
pub const QUIET_EVENT_MASK: u64 = 0x0000_1FFF_FFFF_FFFF | 1 << 46 | 1 << 61;
/// The scratch the bring-up needs: the patch file, the configuration, the image made of them.
pub const PATCH_MAX: usize = 128 * 1024;
pub const IMAGE_MAX: usize = 128 * 1024;
pub const SCRATCH: usize = 2 * PATCH_MAX + 1024 + IMAGE_MAX;

/// Why a port call failed: the device or usb_host is gone (stop), or this call failed (it may pass next time).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortError { Gone, Failed }

/// The firmware files a Realtek controller's patch is made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Firmware { Rtl8821cPatch, Rtl8821cConfig, Rtl8852bPatch }

/// What the driver needs of the machine: the time, a wait, the controller's endpoints, the log, the firmware files
/// (each checked by its pins by the port).
pub trait Port {
    fn now(&mut self) -> u64;
    fn sleep(&mut self, ms: u64);
    /// An HCI command packet to the controller: a class request on endpoint 0.
    fn command(&mut self, packet: &[u8]) -> Result<(), PortError>;
    /// The interrupt IN endpoint's packets since the last call; the first call starts its polling.
    fn events(&mut self, each: &mut dyn FnMut(&[u8])) -> Result<(), PortError>;
    /// The bulk IN endpoint's packets since the last call; the first call starts its polling.
    fn acl_in(&mut self, each: &mut dyn FnMut(&[u8])) -> Result<(), PortError>;
    fn acl_out(&mut self, packet: &[u8]) -> Result<(), PortError>;
    fn say(&mut self, line: core::fmt::Arguments);
    /// A firmware file into `out`, None when it is missing or not the one pinned; its length.
    fn firmware(&mut self, which: Firmware, out: &mut [u8]) -> Option<usize>;
}

/// Why the bring-up stopped. The fields are read through Debug, in the log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fail { Port(PortError), Timeout(u16), Status(u16, u8), File(Firmware), Patch(rtl::Error) }

/// The bring-up's synchronous commands: the events joined, the last answer's return parameters.
pub struct Bring { events: hci::Events, ret: [u8; hci::EVENT_MAX] }

impl Default for Bring { fn default() -> Self { Self::new() } }

impl Bring {
    pub const fn new() -> Self { Self { events: hci::Events::new(), ret: [0; hci::EVENT_MAX] } }

    /// Sends a command and waits for its Command Complete (its return parameters) or, for one answered by a Command
    /// Status, that status; other events meanwhile are dropped, as main's bt_hci dropped them.
    pub fn command(&mut self, port: &mut dyn Port, opcode: u16, params: &[u8]) -> Result<&[u8], Fail> {
        let mut packet = [0u8; hci::COMMAND_MAX];
        let n = hci::command(opcode, params, &mut packet);
        port.command(&packet[..n]).map_err(Fail::Port)?;
        let deadline = port.now() + COMMAND_MS;
        let mut done: Option<Result<usize, Fail>> = None;
        let mut ret = [0u8; hci::EVENT_MAX];
        while done.is_none() {
            let events = &mut self.events;
            port.events(&mut |packet| events.feed(packet, |code, p| match code {
                hci::EV_COMMAND_COMPLETE => if let Some((op, r)) = hci::complete(p) { if op == opcode && done.is_none() { ret[..r.len()].copy_from_slice(r); done = Some(Ok(r.len())); } },
                hci::EV_COMMAND_STATUS => if let Some((status, op)) = hci::status(p) { if op == opcode && done.is_none() { done = Some(if status == 0 { Ok(0) } else { Err(Fail::Status(opcode, status)) }); } },
                _ => {}
            })).map_err(Fail::Port)?;
            if done.is_none() {
                if port.now() > deadline { return Err(Fail::Timeout(opcode)); }
                port.sleep(COMMAND_POLL_MS);
            }
        }
        let len = done.unwrap()?;
        self.ret[..len].copy_from_slice(&ret[..len]);
        // A Command Complete's first return byte is its status.
        if len > 0 && self.ret[0] != 0 { return Err(Fail::Status(opcode, self.ret[0])); }
        Ok(&self.ret[..len])
    }

    pub fn version(&mut self, port: &mut dyn Port) -> Result<hci::Version, Fail> {
        let ret = self.command(port, hci::READ_LOCAL_VERSION, &[])?;
        hci::Version::parse(ret).ok_or(Fail::Status(hci::READ_LOCAL_VERSION, 0xFF))
    }

    /// A Realtek controller's patch: its ROM version, the image from its files, the fragments downloaded, a reset.
    fn patch(&mut self, port: &mut dyn Port, chip: &rtl::Chip, scratch: &mut [u8]) -> Result<(), Fail> {
        let rom = *self.command(port, rtl::READ_ROM_VERSION, &[])?.get(1).ok_or(Fail::Status(rtl::READ_ROM_VERSION, 0xFF))?;
        let (files, image) = scratch.split_at_mut(2 * PATCH_MAX + 1024);
        let (fw_area, config_area) = files.split_at_mut(PATCH_MAX);
        let len = if *chip == rtl::RTL8852B {
            let n = port.firmware(Firmware::Rtl8852bPatch, fw_area).ok_or(Fail::File(Firmware::Rtl8852bPatch))?;
            let fw = &fw_area[..n];
            let e = rtl::parse_core(fw).map_err(Fail::Patch)?;
            let len = rtl::image_core(fw, chip, rom, image).map_err(Fail::Patch)?;
            port.say(format_args!("ROM VERSION {}; PATCH {:016X} FOR PROJECT {} ({} OF {} SECTIONS): {} BYTES IN {} FRAGMENTS", rom, e.version, e.project, e.found, e.counted, len, rtl::fragments(len)));
            len
        } else {
            let n = port.firmware(Firmware::Rtl8821cPatch, fw_area).ok_or(Fail::File(Firmware::Rtl8821cPatch))?;
            let c = port.firmware(Firmware::Rtl8821cConfig, config_area).ok_or(Fail::File(Firmware::Rtl8821cConfig))?;
            let e = rtl::parse(&fw_area[..n]).map_err(Fail::Patch)?;
            let len = rtl::image(&fw_area[..n], &config_area[..c], chip, rom, image).map_err(Fail::Patch)?;
            port.say(format_args!("ROM VERSION {}; PATCH {:08X} FOR PROJECT {}: {} BYTES IN {} FRAGMENTS", rom, e.version, e.project, len, rtl::fragments(len)));
            len
        };
        let started = port.now();
        let mut params = [0u8; 1 + rtl::FRAGMENT];
        for i in 0..rtl::fragments(len) {
            let (index, range) = rtl::fragment(len, i);
            let n = range.len();
            params[0] = index;
            params[1..1 + n].copy_from_slice(&image[range]);
            self.command(port, rtl::DOWNLOAD, &params[..1 + n])?;
        }
        let took = port.now() - started;
        port.say(format_args!("PATCH DOWNLOADED IN {} MS", took));
        self.command(port, hci::RESET, &[])?;
        Ok(())
    }

    /// The controller brought up as main's bt_hci did: the first poll, a reset, its version, the patch for an
    /// unpatched RTL8821C or RTL8852B, its address (as HCI carries it, least significant byte first).
    pub fn up(&mut self, port: &mut dyn Port, scratch: &mut [u8]) -> Result<Addr, Fail> {
        // The first call starts the event endpoint's polling.
        port.events(&mut |_| {}).map_err(Fail::Port)?;
        self.command(port, hci::RESET, &[])?;
        let v = self.version(port)?;
        port.say(format_args!("VERSION: HCI {} REVISION {:04X}, LMP {} SUBVERSION {:04X}, MANUFACTURER {:04X}", v.hci, v.hci_rev, v.lmp, v.lmp_subver, v.manufacturer));
        let unpatched = [rtl::RTL8821C, rtl::RTL8852B].into_iter().find(|c| (v.manufacturer, v.lmp_subver, v.hci_rev) == (rtl::MANUFACTURER, c.lmp_subver, c.hci_rev));
        if let Some(c) = unpatched {
            self.patch(port, &c, scratch)?;
            let v = self.version(port)?;
            port.say(format_args!("{} PATCHED: LMP SUBVERSION {:04X}, HCI REVISION {:04X}", c.name, v.lmp_subver, v.hci_rev));
        } else {
            port.say(format_args!("NOT AN UNPATCHED RTL8821C OR RTL8852B: NO PATCH"));
        }
        let a = hci::bd_addr(self.command(port, hci::READ_BD_ADDR, &[])?).ok_or(Fail::Status(hci::READ_BD_ADDR, 0xFF))?;
        port.say(format_args!("ADDRESS {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}", a[0], a[1], a[2], a[3], a[4], a[5]));
        Ok(Addr(core::array::from_fn(|i| a[5 - i])))
    }

    /// The quiet path's two commands, main's bt_hci's last: its event mask, inquiry results with RSSI or extended data.
    pub fn quiet(&mut self, port: &mut dyn Port) -> Result<(), Fail> {
        self.command(port, hci::SET_EVENT_MASK, &QUIET_EVENT_MASK.to_le_bytes())?;
        self.command(port, hci::WRITE_INQUIRY_MODE, &[2])?;
        Ok(())
    }
}

/// After the bring-up: with no device listed and no listening asked, main's bt_hci's last two commands and None (the
/// caller idles, polling nothing); else the stack started and its driver, the bulk IN endpoint's packet size given.
pub fn start(port: &mut dyn Port, bring: &mut Bring, mut stack: Stack, listen: bool, packet: usize) -> Result<Option<Driver>, Fail> {
    if stack.devices.is_empty() && !listen {
        bring.quiet(port)?;
        port.say(format_args!("UP: NOT LISTENING (NO data/bt-listen.txt OR data/inbox/bt-listen.txt), NO DEVICE LISTED: A SCAN TAKES THE RADIO A WI-FI MAY SHARE"));
        return Ok(None);
    }
    stack.scan.on = listen;
    port.say(format_args!("UP: {}", if listen { "LISTENING EVERY 300 S" } else { "NOT LISTENING: THE DEVICES LISTED ONLY" }));
    stack.start(port.now());
    Ok(Some(Driver::new(stack, packet)))
}

/// The loop's step: wait this long and step again, or stop for good.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next { Sleep(u64), Idle }

/// The active path: the stack, the ACL stream, the polls' failures in a row.
pub struct Driver { pub stack: Stack, events: hci::Events, stream: acl::Stream, failures: u32, out: [u8; acl::PACKET_MAX], pub keys_changed: bool }

impl Driver {
    /// The stack started on the controller brought up; `packet` is the bulk IN endpoint's packet size.
    pub fn new(stack: Stack, packet: usize) -> Self {
        Self { stack, events: hci::Events::new(), stream: acl::Stream::new(packet.clamp(1, 64)), failures: 0, out: [0; acl::PACKET_MAX], keys_changed: false }
    }

    /// One turn of the loop: events in, ACL in while a link is up, the time, commands and ACL out, the log; `up` gets the
    /// HID upcalls, and `keys_changed` says that the keys are to be written. Idle once the controller is gone.
    pub fn step(&mut self, port: &mut dyn Port, up: &mut dyn FnMut(Up)) -> Next {
        let now = port.now();
        let (stack, events) = (&mut self.stack, &mut self.events);
        let polled = port.events(&mut |packet| events.feed(packet, |code, p| stack.event(code, p)));
        match polled {
            Ok(()) => self.failures = 0,
            Err(PortError::Gone) => { port.say(format_args!("THE CONTROLLER IS GONE: STOPPED")); return Next::Idle; }
            Err(PortError::Failed) => {
                self.failures += 1;
                if self.failures >= FAILED_POLLS { port.say(format_args!("{} FAILED POLLS OF THE EVENTS IN A ROW: STOPPED", self.failures)); return Next::Idle; }
            }
        }
        // ACL data come only on a link: the bulk IN endpoint is polled only while one is up.
        if self.stack.links() > 0 {
            let (stack, stream) = (&mut self.stack, &mut self.stream);
            if port.acl_in(&mut |packet| stream.feed(packet, |p| stack.acl(p))) == Err(PortError::Gone) { port.say(format_args!("THE CONTROLLER IS GONE: STOPPED")); return Next::Idle; }
        }
        self.stack.tick(now);
        while let Some(n) = self.stack.ctl.next_command(&mut self.out) {
            if let Err(e) = port.command(&self.out[..n]) { port.say(format_args!("COMMAND {:02X}{:02X} NOT SENT: {:?}", self.out[1], self.out[0], e)); }
        }
        while let Some(n) = self.stack.ctl.next_acl(&mut self.out) {
            if let Err(e) = port.acl_out(&self.out[..n]) { port.say(format_args!("ACL PACKET NOT SENT: {:?}", e)); }
        }
        let mut line = [0u8; LINE];
        while let Some(l) = self.stack.ctl.log.take(&mut line) { port.say(format_args!("{}", l)); }
        let mut buf = [0u8; UP_MAX];
        while let Some(u) = self.stack.ups.take(&mut buf) { if u == Up::Keys { self.keys_changed = true; } else { up(u); } }
        Next::Sleep(if self.stack.links() > 0 { BUSY_MS } else { IDLE_MS })
    }
}
