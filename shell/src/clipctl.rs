//! The system clipboard through the shell (000-APP-0032, `idl/clipboard.wit` 1.0). From `init`'s client in
//! SLOT_CLIPBOARD, which may badge, the shell mints its own copy badged `BADGE_MANAGER`, with which it names the program
//! in front of the console shown as that changes; a program that asks for REQUEST_CLIPBOARD gets a copy badged
//! `BADGE_PROGRAM`, and a window manager one that may badge, from which it mints its own and its windows' copies.
//! The shell's line pastes with Ctrl+V or Shift+Insert and copies itself with Ctrl+Insert.
use mind::abi::{CAP_BADGE, CAP_GRANT, CAP_KIND_ENDPOINT, CAP_WRITE, SLOT_CLIPBOARD};
use mind::clipboard::{self, Error, BADGE_MANAGER, BADGE_PROGRAM};
use mind::ipc::Endpoint;

/// The shell's manager copy, and the program it last named in front.
pub struct Clip { manager: Option<Endpoint>, front: Option<u64> }

impl Default for Clip { fn default() -> Self { Self::new() } }

impl Clip {
    /// The manager's copy, where `init` gave the shell the service's client; without one, no clipboard.
    pub fn new() -> Self {
        let held = mind::dev::cap_info(SLOT_CLIPBOARD).0 == CAP_KIND_ENDPOINT;
        let manager = held.then(|| mind::ipc::mint_badged(SLOT_CLIPBOARD, CAP_WRITE, BADGE_MANAGER).ok()).flatten().map(Endpoint);
        Self { manager, front: None }
    }

    /// `pid` is now the program in front (0: the shell itself); the service is told only of a change.
    pub fn front(&mut self, pid: u64) {
        let Some(manager) = self.manager else { return };
        if self.front == Some(pid) { return; }
        match clipboard::front(manager, pid) {
            Ok(()) => self.front = Some(pid),
            Err(error) => mind::println!("[SHELL] THE CLIPBOARD WAS NOT TOLD PID {} IS IN FRONT: {}", pid, error.text()),
        }
    }

    /// The text the shell copies from its line.
    pub fn copy(&self, text: &str) -> Result<u32, Error> { clipboard::copy_through(self.manager.ok_or(Error::NoClient)?, text) }

    /// The clipboard's text, for the shell's line.
    pub fn paste(&self) -> Result<alloc::string::String, Error> { clipboard::paste_through(self.manager.ok_or(Error::NoClient)?) }

    /// A copy to lend for REQUEST_CLIPBOARD: a program's, or for a window manager one that may badge. The caller drops
    /// it once lent.
    pub fn lend(&self, window_manager: bool) -> Option<usize> {
        if self.manager.is_none() { return None; }
        if window_manager { mind::ipc::mint(SLOT_CLIPBOARD, CAP_WRITE | CAP_GRANT | CAP_BADGE, 0, 0).ok() }
        else { mind::ipc::mint_badged(SLOT_CLIPBOARD, CAP_WRITE | CAP_GRANT, BADGE_PROGRAM).ok() }
    }
}

/// The first line of a pasted text, for the shell's line: up to its first line end, with no control characters.
pub fn line_of(text: &str) -> impl Iterator<Item = char> + '_ {
    text.split(['\n', '\r']).next().unwrap_or("").chars().filter(|c| !c.is_control())
}
