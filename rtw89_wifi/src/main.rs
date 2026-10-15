#![no_std]
#![no_main]

use mind::abi::{BootInfo, CAP_KIND_MMIO, SLOT_DEV0};
use mind::dev::{cap_info, device_config, Mmio};
use rtw89_wifi::{board, io::{self, RegisterIo, Result}};

struct Native { bar: Mmio }

impl RegisterIo for Native {
    fn read8(&mut self, at: usize) -> Result<u8> { io::check_offset(at, 1, self.bar.len())?; Ok(self.bar.read8(at)) }
    fn read16(&mut self, at: usize) -> Result<u16> { io::check_offset(at, 2, self.bar.len())?; Ok(self.bar.read16(at)) }
    fn read32(&mut self, at: usize) -> Result<u32> { io::check_offset(at, 4, self.bar.len())?; Ok(self.bar.read32(at)) }
    fn write8(&mut self, at: usize, value: u8) -> Result<()> { io::check_offset(at, 1, self.bar.len())?; self.bar.write8(at, value); Ok(()) }
    fn write16(&mut self, at: usize, value: u16) -> Result<()> { io::check_offset(at, 2, self.bar.len())?; self.bar.write16(at, value); Ok(()) }
    fn write32(&mut self, at: usize, value: u32) -> Result<()> { io::check_offset(at, 4, self.bar.len())?; self.bar.write32(at, value); Ok(()) }
    fn now_us(&mut self) -> u64 { mind::time::monotonic_ns() / 1_000 }
    fn delay_us(&mut self, us: u32) -> Result<()> { io::short_delay(mind::time::monotonic_ns, us) }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::println!("[RTW89] INITIAL BOARD ACCESS ONLY: NO FIRMWARE, DMA OR WLAN RADIO");
    let (_, resolution, frequency) = mind::time::clock_info();
    if let Err(error) = io::validate_clock(resolution, frequency) {
        mind::println!("[RTW89] PRECISE CLOCK REQUIRED: {:?}", error);
        idle();
    }
    let (kind, _, len) = cap_info(SLOT_DEV0);
    if kind != CAP_KIND_MMIO { mind::println!("[RTW89] NO MMIO BAR2 GRANT"); idle(); }
    let values = [0, 4, 0x18, 0x1c].map(|offset| device_config(SLOT_DEV0, offset));
    let [Ok(id), Ok(command), Ok(low), Ok(high)] = values else { mind::println!("[RTW89] PCI CONFIG UNAVAILABLE"); idle(); };
    if let Err(error) = io::validate_grant(id, command as u16, low, high, len) {
        mind::println!("[RTW89] REJECTED GRANT {:?}; PCI {:08X}, COMMAND {:04X}", error, id, command as u16);
        idle();
    }
    let Ok(bar) = Mmio::map(SLOT_DEV0) else { mind::println!("[RTW89] BAR2 MAP FAILED"); idle(); };
    if bar.len() != len { mind::println!("[RTW89] BAR2 SIZE CHANGED"); idle(); }
    let mut native = Native { bar };
    match board::read(&mut native) {
        Ok(board) => {
            let b = board.identity;
            mind::println!("[RTW89] BOARD READ: CUT {}, RFE {}, XTAL {:02X}, COUNTRY {:02X}{:02X}, PLAN {:02X}", b.cut, b.rfe, b.xtal, b.country[0], b.country[1], b.channel_plan);
            mind::println!("[RTW89] MAC {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}; PHY CAP {} BYTES; MAC POWERED OFF", b.mac[0], b.mac[1], b.mac[2], b.mac[3], b.mac[4], b.mac[5], board.maps.phycap.len());
            mind::println!("[RTW89] CALIBRATION NOT APPLIED; NO NETWORK LINK");
        }
        Err(error) => mind::println!("[RTW89] BOARD ACCESS FAILED {:?}; NO NETWORK LINK", error),
    }
    idle()
}

fn idle() -> ! { loop { mind::time::sleep(60_000); } }
