// LEGACY: the PS/2 mouse on the i8042 auxiliary port (issue 156, docs/legacy.md). Enables the port and its IRQ 12,
// asks for the IntelliMouse wheel (4-byte packets) and turns packets into pointer events (common/abi.rs).
use mind::abi::pointer_event;
use mind::dev::Ports;

const DATA: u16 = 0x60;
const STATUS: u16 = 0x64; // also the command port
const ACK: u8 = 0xFA;

fn wait(status: &Ports, ready: impl Fn(u8) -> bool) -> bool { (0..100_000).any(|_| ready(status.in8(STATUS))) }
fn command(status: &Ports, byte: u8) -> bool { wait(status, |s| s & 2 == 0) && { status.out8(STATUS, byte); true } }
fn read(data: &Ports, status: &Ports) -> Option<u8> { wait(status, |s| s & 1 != 0).then(|| data.in8(DATA)) }
// A byte for the mouse; true when it acknowledged it.
fn send(data: &Ports, status: &Ports, byte: u8) -> bool {
    command(status, 0xD4) && wait(status, |s| s & 2 == 0) && { data.out8(DATA, byte); read(data, status) == Some(ACK) }
}

pub struct Mouse { packet: [u8; 4], len: usize, size: usize }

impl Mouse {
    /// Turns the mouse on; None without one (the keyboard keeps working).
    pub fn setup(data: &Ports, status: &Ports) -> Option<Self> {
        command(status, 0xA8); // enable the auxiliary port
        command(status, 0x20);
        let config = read(data, status)?;
        command(status, 0x60);
        wait(status, |s| s & 2 == 0);
        data.out8(DATA, (config | 2) & !0x20); // IRQ 12 on, auxiliary clock on
        if !send(data, status, 0xF6) { return None; } // defaults
        // IntelliMouse: sample rates 200, 100, 80, then the ID says whether a wheel byte follows.
        for rate in [200, 100, 80] { let _ = send(data, status, 0xF3) && send(data, status, rate); }
        let wheel = send(data, status, 0xF2) && read(data, status) == Some(3);
        if !send(data, status, 0xF4) { return None; } // report movement
        Some(Self { packet: [0; 4], len: 0, size: if wheel { 4 } else { 3 } })
    }

    pub fn wheel(&self) -> bool { self.size == 4 }

    /// One byte from the auxiliary port; a complete packet gives a pointer event word.
    pub fn feed(&mut self, byte: u8) -> Option<usize> {
        if self.len == 0 && byte & 0x08 == 0 { return None; } // not the first byte of a packet: resynchronize
        self.packet[self.len] = byte; self.len += 1;
        if self.len < self.size { return None; }
        self.len = 0;
        let [flags, x, y, z] = self.packet;
        if flags & 0xC0 != 0 { return None; } // overflow: the movement is meaningless
        let dx = x as i32 - ((flags as i32) << 4 & 0x100);
        let dy = y as i32 - ((flags as i32) << 3 & 0x100);
        let wheel = if self.size == 4 { ((z & 0x0F) as i8) << 4 >> 4 } else { 0 } as i32;
        Some(pointer_event(flags & 7, dx, -dy, wheel)) // PS/2 counts y upwards; events count it downwards
    }
}
