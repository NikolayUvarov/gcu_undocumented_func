//! The store volume a program's arguments name (311-APP-0121): the shell lends a read-only scope of it, so the program
//! reads what the user named and no other store volume. No system calls: tests/shell_host.rs.

/// The words of `args` that may name a path on a volume `vfs` does not serve (1 to 16 bytes before the `:`), in order,
/// each with its volume; a URL is not a path. The shell lends the volume of the first that names a path there.
pub fn store_paths<'a>(args: &'a str, vfs: &'a dyn Fn(&str) -> bool) -> impl Iterator<Item = (&'a str, &'a str)> + 'a {
    args.split_whitespace().filter(|word| !word.contains("://")).filter_map(move |word| {
        let volume = &word[..word.find(':')?];
        (!volume.is_empty() && volume.len() <= 16 && !volume.contains('/') && !vfs(volume)).then_some((word, volume))
    })
}
