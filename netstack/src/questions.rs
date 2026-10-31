//! Questions a grant asks for a TCP flow its rules do not name (socket.wit 2.8, 000-NET-0049). The stack keeps them for
//! the policy broker, which gives the user's answer through `decide`; the stack itself decides nothing.
use alloc::string::String;
use alloc::vec::Vec;

/// Questions that may wait at once, in all.
pub const WAITING: usize = 8;
// Answered questions kept for their grant's `asked`, the oldest dropped first.
const KEPT: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum State { Pending, Allowed(u32), Refused }

pub struct Question { pub id: u32, pub badge: u16, pub host: String, pub port: u16, pub why: String, pub state: State }

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Refusal { Invalid, Limit, NotFound }

#[derive(Default)]
pub struct Questions { list: Vec<Question>, next: u32 }

impl Questions {
    /// Grant `badge` asks for `host` on TCP `port`: its id, or why not (one question of a grant waits at a time).
    pub fn ask(&mut self, badge: u16, host: &str, port: u16, why: &str) -> Result<u32, Refusal> {
        if host.is_empty() || port == 0 { return Err(Refusal::Invalid); }
        let waiting = |q: &&Question| q.state == State::Pending;
        if self.list.iter().filter(waiting).count() >= WAITING || self.list.iter().filter(waiting).any(|q| q.badge == badge) { return Err(Refusal::Limit); }
        if self.list.len() >= WAITING + KEPT {
            if let Some(oldest) = self.list.iter().position(|q| q.state != State::Pending) { self.list.remove(oldest); }
        }
        self.next = self.next.wrapping_add(1).max(1);
        self.list.push(Question { id: self.next, badge, host: String::from(host), port, why: String::from(why), state: State::Pending });
        Ok(self.next)
    }

    /// Where question `id` of grant `badge` stands; another grant's is `NotFound` as an unknown one is.
    pub fn asked(&self, badge: u16, id: u32) -> Result<State, Refusal> {
        self.list.iter().find(|q| q.id == id && q.badge == badge).map(|q| q.state).ok_or(Refusal::NotFound)
    }

    /// The questions that wait, oldest first.
    pub fn waiting(&self) -> impl Iterator<Item = &Question> { self.list.iter().filter(|q| q.state == State::Pending) }

    /// The grant and the port of question `id`, while it waits.
    pub fn pending(&self, id: u32) -> Option<(u16, u16)> {
        self.list.iter().find(|q| q.id == id && q.state == State::Pending).map(|q| (q.badge, q.port))
    }

    /// The answer to question `id`, which must wait.
    pub fn settle(&mut self, id: u32, state: State) -> Result<(), Refusal> {
        let question = self.list.iter_mut().find(|q| q.id == id && q.state == State::Pending).ok_or(Refusal::NotFound)?;
        question.state = state;
        Ok(())
    }

    /// Grant `badge` is gone: its questions with it.
    pub fn forget(&mut self, badge: u16) { self.list.retain(|q| q.badge != badge); }
}
