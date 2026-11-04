//! The store volume a program's arguments name (311-APP-0121): the shell lends a read-only scope of it, so the program
//! reads what the user named and no other store volume. No system calls: tests/shell_host.rs.

/// The volume of the first word of `args` that names a path on a volume `vfs` does not serve (1 to 16 bytes before
/// its `:`); a URL is not a path.
pub fn store_volume<'a>(args: &'a str, vfs: &dyn Fn(&str) -> bool) -> Option<&'a str> {
    args.split_whitespace().filter(|word| !word.contains("://")).find_map(|word| {
        let volume = &word[..word.find(':')?];
        (!volume.is_empty() && volume.len() <= 16 && !volume.contains('/') && !vfs(volume)).then_some(volume)
    })
}
