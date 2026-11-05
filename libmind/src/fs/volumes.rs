// Which file server a volume's calls go to (311-KRN-0190); plain, so the host tests build it alone.

/// The volumes vfs_server holds: the boot disk (`""`), `ram`, `log` and `models`. Every other volume is storefs's, asked
/// through SLOT_STOREFS when the program holds a client there, and of nobody when it does not.
pub const VFS_VOLUMES: [&str; 4] = ["", "ram", "log", "models"];

/// Whether vfs_server holds `volume`, its case ignored; storefs refuses these names.
pub fn on_vfs(volume: &str) -> bool { VFS_VOLUMES.iter().any(|v| v.eq_ignore_ascii_case(volume)) }

/// The slot `volume`'s calls go to. vfs_server's volumes: `lent`, a client a launcher lent (every lent client is
/// vfs_server's), or `vfs` when none was lent (`lent == vfs`). Any other: `store` when the program holds storefs's client,
/// else the lent client, else nobody (None) (311-KRN-0203, APP's request for 311-APP-0121).
pub fn route(volume: &str, lent: usize, vfs: usize, store: Option<usize>) -> Option<usize> {
    if on_vfs(volume) { Some(lent) } else { store.or((lent != vfs).then_some(lent)) }
}

/// A call through a volume's kept root, asked once more if it answers `invalid` (311-KRN-0207, STO's request): the
/// root may be an earlier instance's of its service, which restarted and knows none of its handles. `forget` drops the
/// kept root, so the next `call` opens it again; it says whether there was one. A second `invalid`, or one with no kept
/// root to drop, is returned as it is. Only a path is asked again, never an open file's handle.
pub fn again<T, E: PartialEq + Copy>(invalid: E, mut call: impl FnMut() -> Result<T, E>, mut forget: impl FnMut() -> bool) -> Result<T, E> {
    match call() {
        Err(e) if e == invalid && forget() => call(),
        other => other,
    }
}
