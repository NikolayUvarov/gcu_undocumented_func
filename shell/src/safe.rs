//! A safe start told to the user (000-APP-0086): init's lines in the boot's log say that the last run did not end as
//! asked and which services it left out, prepared for `svc start`; the shell shows them above its first prompt and
//! tells its window manager (shell.wit 1.6). No system calls: tests/shell_host.rs.
use alloc::string::String;
use alloc::vec::Vec;

const STARTED: &str = "[INIT] SAFE START:";
const PREPARED: &str = " LEFT OUT BY THE SAFE START, PREPARED";

/// What the boot's log said of a safe start: whether there was one, and the services left out that `svc start` starts.
#[derive(Default)]
pub struct Safe { pub started: bool, pub left_out: Vec<String> }

impl Safe {
    /// Takes one line of the system log.
    pub fn line(&mut self, text: &str) {
        if text.contains(STARTED) { self.started = true; return; }
        let Some(head) = text.find(PREPARED).map(|end| &text[..end]) else { return };
        let Some(name) = head.rsplit("[INIT] ").next().filter(|n| !n.is_empty() && !n.contains(' ')) else { return };
        if !self.left_out.iter().any(|n| n == name) { self.left_out.push(String::from(name)); }
    }

    /// The notice to show, None after a normal boot.
    pub fn notice(&self) -> Option<String> {
        if !self.started { return None; }
        let mut text = String::from("SAFE START: THE LAST RUN DID NOT END AS ASKED");
        if !self.left_out.is_empty() {
            let starts: Vec<String> = self.left_out.iter().map(|n| alloc::format!("svc start {}", n)).collect();
            text.push_str(&alloc::format!(". LEFT OUT: {} ({})", self.left_out.join(", "), starts.join(", ")));
        }
        Some(text)
    }
}
