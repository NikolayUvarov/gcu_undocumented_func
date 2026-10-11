// The task table (issue 171): chunks of CHUNK slots added as needed and dropped when empty at the end. A task never
// moves: the system call path holds raw pointers to tasks while the table may grow. A slot past the end reads and
// writes as an empty one: a slot kept across a shrink (a client waiting for a reply) finds no task, not a panic.
use alloc::boxed::Box;
use alloc::vec::Vec;

pub const CHUNK: usize = 32;

pub struct Table<T> { chunks: Vec<Box<[Option<T>]>>, none: Option<T>, spare: Option<T> }

impl<T> Table<T> {
    pub fn new() -> Self { let mut table = Self { chunks: Vec::new(), none: None, spare: None }; assert!(table.grow(), "task table"); table }
    pub fn grow(&mut self) -> bool {
        let mut chunk = Vec::new();
        if self.chunks.try_reserve(1).is_err() || chunk.try_reserve_exact(CHUNK).is_err() { return false; }
        for _ in 0..CHUNK { chunk.push(None); }
        self.chunks.push(chunk.into_boxed_slice()); true
    }
    // Drops empty chunks at the end, keeping the first.
    pub fn shrink(&mut self) { while self.chunks.len() > 1 && self.chunks.last().unwrap().iter().all(Option::is_none) { self.chunks.pop(); } }
    pub fn len(&self) -> usize { self.chunks.len() * CHUNK }
    pub fn iter(&self) -> impl Iterator<Item = &Option<T>> { self.chunks.iter().flat_map(|c| c.iter()) }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Option<T>> { self.chunks.iter_mut().flat_map(|c| c.iter_mut()) }
    pub fn ptr(&mut self, index: usize) -> *mut Option<T> { &mut self.chunks[index / CHUNK][index % CHUNK] }
}

impl<T> core::ops::Index<usize> for Table<T> {
    type Output = Option<T>;
    fn index(&self, index: usize) -> &Option<T> { self.chunks.get(index / CHUNK).map_or(&self.none, |c| &c[index % CHUNK]) }
}
impl<T> core::ops::IndexMut<usize> for Table<T> {
    // Past the end: an empty slot of its own, emptied again each time, so nothing written there stays.
    fn index_mut(&mut self, index: usize) -> &mut Option<T> {
        if index / CHUNK >= self.chunks.len() { self.spare = None; return &mut self.spare; }
        &mut self.chunks[index / CHUNK][index % CHUNK]
    }
}
