#![no_std]
#![no_main]
// wifi (550-APP-0033): the Wi-Fi networks around and joining one, through the 802.11 station's idl/wifi.wit 1.1 (the
// network track's `wlan`), whose configuration client the shell lends in SLOT_WIFI for REQUEST_WIFI (ABI 10). Without
// it the screen says so, and `--demo` runs the program against a stand-in with fixed networks and no radio.
extern crate alloc;

use alloc::string::String;
use mind::abi::{CAP_KIND_ENDPOINT, SLOT_WIFI};
use mind::ipc::Endpoint;
use mind::tui::{Terminal, DARK};
use wifi::headless::{self, Command};
use wifi::service::Service;
use wifi::station::{Absent, Demo, Station};
use wifi::view::{Flow, Wifi};

mind::request!(REQUEST_WIFI);

// How often the link's state is read, and a scan made again, in ms.
const POLL_MS: usize = 500;
const SCAN_MS: usize = 15_000;

mind::entry!(main);
fn main(info: &'static mind::BootInfo) {
    mind::about!("wifi — the Wi-Fi networks around: their names, signal and protection; Enter joins one, with a passphrase typed\nwithout echo that the station may keep (in keystore, never in a file).\nUsage: wifi [--demo] [--countries]\n↑↓ choose, Enter: join, r or F5: scan again, d: leave, f: forget, c: the country (typing finds one), q or Esc: quit;\n--countries opens the country list at once, as wm's Settings, Network, Country does.\nWithout a screen (scripts, tests): wifi join <name> [--passphrase-file <file>] [--remember]: the file's first line is\nthe name, its second the passphrase; it logs JOINED, REFUSED with the station's reason, or NO ANSWER after 60 s, and\nends with 0, 1 or 2 (3: no station); wifi status: the link as one line; wifi leave;\nwifi scan: the networks around, one line each, strongest first, then their count.\nwifi country [<code>]: the country the station runs under, asked for and applied, or a change to it (UZ, DE…: the station\nleaves the network first); wifi countries [<text>]: the catalogue's countries, those whose code or name holds the text;\nwifi channels [<code>]: the radio's channels under a country, or under the one applied.\nThe 802.11 station that serves it is the network track's wlan (idl/wifi.wit), whose client the shell lends when a radio\nruns; --demo shows the program with a stand-in station and no radio.");
    let args = mind::process::args_str();
    let demo = args.split_whitespace().any(|w| w == "--demo");
    // wm's Settings, Network, Country (550-APP-0082): the screen opens at the country list.
    let countries = args.split_whitespace().any(|w| w == "--countries");
    let command = match headless::parse(args) {
        Ok(command) => command,
        Err(usage) => { mind::println!("{}", usage); mind::process::exit_with(headless::NO_STATION) }
    };
    let mut stand_in = Demo::new();
    // The station's client, as the shell lends it; without one (no radio, so no station; or a launcher that does not
    // lend it) the screen says so.
    let mut service = Service(Endpoint(SLOT_WIFI));
    let held = mind::dev::cap_info(SLOT_WIFI).0 == CAP_KIND_ENDPOINT;
    let mut absent = Absent(String::from("this program holds no client of the 802.11 station (wlan). The shell lends it when a radio runs: start wifi from the shell, or from wm's Settings, Network."));
    let station: &mut dyn Station = if demo { &mut stand_in } else if held { &mut service } else { &mut absent };
    if let Some(command) = command { mind::process::exit_with(without_screen(command, station)) }
    let Some(mut term) = Terminal::open(info, "wifi") else { return };
    let mut view = Wifi::new(demo);
    view.windowed = term.screen().is_none();
    view.refresh(station);
    view.poll(station);
    if countries && view.problem.is_none() { view.open_countries(station); }
    let mut scanned = mind::time::uptime_ms();
    mind::println!("[WIFI] READY {}x{}{} {}", term.cols(), term.rows(), if demo { " DEMO" } else { "" }, match &view.problem { Some(_) => String::from("NO STATION"), None => view.status() });
    // The line under the list, in the log too when it changes: the station's answers (never a passphrase).
    let mut said = String::new();
    let mut say = |view: &Wifi| if view.notice != said { said = view.notice.clone(); if !said.is_empty() { mind::println!("[WIFI] NOTICE {}", said); } };
    loop {
        let cursor = { let mut grid = term.grid(); view.draw(&mut grid, &DARK) };
        term.set_cursor(cursor);
        term.present();
        let Some(key) = mind::input::wait_key(POLL_MS) else {
            if view.problem.is_none() {
                let before = view.status();
                view.poll(station);
                if mind::time::uptime_ms() - scanned >= SCAN_MS { view.refresh(station); scanned = mind::time::uptime_ms(); }
                if view.status() != before { mind::println!("[WIFI] {}", view.status()); }
                say(&view);
            }
            continue;
        };
        if let Flow::Quit = view.key(key, station) { break; }
        say(&view);
        mind::println!("[WIFI] {}", view.status());
    }
    mind::println!("[WIFI] DONE");
}

// `wifi join`, `status`, `leave` (550-APP-0078), `scan` (550-APP-0117), `country`, `countries` or `channels` (550-APP-0082): the lines for the
// log and the console, and the exit status.
fn without_screen(command: Command, station: &mut dyn Station) -> u32 {
    let (code, line) = match command {
        Command::Status => headless::status(station),
        Command::Leave => headless::leave(station),
        Command::Scan => headless::networks(station),
        Command::Country(code) => headless::country(station, code.as_deref()),
        Command::Countries(text) => headless::countries(station, text.as_deref()),
        Command::Channels(code) => headless::channels(station, code.as_deref()),
        Command::Join { ssid, file, remember } => {
            let mut secret = match file.as_deref().map(|path| read_passphrase(path, &ssid)) {
                None => String::new(),
                Some(Ok(secret)) => secret,
                Some(Err(why)) => { mind::println!("[WIFI] REFUSED {}: {}", wifi::station::shown(&ssid), why); return headless::REFUSED; }
            };
            let (mut now, mut pause) = (|| mind::time::uptime_ms() as u64, |ms: u64| { let _ = mind::time::sleep(ms as usize); });
            let result = headless::join(station, &ssid, &secret, remember, &mut now, &mut pause);
            unsafe { secret.as_bytes_mut().fill(0); }
            result
        }
    };
    mind::println!("{}", line);
    code
}

// The passphrase from the file at `path` (its first line `ssid`, its second the passphrase); the buffer is cleared.
fn read_passphrase(path: &str, ssid: &[u8]) -> Result<String, &'static str> {
    let mut file = mind::fs::File::open(path).map_err(|_| "the passphrase file cannot be opened")?;
    let mut text = alloc::vec![0u8; file.size().min(1024)];
    let mut got = 0;
    while got < text.len() { match file.read(&mut text[got..]) { Ok(0) | Err(_) => break, Ok(n) => got += n } }
    let secret = headless::passphrase(&text[..got], ssid);
    text.fill(0);
    secret
}

