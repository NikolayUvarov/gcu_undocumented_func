// The Allwinner A31's watchdog registers (100004-PRT-0015), which the A64, the H5 and the H616 have, as the SoCs' user
// manuals name them; kept apart from the hardware, so tests/sunxi_watchdog_host.rs runs this against a model of them.
pub const CTRL: usize = 0x10;
pub const CFG: usize = 0x14;
pub const MODE: usize = 0x18;
pub const RESTART: u32 = 0xA57 << 1 | 1; // CTRL: the key and the restart bit
pub const SYSTEM: u32 = 0b01; // CFG bits 1:0: an expiry resets the whole system
pub const ENABLE: u32 = 1; // MODE bit 0
pub const SIXTEEN: u32 = 0xB << 4; // MODE bits 7:4: the interval, 16 s
pub const SECONDS: u32 = 16;

/// The watchdog's 32-bit registers, by their offset from its base.
pub trait Registers {
    fn read(&mut self, offset: usize) -> u32;
    fn write(&mut self, offset: usize, value: u32);
}

/// Sets it to reset the whole system 16 s after the last restart and starts it; else why not, and it is left stopped.
pub fn arm(r: &mut impl Registers) -> Result<(), &'static str> {
    r.write(MODE, SIXTEEN);
    r.write(CTRL, RESTART);
    let config = r.read(CFG) & !0b11 | SYSTEM;
    r.write(CFG, config);
    r.write(MODE, SIXTEEN | ENABLE);
    if r.read(CFG) & 0b11 != SYSTEM || r.read(MODE) & 0xF1 != SIXTEEN | ENABLE {
        r.write(MODE, 0);
        return Err("ITS REGISTERS DO NOT TAKE THE SETTING");
    }
    r.write(CTRL, RESTART);
    Ok(())
}

/// The count started again.
pub fn restart(r: &mut impl Registers) { r.write(CTRL, RESTART); }

/// The watchdog stopped.
pub fn disable(r: &mut impl Registers) { r.write(MODE, 0); }
