//! The clipboard's state (000-APP-0032, `idl/clipboard.wit` 1.0): one text, its generation, the program in front, a text
//! being set in parts; who may do what. The text is zeroed before it is let go. No system calls: tests/clipboard_host.rs.
use alloc::vec::Vec;

/// The longest text, in bytes.
pub const MAX: usize = 65536;
/// The longest part of one message, in bytes (`string<4000>`).
pub const PART: usize = 4000;

/// Who calls, by the badge of its client.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Who { Manager, Program, Other }

/// Why a call is refused (`idl/clipboard.wit` `error`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal { Behind, Refused, Invalid, Changed }

#[derive(Default)]
pub struct Clipboard {
    text: Vec<u8>,
    generation: u32,
    front: u64,
    // A text being set: its caller and the bytes so far.
    pending: Option<(u64, Vec<u8>)>,
}

impl Clipboard {
    pub fn new() -> Self { Self::default() }
    pub fn generation(&self) -> u32 { self.generation }
    pub fn length(&self) -> usize { self.text.len() }
    pub fn in_front(&self) -> u64 { self.front }

    // A manager always; a program only while it is the one in front; a client with neither badge never.
    fn may(&self, who: Who, pid: u64) -> Result<(), Refusal> {
        match who {
            Who::Manager => Ok(()),
            Who::Program if self.front != 0 && pid == self.front => Ok(()),
            Who::Program => Err(Refusal::Behind),
            Who::Other => Err(Refusal::Refused),
        }
    }

    /// A part of the text `pid` copies, from byte `at`: the text replaces the clipboard's with `last`, which gives its
    /// new generation; the others give 0.
    pub fn set_part(&mut self, who: Who, pid: u64, at: usize, part: &[u8], last: bool) -> Result<u32, Refusal> {
        self.may(who, pid)?;
        if at == 0 {
            if let Some((_, old)) = self.pending.as_mut() { wipe(old); }
            self.pending = Some((pid, Vec::new()));
        }
        // Only the next part of the caller's own text; one out of order, or past MAX, drops that text.
        let owned = matches!(&self.pending, Some((owner, _)) if *owner == pid);
        if !owned || !matches!(&self.pending, Some((_, bytes)) if bytes.len() == at && at + part.len() <= MAX) {
            if owned { if let Some((_, bytes)) = self.pending.as_mut() { wipe(bytes); } self.pending = None; }
            return Err(Refusal::Invalid);
        }
        let (_, bytes) = self.pending.as_mut().unwrap();
        bytes.extend_from_slice(part);
        if !last { return Ok(0); }
        let (_, new) = self.pending.take().unwrap();
        wipe(&mut self.text);
        self.text = new;
        self.generation = self.generation.wrapping_add(1).max(1);
        Ok(self.generation)
    }

    /// The part of the text from byte `at`, of `generation` (0: the one now): its generation, its whole length and
    /// the bytes, at most PART and ending where a character ends.
    pub fn get_part(&self, who: Who, pid: u64, generation: u32, at: usize) -> Result<(u32, usize, &[u8]), Refusal> {
        self.may(who, pid)?;
        if generation != 0 && generation != self.generation { return Err(Refusal::Changed); }
        if at > self.text.len() || !boundary(&self.text, at) { return Err(Refusal::Invalid); }
        let mut end = (at + PART).min(self.text.len());
        while !boundary(&self.text, end) { end -= 1; }
        Ok((self.generation, self.text.len(), &self.text[at..end]))
    }

    /// The generation, to any client the shell lent.
    pub fn generation_for(&self, who: Who) -> Result<u32, Refusal> {
        if who == Who::Other { Err(Refusal::Refused) } else { Ok(self.generation) }
    }

    /// The program in front, named by a manager.
    pub fn set_front(&mut self, who: Who, pid: u64) -> Result<(), Refusal> {
        if who != Who::Manager { return Err(Refusal::Refused); }
        self.front = pid;
        Ok(())
    }

    /// Whether `who` may have a program's client minted (`clipboard.wit` 1.1): a manager alone.
    pub fn program_client(&self, who: Who) -> Result<(), Refusal> {
        if who == Who::Manager { Ok(()) } else { Err(Refusal::Refused) }
    }
}

// Whether a character starts at `at` (or the text ends there).
fn boundary(text: &[u8], at: usize) -> bool { at >= text.len() || text[at] & 0xC0 != 0x80 }

/// Every byte zeroed, by writes the compiler keeps, then none kept.
pub fn wipe(bytes: &mut Vec<u8>) {
    for b in bytes.iter_mut() { unsafe { core::ptr::write_volatile(b, 0) } }
    bytes.clear();
}
