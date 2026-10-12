#![no_std]
#![no_main]
// wifi (550-APP-0033): the Wi-Fi networks around and joining one. The 802.11 station that is to serve it (the network
// track's `wlan`, requests-NET.md) does not run yet: without it the screen says so, and `--demo` runs the program
// against a stand-in station with fixed networks (the QEMU test; a look at the screen), with no radio.
extern crate alloc;

use alloc::string::String;
use mind::tui::{Terminal, DARK};
use wifi::station::{Absent, Demo, Station};
use wifi::view::{Flow, Wifi};

// How often the link's state is read, and a scan made again, in ms.
const POLL_MS: usize = 500;
const SCAN_MS: usize = 15_000;

mind::entry!(main);
fn main(info: &'static mind::BootInfo) {
    mind::about!("wifi — the Wi-Fi networks around: their names, signal and protection; Enter joins one, with a passphrase typed\nwithout echo that the station may keep (in keystore, never in a file).\nUsage: wifi [--demo]\n↑↓ choose, Enter: join, r or F5: scan again, d: leave, f: forget, q or Esc: quit.\nThe 802.11 station that serves it is the network track's (task 550); until it runs, --demo shows the program with a\nstand-in station and no radio.");
    let demo = mind::process::args_str().split_whitespace().any(|w| w == "--demo");
    let Some(mut term) = Terminal::open(info, "wifi") else { return };
    let mut stand_in = Demo::new();
    let mut absent = Absent(String::from("the network track's 802.11 station (wlan, task 550) does not run here yet, so nothing can scan or join."));
    let station: &mut dyn Station = if demo { &mut stand_in } else { &mut absent };
    let mut view = Wifi::new(demo);
    view.refresh(station);
    view.poll(station);
    let mut scanned = mind::time::uptime_ms();
    mind::println!("[WIFI] READY {}x{}{} {}", term.cols(), term.rows(), if demo { " DEMO" } else { "" }, match &view.problem { Some(_) => String::from("NO STATION"), None => view.status() });
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
            }
            continue;
        };
        if let Flow::Quit = view.key(key, station) { break; }
        mind::println!("[WIFI] {}", view.status());
    }
    mind::println!("[WIFI] DONE");
}
