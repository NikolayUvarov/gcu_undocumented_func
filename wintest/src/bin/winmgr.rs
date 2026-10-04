#![no_std]
#![no_main]
// winmgr: a test manager of the window broker (issue 157), a console program with the manager client:
// `manage <seconds>` attaches, lists the windows with their first row and saved place, moves each one, sends it a key,
//     stays, then ends without detaching (the broker detaches it);
// `closeall` closes every window; `second` tries to attach while another manager is attached.
#[path = "../common.rs"]
mod common;

use common::{map, text, BROKER};
use core::fmt::Write;
use mind::abi::{input_event, BootInfo, KEY_CHAR};
use mind::idl::window::{self as api, Placement};
use mind::util::FixedBuf;

mind::request!(REQUEST_CONSOLE | REQUEST_WINDOW_MANAGER);

fn manage(seconds: u64) {
    match api::attach(BROKER) { Ok(Ok(n)) => mind::println!("[WINMGR] MANAGER ATTACHED: {} WINDOWS", n), other => { mind::println!("[WINMGR] ATTACH FAILED: {:?}", other); return } }
    let Ok(Ok(list)) = api::list(BROKER, 0) else { mind::println!("[WINMGR] LIST FAILED"); return };
    let mut leases = [const { None }; 16];
    for (i, w) in list.as_slice().iter().enumerate() {
        let Some((mapping, surface)) = map(w.id) else { mind::println!("[WINMGR] WINDOW {} NO LEASE", w.id); continue };
        mind::time::sleep(300); // let the program draw at least once
        let mut first = FixedBuf::<64>::new();
        for x in 0..24 { let _ = first.write_char(surface.cell(x, 0).map_or('?', |c| c.0)); }
        mind::println!("[WINMGR] WINDOW {} \"{}\" {:?} {}X{} PLACE {},{} ROW \"{}\"", w.id, w.title.as_str(), w.kind, w.width, w.height, w.place.x, w.place.y, text(&first).trim_end());
        let _ = api::place(BROKER, w.id, &Placement { x: 2 * w.id as u16, y: w.id as u16, columns: 26, rows: 4, z: i as u16, minimized: false, maximized: false });
        surface.push_event(input_event(b'k', KEY_CHAR, 0, true, 'k' as u32));
        leases[i] = Some(mapping);
    }
    mind::time::sleep(seconds as usize * 1000);
    mind::println!("[WINMGR] MANAGER LEAVES");
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut words = mind::process::args_str().split_whitespace();
    match words.next().unwrap_or("") {
        "manage" => manage(words.next().and_then(|s| s.parse().ok()).unwrap_or(0)),
        "closeall" => {
            let _ = api::attach(BROKER);
            mind::println!("[WINMGR] CLOSE ALL: {:?}", api::close_all(BROKER));
        }
        "second" => mind::println!("[WINMGR] SECOND MANAGER: {:?}", api::attach(BROKER)),
        _ => mind::println!("WINMGR MANAGE <SECONDS> | CLOSEALL | SECOND"),
    }
}
