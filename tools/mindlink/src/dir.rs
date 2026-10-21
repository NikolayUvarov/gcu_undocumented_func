//! `serve`'s store: the files of one directory and its subdirectories, a log file followed from a cursor, a command's
//! console, and with `--run` the programs in the directory, started for the master (800-LNK-0009, 0010).
use linkproto::app::{Files, RunOutput, EXIT_UNKNOWN, RUN_ENDED, RUN_RUNNING, RUN_STOPPED};
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
    /// Programs may be started (`serve --run`); those started, oldest first.
    run: bool,
    runs: Vec<Run>,
    next: u32,
}

/// A program started for the master: the last KEEP bytes of its output and standard error, and its end.
struct Run { id: u32, name: String, child: Child, out: Arc<Mutex<Output>>, state: u8, exit: u32 }
#[derive(Default)]
struct Output { kept: Vec<u8>, total: u64, readers_done: u32 }
const KEEP: usize = 64 * 1024;
/// Runs remembered; the oldest ended one is forgotten first.
const RUNS_KEPT: usize = 32;

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
                let (shell, flag) = if cfg!(windows) { ("cmd", "/C") } else { ("/bin/sh", "-c") };
                let mut child = Command::new(shell).arg(flag).arg(cmd).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
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
        let next = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as u32) << 8 | 1;
        Ok(Dir { root, put: None, get: None, log, console, run: false, runs: Vec::new(), next })
    }

    /// Lets the master start programs in the directory: as good as a shell for it, as the user who serves.
    pub fn allow_runs(&mut self) { self.run = true; }

    fn path(&self, name: &str) -> PathBuf { self.root.join(name) }

    fn find(&mut self, id: u32) -> Result<&mut Run, u8> {
        let run = self.runs.iter_mut().find(|r| r.id == id).ok_or(code::NOT_FOUND)?;
        if run.state == RUN_RUNNING {
            // Ended once the program has and both its pipes were read to their end.
            if let Ok(Some(status)) = run.child.try_wait() {
                if run.out.lock().unwrap().readers_done == 2 {
                    run.state = RUN_ENDED;
                    run.exit = status.code().map_or(EXIT_UNKNOWN, |c| c as u32);
                    eprintln!("run {}: {} ended, {}", run.id, run.name, status);
                }
            }
        }
        Ok(run)
    }
}

fn not_empty(path: &std::path::Path) -> bool { fs::read_dir(path).map(|mut d| d.next().is_some()).unwrap_or(false) }

impl Files for Dir {
    fn create(&mut self, name: &str, size: u64) -> Result<(), u8> {
        if !self.path(name).parent().is_some_and(|p| p.is_dir()) { return Err(code::NOT_FOUND); }
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
        if self.path(&name).is_dir() { fs::remove_file(&part).ok(); return Err(code::EXISTS); }
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

    fn mkdir(&mut self, name: &str) -> Result<(), u8> {
        let path = self.path(name);
        if path.exists() && !path.is_dir() { return Err(code::EXISTS); }
        fs::create_dir_all(&path).map_err(io)?;
        eprintln!("made {name}/");
        Ok(())
    }

    fn rename(&mut self, from: &str, to: &str, replace: bool) -> Result<(), u8> {
        if to.strip_prefix(from).is_some_and(|rest| rest.starts_with('/')) { return Err(code::DENIED); }
        let (source, target) = (self.path(from), self.path(to));
        if !source.exists() { return Err(code::NOT_FOUND); }
        if target.is_dir() || target.exists() && !replace { return Err(code::EXISTS); }
        if !target.parent().is_some_and(|p| p.is_dir()) { return Err(code::NOT_FOUND); }
        fs::rename(&source, &target).map_err(io)?;
        eprintln!("moved {from} to {to}");
        Ok(())
    }

    fn rmdir(&mut self, name: &str, recursive: bool) -> Result<(), u8> {
        let path = self.path(name);
        if !path.is_dir() { return Err(if path.exists() { code::DENIED } else { code::NOT_FOUND }); }
        if recursive { fs::remove_dir_all(&path).map_err(io)?; }
        else if not_empty(&path) { return Err(code::NOT_EMPTY); }
        else { fs::remove_dir(&path).map_err(io)?; }
        eprintln!("removed {name}/{}", if recursive { " and all in it" } else { "" });
        Ok(())
    }

    fn list_dir(&mut self, name: &str, cursor: u32, out: &mut [u8]) -> Result<(usize, Option<u32>), u8> {
        let top = name.is_empty();
        let mut lines: Vec<String> = fs::read_dir(if top { self.root.clone() } else { self.path(name) }).map_err(io)?.filter_map(|e| {
            let e = e.ok()?;
            let meta = e.metadata().ok()?;
            let entry = e.file_name().into_string().ok()?;
            if top && entry == PART { return None; }
            Some(if meta.is_dir() { format!("{entry}/\t0\n") } else { format!("{entry}\t{}\n", meta.len()) })
        }).collect();
        lines.sort();
        let mut used = 0;
        for (i, line) in lines.iter().enumerate().skip(cursor as usize) {
            if used + line.len() > out.len() {
                return if used == 0 { Err(code::IO) } else { Ok((used, Some(i as u32))) };
            }
            out[used..used + line.len()].copy_from_slice(line.as_bytes());
            used += line.len();
        }
        Ok((used, None))
    }

    fn run(&mut self, name: &str, args: &str) -> Result<(u32, u64), u8> {
        if !self.run { return Err(code::DENIED); }
        let program = self.path(name);
        if !program.is_file() { return Err(code::NOT_FOUND); }
        // A file put over the link has no execute bit; asked to run it, the owner's is set.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut mode = fs::metadata(&program).map_err(io)?.permissions();
            if mode.mode() & 0o100 == 0 { mode.set_mode(mode.mode() | 0o100); fs::set_permissions(&program, mode).map_err(io)?; }
        }
        if self.runs.len() >= RUNS_KEPT {
            let oldest = self.runs.iter().position(|r| r.state != RUN_RUNNING).ok_or(code::BUSY)?;
            self.runs.remove(oldest);
        }
        let mut child = Command::new(&program).args(args.split_whitespace()).current_dir(&self.root)
            .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().map_err(io)?;
        let out = Arc::new(Mutex::new(Output::default()));
        for mut from in [Box::new(child.stdout.take().unwrap()) as Box<dyn Read + Send>, Box::new(child.stderr.take().unwrap())] {
            let to = out.clone();
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                while let Ok(n @ 1..) = from.read(&mut buf) {
                    let mut o = to.lock().unwrap();
                    o.kept.extend_from_slice(&buf[..n]);
                    o.total += n as u64;
                    let over = o.kept.len().saturating_sub(KEEP);
                    o.kept.drain(..over);
                }
                to.lock().unwrap().readers_done += 1;
            });
        }
        let (id, pid) = (self.next, child.id() as u64);
        self.next = self.next.wrapping_add(1).max(1);
        eprintln!("run {id}: started {name} {args} as PID {pid}");
        self.runs.push(Run { id, name: name.to_string(), child, out, state: RUN_RUNNING, exit: EXIT_UNKNOWN });
        Ok((id, pid))
    }

    fn output(&mut self, id: u32, cursor: u64, out: &mut [u8]) -> Result<RunOutput, u8> {
        let run = self.find(id)?;
        let o = run.out.lock().unwrap();
        let first = o.total - o.kept.len() as u64;
        let from = cursor.clamp(first, o.total);
        let rest = &o.kept[(from - first) as usize..];
        let n = rest.len().min(out.len());
        out[..n].copy_from_slice(&rest[..n]);
        Ok(RunOutput { state: run.state, exit: run.exit, from, len: n })
    }

    fn stop(&mut self, id: u32) -> Result<(), u8> {
        let run = self.find(id)?;
        if run.state != RUN_RUNNING { return Ok(()); }
        run.child.kill().map_err(io)?;
        run.child.wait().ok();
        run.state = RUN_STOPPED;
        eprintln!("run {id}: {} stopped", run.name);
        Ok(())
    }

    fn runs(&mut self, cursor: u32, out: &mut [u8]) -> Result<(usize, Option<u32>), u8> {
        let ids: Vec<u32> = self.runs.iter().map(|r| r.id).collect();
        let mut used = 0;
        for (i, id) in ids.into_iter().enumerate().skip(cursor as usize) {
            let run = self.find(id)?;
            let state = ["running", "ended", "stopped"][run.state as usize];
            let line = format!("{}\t{}\t{state}\t{}\n", run.id, run.child.id(), run.name);
            if used + line.len() > out.len() { return Ok((used, Some(i as u32))); }
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
