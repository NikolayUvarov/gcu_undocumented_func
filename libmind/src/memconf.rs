// data/memory.conf (262-KRN-0160, 0163, 0178): the owner's lines `<name> <MiB>`, `#` starting a comment. The loader reads
// it for a program at each run, init for the model service `nnd` at each start; a name it gives may start workers.

pub const FILE: &str = "data/memory.conf";
pub const MAX: usize = 2048; // what is read of it

// The MiB `text` gives `name`, at most 65 534 (the quota field's all-of-the-spawner's is one above); None without a line.
pub fn mib(text: &str, name: &str) -> Option<u16> {
    text.lines().find_map(|line| {
        let mut words = line.split('#').next()?.split_whitespace();
        (words.next()?.eq_ignore_ascii_case(name)).then(|| words.next()?.parse::<u32>().ok())?
    }).map(|mib| mib.min(0xFFFF - 1) as u16)
}
