#![no_std]
#![no_main]
// storefs (issue 311-STO-0032, docs/storage; MC-3.4, 4.1, 4.3, 4.9): `vfs.wit` 2.6 over the store's names `fs/<volume>`,
// each a directory tree, through fs.rs (its logic: tests/storefs_host.rs). A handle belongs to the client that opened it,
// by the PID and badge of its request. A files client asks vfs_server first and storefs for a volume vfs_server answers
// `not-found` (the kernel track's way (b), 2026-10-30), so the boot disk, `ram`, `log` and `models` are refused here.
// init starts it after blockstore and gives the shell a client badged BADGE_USER (311-KRN-0190); a scope
// (311-STO-0037) is a narrower client, of one directory, that the shell can lend. The state is in statics.
mod fs;

// fs.rs names these as crate::cid, crate::dag, crate::tree, crate::worktree and crate::vfs, so the host tests build it
// from the libmind files.
use mind::idl::vfs;
use mind::{cid, dag, tree, worktree};

use cid::{Cid, Codec};
use dag::{Blocks, CHUNK};
use fs::{Client, Error, Fs, Roots, CHUNKS, CHUNKS_EACH, FILES, FILES_EACH, FILE_MAX, HANDLES, HANDLES_EACH, PAGE, SCOPES, SCOPES_EACH, SCOPE_BADGE_FIRST, TREES, TREES_EACH};
use mind::abi::{BootInfo, CAP_GRANT, CAP_KIND_ENDPOINT, CAP_WRITE, SLOT_BLOCKSTORE, SLOT_SERVICE};
use mind::fs::{BADGE_USER, ENTRY_DIR};
use mind::idl::blockstore;
use mind::idl::codec::Text;
use mind::idl::vfs::Request;
use mind::idl::wire::{self, Call};
use mind::ipc::{self, Endpoint};
use tree::Kind;
use worktree::Names;

const RECEIVED: usize = 9;
/// The store's client, that may get, put and publish, in SLOT_BLOCKSTORE, where init grants it (311-KRN-0190).
const STORE: Endpoint = Endpoint(SLOT_BLOCKSTORE);

static mut FS: Fs = Fs::new();
static mut SCRATCH: [u8; vfs::REQUEST_MAX] = [0; vfs::REQUEST_MAX];
static mut DATA: [u8; CHUNK] = [0; CHUNK];

/// The store, through its client.
struct Remote;

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

// A call's answer; one that did not reach the store is Store.
fn answer<T, E>(result: Result<Result<T, blockstore::Error>, E>) -> Result<T, dag::Error> { result.map_err(|_| dag::Error::Store)?.map_err(store_error) }

impl Blocks for Remote {
    fn put(&mut self, codec: Codec, data: &[u8]) -> Result<Cid, dag::Error> {
        let codec = match codec { Codec::Raw => blockstore::Codec::Raw, Codec::DagCbor => blockstore::Codec::DagCbor };
        let mut out = [0u8; cid::BYTES];
        let n = answer(blockstore::put(STORE, codec, data, &mut out))?;
        Cid::from_bytes(&out[..n]).map_err(|_| dag::Error::Store)
    }
    fn get(&mut self, cid: &Cid, out: &mut [u8]) -> Result<usize, dag::Error> { answer(blockstore::get(STORE, &cid.to_bytes(), out)) }
    fn has(&mut self, cid: &Cid) -> Result<bool, dag::Error> { answer(blockstore::has(STORE, &cid.to_bytes())) }
}

impl Names for Remote {
    fn publish(&mut self, name: &str, expected: u64, root: &Cid) -> Result<Option<u64>, dag::Error> {
        match blockstore::publish(STORE, name, expected, &root.to_bytes()) {
            Ok(Err(blockstore::Error::Conflict)) => Ok(None),
            done => answer(done).map(|published| Some(published.version)),
        }
    }
    fn version(&mut self, name: &str) -> Result<u64, dag::Error> { self.current(name).map(|(version, _)| version) }
}

impl Roots for Remote {
    fn current(&mut self, name: &str) -> Result<(u64, Option<Cid>), dag::Error> {
        let name = Text::new(name).ok_or(dag::Error::Store)?;
        let heads = answer(blockstore::snapshot(STORE, &[name]))?;
        let head = heads.as_slice().first().ok_or(dag::Error::Store)?;
        Ok((head.version, Cid::from_bytes(head.root.as_slice()).ok()))
    }
}

// Whether a client may write a volume: the user's badge (the shell's client, 311-KRN-0190) writes every volume, every
// other badge reads. A scope's badge (from SCOPE_BADGE_FIRST) is never asked: fs.rs gives it what its scope says.
fn writable(badge: u16, _volume: &str) -> bool { badge == BADGE_USER }

// An entry as `vfs.wit` gives it: a directory's size is 0, as FAT's, and no time is kept.
fn entry(name: &[u8], kind: Kind, size: u64) -> vfs::Entry {
    let directory = kind == Kind::Directory;
    let name = core::str::from_utf8(name).ok().and_then(Text::new).unwrap_or_default();
    vfs::Entry { name, size: if directory { 0 } else { size.min(u32::MAX as u64) as u32 }, modified: 0, attributes: if directory { ENTRY_DIR } else { 0 }, directory }
}

// When a table is full, the scopes of ended tasks end and the handles of clients that have ended are closed,
// unpublished, as vfs_server frees them.
fn sweep(fs: &mut Fs) {
    let (handles, trees, files, _) = fs.held();
    if handles < HANDLES && trees < TREES && files < FILES { return; }
    end_scopes(fs);
    let gone = fs.forget(|pid| !mind::process::alive(pid));
    if gone > 0 { mind::println!("[STOREFS] {} HANDLES OF ENDED CLIENTS CLOSED", gone); }
}

// The scopes whose task has ended, or that nobody took up within a minute, end: the capability kept for each is
// revoked, so no copy of it reaches storefs again, and dropped.
fn end_scopes(fs: &mut Fs) {
    fs.sweep_scopes(mind::time::uptime_ms() as u64, mind::process::alive, |badge, cap| {
        let _ = ipc::revoke(cap);
        let _ = ipc::drop_cap(cap);
        mind::println!("[STOREFS] SCOPE {:#x} ENDED", badge);
    });
}

fn serve(fs: &mut Fs, store: &mut Remote, client: Client, request: Request, call: Call, data: &mut [u8; CHUNK]) -> mind::Result<()> {
    // A scope's client is served only while its scope lasts.
    if client.badge >= SCOPE_BADGE_FIRST { end_scopes(fs); }
    match request {
        Request::Root { name } => {
            sweep(fs);
            vfs::reply_root(call, fs.root(store, client, name.as_str(), writable).map_err(Error::wire))
        }
        Request::OpenDir { dir, path, create } => {
            sweep(fs);
            vfs::reply_open_dir(call, fs.open_dir(store, client, dir, path.as_str(), create).map_err(Error::wire))
        }
        Request::Open { dir, path, mode } => {
            sweep(fs);
            vfs::reply_open(call, fs.open(store, client, dir, path.as_str(), mode).map_err(Error::wire))
        }
        Request::Read { file, offset, length } => {
            let n = (length as usize).min(CHUNK);
            let read = fs.read(store, client, file, offset as u64, &mut data[..n]);
            vfs::reply_read(call, read.map(|n| &data[..n]).map_err(Error::wire))
        }
        // A file is FILE_MAX at most, within `vfs.wit`'s sizes.
        Request::Write { file, offset, data } => vfs::reply_write(call, fs.write(store, client, file, offset as u64, data).map(|n| n as u32).map_err(Error::wire)),
        Request::Truncate { file, size } => vfs::reply_truncate(call, fs.truncate(store, client, file, size as u64).map_err(Error::wire)),
        Request::Stat { handle } => {
            let stat = fs.stat(store, client, handle).map(|e| entry(e.name(), e.kind, e.size));
            vfs::reply_stat(call, stat.as_ref().map_err(|e| e.wire()))
        }
        Request::List { dir, start } => {
            let (mut page, mut n) = ([vfs::Entry::default(); PAGE], 0);
            let listed = fs.list(store, client, dir, start, |name, kind, size| { page[n] = entry(name, kind, size); n += 1; });
            vfs::reply_list(call, listed.map(|_| &page[..n]).map_err(Error::wire))
        }
        Request::Remove { dir, path } => vfs::reply_remove(call, fs.remove(store, client, dir, path.as_str()).map_err(Error::wire)),
        Request::Rename { dir, from, target, to } => {
            vfs::reply_rename(call, fs.rename(store, client, dir, from.as_str(), target, to.as_str()).map_err(Error::wire))
        }
        Request::Volume { handle } => {
            // The store's room is the store's to say (`blocks stat`); a chunk is the unit a file is held in.
            let volume = fs.volume(store, client, handle).map(|v| vfs::Volume {
                name: Text::new(v.name()).unwrap_or_default(), label: Text::new("MIND STORE").unwrap_or_default(), fat_bits: 0, bytes: 0, free: 0,
                cluster: CHUNK as u32, writable: v.writable,
            });
            vfs::reply_volume(call, volume.as_ref().map_err(|e| e.wire()))
        }
        Request::Flush { handle } => vfs::reply_flush(call, fs.flush(store, client, handle).map_err(Error::wire)),
        Request::Close { handle } => vfs::reply_close(call, fs.close(store, client, handle).map_err(Error::wire)),
        Request::Scope { dir, writable } => {
            end_scopes(fs);
            // A capability of storefs's own endpoint with the scope's badge, kept, so ending the scope revokes every copy.
            let now = mind::time::uptime_ms() as u64;
            let made = fs.scope(store, client, dir, writable, now, |badge| ipc::mint_badged(SLOT_SERVICE, CAP_WRITE | CAP_GRANT, badge).ok());
            if let Ok((badge, _)) = made {
                if let Some(scope) = fs.scoped(badge) {
                    mind::println!("[STOREFS] SCOPE {:#x} FOR {}:/{} ({})", badge, scope.volume, scope.path, if scope.writable { "WRITABLE" } else { "READ-ONLY" });
                }
            }
            vfs::reply_scope(call, made.map(|(_, cap)| cap).map_err(Error::wire))
        }
        Request::Adopt { scope, task } => {
            end_scopes(fs);
            vfs::reply_adopt(call, fs.adopt(client, scope, task).map_err(Error::wire))
        }
        // vfs_server's alone: a FAT check, formatting, a fill of a memory object.
        Request::Check { .. } => vfs::reply_check(call, Err(vfs::Error::Denied)),
        Request::Format { .. } => vfs::reply_format(call, Err(vfs::Error::Denied)),
        Request::Fill { .. } => vfs::reply_fill(call, Err(vfs::Error::Denied)),
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let (fs, scratch, data) = unsafe { (&mut *core::ptr::addr_of_mut!(FS), &mut *core::ptr::addr_of_mut!(SCRATCH), &mut *core::ptr::addr_of_mut!(DATA)) };
    // Without a store client every volume fails (`io`), so clients are not left waiting.
    let held = mind::dev::cap_info(SLOT_BLOCKSTORE).0 == CAP_KIND_ENDPOINT;
    mind::println!("[STOREFS] READY VOLUMES fs/<NAME>: {} TREES, {} HANDLES, {} FILES, {} KIB OF WRITTEN CHUNKS, {} SCOPES ({}, {}, {}, {} KIB, {} A CLIENT), FILES UP TO {} MIB{}",
                   TREES, HANDLES, FILES, CHUNKS * CHUNK / 1024, SCOPES, TREES_EACH, HANDLES_EACH, FILES_EACH, CHUNKS_EACH * CHUNK / 1024, SCOPES_EACH, FILE_MAX >> 20,
                   if held { "" } else { "; NO STORE CLIENT" });
    let mut store = Remote;
    loop {
        let Ok(request) = Endpoint::SERVICE.recv(RECEIVED) else { continue };
        if !request.is_call { continue; }
        let client = Client { pid: request.sender, badge: request.badge };
        let _ = match vfs::decode(&request, RECEIVED, scratch) {
            Ok((decoded, call)) => serve(fs, &mut store, client, decoded, call, data),
            Err(reason) => wire::reject(reason),
        };
    }
}
