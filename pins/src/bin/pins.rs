#![no_std]
#![no_main]
// pins: the pins of an ARM board through the gpio service (issue u015), a console program. The shell lends its
// control client of `gpio` in SLOT_GPIO for REQUEST_GPIO where a pin controller runs; without it there is nothing to
// show. Reading is open, a change needs the control badge, a pin the board reserves the platform's (idl/gpio.wit).
extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use mind::abi::{CAP_KIND_ENDPOINT, SLOT_GPIO};
use pins::service::Service;
use pins::tool::{self, Command};

mind::request!(REQUEST_CONSOLE | REQUEST_GPIO);

mind::entry!(main);
fn main(_info: &'static mind::BootInfo) {
    mind::about!("pins — the pins of an ARM board through the gpio service: functions, levels, pulls; changes them.\nUsage: pins [-c controller] [pin | set <pin> in|out|alt<k> | write <pin> 0|1 | pull <pin> up|down|none | watch <pin>... [-t seconds]]\nWithout arguments: every pin with its header position, active function, level and pull. A pin number: all its functions,\nthe active one marked *. Names of functions come from hwdocs/ on the boot disk. Changing a pin needs the shell's grant.");
    let options = match tool::parse(mind::process::args_str()) { Ok(options) => options, Err(usage) => { mind::println!("{}", usage); mind::process::exit_with(2) } };
    if mind::dev::cap_info(SLOT_GPIO).0 != CAP_KIND_ENDPOINT {
        mind::println!("pins: no client of the gpio service: it runs only where the firmware names a known pin controller (none on x86 and QEMU), and the shell lends it (start pins there)");
        mind::process::exit_with(1)
    }
    let mut gpio = Service::new();
    if let Command::Watch(list, seconds) = &options.command { watch(&mut gpio, options.controller, list, *seconds) }
    let mut out = String::new();
    let result = tool::run(&options, &mut gpio, &mut out);
    for line in out.lines() { mind::println!("{}", line); }
    if let Err((message, code)) = result { mind::println!("pins: {}", message); mind::process::exit_with(code as u32) }
}

// Levels every 100 ms, a line for each change, for `seconds` or until stopped (Esc in the shell).
fn watch(gpio: &mut Service, controller: u8, list: &[u8], seconds: Option<u32>) -> ! {
    let end = seconds.map(|s| mind::time::uptime_ms() as u64 + s as u64 * 1000);
    let mut before: Option<Vec<(u8, bool)>> = None;
    loop {
        match tool::levels(gpio, controller, list) {
            Ok(now) => { for line in tool::changes(before.as_deref(), &now) { mind::println!("{}", line); } before = Some(now); }
            Err(refusal) => { mind::println!("pins: {}", tool::explain(&refusal, controller, list[0], &Command::Watch(Vec::new(), None))); mind::process::exit_with(1) }
        }
        if end.is_some_and(|end| mind::time::uptime_ms() as u64 >= end) { mind::process::exit_with(0) }
        let _ = mind::time::sleep(100);
    }
}
