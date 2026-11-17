//! `netpolicy add`'s note on a boot service's line (801-APP-0143, 801's S5): `init` takes a boot service's flow grant
//! once a boot and keeps it for every restart, so nothing renews its term or volume (MC-3.13, docs/network/policy.md).
//! A line that gives one a finite term or volume (3600 s and 16 MiB when it names none) ends its network until the next
//! boot; the question before the line is added says so, and names the line that lasts. No system calls:
//! tests/shell_host.rs.
use alloc::string::String;
use alloc::vec::Vec;

/// The longest term and the largest volume a line can give (docs/network/policy.md, "Term and volume").
pub const TERM_MAX: u64 = 4_294_967_295;
pub const VOLUME_MAX: u64 = 18_446_744_073_709_551_615;
const TERM_DEFAULT: u64 = 3600;
const VOLUME_DEFAULT: u64 = 16 << 20;

/// The program a flow line names, the words before its term, and its term and volume; None for a line that grants no flow
/// (`default card N`, `card N off`) or does not parse, which the broker answers itself.
fn grant(line: &str) -> Option<(&str, Vec<&str>, u64, u64)> {
    let words: Vec<&str> = line.split_whitespace().collect();
    // The words that come before the term: `program address proto [port]`, `browse program public tcp PORTS`,
    // `listen program tcp PORT from NETS`.
    let (program, fixed, port_optional) = match words.first()? {
        &"browse" if words.len() >= 5 => (words[1], 5, false),
        &"listen" if words.len() >= 6 => (words[1], 6, false),
        &"browse" | &"listen" | &"default" | &"card" => return None,
        _ if words.len() >= 3 => (words[0], 3, true),
        _ => return None,
    };
    let mut head: Vec<&str> = words[..fixed].to_vec();
    let mut rest = &words[fixed..];
    if port_optional {
        match rest.first() { Some(port) => { head.push(port); rest = &rest[1..]; } None => head.push("0") }
    }
    let number = |i: usize, default: u64| rest.get(i).map_or(Some(default), |w| w.parse().ok());
    Some((program, head, number(0, TERM_DEFAULT)?, number(1, VOLUME_DEFAULT)?))
}

/// The note added to `netpolicy add`'s question when `line` gives one of `services` (the boot services) a term or a
/// volume short of the longest; None for every other line.
pub fn note(line: &str, services: &[&str]) -> Option<String> {
    let (program, head, term, volume) = grant(line)?;
    if !services.contains(&program) || (term == TERM_MAX && volume == VOLUME_MAX) { return None; }
    Some(alloc::format!(
        "NOTE: {} IS A BOOT SERVICE: INIT TAKES ITS GRANT ONCE A BOOT AND NOTHING RENEWS IT, SO THIS LINE ENDS ITS NETWORK AFTER {} S OR {} BYTES, UNTIL THE NEXT BOOT. \
         THE LINE THAT LASTS AS LONG AS THE MACHINE RUNS: NETPOLICY ADD {} {} {}",
        program.to_ascii_uppercase(), term, volume, head.join(" "), TERM_MAX, VOLUME_MAX))
}
