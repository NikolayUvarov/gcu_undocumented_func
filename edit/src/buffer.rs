//! The text of the editor: a piece table over the original file (never changed) and an append buffer, with the start
//! of every line kept up to date, and undo/redo of grouped changes. Bytes are kept as they are: line endings (LF or
//! CRLF) and invalid UTF-8 survive a save byte for byte.
use alloc::vec;
use alloc::vec::Vec;

/// The largest text the editor opens (the per-task heap is 16 MiB).
pub const LIMIT: usize = 8 << 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source { Original, Added }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Piece { source: Source, start: usize, len: usize }

/// One change: bytes inserted or deleted at a position.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Change { Insert { at: usize, bytes: Vec<u8> }, Delete { at: usize, bytes: Vec<u8> } }

/// A group of changes undone together, with the cursor before and after.
#[derive(Clone, Debug, Default)]
struct Group { changes: Vec<Change>, before: usize, after: usize, typing: bool }

pub struct Buffer {
    original: Vec<u8>,
    added: Vec<u8>,
    pieces: Vec<Piece>,
    len: usize,
    lines: Vec<usize>, // byte offset where each line starts; lines[0] == 0
    undo: Vec<Group>,
    redo: Vec<Group>,
    saved: usize, // undo depth at the last save (usize::MAX: never matches)
    open_group: bool,
}

impl Buffer {
    pub fn new(original: Vec<u8>) -> Self {
        let len = original.len();
        let pieces = if len > 0 { vec![Piece { source: Source::Original, start: 0, len }] } else { Vec::new() };
        let mut lines = vec![0];
        lines.extend(original.iter().enumerate().filter(|(_, &b)| b == b'\n').map(|(i, _)| i + 1));
        Self { original, added: Vec::new(), pieces, len, lines, undo: Vec::new(), redo: Vec::new(), saved: 0, open_group: false }
    }

    pub fn len(&self) -> usize { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
    pub fn line_count(&self) -> usize { self.lines.len() }
    /// Byte offset of the start of line `line` (0-based; clamped).
    pub fn line_start(&self, line: usize) -> usize { self.lines[line.min(self.lines.len() - 1)] }
    /// Byte offset of the end of line `line`, before its line ending.
    pub fn line_end(&self, line: usize) -> usize {
        let next = if line + 1 < self.lines.len() { self.lines[line + 1] - 1 } else { return self.len };
        if next > self.line_start(line) && self.byte(next - 1) == Some(b'\r') { next - 1 } else { next }
    }
    /// The line that holds byte offset `at`.
    pub fn line_of(&self, at: usize) -> usize { self.lines.partition_point(|&start| start <= at) - 1 }

    fn slice(&self, piece: &Piece) -> &[u8] {
        let data = match piece.source { Source::Original => &self.original, Source::Added => &self.added };
        &data[piece.start..piece.start + piece.len]
    }

    pub fn byte(&self, at: usize) -> Option<u8> {
        let mut offset = 0;
        for piece in &self.pieces {
            if at < offset + piece.len { return Some(self.slice(piece)[at - offset]); }
            offset += piece.len;
        }
        None
    }

    /// Bytes `start..end` copied out.
    pub fn bytes(&self, start: usize, end: usize) -> Vec<u8> {
        let end = end.min(self.len);
        let mut out = Vec::with_capacity(end.saturating_sub(start));
        let mut offset = 0;
        for piece in &self.pieces {
            let (a, b) = (offset, offset + piece.len);
            if b > start && a < end {
                let from = start.max(a) - a; let to = end.min(b) - a;
                out.extend_from_slice(&self.slice(piece)[from..to]);
            }
            offset = b;
            if offset >= end { break; }
        }
        out
    }

    /// The whole text.
    pub fn text(&self) -> Vec<u8> { self.bytes(0, self.len) }

    /// Copies bytes from `at` into `out`; returns how many (for the viewer).
    pub fn read(&self, at: usize, out: &mut [u8]) -> usize {
        let data = self.bytes(at, at + out.len());
        out[..data.len()].copy_from_slice(&data);
        data.len()
    }

    // Index of the piece holding `at` and the offset inside it, splitting a piece so a boundary falls at `at`.
    fn split(&mut self, at: usize) -> usize {
        let mut offset = 0;
        for i in 0..self.pieces.len() {
            let piece = self.pieces[i];
            if at == offset { return i; }
            if at < offset + piece.len {
                let inside = at - offset;
                self.pieces[i].len = inside;
                self.pieces.insert(i + 1, Piece { source: piece.source, start: piece.start + inside, len: piece.len - inside });
                return i + 1;
            }
            offset += piece.len;
        }
        self.pieces.len()
    }

    fn raw_insert(&mut self, at: usize, data: &[u8]) {
        if data.is_empty() { return; }
        let at = at.min(self.len);
        let start = self.added.len();
        self.added.extend_from_slice(data);
        let index = self.split(at);
        // Typing at the end of the last added piece extends it.
        if index > 0 && self.pieces[index - 1].source == Source::Added && self.pieces[index - 1].start + self.pieces[index - 1].len == start {
            self.pieces[index - 1].len += data.len();
        } else {
            self.pieces.insert(index, Piece { source: Source::Added, start, len: data.len() });
        }
        self.len += data.len();
        let line = self.line_of(at);
        for start in &mut self.lines[line + 1..] { *start += data.len(); }
        let new: Vec<usize> = data.iter().enumerate().filter(|(_, &b)| b == b'\n').map(|(i, _)| at + i + 1).collect();
        let tail = self.lines.split_off(line + 1);
        self.lines.extend(new);
        self.lines.extend(tail);
    }

    fn raw_delete(&mut self, start: usize, end: usize) -> Vec<u8> {
        let end = end.min(self.len);
        if start >= end { return Vec::new(); }
        let removed = self.bytes(start, end);
        let first = self.split(start);
        let last = self.split(end);
        self.pieces.drain(first..last);
        self.len -= end - start;
        let n = end - start;
        self.lines.retain(|&s| s <= start || s > end);
        for s in &mut self.lines { if *s > end { *s -= n; } }
        removed
    }

    fn record(&mut self, change: Change, cursor_before: usize, cursor_after: usize, typing: bool) {
        self.redo.clear();
        let merge = typing && self.open_group && self.undo.last().is_some_and(|g| g.typing);
        if merge {
            let group = self.undo.last_mut().unwrap();
            group.changes.push(change);
            group.after = cursor_after;
        } else {
            if self.saved > self.undo.len() { self.saved = usize::MAX; }
            self.undo.push(Group { changes: vec![change], before: cursor_before, after: cursor_after, typing });
        }
        self.open_group = typing;
    }

    /// Ends the current typing group: the next change is undone separately.
    pub fn close_group(&mut self) { self.open_group = false; }

    /// Inserts `data` at `at`; `typing` groups consecutive keys for undo.
    pub fn insert(&mut self, at: usize, data: &[u8], typing: bool) {
        if data.is_empty() || self.len + data.len() > LIMIT { return; }
        self.raw_insert(at, data);
        self.record(Change::Insert { at, bytes: data.to_vec() }, at, at + data.len(), typing);
    }

    /// Deletes `start..end`; returns the deleted bytes.
    pub fn delete(&mut self, start: usize, end: usize, typing: bool) -> Vec<u8> {
        let removed = self.raw_delete(start, end);
        if !removed.is_empty() { self.record(Change::Delete { at: start, bytes: removed.clone() }, end, start, typing); }
        removed
    }

    /// Replaces `start..end` with `data` as one change for undo.
    pub fn replace(&mut self, start: usize, end: usize, data: &[u8]) {
        let end = end.min(self.len);
        if start > end || self.len - (end - start) + data.len() > LIMIT { return; }
        self.close_group();
        let removed = self.raw_delete(start, end);
        self.raw_insert(start, data);
        self.redo.clear();
        if self.saved > self.undo.len() { self.saved = usize::MAX; }
        let mut changes = Vec::new();
        if !removed.is_empty() { changes.push(Change::Delete { at: start, bytes: removed }); }
        if !data.is_empty() { changes.push(Change::Insert { at: start, bytes: data.to_vec() }); }
        self.undo.push(Group { changes, before: end, after: start + data.len(), typing: false });
    }

    /// Several replacements as one change for undo (replace all): (start, end, new bytes), in increasing order.
    pub fn replace_all(&mut self, ranges: &[(usize, usize)], data: &[u8]) {
        let removed: usize = ranges.iter().map(|&(start, end)| end - start).sum();
        if ranges.is_empty() || self.len - removed + ranges.len() * data.len() > LIMIT { return; }
        self.close_group();
        let mut changes = Vec::new();
        // From the end, so earlier positions stay valid.
        for &(start, end) in ranges.iter().rev() {
            let removed = self.raw_delete(start, end);
            self.raw_insert(start, data);
            changes.push(Change::Delete { at: start, bytes: removed });
            changes.push(Change::Insert { at: start, bytes: data.to_vec() });
        }
        self.redo.clear();
        if self.saved > self.undo.len() { self.saved = usize::MAX; }
        let first = ranges[0].0;
        self.undo.push(Group { changes, before: first, after: first, typing: false });
    }

    fn apply(&mut self, change: &Change, forward: bool) {
        match (change, forward) {
            (Change::Insert { at, bytes }, true) | (Change::Delete { at, bytes }, false) => self.raw_insert(*at, bytes),
            (Change::Insert { at, bytes }, false) | (Change::Delete { at, bytes }, true) => { self.raw_delete(*at, at + bytes.len()); }
        }
    }

    /// Undoes the last group; returns where the cursor goes.
    pub fn undo(&mut self) -> Option<usize> {
        self.close_group();
        let group = self.undo.pop()?;
        for change in group.changes.iter().rev() { self.apply(change, false); }
        let cursor = group.before;
        self.redo.push(group);
        Some(cursor)
    }

    /// Redoes the last undone group; returns where the cursor goes.
    pub fn redo(&mut self) -> Option<usize> {
        let group = self.redo.pop()?;
        for change in &group.changes { self.apply(change, true); }
        let cursor = group.after;
        self.undo.push(group);
        Some(cursor)
    }

    /// The text differs from what was last saved (or loaded).
    pub fn modified(&self) -> bool { self.saved != self.undo.len() }
    pub fn mark_saved(&mut self) { self.close_group(); self.saved = self.undo.len(); }

    /// The line ending the text uses: CRLF if its first line ends so.
    pub fn crlf(&self) -> bool { self.lines.len() > 1 && self.lines[1] >= 2 && self.byte(self.lines[1] - 2) == Some(b'\r') }

    /// The character at `at` and its length; an invalid byte is U+FFFD of length 1.
    pub fn char_at(&self, at: usize) -> Option<(char, usize)> {
        if at >= self.len { return None; }
        let bytes = self.bytes(at, at + 4);
        Some(decode(&bytes, 0))
    }

    /// The start of the character before `at` (an invalid byte is a character of its own).
    pub fn previous(&self, at: usize) -> usize {
        if at == 0 { return 0; }
        for back in (2..=4.min(at)).rev() {
            let bytes = self.bytes(at - back, at);
            if core::str::from_utf8(&bytes).is_ok_and(|s| s.chars().count() == 1) { return at - back; }
        }
        at - 1
    }

    /// The start of the character after the one at `at`.
    pub fn next(&self, at: usize) -> usize { self.char_at(at).map_or(self.len, |(_, len)| at + len) }

    /// Finds `needle` from `from` ignoring case (Unicode lower case), wrapping around; returns its byte range.
    pub fn find(&self, needle: &str, from: usize) -> Option<(usize, usize)> { find_in(&self.text(), needle, from, true) }

    /// Every match of `needle` (ignoring case), in order, not overlapping.
    pub fn find_all(&self, needle: &str) -> Vec<(usize, usize)> {
        let text = self.text();
        let mut out = Vec::new();
        let mut from = 0;
        while let Some((start, end)) = find_in(&text, needle, from, false) { out.push((start, end)); from = end; }
        out
    }
}

/// A character of `text` at `at` and its length; an invalid byte is U+FFFD of length 1.
pub fn decode(text: &[u8], at: usize) -> (char, usize) {
    let lead = text[at];
    let need = match lead { 0x00..=0x7F => return (lead as char, 1), 0xC2..=0xDF => 1, 0xE0..=0xEF => 2, 0xF0..=0xF4 => 3, _ => return ('\u{FFFD}', 1) };
    match text.get(at..at + need + 1).and_then(|s| core::str::from_utf8(s).ok()).and_then(|s| s.chars().next()) { Some(c) => (c, need + 1), None => ('\u{FFFD}', 1) }
}

fn lower(c: char) -> char { c.to_lowercase().next().unwrap_or(c) }

fn find_in(text: &[u8], needle: &str, from: usize, wrap: bool) -> Option<(usize, usize)> {
    if needle.is_empty() { return None; }
    let needle: Vec<char> = needle.chars().map(lower).collect();
    let try_at = |start: usize| -> Option<usize> {
        let mut at = start;
        for &want in &needle {
            if at >= text.len() { return None; }
            let (c, n) = decode(text, at);
            if lower(c) != want { return None; }
            at += n;
        }
        Some(at)
    };
    let from = from.min(text.len());
    let wrapped = if wrap { 0..from } else { 0..0 };
    (from..text.len()).chain(wrapped).filter(|&i| text[i] & 0xC0 != 0x80).find_map(|i| try_at(i).map(|end| (i, end)))
}
