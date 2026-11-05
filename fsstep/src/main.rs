#![no_std]
#![no_main]
// fsstep: a file written in steps through the scope its launcher lends, for the store volume's QEMU checks
// (311-STO-0032, 0037; docs/storage). It writes the first bytes of a fixed pattern and holds them, unpublished, for a
// while, then writes the rest and closes, the close publishing them, so a test can make a second writer, or a cut, while
// data is held. `--read-only` makes, from the scope it was lent, a scope of the same directory that does not write, and
// reads through it; `--bounds` shows where the lent scope writes. A console program: it asks the shell for its file
// (REQUEST_FILE), whose directory the shell lends in SLOT_FILE, and keeps full paths (mind::fs::use_scope). Where
// mind::fs would hide the service's answer (a close's) or refuse a path itself (another volume), it calls vfs.wit.
use core::fmt::Write;
use mind::abi::{BootInfo, CAP_KIND_ENDPOINT, SLOT_FILE};
use mind::fs::{self, Dir, Error, File, CHUNK, MODE_CREATE, MODE_TRUNCATE, MODE_WRITE};
use mind::idl::vfs;
use mind::ipc::Endpoint;
use mind::sha256::Sha256;

mind::request!(REQUEST_CONSOLE | REQUEST_FILE);

/// The client the shell lent: a scope of the file's directory.
const LENT: Endpoint = Endpoint(SLOT_FILE);
/// A fixed slot this program holds nothing in, where the read-only scope it makes is received (as wm's SCOPE).
const NARROW: usize = 13;
/// What `--bounds` writes in its directory.
const INSIDE: &[u8] = b"in its directory\n";
/// The volumes `--bounds` asks the lent client for besides its own: vfs_server's RAM disk and another store volume.
const OTHERS: [&str; 2] = ["ram", "notes"];
const USAGE: &str = "Usage: fsstep <file> <bytes> <seconds> <bytes> | fsstep <file> --read-only | fsstep <file> --bounds";

static mut BUFFER: [u8; CHUNK] = [0; CHUNK];

// Byte `i` of the pattern of tests/dag_host.rs and `blocks pattern`.
fn pattern(i: usize) -> u8 { ((i * 31 + 7) % 251) as u8 }

// Formats into a fixed buffer.
struct Line<const N: usize> { bytes: [u8; N], len: usize }
impl<const N: usize> Line<N> {
    fn new() -> Self { Self { bytes: [0; N], len: 0 } }
    fn as_str(&self) -> &str { core::str::from_utf8(&self.bytes[..self.len]).unwrap_or("") }
}
impl<const N: usize> Write for Line<N> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let end = self.len + s.len();
        self.bytes.get_mut(self.len..end).ok_or(core::fmt::Error)?.copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}

// A generated call: the IPC failure and the service's error, as mind::fs names them.
fn call<T>(result: mind::Result<Result<T, vfs::Error>>) -> Result<T, Error> { result.map_err(Error::from)?.map_err(Error::from) }

// The file's name in its directory, the lent scope's root.
fn leaf(path: &str) -> &str { fs::split(path).1.rsplit('/').next().unwrap_or("") }

// Writes bytes `from..from + n` of the pattern at their own offsets in `file`, at most CHUNK a call.
fn write(file: u32, from: usize, n: usize, buffer: &mut [u8; CHUNK]) -> Result<(), Error> {
    let mut done = 0;
    while done < n {
        let (at, piece) = (from + done, (n - done).min(CHUNK));
        for (k, b) in buffer[..piece].iter_mut().enumerate() { *b = pattern(at + k); }
        let wrote = call(vfs::write(LENT, file, at as u32, &buffer[..piece]))? as usize;
        if wrote == 0 { return Err(Error::NoSpace); }
        done += wrote;
    }
    Ok(())
}

// `path` made or emptied, its first bytes written and held unpublished for `seconds`, the rest written, then closed:
// the close publishes them, or says why it did not.
fn step(path: &str, first: usize, seconds: usize, rest: usize, buffer: &mut [u8; CHUNK]) {
    let root = match call(vfs::root(LENT, fs::split(path).0)) { Ok(root) => root, Err(e) => return mind::println!("FSSTEP ROOT: {:?}", e) };
    let file = match call(vfs::open(LENT, root, leaf(path), MODE_WRITE | MODE_CREATE | MODE_TRUNCATE)) {
        Ok(file) => file,
        Err(e) => return mind::println!("FSSTEP OPEN: {:?}", e),
    };
    let written = write(file, 0, first, buffer).and_then(|()| {
        mind::println!("FSSTEP HOLDING {} BYTES", first);
        mind::time::sleep(seconds.saturating_mul(1000));
        write(file, first, rest, buffer)
    });
    // A write refused leaves the file open, so what it held is dropped unpublished when the scope ends.
    if let Err(e) = written { return mind::println!("FSSTEP WRITE: {:?}", e); }
    match call(vfs::close(LENT, file)) {
        Ok(()) => mind::println!("FSSTEP CLOSED {} BYTES", first + rest),
        Err(e) => mind::println!("FSSTEP CLOSE: {:?}", e),
    }
    let _ = vfs::close(LENT, root);
}

// The file's size and SHA-256, read through mind::fs.
fn read(path: &str, buffer: &mut [u8; CHUNK]) -> Result<(usize, [u8; 32]), Error> {
    let file = File::open(path)?;
    let (mut hash, mut at) = (Sha256::new(), 0);
    loop {
        let n = file.read_at(at, buffer)?;
        hash.update(&buffer[..n]);
        at += n;
        if n < buffer.len() { return Ok((at, hash.finish())); }
    }
}

// From the scope it was lent, a scope of the same directory that does not write (`Dir::scope`); through it the
// directory listed and the file read, then a write to the file and a new file refused.
fn read_only(path: &str, base: &str, buffer: &mut [u8; CHUNK]) {
    if let Err(e) = Dir::open(base).and_then(|dir| dir.scope(false, NARROW)) { return mind::println!("FSSTEP SCOPE: {:?}", e); }
    fs::use_scope(Endpoint(NARROW), base);
    mind::println!("FSSTEP SCOPE READ-ONLY OF {}", base);
    let listed = fs::list(base, |e| mind::println!("FSSTEP ENTRY {} {} {}", if e.is_dir { "DIR" } else { "FILE" }, e.size, e.name_str()));
    match listed { Ok(n) => mind::println!("FSSTEP LISTED {} ENTRIES", n), Err(e) => mind::println!("FSSTEP LIST: {:?}", e) }
    match read(path, buffer) {
        Ok((n, digest)) => {
            let mut hex = Line::<64>::new();
            for b in digest { let _ = write!(hex, "{:02x}", b); }
            mind::println!("FSSTEP READ {} BYTES SHA-256 {}", n, hex.as_str());
        }
        Err(e) => mind::println!("FSSTEP READ: {:?}", e),
    }
    match File::open_mode(path, MODE_WRITE) { Ok(_) => mind::println!("FSSTEP WRITE {} ALLOWED", path), Err(e) => mind::println!("FSSTEP WRITE {} REFUSED: {:?}", path, e) }
    let mut new = Line::<300>::new();
    let _ = write!(new, "{}{}fsstep.new", base, if base.is_empty() || base.ends_with(':') { "" } else { "/" });
    match File::create(new.as_str()) { Ok(_) => mind::println!("FSSTEP CREATE {} ALLOWED", new.as_str()), Err(e) => mind::println!("FSSTEP CREATE {} REFUSED: {:?}", new.as_str(), e) }
}

// The lent scope writes in its directory and nowhere else: the file written there; then, asked of the lent client
// itself, as mind::fs refuses such paths before asking, its own volume's root is that directory, and a path through
// `..` and another volume's root are refused.
fn bounds(path: &str) {
    match File::create(path).and_then(|mut f| { f.write(INSIDE)?; f.flush() }) {
        Ok(()) => mind::println!("FSSTEP WROTE {} BYTES TO {}", INSIDE.len(), path),
        Err(e) => mind::println!("FSSTEP WRITE {}: {:?}", path, e),
    }
    let volume = fs::split(path).0;
    match call(vfs::root(LENT, volume)) {
        Ok(root) => {
            match call(vfs::stat(LENT, root)) {
                Ok(e) => mind::println!("FSSTEP ROOT OF {} IS {}", volume, if e.name.as_str().is_empty() { "/" } else { e.name.as_str() }),
                Err(e) => mind::println!("FSSTEP STAT: {:?}", e),
            }
            match call(vfs::open(LENT, root, "../fsstep.out", MODE_WRITE | MODE_CREATE)) {
                Ok(file) => { let _ = vfs::close(LENT, file); mind::println!("FSSTEP OPEN ../fsstep.out ALLOWED") }
                Err(e) => mind::println!("FSSTEP OPEN ../fsstep.out REFUSED: {:?}", e),
            }
            match call(vfs::open_dir(LENT, root, "..", false)) {
                Ok(dir) => { let _ = vfs::close(LENT, dir); mind::println!("FSSTEP OPEN-DIR .. ALLOWED") }
                Err(e) => mind::println!("FSSTEP OPEN-DIR .. REFUSED: {:?}", e),
            }
            let _ = vfs::close(LENT, root);
        }
        Err(e) => mind::println!("FSSTEP ROOT {}: {:?}", volume, e),
    }
    for other in OTHERS.iter().filter(|o| !o.eq_ignore_ascii_case(volume)) {
        match call(vfs::root(LENT, other)) {
            Ok(root) => { let _ = vfs::close(LENT, root); mind::println!("FSSTEP ROOT {} ALLOWED", other) }
            Err(e) => mind::println!("FSSTEP ROOT {} REFUSED: {:?}", other, e),
        }
    }
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    mind::about!("fsstep — a file written in steps through the scope its launcher lends, its first bytes held unpublished, for the store volume's checks (311-STO-0032, 0037).\nUsage: fsstep <file> <bytes> <seconds> <bytes> | fsstep <file> --read-only | fsstep <file> --bounds\nThe bytes are the pattern (i*31+7) % 251 at offset i; `--read-only` reads through a read-only scope of the file's directory and is refused a write; `--bounds` writes the file and is refused outside its directory.");
    let (mut all, mut count) = ([""; 5], 0);
    for w in mind::process::args_str().split_whitespace() {
        if let Some(slot) = all.get_mut(count) { *slot = w; }
        count += 1;
    }
    let Some(&[path, ref rest @ ..]) = all.get(..count) else { return mind::println!("{}", USAGE) };
    if mind::dev::cap_info(SLOT_FILE).0 != CAP_KIND_ENDPOINT { return mind::println!("fsstep: no client of {}'s directory: the shell lends one to a program it starts with its file", path); }
    // The lent client's root is the file's directory, as edit takes it.
    let (volume, inside) = fs::split(path);
    let mut base = Line::<300>::new();
    let _ = write!(base, "{}{}{}", volume, if volume.is_empty() { "" } else { ":" }, inside.rfind('/').map_or("", |i| &inside[..i]));
    fs::use_scope(LENT, base.as_str());
    let buffer = unsafe { &mut *core::ptr::addr_of_mut!(BUFFER) };
    match rest {
        ["--read-only"] => read_only(path, base.as_str(), buffer),
        ["--bounds"] => bounds(path),
        [first, seconds, then] => match (first.parse::<usize>(), seconds.parse::<usize>(), then.parse::<usize>()) {
            (Ok(first), Ok(seconds), Ok(then)) if first.checked_add(then).is_some_and(|total| total <= u32::MAX as usize) => step(path, first, seconds, then, buffer),
            _ => mind::println!("fsstep: two sizes in bytes, together below 4 GiB, and a number of seconds"),
        },
        _ => mind::println!("{}", USAGE),
    }
}
