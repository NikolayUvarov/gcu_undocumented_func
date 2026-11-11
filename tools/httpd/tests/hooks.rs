//! The test hooks (feature `test-hooks`, 110-NET-0068): a cut resumed with `Range`, a hold released and one that runs
//! out, the requests recorded; and a server built without the feature refuses `--test-hooks`.

#[cfg(feature = "test-hooks")]
mod with_hooks {
    use httpd::hooks::{Hooks, HOLD_MAX};
    use httpd::log::Log;
    use httpd::server::{self, Config};
    use std::fs;
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    const DIGEST: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

    fn blob() -> Vec<u8> { (0..50_000u32).map(|i| (i % 253) as u8).collect() }

    // A server over a small layout with its hooks in `base/hooks`; returns the base, the hooks' directory and its address.
    fn site(tag: &str) -> (PathBuf, PathBuf, SocketAddr) {
        let base = std::env::temp_dir().join(format!("httpd-hooks-test-{}-{tag}", std::process::id()));
        let (root, hooks, logs) = (base.join("root"), base.join("hooks"), base.join("logs"));
        for dir in ["blobs", "channels"] { fs::create_dir_all(root.join(dir)).unwrap(); }
        fs::create_dir_all(&logs).unwrap();
        fs::write(root.join("blobs").join(DIGEST), blob()).unwrap();
        fs::write(root.join("channels/stable"), b"{\"version\":6}\n").unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let mut config = Config::new(root.canonicalize().unwrap());
        config.hooks = Some(Arc::new(Hooks::new(hooks.clone()).unwrap()));
        std::thread::spawn(move || server::run(listener, config, Log::new(Some(logs), 30)));
        (base, hooks, addr)
    }

    // One GET on its own connection: the status line, the headers' text and the body, up to the connection's end.
    fn get(addr: SocketAddr, path: &str, range: Option<&str>) -> (String, String, Vec<u8>) {
        let mut stream = TcpStream::connect(addr).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
        let range = range.map(|r| format!("Range: {r}\r\n")).unwrap_or_default();
        write!(stream, "GET {path} HTTP/1.1\r\nHost: t\r\n{range}Connection: close\r\n\r\n").unwrap();
        let mut all = Vec::new();
        let _ = stream.read_to_end(&mut all);
        let end = all.windows(4).position(|w| w == b"\r\n\r\n").expect("no head") + 4;
        let head = String::from_utf8_lossy(&all[..end]).into_owned();
        let (status, headers) = head.split_once("\r\n").unwrap();
        (status.to_string(), headers.to_string(), all[end..].to_vec())
    }

    fn logged(base: &PathBuf) -> String {
        fs::read_dir(base.join("logs")).unwrap().flatten().map(|e| fs::read_to_string(e.path()).unwrap()).collect()
    }

    #[test]
    fn a_cut_resumed_and_recorded() {
        let (base, hooks, addr) = site("cut");
        let path = format!("/blobs/{DIGEST}");
        fs::write(hooks.join("cut").join(format!("blobs+{DIGEST}")), "1000\n").unwrap();
        let (status, headers, body) = get(addr, &path, None);
        assert!(status.contains(" 200 "), "{status}");
        assert!(headers.contains("Content-Length: 50000\r\n"), "{headers}");
        assert_eq!(body, blob()[..1000]);
        // The hook is taken once: the resumed request gets the rest whole.
        let (status, headers, body) = get(addr, &path, Some("bytes=1000-"));
        assert!(status.contains(" 206 "), "{status}");
        assert!(headers.contains("Content-Range: bytes 1000-49999/50000\r\n"), "{headers}");
        assert_eq!(body, blob()[1000..]);
        assert_eq!(fs::read_to_string(hooks.join("requests")).unwrap(), format!("{path} -\n{path} bytes=1000-\n"));
        assert!(logged(&base).contains(&format!("GET {path} 200 1000 ")) && logged(&base).contains("(cut by a test hook)"), "{}", logged(&base));
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn a_hold_released_and_one_that_runs_out() {
        let (base, hooks, addr) = site("hold");
        fs::write(hooks.join("hold/channels+stable"), "").unwrap();
        let started = Instant::now();
        let asked = std::thread::spawn(move || get(addr, "/channels/stable", None));
        let held = hooks.join("held/channels+stable");
        while !held.exists() { assert!(started.elapsed() < Duration::from_secs(5), "never held"); std::thread::sleep(Duration::from_millis(10)); }
        std::thread::sleep(Duration::from_millis(300));
        assert!(!asked.is_finished(), "answered while held");
        assert_eq!(fs::read_to_string(hooks.join("requests")).unwrap(), "/channels/stable -\n");
        fs::remove_file(&held).unwrap();
        let (status, _, body) = asked.join().unwrap();
        assert!(status.contains(" 200 ") && body == b"{\"version\":6}\n", "{status}");
        assert!(started.elapsed() < HOLD_MAX, "{:?}", started.elapsed());
        // A hold nobody releases runs out after HOLD_MAX, and the response follows.
        fs::write(hooks.join("hold/channels+stable"), "").unwrap();
        let started = Instant::now();
        let (status, _, _) = get(addr, "/channels/stable", None);
        assert!(status.contains(" 200 ") && started.elapsed() >= HOLD_MAX, "{status} {:?}", started.elapsed());
        assert!(!held.exists());
        fs::remove_dir_all(base).unwrap();
    }
}

#[cfg(not(feature = "test-hooks"))]
#[test]
fn without_the_feature_test_hooks_is_refused() {
    let root = std::env::temp_dir().join(format!("httpd-nohooks-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_httpd")).arg(&root).args(["--test-hooks", "/tmp", "--listen", "127.0.0.1:0"]).output().unwrap();
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("unknown flag --test-hooks"), "{}", String::from_utf8_lossy(&out.stderr));
}
