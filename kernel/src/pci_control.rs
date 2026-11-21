//! PCI Command transitions preserve Status W1C flags and verify the requested decode/DMA state.

pub trait Access {
    fn read(&mut self) -> u32;
    fn write_command(&mut self, value: u16);
}

#[derive(Clone, Copy)]
// Memory: memory decoding and mastering, I/O decoding left as found (a memory BAR's grant, 213-KRN-0271).
pub enum Mode { Stopped, Registers, Dma { decode: u16 }, Memory }

pub fn transition(access: &mut impl Access, mode: Mode) -> bool {
    if matches!(mode, Mode::Dma { decode } if decode == 0 || decode & !3 != 0) { return false; }
    let original = access.read();
    if original == u32::MAX { return false; }
    let bits = match mode { Mode::Stopped => 0, Mode::Registers => 2, Mode::Dma { decode } => decode | 4, Mode::Memory => original as u16 & 1 | 6 };
    access.write_command((original as u16 & !7) | bits);
    let after = access.read();
    after != u32::MAX && after as u16 & 7 == bits
}
