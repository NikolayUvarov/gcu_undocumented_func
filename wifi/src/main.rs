#![no_std]
#![no_main]
// wifi (550-APP-0033): the Wi-Fi networks around and joining one, through the 802.11 station's idl/wifi.wit 1.0 (the
// network track's `wlan`), whose configuration client the shell lends in SLOT_WIFI for REQUEST_WIFI (ABI 10). Without
// it the screen says so, and `--demo` runs the program against a stand-in with fixed networks and no radio.
extern crate alloc;

use alloc::string::String;
use mind::abi::{CAP_KIND_ENDPOINT, SLOT_WIFI};
use mind::ipc::Endpoint;
use mind::tui::{Terminal, DARK};
use wifi::service::Service;
use wifi::station::{Absent, Demo, Station};
use wifi::view::{Flow, Wifi};

mind::request!(REQUEST_WIFI);

// How often the link's state is read, and a scan made again, in ms.
const POLL_MS: usize = 500;
const SCAN_MS: usize = 15_000;

mind::entry!(main);
fn main(info: &'static mind::BootInfo) {
    mind::about!("wifi — the Wi-Fi networks around: their names, signal and protection; Enter joins one, with a passphrase typed\nwithout echo that the station may keep (in keystore, never in a file).\nUsage: wifi [--demo]\n↑↓ choose, Enter: join, r or F5: scan again, d: leave, f: forget, q or Esc: quit.\nThe 802.11 station that serves it is the network track's wlan (idl/wifi.wit), whose client the shell lends when a radio\nruns; --demo shows the program with a stand-in station and no radio.");
    let demo = mind::process::args_str().split_whitespace().any(|w| w == "--demo");
    let Some(mut term) = Terminal::open(info, "wifi") else { return };
    let mut stand_in = Demo::new();
    // The station's client, as the shell lends it; without one (no radio, so no station; or a launcher that does not
    // lend it) the screen says so.
    let mut service = Service(Endpoint(SLOT_WIFI));
    let held = mind::dev::cap_info(SLOT_WIFI).0 == CAP_KIND_ENDPOINT;
    let mut absent = Absent(String::from("this program holds no client of the 802.11 station (wlan). The shell lends it when a radio runs: start wifi from the shell, or from wm's Settings, Network."));
    let station: &mut dyn Station = if demo { &mut stand_in } else if held { &mut service } else { &mut absent };
    let mut view = Wifi::new(demo);
    view.refresh(station);
    view.poll(station);
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
