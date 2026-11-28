// No I/O ports (215-PRT-0017).
pub unsafe fn read(_port: u16, _width: usize) -> usize { 0 }
pub unsafe fn write(_port: u16, _width: usize, _value: usize) {}
