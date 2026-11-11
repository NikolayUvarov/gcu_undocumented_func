//! One line a request, a file a day, kept a stated number of days (MC-10.3); never a query string or a header's value.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

pub struct Log {
    dir: Option<PathBuf>,
    days: u64,
    open: Mutex<Option<(String, File)>>,
}

/// The target as the log may hold it: up to a query, bytes outside the names' alphabet as `?`, at most 200 bytes.
pub fn shown(target: &str) -> String {
    target.split('?').next().unwrap_or("").bytes().take(200)
        .map(|b| if b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'/') { b as char } else { '?' }).collect()
}

impl Log {
    /// Lines to `dir/httpd-YYYY-MM-DD.log`, files older than `days` removed; without a directory, to standard error.
    pub fn new(dir: Option<PathBuf>, days: u64) -> Log { Log { dir, days, open: Mutex::new(None) } }

    pub fn line(&self, now: SystemTime, client: &str, method: &str, target: &str, status: u16, bytes: u64, took: Duration, why: &str) {
        let mut line = format!("{} {} {} {} {} {} {}ms", crate::date::rfc3339(now), client, method, shown(target), status, bytes, took.as_millis());
        if !why.is_empty() { line.push_str(&format!(" ({why})")); }
        line.push('\n');
        let Some(dir) = &self.dir else { eprint!("{line}"); return };
        let day = crate::date::day(now);
        let mut open = self.open.lock().unwrap();
        if open.as_ref().map(|(d, _)| d != &day).unwrap_or(true) {
            let path = dir.join(format!("httpd-{day}.log"));
            *open = OpenOptions::new().create(true).append(true).open(&path).ok().map(|f| (day.clone(), f));
            self.prune(now);
        }
        if let Some((_, file)) = open.as_mut() { let _ = file.write_all(line.as_bytes()); }
    }

    // Removes the day files older than the days kept.
    fn prune(&self, now: SystemTime) {
        let Some(dir) = &self.dir else { return };
        let oldest = crate::date::day(now - Duration::from_secs(self.days.saturating_sub(1) * 86_400));
        let Ok(entries) = fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(day) = name.strip_prefix("httpd-").and_then(|n| n.strip_suffix(".log")) {
                if day.len() == 10 && day < oldest.as_str() { let _ = fs::remove_file(entry.path()); }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::shown;

    #[test]
    fn what_the_log_holds() {
        assert_eq!(shown("/channels/stable?token=secret"), "/channels/stable");
        assert_eq!(shown("/a b\r\n%00"), "/a?b???00");
        assert_eq!(shown(&"/x".repeat(300)).len(), 200);
    }
}
