// File commands of the shell (mind::fs over idl/vfs.wit). The shell's VFS capability carries the user's badge: it
// writes on `ram:`, on `log:` and in `data/` of the boot disk; boot files and the rest of the disk stay read-only.
use crate::console::Console;
use core::fmt::Write;
use mind::fs::{self, Error, File};

pub fn text(error: Error) -> &'static str {
    match error {
        Error::NotFound => "NOT FOUND", Error::Exists => "ALREADY EXISTS", Error::NotEmpty => "DIRECTORY NOT EMPTY", Error::Invalid => "INVALID PATH OR REQUEST",
        Error::Denied => "DENIED (ONLY RAM:, LOG: AND DATA/ ARE WRITABLE)", Error::NoSpace => "NO SPACE", Error::ReadOnly => "READ-ONLY DEVICE", Error::Io => "I/O ERROR",
        Error::Handles => "TOO MANY OPEN FILES", Error::Name => "INVALID NAME", Error::NotDirectory => "NOT A DIRECTORY", Error::IsDirectory => "IS A DIRECTORY",
        Error::NoMemory => "OUT OF MEMORY", Error::NoService => "VFS NOT AVAILABLE",
    }
}

fn report(out: &mut Console, what: &str, error: Error) { let _ = writeln!(out, "ERROR: {}: {}", what, text(error)); }

fn utf8(bytes: &[u8]) -> &str { core::str::from_utf8(bytes).unwrap_or("") }

/// `ls [path]`: entries with size, date and time.
pub fn ls(out: &mut Console, path: &[u8]) {
    let path = utf8(path);
    let (mut files, mut bytes) = (0usize, 0u64);
    let result = fs::list(path, |e| {
        let (y, mo, d, h, mi, _) = fs::fat_time(e.modified);
        if e.is_dir { let _ = write!(out, "{:<32} {:>10}", e.name_str(), "<DIR>"); } else { let _ = write!(out, "{:<32} {:>10}", e.name_str(), e.size); files += 1; bytes += e.size as u64; }
        if e.modified != 0 { let _ = write!(out, "  {}-{:02}-{:02} {:02}:{:02}", y, mo, d, h, mi); }
        let _ = writeln!(out);
    });
    match result {
        Ok(count) => { let _ = writeln!(out, "{} ENTRIES, {} FILES, {} BYTES", count, files, bytes); }
        Err(error) => report(out, "LS", error),
    }
}

/// `cat <file>`: the text of a file (the first 16 KiB).
pub fn cat(out: &mut Console, path: &[u8]) {
    let mut file = match File::open(utf8(path)) { Ok(f) => f, Err(e) => return report(out, "CAT", e) };
    let mut buffer = [0u8; 4096];
    let mut shown = 0;
    while shown < 16 * 1024 {
        match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => { for &b in &buffer[..n] { out.print_char(b); } shown += n; }
            Err(e) => return report(out, "CAT", e),
        }
    }
    if out.position().col != 0 { out.print_char(b'\n'); }
    if file.size() > shown { let _ = writeln!(out, "... {} MORE BYTES", file.size() - shown); }
}

/// `write <file> <text>`: the file holds the text and a line end.
pub fn write(out: &mut Console, args: &[u8]) {
    let split = args.iter().position(|b| b.is_ascii_whitespace()).unwrap_or(args.len());
    let (path, body) = (utf8(&args[..split]), args[split..].trim_ascii());
    if path.is_empty() { return report(out, "USAGE: WRITE <FILE> <TEXT>", Error::Invalid); }
    let result = File::create(path).and_then(|mut f| { f.write(body)?; f.write(b"\n")?; f.flush() });
    match result { Ok(()) => { let _ = writeln!(out, "WROTE {} BYTES TO {}", body.len() + 1, path); } Err(e) => report(out, "WRITE", e) }
}

/// `mkdir <path>`, `rm <path>`, `mv <from> <to>`: changes are written to the disk at once.
pub fn change(out: &mut Console, command: &str, args: &[u8]) {
    let args = utf8(args);
    let mut words = args.split_ascii_whitespace();
    let (first, second) = (words.next().unwrap_or(""), words.next());
    if first.is_empty() || (command == "MV") != second.is_some() || words.next().is_some() { let _ = writeln!(out, "USAGE: MKDIR <PATH>, RM <PATH>, MV <FROM> <TO>"); return; }
    let result = match command { "MKDIR" => fs::mkdir(first), "RM" => fs::remove(first), _ => fs::rename(first, second.unwrap_or("")) }.and_then(|_| flush(first));
    match result { Ok(()) => { let _ = writeln!(out, "OK"); } Err(e) => report(out, command, e) }
}

fn flush(path: &str) -> Result<(), Error> { fs::Dir::root(fs::split(path).0)?.flush() }

// Flushes every volume; the errors of the ones that failed (a missing volume is not one).
fn flush_volumes(mut failed: impl FnMut(Error)) {
    for volume in ["", "ram"] {
        if let Err(e) = fs::Dir::root(volume).and_then(|d| d.flush()) { if e != Error::NotFound { failed(e); } }
    }
}

/// Writes what is cached for every volume (before the system stops).
pub fn flush_all() { flush_volumes(|_| {}); }

/// `sync`: writes what is cached for every volume.
pub fn sync(out: &mut Console) {
    let mut ok = true;
    flush_volumes(|e| { ok = false; report(out, "SYNC", e); });
    if ok { let _ = writeln!(out, "OK"); }
}
