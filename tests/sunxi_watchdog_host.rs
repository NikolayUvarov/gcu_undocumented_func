//! The Allwinner A31's watchdog (100004-PRT-0015) against a model of its registers: armed to reset the whole system
//! (configured, then enabled with 16 s, then restarted), fed, stopped; a register that does not take the setting
//! refused and the watchdog left stopped; what else CFG holds kept.
#[allow(dead_code)]
#[path = "../kernel/src/arch/aarch64/sunxi_wdt.rs"]
mod sunxi_wdt;
use sunxi_wdt::*;

#[derive(Default)]
struct Model { ctrl: u32, cfg: u32, mode: u32, deaf: Option<usize>, writes: Vec<(usize, u32)> }

impl Registers for Model {
    fn read(&mut self, offset: usize) -> u32 { match offset { CTRL => self.ctrl, CFG => self.cfg, MODE => self.mode, _ => panic!("read {offset:#x}") } }
    fn write(&mut self, offset: usize, value: u32) {
        self.writes.push((offset, value));
        if self.deaf == Some(offset) { return; }
        match offset { CTRL => self.ctrl = value, CFG => self.cfg = value, MODE => self.mode = value, _ => panic!("write {offset:#x}") }
    }
}

#[test]
fn armed_fed_and_stopped() {
    // As U-Boot may leave it: an interrupt only, another interval, enabled; and bits of CFG this code does not set.
    let mut m = Model { cfg: 0xA500_0000 | 0b10, mode: 0x5 << 4 | 1, ..Model::default() };
    arm(&mut m).unwrap();
    assert_eq!(m.writes, [(MODE, 0xB0), (CTRL, 0x14AF), (CFG, 0xA500_0001), (MODE, 0xB1), (CTRL, 0x14AF)], "set while stopped, configured, then enabled and restarted");
    assert_eq!((m.cfg, m.mode), (0xA500_0001, 0xB1));
    m.writes.clear();
    restart(&mut m);
    assert_eq!(m.writes, [(CTRL, RESTART)]);
    disable(&mut m);
    assert_eq!(m.mode, 0);
    assert_eq!((RESTART, SYSTEM, SIXTEEN | ENABLE, SECONDS), (0xA57 << 1 | 1, 1, 0xB1, 16));
}

#[test]
fn a_register_that_does_not_take_the_setting() {
    for deaf in [CFG, MODE] {
        let mut m = Model { deaf: Some(deaf), ..Model::default() };
        assert_eq!(arm(&mut m), Err("ITS REGISTERS DO NOT TAKE THE SETTING"), "{deaf:#x}");
        assert_eq!(m.writes.last(), Some(&(MODE, 0)), "left stopped");
        assert!(!m.writes.iter().skip(2).any(|&w| w == (CTRL, RESTART)), "not restarted after the setting failed");
    }
}
