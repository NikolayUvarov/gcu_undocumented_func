// Which file server a volume's calls go to (311-KRN-0190); plain, so the host tests build it alone.

/// The volumes vfs_server holds: the boot disk (`""`), `ram`, `log` and `models`. Every other volume is storefs's, asked
/// through SLOT_STOREFS when the program holds a client there, and of nobody when it does not.
pub const VFS_VOLUMES: [&str; 4] = ["", "ram", "log", "models"];

/// Whether vfs_server holds `volume`, its case ignored; storefs refuses these names.
pub fn on_vfs(volume: &str) -> bool { VFS_VOLUMES.iter().any(|v| v.eq_ignore_ascii_case(volume)) }

/// The slot `volume`'s calls go to: `lent`, a client a launcher lent, for every volume when it is not `vfs`'s slot; else
/// `vfs` for vfs_server's volumes, and `store` for the others when the program holds storefs's client. None: nobody.
pub fn route(volume: &str, lent: usize, vfs: usize, store: Option<usize>) -> Option<usize> {
    if lent != vfs || on_vfs(volume) { Some(lent) } else { store }
}
