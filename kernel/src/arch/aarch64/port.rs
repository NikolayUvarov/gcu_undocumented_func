// aarch64 has no I/O ports: no port capability is ever granted here, so these are never reached with one.
pub unsafe fn read(_port: u16, _width: usize) -> usize { 0 }
pub unsafe fn write(_port: u16, _width: usize, _value: usize) {}
