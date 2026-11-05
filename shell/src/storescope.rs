//! The system calls of a store volume's scope for a start (311-APP-0121): whether a volume the arguments name has been
//! written, asked of the block store, and a read-only scope of its root made through a root of its own. Neither uses the
//! shell's own roots, and storefs is asked nothing for a word that names no written volume, so such a word does not
//! hold a start while the store mounts (round 56dc414c, 2026-11-04).

use crate::named;
use alloc::string::String;
use mind::abi::{CAP_KIND_ENDPOINT, SLOT_BLOCKSTORE_READ, SLOT_STOREFS};
use mind::idl::{blockstore, vfs, wire};
use mind::ipc::Endpoint;

// How long a start waits for the block store's answer, in ms: while it mounts it answers nothing, and the start goes on
// without the scope.
const ASK_MS: u32 = 500;

/// The first store volume the words of `args` name that has been written, or None. The block store's name of each is
/// asked through the shell's read-only client, which reads names and makes nothing; once one is not answered within
/// ASK_MS, no other is asked.
pub fn named(args: &str) -> Option<String> {
    if mind::dev::cap_info(SLOT_BLOCKSTORE_READ).0 != CAP_KIND_ENDPOINT { return None; }
    for (_, volume) in named::store_paths(args, &mind::fs::on_vfs) {
        let name = named::store_name(volume);
        match wire::with_timeout(ASK_MS, || blockstore::resolve(Endpoint(SLOT_BLOCKSTORE_READ), &name)) {
            Ok(Ok(head)) if !head.root.as_slice().is_empty() => return Some(String::from(volume)),
            Ok(_) => {}
            Err(error) => {
                mind::println!("[SHELL] THE BLOCK STORE DID NOT ANSWER FOR {} IN {} MS ({:?}): THE PROGRAM STARTS WITHOUT A SCOPE OF IT", name, ASK_MS, error);
                return None;
            }
        }
    }
    None
}

/// A read-only scope of `volume`'s root in `receive`, from a root of its own that is closed after, so that the shell
/// keeps no more of storefs's trees (a client holds two) than its own commands opened; when storefs has no tree left
/// for it, from the shell's own root of that volume, if its commands opened one.
pub fn make(volume: &str, receive: usize) -> bool {
    let store = Endpoint(SLOT_STOREFS);
    match vfs::root(store, volume) {
        Ok(Ok(root)) => {
            let made = vfs::scope(store, root, false, receive) == Ok(Ok(()));
            let _ = vfs::close(store, root);
            made
        }
        Ok(Err(vfs::Error::Handles)) => mind::fs::Dir::root(volume).and_then(|root| root.scope(false, receive)).is_ok(),
        _ => false,
    }
}
