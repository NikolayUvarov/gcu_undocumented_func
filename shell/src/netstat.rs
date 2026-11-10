//! `netstat` (000-APP-0031): the stack's open sockets as a table, one row each, and the program each belongs to. No
//! system calls: tests/shell_host.rs.
use alloc::format;
use alloc::string::String;

/// One socket as the table shows it.
pub struct Row<'a> { pub protocol: &'a str, pub local_port: u16, pub remote: Option<(u32, u16)>, pub state: &'a str, pub sent: u64, pub received: u64, pub program: &'a str, pub pid: u64 }

pub const HEADER: &str = "PROTO LOCAL REMOTE                STATE        SENT       RECEIVED   PROGRAM";

fn dotted(address: u32) -> String { let b = address.to_be_bytes(); format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3]) }

/// The row's line, its columns as `HEADER` has them.
pub fn line(row: &Row) -> String {
    let remote = row.remote.filter(|&(address, _)| address != 0).map_or(String::from("-"), |(address, port)| format!("{}:{}", dotted(address), port));
    format!("{:<5} {:<5} {:<21} {:<12} {:<10} {:<10} {} (PID {})", row.protocol, row.local_port, remote, row.state, row.sent, row.received, row.program, row.pid)
}

/// A TCP state as RFC 9293 writes it; `-` for a socket with none (UDP).
pub fn state(code: usize) -> &'static str {
    ["-", "CLOSED", "LISTEN", "SYN-SENT", "SYN-RECEIVED", "ESTABLISHED", "FIN-WAIT-1", "FIN-WAIT-2", "CLOSE-WAIT", "CLOSING", "LAST-ACK", "TIME-WAIT"].get(code).copied().unwrap_or("?")
}

/// What `ip` says of a card after its kind (550-APP-0085, 550-APP-0129): `DOWN`, `PREFERRED`, `DEFAULT` and `OFF`,
/// a card the policy took out of use (`socket.wit` 2.11 `withdrawn`), where they apply.
pub fn card_words(wireless: bool, down: bool, preferred: bool, default: bool, off: bool) -> String {
    let mut words = String::from(if wireless { " WI-FI" } else { " WIRED" });
    for (on, word) in [(down, " DOWN"), (preferred, " PREFERRED"), (default, " DEFAULT"), (off, " OFF")] {
        if on { words.push_str(word); }
    }
    words
}

/// Whether card `card` is out of use in `withdrawn`'s answer, a bit each.
pub fn off(withdrawn: u8, card: u8) -> bool { card < 8 && withdrawn >> card & 1 != 0 }
