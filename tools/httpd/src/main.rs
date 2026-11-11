//! httpd ROOT [--cert CHAIN.pem --key KEY.pem [--allow-rsa]] [--listen ADDR:PORT] [--connections N] [--per-client N] [--per-minute N] [--log DIR] [--log-days N]

use std::net::TcpListener;
use std::path::PathBuf;
use std::process::exit;

fn usage(why: &str) -> ! {
    eprintln!("httpd: {why}\nusage: httpd ROOT [--cert CHAIN.pem --key KEY.pem [--allow-rsa]] [--listen 127.0.0.1:8080] [--connections 64] [--per-client 8] [--per-minute 600] [--log DIR] [--log-days 30]");
    exit(2)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(root) = args.next() else { usage("no root") };
    let root = PathBuf::from(root).canonicalize().unwrap_or_else(|e| usage(&format!("the root: {e}")));
    let mut config = httpd::server::Config::new(root.clone());
    let (mut listen, mut log_dir, mut days, mut cert, mut key, mut rsa) = ("127.0.0.1:8080".to_string(), None, 30u64, None, None, false);
    while let Some(flag) = args.next() {
        let mut value = || args.next().unwrap_or_else(|| usage(&format!("{flag} needs a value")));
        let number = |v: String| v.parse::<u64>().ok().filter(|&n| n > 0).unwrap_or_else(|| usage(&format!("{flag} takes a number above 0")));
        match flag.as_str() {
            "--listen" => listen = value(),
            "--connections" => config.connections = number(value()) as usize,
            "--per-client" => config.per_client = number(value()) as usize,
            "--per-minute" => config.per_minute = number(value()).min(u32::MAX as u64) as u32,
            "--log" => log_dir = Some(PathBuf::from(value())),
            "--log-days" => days = number(value()),
            "--cert" => cert = Some(PathBuf::from(value())),
            "--key" => key = Some(PathBuf::from(value())),
            "--allow-rsa" => rsa = true,
            other => usage(&format!("unknown flag {other}")),
        }
    }
    // The certificate and key are read again when their files change, every 10 s; the key should stay across renewals.
    config.tls = match (cert, key) {
        (Some(cert), Some(key)) => Some(httpd::tls::Reloading::start(cert, key, rsa, std::time::Duration::from_secs(10)).unwrap_or_else(|why| { eprintln!("httpd: {why}"); exit(1) })),
        (None, None) => None,
        _ => usage("--cert and --key go together"),
    };
    let listener = TcpListener::bind(&listen).unwrap_or_else(|e| { eprintln!("httpd: {listen}: {e}"); exit(1) });
    eprintln!("HTTPD: SERVING {} ON {}", root.display(), listener.local_addr().map(|a| a.to_string()).unwrap_or(listen));
    if let Err(e) = httpd::server::run(listener, config, httpd::log::Log::new(log_dir, days)) { eprintln!("httpd: {e}"); exit(1) }
}
