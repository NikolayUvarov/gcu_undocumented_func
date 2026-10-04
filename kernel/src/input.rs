// Bounded queue (input events, logs, console output); the oldest element is dropped when full. Zero is never stored.
pub struct Queue<T: Copy + Default + PartialEq, const N: usize> {
    items: [T; N],
    head: usize,
    len: usize,
}
impl<T: Copy + Default + PartialEq, const N: usize> Queue<T, N> {
    pub fn new() -> Self {
        Self {
            items: [T::default(); N],
            head: 0,
            len: 0,
        }
    }
    pub fn push(&mut self, value: T) {
        if value == T::default() {
            return;
        }
        if self.len == N {
            self.head = (self.head + 1) % N;
            self.len -= 1;
        }
        self.items[(self.head + self.len) % N] = value;
        self.len += 1;
    }
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        let value = self.items[self.head];
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
