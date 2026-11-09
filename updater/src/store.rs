// The block store through the updater's own client in SLOT_BLOCKSTORE (351-UPD-0033). `init` badges it
// `OWNER_UPDATER` with get, put and publish (351-KRN-0218), so its pins are the updater's alone. What it keeps there is
// kept.rs's.
use alloc::vec::Vec;
use mind::abi::{CAP_KIND_ENDPOINT, SLOT_BLOCKSTORE};
use mind::cid::{self, Cid, Codec};
use mind::dag::{self, Blocks};
use mind::idl::blockstore::{self, Durability};
use mind::ipc::Endpoint;

const STORE: Endpoint = Endpoint(SLOT_BLOCKSTORE);

/// The store, once it answers on a medium that outlives a reset.
pub struct Store;

// The store's error as the blocks' reader takes it.
fn store_error(e: blockstore::Error) -> dag::Error {
    match e {
        blockstore::Error::NotFound | blockstore::Error::Incomplete => dag::Error::NotFound,
        blockstore::Error::Corrupt => dag::Error::Corrupt,
        blockstore::Error::Full | blockstore::Error::Quota => dag::Error::Full,
        blockstore::Error::Invalid => dag::Error::Shape,
        _ => dag::Error::Store,
    }
}

// A call's answer, the store's own error kept; one that did not reach it is None.
fn answer<T>(result: mind::sys::Result<Result<T, blockstore::Error>>) -> Result<T, Option<blockstore::Error>> { result.map_err(|_| None)?.map_err(Some) }

fn blocks<T>(result: mind::sys::Result<Result<T, blockstore::Error>>) -> Result<T, dag::Error> { answer(result).map_err(|e| e.map_or(dag::Error::Store, store_error)) }

impl Blocks for Store {
    fn put(&mut self, codec: Codec, data: &[u8]) -> Result<Cid, dag::Error> {
        let codec = match codec { Codec::Raw => blockstore::Codec::Raw, Codec::DagCbor => blockstore::Codec::DagCbor };
        let mut out = [0u8; cid::BYTES];
        let n = blocks(blockstore::put(STORE, codec, data, &mut out))?;
        Cid::from_bytes(&out[..n]).map_err(|_| dag::Error::Store)
    }
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, dag::Error> { blocks(blockstore::get(STORE, &cid.to_bytes(), out)) }
    fn has(&mut self, cid: &Cid) -> Result<bool, dag::Error> { blocks(blockstore::has(STORE, &cid.to_bytes())) }
}

impl Store {
    /// The store, or why no release is kept there: no client, no answer, not mounted, or a RAM disk.
    pub fn open() -> Result<Self, &'static str> {
        if mind::dev::cap_info(SLOT_BLOCKSTORE).0 != CAP_KIND_ENDPOINT { return Err("THE UPDATER HOLDS NO CLIENT OF THE BLOCK STORE"); }
        match answer(blockstore::stat(STORE)) {
            Ok(stats) if stats.durability == Durability::Volatile => Err("THE BLOCK STORE'S MEDIUM IS A RAM DISK"),
            Ok(_) => Ok(Store),
            Err(Some(blockstore::Error::Key)) => Err("THE BLOCK STORE IS NOT MOUNTED"),
            Err(_) => Err("THE BLOCK STORE DOES NOT ANSWER"),
        }
    }

    /// The updater's pins: id, root and size.
    pub fn pins(&self) -> Result<Vec<(u32, Cid, u64)>, Option<blockstore::Error>> {
        let pins = answer(blockstore::pins(STORE))?;
        Ok(pins.as_slice().iter().filter_map(|p| Cid::from_bytes(p.root.as_slice()).ok().map(|root| (p.id, root, p.size))).collect())
    }

    /// Pins `root` for the updater; the pin's id.
    pub fn pin(&self, root: &Cid) -> Result<u32, Option<blockstore::Error>> { answer(blockstore::pin(STORE, &root.to_bytes())).map(|p| p.id) }

    /// Ends the updater's pin `id`.
    pub fn unpin(&self, id: u32) -> Result<(), Option<blockstore::Error>> { answer(blockstore::unpin(STORE, id)) }
}
