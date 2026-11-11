//! Test hooks (feature `test-hooks`, for the project's QEMU checks alone, 110-NET-0068): files in one directory cut or
//! hold the next response for a path, and each request's path and `Range` are recorded.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// The longest a held response waits.
pub const HOLD_MAX: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub struct Hooks { dir: PathBuf }

/// A path as a file name in the hooks' directories: without its first `/`, each `/` as `+`, which no name of the layout holds.
pub fn name(target: &str) -> String { target.trim_start_matches('/').replace('/', "+") }

impl Hooks {
    /// The hooks of `dir`, whose `cut/`, `hold/` and `held/` are made if missing.
    pub fn new(dir: PathBuf) -> io::Result<Hooks> {
        for sub in ["cut", "hold", "held"] { fs::create_dir_all(dir.join(sub))?; }
        Ok(Hooks { dir })
    }

    /// Records a request in `requests`: its path and its `Range` header, `-` without one, each a line's printable bytes.
    pub fn asked(&self, target: &str, range: Option<&str>) {
        let printable = |s: &str| s.bytes().map(|b| if b.is_ascii_graphic() || b == b' ' { b as char } else { '?' }).collect::<String>();
        let line = format!("{} {}\n", crate::log::shown(target), range.map(|r| printable(r.trim())).unwrap_or_else(|| "-".into()));
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(self.dir.join("requests")) { let _ = file.write_all(line.as_bytes()); }
    }

    /// The body bytes after which this response is cut, when `cut/<name>` asked for it; one request takes it.
    pub fn cut(&self, target: &str) -> Option<u64> {
        let path = self.dir.join("cut").join(name(target));
        let text = fs::read_to_string(&path).ok()?;
        fs::remove_file(&path).ok()?;
        text.trim().parse().ok()
    }

    /// Waits while `held/<name>` is there, at most `HOLD_MAX`, when `hold/<name>` asked for it; whether it held.
    pub fn hold(&self, target: &str) -> bool {
        let (hold, held) = (self.dir.join("hold").join(name(target)), self.dir.join("held").join(name(target)));
        if fs::rename(&hold, &held).is_err() { return false; }
        let until = Instant::now() + HOLD_MAX;
        while held.exists() && Instant::now() < until { std::thread::sleep(Duration::from_millis(20)); }
        let _ = fs::remove_file(&held);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_takes() {
        assert_eq!(name("/blobs/ab"), "blobs+ab");
        let dir = std::env::temp_dir().join(format!("httpd-hooks-{}", std::process::id()));
        let hooks = Hooks::new(dir.clone()).unwrap();
        fs::write(dir.join("cut/channels+stable"), "12\n").unwrap();
        assert_eq!(hooks.cut("/channels/stable"), Some(12));
        assert_eq!(hooks.cut("/channels/stable"), None);
        hooks.asked("/a?q=secret", Some(" bytes=5-\r"));
        hooks.asked("/b", None);
        assert_eq!(fs::read_to_string(dir.join("requests")).unwrap(), "/a bytes=5-\n/b -\n");
        assert!(!hooks.hold("/b"));
        fs::remove_dir_all(dir).unwrap();
    }
}
