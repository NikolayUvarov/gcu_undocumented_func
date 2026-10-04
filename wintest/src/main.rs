#![no_std]
#![no_main]
// wintest: a test program of the window broker (issue 157), a console program with a plain broker client:
// `show <title> <seconds>` a text window: it writes a tick counter into its surface every 200 ms, reports events and
//     states, and ends when its window is closed or the time is up;
// `intrude` tries manager requests and the surfaces of other programs' windows. The manager side is `winmgr`.
mod common;

use common::{map, text, BROKER, RECEIVE};
use core::fmt::Write;
use mind::abi::BootInfo;
use mind::idl::window::{self as api, Kind};
use mind::util::FixedBuf;
use mind::window::{STATE_CLOSE, STATE_HIDDEN};

mind::request!(REQUEST_CONSOLE | REQUEST_WINDOW);

fn show(title: &str, seconds: u64) {
    let id = match api::create(BROKER, Kind::Text, 24, 2) { Ok(Ok(id)) => id, other => { mind::println!("[WINTEST] CREATE FAILED: {:?}", other); return } };
    let Some((_mapping, surface)) = map(id) else { mind::println!("[WINTEST] NO SURFACE"); return };
    surface.set_title(title);
    mind::println!("[WINTEST] {} WINDOW {}", title, id);
    let (end, mut tick, mut last_state) = (mind::time::uptime_ms() as u64 + seconds * 1000, 0u32, u32::MAX);
    while (mind::time::uptime_ms() as u64) < end {
        tick += 1;
        let mut line = FixedBuf::<64>::new(); let _ = write!(line, "{} TICK {}", title, tick);
        for (x, ch) in text(&line).chars().chain(core::iter::repeat(' ')).take(24).enumerate() { surface.set_cell(x, 0, ch, 0xFFFFFF, 0); }
        surface.changed(Some((0, 0, 24, 1)));
        while let Some(event) = surface.event() { mind::println!("[WINTEST] {} EVENT CHAR {}", title, char::from_u32((event >> 40) as u32).unwrap_or('?')); }
        let state = surface.state();
        if state != last_state { last_state = state; mind::println!("[WINTEST] {} STATE {}", title, match state { STATE_HIDDEN => "HIDDEN", STATE_CLOSE => "CLOSE", _ => "SHOWN" }); }
        if state == STATE_CLOSE { mind::println!("[WINTEST] {} CLOSED AFTER {} TICKS", title, tick); return; }
        mind::time::sleep(200);
    }
    mind::println!("[WINTEST] {} DONE AFTER {} TICKS", title, tick);
}

mind::entry!(main);
fn main(_info: &'static BootInfo) {
    let mut words = mind::process::args_str().split_whitespace();
    match words.next().unwrap_or("") {
        "show" => show(words.next().unwrap_or("W"), words.next().and_then(|s| s.parse().ok()).unwrap_or(30)),
        "intrude" => {
            mind::println!("[WINTEST] INTRUDER ATTACH: {:?}", api::attach(BROKER));
            mind::println!("[WINTEST] INTRUDER LIST: {:?}", api::list(BROKER, 0).map(|r| r.map(|l| l.as_slice().len())));
            for id in 1..=2 { mind::println!("[WINTEST] INTRUDER SURFACE {}: {:?}", id, api::surface(BROKER, id, RECEIVE)); }
        }
        _ => mind::println!("WINTEST SHOW <TITLE> <SECONDS> | INTRUDE"),
    }
}
