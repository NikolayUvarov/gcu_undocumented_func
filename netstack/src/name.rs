// The machine's name in DHCP (000-NET-0050): option 12 (Host Name, RFC 2132) in each DISCOVER and REQUEST, read from
// data/machine/name through init's read-only scope of that directory alone; `mindcore` without a name.

use alloc::boxed::Box;
use alloc::string::String;
use mind::hostname::{self, Source};
use smoltcp::wire::DhcpOption;

// The scope's slot: slot 3, where mind::fs looks by default, is the second card's (SLOT_DEV1).
pub const SLOT_NAME: usize = mind::abi::SLOT_FILE;
// How often the file is read again: a changed name goes out with the next renewal or new lease.
pub const EVERY_MS: u64 = 60_000;
// A vfs_server that has not answered by then leaves the name as it was: the stack does not wait on files.
const READ_MS: u32 = 500;
const HOST_NAME: u8 = 12;

pub struct Name { text: String, options: &'static [DhcpOption<'static>], pub read: u64 }

impl Name {
    pub fn new() -> Self {
        mind::fs::use_endpoint(mind::ipc::Endpoint(SLOT_NAME));
        Self { text: String::new(), options: &[], read: 0 }
    }

    /// The options every DHCP socket sends (none before the first read).
    pub fn options(&self) -> &'static [DhcpOption<'static>] { self.options }

    /// Reads the file again; true when the name changed, which it logs. A read that failed keeps the name as it was.
    pub fn refresh(&mut self, now: u64) -> bool {
        self.read = now;
        let (text, source) = read();
        if matches!(source, Source::Unread(_)) && !self.text.is_empty() { return false; }
        if text == self.text { return false; }
        match source {
            Source::File => mind::println!("[NETSTACK] NAME FOR DHCP: {} ({})", text, hostname::FILE),
            Source::Missing => mind::println!("[NETSTACK] NAME FOR DHCP: {} (NO {})", text, hostname::FILE),
            Source::Invalid(why) => mind::println!("[NETSTACK] NAME FOR DHCP: {} ({} HOLDS NO NAME: {})", text, hostname::FILE, why),
            Source::Unread(why) => mind::println!("[NETSTACK] NAME FOR DHCP: {} ({} NOT READ: {})", text, hostname::FILE, why),
        }
        // smoltcp keeps the options by reference for the socket's life: each new name's few bytes stay allocated.
        let data: &'static [u8] = Box::leak(text.as_bytes().to_vec().into_boxed_slice());
        self.options = Box::leak(Box::new([DhcpOption { kind: HOST_NAME, data }]));
        self.text = text;
        true
    }
}

fn read() -> (String, Source) {
    let mut bytes = [0u8; 2 * hostname::MAX];
    let got = mind::idl::wire::with_timeout(READ_MS, || mind::fs::File::open(hostname::NAME).and_then(|mut file| file.read(&mut bytes)));
    match got {
        Ok(n) => hostname::read(Some(&bytes[..n])),
        Err(mind::fs::Error::NotFound) => hostname::read(None),
        Err(_) => (String::from(hostname::DEFAULT), Source::Unread("NO SCOPE OF data/machine/, OR A READ THAT FAILED")),
    }
}
