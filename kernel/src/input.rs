// Bounded byte queue (input, logs, console output); the oldest byte is dropped when full.
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

// Bounded queue of input event words; the oldest event is dropped when full.
pub struct Events<const N: usize> { words: [usize; N], head: usize, len: usize }
impl<const N: usize> Events<N> {
    pub const fn new() -> Self { Self { words: [0; N], head: 0, len: 0 } }
    pub fn push(&mut self, event: usize) {
        if event == 0 { return; }
        if self.len == N { self.head = (self.head + 1) % N; self.len -= 1; }
        self.words[(self.head + self.len) % N] = event; self.len += 1;
    }
    pub fn pop(&mut self) -> Option<usize> {
        if self.len == 0 { return None; }
        let event = self.words[self.head]; self.head = (self.head + 1) % N; self.len -= 1; Some(event)
    }
    pub fn clear(&mut self) { self.head = 0; self.len = 0; }
    pub fn is_empty(&self) -> bool { self.len == 0 }
}
