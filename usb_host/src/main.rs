#![no_std]
#![no_main]
// Ring 3 USB host controller driver (issue 164): the xHCI controller through its MMIO capability and a DMA region. It
// enumerates the devices on the root ports and behind USB 2 hubs, configures the endpoints of the interfaces it has
// class drivers for (HID, mass storage), and serves idl/usb.wit: a class driver claims an interface of the one class
// its badge names and moves data through the buffer it lends. Ports are scanned every SCAN_MS, so devices may come and
// go. Events are polled (no interrupt line).
mod xhci;

use mind::abi::{BootInfo, SLOT_DEV0, SLOT_MEM, ERR_TIMEOUT};
use mind::dev::{Dma, Mmio};
use mind::idl::{usb, wire};
use mind::ipc::Endpoint;
use mind::mem::Mapping;
use mind::sys::{Error, Result};
use mind::usb::{class_of, EndpointInfo, Interface, BULK_MAX, CONTROL_MAX, MAX_ENDPOINTS};
use xhci::{Ring, Xhci, DATA, EP_BULK_IN, EP_BULK_OUT, EP_CONTROL, EP_INTERRUPT_IN, EP_INTERRUPT_OUT, IOC, ISP, SMALL, STALL, TYPE_NORMAL};

const RECEIVED_CAP: usize = 9;
const SCAN_MS: u64 = 250;
const MAX_DEVICES: usize = 16; const MAX_INTERFACES: usize = 4; const MAX_RINGS: usize = 6; const MAX_DEPTH: u8 = 5;
const CLASS_HUB: u8 = 9;
const SERVED: [u8; 2] = [3, 8]; // HID, mass storage: the classes with a class driver

#[derive(Clone, Copy, Default)]
struct Iface { info: Interface, owner: u16 }

#[derive(Clone, Copy)]
struct Device {
    slot: u8, root: u8, route: u32, depth: u8, speed: u8,
    parent: Option<(usize, u8)>, // the hub it is on and the port
    tt: Option<(u8, u8)>,        // the transaction translator a full- or low-speed device behind a high-speed hub uses
    hub_ports: u8, failed: u16,  // a hub's ports, and those whose device could not be set up
    status: u8, pending: u16,     // a hub's status change endpoint (DCI) and the ports it said changed
    generation: u16, output: usize, ep0: Ring, rings: [(u8, Ring); MAX_RINGS], ring_count: usize,
    interfaces: [Iface; MAX_INTERFACES], count: usize,
}

struct Host { xhci: Xhci, devices: [Option<Device>; MAX_DEVICES], generation: u16, buffers: [Option<Mapping>; 3], root_failed: u64, scanned: u64, step: &'static str }

fn speed_name(speed: u8) -> &'static str { match speed { 1 => "FULL", 2 => "LOW", 3 => "HIGH", 4 => "SUPER", _ => "?" } }
fn dci(address: u8) -> u8 { (address & 0xF) * 2 + (address >> 7) }

// Where a device is: its root port, then the hub port at each tier (5.1: port 1 of the hub on root port 5).
struct Path(u8, u32);
impl core::fmt::Display for Path {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{}", self.0)?;
        let mut route = self.1;
        while route != 0 { write!(f, ".{}", route & 0xF)?; route >>= 4; }
        Ok(())
    }
}

impl Host {
    fn small(&mut self, length: usize) -> &mut [u8] { self.xhci.dma.bytes(SMALL, length) }

    // A control transfer on device `index`'s endpoint 0 (data in the SMALL area).
    fn control(&mut self, index: usize, request_type: u8, request: u8, value: u16, windex: u16, length: u16) -> core::result::Result<usize, u32> {
        let Some(device) = self.devices[index].as_mut() else { return Err(0) };
        let mut ring = device.ep0;
        let result = self.xhci.control(device.slot, &mut ring, request_type, request, value, windex, length);
        if let Some(device) = self.devices[index].as_mut() { device.ep0 = ring; }
        result
    }

    // Gives back a slot and its pages.
    fn discard(&mut self, slot: u8, pages: &[usize]) { self.xhci.disable_slot(slot); for &page in pages { self.xhci.release(page); } }

    /// Sets up the device on a port: address, descriptors, configuration, its endpoints or, for a hub, its ports.
    fn enumerate(&mut self, root: u8, route: u32, depth: u8, speed: u8, parent: Option<(usize, u8)>) -> Option<usize> {
        self.step = "NO FREE DEVICE";
        let index = self.devices.iter().position(Option::is_none)?;
        self.step = "ENABLE SLOT";
        let slot = self.xhci.enable_slot()?;
        let Some(output) = self.xhci.alloc() else { self.discard(slot, &[]); return None };
        let Some(ep0) = self.xhci.ring(false) else { self.discard(slot, &[output]); return None };
        self.xhci.set_output(slot, output);
        let tt = parent.and_then(|(p, port)| { let hub = self.devices[p]?; if hub.speed == 3 && speed < 3 { Some((hub.slot, port)) } else { hub.tt } });
        self.xhci.input_reset(0b11);
        self.xhci.slot_context(route, speed, 1, root, None, tt);
        self.xhci.endpoint_context(1, EP_CONTROL, match speed { 4 => 512, 3 => 64, _ => 8 }, 0, ep0);
        self.step = "ADDRESS DEVICE";
        if self.xhci.address(slot).is_none() { self.discard(slot, &[output, ep0.page]); return None; }
        self.generation = self.generation.wrapping_add(1).max(1);
        self.devices[index] = Some(Device { slot, root, route, depth, speed, parent, tt, hub_ports: 0, failed: 0, status: 0, pending: 0, generation: self.generation, output, ep0,
            rings: [(0, Ring::default()); MAX_RINGS], ring_count: 0, interfaces: [Iface::default(); MAX_INTERFACES], count: 0 });
        if self.configure(index).is_none() { self.remove(index); return None; }
        Some(index)
    }

    fn configure(&mut self, index: usize) -> Option<()> {
        let device = self.devices[index]?;
        let slot = device.slot;
        // Endpoint 0's real packet size is in the first 8 bytes of the device descriptor (8 to 64 at full speed).
        self.step = "DEVICE DESCRIPTOR";
        self.control(index, 0x80, 6, 0x0100, 0, 8).ok()?;
        let packet = self.small(8)[7] as u16;
        if device.speed == 1 && packet != 8 && matches!(packet, 16 | 32 | 64) {
            self.xhci.input_reset(0b10);
            self.xhci.endpoint_context(1, EP_CONTROL, packet, 0, device.ep0);
            self.xhci.evaluate(slot)?;
        }
        self.control(index, 0x80, 6, 0x0100, 0, 18).ok()?;
        let descriptor: [u8; 18] = self.small(18).try_into().ok()?;
        let (class, vendor, product) = (descriptor[4], u16::from_le_bytes([descriptor[8], descriptor[9]]), u16::from_le_bytes([descriptor[10], descriptor[11]]));
        self.step = "CONFIGURATION DESCRIPTOR";
        self.control(index, 0x80, 6, 0x0200, 0, 9).ok()?;
        let total = u16::from_le_bytes([self.small(4)[2], self.small(4)[3]]).clamp(9, CONTROL_MAX as u16);
        self.control(index, 0x80, 6, 0x0200, 0, total).ok()?;
        let mut config = [0u8; CONTROL_MAX];
        config[..total as usize].copy_from_slice(self.small(total as usize));
        let config = &config[..total as usize];
        // Interfaces (alternate setting 0) and their endpoints.
        let (mut interfaces, mut count, mut current) = ([Iface::default(); MAX_INTERFACES], 0usize, None);
        let mut at = 0;
        while at + 2 <= config.len() && config[at] >= 2 {
            let (length, kind) = (config[at] as usize, config[at + 1]);
            if kind == 4 && at + 9 <= config.len() {
                current = None;
                if config[at + 3] == 0 && count < MAX_INTERFACES {
                    interfaces[count].info = Interface { number: config[at + 2], class: config[at + 5], subclass: config[at + 6], protocol: config[at + 7], speed: device.speed, vendor, product, ..Default::default() };
                    current = Some(count); count += 1;
                }
            }
            if let (5, Some(i)) = (kind, current) {
                let info = &mut interfaces[i].info;
                if at + 7 <= config.len() && (info.count as usize) < MAX_ENDPOINTS {
                    info.endpoints[info.count as usize] = EndpointInfo { address: config[at + 2], attributes: config[at + 3], packet: u16::from_le_bytes([config[at + 4], config[at + 5]]) & 0x7FF, interval: config[at + 6] };
                    info.count += 1;
                }
            }
            at += length;
        }
        self.step = "SET CONFIGURATION";
        self.control(index, 0x00, 9, config[5] as u16, 0, 0).ok()?; // SET_CONFIGURATION
        let hub = class == CLASS_HUB || interfaces[..count].iter().any(|i| i.info.class == CLASS_HUB);
        mind::println!("[USB] {:04X}:{:04X} ON PORT {} ({} SPEED){}", vendor, product, Path(device.root, device.route), speed_name(device.speed), if hub { " HUB" } else { "" });
        // A USB 2 hub: its ports, think time and power-on delay (a SuperSpeed hub's USB 2 side is used instead).
        let hub = if !hub || device.speed == 4 || device.depth >= MAX_DEPTH { None } else {
            self.step = "HUB DESCRIPTOR";
            self.control(index, 0xA0, 6, 0x2900, 0, 9).ok()?;
            let descriptor: [u8; 9] = self.small(9).try_into().ok()?;
            Some((descriptor[2].min(15), ((descriptor[3] >> 5) & 3) as u8, descriptor[5] as u64 * 2))
        };
        // The endpoints of the interfaces a class driver serves (and a hub's status endpoint), in one Configure Endpoint.
        let wanted = |class: u8| SERVED.contains(&class) || (class == CLASS_HUB && hub.is_some());
        let mut planned = [(0u8, 0u32, 0u16, 0u8); MAX_RINGS];
        let (mut add, mut entries, mut rings, mut ring_count, mut status) = (1u32, 1u32, [(0u8, Ring::default()); MAX_RINGS], 0, None);
        for iface in interfaces[..count].iter().filter(|i| wanted(i.info.class)) {
            for endpoint in iface.info.endpoints() {
                if !(endpoint.is_bulk() || endpoint.is_interrupt()) || ring_count == MAX_RINGS { continue; }
                let (target, input) = (dci(endpoint.address), endpoint.is_in());
                let Some(ring) = self.xhci.ring(endpoint.is_interrupt() && input) else { break };
                let kind = match (endpoint.is_bulk(), input) { (true, false) => EP_BULK_OUT, (true, true) => EP_BULK_IN, (false, false) => EP_INTERRUPT_OUT, (false, true) => EP_INTERRUPT_IN };
                // The interval as 2^n units of 125 µs: frames (1 ms) at full and low speed, an exponent above.
                let interval = if !endpoint.is_interrupt() { 0 } else if device.speed >= 3 { endpoint.interval.clamp(1, 16) - 1 }
                               else { (31 - (endpoint.interval.max(1) as u32 * 8).leading_zeros()).clamp(3, 10) as u8 };
                if iface.info.class == CLASS_HUB && endpoint.is_interrupt() && input { status = Some((target, ring, endpoint.packet)); }
                rings[ring_count] = (target, ring); planned[ring_count] = (target, kind, endpoint.packet, interval); ring_count += 1;
                add |= 1 << target; entries = entries.max(target as u32);
            }
        }
        // The rings are the device's from here, so `remove` gives them back if configuring fails.
        if let Some(d) = self.devices[index].as_mut() { d.rings = rings; d.ring_count = ring_count; d.interfaces = interfaces; d.count = count; }
        if ring_count > 0 || hub.is_some() {
            self.xhci.input_reset(add);
            let hub_fields = hub.map(|(ports, think, _)| (ports, if device.speed == 3 { think } else { 0 }));
            self.xhci.slot_context(device.route, device.speed, entries, device.root, hub_fields, device.tt);
            for (&(target, kind, packet, interval), &(_, ring)) in planned[..ring_count].iter().zip(&rings[..ring_count]) {
                self.xhci.endpoint_context(target, kind, packet, interval, ring);
            }
            self.step = "CONFIGURE ENDPOINT";
            self.xhci.configure(slot)?;
        }
        if let Some((ports, _, power)) = hub {
            for port in 1..=ports { let _ = self.control(index, 0x23, 3, 8, port as u16, 0); } // SET_FEATURE PORT_POWER
            mind::time::sleep(power.clamp(20, 500) as usize);
            // Every port is looked at once; after that only those the status endpoint reports (all, without one).
            let armed = status.is_some_and(|(target, ring, packet)| self.xhci.arm(slot, target, ring, packet));
            if let Some(d) = self.devices[index].as_mut() { d.hub_ports = ports; d.pending = 0xFFFE; d.status = if armed { status.map_or(0, |s| s.0) } else { 0 }; }
        }
        Some(())
    }

    /// Removes a device that went away, and everything behind it if it is a hub.
    fn remove(&mut self, index: usize) {
        for child in 0..MAX_DEVICES { if self.devices[child].is_some_and(|d| d.parent.is_some_and(|p| p.0 == index)) { self.remove(child); } }
        let Some(device) = self.devices[index].take() else { return };
        let mut pages = [0usize; MAX_RINGS + 2];
        pages[0] = device.output; pages[1] = device.ep0.page;
        for (i, ring) in device.rings[..device.ring_count].iter().enumerate() { pages[2 + i] = ring.1.page; }
        self.discard(device.slot, &pages[..2 + device.ring_count]);
        mind::println!("[USB] DEVICE ON PORT {} GONE", Path(device.root, device.route));
    }

    // A hub port's status and change bits.
    fn hub_port(&mut self, hub: usize, port: u8) -> Option<(u16, u16)> {
        self.control(hub, 0xA3, 0, 0, port as u16, 4).ok()?;
        let s = self.small(4);
        Some((u16::from_le_bytes([s[0], s[1]]), u16::from_le_bytes([s[2], s[3]])))
    }

    /// Looks at every root and hub port: new devices are set up, gone ones removed.
    fn scan(&mut self) {
        for port in 1..=self.xhci.ports().min(64) {
            let changed = self.xhci.acknowledge(port);
            let connected = self.xhci.connected(port);
            let present = (0..MAX_DEVICES).find(|&i| self.devices[i].is_some_and(|d| d.parent.is_none() && d.root as usize == port));
            if changed { self.root_failed &= !(1 << (port - 1)); }
            if let Some(index) = present { if !connected || changed { self.remove(index); } else { continue; } }
            if !connected || self.root_failed & 1 << (port - 1) != 0 { continue; }
            self.step = "PORT RESET"; self.xhci.last = 0;
            let set_up = self.xhci.enable_port(port).and_then(|speed| self.enumerate(port as u8, 0, 0, speed, None));
            self.xhci.acknowledge(port); // the reset's own change bits are not a new connection
            // The step that failed, the controller's completion code and the port's status (211-PRT-0004: a Mac's ports).
            if set_up.is_none() { self.root_failed |= 1 << (port - 1); mind::println!("[USB] PORT {}: DEVICE NOT SET UP AT {} (COMPLETION {}, PORTSC {:08X})", port, self.step, self.xhci.last, self.xhci.port_status(port)); }
        }
        for hub in 0..MAX_DEVICES {
            let Some(device) = self.devices[hub] else { continue };
            if device.hub_ports == 0 { continue; }
            // The ports whose status changed: bit n of the status endpoint's report is port n.
            let mut pending = device.pending;
            if device.status == 0 { pending = 0xFFFE; }
            else if let Some(armed) = self.xhci.armed(device.slot, device.status) {
                let _ = self.xhci.take_reports(armed, |report| { for (i, &byte) in report.iter().take(2).enumerate() { pending |= (byte as u16) << (8 * i); } });
            }
            if let Some(d) = self.devices[hub].as_mut() { d.pending = 0; }
            for port in (1..=device.hub_ports).filter(|&p| pending & 1 << p != 0) {
                let Some((status, change)) = self.hub_port(hub, port) else { break };
                if change & 1 != 0 { let _ = self.control(hub, 0x23, 1, 16, port as u16, 0); } // CLEAR_FEATURE C_PORT_CONNECTION
                let bit = 1u16 << port;
                if change & 1 != 0 { if let Some(d) = self.devices[hub].as_mut() { d.failed &= !bit; } }
                let child = (0..MAX_DEVICES).find(|&i| self.devices[i].is_some_and(|d| d.parent == Some((hub, port))));
                let connected = status & 1 != 0;
                if let Some(index) = child { if !connected || change & 1 != 0 { self.remove(index); } else { continue; } }
                if !connected || self.devices[hub].is_none_or(|d| d.failed & bit != 0) { continue; }
                self.step = "HUB PORT RESET"; self.xhci.last = 0;
                let set_up = self.reset_hub_port(hub, port).and_then(|speed| {
                    let hub_device = self.devices[hub]?;
                    let route = hub_device.route | (port as u32) << (4 * hub_device.depth as u32);
                    self.enumerate(hub_device.root, route, hub_device.depth + 1, speed, Some((hub, port)))
                });
                if self.hub_port(hub, port).is_some_and(|(_, change)| change & 1 != 0) { let _ = self.control(hub, 0x23, 1, 16, port as u16, 0); }
                if set_up.is_none() { if let Some(d) = self.devices[hub].as_mut() { d.failed |= bit; } mind::println!("[USB] PORT {}.{}: DEVICE NOT SET UP AT {} (COMPLETION {})", Path(device.root, device.route), port, self.step, self.xhci.last); }
            }
        }
    }

    // Resets a hub port; the speed of the device on it once enabled.
    fn reset_hub_port(&mut self, hub: usize, port: u8) -> Option<u8> {
        self.control(hub, 0x23, 3, 4, port as u16, 0).ok()?; // SET_FEATURE PORT_RESET
        let mut status = 0;
        for _ in 0..50 {
            mind::time::sleep(10);
            let (s, change) = self.hub_port(hub, port)?;
            if change & 0x10 != 0 { status = s; break; }
        }
        let _ = self.control(hub, 0x23, 1, 20, port as u16, 0); // CLEAR_FEATURE C_PORT_RESET
        if status & 2 == 0 { return None; }
        mind::time::sleep(10); // reset recovery
        Some(if status & 0x200 != 0 { 2 } else if status & 0x400 != 0 { 3 } else { 1 })
    }

    // The device and interface a handle names, if the client with `badge` holds it.
    fn interface(&self, handle: u32, badge: u16) -> Result<(usize, usize)> {
        let (generation, index, iface) = ((handle >> 16) as u16, ((handle >> 8) & 0xFF) as usize, (handle & 0xFF) as usize);
        let device = self.devices.get(index).copied().flatten().ok_or(Error::NotFound)?;
        if device.generation != generation || iface >= device.count || device.interfaces[iface].owner != badge { return Err(Error::NotFound); }
        Ok((index, iface))
    }

    fn buffer(&mut self, badge: u16) -> Result<&mut [u8]> {
        Ok(self.buffers.get_mut(badge as usize).and_then(Option::as_mut).ok_or(Error::NotFound)?.as_mut_slice())
    }

    fn claim(&mut self, badge: u16) -> Result<u32> {
        let class = class_of(badge).ok_or(Error::Rights)?;
        self.buffer(badge).map_err(|_| Error::Invalid)?; // no buffer: this instance is new to the client
        for index in 0..MAX_DEVICES {
            let Some(device) = self.devices[index].as_mut() else { continue };
            let Some(iface) = (0..device.count).find(|&i| device.interfaces[i].info.class == class && device.interfaces[i].owner == 0) else { continue };
            device.interfaces[iface].owner = badge;
            let (info, generation) = (device.interfaces[iface].info, device.generation);
            info.encode(self.buffer(badge)?);
            return Ok((generation as u32) << 16 | (index as u32) << 8 | iface as u32);
        }
        Err(Error::NotFound)
    }

    fn release(&mut self, handle: u32, badge: u16) -> Result<()> {
        let (index, iface) = self.interface(handle, badge)?;
        let device = self.devices[index].as_mut().ok_or(Error::NotFound)?;
        device.interfaces[iface].owner = 0;
        let (slot, info) = (device.slot, device.interfaces[iface].info);
        for endpoint in info.endpoints() { self.xhci.forget(slot, dci(endpoint.address)); }
        Ok(())
    }

    fn release_all(&mut self, badge: u16) {
        for index in 0..MAX_DEVICES {
            let Some(device) = self.devices[index] else { continue };
            for iface in (0..device.count).filter(|&i| device.interfaces[i].owner == badge) {
                for endpoint in device.interfaces[iface].info.endpoints() { self.xhci.forget(device.slot, dci(endpoint.address)); }
                if let Some(d) = self.devices[index].as_mut() { d.interfaces[iface].owner = 0; }
            }
        }
    }

    // A class driver may ask its interface (class and vendor requests, standard ones to the interface), its endpoints
    // (clearing a halt) and descriptors; it may not change the device's address or configuration.
    fn allowed(info: &Interface, request_type: u8, request: u8, index: u16) -> bool {
        let (kind, recipient) = ((request_type >> 5) & 3, request_type & 0x1F);
        match (kind, recipient) {
            (0, 0) => request == 6 && request_type & 0x80 != 0, // GET_DESCRIPTOR from the device
            (_, 1) => index as u8 == info.number,
            (_, 2) => info.endpoints().iter().any(|e| e.address == index as u8),
            _ => false,
        }
    }

    fn control_request(&mut self, badge: u16, handle: u32, request_type: u8, request: u8, value: u16, index: u16, length: u16) -> Result<u16> {
        let (device, iface) = self.interface(handle, badge)?;
        let info = self.devices[device].ok_or(Error::NotFound)?.interfaces[iface].info;
        if !Self::allowed(&info, request_type, request, index) { return Err(Error::Rights); }
        if length as usize > CONTROL_MAX { return Err(Error::Invalid); }
        let input = request_type & 0x80 != 0;
        if !input { let data: [u8; CONTROL_MAX] = { let mut d = [0; CONTROL_MAX]; d[..length as usize].copy_from_slice(&self.buffer(badge)?[..length as usize]); d }; self.small(length as usize).copy_from_slice(&data[..length as usize]); }
        let got = self.control(device, request_type, request, value, index, length).map_err(|code| if code == 0 { Error::Other(ERR_TIMEOUT) } else { Error::Invalid })?;
        if input { let mut data = [0u8; CONTROL_MAX]; data[..got].copy_from_slice(self.small(got)); self.buffer(badge)?[..got].copy_from_slice(&data[..got]); }
        Ok(got as u16)
    }

    fn bulk(&mut self, badge: u16, handle: u32, address: u8, offset: u32, length: u32) -> Result<u32> {
        let (index, iface) = self.interface(handle, badge)?;
        let device = self.devices[index].ok_or(Error::NotFound)?;
        let endpoint = device.interfaces[iface].info.endpoints().iter().copied().find(|e| e.address == address && e.is_bulk()).ok_or(Error::Invalid)?;
        let (offset, length) = (offset as usize, length as usize);
        let size = self.buffer(badge)?.len();
        if length == 0 || length > BULK_MAX || offset.checked_add(length).is_none_or(|end| end > size) { return Err(Error::Invalid); }
        let target = dci(endpoint.address);
        let at = device.rings[..device.ring_count].iter().position(|r| r.0 == target).ok_or(Error::Invalid)?;
        let mut ring = device.rings[at].1;
        if !endpoint.is_in() {
            let source = self.buffers[badge as usize].as_ref().ok_or(Error::NotFound)?.as_slice()[offset..offset + length].as_ptr();
            self.xhci.dma.bytes(DATA, length).copy_from_slice(unsafe { core::slice::from_raw_parts(source, length) });
        }
        let data = self.xhci.physical(DATA);
        let result = self.xhci.transfer(device.slot, target, &mut ring, &[(data, length as u32, TYPE_NORMAL << 10 | IOC | ISP)]);
        if let Some(d) = self.devices[index].as_mut() { d.rings[at].1 = ring; }
        match result {
            Ok(residue) => {
                let got = length.saturating_sub(residue as usize);
                if endpoint.is_in() {
                    let source = self.xhci.dma.bytes(DATA, got).as_ptr();
                    self.buffer(badge)?[offset..offset + got].copy_from_slice(unsafe { core::slice::from_raw_parts(source, got) });
                }
                Ok(got as u32)
            }
            Err(STALL) => {
                // The endpoint halted: reset it on both sides, so the next transfer can run (BOT recovery).
                self.xhci.recover(device.slot, target, &ring);
                let _ = self.control(index, 0x02, 1, 0, address as u16, 0); // CLEAR_FEATURE ENDPOINT_HALT
                Err(Error::Invalid)
            }
            Err(0) => Err(Error::Other(ERR_TIMEOUT)),
            Err(_) => Err(Error::Invalid),
        }
    }

    fn reports(&mut self, badge: u16, handle: u32, address: u8) -> Result<u16> {
        let (index, iface) = self.interface(handle, badge)?;
        let device = self.devices[index].ok_or(Error::NotFound)?;
        let endpoint = device.interfaces[iface].info.endpoints().iter().copied().find(|e| e.address == address && e.is_interrupt() && e.is_in()).ok_or(Error::Invalid)?;
        let target = dci(address);
        let armed = match self.xhci.armed(device.slot, target) {
            Some(armed) => armed,
            None => {
                let ring = device.rings[..device.ring_count].iter().find(|r| r.0 == target).ok_or(Error::Invalid)?.1;
                if !self.xhci.arm(device.slot, target, ring, endpoint.packet) { return Err(Error::NoMemory); }
                self.xhci.armed(device.slot, target).ok_or(Error::NoMemory)?
            }
        };
        let mut out = [0u8; 8 * 65]; let mut at = 0;
        let count = self.xhci.take_reports(armed, |report| { out[at] = report.len() as u8; out[at + 1..at + 1 + report.len()].copy_from_slice(report); at += 1 + report.len(); })
            .map_err(|_| Error::NotFound)?;
        self.buffer(badge)?[..at].copy_from_slice(&out[..at]);
        Ok(count as u16)
    }

    fn serve(&mut self, badge: u16, request: usb::Request, call: wire::Call) {
        let _ = match request {
            usb::Request::Attach { buffer } => {
                // The earlier buffer goes first: its address may be the one the new mapping gets.
                let result = if class_of(badge).is_none() { Err(Error::Rights) } else {
                    drop(self.buffers[badge as usize].take());
                    self.release_all(badge); // a new buffer is a new client instance: the old one's interfaces are free
                    Mapping::new(buffer).and_then(|m| if m.len() < mind::usb::BUFFER { Err(Error::Invalid) } else { self.buffers[badge as usize] = Some(m); Ok(()) })
                };
                usb::reply_attach(call, result)
            }
            usb::Request::Claim => { let r = self.claim(badge); usb::reply_claim(call, r) }
            usb::Request::Release { handle } => { let r = self.release(handle, badge); usb::reply_release(call, r) }
            usb::Request::Control { handle, request_type, request, value, index, length } => { let r = self.control_request(badge, handle, request_type, request, value, index, length); usb::reply_control(call, r) }
            usb::Request::Bulk { handle, address, offset, length } => { let r = self.bulk(badge, handle, address, offset, length); usb::reply_bulk(call, r) }
            usb::Request::Reports { handle, address } => { let r = self.reports(badge, handle, address); usb::reply_reports(call, r) }
        };
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let (Ok(mmio), Ok(dma)) = (Mmio::map(SLOT_DEV0), Dma::map(SLOT_MEM)) else { mind::println!("[USB] NO CONTROLLER OR DMA REGION"); return };
    let Some(xhci) = Xhci::init(mmio, dma) else { mind::println!("[USB] CONTROLLER DID NOT START"); return };
    mind::println!("[USB] XHCI: {} PORTS, {} SLOTS", xhci.ports(), xhci.slots);
    let mut host = Host { xhci, devices: [None; MAX_DEVICES], generation: 0, buffers: [None, None, None], root_failed: 0, scanned: 0, step: "" };
    // Ports that come up a little later are found by the next scans.
    mind::time::sleep(50);
    host.scan();
    host.scanned = mind::time::uptime_ms() as u64;
    loop {
        let received = Endpoint::SERVICE.recv_timeout(RECEIVED_CAP, SCAN_MS as u32);
        host.xhci.pump();
        let now = mind::time::uptime_ms() as u64;
        if now >= host.scanned + SCAN_MS { host.scan(); host.scanned = now; }
        let Ok(request) = received else { continue };
        match usb::decode(&request, RECEIVED_CAP) {
            Ok((call_request, call)) => host.serve(request.badge, call_request, call),
            Err(reason) => if request.is_call { let _ = wire::reject(reason); },
        }
    }
}
