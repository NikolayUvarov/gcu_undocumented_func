//! PCI Command transitions preserve Status W1C flags and verify the requested decode/DMA state.

pub trait Access {
    fn read(&mut self) -> u32;
    fn write_command(&mut self, value: u16);
}

#[derive(Clone, Copy)]
pub enum Mode { Stopped, Registers, Dma { decode: u16 } }

pub fn transition(access: &mut impl Access, mode: Mode) -> bool {
    let bits = match mode { Mode::Stopped => 0, Mode::Registers => 2, Mode::Dma { decode } if decode != 0 && decode & !3 == 0 => decode | 4, Mode::Dma { .. } => return false };
    let original = access.read();
    if original == u32::MAX { return false; }
    access.write_command((original as u16 & !7) | bits);
    let after = access.read();
    after != u32::MAX && after as u16 & 7 == bits
}
