use crate::{btc_session, fwcmd, io::{self, RegisterIo}, model};
#[path="rtw89_channel_device.rs"] pub(super) mod channel;
use channel::ChannelModel;

// Extend one cold device with write-triggered RF/LTE/calibration replies, without reseeding firmware or MAC guards.
pub(super) struct Device {
    pub(super) channel: Option<ChannelModel>,
    pub(super) inner: model::Device, rf: [[u32; 256]; 2], rf_read: Option<(usize, usize)>,
    lte: [u32; 2], lte_pending: Option<u32>, pub(super) bt: u32, pub(super) drck: bool, pub(super) addck: [bool; 2],
    msbk: [[bool; 2]; 2], dadck: [[bool; 2]; 2], pub(super) nctl_stuck: bool, pub(super) rck_stuck: bool,
    pub(super) drck_stuck: bool, pub(super) fail_write: Option<usize>,
}
impl Device {
    pub(super) fn new(inner: model::Device) -> Self {
        Self { channel: None, inner, rf: [[0; 256]; 2], rf_read: None, lte: [0; 2], lte_pending: None, bt: 0,
            drck: false, addck: [false; 2], msbk: [[false; 2]; 2], dadck: [[false; 2]; 2],
            nctl_stuck: false, rck_stuck: false, drck_stuck: false, fail_write: None }
    }
    fn write(&mut self, at: usize, width: usize, value: u32) -> io::Result<()> {
        self.inner.now += 1;
        let previous = self.inner.get(at, width);
        match width { 1 => self.inner.write8(at, value as u8)?, 2 => self.inner.write16(at, value as u16)?, _ => self.inner.write32(at, value)? }
        if at == 0x10378 { self.rf_read = Some((((value >> 8) & 7) as usize, (value & 255) as usize)); }
        if at == 0x10370 {
            let path = ((value >> 28) & 3) as usize; assert!(path < 2);
            let reg = ((value >> 20) & 255) as usize;
            let mask = if value >> 31 != 0 { self.inner.get(0x10374, 4) & 0xfffff } else { 0xfffff };
            self.rf[path][reg] = (self.rf[path][reg] & !mask) | (value & mask); self.rf_read = None;
            if reg == 0x1b && value & 0xfffff == 0x240 && !self.rck_stuck {
                self.rf[path][0x1c] |= 8; self.rf[path][0x1b] = (7 + path as u32 * 4) << 10;
            }
        }
        if at == 0xdaf0 { assert!(self.lte_pending.is_none()); self.lte_pending = Some(value); }
        if at == 0x1c0cc && value & 64 != 0 { self.drck = true; }
        for path in 0..2 {
            let base = 0x1c000 + path * 0x100;
            if at == base + 0xf4 && value & 0x300 == 0x100 { self.addck[path] = true; }
            for iq in 0..2 {
                if at == base + iq * 0x20 + 4 && value & 1 != 0 { self.msbk[path][iq] = true; }
                if at == base + iq * 0x20 + 12 && value & 4 != 0 { self.dadck[path][iq] = true; }
            }
        }
        if let Some(channel) = &mut self.channel { channel.write(&mut self.inner, &mut self.rf, at, previous, value); }
        if self.fail_write == Some(at) { self.fail_write = None; return Err(io::Fault::new(io::FaultKind::Access, at, value)); }
        Ok(())
    }
}
impl RegisterIo for Device {
    fn read8(&mut self, at: usize) -> io::Result<u8> {
        self.inner.now += 1;
        if at == 0xdaf3 {
            if let Some(command) = self.lte_pending.take() {
                if command >> 30 == 3 { self.lte[usize::from(command & 255 == 0x3c)] = self.inner.get(0xdaf4, 4); }
            }
            return Ok(0x20);
        }
        self.inner.read8(at)
    }
    fn read16(&mut self, at: usize) -> io::Result<u16> { self.inner.now += 1; self.inner.read16(at) }
    fn read32(&mut self, at: usize) -> io::Result<u32> {
        self.inner.now += 1;
        if let Some(channel) = &self.channel { if let Some(result) = channel.read(&self.inner, at) { return result; } }
        if at == 0xac { return Ok(self.bt); }
        if at == 0xdaf8 { let command = self.lte_pending.unwrap(); assert_eq!(command >> 30, 2); return Ok(self.lte[usize::from(command & 255 == 0x3c)]); }
        if at == 0x1174c { return Ok(self.rf_read.map_or(0, |(path, reg)| 1 << 26 | self.rf[path][reg])); }
        if at == 0x18080 && self.nctl_stuck { return Ok(0); }
        if at == 0x1c0d0 { return Ok(if self.drck && !self.drck_stuck { 17 << 15 | 8 } else { 0 }); }
        for path in 0..2 {
            let base = 0x1c000 + path * 0x100;
            if at == base + 0xfc { return Ok(if self.addck[path] { 0x123 << 10 | 0x155 } else { 0 }); }
            for iq in 0..2 {
                if at == base + 0x40 + iq * 0x24 { return Ok(u32::from(self.msbk[path][iq]) << 31); }
                if at == base + 0x5c + iq * 0x24 {
                    let index = (self.inner.get(base + iq * 0x20, 4) >> 1) & 15;
                    return Ok(u32::from(self.dadck[path][iq]) * 4 | (0x10 + path as u32 * 0x40 + iq as u32 * 0x20 + index) << 24);
                }
            }
        }
        self.inner.read32(at)
    }
    fn write8(&mut self, at: usize, value: u8) -> io::Result<()> { self.write(at, 1, value as u32) }
    fn write16(&mut self, at: usize, value: u16) -> io::Result<()> { self.write(at, 2, value as u32) }
    fn write32(&mut self, at: usize, value: u32) -> io::Result<()> { self.write(at, 4, value) }
    fn now_us(&mut self) -> u64 { self.inner.now }
    fn delay_us(&mut self, us: u32) -> io::Result<()> { self.inner.delay_us(us) }
}
impl fwcmd::Bus for Device { fn pci_command(&mut self) -> io::Result<u16> { Ok(self.inner.command) } }
impl btc_session::DeviceIo for Device {
    fn pci_id(&mut self) -> io::Result<u32> { Ok(0xb85210ec) }
    fn bar_len(&self) -> usize { 0x20000 }
}

