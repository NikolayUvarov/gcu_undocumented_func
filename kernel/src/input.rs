use crate::{inb, serial_has_data, serial_read_byte};

pub struct Key {
    pub app: u8,
    pub shell: u8,
    pub background: bool,
}

pub struct Keyboard {
    shift: bool,
    control: bool,
    extended: bool,
}

impl Keyboard {
    pub const fn new() -> Self {
        Self {
            shift: false,
            control: false,
            extended: false,
        }
    }

    pub fn read(&mut self) -> Option<Key> {
        unsafe {
            if serial_has_data() {
                let byte = serial_read_byte();
                return Some(Key {
                    app: byte,
                    shell: match byte {
                        b'\r' => b'\n',
                        127 => 8,
                        _ => byte,
                    },
                    background: byte == 26,
                });
            }
            let status = inb(0x64);
            if status & 1 == 0 {
                return None;
            }
            let code = inb(0x60);
            // Discard mouse bytes, PS/2 prefixes and modifier make/break codes.
            if status & 0x20 != 0 {
                return Some(Key {
                    app: 0,
                    shell: 0,
                    background: false,
                });
            }
            Some(self.scancode(code))
        }
    }

    fn scancode(&mut self, code: u8) -> Key {
        let mut key = Key {
            app: 0,
            shell: 0,
            background: false,
        };
        if code == 0xe0 || code == 0xe1 {
            self.extended = true;
            return key;
        }
        if code & 0x7f == 0x1d {
            self.control = code & 0x80 == 0;
            self.extended = false;
            return key;
        }
        if self.extended {
            self.extended = false;
            return key;
        }
        if code & 0x7f == 0x2a || code & 0x7f == 0x36 {
            self.shift = code & 0x80 == 0;
            return key;
        }
        if code & 0x80 != 0 {
            return key;
        }
        if self.control && code == 0x2c {
            key.background = true;
            return key;
        }
        const NORMAL: &[u8] =
            b"\0\x1b1234567890-=\x08\tqwertyuiop[]\n\0asdfghjkl;'`\0\\zxcvbnm,./\0*\0 ";
        const SHIFT: &[u8] =
            b"\0\x1b!@#$%^&*()_+\x08\tQWERTYUIOP{}\n\0ASDFGHJKL:\"~\0|ZXCVBNM<>?\0*\0 ";
        key.app = code;
        key.shell = if self.shift { SHIFT } else { NORMAL }
            .get(code as usize)
            .copied()
            .unwrap_or(0);
        key
    }
}

pub struct Queue<const N: usize> {
    bytes: [u8; N],
    head: usize,
    len: usize,
}
impl<const N: usize> Queue<N> {
    pub const fn new() -> Self {
        Self {
            bytes: [0; N],
            head: 0,
            len: 0,
        }
    }
    pub fn push(&mut self, value: u8) {
        if value == 0 {
            return;
        }
        if self.len == N {
            self.head = (self.head + 1) % N;
            self.len -= 1;
        }
        self.bytes[(self.head + self.len) % N] = value;
        self.len += 1;
    }
    pub fn pop(&mut self) -> Option<u8> {
        if self.len == 0 {
            return None;
        }
        let value = self.bytes[self.head];
        self.head = (self.head + 1) % N;
        self.len -= 1;
        Some(value)
    }
    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}
