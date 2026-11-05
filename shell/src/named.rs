//! The store volume a program's arguments name (311-APP-0121): the shell lends a read-only scope of it, so the program
//! reads what the user named and no other store volume. No system calls: tests/shell_host.rs.

use alloc::string::String;

/// The longest store volume name, storefs's VOLUME_MAX.
pub const VOLUME_MAX: usize = 16;

/// The words of `args` that may name a path on a store volume, in order, each with its volume: a name storefs takes (1
/// to 16 bytes of `A-Z a-z 0-9 . _ -`) that `vfs` does not serve, and no other `:` after it, which a program's own word
/// has (`secret-put:token:file`, `ask:11.0.2.80:81:3`); a URL is not a path. The shell lends the first one written.
pub fn store_paths<'a>(args: &'a str, vfs: &'a dyn Fn(&str) -> bool) -> impl Iterator<Item = (&'a str, &'a str)> + 'a {
    args.split_whitespace().filter(|word| !word.contains("://")).filter_map(move |word| {
        let at = word.find(':')?;
        let (volume, rest) = (&word[..at], &word[at + 1..]);
        let named = !volume.is_empty() && volume.len() <= VOLUME_MAX && volume.bytes().all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c));
        (named && !rest.contains(':') && !vfs(volume)).then_some((word, volume))
    })
}

/// The block store's name of store volume `volume`, `fs/<volume>` in lower case, as storefs writes it: there once the
/// volume has been written.
pub fn store_name(volume: &str) -> String {
    let mut name = String::from("fs/");
    name.push_str(&volume.to_ascii_lowercase());
    name
}
