//! One cumulative MMIO model: cold OTP access, MAC/PCIe setup and a synchronous command-DMA consumer.

use crate::{board, efuse, fwcmd, io, mac, pcie, power, registers::*, startup};
use io::RegisterIo;
use std::alloc::{alloc_zeroed, dealloc, Layout};

pub struct Region { pub base: *mut u8, layout: Layout }
impl Region {
    pub fn new() -> Self {
        let layout = Layout::from_size_align(startup::DMA_GRANT_BYTES, 4096).unwrap();
        let base = unsafe { alloc_zeroed(layout) }; assert!(!base.is_null());
        Self { base, layout }
    }
    pub fn queue<'a>(&'a mut self, device: &mut Device) -> fwcmd::Queue<'a> {
        device.dma = self.base;
        unsafe { fwcmd::Queue::new(device, self.base, self.layout.size(), 0x200000) }.unwrap()
    }
}
impl Drop for Region { fn drop(&mut self) { unsafe { dealloc(self.base, self.layout); } } }

pub struct Device {
    registers: Vec<u8>, xtal: [u8; 256], otp: [u8; 0x800], dav: [u8; 96],
    config: [u8; 4096], mdio: [u16; 128], dma: *mut u8,
    pub command: u16, pub now: u64, pub writes: Vec<(usize, usize, u32)>,
    pub packets: Vec<(bool, Vec<u8>)>, pub transferred: usize, pub no_h2c: bool,
}

impl Device {
    pub fn new(link_speed: u8) -> Self {
        let mut d = Self { registers: vec![0; 0x10000], xtal: [0; 256], otp: [0xff; 0x800], dav: [0xff; 96],
            config: [0; 4096], mdio: [0; 128], dma: std::ptr::null_mut(), command: 2, now: 0,
            writes: Vec::new(), packets: Vec::new(), transferred: 0, no_h2c: false };
        d.put(SYS_CFG1, 4, 1 << 12);
        d.put(R_AX_SYS_PW_CTRL, 4, B_AX_RDY_SYSPWR);
        d.put(R_AX_SYS_WL_EFUSE_CTRL, 2, B_AX_AUTOLOAD_SUS);
        d.put(fwcmd::INIT_CONFIG, 4, 0x30);
        d.config[0x82] = link_speed; d.config[0x719] = 0x38;
        d.mdio[0x1b] = 0xa5a5; d.mdio[0x1d] = 0xa500;
        d.mdio[0x30] = 0xa234; d.mdio[0x70] = 0xa234;
        let mut logical = [0xff; efuse::LOGICAL_SIZE];
        logical[efuse::MAC_AT..efuse::MAC_AT + 6].copy_from_slice(&[2, 3, 4, 5, 6, 7]);
        logical[efuse::RFE_AT] = 1; logical[efuse::XTAL_AT] = 0x20;
        logical[efuse::COUNTRY_AT..efuse::COUNTRY_AT + 2].copy_from_slice(b"US");
        logical[efuse::CHANNEL_PLAN_AT] = 0x7f;
        let mut at = 4;
        for (block, bytes) in logical.chunks_exact(8).enumerate() {
            if bytes == [0xff; 8] { continue; }
            d.otp[at] = 0x30 | (block >> 4) as u8; d.otp[at + 1] = (block as u8) << 4;
            d.otp[at + 2..at + 10].copy_from_slice(bytes); at += 10;
        }
        d.otp[efuse::PHYCAP_AT..efuse::PHYCAP_AT + efuse::PHYCAP_SIZE].fill(0x5a);
        d.otp[0x5ec..0x5ee].fill(0xff);
        d
    }

    pub fn get(&self, at: usize, width: usize) -> u32 {
        let mut raw = [0; 4]; raw[..width].copy_from_slice(&self.registers[at..at + width]); u32::from_le_bytes(raw)
    }
    fn put(&mut self, at: usize, width: usize, value: u32) { self.registers[at..at + width].copy_from_slice(&value.to_le_bytes()[..width]); }

    fn consume(&mut self, index: u32) {
        assert!(matches!(self.command & 7, 6 | 7));
        assert_eq!(self.get(fwcmd::DMA_STOP, 4) & 0x001f_0f03, 0x000b_0f03);
        assert_eq!(self.get(fwcmd::INIT_CONFIG, 4) & 0x2800, 0x800);
        assert!(!self.dma.is_null());
        let descriptor = ((index as usize + fwcmd::DESCRIPTORS - 1) % fwcmd::DESCRIPTORS) * 8;
        let (size, physical) = unsafe {
            (u32::from_le(self.dma.add(descriptor).cast::<u32>().read()),
             u32::from_le(self.dma.add(descriptor + 4).cast::<u32>().read()))
        };
        assert_ne!(size & (1 << 30), 0);
        let at = (physical as usize).checked_sub(0x200000).expect("descriptor before DMA region");
        let length = (size & 0xffff) as usize;
        assert!((fwcmd::TXWD_BYTES..=fwcmd::TXWD_BYTES + fwcmd::MAX_PAYLOAD).contains(&length));
        let end = at.checked_add(length).expect("descriptor end overflow");
        assert!(at >= fwcmd::BUFFERS_AT && end <= fwcmd::DMA_BYTES);
        let txwd = unsafe { u32::from_le(self.dma.add(at).cast::<u32>().read_unaligned()) };
        assert_eq!((txwd >> 16) & 15, 12);
        let part = txwd & (1 << 20) != 0;
        let packet = unsafe { std::slice::from_raw_parts(self.dma.add(at + fwcmd::TXWD_BYTES), length - fwcmd::TXWD_BYTES) }.to_vec();
        if part { self.transferred += packet.len(); } else { self.registers[0x1e0] |= 4; }
        self.packets.push((part, packet));
        self.put(fwcmd::INDEX, 4, index | index << 16);
        if self.transferred == 326_472 { self.registers[0x1e0] = (self.registers[0x1e0] & !0xe0) | 0xe0; }
    }

    fn write(&mut self, at: usize, width: usize, value: u32) -> io::Result<()> {
        io::check_offset(at, width, self.registers.len())?;
        let previous = self.get(at, width);
        self.writes.push((at, width, value)); self.put(at, width, value);
        if at == R_AX_SYS_PW_CTRL && width == 4 {
            if value & B_AX_APFN_ONMAC != 0 { self.put(R_AX_IC_PWR_STATE, 4, 1 << 8); }
            if value & B_AX_APFM_OFFMAC != 0 { self.put(R_AX_IC_PWR_STATE, 4, 0); }
            self.put(at, 4, value & !(B_AX_APFN_ONMAC | B_AX_APFM_OFFMAC));
        }
        if at == R_AX_EFUSE_CTRL && width == 4 {
            let address = ((value & B_AX_EF_ADDR_MASK) >> 16) as usize;
            self.put(at, 4, B_AX_EF_RDY | self.otp[address] as u32);
        }
        if at == R_AX_WLAN_XTAL_SI_CTRL && width == 4 {
            let address = (value & 0xff) as usize;
            if value & 0x01000000 != 0 { self.put(at, 4, (self.xtal[address] as u32) << 8 | address as u32); }
            else {
                let mask = (value >> 16) as u8;
                self.xtal[address] = (self.xtal[address] & !mask) | ((value >> 8) as u8 & mask);
                if address == XTAL_SI_CTRL as usize && mask == XTAL_SI_MODE_SEL_MASK as u8 {
                    let index = self.xtal[XTAL_SI_LOW_ADDR as usize] as usize | ((self.xtal[address] as usize & 7) << 8);
                    self.xtal[XTAL_SI_READ_VAL as usize] = self.dav[index]; self.xtal[address] |= XTAL_SI_RDY as u8;
                }
                self.put(at, 4, value & !B_AX_WL_XTAL_SI_CMD_POLL);
            }
        }
        if at == R_AX_DMAC_FUNC_EN && value & 0x0480_0000 == 0x0480_0000 {
            self.put(mac::WDE_READY, 4, 3); self.put(mac::PLE_READY, 4, 3);
        }
        if at == 0x10a0 && width == 2 && value & 0x300 != 0 {
            let index = ((value >> 12) & 3) as usize * 32 + (value & 31) as usize;
            if value & 0x100 != 0 { self.mdio[index] = self.get(0x10a4, 2) as u16; }
            else { self.put(0x10a6, 2, self.mdio[index] as u32); }
            self.put(at, 2, value & !0x300);
        }
        if at == 0x1092 && width == 1 {
            let command = self.get(0x1090, 2) as usize;
            assert!(value == 1 || value == 2);
            if value == 1 {
                let lane = ((command >> 12) & 15).trailing_zeros() as usize; assert!(lane < 4);
                self.config[(command & 0xffc) + lane] = self.registers[0x1094 + lane];
            } else {
                for lane in 0..4 { self.registers[0x1098 + lane] = self.config[(command & 0xffc) + lane]; }
            }
            self.registers[0x1092] = 0;
        }
        if at == fwcmd::CLEAR_INDEX {
            for (bit, index) in [(0, 0x1058), (1, 0x105c), (2, 0x1060), (3, 0x1064), (8, 0x1078), (9, 0x107c), (10, fwcmd::INDEX)] {
                if value & (1 << bit) != 0 { self.put(index, 4, 0); }
            }
            self.put(at, 4, 0);
        }
        if at == 0x1018 { self.put(0x1050, 4, 0); self.put(0x1054, 4, 0); self.put(at, 4, 0); }
        if at == fwcmd::INIT_CONFIG && value & 8 != 0 { self.put(at, 4, value & !8); }
        if at == 0x11d8 { self.put(at, 4, value & !2); }
        if at == R_AX_PLATFORM_ENABLE && value & 2 != 0 && previous & 2 == 0 && !self.no_h2c { self.registers[0x1e0] |= 2; }
        if at == fwcmd::INDEX { assert_eq!(width, 2); self.consume(value); }
        Ok(())
    }
}

impl RegisterIo for Device {
    fn read8(&mut self, at: usize) -> io::Result<u8> { io::check_offset(at, 1, self.registers.len())?; Ok(self.get(at, 1) as u8) }
    fn read16(&mut self, at: usize) -> io::Result<u16> { io::check_offset(at, 2, self.registers.len())?; Ok(self.get(at, 2) as u16) }
    fn read32(&mut self, at: usize) -> io::Result<u32> { io::check_offset(at, 4, self.registers.len())?; Ok(self.get(at, 4)) }
    fn write8(&mut self, at: usize, value: u8) -> io::Result<()> { self.write(at, 1, value as u32) }
    fn write16(&mut self, at: usize, value: u16) -> io::Result<()> { self.write(at, 2, value as u32) }
    fn write32(&mut self, at: usize, value: u32) -> io::Result<()> { self.write(at, 4, value) }
    fn now_us(&mut self) -> u64 { self.now }
    fn delay_us(&mut self, us: u32) -> io::Result<()> { self.now += u64::from(us); Ok(()) }
}
impl fwcmd::Bus for Device { fn pci_command(&mut self) -> io::Result<u16> { Ok(self.command) } }

pub fn read_board(device: &mut Device) -> board::Board {
    let result = board::read(device).unwrap();
    assert_eq!(result.identity.cut, 1); assert_eq!(result.identity.rfe, 1);
    assert_eq!(result.maps.phycap[0x6c..0x6e], [0xff, 0xff]);
    assert_eq!(device.get(R_AX_IC_PWR_STATE, 4) & B_AX_WLMAC_PWR_STE_MASK, 0);
    assert_eq!(device.get(R_AX_SYS_FUNC_EN, 1) & 3, 0);
    result
}

pub fn prepare<'a>(device: &mut Device, region: &'a mut Region) -> fwcmd::Queue<'a> {
    let mut queue = region.queue(device);
    power::on_for_board_read(&mut io::TimedIo::new(device, 1_000_000)).unwrap();
    mac::prepare_download(device).unwrap();
    let prepared = pcie::prepare(device).unwrap();
    queue.install(device).unwrap(); prepared.finish(device, &queue).unwrap();
    assert_eq!(device.command, 2);
    assert_eq!(device.get(R_AX_CMAC_FUNC_EN, 4), 0x7000803f);
    assert_eq!(device.get(fwcmd::DMA_STOP, 4) & 0x001f_0f03, 0x001f_0f03);
    device.command = 7;
    startup::command_dma(device).unwrap();
    unsafe { queue.start(device) }.unwrap();
    queue
}

pub fn power_off(device: &mut Device) {
    device.command = 2;
    power::off(&mut io::TimedIo::new(device, 1_000_000), Some(1)).unwrap();
    assert_eq!(device.get(R_AX_IC_PWR_STATE, 4) & B_AX_WLMAC_PWR_STE_MASK, 0);
    assert_eq!(device.get(R_AX_SYS_FUNC_EN, 1) & 3, 0);
    device.command = 0;
}
