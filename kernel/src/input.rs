use crate::{serial_has_data, serial_read_byte};

pub struct Key {
    pub app: u8,
    pub shell: u8,
    pub background: bool,
}

pub struct Keyboard;

impl Keyboard {
    pub const fn new() -> Self {
        Self
    }

    pub fn read_serial(&mut self) -> Option<Key> {
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
            None
        }
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
