//! `serve`'s store: the files of one directory, a log file followed from a cursor, a command's console.
use linkproto::app::Files;
use linkproto::endpoint::code;
use linkproto::sha256::Sha256;
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};

/// Where a put is written until its SHA-256 matches.
const PART: &str = ".mindlink.part";

pub struct Dir {
    root: PathBuf,
    put: Option<(File, String, u64)>,
    get: Option<File>,
    log: Option<PathBuf>,
    console: Option<Console>,
}

struct Console { _child: Child, input: ChildStdin, output: Arc<Mutex<Vec<u8>>> }

fn io(e: std::io::Error) -> u8 {
    match e.kind() { ErrorKind::NotFound => code::NOT_FOUND, ErrorKind::PermissionDenied => code::DENIED, _ => code::IO }
}

impl Dir {
    pub fn new(root: PathBuf, log: Option<PathBuf>, console: Option<&str>) -> Result<Dir, String> {
        if !root.is_dir() { return Err(format!("{}: not a directory", root.display())); }
        let console = match console {
            None => None,
            Some(cmd) => {
                let mut child = Command::new("/bin/sh").arg("-c").arg(cmd).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
                    .spawn().map_err(|e| format!("{cmd}: {e}"))?;
                let output = Arc::new(Mutex::new(Vec::new()));
                for mut from in [Box::new(child.stdout.take().unwrap()) as Box<dyn Read + Send>, Box::new(child.stderr.take().unwrap())] {
                    let to = output.clone();
                    std::thread::spawn(move || {
                        let mut buf = [0u8; 4096];
                        while let Ok(n @ 1..) = from.read(&mut buf) { to.lock().unwrap().extend_from_slice(&buf[..n]); }
                    });
                }
                Some(Console { input: child.stdin.take().unwrap(), _child: child, output })
            }
        };
        Ok(Dir { root, put: None, get: None, log, console })
    }
}

impl Files for Dir {
    fn create(&mut self, name: &str, size: u64) -> Result<(), u8> {
        let file = OpenOptions::new().write(true).create(true).truncate(true).open(self.root.join(PART)).map_err(io)?;
        eprintln!("receiving {name} ({size} bytes)");
        self.put = Some((file, name.to_string(), size));
        Ok(())
    }

    fn write_at(&mut self, offset: u64, data: &[u8]) -> Result<(), u8> {
        let (file, _, _) = self.put.as_mut().ok_or(code::BAD_REQUEST)?;
        file.seek(SeekFrom::Start(offset)).map_err(io)?;
        file.write_all(data).map_err(io)
    }

    fn commit(&mut self, digest: &[u8; 32], matches: bool) -> Result<(), u8> {
        let (file, name, size) = self.put.take().ok_or(code::BAD_REQUEST)?;
        let part = self.root.join(PART);
        if !matches {
            drop(file);
            fs::remove_file(&part).ok();
            eprintln!("{name}: SHA-256 does not match, dropped");
            return Ok(());
        }
        file.sync_all().map_err(io)?;
        fs::rename(&part, self.root.join(&name)).map_err(io)?;
        eprintln!("received {name} ({size} bytes, SHA-256 {})", crate::hex(digest));
        Ok(())
    }

    fn open(&mut self, name: &str) -> Result<u64, u8> {
        let file = File::open(self.root.join(name)).map_err(io)?;
        let meta = file.metadata().map_err(io)?;
        if !meta.is_file() { return Err(code::NOT_FOUND); }
        eprintln!("sending {name} ({} bytes)", meta.len());
        self.get = Some(file);
        Ok(meta.len())
    }

    fn read_at(&mut self, offset: u64, out: &mut [u8]) -> Result<usize, u8> {
        let file = self.get.as_mut().ok_or(code::BAD_REQUEST)?;
        file.seek(SeekFrom::Start(offset)).map_err(io)?;
        let mut n = 0;
        while n < out.len() {
            match file.read(&mut out[n..]).map_err(io)? { 0 => break, got => n += got }
        }
        if n == 0 { return Err(code::IO); }
        Ok(n)
    }

    fn stat(&mut self, name: &str) -> Result<(u64, [u8; 32]), u8> {
        let mut file = File::open(self.root.join(name)).map_err(io)?;
        if !file.metadata().map_err(io)?.is_file() { return Err(code::NOT_FOUND); }
        let (mut sha, mut buf, mut size) = (Sha256::new(), vec![0u8; 1 << 16], 0u64);
        loop {
            let n = file.read(&mut buf).map_err(io)?;
            if n == 0 { break; }
            sha.update(&buf[..n]);
            size += n as u64;
        }
        Ok((size, sha.finish()))
    }

    fn delete(&mut self, name: &str) -> Result<(), u8> {
        fs::remove_file(self.root.join(name)).map_err(io)?;
        eprintln!("deleted {name}");
        Ok(())
    }

    fn list(&mut self, cursor: u32, out: &mut [u8]) -> Result<(usize, Option<u32>), u8> {
        let mut names: Vec<(String, u64)> = fs::read_dir(&self.root).map_err(io)?.filter_map(|e| {
            let e = e.ok()?;
            let meta = e.metadata().ok()?;
            let name = e.file_name().into_string().ok()?;
            (meta.is_file() && name != PART).then_some((name, meta.len()))
        }).collect();
        names.sort();
        let mut used = 0;
        for (i, (name, size)) in names.iter().enumerate().skip(cursor as usize) {
            let line = format!("{name}\t{size}\n");
            if used + line.len() > out.len() {
                return if used == 0 { Err(code::IO) } else { Ok((used, Some(i as u32))) };
            }
            out[used..used + line.len()].copy_from_slice(line.as_bytes());
            used += line.len();
        }
        Ok((used, None))
    }

    fn log(&mut self, cursor: u64, out: &mut [u8]) -> Result<(usize, u64), u8> {
        let path = self.log.as_ref().ok_or(code::UNSUPPORTED)?;
        let mut file = File::open(path).map_err(io)?;
        let len = file.metadata().map_err(io)?.len();
        // A log cut short starts again from its beginning.
        let from = if cursor > len { 0 } else { cursor };
        file.seek(SeekFrom::Start(from)).map_err(io)?;
        let n = file.read(out).map_err(io)?;
        Ok((n, from + n as u64))
    }

    fn console(&mut self, input: &[u8], out: &mut [u8]) -> Result<usize, u8> {
        let c = self.console.as_mut().ok_or(code::UNSUPPORTED)?;
        if !input.is_empty() { c.input.write_all(input).and_then(|_| c.input.flush()).map_err(io)?; }
        let mut buf = c.output.lock().unwrap();
        let n = buf.len().min(out.len());
        out[..n].copy_from_slice(&buf[..n]);
        buf.drain(..n);
        Ok(n)
    }
}
