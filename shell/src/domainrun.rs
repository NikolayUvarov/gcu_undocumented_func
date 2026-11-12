//! `run -d <domain> <program>` (312-APP-0134): the program gets, in SLOT_BLOCKSTORE in place of the shell's client of
//! the volume, the client of the shell's memory domain `<domain>` the store mints (`blockstore.wit` 2.2 `domain-client`),
//! through the shell's client for REQUEST_BLOCKSTORE and through its read-only one for REQUEST_BLOCKSTORE_READ. No system
//! calls: tests/shell_host.rs.

/// The shell's own errors for a start refused before it begins; they never cross an endpoint.
pub const NOT_ASKED: usize = usize::MAX - 0x1100;
pub const NOT_LENT: usize = usize::MAX - 0x1101;
pub const NOT_FOUND: usize = usize::MAX - 0x1102;
pub const NOT_OURS: usize = usize::MAX - 0x1103;
pub const FULL: usize = usize::MAX - 0x1104;
pub const NO_CLIENT: usize = usize::MAX - 0x1105;
pub const SERVICE: usize = usize::MAX - 0x1106;

pub const USAGE: &str = "USAGE: RUN [-D <DOMAIN>] <NAME> [ARGUMENTS] [&]";

/// What `run`'s words say: no `-d`, or the domain's number and the words after it; `Err` for `-d` without a number or a program.
pub fn split(words: &str) -> Result<Option<(u16, &str)>, ()> {
    let words = words.trim_start();
    let Some(rest) = words.strip_prefix("-d").filter(|r| r.is_empty() || r.starts_with(char::is_whitespace)) else { return Ok(None) };
    let rest = rest.trim_start();
    let (number, program) = rest.split_once(char::is_whitespace).map_or((rest, ""), |(n, p)| (n, p.trim_start()));
    let id = number.parse::<u16>().map_err(|_| ())?;
    if program.is_empty() { return Err(()); }
    Ok(Some((id, program)))
}

/// The text of one of the errors above.
pub fn text(code: usize) -> Option<&'static str> {
    Some(match code {
        NOT_ASKED => "RUN -D: THE PROGRAM ASKS FOR NO BLOCK STORE, SO A MEMORY DOMAIN GIVES IT NOTHING",
        NOT_LENT => "RUN -D: NO BLOCK STORE CLIENT TO LEND IT (NO STORE RUNS, OR THE SCRIPT DOES NOT REQUIRE BLOCKSTORE)",
        NOT_FOUND => "RUN -D: NO LIVE MEMORY DOMAIN OF THE SHELL'S HAS THAT NUMBER (BLOCKS DOMAINS LISTS THEM)",
        NOT_OURS => "RUN -D: THAT MEMORY DOMAIN IS NOT THE SHELL'S",
        FULL => "RUN -D: THE STORE LENDS 64 DOMAIN CLIENTS AT MOST, ALL IN USE",
        NO_CLIENT => "RUN -D: THE BLOCK STORE GAVE NO CLIENT OF THAT DOMAIN",
        SERVICE => "RUN -D STARTS A PROGRAM, NOT A SERVICE",
        _ => return None,
    })
}
